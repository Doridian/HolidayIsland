//! Saved games: the world and the view as `save_map` (1048:2bae) writes
//! them and `load_map` (1048:2a19) reads them back. The format, field by
//! field, is `formats::savegame`'s and `docs/savegame.md`'s; the parsing of
//! the world's fields is [`World::from_map`].
//!
//! A map file holds both what the port's [`World`] is and what one
//! player's screen shows: the scroll position, the forms' windows, the
//! tool, the TV's position and page. The world's fields go into the [`World`], the
//! view's into a [`MapView`]. Fields the port doesn't use yet are kept as
//! read ([`Fields`]), on whichever side they belong to, so a loaded game
//! is saved again as it came.
//!
//! `load_map` only overwrites what the file holds. Everything else stays
//! as it was in memory: after a game was loaded from the start menu, as
//! the start of a game set it up (states 1000–1500), and after one was
//! loaded in the middle of a game, as that game left it.
//! [`World::keep_unsaved`] is that part. With `fix.saved_state` the
//! port's chunk keeps that state too (`state.` lines,
//! [`World::to_map`]), and loading takes it from there.

use std::collections::BTreeMap;

use formats::savegame::{SaveGame, Source};

use crate::computer::Candidates;
use crate::darstellung::Camera;
use crate::enhance::Fix;
use crate::events::Event;
use crate::hauptschirm::Hauptschirm;
use crate::world::World;

/// Map fields as they were read, by destination and occurrence (the
/// `n`th field stored at that destination). Fields of zeros aren't kept:
/// a missing one is written as zeros, which is also what a new game has
/// for every field the port doesn't model.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Fields(BTreeMap<(&'static str, usize), Vec<u8>>);

impl Fields {
    /// The `n`th field at `dest`.
    #[must_use]
    pub fn get(&self, dest: &str, n: usize) -> Option<&[u8]> {
        self.0
            .iter()
            .find(|((d, k), _)| *d == dest && *k == n)
            .map(|(_, v)| &v[..])
    }

    fn insert(&mut self, dest: &'static str, n: usize, data: &[u8]) {
        if data.iter().any(|&b| b != 0) {
            self.0.insert((dest, n), data.to_vec());
        }
    }

    /// Sets the `n`th field at `dest` to `data`; zeros take it out, as
    /// they're what a missing field is written as.
    pub fn set(&mut self, dest: &'static str, n: usize, data: &[u8]) {
        if data.iter().any(|&b| b != 0) {
            self.0.insert((dest, n), data.to_vec());
        } else {
            self.0.remove(&(dest, n));
        }
    }

    /// How many fields are kept.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// What a map file holds of the human's view. Single human: the original
/// saves the one player's screen with the world; a multiplayer save would
/// keep one of these per seat, or none.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct MapView {
    /// The main screen's scroll position, `wo` and `ns`. Loading puts it
    /// back, but both ways into a game work it out again from the view's
    /// origin (1020:074d), so the port only saves it.
    /// // [1110:6532]+0x408, +0x40a
    pub scroll: (i16, i16),
    /// Buildings show a pointer in their owner's colour. // 1110:0058
    pub owner_pointers: bool,
    /// Or one for how full they are. // 1110:0059
    pub occupancy_pointers: bool,
    /// Which kinds of news (0 to 3: events, new buildings, sabotage,
    /// congratulations, `crate::news::KINDS`) open the TV: the player's
    /// options (`crate::optionen::Settings::news`), which the original
    /// saves with the game (unit 1000:0c6a). // 1110:005c..1110:005f
    pub news: [bool; 4],
    /// The view's fields the port doesn't use yet, or uses through
    /// the forms that read them: the forms' windows (1010:08d7,
    /// [`Windows`]), the main screen's and the other forms' fields, the
    /// tool and the mouse (`1110:5fb2`..), the TV (1048:25ea, but the news
    /// archive, which is the world's), the drawing's tables (1050:313a), the pressed buttons
    /// (`1110:651a`).
    pub unported: Fields,
    /// With `fix.view_people`: the viewpoint and the people the file gets
    /// instead of the world's, which go by no view: the world's placed
    /// for this view and the view's own among them
    /// (`crate::people::view::ViewPeople::saved_with`), as the original
    /// would have them, so it can load the file.
    pub people: Option<(crate::people::Viewpoint, crate::people::People)>,
    /// The saving player's view origin, which a saved game writes where
    /// the news and the events point at no square (`fix.view_news`,
    /// [`crate::news::NOWHERE`]), as the original would have it; none for
    /// the world's viewpoint (`1110:5530`).
    pub origin: Option<crate::world::Pos>,
}

impl MapView {
    /// The view's part of `save`. // 1048:2a19
    #[must_use]
    pub fn from_map(save: &SaveGame) -> Self {
        let byte = |dest| save.fields(dest).last().and_then(|d| d.first().copied()).unwrap_or(0);
        let scroll = save
            .fields("[1110:6532]+0x408")
            .last()
            .filter(|d| d.len() == 4)
            .map_or((0, 0), |d| {
                (i16::from_le_bytes([d[0], d[1]]), i16::from_le_bytes([d[2], d[3]]))
            });
        let mut unported = Fields::default();
        for (n, f) in save.numbered() {
            if is_view(f.dest) && view_field(&MapView::default(), f.dest).is_none() {
                unported.insert(f.dest, n, &f.data);
            }
        }
        MapView {
            scroll,
            owner_pointers: byte("1110:0058") != 0,
            occupancy_pointers: byte("1110:0059") != 0,
            news: NEWS_OPTIONS.map(|dest| byte(dest) != 0),
            unported,
            people: None,
            origin: None,
        }
    }
}

impl MapView {
    /// The camera the main screen shows `world` with after loading: the
    /// view the world's people code reads, and the file's pointers.
    #[must_use]
    pub fn camera(&self, world: &World) -> Camera {
        Camera {
            view: world.viewpoint.rotation,
            origin: world.viewpoint.origin,
            parity: world.viewpoint.parity,
            owner_pointers: self.owner_pointers,
            occupancy_pointers: self.occupancy_pointers,
        }
    }

    /// What `save_map` writes of the main screen `screen` and the
    /// player's options `settings`, with the view's other fields
    /// `unported` (those of the file the game came from, or none).
    /// // 1048:2bae
    #[must_use]
    pub fn of(screen: &Hauptschirm, settings: &crate::optionen::Settings, mut unported: Fields) -> Self {
        let camera = screen.camera();
        settings.store(&mut unported);
        MapView {
            scroll: screen.scroll_position(),
            owner_pointers: camera.owner_pointers,
            occupancy_pointers: camera.occupancy_pointers,
            news: settings.news,
            unported,
            people: None,
            origin: Some(camera.origin),
        }
    }

    /// [`MapView::of`] for `world`: with `fix.view_people` also the view's
    /// viewpoint and people ([`MapView::people`]).
    #[must_use]
    pub fn of_world(
        screen: &Hauptschirm,
        settings: &crate::optionen::Settings,
        unported: Fields,
        world: &World,
    ) -> Self {
        let viewpoint = screen.viewpoint();
        MapView {
            people: world
                .people_split()
                .then(|| (viewpoint, screen.people.saved_with(world, viewpoint))),
            ..MapView::of(screen, settings, unported)
        }
    }
}

/// The world's fields of `save` the port doesn't use yet. // 1048:2a19
pub(crate) fn unported_world_fields(save: &SaveGame) -> Fields {
    let modelled = World::default();
    let mut fields = Fields::default();
    for (n, f) in save.numbered() {
        let view = is_view(f.dest) || view_field(&MapView::default(), f.dest).is_some();
        if !view && world_field(&modelled, f.dest, n).is_none() {
            fields.insert(f.dest, n, &f.data);
        }
    }
    fields
}

/// The `DGROUP` address of a global (`1110:xxxx`) or of a block a global
/// points to (`*1110:xxxx`).
/// The forms whose windows a map file keeps, in `1110:646c`'s order
/// (set by 1010:064a): 2, the file menu, isn't saved, and 10 and 11 are
/// empty slots, saved as zeros. The pause and the options aren't among
/// them. // 1110:646c
pub const WINDOW_FORMS: [Option<&str>; 17] = [
    Some("Auktionsform"),
    Some("Baumenue"),
    Some("Dateimenue"),
    Some("Endeform"),
    Some("Geldmenuform"),
    Some("Helpform"),
    Some("highscoreform"),
    Some("Infobox2"),
    Some("Kompassform"),
    Some("Fernseher"),
    None,
    None,
    Some("Sabowin"),
    Some("Hauptschirm"),
    Some("Schlussform"),
    Some("umenuform"),
    Some("Uebersichtsmap2"),
];

