//! `THauptschirm`, the main screen (segment `1020`): a 640×479 window in the
//! middle of the screen with a title bar, an info bar, the button bar, the
//! island (`Hauptbild`) with a scroll bar on the right and one below, and
//! the status bar.
//!
//! Like the start menu, the form is a set of `TImage`s the game draws into
//! itself. This module keeps one [`Surface`] per image; the caller puts
//! them into the VCL form at the bounds [`LAYOUT`] gives.
//!
//! The scroll position is two numbers, `wo` (west–east, the lower scroll
//! bar) and `ns` (north–south, the right one), in the map's own diagonal
//! coordinates. The camera's origin on the map is worked out from them
//! (1020:0697).

use formats::data::Land;
use vcl::Rect;
use vcl::surface::{Rop, Surface};

use crate::darstellung::{self, Camera, Tiles, VIEW_HEIGHT, VIEW_WIDTH};
use crate::monate::Date;
use crate::money::format_money;
use crate::people::Viewpoint;
use crate::text::{Align, Font};
use crate::widgets::ShiftState;
use crate::world::{Pos, World};

/// The form's size. // 1110:1578, 1110:157a
pub const WIDTH: i32 = 640;
pub const HEIGHT: i32 = 479;

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

/// The bitmaps the frame is drawn from, from `GRAFIK`, in the order
/// [`Hauptschirm::new`] takes them.
pub const BITMAP_FILES: [&str; 10] = [
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
const VK_DELETE: [u16; 2] = [0x2e, 0x6e];

/// What a key did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyResult {
    /// The view moved or turned and was redrawn.
    Redrawn,
    /// Not a key this port handles yet.
    Ignored,
}

/// What the info bar's texts need besides the world.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InfoTexts {
    /// `MONATE.TXT`. // [1110:65d6]
    pub months: Vec<Vec<u8>>,
    /// The country from `MODATYP.DAT`, for the date's and the money's
    /// format. // 1110:64be
    pub land: Land,
    /// `Waehrungsfaktor` from `SONSTIGE.TXT`. // 1110:7346
    pub currency_factor: i32,
    /// The currency's name, `SONSTIGE.TXT`'s sixth text. // *1110:7352
    pub currency: Vec<u8>,
}

/// The main screen's state and images.
#[derive(Debug, Clone)]
pub struct Hauptschirm {
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
}

fn image(name: &str) -> Surface {
    let r = LAYOUT
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
        assert_eq!(bitmaps.len(), BITMAP_FILES.len(), "one bitmap per file");
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
        };
        // 1020:1e4f: the game's name over the title bar.
        let width = LAYOUT[0].1.width();
        font.draw(&mut h.title, title, (0, 1), width, Align::Center, 1);
        h.paint_frame();
        h.set_origin(origin);
        h
    }

    /// The images that don't show the game's state.
    fn paint_frame(&mut self) {
        let b = &self.bitmaps;
        copy(&mut self.info, &b[INFO]); // 1030:213f
        copy(&mut self.status, &b[STATUS]); // 1030:21a8
        // 1010:2af5: every button up, from buttoff.bmp (1050:0074).
        let all = self.buttons.bounds();
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
    }

    /// Puts the info bar's background back from `x` on, `width` wide.
    /// // 1030:213f
    fn restore_info_bar(&mut self, x: i32, width: i32) {
        let all = self.info.bounds();
        let from = Rect::new(x, 0, width, all.height());
        self.info.blit(x, 0, &self.bitmaps[INFO], from, Rop::Copy, all);
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
            let mut text = format_money(money, texts.currency_factor, texts.land);
            text.push(b' '); // 1078:01e5
            text.extend_from_slice(&texts.currency);
            text.truncate(255);
            self.restore_info_bar(0, 152);
            let colour = if text.first() == Some(&b'-') { 11 } else { 10 };
            font.draw(&mut self.info, &text, (4, 1), 120, Align::Right, colour);
        }
        if shown.is_none_or(|(d, _)| d != date) {
            // 1030:0148, 1030:228d
            let text = date.text(&texts.months, texts.land);
            self.restore_info_bar(400, 640);
            font.draw(&mut self.info, &text, (450, 1), 182, Align::Right, 10);
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
        let (wo, ns) = (i32::from(self.wo), i32::from(self.ns));
        // `div 2`, truncating, of sums that may round differently.
        let (i, j) = if self.keep_origin {
            (wo + ns + 1002, ns - wo + 1001)
        } else {
            (wo + ns + 1001, ns + 1000 - wo)
        };
        let (i, j) = (i / 2 - 626, j / 2 - 375);
        let short = |n: i32| i16::try_from(n).unwrap_or(0);
        ((short(i), short(j)), short((wo + 250 + ns) % 2))
    }

    /// The origin to the camera, and the scroll bars to the position, as
    /// the view sees it. // 1020:05a7
    fn update_bars(&mut self) {
        let ((i, j), parity) = self.origin();
        self.camera.origin = (i, j);
        self.camera.parity = parity;
        let (ns, wo) = match self.camera.view & 3 {
            0 => (self.ns, self.wo),
            1 => (self.ns_bar.max - self.wo, self.ns),
            2 => (self.ns_bar.max - self.ns, self.wo_bar.max - self.wo),
            _ => (self.wo, self.wo_bar.max - self.ns),
        };
        self.ns_bar.position = ns;
        self.wo_bar.position = wo;
        // 1020:0c8b, 1020:0bf4: the trough, then the thumb.
        let scroll = &self.bitmaps[SCROLL1];
        let img = &mut self.ns_image;
        copy(img, &self.bitmaps[SCRONOSU]);
        let y = self.ns_bar.thumb(i32::try_from(img.height).unwrap_or(0));
        let all = img.bounds();
        img.blit(0, y, scroll, Rect::new(0, 16, 16, 20), Rop::And, all);
        img.blit(0, y, scroll, Rect::new(16, 16, 16, 20), Rop::Invert, all);
        // 1020:1413, 1020:137c
        let img = &mut self.wo_image;
        copy(img, &self.bitmaps[SCROWEOS]);
        let x = self.wo_bar.thumb(i32::try_from(img.width).unwrap_or(0));
        let all = img.bounds();
        img.blit(x, 0, scroll, Rect::new(0, 0, 20, 16), Rop::And, all);
        img.blit(x, 0, scroll, Rect::new(20, 0, 20, 16), Rop::Invert, all);
    }

    /// Redraws the island, without the people. // 1050:2700
    pub fn paint_map(&mut self, world: &World, tiles: &Tiles) {
        darstellung::draw_view(&mut self.island, world, tiles, &self.camera);
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
            let mut text = format_money(amount, texts.currency_factor, texts.land);
            text.push(b' ');
            text.extend_from_slice(&texts.currency);
            text.truncate(255);
            font.draw(dst, &text, at, 100, Align::Left, 1); // 1030:23d3
        };
        darstellung::draw_people(&mut self.map, world, tiles, &self.camera, &mut money);
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

    /// `FormKeyDown`: so far the keys that move and turn the view.
    /// // 1020:2e09
    pub fn key_down(&mut self, key: u16, shift: ShiftState, world: &World, tiles: &Tiles) -> KeyResult {
        let mods = shift.0 & (ShiftState::SHIFT.0 | ShiftState::ALT.0 | ShiftState::CTRL.0);
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

    /// The form's top left on a `width` × `height` screen. // 1020:1ce9
    #[must_use]
    pub fn position(width: i32, height: i32) -> (i32, i32) {
        (width / 2 - WIDTH / 2, height / 2 - HEIGHT / 2)
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
}
