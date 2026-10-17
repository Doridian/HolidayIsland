//! `TUebersichtsmap2`, the overview map (segment `10b8`): the whole
//! island from above, two pixels a square, in one of six colourings, with
//! a frame where the main screen looks. The button bar's sixth button
//! (5, or the keys `6`, `U` and `Ü`) opens and closes it (1010:2c8b).
//!
//! The form is five `TImage`s laid out by `UEBERMAP.DAT`, in a big and a
//! small size: the title bar (`Ueberschriftbild`, with the close button and
//! the size button), the colouring's toggle (`Umschaltbild`, heights or
//! slopes), the vertical scroll bar (`NSScrollbild`), the map (`MapBild`)
//! and the menu below it (`Menubild`: the horizontal scroll bar, five
//! buttons for the colourings and "Schließen"). Unlike the other forms it
//! uses no widgets of the game's toolkit: everything is hit-tested against
//! the layout's rectangles (10b8:02e9).
//!
//! The map is a window, [`Uebersichtsmap::map_size`] big, onto a picture of
//! [`MAP_WIDTH`] × [`MAP_HEIGHT`] that the original never draws whole: it
//! draws the window, scrolls it by copying and draws the strips that come
//! into view (10b8:1a6e). A left click or drag on it moves the main
//! screen there (10b8:37f5), which comes back as [`Response::scroll`], the
//! main screen's new scroll position; a right click centres the map on the
//! point.
//!
//! All of this is the view's, saved with the map's view fields
//! (10b8:4e34). The one thing that reaches the world is the scroll
//! arrows': while one is held, no new people come (10b8:21c7).

// Pascal's `div` by 2, kept as it's written.
#![allow(clippy::manual_midpoint)]

use formats::data::{Rect as DataRect, UebermapData};
use vcl::Rect;
use vcl::surface::{Rop, Surface};

use crate::beach_life::BEACH_LIFE;
use crate::command::Action;
use crate::enhance::{Enhancement, Enhancements};
use crate::optionen::WorldOption;
use crate::savegame::Fields;
use crate::simulation::Tick;
use crate::text::{Align, Font};
use crate::widgets::ShiftState;
use crate::world::{NO_BUILDING, PALM, Pos, World, on_map};

/// The bitmaps the form draws with, from `GRAFIK`, in the order
/// [`Uebersichtsmap::new`] takes them: the form's own (10b8:4538), then
/// the game's (1030:307a).
pub const BITMAP_FILES: [&str; 21] = [
    "Ueberma2.BMP", // the sea's tile and the colours, +0x1d2
    "UeRecht.BMP",  // the main screen's frame, +0x1d6
    "uebigueb.BMP", // the title bar, big, +0x1da
    "uesmaueb.BMP", // small, +0x1de
    "uebigpre.BMP", // the size button held down, small, +0x1e2
    "uesmapre.BMP", // big, +0x1e6
    "uehoebut.BMP", // the toggle with slopes shown, +0x1ea
    "ueschbut.BMP", // with heights shown, +0x1ee
    "uehoepre.BMP", // held down, +0x1f2
    "ueschpre.BMP", // +0x1f6
    "uebigmen.BMP", // the menu, big, +0x1fa
    "uesmamen.BMP", // small, +0x1fe
    "uemenpre.BMP", // its buttons held down, +0x202
    "uebigns.BMP",  // the scroll bar, big, +0x206
    "uesmans.BMP",  // small, +0x20a
    "closepre.bmp", // 1110:54bc
    "scroll1.BMP",  // the thumbs, 1110:6016
    "scrbutno.BMP", // the up arrow held down, 1110:6022
    "scrbutsu.BMP", // down, 1110:6026
    "scrbutwe.BMP", // left, 1110:602a
    "scrbutos.BMP", // right, 1110:602e
];
const COLOURS: usize = 0;
const FRAME: usize = 1;
const TITLE_BIG: usize = 2;
const TITLE_SMALL: usize = 3;
const TO_BIG_PRESSED: usize = 4;
const TO_SMALL_PRESSED: usize = 5;
const TO_HEIGHTS: usize = 6;
const TO_SLOPES: usize = 7;
const TO_HEIGHTS_PRESSED: usize = 8;
const TO_SLOPES_PRESSED: usize = 9;
const MENU_BIG: usize = 10;
const MENU_SMALL: usize = 11;
const MENU_PRESSED: usize = 12;
const NS_BIG: usize = 13;
const NS_SMALL: usize = 14;
const CLOSE_PRESSED: usize = 15;
const SCROLL: usize = 16;
const UP_PRESSED: usize = 17;
const DOWN_PRESSED: usize = 18;
const LEFT_PRESSED: usize = 19;
const RIGHT_PRESSED: usize = 20;

/// The whole map's picture: `wo` and `ns` of the main screen, doubled and
/// as they are, with room for the frame. // 10b8:3553
pub const MAP_WIDTH: i32 = 0x3ea;
pub const MAP_HEIGHT: i32 = 0x1f6;

/// The rectangles of a layout (`UEBERMAP.DAT`'s `TUebermapding`), in the
/// form or in the image they're part of.
pub mod part {
    /// The form.
    pub const FORM: usize = 0;
    /// `Ueberschriftbild`.
    pub const TITLE: usize = 1;
    /// In the title bar.
    pub const CLOSE: usize = 2;
    /// In the title bar: big or small.
    pub const SIZE: usize = 3;
    /// `Umschaltbild`.
    pub const TOGGLE: usize = 4;
    /// `NSScrollbild`.
    pub const NS: usize = 5;
    /// In the scroll bar.
    pub const NS_UP: usize = 6;
    pub const NS_TROUGH: usize = 7;
    pub const NS_DOWN: usize = 8;
    /// `MapBild`.
    pub const MAP: usize = 9;
    /// `Menubild`.
    pub const MENU: usize = 10;
    /// In the menu: the horizontal scroll bar.
    pub const WO: usize = 11;
    pub const WO_LEFT: usize = 12;
    pub const WO_TROUGH: usize = 13;
    pub const WO_RIGHT: usize = 14;
    /// In the menu: the five colourings' buttons.
    pub const BUTTONS: usize = 15;
    /// The first of them, owners; then buildings, location, green and
    /// occupancy (`UEBesitzermapbutton`..).
    pub const OWNERS: usize = 16;
    pub const OCCUPANCY: usize = 20;
    /// "Schließen" (`UESchliessenbutton`).
    pub const SCHLIESSEN: usize = 21;
}

/// The colourings (+0x199). Each of the first five has a button; pressing
/// the one that's down shows [`TERRAIN`].
pub mod mode {
    /// Buildings in their owner's colour.
    pub const OWNERS: u8 = 0;
    /// Buildings by their kind (`HAUSDATA.DAT` column 5).
    pub const BUILDINGS: u8 = 1;
    /// The location quality.
    pub const LOCATION: u8 = 2;
    /// Palms, parks and rivers.
    pub const GREEN: u8 = 3;
    /// How full the buildings are.
    pub const OCCUPANCY: u8 = 4;
    /// The land alone.
    pub const TERRAIN: u8 = 5;
}

/// The colourings an area redraw is for, a bit for each mode
/// (10b8:1684's last argument).
const ALL_MODES: u8 = 0x3f;
const OCCUPANCY_MODE: u8 = 0x10;
const LOCATION_MODE: u8 = 4;

/// The button down in the menu (+0x198) when none is.
const NO_BUTTON: u8 = 10;

/// `play_sound`'s numbers.
const PRESS: u8 = 8;
const SELECT: u8 = 9;
const RELEASE: u8 = 10;
const OPEN_SOUND: u8 = 0xd;
const CLOSE_SOUND: u8 = 0xe;

/// The keys that scroll the window, both codes of each, and by how much
/// down and right. // 10b8:4c95
const SCROLL_KEYS: [(u16, u16, i32, i32); 8] = [
    (0x23, 0x61, 0x14, -0x14),  // End
    (0x28, 0x62, 0x14, 0),      // Down
    (0x22, 0x63, 0x14, 0x14),   // Page Down
    (0x25, 0x64, 0, -0x14),     // Left
    (0x27, 0x66, 0, 0x14),      // Right
    (0x24, 0x67, -0x14, -0x14), // Home
    (0x26, 0x68, -0x14, 0),     // Up
    (0x21, 0x69, -0x14, 0x14),  // Page Up
];

/// The help texts of the colourings' buttons, for the tooltips (1020:1fe2).
/// // 1110:1636
pub const HELP_TEXTS: [u8; 5] = [0x18, 0x19, 0x1a, 0x1b, 0x1c];

/// The main screen's frame on the map for each rotation of the view: its
/// size, where its mask and picture are in `UeRecht.BMP`, how far up and
/// left of the main screen's scroll position it's drawn, and where in it
/// a click puts the scroll position. // 1110:3d80
struct FrameShape {
    size: (i32, i32),
    mask: (i32, i32),
    picture: (i32, i32),
    offset: (i32, i32),
    click: (i32, i32),
}

/// `big_map`'s frame's outline: the colour of the frame's outline in
/// `UeRecht.BMP`'s pictures, over its mask's black.
const BIG_FRAME_COLOUR: u8 = 16;
/// `big_map`'s frame's mark for the screen's bottom edge, by rotation:
/// the part of each [`FRAMES`] picture beside its box (mask, picture,
/// size), below it, left of it, above it and right of it.
const BIG_FRAME_MARKS: [[(i32, i32); 3]; 4] = [
    [(0, 25), (40, 25), (40, 15)],
    [(0, 130), (0, 155), (15, 25)],
    [(0, 40), (40, 40), (40, 15)],
    [(40, 80), (40, 105), (15, 25)],
];

const FRAMES: [FrameShape; 4] = [
    FrameShape {
        size: (40, 40),
        mask: (0, 0),
        picture: (40, 0),
        offset: (0, 0),
        click: (20, 12),
    },
    FrameShape {
        size: (55, 25),
        mask: (0, 130),
        picture: (0, 155),
        offset: (15, 4),
        click: (20, 8),
    },
    FrameShape {
        size: (40, 40),
        mask: (0, 40),
        picture: (40, 40),
        offset: (0, 15),
        click: (20, 12),
    },
    FrameShape {
        size: (55, 25),
        mask: (0, 80),
        picture: (0, 105),
        offset: (0, -1),
        click: (20, 13),
    },
];