/// The slots of [`WINDOW_FORMS`]. // 1110:646c
pub mod window {
    pub const AUCTION: usize = 0;
    pub const BUILD_MENU: usize = 1;
    pub const FILE_MENU: usize = 2;
    pub const QUIT_QUESTION: usize = 3;
    pub const MONEY_MENU: usize = 4;
    pub const TOOLTIP: usize = 5;
    pub const HIGH_SCORES: usize = 6;
    pub const INFO_BOX: usize = 7;
    pub const COMPASS: usize = 8;
    pub const TV: usize = 9;
    pub const SABOTAGE: usize = 12;
    pub const MAIN_SCREEN: usize = 13;
    pub const END_FORM: usize = 14;
    pub const BUILDINGS_OVERVIEW: usize = 15;
    pub const OVERVIEW_MAP: usize = 16;
}

/// The forms whose saved `Top`, `Left`, `Width` and `Height`
/// `load_window_positions` reads and drops: the quit question, the
/// compass and the end form, which keep where and how big they are. The
/// set constant at `1110:148e` (`0c 4d 00`, `[2, 3, 8, 10, 11, 14]`, the
/// same in the later build) also holds the file menu and the empty
/// slots, which the loop skips before. // 1010:08d7, en 1000:2cb6
pub const KEEP_PLACE: [usize; 3] = [window::QUIT_QUESTION, window::COMPASS, window::END_FORM];

/// The forms `load_window_positions` doesn't show even when the file has
/// them shown: the TV and the buildings' overview, which it hides
/// instead if they're shown (the file menu isn't read). // 1010:0a65
pub const NEVER_SHOWN: [usize; 2] = [window::TV, window::BUILDINGS_OVERVIEW];

/// The destinations of a window's six fields. // 1010:0799, 1010:08d7
const WINDOW_DESTS: [&str; 6] = [
    "[1110:646c[i]].Top",
    "[1110:646c[i]].Left",
    "[1110:646c[i]].Width",
    "[1110:646c[i]].Height",
    "[1110:646c[i]].Visible",
    "[1110:646c[i]].Enabled",
];

/// A form's window as a map file keeps it: `Top`, `Left`, `Width`,
/// `Height` and the bytes at `+0x29` (`Visible`) and `+0x2a`
/// (`Enabled`). // 1010:0799
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Window {
    pub top: i16,
    pub left: i16,
    pub width: i16,
    pub height: i16,
    pub visible: u8,
    pub enabled: u8,
}

impl Window {
    /// The window on the screen.
    #[must_use]
    pub fn rect(&self) -> vcl::Rect {
        vcl::Rect::new(
            i32::from(self.left),
            i32::from(self.top),
            i32::from(self.width),
            i32::from(self.height),
        )
    }

    /// Its `Visible`.
    #[must_use]
    pub fn shown(&self) -> bool {
        self.visible != 0
    }
}

/// The windows of the 17 forms of [`WINDOW_FORMS`], as a map file keeps
/// them: none for the file menu (2), zeros for the empty slots (10, 11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Windows(pub [Window; 17]);

impl Windows {
    /// The `n`th window of the file, which is form `n` but the file
    /// menu's that isn't saved.
    fn occurrence(form: usize) -> Option<usize> {
        match form {
            window::FILE_MENU => None,
            f if f < window::FILE_MENU => Some(f),
            f => Some(f - 1),
        }
    }

    /// The windows `get` gives the bytes of (a destination of
    /// [`WINDOW_DESTS`] and its occurrence), none if no window has
    /// anything but zeros: a file the port wrote before it kept the
    /// windows, which loading leaves alone. // 1010:08d7
    fn read(get: impl Fn(&str, usize) -> Option<Vec<u8>>) -> Option<Windows> {
        let mut out = Windows::default();
        let mut any = false;
        for (form, w) in out.0.iter_mut().enumerate() {
            let Some(n) = Windows::occurrence(form) else { continue };
            let bytes: Vec<Vec<u8>> = WINDOW_DESTS.iter().map(|d| get(d, n).unwrap_or_default()).collect();
            let word = |k: usize| bytes[k].get(..2).map_or(0, |b| i16::from_le_bytes([b[0], b[1]]));
            let byte = |k: usize| bytes[k].first().copied().unwrap_or(0);
            *w = Window {
                top: word(0),
                left: word(1),
                width: word(2),
                height: word(3),
                visible: byte(4),
                enabled: byte(5),
            };
            any |= *w != Window::default();
        }
        any.then_some(out)
    }

    /// The windows among the view's `fields` (`MapView::unported`).
    #[must_use]
    pub fn from_fields(fields: &Fields) -> Option<Windows> {
        Windows::read(|dest, n| fields.get(dest, n).map(<[u8]>::to_vec))
    }

    /// The windows of `save`.
    #[must_use]
    pub fn from_map(save: &SaveGame) -> Option<Windows> {
        Windows::read(|dest, n| save.fields(dest).nth(n).map(<[u8]>::to_vec))
    }

    /// Puts the windows into the view's `fields` for `save_map`: the file
    /// menu's isn't saved, the empty slots are zeros. // 1010:0799
    pub fn store(&self, fields: &mut Fields) {
        for (form, w) in self.0.iter().enumerate() {
            let Some(n) = Windows::occurrence(form) else { continue };
            let w = if WINDOW_FORMS[form].is_some() {
                *w
            } else {
                Window::default()
            };
            let values = [
                w.top.to_le_bytes().to_vec(),
                w.left.to_le_bytes().to_vec(),
                w.width.to_le_bytes().to_vec(),
                w.height.to_le_bytes().to_vec(),
                vec![w.visible],
                vec![w.enabled],
            ];
            for (dest, v) in WINDOW_DESTS.into_iter().zip(values) {
                fields.set(dest, n, &v);
            }
        }
    }
}

/// Whether `save` has the money menu shown: `load_window_positions`
/// gives the form its saved height, which 1070:2617 asks for. None for
/// a file without windows. // 1010:08d7
#[must_use]
pub fn money_menu_shown(save: &SaveGame) -> Option<bool> {
    Windows::from_map(save).map(|w| w.0[window::MONEY_MENU].height > 0)
}

/// Where `load_window_positions` puts a form whose saved window is
/// `form` on the `screen` the game lays out for: its saved top left,
/// moved up and left as far as it takes to keep its bottom (at least its
/// 16-pixel title bar) and its right edge on the screen. Nothing keeps it
/// off the top or the left edge. // 1010:08d7
///
/// The port also uses it to keep the forms on the screen when the
/// window's size changes.
#[must_use]
pub fn keep_on_screen(form: vcl::Rect, screen: (i32, i32)) -> (i32, i32) {
    let top = (screen.1 - form.height().max(16)).min(form.top);
    let left = (screen.0 - form.width()).min(form.left);
    (left, top)
}

fn global(dest: &str) -> Option<u16> {
    let hex = dest.strip_prefix('*').unwrap_or(dest).strip_prefix("1110:")?;
    u16::from_str_radix(hex, 16).ok()
}

/// The view's globals, besides those of the ranges in [`is_view`]: the
/// view's size (`1110:12bc`, `12be`), the road tool's drag (`6344`,
/// `6348`), the forms' `Enabled` flags (`645a`), the button bar's pressed
/// buttons (`651a`), the auction form's state (`6598`, `659a`), the TV's
/// first button (`662e`, `crate::fernseher`) and the player's options
/// that [`crate::optionen::Settings`] keeps besides the news and the
/// pointers: the music (`005b`), the tooltips (`0060`), the sounds
/// (`0061`), the year's balance (`0064`) and the speech (`0066`).
const VIEW_GLOBALS: [u16; 14] = [
    0x12bc, 0x12be, 0x6344, 0x6348, 0x645a, 0x651a, 0x6598, 0x659a, 0x662e, 0x005b, 0x0060, 0x0061, 0x0064, 0x0066,
];

/// The news options, `MapView::news`. // 1110:005c..1110:005f
const NEWS_OPTIONS: [&str; 4] = ["1110:005c", "1110:005d", "1110:005e", "1110:005f"];

/// The world's globals of unit 1048:25ea, among the TV's: the news
/// archive (`*1110:663a`), its last entry (`6664`) and whether anything
/// was posted yet (`667c`) (`crate::news`).
const NEWS_GLOBALS: [u16; 3] = [0x663a, 0x6664, 0x667c];

