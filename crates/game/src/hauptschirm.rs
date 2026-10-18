//! `THauptschirm`, the main screen (segment `1020`): a 640×479 window in the
//! middle of the screen with a title bar, an info bar, the button bar, the
//! island (`Hauptbild`) with a scroll bar on the right and one below, and
//! the status bar. It's that size on every screen; its title bar drags it
//! (`docs/layout.md`).
//!
//! Like the start menu, the form is a set of `TImage`s the game draws into
//! itself. This module keeps one [`Surface`] per image; the caller puts
//! them into the VCL form at the bounds [`LAYOUT`] gives.
//!
//! The scroll position is two numbers, `wo` (west–east, the lower scroll
//! bar) and `ns` (north–south, the right one), in the map's own diagonal
//! coordinates. The camera's origin on the map is worked out from them
//! (1020:0697).
//!
//! With the `big_map` enhancement and a screen bigger than 640×480 the
//! form fills the screen instead ([`Hauptschirm::place`]) and its map
//! view grows. The scroll position, the camera ([`Hauptschirm::camera`])
//! and what the world sees ([`Hauptschirm::viewpoint`]) stay those of the
//! original's 20 × 27 view, which lies in the middle of the bigger one;
//! the island is drawn with a bigger camera around it
//! ([`Hauptschirm::drawn_camera`]). `docs/layout.md`, "`big_map`".
//! Filling the screen, with `no_main_title` it has no title bar
//! ([`layout_for`]).
//!
//! The two scroll bars are pictures the form's handlers make scroll bars
//! of ([`Hauptschirm::bar_mouse_down`], [`Hauptschirm::bar_tick`];
//! `docs/map.md`, "The scroll bars").

use vcl::Rect;
use vcl::surface::{Rop, Surface};

use crate::darstellung::{self, Camera, Tiles, VIEW_HEIGHT, VIEW_WIDTH, ViewSize};
use crate::enhance::{Enhancement, Enhancements};
use crate::monate::Date;
use crate::money::format_money;
use crate::people::Viewpoint;
use crate::text::{Align, Font};
use crate::widgets::{Part, ShiftState};
use crate::world::{Pos, World};

/// The form's size. // 1110:1578, 1110:157a
pub const WIDTH: i32 = 640;
pub const HEIGHT: i32 = 479;
/// The title bar's height, which the form shrinks to while it's dragged.
/// // 1020:1b8e
pub const TITLE_HEIGHT: i32 = 16;

/// The images' bounds in the form, from the layout table at `1110:1572`
/// (entries 2 and 4–10, set by 1020:1ce9), and `CloseBild`'s and
/// `Kleinbild`'s, which `hauptschirm_init` sets. Entries 0, 1 and 3 (the
/// form itself, and two hidden 16×16 slots) aren't images.
pub const LAYOUT: [(&str, Rect); 10] = [
    ("Ueberschriftbild", Rect::new(16, 0, 608, 16)), // 1110:1586
    ("InfoImage", Rect::new(0, 16, 640, 18)),        // 1110:159a
    ("ButtonImage", Rect::new(0, 34, 640, 25)),      // 1110:15a4
    ("Hauptbild", Rect::new(0, 59, 624, 404)),       // 1110:15ae
    ("NSScrollbild", Rect::new(624, 59, 16, 404)),   // 1110:15b8
    ("Statusimage", Rect::new(0, 463, 219, 16)),     // 1110:15c2
    ("WOScrollbild", Rect::new(219, 463, 405, 16)),  // 1110:15cc
    ("eckimage", Rect::new(624, 463, 16, 16)),       // 1110:15d6
    ("CloseBild", Rect::new(0, 0, 16, 16)),          // 1020:2501
    ("Kleinbild", Rect::new(624, 0, 16, 16)),        // 1020:2501
];

/// The images' bounds in a form `width` × `height` (`big_map`): as
/// [`LAYOUT`], with what's at the right and bottom edges moved there and
/// the bars and the island stretched between. The status bar keeps its
/// width. `layout_for(WIDTH, HEIGHT, true)` is [`LAYOUT`].
///
/// Without a `title` bar (`no_main_title`) the title bar and its close
/// and minimize widgets are empty rectangles at the top, and the info
/// bar, the button bar and everything below them move up by
/// [`TITLE_HEIGHT`], the island's view growing by as much.
#[must_use]
pub fn layout_for(width: i32, height: i32, title: bool) -> [(&'static str, Rect); 10] {
    let (w, h) = (width, height);
    let t = if title { TITLE_HEIGHT } else { 0 };
    let bar = |r: Rect| if title { r } else { Rect::new(r.left, 0, 0, 0) };
    [
        ("Ueberschriftbild", bar(Rect::new(16, 0, w - 32, 16))),
        ("InfoImage", Rect::new(0, t, w, 18)),
        ("ButtonImage", Rect::new(0, t + 18, w, 25)),
        ("Hauptbild", Rect::new(0, t + 43, w - 16, h - t - 59)),
        ("NSScrollbild", Rect::new(w - 16, t + 43, 16, h - t - 59)),
        ("Statusimage", Rect::new(0, h - 16, 219, 16)),
        ("WOScrollbild", Rect::new(219, h - 16, w - 16 - 219, 16)),
        ("eckimage", Rect::new(w - 16, h - 16, 16, 16)),
        ("CloseBild", bar(Rect::new(0, 0, 16, 16))),
        ("Kleinbild", bar(Rect::new(w - 16, 0, 16, 16))),
    ]
}

/// The smallest screen `big_map` grows the main screen on: anything
/// bigger than the original's smallest, 640 × 480, either way.
pub const BIG_FROM: (i32, i32) = (640, 480);
/// `buttoff.bmp`'s wooden piece between the button groups (the one at
/// 75), which closes off a wider button bar's panel at both ends.
const BAR_GAP: Rect = Rect::new(75, 0, 13, BUTTON_SIZE);
/// The flat brown panel under the overview map's and money menu's
/// buttons (`uebigmen.bmp`, `geldmenu.bmp`): its fill, light top and left
/// edge and dark bottom and right edge, as palette indices.
const PANEL: [u8; 3] = [187, 185, 188];
/// How much of a scroll bar's trough stays at each end (the arrows and a
/// little more) when `big_map` stretches it.
const TROUGH_END: i32 = 32;

/// The bitmaps the frame is drawn from, from `GRAFIK`, in the order
/// [`Hauptschirm::new`] takes them.
pub const BITMAP_FILES: [&str; 15] = [
    "menulei3.bmp", // the title bar, 1020:1df3
    "statusle.bmp", // the info bar's, despite the name: 1110:600e
    "buttoff.bmp",  // 1110:600a
    "scronosu.BMP", // the right scroll bar's trough, 1110:601a
    "scroweos.BMP", // the lower one's, 1110:601e
    "scroll1.BMP",  // both thumbs, 1110:6016
    "infolei.bmp",  // the status bar's: 1110:6012
    "kompkn.bmp",   // the compass knob, 1020:2501
    "close.bmp",    // 1110:54b8
    "minimize.bmp", // 1020:2501
    "button.bmp",   // the buttons held down, 1110:6006
    "scrbutno.BMP", // the up arrow held down, 1110:6022 (1030:307a)
    "scrbutsu.BMP", // down, 1110:6026
    "scrbutwe.BMP", // left, 1110:602a
    "scrbutos.BMP", // right, 1110:602e
];
const MENULEI: usize = 0;
const INFO: usize = 1;
const BUTTOFF: usize = 2;
const SCRONOSU: usize = 3;
const SCROWEOS: usize = 4;
const SCROLL1: usize = 5;
const STATUS: usize = 6;
const KOMPKN: usize = 7;
const CLOSE: usize = 8;
const MINIMIZE: usize = 9;
const BUTTON: usize = 10;
const UP_PRESSED: usize = 11;
const DOWN_PRESSED: usize = 12;
const LEFT_PRESSED: usize = 13;
const RIGHT_PRESSED: usize = 14;

/// The left edges of the 23 buttons in the button bar, each 25 wide.
/// // 1110:12c0
const BUTTONS: [i32; 23] = [
    0, 25, 50, 88, 113, 138, 176, 201, 226, 264, 289, 314, 339, 364, 389, 414, 439, 464, 489, 514, 539, 577, 615,
];
const BUTTON_SIZE: i32 = 25;

/// Where `hauptschirm_init` puts a new game's view before the map file's
/// position replaces it: +0x408 and +0x40a. // 1020:2501
pub const START_WO: i16 = 240;
pub const START_NS: i16 = 237;

/// A scroll bar's range and position: +0x1bc (max), +0x1be (min), +0x1c0
/// (position) for the right one, +0x1c2.. for the lower one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Bar {
    min: i16,
    max: i16,
    position: i16,
}

impl Bar {
    fn clamp(self, n: i16) -> i16 {
        n.max(self.min).min(self.max)
    }