/// `free_squares`: no building, a palm or beach life. // 1110:1358
const FREE_SQUARES: [u8; 3] = [PALM, BEACH_LIFE, NO_BUILDING];
/// `not_buildings`: those and the bridges' 11 and 52. // 1110:1378
const NOT_BUILDINGS: [u8; 5] = [11, PALM, BEACH_LIFE, 52, NO_BUILDING];

/// A value into an `Integer`, saturating (the port's values fit).
fn short(n: i32) -> i16 {
    i16::try_from(n).unwrap_or(if n < 0 { i16::MIN } else { i16::MAX })
}

/// A scroll bar's range and position (+0x1c0 max, +0x1c2 min, +0x1c4
/// position for the vertical one, +0x1c6.. for the horizontal one).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Bar {
    max: i16,
    min: i16,
    position: i16,
}

impl Bar {
    fn range(self) -> i32 {
        i32::from(self.max) - i32::from(self.min)
    }

    /// Where the thumb starts on a bar whose arrow is 16 pixels and whose
    /// thumb travels to `travel`. // 10b8:1e96, 10b8:25cb
    fn thumb(self, travel: i32) -> i32 {
        let range = self.range();
        // BUG(original): a bar without range divides by zero; the port puts the thumb at the start.
        if range == 0 {
            return 16;
        }
        (range * 16 + (i32::from(self.position) - i32::from(self.min)) * (travel - 16)) / range
    }

    /// The position for the thumb at `at`. // 10b8:1f52, 10b8:26a8
    fn position_at(self, at: i32, travel: i32) -> i32 {
        let span = travel - 16;
        if span == 0 {
            return i32::from(self.min);
        }
        (span * i32::from(self.min) + self.range() * (at - 16)) / span
    }
}

/// One of the form's images.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Image {
    /// `Ueberschriftbild`
    Title,
    /// `Umschaltbild`
    Toggle,
    /// `NSScrollbild`
    Scroll,
    /// `MapBild`
    Map,
    /// `Menubild`
    Menu,
}

impl Image {
    /// The `TImage`'s name in the DFM.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Image::Title => "Ueberschriftbild",
            Image::Toggle => "Umschaltbild",
            Image::Scroll => "NSScrollbild",
            Image::Map => "MapBild",
            Image::Menu => "Menubild",
        }
    }

    fn part(self) -> usize {
        match self {
            Image::Title => part::TITLE,
            Image::Toggle => part::TOGGLE,
            Image::Scroll => part::NS,
            Image::Map => part::MAP,
            Image::Menu => part::MENU,
        }
    }

    const ALL: [Image; 5] = [Image::Title, Image::Toggle, Image::Scroll, Image::Menu, Image::Map];
}

/// What an event did.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[must_use]
pub struct Response {
    /// An image changed.
    pub refresh: bool,
    /// The sounds played, in order (`play_sound`, 10a0:30b6).
    pub sounds: Vec<u8>,
    /// The main screen's new scroll position, `wo` and `ns`
    /// (`THauptschirm+0x408`, `+0x40a`): set them, the scroll bars and
    /// the origin (`hauptschirm_update_scroll`, 1020:05a7) and draw the
    /// island (1050:2700). They aren't clamped to the scroll bars.
    pub scroll: Option<(i16, i16)>,
    /// Draw the main screen's island again (1050:2700).
    pub draw_main: bool,
    /// The form closed (10b8:3f7f): the button bar's button 5 comes up
    /// (1010:2b37) and the main screen gets the focus.
    pub closed: bool,
    /// The main screen gets the focus (Space).
    pub focus_main: bool,
    /// The key goes on to the main screen's `FormKeyDown` (1020:2e09).
    pub key: Option<(u16, ShiftState)>,
    /// The form moved or changed its size.
    pub moved: bool,
    /// The human's command for the world.
    pub action: Option<Action>,
}

impl Response {
    fn sound(&mut self, n: u8) {
        self.sounds.push(n);
    }
}

/// The overview map.
#[allow(clippy::struct_excessive_bools)] // The original's flags.
#[derive(Debug, Clone)]
pub struct Uebersichtsmap {
    font: Font,
    bitmaps: Vec<Surface>,
    /// `UEBERMAP.DAT`'s two layouts. // +0x20e
    layouts: [[DataRect; 22]; 2],
    /// Where the menu's buttons are in `uemenpre.BMP`, for parts 16 to
    /// 21. // +0x1a8
    pressed_at: [DataRect; 6],
    /// `UEBERMAP.TXT`'s title. // +0x1ac
    title_text: Vec<u8>,
    /// "Schließen", `SONSTIGE.TXT`'s 14th text (`*1110:734e + 0xd00`).
    close_text: Vec<u8>,
    /// `MaxGrueneinfluss` (`MODATYP.DAT`), only its low byte read.
    /// // 1110:64c0
    max_green: u8,

    /// The form's top left on the screen and its size; 0 high while it's
    /// hidden.
    pub left: i32,
    pub top: i32,
    pub width: i32,
    pub height: i32,

    /// The menu's button down, [`NO_BUTTON`] for none. // +0x198
    button: u8,
    /// The colouring ([`mode`]). // +0x199
    mode: u8,
    /// The part the mouse went down on: 1 the title bar, 2 close, 3 the
    /// size button, 4 the toggle, the menu's parts by number. // +0x19a
    pressed: u8,
    /// `MapBild`'s width and height. // +0x19b, +0x19d
    map_size: (i16, i16),
    /// Heights, else slopes, shade the land. // +0x19f
    heights: bool,
    /// Saved, never used. // +0x1a0, +0x1a2, +0x1a4, +0x1d0
    unused: (i16, i16, u8, i16),
    /// The scroll bar's thumb is dragged; its arrows are held. // +0x1a5,
    /// +0x1a6, +0x1a7
    ns_dragged: bool,
    up_held: bool,
    down_held: bool,
    /// Where `MapBild` shows the whole map from. // +0x1b0, +0x1b2
    origin: (i16, i16),
    /// Where the title bar was grabbed, from the form's top left, and the
    /// mouse when it last moved. // +0x1b4, +0x1b8
    grab: (i16, i16),
    cursor: (i16, i16),
    /// Where the thumbs were grabbed. // +0x1bc, +0x1be
    wo_grab: i16,
    ns_grab: i16,
    ns_bar: Bar,
    wo_bar: Bar,
    /// `MapBild`'s middle on the screen, at the last right click. // +0x1cc
    centre: (i16, i16),
    /// The title bar's last press was a double click: the form stays
    /// shrunk to it. // +0x212
    double: bool,
    /// The small layout. // +0x213
    small: bool,

    /// The scroll timers' `Tag`s while they run: which part repeats.
    /// // `NSScrolltimer`, `WOScrolltimer`
    ns_timer: Option<u8>,
    wo_timer: Option<u8>,

    /// The main screen's scroll position and rotation, which the frame
    /// shows.
    main_view: (i16, i16, u8),
    /// `big_map`'s bigger main view on the map's picture
    /// ([`Uebersichtsmap::set_big_frame`]), which the frame shows instead.
    big_frame: Option<Rect>,
    /// The location map as last drawn, in [`mode::LOCATION`]: the port's
    /// stand-in for the redraws of 1030:185d (10b8:15c4), whose world
    /// routine gets no tick.
    location_seen: Vec<u8>,

    pub title: Surface,
    pub toggle: Surface,
    pub scroll: Surface,
    pub menu: Surface,
    /// `MapBild` with the frame on it.
    pub map: Surface,
    /// `MapBild` without the frame. The original draws the frame into the
    /// picture and takes it off by drawing the area under it again
    /// (10b8:1485).
    bare: Surface,
    /// The player's view enhancements (`overview_colours`,
    /// `overview_scrolling`).
    pub enhance: Enhancements,
}

fn rect(r: DataRect) -> Rect {
    let [l, t, r, b] = r.map(i32::from);
    Rect::new(l, t, r - l + 1, b - t + 1)
}

fn surface(r: DataRect) -> Surface {
    let r = rect(r);
    Surface::new(
        usize::try_from(r.width()).unwrap_or(0),
        usize::try_from(r.height()).unwrap_or(0),
        0,
    )
}

/// The `n`th rectangle of a layout, zeros where `UEBERMAP.DAT` is short.
fn layout_of(rects: &[DataRect]) -> [DataRect; 22] {
    let mut out = [[0; 4]; 22];
    for (o, r) in out.iter_mut().zip(rects) {
        *o = *r;
    }
    out
}

/// A road piece (10–58) or one of the pieces 95–100.
fn road_like(ground: u8) -> bool {
    (10..=0x3a).contains(&ground) || (0x5f..=100).contains(&ground)
}

/// A river piece, as the road check leaves them (59–94).
fn river(ground: u8) -> bool {
    (0x3b..=0x5e).contains(&ground)
}

/// Any river piece (55–94).
fn any_river(ground: u8) -> bool {
    (0x37..=0x5e).contains(&ground)
}

/// Grass, or a road on grass (25–39).
fn grassy(ground: u8) -> bool {
    ground == 1 || (0x19..0x28).contains(&ground)
}

/// The colours' row for the height of `(i, j)`'s corner: grass from 3 up,
/// two rows a step to 36, anything else from 26. With `overview_colours`
/// (`fixed`) lower grass takes the lowest grass's row and higher sand the
/// highest sand's.
fn height_row(world: &World, pos: Pos, grass: bool, fixed: bool) -> i32 {
    let h = i32::from(world.island.heights.at(pos));
    if grass {
        // BUG(original): grass below height 3 gives a row above the bitmap; the port draws nothing there.
        let row = ((h - 3) * 2).min(0x24);
        if fixed { row.max(0) } else { row }
    } else {
        // BUG(original): sand from height 6 up gives a row below the bitmap; the port draws nothing there.
        let row = h * 2 + 0x1a;
        if fixed { row.min(0x24) } else { row }
    }
}