/// Whether the field at `dest` is the view's. The forms' fields are,
/// except the clock's counters in the main screen's (`+0x1d6`, `+0x1d8`),
/// the date in `TMonate` (`1110:65d6`), the books in the money menu's
/// (`+0x191`, `+0x24b`) and the build menu's lists (`*[1110:6516]+0x180`,
/// `+0x184`, `+0x1c6`), which the world keeps. So are the globals of the
/// main screen's tools and mouse (`1110:5fb2` to `1110:6090`), the TV
/// (`1110:6638` to `667c`, unit 1048:25ea, but [`NEWS_GLOBALS`]) and the
/// drawing (`1110:667e` to `66c6`, unit 1050:313a).
fn is_view(dest: &str) -> bool {
    const WORLD_FORM_FIELDS: [&str; 10] = [
        "[1110:6532]+0x1d6",
        "[1110:6532]+0x1d8",
        "[1110:65d6]+0x4",
        "[1110:65d6]+0x5",
        "[1110:65d6]+0x6",
        "[1110:6f04]+0x191",
        "*[1110:6f04]+0x24b",
        // The build menu's lists (`crate::build_menu`).
        "*[1110:6516]+0x180",
        "*[1110:6516]+0x184",
        "[1110:6516]+0x1c6",
    ];
    if dest.starts_with('[') || dest.starts_with("*[1110:6") || dest.starts_with("*[1110:7") {
        // The computers' lists (`*[1110:70e2[i]]`, `*[1110:70e6[i]]`) are
        // the world's.
        return !WORLD_FORM_FIELDS.contains(&dest) && !dest.starts_with("*[1110:70e");
    }
    global(dest).is_some_and(|a| {
        (0x5fb2..=0x6090).contains(&a)
            || ((0x6638..=0x66c6).contains(&a) && !NEWS_GLOBALS.contains(&a))
            || VIEW_GLOBALS.contains(&a)
    })
}

fn word(v: i16) -> Vec<u8> {
    v.to_le_bytes().to_vec()
}

fn longint(v: i32) -> Vec<u8> {
    v.to_le_bytes().to_vec()
}

fn longints(v: &[i32]) -> Vec<u8> {
    v.iter().flat_map(|n| n.to_le_bytes()).collect()
}

fn flag(b: bool) -> Vec<u8> {
    vec![u8::from(b)]
}

/// A `string`: the length, the characters, zeros to 256 bytes.
fn pstring(s: &[u8]) -> Vec<u8> {
    let s = &s[..s.len().min(255)];
    let mut out = vec![0; 256];
    out[0] = u8::try_from(s.len()).unwrap_or(u8::MAX);
    out[1..=s.len()].copy_from_slice(s);
    out
}

/// An event's record. // *1110:6536, 0x34 bytes
pub(crate) fn event(e: &Event) -> Vec<u8> {
    let mut r = longints(&[
        e.chance,
        e.first_day,
        e.last_day,
        e.running,
        e.duration,
        e.value,
        e.start,
        e.end,
        e.column_8,
        e.building,
        e.kind,
        e.arg,
    ]);
    r.extend_from_slice(&e.pos.0.to_le_bytes());
    r.extend_from_slice(&e.pos.1.to_le_bytes());
    r
}