    /// Where the thumb starts along a bar `length` pixels long: after the
    /// 16-pixel arrow, over the rest less both arrows and the thumb.
    /// // 1020:0b38, 1020:12c0
    fn thumb(self, length: i32) -> i32 {
        let range = i32::from(self.max) - i32::from(self.min);
        if range == 0 {
            return 16;
        }
        (16 * range + (i32::from(self.position) - i32::from(self.min)) * (length - 16 - 19 - 16)) / range
    }
}

/// One of the two scroll bars.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    /// `NSScrollbild`, the right one: north–south, down the screen.
    Ns,
    /// `WOScrollbild`, the lower one: west–east, across.
    Wo,
}

/// A press on a scroll bar, until the mouse comes up: an arrow shows
/// pressed while it's held (+0x1d0, +0x1d1 for the right bar's, +0x1ce,
/// +0x1cf for the lower one's), an arrow or the trough repeats in the
/// clock's tick (the `Tag` of the component at +0x190, 1 to 4 for the
/// right bar's up arrow, page up, page down and down arrow, 5 to 8 for
/// the lower one's), the thumb is dragged (+0x1cd, +0x1cc).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BarPress {
    axis: Axis,
    part: Part,
    /// Where the thumb was grabbed, from its start. // +0x1ba, +0x1b8
    grab: i32,
    /// Where the last move put the thumb's start, as the mouse had it
    /// (both bars share the `Longint`). // +0x1c8
    last: i32,
    /// The mouse along the bar when last seen (the port's, for
    /// `scroll_bars`).
    at: i32,
    /// The clock's ticks since the press (the port's, for
    /// `scroll_bars`' wait before repeating).
    ticks: u8,
}

/// An arrow's step, on both bars. // 1020:0d92, 1020:151a
const BAR_STEP: i16 = 1;
/// The trough's step: 16 half rows down the right bar, 18 columns across
/// the lower one. // 1020:0d92 (0x10), 1020:151a (0x12)
const NS_PAGE: i16 = 0x10;
const WO_PAGE: i16 = 0x12;
/// The thumb's length along the bar (`scroll1.BMP`'s 16 × 20 pieces).
const THUMB: i32 = 20;
/// With `scroll_bars`, a part held repeats from the clock's third tick
/// after the press (200 to 300 ms), so a click is one step.
const REPEAT_WAIT: u8 = 2;
/// The mouse wheel's step a notch (`mouse_wheel`): two, as the arrow
/// keys' (1020:2e09).
pub const WHEEL_STEP: i16 = 2;

/// The part of a scroll bar `length` long at `along`, as the presses
/// test it (1020:0d92, 1020:151a): the back arrow up to 16 (`< 0x11`),
/// the forward arrow from `length - 16`, and between them the thumb from
/// its start and 15 on, the trough before and after it. With
/// `whole_thumb` (`scroll_bars`) all 20 pixels of the thumb.
fn part_at(bar: Bar, length: i32, along: i32, whole_thumb: bool) -> Part {
    if along < 0x11 {
        return Part::BackArrow;
    }
    if along >= length - 0x10 {
        return Part::ForwardArrow;
    }
    let thumb = bar.thumb(length);
    // BUG(original): the thumb is 20 pixels long, but only its first 16 grab it: a press on its last four pages forward.
    let end = if whole_thumb { thumb + THUMB - 1 } else { thumb + 0xf };
    if along < thumb {
        Part::PageBack
    } else if along > end {
        Part::PageForward
    } else {
        Part::Thumb
    }
}

/// The right scroll bar's range. // 1020:2501 (1020:0b0f)
const NS_MAX: i16 = 476;
/// The lower scroll bar's range. // 1020:2501 (1020:1297)
const WO_MAX: i16 = 481;

/// Scrolling with the keyboard: each arrow (or the number pad) moves two
/// steps, six with Ctrl, in `ns` and `wo`. // 1020:2e09
const SCROLL_KEYS: [(u16, u16, i16, i16); 8] = [
    (0x23, 0x61, 1, -1),  // End
    (0x28, 0x62, 1, 0),   // Down
    (0x22, 0x63, 1, 1),   // Page Down
    (0x25, 0x64, 0, -1),  // Left
    (0x27, 0x66, 0, 1),   // Right
    (0x24, 0x67, -1, -1), // Home
    (0x26, 0x68, -1, 0),  // Up
    (0x21, 0x69, -1, 1),  // Page Up
];
/// Insert and Delete turn the view. // 1020:2e09, 1110:007e, 1110:0082
const VK_INSERT: [u16; 2] = [0x2d, 0x60];

/// The keys that press a button of the button bar, without Ctrl or Alt,
/// and the button: `1`/`D` the file menu, `2`/Pause/`P`, ... and F1 to
/// F12 the buttons 9 to 0x14. // 1020:2e09, 1020:2daa
const BUTTON_KEYS: [(&[u16], u8); 23] = [
    (&[0x31, 0x44], 0),
    (&[0x32, 0x13, 0x50], 1),
    (&[0x33, 0x4f], 2),
    (&[0x34, 0x4e], 3),
    (&[0x35, 0x47], 4),
    (&[0x36, 0x55, 0xba], 5),
    (&[0x37, 0x48], 6),
    (&[0x38, 0x53], 7),
    (&[0x39, 0x41], 8),
    (&[0x30, 0x59], 0x15),
    (&[0xdb, 0x49], 0x16),
    (&[0x70], 9),
    (&[0x71], 10),
    (&[0x72], 0xb),
    (&[0x73], 0xc),
    (&[0x74], 0xd),
    (&[0x75], 0xe),
    (&[0x76], 0xf),
    (&[0x77], 0x10),
    (&[0x78], 0x11),
    (&[0x79], 0x12),
    (&[0x7a], 0x13),
    (&[0x7b], 0x14),
];

/// The button bar's file menu button. // 1010:2c8b
pub const FILE_MENU_BUTTON: u8 = 0;
const VK_DELETE: [u16; 2] = [0x2e, 0x6e];

/// What a key did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyResult {
    /// The view moved or turned and was redrawn.
    Redrawn,
    /// Not a key this port handles yet.
    Ignored,
    /// The key presses button `n` of the button bar (1020:2daa, then
    /// 1010:2c8b, which opens or closes what the button stands for).
    Button(u8),
}

/// The button bar's button 10, the hotels' submenu, which the later
/// build's scenario 10 (B&B only) takes away: the mouse (`button_at_mouse`)
/// finds no button there, F2 does nothing and the build menu's arrows go
/// past it. // en 1018:363c, en 1020:2e09, en 1018:32a5
pub const HOTELS_BUTTON: u8 = 10;
const BB_ONLY: u8 = 10;

/// Whether `button` can't be used in `scenario` ([`HOTELS_BUTTON`] in
/// scenario 10).
#[must_use]
pub fn button_blocked(scenario: u8, button: u8) -> bool {
    scenario == BB_ONLY && button == HOTELS_BUTTON
}

/// What the info bar's texts need besides the world.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InfoTexts {
    /// `MONATE.TXT`. // [1110:65d6]
    pub months: Vec<Vec<u8>>,
    /// The player's language, for the date's and the money's format,
    /// which the original takes from `MODATYP.DAT`'s country (`land`,
    /// `1110:64be`; [`crate::lang`]).
    pub language: crate::lang::Language,
    /// `Waehrungsfaktor` from `SONSTIGE.TXT`. // 1110:7346
    pub currency_factor: i32,
    /// The currency's name, `SONSTIGE.TXT`'s sixth text. // *1110:7352
    pub currency: Vec<u8>,
}