/// The owner's colour's row: `(1110:0073[owner] - 1) * 2`, where the
/// player colours are at `1110:0074`. With `overview_colours` (`fixed`)
/// none for a square of nobody's, which the caller shows as the land.
fn owner_row(world: &World, pos: Pos, fixed: bool) -> Option<i32> {
    let owner = usize::from(world.island.owners.at(pos));
    // BUG(original): owner 0 reads the never-written byte 1110:0073 (0), owners past 8 the globals after the colours; the port takes 0 for both, a row above the bitmap that draws nothing.
    let colour = owner
        .checked_sub(1)
        .and_then(|o| world.player_colours.get(o))
        .copied()
        .unwrap_or(0);
    if fixed && colour == 0 {
        return None;
    }
    Some((i32::from(colour) - 1) * 2)
}

/// A building's kind's row (`HAUSDATA.DAT` column 5).
fn kind_row(world: &World, building: u8) -> i32 {
    i32::from(world.tables.hausdata(building, 5)) * 2 + 2
}

/// The column and row in `Ueberma2.BMP` of the colour for `pos`, by the
/// colouring and shading. // 10b8:17cf
#[allow(clippy::too_many_lines)]
fn colour(world: &World, pos: Pos, mode: u8, heights: bool, fixed: bool) -> (i32, i32) {
    let island = &world.island;
    let ground = island.ground.at(pos);
    let building = island.buildings.at(pos);
    let free = FREE_SQUARES.contains(&building);
    let building_proper = !NOT_BUILDINGS.contains(&building);
    let form_row = || i32::from(island.forms.at(pos)) * 2;
    // The rows that roads, rivers and the owners' or kinds' colours have
    // in common, else `land`.
    let common = |by_building: &dyn Fn() -> i32, land: &dyn Fn() -> i32| {
        if road_like(ground) {
            0x18
        } else if river(ground) {
            0x1a
        } else if !free {
            by_building()
        } else {
            land()
        }
    };
    match (mode, heights) {
        (mode::OWNERS, true) => (
            // 10b8:045f
            if ground == 1 && free { 0x32 } else { 0x30 },
            // 10b8:04b1
            common(
                &|| owner_row(world, pos, fixed).unwrap_or_else(|| height_row(world, pos, ground == 1, fixed)),
                &|| height_row(world, pos, ground == 1, fixed),
            ),
        ),
        (mode::OWNERS, false) => (
            // 10b8:05f8
            match (free, ground) {
                (true, 1) => 0x36,
                (true, 0 | 4) => 0x34,
                _ => 0x30,
            },
            // 10b8:0684
            common(&|| owner_row(world, pos, fixed).unwrap_or_else(form_row), &form_row),
        ),
        (mode::BUILDINGS, true) => (
            // 10b8:098c
            if building_proper {
                0x38
            } else if ground == 1 && free {
                0x32
            } else {
                0x30
            },
            // 10b8:0a0a
            common(&|| kind_row(world, building), &|| {
                height_row(world, pos, ground == 1, fixed)
            }),
        ),
        (mode::BUILDINGS, false) => (
            // 10b8:0b57
            match (building_proper, free, ground) {
                (true, _, _) => 0x38,
                (false, true, 1) => 0x36,
                (false, true, 0 | 4) => 0x34,
                _ => 0x30,
            },
            // 10b8:0c10
            common(&|| kind_row(world, building), &form_row),
        ),
        (mode::LOCATION, _) => (
            0x3a, // 10b8:0d18
            // 10b8:0d28
            0x24 - i32::from(world.location_at(pos)) * 0x24 / 0xff,
        ),
        (mode::GREEN, _) => (
            0x38, // 10b8:0dc2
            // 10b8:0dd2
            if building == PALM {
                0x1e
            } else if building_proper {
                if world.tables.hausdata(building, 5) == 6 {
                    0x20
                } else {
                    0x22
                }
            } else if road_like(ground) {
                0x22
            } else if any_river(ground) {
                0x20
            } else {
                0x24
            },
        ),
        (mode::OCCUPANCY, _) => (
            0x3c, // 10b8:0d53
            // 10b8:0d63
            if building_proper {
                0x20 - i32::from(island.occupancy.at(pos)) * 32 / 100
            } else {
                0x24
            },
        ),
        (_, true) => (
            // 10b8:0786
            if grassy(ground) { 0x32 } else { 0x30 },
            // 10b8:07df
            if any_river(ground) {
                0x1a
            } else {
                height_row(world, pos, grassy(ground), fixed)
            },
        ),
        (_, false) => {
            // 10b8:08a8, 10b8:0934
            if any_river(ground) {
                (0x30, 0x1a)
            } else if grassy(ground) {
                (0x36, form_row())
            } else {
                (0x34, form_row())
            }
        }
    }
}

impl Uebersichtsmap {
    /// The form as `load_game_rest` creates it for a game (10b8:4538):
    /// hidden at the screen's top left, the land shaded by heights,
    /// [`mode::TERRAIN`] shown, the map's window around the main screen's
    /// scroll position `wo`, `ns` (turned `rotation`).
    ///
    /// `bitmaps` are those of [`BITMAP_FILES`] (a missing one draws
    /// nothing), `layout` is `UEBERMAP.DAT`, `title` `UEBERMAP.TXT`'s
    /// (`formats::data::uebermap_txt`), `close` `SONSTIGE.TXT`'s 14th text
    /// and `max_green` `MODATYP.DAT`'s `MaxGrueneinfluss`.
    #[must_use]
    pub fn new(
        font: Font,
        bitmaps: Vec<Surface>,
        layout: &UebermapData,
        title: &[u8],
        close: &[u8],
        max_green: i16,
        main_view: (i16, i16, u8),
    ) -> Self {
        let layouts = [layout_of(&layout.layouts[0]), layout_of(&layout.layouts[1])];
        let mut pressed_at = [[0; 4]; 6];
        for (o, r) in pressed_at.iter_mut().zip(&layout.pressed) {
            *o = *r;
        }
        let empty = Surface::new(0, 0, 0);
        let mut m = Uebersichtsmap {
            font,
            bitmaps,
            layouts,
            pressed_at,
            title_text: title.to_vec(),
            close_text: close.to_vec(),
            max_green: max_green.to_le_bytes()[0],
            left: 0,
            top: 0,
            width: 0,
            height: 0,
            button: NO_BUTTON,
            mode: mode::TERRAIN,
            pressed: 0,
            map_size: (0, 0),
            heights: true,
            unused: (0, 0, 0, 0),
            ns_dragged: false,
            up_held: false,
            down_held: false,
            origin: (0, 0),
            grab: (0, 0),
            cursor: (0, 0),
            wo_grab: 0,
            ns_grab: 0,
            ns_bar: Bar::default(),
            wo_bar: Bar::default(),
            centre: (0, 0),
            double: false,
            small: false,
            ns_timer: None,
            wo_timer: None,
            main_view,
            big_frame: None,
            location_seen: Vec::new(),
            title: empty.clone(),
            toggle: empty.clone(),
            scroll: empty.clone(),
            menu: empty.clone(),
            map: empty.clone(),
            bare: empty,
            enhance: Enhancements::ORIGINAL,
        };
        m.layout();
        m.width = m.r(part::FORM)[2] + 1;
        m.centre_on_main_view();
        m.set_ns_bar();
        m.set_wo_bar();
        m.compose();
        m
    }

    /// The current layout's rectangle `n`: left, top, right, bottom.
    fn r(&self, n: usize) -> [i32; 4] {
        self.layouts[usize::from(self.small)][n].map(i32::from)
    }

    fn data_rect(&self, n: usize) -> DataRect {
        self.layouts[usize::from(self.small)][n]
    }

    /// `point_in_rect` with the current layout's rectangle `n`, edges
    /// included. // 10b8:02e9, 1030:1517
    fn hit(&self, n: usize, x: i32, y: i32) -> bool {
        let [left, top, right, bottom] = self.r(n);
        (left..=right).contains(&x) && (top..=bottom).contains(&y)
    }

    /// The form's height when it's shown.
    fn full_height(&self) -> i32 {
        self.r(part::FORM)[3] + 1
    }

    /// Whether the form is shown.
    #[must_use]
    pub fn visible(&self) -> bool {
        self.height > 0
    }

    /// The form on the screen.
    #[must_use]
    pub fn bounds(&self) -> Rect {
        Rect::new(self.left, self.top, self.width, self.height)
    }

    /// `MapBild`'s size.
    #[must_use]
    pub fn map_size(&self) -> (i32, i32) {
        (i32::from(self.map_size.0), i32::from(self.map_size.1))
    }

    /// Where `MapBild` shows the whole map from.
    #[must_use]
    pub fn origin(&self) -> (i32, i32) {
        (i32::from(self.origin.0), i32::from(self.origin.1))
    }

    /// The colouring ([`mode`]).
    #[must_use]
    pub fn mode(&self) -> u8 {
        self.mode
    }

    /// Whether heights shade the land, else slopes.
    #[must_use]
    pub fn heights(&self) -> bool {
        self.heights
    }

    /// The small layout is shown.
    #[must_use]
    pub fn small(&self) -> bool {
        self.small
    }

    /// Each image with its bounds in the form and its picture.
    #[must_use]
    pub fn images(&self) -> [(Image, Rect, &Surface); 5] {
        Image::ALL.map(|image| {
            let s = match image {
                Image::Title => &self.title,
                Image::Toggle => &self.toggle,
                Image::Scroll => &self.scroll,
                Image::Map => &self.map,
                Image::Menu => &self.menu,
            };
            (image, rect(self.data_rect(image.part())), s)
        })
    }

    /// The image at `x`, `y` in the form, if any.
    #[must_use]
    pub fn image_at(&self, x: i32, y: i32) -> Option<(Image, i32, i32)> {
        Image::ALL.into_iter().find_map(|image| {
            let r = rect(self.data_rect(image.part()));
            (x >= r.left && x < r.right && y >= r.top && y < r.bottom).then_some((image, x - r.left, y - r.top))
        })
    }