/// The bytes the world has for the `n`th field at `dest`, `None` for a
/// field it doesn't model. The inverse of [`World::from_map`].
#[allow(clippy::too_many_lines, clippy::many_single_char_names)] // One field after the other.
fn world_field(w: &World, dest: &str, n: usize) -> Option<Vec<u8>> {
    let island = &w.island;
    let f = &w.finances;
    let s = &w.settlement;
    let c = &w.computers;
    let p = &w.people;
    let a = &w.auction;
    let pos = |(i, j): (i16, i16)| [i.to_le_bytes(), j.to_le_bytes()].concat();
    let squares = |v: &[(i16, i16)]| v.iter().flat_map(|&q| pos(q)).collect::<Vec<u8>>();
    let words = |v: &[u16]| v.iter().flat_map(|n| n.to_le_bytes()).collect::<Vec<u8>>();
    let integers = |v: &[i16]| v.iter().flat_map(|n| n.to_le_bytes()).collect::<Vec<u8>>();
    let column = |k: usize| longints(&s.factors.iter().map(|r| r[k]).collect::<Vec<_>>());
    Some(match dest {
        // Unit 1000:0c6a.
        "1110:0067" => flag(w.clock.paused),
        "1110:009e" => word(w.clock.slice),
        "1110:0054" => flag(w.options.animations),
        "1110:0055" => flag(w.options.people_move),
        "1110:0056" => flag(w.options.people_spawn),
        "1110:0057" => flag(w.computers_play),
        "1110:0063" => flag(w.options.auction_waits),
        // Unit 1048:25ea: the news archive (`crate::news`).
        "1110:6664" => word(w.news.last),
        "1110:667c" => flag(w.news.nothing_yet),
        "*1110:663a" => w.news.archive_bytes(),
        "1110:0065" => flag(w.disasters),
        "1110:0074" => w.player_colours.to_vec(),
        "1110:007c" => vec![w.viewpoint.rotation],
        "1110:0086" => flag(w.flood),
        "1110:0087" => flag(w.earthquake),
        "1110:008a" => word(w.island_settings[0]),
        "1110:008e" => word(w.island_settings[1]),
        "1110:0090" => word(w.island_settings[2]),
        "1110:0092" => word(w.island_settings[3]),
        "1110:0096" => longint(w.clock.speed),
        "1110:00a2" => c.height_limit.to_le_bytes().to_vec(),
        "1110:0902" => (0..9)
            .flat_map(|i| pstring(w.player_names.get(i).map_or(&[][..], |n| &n[..])))
            .collect(),
        "*1110:5500" => island.heights.bytes().to_vec(),
        "*1110:5504" => island.ground.bytes().to_vec(),
        "*1110:5508" => island.buildings.bytes().to_vec(),
        "*1110:550c" => island.forms.bytes().to_vec(),
        "*1110:5510" => island.parts.bytes().to_vec(),
        "*1110:5514" => island.owners.bytes().to_vec(),
        "*1110:5518" => island.occupancy.bytes().to_vec(),
        "*1110:551c" => island.layer_551c.bytes().to_vec(),
        "*1110:5520" => island.layer_5520.bytes().to_vec(),
        "*1110:5524" => island.location.bytes().to_vec(),
        "*1110:5528" => island.plot_map.bytes(),
        // `plot_count`, read again by the plots' list.
        "1110:552c" => u16::try_from(w.plots.len()).unwrap_or(u16::MAX).to_le_bytes().to_vec(),
        // Saved twice, both times the view's origin and parity.
        "1110:5530" => pos(w.viewpoint.origin),
        "1110:5534" => word(w.viewpoint.parity),
        "1110:008c" => word(w.difficulty),
        "1110:634c" => flag(w.road_dragging),
        "1110:704e" => vec![w.road_drag_ground],
        "1110:704f" => vec![w.road_drag_form],
        "*[1110:6516]+0x180" | "*[1110:6516]+0x184" | "[1110:6516]+0x1c6" => return w.build_menu.field(dest),
        "1110:006e" => vec![w.scenario],
        "1110:006f" => flag(w.cheated),
        "1110:0071" => flag(w.won),
        "1110:0072" => flag(w.insured),
        // Unit 1068:32b5: the lists. `SaveGame::build` writes their counts
        // and takes the records from [`world_records`].
        "1110:6f1a" | "1110:6f1c" | "1110:6f18" | "*1110:6f0c" | "*1110:6f10" | "*1110:6f14" | "*1110:6f08" => {
            Vec::new()
        }
        // Unit 1010:0d4c.
        "1110:133c" => flag(w.played_without_disasters),
        "1110:1479" => vec![w.player_count],
        "1110:1334" => w.renovation_years.to_vec(),
        "1110:64bf" => vec![w.max_location_effect],
        "1110:64c2" => longint(w.renovation_cost),
        "1110:64c6" => longint(s.advertising_cost),
        "1110:64ca" => longint(f.credit_size),
        "1110:64ce" => longint(f.max_debt),
        "1110:64d2" => longint(f.interest_rate),
        "1110:64d6" => longint(f.start_money),
        "1110:64da" => longint(w.land_costs.from_water),
        "1110:64de" => longint(w.land_costs.raise),
        "1110:64e2" => longint(w.land_costs.into_water),
        "1110:64e6" => longint(w.land_costs.lower),
        "1110:64ea" => longint(w.road_demolition_cost),
        "1110:64f2" => longint(w.land_costs.palm),
        "1110:64f6" => longint(w.foundation_cost),
        "1110:64fa" => longint(s.living_costs),
        "1110:64fe" => longint(s.mix_divisor),
        "*1110:6502" => s.class_mix.iter().flat_map(|row| integers(row)).collect(),
        "1110:6508" => w.connection_values.to_vec(),
        "1110:650e" => s.decay.to_vec(),
        "*1110:6512" => longints(&w.computer_budgets),
        // Unit 1008:539f.
        "1110:004c" => vec![s.base_demand],
        "1110:4a88" => w.accounting.last_day.to_le_bytes().to_vec(),
        "1110:4a84" => longint(w.accounting.step),
        "1110:4a8a" => longint(s.min_rent),
        "1110:4a8e" => longint(s.max_rent),
        "1110:4a92" => longint(s.rent_range),
        "1110:4a96" => longints(&s.min_price),
        // `MaxPreis` has 11 entries in the file; the 11th is the sums'
        // first.
        "1110:4ac2" => [longints(&s.max_price), longint(s.sums[0])].concat(),
        "1110:4aee" => longints(&s.sums[1..]),
        "*1110:4c6e" => column(0),
        "*1110:4c72" => column(1),
        "*1110:4c76" => column(2),
        "*1110:4c7a" => column(3),
        // Unit 1088:8a07.
        "1110:3c2e" => p.directions.to_vec(),
        "*1110:6fd6" => p.records.iter().flat_map(|r| r.0).collect(),
        "*1110:6fda" => p.paths.clone(),
        "*1110:6fde" => p.entries.clone(),
        "*1110:6fe2" => p.tracks.clone(),
        "1110:6ff4" => word(p.last),
        "1110:6fee" => word(p.ships),
        "1110:6ff0" => word(p.boats),
        // Unit 1028:2506.
        "1110:6548" => word(a.bidder),
        "1110:654a" => longint(a.price),
        "1110:6596" => a.colour.to_le_bytes().to_vec(),
        "1110:656e" => word(a.item),
        "1110:6599" => flag(a.held),
        "1110:659c" => longint(a.start_price),
        "1110:65a0" => a.going.to_le_bytes().to_vec(),
        "1110:65a2" => longint(a.ticks),
        "1110:65a6" => longint(a.wait),
        // Unit 10a8:32ec.
        "1110:7062" => words(&c.waited),
        "1110:7072" => words(&c.demolish_wait),
        "1110:7082" => words(&c.levelling),
        "1110:7092" => c.level.to_vec(),
        "1110:709a" => c.blocked_by.to_vec(),
        "*1110:70a2" => squares(&c.site),
        "*1110:70a6" => squares(&c.centre),
        "*1110:70aa" => squares(&c.road),
        "*1110:70ae" => c.road_direction.to_vec(),
        "*1110:70b2" => integers(&c.unknown[0]),
        "*1110:70b6" => integers(&c.unknown[1]),
        "*1110:70ba" => integers(&c.unknown[2]),
        "*1110:70be" => integers(&c.unknown[3]),
        "1110:70c2" => longint(w.computer_turns),
        "*1110:70c6" => c.build_state.to_vec(),
        "*1110:70ca" => c.state.to_vec(),
        "*1110:70ce" => c.category.to_vec(),
        "*1110:70d2" => c.new_area.to_vec(),
        "*1110:70d6" => c.first_road.to_vec(),
        "*1110:70da" => c.building.to_vec(),
        "*1110:70de" => c.counters.iter().flatten().copied().collect(),
        // Three lists for each of players 2 to the player count.
        "*[1110:70e2[i]]" => candidates(&c.road_ends, n),
        "*[1110:70e6[i]]" => candidates(&c.roads, n),
        "*1110:70ea" => c.types.clone(),
        "*1110:70ee" => c.compbau.iter().flatten().copied().collect(),
        "1110:70f2" => pstring(&c.status),
        // Unit 1010:3e8a.
        "*1110:6536" => w.events.iter().flat_map(event).collect(),
        "1110:653a" => longint(w.days),
        // Unit 1070:3bf8.
        "[1110:6f04]+0x191" => longints(&w.books.year),
        "*[1110:6f04]+0x24b" => w.books.history.clone(),
        // Unit 1030:2bc5.
        "[1110:65d6]+0x4" => vec![w.date.day],
        "[1110:65d6]+0x5" => vec![w.date.month],
        "[1110:65d6]+0x6" => w.date.year.to_le_bytes().to_vec(),
        // Unit 1020:2b1d.
        "[1110:6532]+0x1d6" => w.clock.hour_ticks.to_le_bytes().to_vec(),
        "[1110:6532]+0x1d8" => w.clock.day_ticks.to_le_bytes().to_vec(),
        // Unit 1078:34ad.
        "*1110:6f20" => w.totals.clone(),
        "*1110:6f24" => w.player_block.clone(),
        "1110:6f28" => f.credit_refused.map(u8::from).to_vec(),
        "1110:6f30" => longints(&f.money),
        "1110:6f50" => longints(&f.loan),
        "1110:6f70" => longints(&f.interest),
        "1110:6f90" => longints(&f.guest_count),
        "1110:6fb0" => word(w.economy_periods[0]),
        "1110:6fb2" => word(w.economy_periods[1]),
        // Unit 10a0:23c9.
        "*1110:705a" => w.sabotage.iter().flat_map(crate::sabotage::Sabotage::record).collect(),
        // `save_map` writes this block as zeros (`FillChar`, 10a0:232f),
        // `load_map` reads it into a local and drops it.
        "(a local in 10a0:23c9)" => vec![0; 0x15e],
        "1110:7050" => longint(w.damage.amount),
        "1110:7054" => flag(w.damage.insured),
        "1110:7056" => pos(w.damage.slick),
        "*1110:705e" => w
            .championships
            .iter()
            .copied()
            .flat_map(crate::sabotage::Championship::record)
            .collect(),
        _ => return None,
    })
}

/// List `n % 3` of computer player `2 + n / 3`'s candidates.
fn candidates(lists: &[Candidates], n: usize) -> Vec<u8> {
    lists.get(n / 3).map_or_else(Vec::new, |c| c.list(n % 3).to_vec())
}

/// The records of the list at `dest`, in `save_map`'s order: from the
/// list's head, so the building sites go out newest first. // 1068:32df
fn world_records(w: &World, dest: &str) -> Vec<Vec<u8>> {
    match dest {
        "*1110:6f0c" => w.plots.iter().map(|p| p.record().to_vec()).collect(), // 1068:3244
        "*1110:6f10" => w.houses.iter().map(|h| h.record().to_vec()).collect(), // 1068:2f73
        "*1110:6f14" => w.facilities.iter().map(|f| f.record().to_vec()).collect(), // 1068:2fe1
        "*1110:6f08" => w.building_sites.iter().map(|s| s.record().to_vec()).collect(), // 1068:30d0
        _ => Vec::new(),
    }
}

/// The bytes the view has for the field at `dest`, `None` for one it
/// doesn't model.
fn view_field(v: &MapView, dest: &str) -> Option<Vec<u8>> {
    if let Some((viewpoint, people)) = &v.people {
        let saved = match dest {
            "*1110:6fd6" => Some(people.records.iter().flat_map(|r| r.0).collect()),
            "1110:6ff4" => Some(word(people.last)),
            "1110:007c" => Some(vec![viewpoint.rotation]),
            "1110:5530" => Some([viewpoint.origin.0.to_le_bytes(), viewpoint.origin.1.to_le_bytes()].concat()),
            "1110:5534" => Some(word(viewpoint.parity)),
            _ => None,
        };
        if saved.is_some() {
            return saved;
        }
    }
    Some(match dest {
        "1110:0058" => flag(v.owner_pointers),
        "1110:0059" => flag(v.occupancy_pointers),
        "1110:005c" => flag(v.news[0]),
        "1110:005d" => flag(v.news[1]),
        "1110:005e" => flag(v.news[2]),
        "1110:005f" => flag(v.news[3]),
        "[1110:6532]+0x408" => [v.scroll.0.to_le_bytes(), v.scroll.1.to_le_bytes()].concat(),
        _ => return None,
    })
}

/// [`SaveGame::build`]'s source for [`World::to_map`].
struct Saving<'a> {
    world: &'a World,
    view: &'a MapView,
}