/// The main screen's state and images.
#[allow(clippy::struct_excessive_bools)] // The original's flags.
#[derive(Debug, Clone)]
pub struct Hauptschirm {
    /// The camera of the original's 20 × 27 view: what a map file saves
    /// and the world sees. With `big_map` the island is drawn with
    /// [`Hauptschirm::drawn_camera`].
    pub camera: Camera,
    /// +0x408
    wo: i16,
    /// +0x40a
    ns: i16,
    ns_bar: Bar,
    wo_bar: Bar,
    /// After turning the view the origin is worked out with 1020:06f1,
    /// which keeps it, instead of 1020:0697, until the next scroll.
    /// // 1110:133e
    keep_origin: bool,
    /// The date and player 1's money the info bar shows, once drawn.
    info_shown: Option<(Date, i32)>,
    /// The ticks the status bar's message stays. // 1110:009a
    status_timeout: i16,
    bitmaps: Vec<Surface>,
    /// The island without the people, which [`Hauptschirm::show_people`]
    /// draws over it into [`Hauptschirm::map`]. The original keeps what's
    /// under each picture instead (1088:3496).
    island: Surface,
    pub title: Surface,
    pub info: Surface,
    pub buttons: Surface,
    pub map: Surface,
    pub ns_image: Surface,
    pub wo_image: Surface,
    pub status: Surface,
    pub corner: Surface,
    pub close: Surface,
    pub minimize: Surface,
    /// The form's top left on the screen: [`Hauptschirm::layout`] centres
    /// it, dragging its title bar moves it. `Left`, `Top`
    pub left: i32,
    pub top: i32,
    /// The form's height: [`HEIGHT`], or [`TITLE_HEIGHT`] while its title
    /// bar is held or after a double click on it. `Height`
    pub height: i32,
    /// Where the title bar was grabbed, from the form's top left.
    /// // 1110:6046, 1110:6048
    grab: (i32, i32),
    /// The last press on the title bar was a double click, which leaves
    /// the form rolled up to its title bar. // +0x1da
    pub rolled_up: bool,
    /// The `big_map` enhancement is on: [`Hauptschirm::place`] lets the
    /// form fill a screen bigger than [`BIG_FROM`].
    pub big_map: bool,
    /// The form's size: [`WIDTH`] × [`HEIGHT`], or with `big_map` the
    /// screen's.
    width: i32,
    full_height: i32,
    /// The images' bounds at that size ([`layout_for`]).
    rects: [(&'static str, Rect); 10],
    /// The map view `Hauptbild` shows.
    view_size: ViewSize,
    /// The bars' backgrounds at the form's width and height: the info
    /// bar's, the scroll bars' troughs.
    info_bar: Surface,
    ns_trough: Surface,
    wo_trough: Surface,
    /// The font and the game's name, for the title bar.
    font: Font,
    title_text: Vec<u8>,
    /// The player's view enhancements (`form_drags`, `overview_scrolling`).
    pub enhance: Enhancements,
    /// The title bar was pressed and the button hasn't come up yet
    /// (`form_drags`; the original doesn't keep it).
    title_held: bool,
    /// The form has its title bar: always but with `no_main_title` when
    /// it fills the screen.
    titled: bool,
    /// A scroll bar held down.
    bar_press: Option<BarPress>,
}

/// `src` repeated over a `width` × `height` surface.
fn tiled(src: &Surface, width: i32, height: i32) -> Surface {
    let mut dst = Surface::new(
        usize::try_from(width).unwrap_or(0),
        usize::try_from(height).unwrap_or(0),
        0,
    );
    let all = dst.bounds();
    let (sw, sh) = (src.bounds().width().max(1), src.bounds().height().max(1));
    for y in (0..height).step_by(usize::try_from(sh).unwrap_or(1)) {
        for x in (0..width).step_by(usize::try_from(sw).unwrap_or(1)) {
            dst.blit(x, y, src, src.bounds(), Rop::Copy, all);
        }
    }
    dst
}

/// A scroll bar's trough `length` long (across if `across`, else down)
/// from `src`, which the original shows from its start, `original` long:
/// that as it is if it's no longer, else its ends ([`TROUGH_END`]) at
/// the ends and its middle repeated between (`big_map`).
fn trough(src: &Surface, length: i32, original: i32, across: bool) -> Surface {
    let thickness = if across {
        src.bounds().height()
    } else {
        src.bounds().width()
    };
    let (w, h) = if across {
        (length, thickness)
    } else {
        (thickness, length)
    };
    let mut dst = Surface::new(usize::try_from(w).unwrap_or(0), usize::try_from(h).unwrap_or(0), 0);
    let all = dst.bounds();
    // A piece of the source along the bar, `from`..`from + n`, to `at`.
    let mut piece = |from: i32, n: i32, at: i32| {
        let (from_rect, x, y) = if across {
            (Rect::new(from, 0, n, thickness), at, 0)
        } else {
            (Rect::new(0, from, thickness, n), 0, at)
        };
        dst.blit(x, y, src, from_rect, Rop::Copy, all);
    };
    if length <= original {
        piece(0, length, 0);
        return dst;
    }
    let middle = (original - 2 * TROUGH_END).max(1);
    piece(0, TROUGH_END, 0);
    let mut at = TROUGH_END;
    while at < length - TROUGH_END {
        let n = middle.min(length - TROUGH_END - at);
        piece(TROUGH_END, n, at);
        at += n;
    }
    piece(original - TROUGH_END, TROUGH_END, length - TROUGH_END);
    dst
}

fn image(rects: &[(&str, Rect); 10], name: &str) -> Surface {
    let r = rects
        .iter()
        .find(|(n, _)| *n == name)
        .map_or(Rect::default(), |(_, r)| *r);
    Surface::new(
        usize::try_from(r.width()).unwrap_or(0),
        usize::try_from(r.height()).unwrap_or(0),
        0,
    )
}

/// Copies `src`'s top left over all of `dst`, as the game's `BitBlt(..,
/// Width, Height, .., 0, 0, SRCCOPY)` calls do.
fn copy(dst: &mut Surface, src: &Surface) {
    let all = dst.bounds();
    dst.blit(0, 0, src, all, Rop::Copy, all);
}

/// The origin and parity the scroll position `wo`, `ns` shows; with
/// `keep`, as after turning the view. // 1020:0697, 1020:06f1
fn scroll_origin(wo: i16, ns: i16, keep: bool) -> (Pos, i16) {
    let (wo, ns) = (i32::from(wo), i32::from(ns));
    // `div 2`, truncating, of sums that may round differently.
    let (i, j) = if keep {
        (wo + ns + 1002, ns - wo + 1001)
    } else {
        (wo + ns + 1001, ns + 1000 - wo)
    };
    let (i, j) = (i / 2 - 626, j / 2 - 375);
    let short = |n: i32| i16::try_from(n).unwrap_or(0);
    ((short(i), short(j)), short((wo + 250 + ns) % 2))
}

/// The view a new island is first seen from: `hauptschirm_init`'s scroll
/// position ([`START_WO`], [`START_NS`]), turned 0. A generated island
/// gets no `game_start_setup`, so it stays there. // 1020:2501
#[must_use]
pub fn start_viewpoint() -> Viewpoint {
    let (origin, parity) = scroll_origin(START_WO, START_NS, false);
    Viewpoint {
        rotation: 0,
        origin,
        parity,
    }
}

impl Hauptschirm {
    /// The main screen as `hauptschirm_init` (1020:2501) and the start of a
    /// game (`game_start_setup`, 1010:2820) leave it, looking at the map
    /// with the `camera` and `origin` (`1110:5530`) a map file saved.
    /// `bitmaps` are those of [`BITMAP_FILES`], `title` is `SONSTIGE.TXT`'s
    /// first text.
    ///
    /// # Panics
    /// If there aren't as many bitmaps as files.
    #[must_use]
    pub fn new(camera: Camera, origin: Pos, bitmaps: Vec<Surface>, font: &Font, title: &[u8]) -> Self {
        Hauptschirm::build(camera, Some(origin), bitmaps, font, title)
    }

    /// The main screen for a new island, as `hauptschirm_init` (1020:2501)
    /// leaves it without a map file: at its first scroll position
    /// ([`start_viewpoint`]).
    ///
    /// # Panics
    /// If there aren't as many bitmaps as files.
    #[must_use]
    pub fn new_game(camera: Camera, bitmaps: Vec<Surface>, font: &Font, title: &[u8]) -> Self {
        Hauptschirm::build(camera, None, bitmaps, font, title)
    }

    fn build(camera: Camera, origin: Option<Pos>, bitmaps: Vec<Surface>, font: &Font, title: &[u8]) -> Self {
        assert_eq!(bitmaps.len(), BITMAP_FILES.len(), "one bitmap per file");
        let image = |name| image(&LAYOUT, name);
        let info_bar = bitmaps[INFO].clone();
        let ns_trough = trough(&bitmaps[SCRONOSU], LAYOUT[4].1.height(), LAYOUT[4].1.height(), false);
        let wo_trough = trough(&bitmaps[SCROWEOS], LAYOUT[6].1.width(), LAYOUT[6].1.width(), true);
        let mut h = Hauptschirm {
            camera,
            wo: START_WO,
            ns: START_NS,
            ns_bar: Bar {
                min: 0,
                max: NS_MAX,
                position: START_NS,
            },
            wo_bar: Bar {
                min: 0,
                max: WO_MAX,
                position: START_WO,
            },
            keep_origin: false,
            info_shown: None,
            status_timeout: 0,
            title: bitmaps
                .get(MENULEI)
                .cloned()
                .unwrap_or_else(|| image("Ueberschriftbild")),
            bitmaps,
            info: image("InfoImage"),
            buttons: image("ButtonImage"),
            map: image("Hauptbild"),
            island: image("Hauptbild"),
            ns_image: image("NSScrollbild"),
            wo_image: image("WOScrollbild"),
            status: image("Statusimage"),
            corner: image("eckimage"),
            close: image("CloseBild"),
            minimize: image("Kleinbild"),
            left: 0,
            top: 0,
            height: HEIGHT,
            grab: (0, 0),
            rolled_up: false,
            big_map: false,
            width: WIDTH,
            full_height: HEIGHT,
            rects: LAYOUT,
            view_size: ViewSize::ORIGINAL,
            info_bar,
            ns_trough,
            wo_trough,
            font: font.clone(),
            title_text: title.to_vec(),
            enhance: Enhancements::ORIGINAL,
            title_held: false,
            titled: true,
            bar_press: None,
        };
        // 1020:1e4f: the game's name over the title bar.
        let width = LAYOUT[0].1.width();
        font.draw(&mut h.title, title, (0, 1), width, Align::Center, 1);
        h.paint_frame();
        match origin {
            Some(origin) => h.set_origin(origin),
            None => h.update_bars(),
        }
        h
    }

    /// The images that don't show the game's state.
    fn paint_frame(&mut self) {
        let b = &self.bitmaps;
        copy(&mut self.info, &self.info_bar); // 1030:213f
        copy(&mut self.status, &b[STATUS]); // 1030:21a8
        // 1010:2b3b: the whole bar first (button 0x17 is all 640 pixels,
        // the wooden pieces between the groups too), then 1010:2af5: every
        // button up, from buttoff.bmp (1050:0074).
        let all = self.buttons.bounds();
        let whole = Rect::new(0, 0, WIDTH, BUTTON_SIZE);
        self.buttons.blit(0, 0, &b[BUTTOFF], whole, Rop::Copy, all);
        for x in BUTTONS {
            let r = Rect::new(x, 0, BUTTON_SIZE, BUTTON_SIZE);
            self.buttons.blit(x, 0, &b[BUTTOFF], r, Rop::Copy, all);
        }
        // The compass knob, close and minimize widgets, drawn up.
        for (dst, index) in [
            (&mut self.corner, KOMPKN),
            (&mut self.close, CLOSE),
            (&mut self.minimize, MINIMIZE),
        ] {
            let all = dst.bounds();
            dst.draw(0, 0, &b[index], all);
        }
        self.fill_button_bar();
    }

    /// `big_map`'s wider button bar: right of the last button, a wooden
    /// piece like those between the button groups, the brown panel the
    /// other menus have under their buttons, and a wooden piece at the end.
    fn fill_button_bar(&mut self) {
        let all = self.buttons.bounds();
        let gap = BAR_GAP.width();
        if all.width() <= WIDTH {
            return;
        }
        let gaps = [WIDTH, (all.width() - gap).max(WIDTH)];
        for x in gaps {
            self.buttons.blit(x, 0, &self.bitmaps[BUTTOFF], BAR_GAP, Rop::Copy, all);
        }
        let (left, right) = (WIDTH + gap, gaps[1]);
        if right <= left {
            return;
        }
        let (w, h) = (right - left, BUTTON_SIZE);
        let [fill, light, dark] = PANEL;
        self.buttons.fill_rect(Rect::new(left, 0, w, h), all, fill);
        self.buttons.fill_rect(Rect::new(left, 0, w, 1), all, light);
        self.buttons.fill_rect(Rect::new(left, 0, 1, h), all, light);
        self.buttons.fill_rect(Rect::new(left, h - 1, w, 1), all, dark);
        self.buttons.fill_rect(Rect::new(right - 1, 0, 1, h), all, dark);
    }

    /// Puts the info bar's background back from `x` on, `width` wide.
    /// // 1030:213f
    fn restore_info_bar(&mut self, x: i32, width: i32) {
        let all = self.info.bounds();
        let from = Rect::new(x, 0, width, all.height());
        self.info.blit(x, 0, &self.info_bar, from, Rop::Copy, all);
    }

    /// Shows the world's date and player 1's money on the info bar if they
    /// changed since it last did, and says whether it did. The original
    /// draws them where they change: the money in `add_money` (1078:01e7)
    /// and the date in the clock's tick (1020:1e9b); both at the start of
    /// a game (1010:26fa).
    pub fn show_info(&mut self, world: &World, font: &Font, texts: &InfoTexts) -> bool {
        let (date, money) = (world.date, world.finances.money[0]);
        let shown = self.info_shown;
        if shown.is_some_and(|(_, m)| m == money) && shown.is_some_and(|(d, _)| d == date) {
            return false;
        }
        if shown.is_none_or(|(_, m)| m != money) {
            // 1078:01e7, 1030:2211: the amount, a space, the currency;
            // in colour 11 when it's negative.
            let mut text = format_money(money, texts.currency_factor, texts.language);
            text.push(b' '); // 1078:01e5
            text.extend_from_slice(&texts.currency);
            text.truncate(255);
            self.restore_info_bar(0, 152);
            let colour = if text.first() == Some(&b'-') { 11 } else { 10 };
            font.draw(&mut self.info, &text, (4, 1), 120, Align::Right, colour);
        }
        if shown.is_none_or(|(d, _)| d != date) {
            // 1030:0148, 1030:228d; with `big_map` at the wider bar's
            // right end.
            let text = date.text(&texts.months, texts.language);
            let right = self.width - WIDTH;
            self.restore_info_bar(400 + right, 640);
            font.draw(&mut self.info, &text, (450 + right, 1), 182, Align::Right, 10);
        }
        self.info_shown = Some((date, money));
        true
    }

    /// The scroll position for a map origin. // 1020:074d
    fn set_origin(&mut self, (i, j): Pos) {
        self.wo = i + 250 - j;
        self.ns = i + j;
        self.update_bars();
    }

    /// The camera's origin and parity from the scroll position.
    /// // 1020:0697, 1020:06f1
    fn origin(&self) -> (Pos, i16) {
        scroll_origin(self.wo, self.ns, self.keep_origin)
    }

    /// The view jumps to the square `(i, j)`: the view's origin goes to
    /// `(i - 10, j)`, the scroll position and bars follow it and the island
    /// is drawn again. From the TV (the news' square, `crate::fernseher`)
    /// and the buildings' overview's "Springe hin" (`crate::umenuform`).
    /// The original also redraws the overview map's frame
    /// (`TUebersichtsmap2`, 10b8:1485, 10b8:1349), which is the caller's
    /// (`Uebersichtsmap::set_main_view`), and the caller hands the world
    /// the new viewpoint. // 1020:0787
    pub fn jump_to(&mut self, (i, j): Pos, world: &World, tiles: &Tiles) {
        self.set_origin((i.wrapping_sub(10), j)); // 1020:074d, 1020:05a7
        self.paint_map(world, tiles); // 1050:2700
    }

    /// The origin to the camera, and the scroll bars to the position, as
    /// the view sees it. // 1020:05a7
    fn update_bars(&mut self) {
        let ((i, j), parity) = self.origin();
        self.camera.origin = (i, j);
        self.camera.parity = parity;
        // The bars' ends, which are the original's `NS_MAX` and `WO_MAX`;
        // `big_map`'s bars are shorter by the bigger view's margins.
        let (ns, wo) = match self.camera.view & 3 {
            0 => (self.ns, self.wo),
            1 => (NS_MAX - self.wo, self.ns),
            2 => (NS_MAX - self.ns, WO_MAX - self.wo),
            _ => (self.wo, WO_MAX - self.ns),
        };
        self.ns_bar.position = ns;
        self.wo_bar.position = wo;
        self.draw_bars();
    }

    /// Draws both scroll bars: the trough, an arrow held down (its
    /// `scrbut*.BMP` over the trough's, 1020:0d92, 1020:151a; the trough's
    /// back when it comes up, 1020:10b6, 1020:181f), then the thumb.
    fn draw_bars(&mut self) {
        let pressed = |axis: Axis| {
            self.bar_press
                .filter(|p| p.axis == axis)
                .map(|p| p.part)
                .filter(|&p| matches!(p, Part::BackArrow | Part::ForwardArrow))
        };
        let (ns_arrow, wo_arrow) = (pressed(Axis::Ns), pressed(Axis::Wo));
        let b = &self.bitmaps;
        let scroll = &b[SCROLL1];
        let arrow = Rect::new(0, 0, 16, 16);
        // 1020:0c8b, 1020:0bf4: the trough, then the thumb.
        let img = &mut self.ns_image;
        copy(img, &self.ns_trough);
        let length = i32::try_from(img.height).unwrap_or(0);
        let all = img.bounds();
        match ns_arrow {
            Some(Part::BackArrow) => img.blit(0, 0, &b[UP_PRESSED], arrow, Rop::Copy, all),
            Some(_) => img.blit(0, length - 16, &b[DOWN_PRESSED], arrow, Rop::Copy, all),
            None => {}
        }
        let y = self.ns_bar.thumb(length);
        img.blit(0, y, scroll, Rect::new(0, 16, 16, 20), Rop::And, all);
        img.blit(0, y, scroll, Rect::new(16, 16, 16, 20), Rop::Invert, all);
        // 1020:1413, 1020:137c
        let img = &mut self.wo_image;
        copy(img, &self.wo_trough);
        let length = i32::try_from(img.width).unwrap_or(0);
        let all = img.bounds();
        match wo_arrow {
            Some(Part::BackArrow) => img.blit(0, 0, &b[LEFT_PRESSED], arrow, Rop::Copy, all),
            Some(_) => img.blit(length - 16, 0, &b[RIGHT_PRESSED], arrow, Rop::Copy, all),
            None => {}
        }
        let x = self.wo_bar.thumb(length);
        img.blit(x, 0, scroll, Rect::new(0, 0, 20, 16), Rop::And, all);
        img.blit(x, 0, scroll, Rect::new(20, 0, 20, 16), Rop::Invert, all);
    }

    fn bar(&self, axis: Axis) -> Bar {
        match axis {
            Axis::Ns => self.ns_bar,
            Axis::Wo => self.wo_bar,
        }
    }

    /// The bar's length: `NSScrollbild`'s `Height`, `WOScrollbild`'s
    /// `Width` (+0x24, +0x22).
    fn bar_length(&self, axis: Axis) -> i32 {
        let n = match axis {
            Axis::Ns => self.ns_image.height,
            Axis::Wo => self.wo_image.width,
        };
        i32::try_from(n).unwrap_or(0)
    }

    /// `hauptschirm_scroll_ns`, `hauptschirm_scroll_wo`: the bar's
    /// position moves `by`, kept within its ends, and the view moves as
    /// much down or across the screen ([`Hauptschirm::scroll`]). The
    /// original then draws the overview map's frame again, which is the
    /// caller's. // 1020:0cf7, 1020:147f
    fn scroll_bar(&mut self, axis: Axis, by: i16, world: &World, tiles: &Tiles) {
        let bar = self.bar(axis);
        let mut to = bar.position.wrapping_add(by);
        if to < bar.min {
            to = bar.min;
        } else if bar.max < to {
            to = bar.max;
        }
        let by = to.wrapping_sub(bar.position);
        match axis {
            Axis::Ns => self.scroll(by, 0, world, tiles),
            Axis::Wo => self.scroll(0, by, world, tiles),
        }
    }

    /// What holding `part` of the `axis` bar steps by.
    fn bar_step(axis: Axis, part: Part) -> i16 {
        let page = match axis {
            Axis::Ns => NS_PAGE,
            Axis::Wo => WO_PAGE,
        };
        match part {
            Part::BackArrow => -BAR_STEP,
            Part::PageBack => -page,
            Part::PageForward => page,
            Part::ForwardArrow => BAR_STEP,
            Part::Thumb => 0,
        }
    }

    /// `NSScrollBildMouseDown`, `WOScrollBildMouseDown`: a press `along`
    /// the bar (down the right one, across the lower one). An arrow
    /// shows pressed and steps one, the trough beside the thumb pages, and
    /// both go on in the clock's tick while the mouse is held
    /// ([`Hauptschirm::bar_tick`]); the thumb is grabbed. Only a press of
    /// exactly the left button counts; with `modifier_clicks` Shift, Ctrl
    /// and a double click don't matter. Whether the view moved, and the
    /// caller follows it as after [`Hauptschirm::scroll`].
    /// // 1020:0d92, 1020:151a
    pub fn bar_mouse_down(&mut self, axis: Axis, along: i32, shift: ShiftState, world: &World, tiles: &Tiles) -> bool {
        // BUG(original): only exactly [ssLeft]: with Shift or Ctrl held, or on a double click's second press, the bars do nothing, so quick clicks on an arrow step every other time.
        let shift = if self.enhance.has(Enhancement::ModifierClicks) {
            ShiftState(shift.without_keys().0 & !ShiftState::DOUBLE.0)
        } else {
            shift
        };
        if shift != ShiftState::LEFT {
            return false;
        }
        let (bar, length) = (self.bar(axis), self.bar_length(axis));
        let part = part_at(bar, length, along, self.enhance.has(Enhancement::ScrollBars));
        let grab = along - bar.thumb(length);
        self.bar_press = Some(BarPress {
            axis,
            part,
            grab,
            last: along - grab,
            at: along,
            ticks: 0,
        });
        if part == Part::Thumb {
            return false;
        }
        self.scroll_bar(axis, Self::bar_step(axis, part), world, tiles);
        true
    }

    /// `NSScrollbildMouseMove`, `WOScrollbildMouseMove` at `along`: while
    /// the left button drags the thumb, the view follows. Whether it
    /// moved. // 1020:0fa2, 1020:172a
    ///
    /// The original moves the position by the mouse's move since the last
    /// one, scaled by the bar's length less 48, not the thumb's travel
    /// (less 51), rounded by adding half the divisor also to a move back,
    /// and only while the mouse is between the arrows; so the thumb
    /// creeps away from the mouse, faster going back. With `scroll_bars`
    /// the thumb goes where the mouse holds it.
    pub fn bar_mouse_move(&mut self, axis: Axis, along: i32, shift: ShiftState, world: &World, tiles: &Tiles) -> bool {
        let Some(mut p) = self.bar_press.filter(|p| p.axis == axis) else {
            return false;
        };
        p.at = along;
        self.bar_press = Some(p);
        if p.part != Part::Thumb || !shift.contains(ShiftState::LEFT) {
            return false;
        }
        let (bar, length) = (self.bar(axis), self.bar_length(axis));
        let range = i32::from(bar.max) - i32::from(bar.min);
        let short = |n: i32| i16::try_from(n).unwrap_or(if n < 0 { i16::MIN } else { i16::MAX });
        if self.enhance.has(Enhancement::ScrollBars) {
            let travel = length - 16 - 19 - 16;
            let at = (along - p.grab - 16).max(0).min(travel.max(0));
            let Some(offset) = (at * range + travel / 2).checked_div(travel) else {
                return false;
            };
            let to = short(i32::from(bar.min) + offset);
            self.scroll_bar(axis, to.wrapping_sub(bar.position), world, tiles);
            return true;
        }
        // BUG(original): the drag moves by the mouse's moves, scaled by the bar less 48 (not the thumb's travel) and rounded up also going back, only between the arrows: the thumb drifts from the mouse.
        if !(0x10 < along && along < length - 0x10) {
            return false;
        }
        let mut at = along - p.grab;
        if at < 0 {
            at = 0;
        } else if length - 0xe < at {
            at = length - 0xe;
        }
        let Some(by) = ((at - p.last) * range + (length - 0x2f) / 2).checked_div(length - 0x30) else {
            return false;
        };
        self.scroll_bar(axis, short(by), world, tiles);
        if let Some(q) = &mut self.bar_press {
            q.last = at;
        }
        true
    }

    /// `NSScrollBildMouseUp`, `WOScrollBildMouseUp`: an arrow held comes
    /// up, the thumb is let go and the clock stops repeating. Whether the
    /// bar was drawn again. // 1020:10b6, 1020:181f
    pub fn bar_mouse_up(&mut self) -> bool {
        let arrow = self
            .bar_press
            .take()
            .is_some_and(|p| matches!(p.part, Part::BackArrow | Part::ForwardArrow));
        if arrow {
            self.draw_bars();
        }
        arrow
    }

    /// The clock's tick's part for the scroll bars: an arrow or the
    /// trough held steps again, by its press's step. Whether it did, and
    /// the caller follows the view as after [`Hauptschirm::scroll`].
    /// // 1020:1e9b (the `Tag`'s cases 1 to 8)
    ///
    /// With `scroll_bars` the repeat waits two ticks after the press, and
    /// steps only while the mouse is on the part held: an arrow, or the
    /// trough on the side of the thumb it was pressed, so paging stops
    /// when the thumb reaches the mouse.
    pub fn bar_tick(&mut self, world: &World, tiles: &Tiles) -> bool {
        let Some(mut p) = self.bar_press else {
            return false;
        };
        if p.part == Part::Thumb {
            return false;
        }
        if self.enhance.has(Enhancement::ScrollBars) {
            p.ticks = p.ticks.saturating_add(1);
            self.bar_press = Some(p);
            let (bar, length) = (self.bar(p.axis), self.bar_length(p.axis));
            if p.ticks <= REPEAT_WAIT || part_at(bar, length, p.at, true) != p.part {
                return false;
            }
        }
        // BUG(original): the repeat starts at the next tick and goes on wherever the mouse is: holding the trough pages past the mouse to the bar's end.
        self.scroll_bar(p.axis, Self::bar_step(p.axis, p.part), world, tiles);
        true
    }

    /// Whether a scroll bar is held, so that the clock's tick may move
    /// the view.
    #[must_use]
    pub fn bar_held(&self) -> bool {
        self.bar_press.is_some()
    }

    /// The mouse wheel (`mouse_wheel`, a port extension): `dy` notches up
    /// move the view up the screen, `dx` notches right move it right,
    /// [`WHEEL_STEP`] each, as the arrow keys do ([`Hauptschirm::scroll`],
    /// 1020:07dc). Whether it moved, and the caller follows it.
    pub fn wheel(&mut self, dx: i32, dy: i32, world: &World, tiles: &Tiles) -> bool {
        if (dx, dy) == (0, 0) {
            return false;
        }
        let step = |n: i32| {
            let n = n.saturating_mul(i32::from(WHEEL_STEP)).clamp(-1000, 1000);
            i16::try_from(n).unwrap_or(0)
        };
        let before = self.scroll_position();
        self.scroll(step(-dy), step(dx), world, tiles);
        self.scroll_position() != before
    }

    /// The view part of `game_show_state` after a game was loaded in
    /// the middle of another: the loaded camera, the scroll position
    /// worked out from its origin, the scroll bars, the island and the
    /// info bar drawn again. // 1010:26fa, 1020:074d
    pub fn show_loaded(&mut self, camera: Camera, world: &World, tiles: &Tiles) {
        self.camera = camera;
        self.keep_origin = false;
        self.set_origin(camera.origin);
        self.paint_map(world, tiles);
        self.info_shown = None;
    }

    /// The button of the button bar at `x`, `y` in `ButtonImage`, if any;
    /// the caller leaves out a [`button_blocked`] one. // 1110:12c0
    #[must_use]
    pub fn button_at(x: i32, y: i32) -> Option<u8> {
        if !(0..BUTTON_SIZE).contains(&y) {
            return None;
        }
        let n = BUTTONS
            .iter()
            .position(|&left| (left..left + BUTTON_SIZE).contains(&x))?;
        u8::try_from(n).ok()
    }

    /// Draws the button bar: each button down or up, from `button.bmp`
    /// or `buttoff.bmp`. // 1010:2af5, 1050:0002, 1050:0074
    pub fn draw_buttons(&mut self, down: &[bool]) {
        let all = self.buttons.bounds();
        for (n, x) in BUTTONS.into_iter().enumerate() {
            let from = if down.get(n).copied().unwrap_or(false) {
                BUTTON
            } else {
                BUTTOFF
            };
            let r = Rect::new(x, 0, BUTTON_SIZE, BUTTON_SIZE);
            self.buttons.blit(x, 0, &self.bitmaps[from], r, Rop::Copy, all);
        }
    }

    /// Shows `text` on the status bar, centred, in colour 11, for 20
    /// ticks. // 1030:22de (1030:232d, 1030:235a)
    pub fn show_status(&mut self, text: &[u8], font: &Font) {
        self.status_timeout = 20;
        self.draw_status(text, font);
    }

    /// The ticks the status bar's message still stays. // 1110:009a
    #[must_use]
    pub fn status_timeout(&self) -> i16 {
        self.status_timeout
    }

    fn draw_status(&mut self, text: &[u8], font: &Font) {
        // 1030:21a8: the bar's picture back over its first 220 pixels.
        let all = self.status.bounds();
        let from = Rect::new(0, 0, 0xdc, all.height());
        self.status.blit(0, 0, &self.bitmaps[STATUS], from, Rop::Copy, all);
        font.draw(&mut self.status, text, (2, 0), 200, Align::Center, 0xb); // 1030:23d3
    }

    /// The clock's step 2: the status bar's message goes after its time.
    /// Whether the bar changed. // 1020:1e9b
    pub fn status_tick(&mut self, font: &Font) -> bool {
        if self.status_timeout > 0 {
            self.status_timeout -= 1;
            if self.status_timeout == 0 {
                self.draw_status(b"", font);
                return true;
            }
        }
        false
    }

    /// A right click on `Hauptbild` at `x`, `y`: the view moves towards it,
    /// by the distance from the middle in half rows of 18 and columns of
    /// 32 pixels. // 1020:1984
    pub fn right_click(&mut self, x: i32, y: i32, world: &World, tiles: &Tiles) {
        let (w, h) = (self.rects[3].1.width(), self.rects[3].1.height());
        let short = |n: i32| i16::try_from(n).unwrap_or(0);
        self.scroll(short((y - h / 2) / 0x12), short((x - w / 2) / 0x20), world, tiles);
    }

    /// Redraws the island, without the people. // 1050:2700
    pub fn paint_map(&mut self, world: &World, tiles: &Tiles) {
        let cam = self.drawn_camera();
        darstellung::draw_view_in(&mut self.island, world, tiles, &cam, &self.view_size);
        copy(&mut self.map, &self.island);
    }

    /// The island with the people and vehicles on it, as the clock's tick
    /// draws them after it moved them (1020:1e9b): only with the
    /// animations on and new people coming (`1110:0054`, `1110:0056`); the
    /// money rising from a square is its amount and the currency in
    /// colour 1. // 1088:385d, 1088:38c4
    pub fn show_people(&mut self, world: &World, tiles: &Tiles, font: &Font, texts: &InfoTexts) {
        copy(&mut self.map, &self.island);
        if !(world.options.animations && world.options.people_spawn) {
            return;
        }
        let mut money = |dst: &mut Surface, amount: i32, at: (i32, i32)| {
            // 1088:3a19: format_money, ' ' (1088:3a17), the currency.
            let mut text = format_money(amount, texts.currency_factor, texts.language);
            text.push(b' ');
            text.extend_from_slice(&texts.currency);
            text.truncate(255);
            font.draw(dst, &text, at, 100, Align::Left, 1); // 1030:23d3
        };
        let cam = self.drawn_camera();
        darstellung::draw_people_in(&mut self.map, world, tiles, &cam, &self.view_size, &mut money);
    }

    /// Redraws the squares the simulation changed, and says whether there
    /// were any. The original redraws the columns each square covers, if
    /// they're in view (1050:21f9); this redraws the whole island once.
    pub fn redraw_squares(&mut self, squares: &[Pos], world: &World, tiles: &Tiles) -> bool {
        if squares.is_empty() {
            return false;
        }
        self.paint_map(world, tiles);
        true
    }

    /// Moves the view by `ns` half rows and `wo` columns on the screen,
    /// which the view's rotation turns into the map's directions, as far as
    /// the scroll bars go. // 1020:07dc
    ///
    /// The original scrolls `Hauptbild`'s picture and draws only the strip
    /// that came into view (1050:0144), or everything for diagonal or long
    /// moves; this redraws everything.
    pub fn scroll(&mut self, ns: i16, wo: i16, world: &World, tiles: &Tiles) {
        let (nsb, wob) = (self.ns_bar, self.wo_bar);
        match self.camera.view & 3 {
            0 => {
                self.wo = wob.clamp(self.wo + wo);
                self.ns = nsb.clamp(self.ns + ns);
            }
            1 => {
                self.wo = nsb.clamp(self.wo - ns);
                self.ns = wob.clamp(self.ns + wo);
            }
            2 => {
                self.wo = wob.clamp(self.wo - wo);
                self.ns = nsb.clamp(self.ns - ns);
            }
            _ => {
                self.wo = nsb.clamp(self.wo + ns);
                self.ns = wob.clamp(self.ns - wo);
            }
        }
        self.keep_origin = false;
        self.update_bars();
        self.paint_map(world, tiles);
    }

    /// Turns the view to `view`, keeping the origin. // 1010:254d
    pub fn set_view(&mut self, view: u8, world: &World, tiles: &Tiles) {
        self.camera.view = view & 3;
        self.keep_origin = true;
        self.set_origin(self.camera.origin);
        self.paint_map(world, tiles);
    }

    /// `FormKeyDown`: so far the keys that move and turn the view, and
    /// those of the button bar's buttons. // 1020:2e09
    pub fn key_down(&mut self, key: u16, shift: ShiftState, world: &World, tiles: &Tiles) -> KeyResult {
        let mods = shift.0 & (ShiftState::SHIFT.0 | ShiftState::ALT.0 | ShiftState::CTRL.0);
        if (mods == 0 || mods == ShiftState::SHIFT.0)
            && let Some(&(_, n)) = BUTTON_KEYS.iter().find(|(keys, _)| keys.contains(&key))
        {
            // en 1020:2e09: F2 does nothing in scenario 10.
            if button_blocked(world.scenario, n) {
                return KeyResult::Ignored;
            }
            return KeyResult::Button(n);
        }
        let step = if mods == ShiftState::CTRL.0 {
            6
        } else if mods == 0 || mods == ShiftState::SHIFT.0 {
            2
        } else {
            return KeyResult::Ignored;
        };
        if let Some(&(_, _, ns, wo)) = SCROLL_KEYS.iter().find(|k| k.0 == key || k.1 == key) {
            self.scroll(ns * step, wo * step, world, tiles);
            return KeyResult::Redrawn;
        }
        if step != 2 {
            return KeyResult::Ignored;
        }
        let view = self.camera.view & 3;
        if VK_INSERT.contains(&key) {
            self.set_view((view + 1) % 4, world, tiles);
        } else if VK_DELETE.contains(&key) {
            self.set_view((view + 3) % 4, world, tiles);
        } else {
            return KeyResult::Ignored;
        }
        KeyResult::Redrawn
    }

    /// The overview map's click (10b8:37f5, `crate::uebersichtsmap`): the
    /// scroll position set as it is, past the scroll bars' ends too, the
    /// origin and bars from it (1020:05a7) and the island drawn.
    ///
    /// With `overview_scrolling` the position stays within the scroll
    /// bars' ends, as [`Hauptschirm::scroll`] keeps it.
    pub fn set_scroll(&mut self, wo: i16, ns: i16, world: &World, tiles: &Tiles) {
        // BUG(original): the overview map's click can put the position past the bars' ends (10b8:37f5).
        let (wo, ns) = if self.enhance.has(Enhancement::OverviewScrolling) {
            let (nsb, wob) = (self.ns_bar, self.wo_bar);
            match self.camera.view & 3 {
                0 | 2 => (wob.clamp(wo), nsb.clamp(ns)),
                _ => (nsb.clamp(wo), wob.clamp(ns)),
            }
        } else {
            (wo, ns)
        };
        self.wo = wo;
        self.ns = ns;
        self.update_bars();
        self.paint_map(world, tiles);
    }

    /// The scroll position, `wo` and `ns`, as a map file saves it.
    /// // +0x408, +0x40a
    #[must_use]
    pub fn scroll_position(&self) -> (i16, i16) {
        (self.wo, self.ns)
    }

    /// The camera of the original's view, which a map file saves; the
    /// island is drawn with [`Hauptschirm::drawn_camera`], which is the
    /// same without `big_map`.
    #[must_use]
    pub fn camera(&self) -> Camera {
        self.camera
    }

    /// The camera `Hauptbild` is drawn and picked with: the original's,
    /// or with `big_map`'s bigger view the one whose view has the
    /// original's in its middle. That's the scroll position moved up
    /// and left by [`ViewSize::core_offset`] on the screen, the way
    /// [`Hauptschirm::scroll`] turns a move on the screen into the map's
    /// directions, without the bars' ends; the offset is even, so its
    /// origin and parity come out as the original's would there.
    #[must_use]
    pub fn drawn_camera(&self) -> Camera {
        if !self.view_size.is_big() {
            return self.camera;
        }
        let (dc, dr) = self.view_size.core_offset();
        let (wo, ns) = (self.wo, self.ns);
        let (wo, ns) = match self.camera.view & 3 {
            0 => (wo - dc, ns - dr),
            1 => (wo + dr, ns - dc),
            2 => (wo + dc, ns + dr),
            _ => (wo - dr, ns + dc),
        };
        let (origin, parity) = scroll_origin(wo, ns, self.keep_origin);
        Camera {
            origin,
            parity,
            ..self.camera
        }
    }

    /// The map view `Hauptbild` shows: the original's, or `big_map`'s.
    #[must_use]
    pub fn view_size(&self) -> ViewSize {
        self.view_size
    }

    /// The images' bounds in the form now: [`LAYOUT`], or with `big_map`
    /// [`layout_for`] its size.
    #[must_use]
    pub fn rects(&self) -> &[(&'static str, Rect); 10] {
        &self.rects
    }

    /// `Hauptbild`'s bounds in the form.
    #[must_use]
    pub fn hauptbild(&self) -> Rect {
        self.rects[3].1
    }

    /// The form's full size (not rolled up).
    #[must_use]
    pub fn size(&self) -> (i32, i32) {
        (self.width, self.full_height)
    }

    /// Where `big_map`'s bigger view lies on the overview map's picture
    /// (`crate::uebersichtsmap`, a square `(i, j)` at `((i - j + 250) · 2,
    /// i + j)`): the box its corners' squares span, with the original's
    /// frame one pixel to the left (10b8:1349). None for the original's
    /// view, whose frame is the overview's own picture.
    #[must_use]
    pub fn overview_frame(&self, tiles: &Tiles) -> Option<Rect> {
        if !self.view_size.is_big() {
            return None;
        }
        let cam = self.drawn_camera();
        let size = self.view_size;
        // The columns and half rows the drawing area shows.
        let cols = i16::try_from(size.width / darstellung::COLUMN_WIDTH).unwrap_or(0);
        let rows = i16::try_from(size.height / darstellung::HALF_ROW).unwrap_or(0);
        let at = |(i, j): Pos| ((i32::from(i) - i32::from(j) + 250) * 2, i32::from(i) + i32::from(j));
        let corners =
            [(0, 0), (cols, 0), (0, rows), (cols, rows)].map(|(c, r)| at(darstellung::view_square(tiles, &cam, c, r)));
        let left = corners.iter().map(|c| c.0).min().unwrap_or(0) - 1;
        let right = corners.iter().map(|c| c.0).max().unwrap_or(0) - 1;
        let top = corners.iter().map(|c| c.1).min().unwrap_or(0);
        let bottom = corners.iter().map(|c| c.1).max().unwrap_or(0);
        Some(Rect::new(left, top, right - left, bottom - top))
    }

    /// `big_map`'s scroll bars: the original's ranges less the bigger
    /// view's margins at both ends, so that the bigger view stays on the
    /// map as the original's does at the original's ends. Without it, 0 to
    /// [`NS_MAX`] and [`WO_MAX`] as `hauptschirm_init` sets them
    /// (1020:2501).
    fn set_bar_ranges(&mut self) {
        let (dc, dr) = self.view_size.core_offset();
        let (dc, dr) = (dc.min(WO_MAX / 2), dr.min(NS_MAX / 2));
        self.ns_bar.min = dr;
        self.ns_bar.max = NS_MAX - dr;
        self.wo_bar.min = dc;
        self.wo_bar.max = WO_MAX - dc;
    }

    /// The form `width` × `height` (`big_map`; the original's is always
    /// [`WIDTH`] × [`HEIGHT`]): the images at [`layout_for`]'s bounds, the
    /// bars stretched, the frame and the bars drawn again, and a map view
    /// that covers `Hauptbild` ([`ViewSize::for_image`]). The scroll
    /// position stays, past the new bars' ends too until the next scroll.
    /// Whether the size changed; then the caller draws the island and the
    /// people again.
    fn set_size(&mut self, width: i32, height: i32, titled: bool) -> bool {
        if (width, height, titled) == (self.width, self.full_height, self.titled) {
            return false;
        }
        let original = (width, height, titled) == (WIDTH, HEIGHT, true);
        self.width = width;
        self.full_height = height;
        self.titled = titled;
        self.bar_press = None;
        self.rects = if original {
            LAYOUT
        } else {
            layout_for(width, height, titled)
        };
        let rects = self.rects;
        let image = |name| image(&rects, name);
        let bild = self.hauptbild();
        self.view_size = if original {
            ViewSize::ORIGINAL
        } else {
            ViewSize::for_image(bild.width(), bild.height())
        };
        self.title = tiled(&self.bitmaps[MENULEI], rects[0].1.width(), rects[0].1.height());
        if titled {
            self.font.draw(
                &mut self.title,
                &self.title_text,
                (0, 1),
                rects[0].1.width(),
                Align::Center,
                1,
            );
        }
        self.close = image("CloseBild");
        self.minimize = image("Kleinbild");
        self.info_bar = tiled(&self.bitmaps[INFO], width, rects[1].1.height());
        self.ns_trough = trough(
            &self.bitmaps[SCRONOSU],
            rects[4].1.height(),
            LAYOUT[4].1.height(),
            false,
        );
        self.wo_trough = trough(&self.bitmaps[SCROWEOS], rects[6].1.width(), LAYOUT[6].1.width(), true);
        self.info = image("InfoImage");
        self.buttons = image("ButtonImage");
        self.map = image("Hauptbild");
        self.island = image("Hauptbild");
        self.ns_image = image("NSScrollbild");
        self.wo_image = image("WOScrollbild");
        self.status = image("Statusimage");
        self.paint_frame();
        self.info_shown = None;
        self.status_timeout = 0;
        self.set_bar_ranges();
        self.update_bars();
        true
    }

    /// Lays the form out for the `screen` the game lays out for on a
    /// `desktop` (the window): with `big_map` and a desktop bigger than
    /// [`BIG_FROM`] it fills the desktop at 0, 0 (with `no_main_title`
    /// without its title bar), else it's the original's
    /// ([`Hauptschirm::layout`]). Whether its size changed, which the
    /// caller follows by drawing the island, the people and the buttons
    /// again.
    pub fn place(&mut self, screen: (i32, i32), desktop: (i32, i32)) -> bool {
        if self.big_map && (desktop.0 > BIG_FROM.0 || desktop.1 > BIG_FROM.1) {
            // The window's own title bar names the game; a second one
            // would only take room from the island.
            let titled = !self.enhance.has(Enhancement::NoMainTitle);
            let changed = self.set_size(desktop.0, desktop.1, titled);
            (self.left, self.top) = (0, 0);
            self.height = self.full_height;
            self.rolled_up = false;
            changed
        } else {
            // BUG(original): the main screen and its map view are 640 × 479 and 20 × 27 on every screen; a bigger one shows more background, not more island.
            let changed = self.set_size(WIDTH, HEIGHT, true);
            self.layout(screen);
            changed
        }
    }

    /// The form's top left on a `width` × `height` screen: centred, with
    /// Delphi's `div` on each half. // 1020:1ce9
    #[must_use]
    pub fn position(width: i32, height: i32) -> (i32, i32) {
        (width / 2 - WIDTH / 2, height / 2 - HEIGHT / 2)
    }

    /// `hauptschirm_layout`: the form centred on the `screen` the game
    /// lays out for, and its images at [`LAYOUT`]'s bounds (the caller's,
    /// from [`Hauptschirm::images`]); `hauptschirm_show` (1020:283b) gives
    /// it its full height. The port also runs it again when the window's
    /// size changes. // 1020:1ce9
    pub fn layout(&mut self, screen: (i32, i32)) {
        (self.left, self.top) = Hauptschirm::position(screen.0, screen.1);
        self.height = HEIGHT;
        self.rolled_up = false;
    }

    /// The form's bounds on the screen.
    #[must_use]
    pub fn bounds(&self) -> Rect {
        Rect::new(self.left, self.top, self.width, self.height)
    }

    /// Whether the form has its title bar: all but `no_main_title`'s
    /// filling the screen.
    #[must_use]
    pub fn titled(&self) -> bool {
        self.titled
    }

    /// The form fills the screen (`big_map`), so its title bar doesn't
    /// drag it: a press leaves it where it is, a double click rolls it up
    /// to its title bar and the next press unrolls it.
    fn filled(&self) -> bool {
        self.width != WIDTH || self.full_height != HEIGHT
    }

    /// `UeberschriftbildMouseDown`: the press is kept from the form's top
    /// left, and the form shrinks to its title bar; a double click (the
    /// press with `ssDouble`) leaves it like that when let go.
    /// // 1020:1b8e
    pub fn title_mouse_down(&mut self, shift: ShiftState, cursor: (i32, i32)) {
        self.title_held = true;
        self.rolled_up = shift.contains(ShiftState::DOUBLE);
        if self.filled() {
            self.height = if self.rolled_up { TITLE_HEIGHT } else { self.full_height };
            return;
        }
        self.grab = (cursor.0 - self.left, cursor.1 - self.top);
        self.height = TITLE_HEIGHT;
    }

    /// `UeberschriftbildMouseMove` with the `cursor` on the screen: while
    /// the left button is down the form follows it, kept on the `screen`
    /// by its width and its height then. Whether it moved. With
    /// `form_drags` only after a press on the title bar. // 1020:1bd9
    pub fn title_mouse_move(&mut self, shift: ShiftState, cursor: (i32, i32), screen: (i32, i32)) -> bool {
        // BUG(original): nothing says the press was on the title bar, so a left drag that started off every form and comes over it moves the form by the last grab.
        let held = self.title_held || !self.enhance.has(Enhancement::FormDrags);
        if !shift.contains(ShiftState::LEFT) || self.filled() || !held {
            return false;
        }
        self.left = (cursor.0 - self.grab.0).max(0).min(screen.0 - WIDTH);
        self.top = (cursor.1 - self.grab.1).max(0).min(screen.1 - self.height);
        true
    }

    /// `UeberschriftbildMouseUp`: the form gets its height back, unless
    /// the press was a double click. // 1020:1c58
    pub fn title_mouse_up(&mut self) {
        self.title_held = false;
        if !self.rolled_up {
            self.height = self.full_height;
        }
    }

    /// What the world's people code reads of this view
    /// ([`World::set_viewpoint`]): the rotation, origin and parity.
    #[must_use]
    pub fn viewpoint(&self) -> Viewpoint {
        Viewpoint {
            rotation: self.camera.view,
            origin: self.camera.origin,
            parity: self.camera.parity,
        }
    }

    /// Each image with the name of the `TImage` it goes into.
    #[must_use]
    pub fn images(&self) -> [(&'static str, &Surface); 10] {
        [
            ("Ueberschriftbild", &self.title),
            ("InfoImage", &self.info),
            ("ButtonImage", &self.buttons),
            ("Hauptbild", &self.map),
            ("NSScrollbild", &self.ns_image),
            ("Statusimage", &self.status),
            ("WOScrollbild", &self.wo_image),
            ("eckimage", &self.corner),
            ("CloseBild", &self.close),
            ("Kleinbild", &self.minimize),
        ]
    }
}

/// The view's drawing area is a little bigger than `Hauptbild`.
const _: () = assert!(VIEW_WIDTH == 624 && VIEW_HEIGHT == 405);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thumb() {
        let bar = Bar {
            min: 0,
            max: 476,
            position: 0,
        };
        assert_eq!(bar.thumb(404), 16);
        let end = Bar { position: 476, ..bar };
        assert_eq!(end.thumb(404), 404 - 19 - 16);
    }

    #[test]
    fn layouts_without_the_title_bar() {
        for (w, h) in [(800, 600), (1024, 768), (1280, 1024), (1366, 700), (1920, 1080)] {
            let with = layout_for(w, h, true);
            let without = layout_for(w, h, false);
            let rect = |l: &[(&str, Rect); 10], n: &str| l.iter().find(|(m, _)| *m == n).unwrap().1;
            // The title bar and its widgets are gone.
            for n in ["Ueberschriftbild", "CloseBild", "Kleinbild"] {
                let r = rect(&without, n);
                assert_eq!((r.width(), r.height(), r.top), (0, 0, 0), "{n}");
            }
            // The info bar is at the top, the button bar under it, and the
            // island and the right scroll bar start 16 higher, as long as
            // the bottom bar stays.
            assert_eq!(rect(&without, "InfoImage"), Rect::new(0, 0, w, 18));
            assert_eq!(rect(&without, "ButtonImage"), Rect::new(0, 18, w, 25));
            let bild = rect(&without, "Hauptbild");
            assert_eq!(bild, Rect::new(0, 43, w - 16, h - 59));
            assert_eq!(bild.bottom, rect(&with, "Hauptbild").bottom);
            assert_eq!(rect(&without, "NSScrollbild"), Rect::new(w - 16, 43, 16, h - 59));
            for n in ["Statusimage", "WOScrollbild", "eckimage"] {
                assert_eq!(rect(&without, n), rect(&with, n), "{n}");
                assert_eq!(rect(&without, n).bottom, h);
            }
            // The island's view grows by the title bar's height or more.
            let (a, b) = (ViewSize::for_image(bild.width(), bild.height()), {
                let r = rect(&with, "Hauptbild");
                ViewSize::for_image(r.width(), r.height())
            });
            assert!(a.half_rows >= b.half_rows && a.height == b.height + TITLE_HEIGHT);
        }
    }

    #[test]
    fn scroll_bar_parts() {
        // The right bar at the start, 404 long: the thumb at 16..36.
        let bar = Bar {
            min: 0,
            max: NS_MAX,
            position: 0,
        };
        let at = |along, whole| part_at(bar, 404, along, whole);
        assert_eq!(at(0, false), Part::BackArrow);
        assert_eq!(at(16, false), Part::BackArrow, "`< 0x11`");
        assert_eq!(at(17, false), Part::Thumb);
        assert_eq!(at(31, false), Part::Thumb);
        // BUG(original): the thumb's last four pixels page.
        assert_eq!(at(32, false), Part::PageForward);
        assert_eq!(at(32, true), Part::Thumb);
        assert_eq!(at(35, true), Part::Thumb);
        assert_eq!(at(36, true), Part::PageForward);
        assert_eq!(at(387, false), Part::PageForward);
        assert_eq!(at(388, false), Part::ForwardArrow);
        // In the middle the trough is on both sides.
        let mid = Bar { position: 238, ..bar };
        let thumb = mid.thumb(404);
        assert_eq!(part_at(mid, 404, thumb - 1, false), Part::PageBack);
        assert_eq!(part_at(mid, 404, thumb, false), Part::Thumb);
        // The steps: arrows one, pages 16 down and 18 across.
        assert_eq!(Hauptschirm::bar_step(Axis::Ns, Part::PageForward), 16);
        assert_eq!(Hauptschirm::bar_step(Axis::Wo, Part::PageBack), -18);
        assert_eq!(Hauptschirm::bar_step(Axis::Wo, Part::BackArrow), -1);
    }

    #[test]
    fn big_layouts() {
        assert_eq!(layout_for(WIDTH, HEIGHT, true), LAYOUT);
        assert_eq!(ViewSize::for_image(624, 404), ViewSize::ORIGINAL);
        let r = layout_for(1024, 768, true);
        assert_eq!(r[3].1, Rect::new(0, 59, 1008, 693));
        assert_eq!(r[7].1, Rect::new(1008, 752, 16, 16));
        // A trough no longer than the original's is the original's start.
        let src = Surface {
            width: 6,
            height: 1,
            pixels: vec![1, 2, 3, 4, 5, 6],
        };
        assert_eq!(trough(&src, 5, 5, true).pixels, [1, 2, 3, 4, 5]);
        assert_eq!(tiled(&src, 8, 1).pixels, [1, 2, 3, 4, 5, 6, 1, 2]);
    }
}