    /// Copies part `n` of the current layout from the same place in
    /// bitmap `from` into `dst`. // 10b8:31f8
    fn restore(dst: &mut Surface, from: &Surface, r: DataRect) {
        let r = rect(r);
        let clip = dst.bounds();
        dst.blit(r.left, r.top, from, r, Rop::Copy, clip);
    }

    /// Copies the top left of bitmap `from` over all of `dst`. // 10b8:346a
    fn fill(dst: &mut Surface, from: &Surface) {
        let all = dst.bounds();
        dst.blit(0, 0, from, all, Rop::Copy, all);
    }

    fn blit(dst: &mut Surface, from: &Surface, src: (i32, i32), size: (i32, i32), at: (i32, i32), rop: Rop) {
        let clip = dst.bounds();
        dst.blit(at.0, at.1, from, Rect::new(src.0, src.1, size.0, size.1), rop, clip);
    }

    fn title_bitmap(&self) -> usize {
        if self.small { TITLE_SMALL } else { TITLE_BIG }
    }

    fn menu_bitmap(&self) -> usize {
        if self.small { MENU_SMALL } else { MENU_BIG }
    }

    fn ns_bitmap(&self) -> usize {
        if self.small { NS_SMALL } else { NS_BIG }
    }

    /// Lays the images out for the size, draws the title bar, the scroll
    /// bar, the menu and the toggle, and keeps the map's window inside the
    /// map. // 10b8:3553
    fn layout(&mut self) {
        let image = self.data_rect(part::MAP);
        self.map_size = (image[2] - image[0] + 1, image[3] - image[1] + 1);
        self.title = surface(self.data_rect(part::TITLE));
        self.toggle = surface(self.data_rect(part::TOGGLE));
        self.scroll = surface(self.data_rect(part::NS));
        self.menu = surface(self.data_rect(part::MENU));
        self.bare = surface(image);
        self.map = self.bare.clone();
        let (w, h) = self.map_size();
        self.origin.1 = short((MAP_HEIGHT - h).min(i32::from(self.origin.1)));
        self.origin.0 = short((MAP_WIDTH - w).min(i32::from(self.origin.0)));
        // 10b8:1e4a, 10b8:257f: the bars' ranges and thumbs, which the
        // bitmaps then cover.
        self.ns_bar = Bar {
            max: short(MAP_HEIGHT - h),
            min: 0,
            position: self.origin.1,
        };
        self.wo_bar = Bar {
            max: short(MAP_WIDTH - w),
            min: 0,
            position: self.origin.0,
        };
        let b = &self.bitmaps;
        let get = |n: usize| b.get(n);
        if let Some(t) = get(self.title_bitmap()) {
            Self::fill(&mut self.title, t);
        }
        if let Some(t) = get(self.ns_bitmap()) {
            Self::fill(&mut self.scroll, t);
        }
        if let Some(t) = get(self.menu_bitmap()) {
            Self::fill(&mut self.menu, t);
        }
        self.draw_toggle(false);
        self.draw_close_text(); // 10b8:3eee
        // The title, centred over the title bar's width, in colour 9.
        let width = self.r(part::TITLE)[2];
        self.font
            .draw(&mut self.title, &self.title_text, (0, 2), width, Align::Center, 9);
    }

    /// The toggle: the shading it switches to, up or held down.
    fn draw_toggle(&mut self, held: bool) {
        let n = match (self.heights, held) {
            (true, false) => TO_SLOPES,
            (false, false) => TO_HEIGHTS,
            (true, true) => TO_SLOPES_PRESSED,
            (false, true) => TO_HEIGHTS_PRESSED,
        };
        if let Some(b) = self.bitmaps.get(n) {
            Self::blit(&mut self.toggle, b, (0, 0), (16, 16), (0, 0), Rop::Copy);
        }
    }

    /// "Schließen" over its button, in colour 9. // 10b8:3eee
    fn draw_close_text(&mut self) {
        let [l, t, r, _] = self.r(part::SCHLIESSEN);
        self.font
            .draw(&mut self.menu, &self.close_text, (l, t + 8), r - l, Align::Center, 9);
    }

    /// The map's window around the main screen's scroll position, inside
    /// the map. // 10b8:4079, 10b8:4538
    fn centre_on_main_view(&mut self) {
        let (wo, ns, _) = self.main_view;
        let (w, h) = self.map_size();
        let x = (i32::from(wo) + 10) * 2 - w / 2;
        let y = i32::from(ns) + 13 - h / 2;
        self.origin = (short(x.min(MAP_WIDTH - w).max(0)), short(y.min(MAP_HEIGHT - h).max(0)));
    }

    /// The vertical bar's thumb's travel.
    fn ns_travel(&self) -> i32 {
        let [_, t, _, b] = self.r(part::NS);
        (b - t + 1) - 16 - 19
    }

    /// The horizontal bar's thumb's travel: the trough's width, which lets
    /// the thumb onto the right arrow; with `overview_scrolling` 4 less,
    /// which keeps it off.
    fn wo_travel(&self) -> i32 {
        let [l, _, r, _] = self.r(part::WO_TROUGH);
        if self.enhance.has(Enhancement::OverviewScrolling) {
            r - l + 1 - 4
        } else {
            r - l + 1
        }
    }

    /// Draws the vertical bar's thumb. // 10b8:200e
    fn draw_ns_thumb(&mut self) {
        let y = self.ns_bar.thumb(self.ns_travel());
        if let Some(s) = self.bitmaps.get(SCROLL) {
            Self::blit(&mut self.scroll, s, (0, 16), (16, 19), (0, y), Rop::And);
            Self::blit(&mut self.scroll, s, (16, 16), (16, 19), (0, y), Rop::Invert);
        }
    }

    /// Takes the vertical bar's thumb off: its trough from the bitmap.
    /// // 10b8:20a5
    fn erase_ns_thumb(&mut self) {
        let r = self.data_rect(part::NS_TROUGH);
        if let Some(b) = self.bitmaps.get(self.ns_bitmap()) {
            Self::restore(&mut self.scroll, b, r);
        }
    }

    /// Draws the horizontal bar's thumb. // 10b8:2785
    fn draw_wo_thumb(&mut self) {
        // BUG(original): the thumb's travel is the trough's width, so near the right end it covers 4 pixels of the right arrow, which taking it off (10b8:281c) leaves behind.
        let x = self.wo_bar.thumb(self.wo_travel());
        if let Some(s) = self.bitmaps.get(SCROLL) {
            Self::blit(&mut self.menu, s, (0, 0), (20, 16), (x, 0), Rop::And);
            Self::blit(&mut self.menu, s, (20, 0), (20, 16), (x, 0), Rop::Invert);
        }
    }

    /// Takes the horizontal bar's thumb off. // 10b8:281c
    fn erase_wo_thumb(&mut self) {
        let r = self.data_rect(part::WO_TROUGH);
        if let Some(b) = self.bitmaps.get(self.menu_bitmap()) {
            Self::restore(&mut self.menu, b, r);
        }
    }

    /// The vertical bar at the window's top. // 10b8:1e73
    fn set_ns_bar(&mut self) {
        self.erase_ns_thumb();
        self.ns_bar.position = self.origin.1;
        self.draw_ns_thumb();
    }

    /// The horizontal bar at the window's left. // 10b8:25a8
    fn set_wo_bar(&mut self) {
        self.erase_wo_thumb();
        self.wo_bar.position = self.origin.0;
        self.draw_wo_thumb();
    }

    /// Draws the area `left`..`right`, `top`..`bottom` of `MapBild`: the
    /// sea's tile, then each square not of water as two by two pixels of
    /// its colour. // 10b8:0eeb
    fn draw_area(&mut self, world: &World, (left, top, right, bottom): (i32, i32, i32, i32)) {
        let Some(colours) = self.bitmaps.get(COLOURS) else {
            return;
        };
        let clip = self.bare.bounds();
        // The tile is 48 × 38; the last column and row are cut to fit.
        let mut y = top;
        while y < bottom {
            let mut x = left;
            while x < right {
                let size = ((right - x).min(0x30), (bottom - y).min(0x26));
                self.bare
                    .blit(x, y, colours, Rect::new(0, 0, size.0, size.1), Rop::Copy, clip);
                x += 0x30;
            }
            y += 0x26;
        }
        let (ox, oy) = self.origin();
        let mut y = top - 2;
        while y <= bottom + 2 {
            let mut x = left - 1;
            while x <= right + 1 {
                // 10b8:038d: the square at the pixel.
                let (px, py) = (x + ox, y + oy);
                let half = px / 2;
                let i = (py + half + 750) / 2 - 500;
                let j = (py - half + 1250) / 2 - 500;
                let pos = (short(i), short(j));
                if on_map(pos) && world.island.ground.at(pos) != 2 {
                    let fixed = self.enhance.has(Enhancement::OverviewColours);
                    let (cx, cy) = colour(world, pos, self.mode, self.heights, fixed);
                    let at = (x - px % 2, y - (py - half + 250) % 2 - 2);
                    self.bare
                        .blit(at.0, at.1, colours, Rect::new(cx, cy, 2, 2), Rop::Copy, clip);
                }
                x += 2;
            }
            y += 2;
        }
    }

    /// Draws `MapBild`'s area `left`..`right`, `top`..`bottom` again if
    /// `modes` has the colouring's bit; with `grow`, for the location
    /// and the green colourings, that much bigger by their reach
    /// (`MaxLageeinfluss`, `MaxGrueneinfluss`). // 10b8:17cf
    fn redraw(&mut self, world: &World, area: (i32, i32, i32, i32), modes: u8, grow: bool) -> bool {
        if self.mode >= 8 || modes & (1 << self.mode) == 0 {
            return false;
        }
        let by = match self.mode {
            mode::LOCATION if grow => i32::from(world.max_location_effect),
            mode::GREEN if grow => i32::from(self.max_green),
            _ => 0,
        };
        let (l, t, r, b) = area;
        self.draw_area(world, (l - by, t - by, r + by, b + by));
        true
    }