impl Source for Saving<'_> {
    fn field(&mut self, dest: &'static str, n: usize, _size: usize) -> Vec<u8> {
        let (w, v) = (self.world, self.view);
        // The view's people and viewpoint first (`MapView::people`).
        let people = if v.people.is_some() { view_field(v, dest) } else { None };
        // The news and events that point nowhere, at the saving player's
        // view (`fix.view_news`).
        let here = v.origin.unwrap_or(w.viewpoint.origin);
        people
            .or_else(|| w.somewhere_bytes(dest, here))
            .or_else(|| world_field(w, dest, n))
            .or_else(|| view_field(v, dest))
            .or_else(|| w.unported.get(dest, n).map(<[u8]>::to_vec))
            .or_else(|| v.unported.get(dest, n).map(<[u8]>::to_vec))
            .unwrap_or_default()
    }

    fn records(&mut self, dest: &'static str) -> Vec<Vec<u8>> {
        world_records(self.world, dest)
    }
}

impl World {
    /// The map file `save_map` writes of this world and `view`: the
    /// version (the later build's `1.100`, v1.0's `1.000`), then each
    /// unit's globals (`docs/savegame.md`).
    /// Fields neither models are zeros. Saving reads the world and
    /// changes nothing, so it isn't a command: any player may save
    /// their copy. // 1048:2bae
    #[must_use]
    pub fn to_map(&self, view: &MapView) -> SaveGame {
        let mut save = SaveGame::build(&mut Saving { world: self, view });
        // The port's chunk after the original's fields, which the
        // original doesn't read (`docs/savegame.md`); with
        // `fix.saved_state` also what the original's fields lose.
        // The seats (`crate::seats`) and the news and events that point
        // nowhere (`fix.view_news`), which the original has no fields for.
        let mut state = self.seats.chunk_lines();
        state.extend(self.nowhere_lines());
        if self.fixes.has(Fix::SavedState) {
            state.extend(self.unsaved_state(view));
        }
        save.trailing = crate::enhance::world_chunk_with(self.fixes, self.language, &state);
        save
    }

    /// The port's chunk's `state.` lines (`fix.saved_state`): what
    /// `load_map` (1048:2a19) leaves as it was in memory
    /// ([`World::keep_unsaved`]), as `save_map` (1048:2bae) doesn't
    /// write it, and of the people the file gets with `view`
    /// ([`MapView::people`]), what differs from the world's: the door
    /// people's buildings (`+0x22`) and the last record. Numbers are
    /// decimal, a flag `0` or `1`, a square `i,j`, a timer that doesn't
    /// run `-`, a byte list hex without its trailing zeros.
    fn unsaved_state(&self, view: &MapView) -> Vec<(String, String)> {
        let mut out = Vec::new();
        let mut put = |key: &str, value: String| out.push((format!("{STATE}{key}"), value));
        let flag = |b: bool| if b { "1" } else { "0" }.to_owned();
        let pos = |(i, j): (i16, i16)| format!("{i},{j}");
        let timer = |t: Option<u16>| t.map_or_else(|| "-".to_owned(), |n| n.to_string());
        put("rand_seed", self.random.seed.to_string()); // 1110:4a06
        put("oil_slick", flag(self.oil_slick)); // 1110:0088
        let p = &self.people;
        put("people.lane", p.lane.to_string()); // 1110:6fcc
        put("people.6fce", p.word_6fce.to_string());
        put("people.last_building", p.last_building.to_string()); // 1110:6ff2
        put("people.beach", pos(p.beach)); // 1110:7040, 1110:7042
        put("people.3c42", flag(p.flag_3c42));
        // What the file's people (the view's, with `fix.view_people`)
        // don't have of the world's.
        if let Some((viewpoint, saved)) = &view.people {
            put("people.view_rotation", viewpoint.rotation.to_string());
            if saved.last != p.last {
                put("people.last", p.last.to_string()); // 1110:6ff4
            }
            let homes: Vec<String> = p
                .records
                .iter()
                .zip(&saved.records)
                .enumerate()
                .filter(|(_, (w, f))| w.kind() != crate::people::NOBODY && w.home() != f.home())
                .map(|(n, (w, _))| format!("{n}:{}", pos(w.home())))
                .collect();
            if !homes.is_empty() {
                put("people.homes", homes.join(" ")); // +0x22
            }
            // The world's free records the view's own took in the file:
            // what they held before (`take_saved_free_people`).
            let free: Vec<String> = p
                .records
                .iter()
                .zip(&saved.records)
                .enumerate()
                .filter(|(_, (w, f))| w.kind() == crate::people::NOBODY && w != f)
                .map(|(n, (w, _))| format!("{n}:{}", hex(&w.0)))
                .collect();
            if !free.is_empty() {
                put("people.free", free.join(" "));
            }
        }
        let a = &self.auction;
        put("auction.limits", join(&a.limits)); // 1110:654e
        put("auction.joined", flag(a.joined)); // 1110:65ae
        put("auction.open", flag(a.open));
        put("auction.opening", timer(a.opening));
        put("auction.sold", timer(a.sold));
        put("auction.bid_pause", a.bid_pause.to_string());
        put("auction.since_call", a.since_call.to_string()); // 1110:65aa
        put("auction.frames", join(&a.frames.values()));
        // The road candidates of the players past the file's count
        // (unit 10a8:32ec saves players 2 to the count).
        let c = &self.computers;
        let saved = usize::from(self.player_count.saturating_sub(1));
        for (k, (ends, roads)) in c.road_ends.iter().zip(&c.roads).enumerate().skip(saved) {
            let lists = |l: &Candidates| (0..3).map(|n| hex(l.list(n))).collect::<Vec<_>>().join(",");
            put(&format!("computers.{}.road_ends", k + 2), lists(ends)); // *[1110:70e2[i]]
            put(&format!("computers.{}.roads", k + 2), lists(roads)); // *[1110:70e6[i]]
        }
        out
    }

