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
//! [`World::keep_unsaved`] is that part.

use std::collections::BTreeMap;

use formats::savegame::{SaveGame, Source};

use crate::computer::Candidates;
use crate::darstellung::Camera;
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
    /// The view's fields the port doesn't use yet: the forms' windows
    /// (1010:08d7), the main screen's and the other forms' fields, the
    /// tool and the mouse (`1110:5fb2`..), the TV (1048:25ea, but the news
    /// archive, which is the world's), the drawing's tables (1050:313a), the pressed buttons
    /// (`1110:651a`).
    pub unported: Fields,
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
fn event(e: &Event) -> Vec<u8> {
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
        world_field(w, dest, n)
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
    /// version `1.000`, then each unit's globals (`docs/savegame.md`).
    /// Fields neither models are zeros. Saving reads the world and
    /// changes nothing, so it isn't a command: any player may save
    /// their copy. // 1048:2bae
    #[must_use]
    pub fn to_map(&self, view: &MapView) -> SaveGame {
        let mut save = SaveGame::build(&mut Saving { world: self, view });
        // The port's chunk after the original's fields, which the
        // original doesn't read (`docs/savegame.md`).
        save.trailing = crate::enhance::fixes_chunk(self.fixes);
        save
    }

    /// Puts back what `load_map` (1048:2a19) leaves as it was, from
    /// `before`, the world in memory when the file was read: `RandSeed`,
    /// the tables from the CD, and the globals the file doesn't hold.
    /// Each of those is state a saved game loses.
    pub fn keep_unsaved(&mut self, before: &World) {
        // BUG(original): RandSeed isn't saved; a loaded game goes on with the numbers of the game in memory.
        self.random = before.random;
        self.tables = std::sync::Arc::clone(&before.tables);
        self.news.date_format.clone_from(&before.news.date_format);
        // BUG(original): an oil slick going on (1110:0088) isn't saved; the loaded game takes the flag of the one in memory.
        self.oil_slick = before.oil_slick;
        self.finances.frozen = before.finances.frozen;
        // The money menu stays as it is.
        self.money_menu_shown = before.money_menu_shown;
        // BUG(original): the people's lane, 1110:6fce, last building, beach and 1110:3c42 aren't saved.
        let (p, q) = (&mut self.people, &before.people);
        p.lane = q.lane;
        p.word_6fce = q.word_6fce;
        p.last_building = q.last_building;
        p.beach = q.beach;
        p.flag_3c42 = q.flag_3c42;
        // BUG(original): the auction's limits (1110:654e), whether the player joined (65ae) and the form's timers aren't saved.
        let (a, b) = (&mut self.auction, &before.auction);
        a.limits = b.limits;
        a.joined = b.joined;
        a.open = b.open;
        a.opening = b.opening;
        a.sold = b.sold;
        a.bid_pause = b.bid_pause;
        a.since_call = b.since_call;
        a.frames = b.frames;
        // BUG(original): only players 2 to the file's player count get their road candidates; the others keep those in memory.
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
    /// fields over what [`World::keep_unsaved`] keeps.
    ///
    /// # Errors
    /// If the file lacks a field the world needs.
    pub fn load_map(&self, save: &SaveGame) -> Result<World, crate::world::Error> {
        let mut w = World::from_map(save)?;
        w.keep_unsaved(self);
        // A file without the port's chunk leaves the fixes as they are.
        w.set_loaded_fixes(crate::enhance::fixes_in(&save.trailing, self.fixes));
        Ok(w)
    }
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
    use crate::enhance::Fix;
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
        assert_eq!(save.trailing, crate::enhance::fixes_chunk(w.fixes));
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