    /// Draws the squares from `(i1, j1)` to `(i2, j2)` again, as an area
    /// of `MapBild`, for the colourings in `modes`. // 10b8:1684
    fn redraw_squares(&mut self, world: &World, (i1, j1): Pos, (i2, j2): Pos, modes: u8, grow: bool) -> bool {
        let (ox, oy) = self.origin();
        let (i1, j1, i2, j2) = (i32::from(i1), i32::from(j1), i32::from(i2), i32::from(j2));
        // 10b8:0422, 10b8:0443: a square's pixel.
        let left = ((i1 - j2) + 250) * 2 - ox;
        let top = i1 + j1 - oy;
        let right = ((i2 - j1) + 250) * 2 - ox;
        let bottom = i2 + j2 - oy + 1;
        self.redraw(world, (left, top, right, bottom), modes, grow)
    }

    /// All of `MapBild`, then the frame. // 10b8:1a49
    fn redraw_all(&mut self, world: &World) {
        let (w, h) = self.map_size();
        self.redraw(world, (0, 0, w, h), ALL_MODES, false);
        self.see_location(world);
        self.compose();
    }

    /// `MapBild` from the bare map and the frame where the main screen
    /// looks: its mask `SRCAND`, its picture `SRCINVERT`. // 10b8:1349
    fn compose(&mut self) {
        self.map.clone_from(&self.bare);
        let (wo, ns, rotation) = self.main_view;
        let (ox, oy) = self.origin();
        if let Some(frame) = self.big_frame {
            self.compose_big(frame, rotation, (ox, oy));
            return;
        }
        // BUG(original): the frame is a 40 × 25 box in every rotation, but turned a quarter the main view covers about 50 × 20 of the map's pixels (25 half rows across, 19½ columns down).
        let f = &FRAMES[usize::from(rotation & 3)];
        let x = i32::from(wo) * 2 - 1 - ox - f.offset.0;
        let y = i32::from(ns) - oy - f.offset.1;
        if let Some(b) = self.bitmaps.get(FRAME) {
            Self::blit(&mut self.map, b, f.mask, f.size, (x, y), Rop::And);
            Self::blit(&mut self.map, b, f.picture, f.size, (x, y), Rop::Invert);
        }
    }

    /// `big_map`'s frame: the bigger view's box `frame` on the map's
    /// picture, outlined in the colour the original's frame picture has,
    /// with the original's mark for the screen's bottom edge (the dotted
    /// arrows of `UeRecht.BMP`) in the middle of that side.
    fn compose_big(&mut self, frame: Rect, rotation: u8, (ox, oy): (i32, i32)) {
        let r = Rect::new(frame.left - ox, frame.top - oy, frame.width(), frame.height());
        let clip = self.map.bounds();
        for edge in [
            Rect::new(r.left, r.top, r.width(), 1),
            Rect::new(r.left, r.bottom - 1, r.width(), 1),
            Rect::new(r.left, r.top, 1, r.height()),
            Rect::new(r.right - 1, r.top, 1, r.height()),
        ] {
            self.map.fill_rect(edge, clip, BIG_FRAME_COLOUR);
        }
        let [mask, picture, size] = BIG_FRAME_MARKS[usize::from(rotation & 3)];
        let (cx, cy) = (r.left + r.width() / 2, r.top + r.height() / 2);
        let at = match rotation & 3 {
            0 => (cx - size.0 / 2, r.bottom),
            1 => (r.left - size.0, cy - size.1 / 2),
            2 => (cx - size.0 / 2, r.top - size.1),
            _ => (r.right, cy - size.1 / 2),
        };
        if let Some(b) = self.bitmaps.get(FRAME) {
            Self::blit(&mut self.map, b, mask, size, at, Rop::And);
            Self::blit(&mut self.map, b, picture, size, at, Rop::Invert);
        }
    }

    /// `big_map`: the main screen's bigger view is `frame` on the map's
    /// picture (`Hauptschirm::overview_frame`), or none for the original's
    /// view, whose frame is [`FRAMES`]'. Whether the map changed.
    pub fn set_big_frame(&mut self, frame: Option<Rect>) -> bool {
        if self.big_frame == frame {
            return false;
        }
        self.big_frame = frame;
        self.compose();
        true
    }

    /// The main screen scrolled or turned: the frame moves there. The
    /// original takes it off before (10b8:1485) and draws it after
    /// (10b8:1349) every scroll and turn of the main screen (1020:07dc and
    /// the scroll bars, the keys, 1010:254d).
    pub fn set_main_view(&mut self, wo: i16, ns: i16, rotation: u8) -> bool {
        if self.main_view == (wo, ns, rotation) {
            return false;
        }
        self.main_view = (wo, ns, rotation);
        self.compose();
        true
    }

    /// Moves the map's window by `dx`, `dy`, as far as the scroll bars go:
    /// what stays in view is copied, the strips that come into view are
    /// drawn. // 10b8:1a6e
    fn scroll_by(&mut self, world: &World, dx: i32, dy: i32) {
        let (ox, oy) = self.origin();
        let wo = (i32::from(self.wo_bar.min), i32::from(self.wo_bar.max));
        let ns = (i32::from(self.ns_bar.min), i32::from(self.ns_bar.max));
        let dx = if ox + dx > wo.1 {
            wo.1 - ox
        } else if ox + dx < wo.0 {
            wo.0 - ox
        } else {
            dx
        };
        let dy = if oy + dy > ns.1 {
            ns.1 - oy
        } else if oy + dy < ns.0 {
            ns.0 - oy
        } else {
            dy
        };
        let (w, h) = self.map_size();
        let old = self.bare.clone();
        let clip = self.bare.bounds();
        self.bare.blit(
            (-dx).max(0),
            (-dy).max(0),
            &old,
            Rect::new(dx.max(0), dy.max(0), w - dx.abs(), h - dy.abs()),
            Rop::Copy,
            clip,
        );
        self.origin = (short(ox + dx), short(oy + dy));
        // 10b8:171b: the strips, by left, top, right, bottom.
        if dx > 0 {
            self.redraw(world, (w - dx, 0, w, h), ALL_MODES, false);
        }
        if dx < 0 {
            self.redraw(world, (0, 0, -dx, h), ALL_MODES, false);
        }
        if dy > 0 {
            self.redraw(world, ((-dx).max(0), h - dy, w - dx.max(0), h), ALL_MODES, false);
        }
        if dy < 0 {
            self.redraw(world, ((-dx).max(0), 0, w - dx.max(0), -dy), ALL_MODES, false);
        }
        self.compose();
    }

    /// Moves the vertical bar by `delta`, and the window with it.
    /// // 10b8:20f2
    fn scroll_ns(&mut self, world: &World, delta: i32) {
        let p = (i32::from(self.ns_bar.position) + delta)
            .max(i32::from(self.ns_bar.min))
            .min(i32::from(self.ns_bar.max));
        self.erase_ns_thumb();
        self.ns_bar.position = short(p);
        self.draw_ns_thumb();
        self.scroll_by(world, 0, p - self.origin().1);
    }

    /// Moves the horizontal bar by `delta`, and the window with it.
    /// // 10b8:2869
    fn scroll_wo(&mut self, world: &World, delta: i32) {
        let p = (i32::from(self.wo_bar.position) + delta)
            .max(i32::from(self.wo_bar.min))
            .min(i32::from(self.wo_bar.max));
        self.erase_wo_thumb();
        self.wo_bar.position = short(p);
        self.draw_wo_thumb();
        self.scroll_by(world, p - self.origin().0, 0);
    }

    /// Both bars where the window is. // 10b8:1e25
    fn update_bars(&mut self) {
        self.set_ns_bar();
        self.set_wo_bar();
    }

    /// Picks the menu's button `n` (16 to 20), or with 10 none changes:
    /// the button that's down again comes up and the land alone shows.
    /// Then the buttons, the one down and the whole map are drawn.
    /// // 10b8:303e
    fn select(&mut self, world: &World, n: u8) {
        if n != NO_BUTTON {
            if self.button == n {
                self.button = NO_BUTTON;
                self.mode = mode::TERRAIN;
            } else {
                self.button = n;
                if let Some(m) = n.checked_sub(16).filter(|&m| m < 5) {
                    self.mode = m;
                }
            }
        }
        if let Some(b) = self.bitmaps.get(self.menu_bitmap()) {
            Self::restore(&mut self.menu, b, self.layouts[usize::from(self.small)][part::BUTTONS]);
        }
        if self.button != NO_BUTTON {
            let k = usize::from(self.button);
            let from = self.pressed_at.get(k.wrapping_sub(16)).copied().unwrap_or_default();
            let [l, t, _, _] = self.r(k.min(21));
            if let Some(b) = self.bitmaps.get(MENU_PRESSED) {
                let from = (i32::from(from[0]), i32::from(from[1]));
                Self::blit(&mut self.menu, b, from, (0x20, 0x20), (l, t), Rop::Copy);
            }
        }
        self.redraw_all(world);
    }

    /// `game_show_state` and `game_start_setup` (10b8:3fab): the big
    /// layout, drawn for the world; shown again if it was.
    pub fn game_start(&mut self, world: &World) {
        let shown = self.visible();
        self.small = false;
        self.height = 0;
        self.width = self.r(part::FORM)[2] + 1;
        self.layout();
        self.select(world, NO_BUTTON);
        self.update_bars();
        if shown {
            self.height = self.full_height();
        }
    }

    /// The button bar's button 5 goes down (1010:2c8b): the window around
    /// the main screen's view, everything drawn, the form shown with the
    /// focus, and sound 0xd. // 10b8:4079
    pub fn open(&mut self, world: &World) -> Response {
        let mut r = Response::default();
        self.centre_on_main_view();
        let (w, h) = self.map_size();
        self.ns_bar = Bar {
            max: short(MAP_HEIGHT - h),
            min: 0,
            position: self.origin.1,
        };
        self.wo_bar = Bar {
            max: short(MAP_WIDTH - w),
            min: 0,
            position: self.origin.0,
        };
        self.draw_ns_thumb(); // 10b8:1e4a
        self.draw_wo_thumb(); // 10b8:257f
        self.select(world, NO_BUTTON);
        self.update_bars();
        self.draw_close_text();
        self.height = self.full_height();
        r.sound(OPEN_SOUND);
        r.refresh = true;
        r.moved = true;
        r
    }