    /// With `fix.saved_state` on in this world just loaded: each value of
    /// the port's chunk's `state.` lines in `trailing`
    /// ([`World::unsaved_state`]) in place of what
    /// [`World::keep_unsaved`] kept of the game in memory. A line that's
    /// missing (an original file, a v1.0 one, an older port's) or that
    /// doesn't read leaves that.
    fn take_saved_state(&mut self, trailing: &[u8]) {
        if !self.fixes.has(Fix::SavedState) {
            return;
        }
        let state: BTreeMap<String, String> = formats::savegame::port_chunk(trailing)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(k, v)| Some((k.strip_prefix(STATE)?.to_owned(), v)))
            .collect();
        let get = |key: &str| state.get(key).map(String::as_str);
        let flag = |key: &str| match get(key) {
            Some("1") => Some(true),
            Some("0") => Some(false),
            _ => None,
        };
        let timer = |key: &str| match get(key)? {
            "-" => Some(None),
            n => n.parse().ok().map(Some),
        };
        if let Some(seed) = num(get("rand_seed")) {
            self.random.seed = seed;
        }
        if let Some(b) = flag("oil_slick") {
            self.oil_slick = b;
        }
        let p = &mut self.people;
        set(&mut p.lane, num(get("people.lane")));
        set(&mut p.word_6fce, num(get("people.6fce")));
        set(&mut p.last_building, num(get("people.last_building")));
        set(&mut p.beach, get("people.beach").and_then(square));
        set(&mut p.flag_3c42, flag("people.3c42"));
        if self.people_split() && get("people.view_rotation").is_some() {
            self.take_saved_people(get("people.homes"), num(get("people.last")));
        }
        let a = &mut self.auction;
        set(&mut a.limits, get("auction.limits").and_then(numbers));
        set(&mut a.joined, flag("auction.joined"));
        set(&mut a.open, flag("auction.open"));
        set(&mut a.opening, timer("auction.opening"));
        set(&mut a.sold, timer("auction.sold"));
        set(&mut a.bid_pause, num(get("auction.bid_pause")));
        set(&mut a.since_call, num(get("auction.since_call")));
        set(
            &mut a.frames,
            get("auction.frames")
                .and_then(numbers)
                .map(crate::auction::Frames::from_values),
        );
        // Only those the file's fields don't hold.
        let saved = usize::from(self.player_count.saturating_sub(1));
        let c = &mut self.computers;
        for (k, (ends, roads)) in c.road_ends.iter_mut().zip(&mut c.roads).enumerate().skip(saved) {
            set(
                ends,
                get(&format!("computers.{}.road_ends", k + 2)).and_then(candidates_of),
            );
            set(
                roads,
                get(&format!("computers.{}.roads", k + 2)).and_then(candidates_of),
            );
        }
    }

    /// With `fix.saved_state` and `fix.view_people`, once the views have
    /// taken their own people from a world just loaded
    /// (`World::hand_over_view_people`): the world's free records they
    /// took in the file get back what they held, from the port's chunk in
    /// `trailing` (`state.people.free`). The file menu's load
    /// (`crate::command::Action::Load`) and the start menu's (after
    /// [`World::split_off_view_people`]) do it.
    pub fn take_saved_free_people(&mut self, trailing: &[u8]) {
        if !self.fixes.has(Fix::SavedState) || !self.people_split() {
            return;
        }
        let Some(free) = formats::savegame::port_chunk(trailing)
            .unwrap_or_default()
            .into_iter()
            .find_map(|(k, v)| (k == format!("{STATE}people.free")).then_some(v))
        else {
            return;
        };
        for (n, bytes) in free.split_whitespace().filter_map(|r| {
            let (n, bytes) = r.split_once(':')?;
            let mut person = crate::people::Person::default();
            let len = person.0.len();
            person.0.copy_from_slice(&unhex(bytes, len)?);
            Some((n.parse::<usize>().ok()?, person))
        }) {
            if let Some(p) = self.people.records.get_mut(n)
                && p.kind() == crate::people::NOBODY
                && bytes.kind() == crate::people::NOBODY
            {
                *p = bytes;
            }
        }
    }

    /// With `fix.view_people`, for a file whose people are a view's
    /// (`state.people.view_rotation`): the world's door people get their
    /// buildings' squares (`+0x22`) back from `homes`, everyone is placed
    /// again for the world's rotation from them (1088:7a14, and the
    /// flying planes' pictures, as `people_for_view` placed them for the
    /// view), and the last record is `last`, when nothing but the views'
    /// people is after it.
    fn take_saved_people(&mut self, homes: Option<&str>, last: Option<i16>) {
        for (n, home) in homes.into_iter().flat_map(str::split_whitespace).filter_map(|h| {
            let (n, home) = h.split_once(':')?;
            Some((n.parse::<usize>().ok()?, square(home)?))
        }) {
            if let Some(p) = self.people.records.get_mut(n)
                && p.kind() != crate::people::NOBODY
            {
                p.set_home(home);
            }
        }
        self.place_people_again_from_home();
        if let Some(last) = last
            && let Ok(from) = usize::try_from(last)
            && from < self.people.records.len()
            && self.people.records[from + 1..]
                .iter()
                .all(|p| p.kind() == crate::people::NOBODY || crate::people::is_view_kind(p.kind()))
        {
            self.people.last = last;
        }
    }

    /// Puts back what `load_map` (1048:2a19) leaves as it was, from
    /// `before`, the world in memory when the file was read: `RandSeed`,
    /// the tables from the CD, and the globals the file doesn't hold.
    /// Each of those is state a saved game loses; with `fix.saved_state`
    /// the port's chunk keeps the marked ones, and [`World::load_map`]
    /// takes them from it after this.
    pub fn keep_unsaved(&mut self, before: &World) {
        // BUG(original): RandSeed isn't saved; a loaded game goes on with the numbers of the game in memory (fix.saved_state: the chunk's).
        self.random = before.random;
        self.tables = std::sync::Arc::clone(&before.tables);
        self.folder_language = before.folder_language;
        // BUG(original): an oil slick going on (1110:0088) isn't saved; the loaded game takes the flag of the one in memory (fix.saved_state: the chunk's).
        self.oil_slick = before.oil_slick;
        self.finances.frozen = before.finances.frozen;
        // A file without windows (the port's before it kept them) leaves
        // the money menu as it is; [`World::load_map`] gives it the
        // file's.
        self.money_menu_shown = before.money_menu_shown;
        // BUG(original): the people's lane, 1110:6fce, last building, beach and 1110:3c42 aren't saved (fix.saved_state: the chunk's).
        let (p, q) = (&mut self.people, &before.people);
        p.lane = q.lane;
        p.word_6fce = q.word_6fce;
        p.last_building = q.last_building;
        p.beach = q.beach;
        p.flag_3c42 = q.flag_3c42;
        // BUG(original): the auction's limits (1110:654e), whether the player joined (65ae) and the form's timers aren't saved (fix.saved_state: the chunk's).
        let (a, b) = (&mut self.auction, &before.auction);
        a.limits = b.limits;
        a.joined = b.joined;
        a.open = b.open;
        a.opening = b.opening;
        a.sold = b.sold;
        a.bid_pause = b.bid_pause;
        a.since_call = b.since_call;
        a.frames = b.frames;
        // BUG(original): only players 2 to the file's player count get their road candidates; the others keep those in memory (fix.saved_state: the chunk's).
        let loaded = usize::from(self.player_count.saturating_sub(1));
        for (k, (ends, roads)) in before
            .computers
            .road_ends
            .iter()
            .zip(&before.computers.roads)
            .enumerate()
            .skip(loaded)
        {
            if let Some(c) = self.computers.road_ends.get_mut(k) {
                c.clone_from(ends);
            }
            if let Some(c) = self.computers.roads.get_mut(k) {
                c.clone_from(roads);
            }
        }
    }

    /// The world a map file loads into while `self` is the game in memory,
    /// as loading a game from the file menu does (1040:0f27): the file's
    /// fields over what [`World::keep_unsaved`] keeps, and with
    /// `fix.saved_state` (the file's, or for a file without the port's
    /// chunk this game's) what the chunk's `state.` lines keep of that.
    ///
    /// # Errors
    /// If the file lacks a field the world needs.
    pub fn load_map(&self, save: &SaveGame) -> Result<World, crate::world::Error> {
        let mut w = World::from_map(save)?;
        w.keep_unsaved(self);
        // The money menu has the height of its saved window
        // (`load_window_positions`, 1010:08d7).
        if let Some(shown) = money_menu_shown(save) {
            w.money_menu_shown = shown;
        }
        // A file without the port's chunk leaves the fixes as they are,
        // and takes the game folder's language.
        w.set_loaded_fixes(crate::enhance::fixes_in(&save.trailing, self.fixes));
        w.language = crate::lang::language_in(&save.trailing).unwrap_or(self.folder_language);
        // The running game's clients stay at their seats, the file's
        // others come back away, and the fixes follow their number.
        w.seats = self.seats.loaded(crate::seats::Seats::from_chunk(&save.trailing));
        w.follow_loaded_seats();
        // With `fix.saved_state`, what the chunk keeps of the rest.
        w.take_saved_state(&save.trailing);
        Ok(w)
    }
}

/// The prefix of the port's chunk's lines for the world's state the
/// original's file loses (`fix.saved_state`, [`World::unsaved_state`]).
const STATE: &str = "state.";

/// Numbers, comma-separated.
fn join<T: ToString>(v: &[T]) -> String {
    v.iter().map(ToString::to_string).collect::<Vec<_>>().join(",")
}

/// A number of the chunk.
fn num<T: std::str::FromStr>(v: Option<&str>) -> Option<T> {
    v?.parse().ok()
}

/// `N` numbers of the chunk, comma-separated.
fn numbers<T: std::str::FromStr + Copy + Default, const N: usize>(v: &str) -> Option<[T; N]> {
    let mut out = [T::default(); N];
    let mut parts = v.split(',');
    for o in &mut out {
        *o = parts.next()?.trim().parse().ok()?;
    }
    parts.next().is_none().then_some(out)
}

/// A square of the chunk, `i,j`.
fn square(v: &str) -> Option<(i16, i16)> {
    numbers::<i16, 2>(v).map(|[i, j]| (i, j))
}

/// `slot` becomes `value`, if there is one.
fn set<T>(slot: &mut T, value: Option<T>) {
    if let Some(v) = value {
        *slot = v;
    }
}

/// Bytes as hex, without the trailing zeros.
fn hex(bytes: &[u8]) -> String {
    let end = bytes.iter().rposition(|&b| b != 0).map_or(0, |n| n + 1);
    bytes[..end].iter().fold(String::new(), |mut out, b| {
        use std::fmt::Write as _;
        let _ = write!(out, "{b:02x}");
        out
    })
}

/// [`hex`]'s bytes, zeros to `len`.
fn unhex(v: &str, len: usize) -> Option<Vec<u8>> {
    if !v.len().is_multiple_of(2) || v.len() / 2 > len {
        return None;
    }
    let mut out = (0..v.len() / 2)
        .map(|n| u8::from_str_radix(v.get(2 * n..2 * n + 2)?, 16).ok())
        .collect::<Option<Vec<u8>>>()?;
    out.resize(len, 0);
    Some(out)
}