    /// Closes the form: sound 0xe, the button bar's button 5 up, the
    /// focus to the main screen. The button bar calls it when button 5
    /// comes up (1010:2c8b), and so do the form's own ways out.
    /// // 10b8:3f7f
    pub fn close(&mut self) -> Response {
        self.height = 0;
        Response {
            sounds: vec![CLOSE_SOUND],
            closed: true,
            refresh: true,
            moved: true,
            ..Response::default()
        }
    }

    /// What the world's tick (or a command) changed, drawn again where the
    /// colouring shows it. Whether `MapBild` changed.
    ///
    /// The original redraws each change's area as the world makes it:
    /// buildings, palms, beach life, demolitions and roads for the five
    /// colourings but the land alone (10b8:1564, 1594, 15f4, 1624, 1654),
    /// the land raised and lowered for all (10b8:1504), the occupancy for
    /// its colouring (10b8:1534) and the location map's changes for its
    /// (10b8:15c4). The port redraws the area around all of
    /// [`Tick::redraw`]'s squares at once, for every colouring, from the
    /// world as the tick left it; the occupancy by [`Tick::occupancy`];
    /// and the location map wherever it differs from what was drawn.
    pub fn show_tick(&mut self, world: &World, tick: &Tick) -> bool {
        if tick.loaded {
            self.game_start(world);
            return true;
        }
        let mut drawn = false;
        if let Some((low, high)) = bounding_box(&tick.redraw) {
            drawn |= self.redraw_squares(world, low, high, ALL_MODES, true);
        }
        for &(first, last) in &tick.occupancy {
            drawn |= self.redraw_squares(world, first, last, OCCUPANCY_MODE, false); // 10b8:1534
        }
        drawn |= self.location_changes(world);
        if drawn {
            self.compose();
        }
        drawn
    }

    /// Remembers the location map as drawn.
    fn see_location(&mut self, world: &World) {
        if self.mode == mode::LOCATION {
            self.location_seen.clear();
            self.location_seen.extend_from_slice(world.island.location.bytes());
        }
    }

    /// In the location colouring, the location map's cells that changed
    /// since they were drawn, drawn again. // 1030:185d, 10b8:15c4
    fn location_changes(&mut self, world: &World) -> bool {
        // BUG(original): 1030:185d asks for the change's radius in squares, though it's in cells of two; the port redraws every changed cell instead.
        if self.mode != mode::LOCATION {
            return false;
        }
        let now = world.island.location.bytes();
        if self.location_seen.len() != now.len() {
            self.see_location(world);
            return false;
        }
        let size = crate::world::LOCATION_SIZE;
        let mut changed: Vec<Pos> = Vec::new();
        for (k, (a, b)) in self.location_seen.iter().zip(now).enumerate() {
            if a != b {
                changed.push((
                    short(i32::try_from(k / size).unwrap_or(0)),
                    short(i32::try_from(k % size).unwrap_or(0)),
                ));
            }
        }
        let Some(((a1, b1), (a2, b2))) = bounding_box(&changed) else {
            return false;
        };
        self.see_location(world);
        let low = (a1 * 2, b1 * 2);
        let high = ((a2 * 2 + 1).min(250), (b2 * 2 + 1).min(250));
        self.redraw_squares(world, low, high, LOCATION_MODE, false)
    }

    /// `UmschaltbildMouseDown`: the left button holds the toggle down,
    /// with sound 8. // 10b8:1c74
    pub fn toggle_mouse_down(&mut self, left: bool) -> Response {
        let mut r = Response::default();
        if left {
            self.draw_toggle(true);
            self.pressed = 4;
            r.sound(PRESS);
            r.refresh = true;
        }
        r
    }

    /// `UmschaltbildMouseUp`: the left button let go over the toggle held
    /// down switches the shading and draws the map again; any left button
    /// let go draws the toggle up, with sound 10. // 10b8:1d21
    pub fn toggle_mouse_up(&mut self, x: i32, y: i32, left: bool, world: &World) -> Response {
        let mut r = Response::default();
        if left {
            if self.pressed == 4 && (0..=16).contains(&x) && (0..=16).contains(&y) {
                self.heights = !self.heights;
                self.redraw_all(world);
            }
            self.draw_toggle(false);
            r.sound(RELEASE);
            r.refresh = true;
            self.pressed = 0;
        }
        r
    }

    /// Starts the vertical scroll timer with `tag`: no new people come
    /// while it runs. // 10b8:21c7
    fn start_ns_timer(&mut self, tag: u8, r: &mut Response) {
        self.ns_timer = Some(tag);
        r.action = Some(Action::SetOption(WorldOption::PeopleSpawn, false));
    }

    /// `NSScrollBildMouseDown` at `y`: the arrows step 5 and the trough
    /// pages 30, again every 25 ms while held; the thumb is grabbed.
    /// // 10b8:222b
    pub fn scroll_mouse_down(&mut self, y: i32, world: &World) -> Response {
        let mut r = Response {
            refresh: true,
            ..Response::default()
        };
        let height = i32::try_from(self.scroll.height).unwrap_or(0);
        if y < 16 {
            if let Some(b) = self.bitmaps.get(UP_PRESSED) {
                Self::blit(&mut self.scroll, b, (0, 0), (16, 16), (0, 0), Rop::Copy);
            }
            self.up_held = true;
            r.sound(SELECT);
            self.scroll_ns(world, -5);
            self.start_ns_timer(1, &mut r);
        } else if y < height - 16 {
            let thumb = self.ns_bar.thumb(self.ns_travel());
            r.sound(SELECT);
            if y < thumb {
                self.scroll_ns(world, -30);
                self.start_ns_timer(2, &mut r);
            } else if y > thumb + 15 {
                self.scroll_ns(world, 30);
                self.start_ns_timer(3, &mut r);
            } else {
                self.ns_grab = short(y - thumb);
                self.ns_dragged = true;
            }
        } else {
            if let Some(b) = self.bitmaps.get(DOWN_PRESSED) {
                Self::blit(&mut self.scroll, b, (0, 0), (16, 16), (0, height - 16), Rop::Copy);
            }
            self.down_held = true;
            r.sound(SELECT);
            self.scroll_ns(world, 5);
            self.start_ns_timer(4, &mut r);
        }
        r
    }

    /// `NSScrollbildMouseMove`: the thumb follows the mouse while the left
    /// button drags it. // 10b8:2421
    pub fn scroll_mouse_move(&mut self, y: i32, shift: ShiftState, world: &World) -> Response {
        let mut r = Response::default();
        if shift.contains(ShiftState::LEFT) && self.ns_dragged {
            let height = i32::try_from(self.scroll.height).unwrap_or(0);
            let at = (y - i32::from(self.ns_grab)).max(0).min(height - 14);
            let p = self.ns_bar.position_at(at, self.ns_travel());
            self.scroll_ns(world, p - self.origin().1);
            r.refresh = true;
        }
        r
    }

    /// `NSScrollBildMouseUp`: the arrows come up, the thumb is let go and
    /// the timer stops; new people come again. // 10b8:24a1
    pub fn scroll_mouse_up(&mut self) -> Response {
        let mut r = Response::default();
        let (down, up) = (self.data_rect(part::NS_DOWN), self.data_rect(part::NS_UP));
        if let Some(b) = self.bitmaps.get(self.ns_bitmap()) {
            if self.down_held {
                Self::restore(&mut self.scroll, b, down);
                r.refresh = true;
            }
            if self.up_held {
                Self::restore(&mut self.scroll, b, up);
                r.refresh = true;
            }
        }
        self.down_held = false;
        self.up_held = false;
        self.ns_dragged = false;
        // 10b8:21fa
        self.ns_timer = None;
        // BUG(original): letting go of the scroll bar turns the "new people" option on whatever the player chose.
        r.action = Some(Action::SetOption(WorldOption::PeopleSpawn, true));
        r
    }

    /// The scroll timers' tick (`NSScrolltimerTimer`, 10b8:216e, and
    /// `WOScrolltimerTimer`, 10b8:28e5), every 25 ms while a scroll bar's
    /// arrow or trough is held. Whether anything moved.
    pub fn scroll_timers(&mut self, world: &World) -> bool {
        let step = |tag: u8| match tag {
            1 => -5,
            2 => -30,
            3 => 30,
            4 => 5,
            _ => 0,
        };
        let mut moved = false;
        if let Some(tag) = self.ns_timer {
            self.scroll_ns(world, step(tag));
            moved = true;
        }
        if let Some(tag) = self.wo_timer {
            self.scroll_wo(world, step(tag));
            moved = true;
        }
        moved
    }

    /// The mouse wheel over the form (`mouse_wheel`, a port extension):
    /// the map's window moves `dy` notches up and `dx` right, by the
    /// arrows' step of 5 each.
    pub fn wheel(&mut self, world: &World, dx: i32, dy: i32) -> Response {
        let mut r = Response::default();
        let step = |n: i32| n.clamp(-100, 100) * 5;
        let before = self.origin();
        if dy != 0 {
            self.scroll_ns(world, step(-dy));
        }
        if dx != 0 {
            self.scroll_wo(world, step(dx));
        }
        r.refresh = self.origin() != before;
        r
    }

    /// Whether a scroll timer runs, for the caller to tick
    /// [`Uebersichtsmap::scroll_timers`] every 25 ms.
    #[must_use]
    pub fn scrolling(&self) -> bool {
        self.ns_timer.is_some() || self.wo_timer.is_some()
    }

    /// Starts the horizontal scroll timer with `tag`. // 10b8:293e
    fn start_wo_timer(&mut self, tag: u8, r: &mut Response) {
        self.wo_timer = Some(tag);
        r.action = Some(Action::SetOption(WorldOption::PeopleSpawn, false));
    }

    /// `MenubildMouseDown` at `x`, `y`: with the left button, the
    /// horizontal bar like the vertical one, a colouring's button (picked
    /// when it's let go), or "Schließen" held down. // 10b8:29a2
    #[allow(clippy::many_single_char_names)]
    pub fn menu_mouse_down(&mut self, x: i32, y: i32, left: bool, world: &World) -> Response {
        let mut r = Response::default();
        if !left {
            return r;
        }
        if self.hit(part::WO_LEFT, x, y) {
            if let Some(b) = self.bitmaps.get(LEFT_PRESSED) {
                Self::blit(&mut self.menu, b, (0, 0), (16, 16), (0, 0), Rop::Copy);
            }
            self.pressed = 12;
            r.sound(SELECT);
            self.scroll_wo(world, -5);
            self.start_wo_timer(1, &mut r);
        } else if self.hit(part::WO_RIGHT, x, y) {
            let at = self.r(part::WO_RIGHT)[0];
            if let Some(b) = self.bitmaps.get(RIGHT_PRESSED) {
                Self::blit(&mut self.menu, b, (0, 0), (16, 16), (at, 0), Rop::Copy);
            }
            self.pressed = 14;
            r.sound(SELECT);
            self.scroll_wo(world, 5);
            self.start_wo_timer(4, &mut r);
        } else if self.hit(part::WO_TROUGH, x, y) {
            let thumb = self.wo_bar.thumb(self.wo_travel());
            r.sound(SELECT);
            if x < thumb {
                self.scroll_wo(world, -30);
                self.start_wo_timer(2, &mut r);
            } else if x > thumb + 15 {
                self.scroll_wo(world, 30);
                self.start_wo_timer(3, &mut r);
            } else {
                self.wo_grab = short(x - thumb);
                self.pressed = 13;
            }
        } else if let Some(n) = (part::OWNERS..=part::OCCUPANCY).find(|&n| self.hit(n, x, y)) {
            self.pressed = u8::try_from(n).unwrap_or(0);
            r.sound(SELECT);
            return r;
        } else if self.hit(part::SCHLIESSEN, x, y) {
            // The picture is found from the buttons' left edge, not
            // `PressedLage`; one pixel narrower than the button.
            let [l, t, rr, b] = self.r(part::SCHLIESSEN);
            let from = l - self.r(part::OWNERS)[0];
            if let Some(p) = self.bitmaps.get(MENU_PRESSED) {
                Self::blit(&mut self.menu, p, (from, 0), (rr - l, b - t + 1), (l, t), Rop::Copy);
            }
            self.draw_close_text();
            self.pressed = 0x15;
            r.sound(PRESS);
        } else {
            return r;
        }
        r.refresh = true;
        r
    }

    /// `MenubildMouseMove`: the horizontal thumb follows the mouse while
    /// the left button drags it. // 10b8:2e01
    pub fn menu_mouse_move(&mut self, x: i32, shift: ShiftState, world: &World) -> Response {
        let mut r = Response::default();
        if shift.contains(ShiftState::LEFT) && self.pressed == 13 {
            let end = self.r(part::WO_TROUGH)[2] - 14;
            let at = (x - i32::from(self.wo_grab)).max(0).min(end);
            let p = self.wo_bar.position_at(at, self.wo_travel());
            self.scroll_wo(world, p - self.origin().0);
            r.refresh = true;
        }
        r
    }

    /// `MenubildMouseUp`: the arrows come up; a colouring's button let go
    /// over itself is picked; "Schließen" let go over itself closes the
    /// form. The timer stops; new people come again. // 10b8:2e99
    pub fn menu_mouse_up(&mut self, x: i32, y: i32, world: &World) -> Response {
        let mut r = Response::default();
        let pressed = self.pressed;
        let menu = self.menu_bitmap();
        match pressed {
            12 | 14 => {
                let at = self.data_rect(usize::from(pressed));
                if let Some(b) = self.bitmaps.get(menu) {
                    Self::restore(&mut self.menu, b, at);
                }
                r.refresh = true;
            }
            0x10..=0x14 => {
                if self.hit(usize::from(pressed), x, y) {
                    self.select(world, pressed);
                    r.refresh = true;
                }
            }
            0x15 => {
                let at = self.data_rect(part::SCHLIESSEN);
                if let Some(b) = self.bitmaps.get(menu) {
                    Self::restore(&mut self.menu, b, at);
                }
                self.draw_close_text();
                r.refresh = true;
                r.sound(RELEASE);
                if self.hit(part::SCHLIESSEN, x, y) {
                    let closed = self.close();
                    r.sounds.extend(closed.sounds);
                    r.closed = true;
                    r.moved = true;
                }
            }
            _ => {}
        }
        self.pressed = 0;
        // 10b8:2971
        self.wo_timer = None;
        // BUG(original): letting go anywhere on the menu turns the "new people" option on whatever the player chose.
        r.action = Some(Action::SetOption(WorldOption::PeopleSpawn, true));
        r
    }

    /// `MapBildMouseDown` at `x`, `y`: with only the left button the main
    /// screen moves there; with only the right one the window centres on
    /// the point. // 10b8:37f5
    #[allow(clippy::many_single_char_names)]
    pub fn map_mouse_down(&mut self, x: i32, y: i32, shift: ShiftState, world: &World) -> Response {
        let mut r = Response::default();
        if shift == ShiftState::LEFT {
            r.scroll = Some(self.move_main_view(x, y));
            r.refresh = true;
        } else if shift == ShiftState::RIGHT {
            let (w, h) = self.map_size();
            // GetCursorPos from `MapBild`'s middle on the screen.
            self.centre = (
                short(self.left + self.r(part::MAP)[0] + w / 2),
                short(self.top + self.r(part::MAP)[1] + h / 2),
            );
            self.scroll_by(world, x - w / 2, y - h / 2);
            self.update_bars();
            r.refresh = true;
        }
        r
    }

    /// The main screen's scroll position for a click at `x`, `y`, and the
    /// frame there. // 10b8:37f5
    fn move_main_view(&mut self, x: i32, y: i32) -> (i16, i16) {
        let rotation = self.main_view.2;
        let f = &FRAMES[usize::from(rotation & 3)];
        let (ox, oy) = self.origin();
        // BUG(original): near the map's edges this puts the main screen's scroll position past its scroll bars' ends (wo from -10 to 490, ns from -13 to 489), and their thumbs past the arrows.
        let wo = short((x - f.click.0 + ox) / 2);
        let ns = short(y - f.click.1 + oy);
        self.set_main_view(wo, ns, rotation);
        (wo, ns)
    }

    /// `MapBildMouseMove`: dragging with only the left button inside
    /// `MapBild` moves the main screen along. // 10b8:39b1
    #[allow(clippy::many_single_char_names)]
    pub fn map_mouse_move(&mut self, x: i32, y: i32, shift: ShiftState) -> Response {
        let mut r = Response::default();
        let (w, h) = self.map_size();
        if (0..=w).contains(&x) && (0..=h).contains(&y) && shift == ShiftState::LEFT {
            r.scroll = Some(self.move_main_view(x, y));
            r.refresh = true;
        }
        r
    }

    /// `MapBildMouseUp`: the left button draws the main screen's island
    /// again. // 10b8:3aab
    pub fn map_mouse_up(&mut self, left: bool) -> Response {
        Response {
            draw_main: left,
            ..Response::default()
        }
    }

    /// `UeberschriftbildMouseDown` at `x` in the title bar, `cursor` the
    /// mouse on the screen: any button on the bar grabs the form, which
    /// shrinks to the bar while it's dragged; the left button holds the
    /// size button or the close button down. // 10b8:3ac3
    pub fn title_mouse_down(&mut self, x: i32, shift: ShiftState, cursor: (i32, i32)) -> Response {
        let mut r = Response::default();
        let left = shift.contains(ShiftState::LEFT);
        if x > self.r(part::CLOSE)[2] {
            if x < self.r(part::SIZE)[0] {
                self.pressed = 1;
                self.grab = (short(cursor.0 - self.left), short(cursor.1 - self.top));
                self.height = 0x10;
                self.double = shift.contains(ShiftState::DOUBLE);
                r.moved = true;
            } else if left {
                self.pressed = 3;
                let n = if self.small { TO_BIG_PRESSED } else { TO_SMALL_PRESSED };
                let at = self.r(part::SIZE)[0];
                if let Some(b) = self.bitmaps.get(n) {
                    Self::blit(&mut self.title, b, (0, 0), (16, 16), (at, 0), Rop::Copy);
                }
                r.refresh = true;
                r.sound(PRESS);
            }
        } else if left {
            self.pressed = 2;
            if let Some(b) = self.bitmaps.get(CLOSE_PRESSED) {
                Self::blit(&mut self.title, b, (0, 0), (16, 16), (0, 0), Rop::Copy);
            }
            r.refresh = true;
            r.sound(PRESS);
        }
        r
    }

    /// `UeberschriftbildMouseMove`: the grabbed form follows the mouse,
    /// within the `screen`. // 10b8:3c9b
    pub fn title_mouse_move(&mut self, shift: ShiftState, cursor: (i32, i32), screen: (i32, i32)) -> Response {
        let mut r = Response::default();
        if shift.contains(ShiftState::LEFT) && self.pressed == 1 {
            self.cursor = (short(cursor.0), short(cursor.1));
            self.left = (cursor.0 - i32::from(self.grab.0)).max(0).min(screen.0 - self.width);
            self.top = (cursor.1 - i32::from(self.grab.1)).max(0).min(screen.1 - self.height);
            r.moved = true;
        }
        r
    }