/// A computer's road candidates of the chunk: three [`hex`] lists.
fn candidates_of(v: &str) -> Option<Candidates> {
    let lists = v.split(',').map(|l| unhex(l.trim(), 256)).collect::<Option<Vec<_>>>()?;
    if lists.len() != 3 {
        return None;
    }
    Candidates::from_fields(&lists.iter().map(Vec::as_slice).collect::<Vec<_>>())
}

/// The names of the fields in which `a` and `b` differ: for tests, and
/// to tell what a desync or a lossy reload changed.
#[must_use]
pub fn differences(a: &World, b: &World) -> Vec<&'static str> {
    let mut out = Vec::new();
    macro_rules! same {
        ($f:ident) => {
            if a.$f != b.$f {
                out.push(stringify!($f));
            }
        };
    }
    same!(island);
    same!(player_colours);
    same!(player_count);
    same!(player_names);
    same!(difficulty);
    same!(island_settings);
    same!(clock);
    same!(date);
    same!(finances);
    same!(accounting);
    same!(days);
    same!(disasters);
    same!(earthquake);
    same!(flood);
    same!(oil_slick);
    same!(insured);
    same!(damage);
    same!(events);
    same!(auction);
    same!(options);
    same!(plots);
    same!(houses);
    same!(facilities);
    same!(building_sites);
    same!(computer_budgets);
    same!(books);
    same!(renovation_years);
    same!(renovation_cost);
    same!(scenario);
    same!(cheated);
    same!(won);
    same!(played_without_disasters);
    same!(player_block);
    same!(sabotage);
    same!(championships);
    same!(max_location_effect);
    same!(road_demolition_cost);
    same!(connection_values);
    same!(totals);
    same!(economy_periods);
    same!(people);
    same!(settlement);
    same!(computers_play);
    same!(computer_turns);
    same!(computers);
    same!(foundation_cost);
    same!(land_costs);
    same!(road_dragging);
    same!(road_drag_ground);
    same!(road_drag_form);
    same!(build_menu);
    same!(viewpoint);
    same!(tables);
    same!(random);
    same!(unported);
    same!(money_menu_shown);
    same!(news);
    same!(fixes);
    same!(seats);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_kept_on_screen() {
        // 1010:08d7: up and left until the bottom (at least 16 rows) and
        // the right edge are on the screen; never down or right.
        let r = |l, t, w, h| vcl::Rect::new(l, t, w, h);
        assert_eq!(keep_on_screen(r(700, 500, 200, 100), (800, 600)), (600, 500));
        assert_eq!(keep_on_screen(r(700, 590, 200, 0), (800, 600)), (600, 584));
        assert_eq!(keep_on_screen(r(-5, -5, 200, 100), (800, 600)), (-5, -5));
        assert_eq!(keep_on_screen(r(320, 272, 640, 479), (800, 600)), (160, 121));
    }

    #[test]
    fn windows_in_the_fields() {
        // Nothing but zeros: no windows.
        assert_eq!(Windows::from_fields(&Fields::default()), None);
        let mut w = Windows::default();
        for (form, x) in w.0.iter_mut().enumerate() {
            let n = i16::try_from(form).unwrap();
            *x = Window {
                top: n,
                left: 100 + n,
                width: 200 + n,
                height: -n,
                visible: 1,
                enabled: u8::try_from(form % 2).unwrap(),
            };
        }
        let mut fields = Fields::default();
        w.store(&mut fields);
        // The file menu's isn't saved: the 4th window is form 4's.
        assert_eq!(fields.get("[1110:646c[i]].Left", 3), Some(&104i16.to_le_bytes()[..]));
        // 16 windows, the empty slots as zeros.
        assert_eq!(fields.get("[1110:646c[i]].Top", 9), None);
        assert!(fields.get("[1110:646c[i]].Top", 15).is_some());
        let back = Windows::from_fields(&fields).unwrap();
        for form in 0..17 {
            let expected = if [window::FILE_MENU, 10, 11].contains(&form) {
                Window::default()
            } else {
                w.0[form]
            };
            assert_eq!(back.0[form], expected, "form {form}");
        }
        assert_eq!(back.0[window::MONEY_MENU].rect(), vcl::Rect::new(104, 4, 204, -4));
    }

    use crate::world::{BuildingSite, Facility, House, Plot};

    #[test]
    fn view_and_world_fields() {
        assert!(is_view("[1110:646c[i]].Top"));
        assert!(is_view("[1110:6532]+0x408"));
        assert!(!is_view("[1110:6532]+0x1d6"));
        assert!(!is_view("*[1110:6f04]+0x24b"));
        assert!(is_view("*[1110:6f04]+0x24b0"));
        assert!(!is_view("*[1110:6516]+0x184"));
        assert!(is_view("[1110:6516]+0x1ae") && is_view("*[1110:6516]+0x1ed"));
        assert!(!is_view("*[1110:70e2[i]]"));
        assert!(is_view("1110:6056") && is_view("1110:6638") && is_view("1110:651a"));
        assert!(!is_view("*1110:663a") && !is_view("1110:6664"));
        assert!(view_field(&MapView::default(), "1110:005c").is_some() && is_view("1110:0064"));
        assert!(!is_view("1110:0058") && !is_view("1110:6f30") && !is_view("(a local in 10a0:23c9)"));
    }

    /// A world with a bit of everything, through a file and back.
    #[test]
    fn round_trip() {
        let mut w = World {
            player_names: (0..9).map(|i| format!("name{i}").into_bytes()).collect(),
            player_count: 3,
            ..World::default()
        };
        w.island.heights.set((5, 6), 7);
        w.island.plot_map.set((5, 6), 1);
        w.plots = vec![Plot::new(1)];
        w.houses = vec![House::new((5, 6))];
        w.facilities = vec![Facility::new(41, (7, 7))];
        w.building_sites = vec![
            BuildingSite {
                pos: (1, 2),
                days: 3,
                building: 4,
                ..BuildingSite::default()
            },
            BuildingSite {
                pos: (9, 9),
                days: 1,
                building: 5,
                budget: -1,
                ..BuildingSite::default()
            },
        ];
        w.finances.money = [1, -2, 3, 4, 5, 6, 7, 8];
        w.events = vec![Event::default(); crate::events::EVENTS];
        w.events[3].pos = (-1, 4);
        w.computers.status = b"busy".to_vec();
        w.island_settings = [0, 4, 1, 9];
        w.random = vcl::random::Random::new(77);
        w.oil_slick = true;
        let view = MapView {
            scroll: (240, 237),
            owner_pointers: true,
            news: [true, false, true, false],
            ..MapView::default()
        };
        let save = w.to_map(&view);
        let bytes = save.to_bytes();
        let parsed = SaveGame::parse(&bytes).unwrap();
        assert_eq!(parsed, save);
        assert_eq!(MapView::from_map(&parsed), view);
        let mut back = World::from_map(&parsed).unwrap();
        // Loading lists the buildings on their plots.
        assert_eq!(back.plots[0].houses, [(5, 6)]);
        back.keep_unsaved(&w);
        // The building sites come back the other way round.
        back.building_sites.reverse();
        let mut expected = w.clone();
        expected.plots[0].houses = vec![(5, 6)];
        assert_eq!(back.unported, Fields::default());
        assert_eq!(differences(&back, &expected), Vec::<&str>::new());
        assert!(back.unported.is_empty());
        // The port's chunk follows the original's fields.
        assert!(bytes.ends_with(&save.trailing));
        assert_eq!(
            save.trailing,
            crate::enhance::world_chunk_with(w.fixes, w.language, &w.seats.chunk_lines())
        );
        // A file without it loads over a game and keeps that game's fixes.
        let mut original = parsed.clone();
        original.trailing.clear();
        let mut playing = w.clone();
        playing.fixes = crate::enhance::Fixes::all_on();
        assert_eq!(playing.load_map(&original).unwrap().fixes, playing.fixes);
        assert_eq!(
            World::from_map(&original).unwrap().fixes,
            crate::enhance::Fixes::ORIGINAL
        );
        // The world's language goes with the file; a file without it
        // takes the game folder's.
        let en = crate::lang::Language::En;
        let mut english = w.clone();
        english.language = en;
        let save = english.to_map(&view);
        assert_eq!(World::from_map(&save).unwrap().language, en);
        assert_eq!(w.load_map(&save).unwrap().language, en);
        playing.language = crate::lang::Language::De;
        playing.folder_language = en;
        let loaded = playing.load_map(&original).unwrap();
        assert_eq!((loaded.language, loaded.folder_language), (en, en));
    }

    /// A world with state the original's file doesn't hold, and another
    /// game to load it into.
    fn unsaved_worlds() -> (World, World) {
        let mut w = World {
            player_names: (0..9).map(|i| format!("name{i}").into_bytes()).collect(),
            player_count: 3,
            fixes: crate::enhance::Fixes::all_on(),
            ..World::default()
        };
        w.events = vec![Event::default(); crate::events::EVENTS];
        w.random = vcl::random::Random::new(0x1234_5678);
        w.oil_slick = true;
        let p = &mut w.people;
        (p.lane, p.word_6fce, p.last_building, p.beach, p.flag_3c42) = (2, -3, 77, (40, 41), true);
        p.last = 0x40;
        let a = &mut w.auction;
        a.limits = [1, -2, 3, 400_000, 5, 6, 7, i32::MAX];
        (a.joined, a.open, a.opening, a.sold, a.bid_pause, a.since_call) = (true, true, None, Some(12), 3, 9);
        a.frames = crate::auction::Frames::from_values([1, 2, 3, 4, 5, 1, 0, 6, 2]);
        // Players 2 and 3 are in the file's fields; 4 to 8 aren't.
        for (k, c) in w.computers.road_ends.iter_mut().enumerate() {
            c.push(1, (10 + i16::try_from(k).unwrap(), 20));
        }
        w.computers.roads[4].push(3, (200, 201));
        let mut other = World {
            player_count: 3,
            ..World::default()
        };
        other.random = vcl::random::Random::new(99);
        other.people.beach = (-1, -1);
        other.auction.limits = [9; 8];
        other.auction.sold = None;
        other.computers.road_ends[5].push(2, (1, 1));
        (w, other)
    }

    #[test]
    fn fix_saved_state() {
        let (w, other) = unsaved_worlds();
        let save = w.to_map(&MapView::default());
        let parsed = SaveGame::parse(&save.to_bytes()).unwrap();
        // Loaded over another game, it's the game that was saved.
        let back = other.load_map(&parsed).unwrap();
        assert_eq!(differences(&back, &w), Vec::<&str>::new());
        // From the start menu's set-up too (`load_saved_game`).
        assert_eq!(
            differences(&World::default().load_map(&parsed).unwrap(), &w),
            Vec::<&str>::new()
        );

        // A file without the chunk: the game in memory's, as the original.
        let mut original = parsed.clone();
        original.trailing.clear();
        let mut playing = other.clone();
        playing.fixes = crate::enhance::Fixes::all_on();
        let back = playing.load_map(&original).unwrap();
        let mut expected = back.clone();
        expected.keep_unsaved(&playing);
        assert_eq!(differences(&back, &expected), Vec::<&str>::new());
        assert_eq!((back.random, back.oil_slick), (other.random, false));
        assert_eq!(back.computers.road_ends[5], other.computers.road_ends[5]);
        // The players in the file keep the file's.
        assert_eq!(back.computers.road_ends[1], w.computers.road_ends[1]);

        // A line missing or that doesn't read falls back to that; one the
        // port doesn't know is skipped.
        let mut lines = parsed.port_chunk().unwrap();
        lines.retain(|(k, _)| k != "state.rand_seed");
        for (k, v) in &mut lines {
            if k == "state.oil_slick" {
                *v = "x".to_owned();
            }
        }
        lines.push(("state.from_the_future".to_owned(), "1".to_owned()));
        let mut edited = parsed.clone();
        edited.trailing = formats::savegame::make_port_chunk(lines.iter().map(|(k, v)| (k.as_str(), v.as_str())));
        let back = other.load_map(&edited).unwrap();
        assert_eq!(differences(&back, &w), ["oil_slick", "random"]);
        assert_eq!((back.random, back.oil_slick), (other.random, other.oil_slick));

        // Off, the file has none of it, and loads as the original's.
        let mut off = w.clone();
        off.fixes.set(Fix::SavedState, false);
        let save = off.to_map(&MapView::default());
        assert!(save.port_chunk().unwrap().iter().all(|(k, _)| !k.starts_with(STATE)));
        let back = other.load_map(&save).unwrap();
        let mut expected = back.clone();
        expected.keep_unsaved(&other);
        assert_eq!(differences(&back, &expected), Vec::<&str>::new());
        assert_eq!(back.random, other.random);
        // A file with the state loaded with the fix off in its chunk
        // (by hand: the port writes none then) ignores the state.
        let mut lines = parsed.port_chunk().unwrap();
        for (k, v) in &mut lines {
            if k == "fix.saved_state" {
                *v = "0".to_owned();
            }
        }
        edited.trailing = formats::savegame::make_port_chunk(lines.iter().map(|(k, v)| (k.as_str(), v.as_str())));
        assert_eq!(other.load_map(&edited).unwrap().random, other.random);
    }

    /// With `fix.view_people` the file's people are the view's; the chunk
    /// gives the world its door people's buildings, its last record and
    /// what the records the view's own took held.
    #[test]
    fn fix_saved_state_view_people() {
        let (mut w, other) = unsaved_worlds();
        let door = 0x30;
        let p = &mut w.people.records[door];
        p.set_kind(0x1a);
        p.set_home((30, 31));
        p.set_pos((31, 32));
        // A free record with what someone who left left there.
        let free = 0x41;
        w.people.records[free].set_pos((7, 8));
        let mut saved = w.people.clone();
        saved.records[door].set_home((0, 0));
        saved.records[door].set_pos((29, 31));
        // A view's own builder in it, the last record.
        saved.records[free] = crate::people::Person::default();
        saved.records[free].set_kind(0x26);
        saved.last = 0x41;
        let view = MapView {
            people: Some((
                crate::people::Viewpoint {
                    rotation: 2,
                    ..Default::default()
                },
                saved,
            )),
            ..MapView::default()
        };
        let load = |w: &World| {
            let mut back = other.clone();
            let tick = back.apply(crate::command::Command {
                player: 1,
                action: crate::command::Action::Load(std::sync::Arc::new(w.to_map(&view))),
            });
            assert_eq!(tick.view_people.len(), 1);
            back
        };
        let back = load(&w);
        // (Placed again from it: on a real island, where the door is
        // the building's, that's where it stood; `orig_savegame.rs`.)
        assert_eq!(back.people.records[door].home(), (30, 31));
        assert_eq!(back.people.records[free], w.people.records[free]);
        assert_eq!(back.people.last, 0x40);
        // Without the fix, the file's.
        w.fixes.set(Fix::SavedState, false);
        let back = load(&w);
        assert_eq!(back.people.records[door].home(), (0, 0));
        assert_eq!(back.people.records[free].pos(), (0, 0));
        assert_eq!(back.people.last, 0x41);
    }

    #[test]
    fn fix_load_order() {
        let mut w = World::default();
        for pos in [(5, 6), (6, 6), (7, 6)] {
            w.island.heights.set(pos, 1);
            w.island.plot_map.set(pos, 1);
        }
        w.plots = vec![Plot::new(1)];
        w.houses = vec![House::new((5, 6)), House::new((6, 6))];
        w.facilities = vec![Facility::new(41, (7, 6)), Facility::new(41, (7, 7))];
        w.plots[0].houses = vec![(5, 6), (6, 6)];
        w.plots[0].facilities = vec![(7, 6)];
        let site = |building| BuildingSite {
            building,
            ..BuildingSite::default()
        };
        w.building_sites = vec![site(4), site(5), site(6)];
        let sites = |w: &World| w.building_sites.iter().map(|s| s.building).collect::<Vec<_>>();
        for fixed in [false, true] {
            w.fixes.set(Fix::LoadOrder, fixed);
            let save = w.to_map(&MapView::default());
            let back = World::from_map(&save).unwrap();
            if fixed {
                assert_eq!(sites(&back), [4, 5, 6]);
                assert_eq!(back.plots[0].houses, [(5, 6), (6, 6)]);
            } else {
                // The original turns them round.
                assert_eq!(sites(&back), [6, 5, 4]);
                assert_eq!(back.plots[0].houses, [(6, 6), (5, 6)]);
            }
            // An original file loaded over a game with the fix.
            let mut original = save.clone();
            original.trailing.clear();
            let mut playing = World::default();
            playing.fixes.set(Fix::LoadOrder, fixed);
            let back = playing.load_map(&original).unwrap();
            assert_eq!(sites(&back) == [4, 5, 6], fixed);
            assert_eq!(back.plots[0].houses == [(5, 6), (6, 6)], fixed);
        }
    }
}