    /// `UeberschriftbildMouseUp` at `x`, `y`: the close button closes the
    /// form, the size button switches between big and small, and the form
    /// let go after a drag gets its height back, unless the press was a
    /// double click. // 10b8:3d2b
    pub fn title_mouse_up(&mut self, x: i32, y: i32, world: &World) -> Response {
        let mut r = Response::default();
        match self.pressed {
            2 => {
                // BUG(original): the close button is drawn back from the small title bar's bitmap in either size.
                let at = self.data_rect(part::CLOSE);
                if let Some(b) = self.bitmaps.get(TITLE_SMALL) {
                    Self::restore(&mut self.title, b, at);
                }
                r.refresh = true;
                r.sound(RELEASE);
                if self.hit(part::CLOSE, x, y) {
                    let closed = self.close();
                    r.sounds.extend(closed.sounds);
                    r.closed = true;
                    r.moved = true;
                }
            }
            3 => {
                let at = self.data_rect(part::SIZE);
                if let Some(b) = self.bitmaps.get(self.title_bitmap()) {
                    Self::restore(&mut self.title, b, at);
                }
                r.refresh = true;
                r.sound(RELEASE);
                if self.hit(part::SIZE, x, y) {
                    self.small = !self.small;
                    self.resize(world);
                    r.moved = true;
                }
            }
            1 if !self.double => {
                self.height = self.full_height();
                r.moved = true;
            }
            _ => {}
        }
        self.pressed = 0;
        r
    }

    /// The form in its new size: hidden, laid out, drawn, shown.
    /// // 10b8:3e73
    fn resize(&mut self, world: &World) {
        self.height = 0;
        self.width = self.r(part::FORM)[2] + 1;
        self.layout();
        self.select(world, NO_BUTTON);
        self.update_bars();
        self.height = self.full_height();
    }

    /// `FormKeyDown`: without Ctrl or Alt, `1`–`5` and F1–F5 pick the
    /// colourings, the arrows and the number pad scroll the window by 20,
    /// Enter and Escape close the form, Space gives the main screen the
    /// focus, and the other keys go on to the main screen; Alt+F4 closes
    /// it. // 10b8:4c95
    pub fn key_down(&mut self, key: u16, shift: ShiftState, world: &World) -> Response {
        let mut r = Response::default();
        let mods = shift.0 & (ShiftState::SHIFT.0 | ShiftState::ALT.0 | ShiftState::CTRL.0);
        if mods == 0 || mods == ShiftState::SHIFT.0 {
            if let Some(n) = (0..5u16).find(|&n| key == 0x31 + n || key == 0x70 + n) {
                self.select(world, 0x10 + u8::try_from(n).unwrap_or(0));
                r.refresh = true;
            } else if let Some(&(_, _, dy, dx)) = SCROLL_KEYS.iter().find(|k| k.0 == key || k.1 == key) {
                // 10b8:4c59
                self.scroll_by(world, dx, dy);
                self.update_bars();
                r.refresh = true;
            } else if key == 0x0d || key == 0x1b {
                r = self.close();
            } else if key == 0x20 {
                r.focus_main = true;
            } else {
                r.key = Some((key, shift));
            }
        } else if mods == ShiftState::ALT.0 && key == 0x73 {
            r = self.close();
        }
        r
    }

    /// The help text (`helpform`, 1040:3dc7) for the button under `x`,
    /// `y` in `Menubild`, which the main screen's clock shows with the
    /// tooltips on while the form is active and its title wasn't double
    /// clicked, and where the bubble goes in `Menubild`: the button's
    /// left and bottom edges. // 1020:1fe2
    #[must_use]
    pub fn help_text_at(&self, x: i32, y: i32) -> Option<(u8, i32, i32)> {
        if self.double {
            return None;
        }
        (part::OWNERS..=part::OCCUPANCY).find(|&n| self.hit(n, x, y)).map(|n| {
            let [left, _, _, bottom] = self.r(n);
            (HELP_TEXTS[n - part::OWNERS], left, bottom)
        })
    }

    /// The form's fields into the view's `fields` of the map file.
    /// // 10b8:4e34
    pub fn save(&self, fields: &mut Fields) {
        let word = |n: i16| n.to_le_bytes();
        let pair = |(a, b): (i16, i16)| {
            let mut v = a.to_le_bytes().to_vec();
            v.extend_from_slice(&b.to_le_bytes());
            v
        };
        fields.set("[1110:723a]+0x198", 0, &[self.button]);
        fields.set("[1110:723a]+0x199", 0, &[self.mode]);
        fields.set("[1110:723a]+0x19a", 0, &[self.pressed]);
        fields.set("[1110:723a]+0x19b", 0, &word(self.map_size.0));
        fields.set("[1110:723a]+0x19d", 0, &word(self.map_size.1));
        fields.set("[1110:723a]+0x19f", 0, &[u8::from(self.heights)]);
        fields.set("[1110:723a]+0x1a0", 0, &word(self.unused.0));
        fields.set("[1110:723a]+0x1a2", 0, &word(self.unused.1));
        fields.set("[1110:723a]+0x213", 0, &[u8::from(self.small)]);
        fields.set("[1110:723a]+0x1a4", 0, &[self.unused.2]);
        fields.set("[1110:723a]+0x1a5", 0, &[u8::from(self.ns_dragged)]);
        fields.set("[1110:723a]+0x1a6", 0, &[u8::from(self.up_held)]);
        fields.set("[1110:723a]+0x1a7", 0, &[u8::from(self.down_held)]);
        fields.set("[1110:723a]+0x1b0", 0, &pair(self.origin));
        fields.set("[1110:723a]+0x1b4", 0, &pair(self.grab));
        fields.set("[1110:723a]+0x1b8", 0, &pair(self.cursor));
        fields.set("[1110:723a]+0x1bc", 0, &word(self.wo_grab));
        fields.set("[1110:723a]+0x1be", 0, &word(self.ns_grab));
        fields.set("[1110:723a]+0x1c0", 0, &word(self.ns_bar.max));
        fields.set("[1110:723a]+0x1c2", 0, &word(self.ns_bar.min));
        fields.set("[1110:723a]+0x1c4", 0, &word(self.ns_bar.position));
        fields.set("[1110:723a]+0x1c6", 0, &word(self.wo_bar.max));
        fields.set("[1110:723a]+0x1c8", 0, &word(self.wo_bar.min));
        fields.set("[1110:723a]+0x1ca", 0, &word(self.wo_bar.position));
        fields.set("[1110:723a]+0x1cc", 0, &pair(self.centre));
        fields.set("[1110:723a]+0x1d0", 0, &word(self.unused.3));
        fields.set("[1110:723a]+0x212", 0, &[u8::from(self.double)]);
    }

    /// The form's fields from a map file (10b8:50de). `game_show_state`
    /// or `game_start_setup` ([`Uebersichtsmap::game_start`]) lays the
    /// form out and draws it after.
    pub fn load(&mut self, fields: &Fields) {
        let byte = |dest: &str| fields.get(dest, 0).and_then(|d| d.first().copied()).unwrap_or(0);
        let word = |dest: &str| {
            fields
                .get(dest, 0)
                .filter(|d| d.len() >= 2)
                .map_or(0, |d| i16::from_le_bytes([d[0], d[1]]))
        };
        let pair = |dest: &str| {
            fields.get(dest, 0).filter(|d| d.len() >= 4).map_or((0, 0), |d| {
                (i16::from_le_bytes([d[0], d[1]]), i16::from_le_bytes([d[2], d[3]]))
            })
        };
        self.button = byte("[1110:723a]+0x198");
        self.mode = byte("[1110:723a]+0x199");
        self.pressed = byte("[1110:723a]+0x19a");
        self.map_size = (word("[1110:723a]+0x19b"), word("[1110:723a]+0x19d"));
        self.heights = byte("[1110:723a]+0x19f") == 1;
        self.unused = (
            word("[1110:723a]+0x1a0"),
            word("[1110:723a]+0x1a2"),
            byte("[1110:723a]+0x1a4"),
            word("[1110:723a]+0x1d0"),
        );
        self.small = byte("[1110:723a]+0x213") != 0;
        self.ns_dragged = byte("[1110:723a]+0x1a5") != 0;
        self.up_held = byte("[1110:723a]+0x1a6") != 0;
        self.down_held = byte("[1110:723a]+0x1a7") != 0;
        self.origin = pair("[1110:723a]+0x1b0");
        self.grab = pair("[1110:723a]+0x1b4");
        self.cursor = pair("[1110:723a]+0x1b8");
        self.wo_grab = word("[1110:723a]+0x1bc");
        self.ns_grab = word("[1110:723a]+0x1be");
        self.ns_bar = Bar {
            max: word("[1110:723a]+0x1c0"),
            min: word("[1110:723a]+0x1c2"),
            position: word("[1110:723a]+0x1c4"),
        };
        self.wo_bar = Bar {
            max: word("[1110:723a]+0x1c6"),
            min: word("[1110:723a]+0x1c8"),
            position: word("[1110:723a]+0x1ca"),
        };
        self.centre = pair("[1110:723a]+0x1cc");
        self.double = byte("[1110:723a]+0x212") != 0;
        // BUG(original): a saved mode past 7 draws nothing (10b8:17cf); one of 6 or 7 shows the land by slopes or heights' last case.
    }
}

/// The smallest area holding all of `squares`.
fn bounding_box(squares: &[Pos]) -> Option<(Pos, Pos)> {
    let first = *squares.first()?;
    Some(squares.iter().fold((first, first), |(lo, hi), &(i, j)| {
        ((lo.0.min(i), lo.1.min(j)), (hi.0.max(i), hi.1.max(j)))
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thumbs() {
        let bar = Bar {
            max: 227,
            min: 0,
            position: 0,
        };
        // The big layout's vertical bar: 275 high.
        let travel = 275 - 16 - 19;
        assert_eq!(bar.thumb(travel), 16);
        assert_eq!(Bar { position: 227, ..bar }.thumb(travel), travel);
        assert_eq!(bar.position_at(16, travel), 0);
        assert_eq!(bar.position_at(travel, travel), 227);
    }

    #[test]
    fn squares_and_pixels() {
        // 10b8:038d inverts 10b8:0422/0443 within a pixel.
        for (i, j) in [(0, 0), (125, 125), (250, 0), (0, 250), (100, 37)] {
            let (x, y) = (((i - j) + 250) * 2, i + j);
            let half = x / 2;
            assert_eq!(((y + half + 750) / 2 - 500, (y - half + 1250) / 2 - 500), (i, j));
        }
    }

    #[test]
    fn bounding() {
        assert_eq!(bounding_box(&[]), None);
        assert_eq!(bounding_box(&[(3, 9), (5, 2), (4, 4)]), Some(((3, 2), (5, 9))));
    }
}
