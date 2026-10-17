//! `TUmenuform`, the buildings' overview ("Gebäudeübersicht", segment
//! `1080`), which the info box's "Übersicht" opens (1038:24d6): the
//! human's buildings of one kind, with how full they are, their
//! condition and profit, and for the selected one its rent or price, to
//! renovate it, change the price or jump to it. `docs/infobox.md` has the
//! whole of it.
//!
//! Like the other forms it's one `TImage`, `bild`, drawn by the game
//! with the toolkit's widgets: a scroll bar, "Renovieren", "Springe hin",
//! the minus and plus buttons and the close button. The world changes are
//! [`Action`]s ([`crate::infobox`] has the world's side).

// The original's names: `x`, `y`, sizes `w`, `h`.
#![allow(clippy::many_single_char_names)]

use formats::data::Strings;
use vcl::Rect;
use vcl::surface::{Rop, Surface};

use crate::command::Action;
use crate::construction::CLASSES;
use crate::enhance::{Enhancement, Enhancements};
use crate::hauptschirm::InfoTexts;
use crate::infobox::decimal_money;
use crate::money::format_money;
use crate::text::{Align, Font};
use crate::widgets::{BitmapId, Button, Frame, Outcome, ScrollBar, ScrollBitmaps, ShiftState, Skin, Widget};
use crate::world::{Pos, World};

/// The form's size, and `bild`'s. // 1080:145f
pub const WIDTH: i32 = 0x140;
pub const HEIGHT: i32 = 0x140;
/// Its height while it's dragged by its title bar, and after a double
/// click there. // 1080:1c3b
const TITLE_HEIGHT: i32 = 0x10;

/// The bitmaps the form draws with, from `GRAFIK`, in the order
/// [`Umenuform::new`] takes them.
pub const BITMAP_FILES: [&str; 20] = [
    "menu.bmp",     // the background, +0x736, 1080:139b
    "menuscu.bmp",  // the scroll bar's arrow up, +0x180
    "menuscup.bmp", // held down, +0x184
    "menuscd.bmp",  // the arrow down, +0x188
    "menuscdp.bmp", // held down, +0x18c
    "ibblnk.bmp",   // the minus button, +0x190
    "ibblnkpr.bmp", // +0x194
    "ibbrts.bmp",   // the plus button, +0x198
    "ibbrtspr.bmp", // +0x19c
    "menuscli.bmp", // the scroll bar's thumb, +0x1a0
    "menuscrl.bmp", // its trough, +0x1a4
    "menubu.bmp",   // a button, +0x1a8
    "menubupr.bmp", // held down, +0x1ac
    "menuexit.bmp", // loaded but not used, +0x1b0
    "menuexpr.bmp", // +0x1b4
    "menuaus.bmp",  // the bars, ten of 32 × 16 from full to empty, +0x1b8
    // `hintergrund`'s close button. // 1110:54b8, 1110:54bc
    "close.bmp",
    "closepre.bmp",
    // The main screen's button bar, up and down, for the kinds' pictures.
    // // 1110:600a, 1110:6006
    "buttoff.bmp",
    "button.bmp",
];
const BACKGROUND: usize = 0;
const SCROLL: ScrollBitmaps = ScrollBitmaps {
    back: BitmapId(1),
    back_pressed: BitmapId(2),
    forward: BitmapId(3),
    forward_pressed: BitmapId(4),
    thumb: BitmapId(9),
    track: BitmapId(10),
};
const MINUS: BitmapId = BitmapId(5);
const MINUS_PRESSED: BitmapId = BitmapId(6);
const PLUS: BitmapId = BitmapId(7);
const PLUS_PRESSED: BitmapId = BitmapId(8);
const BUTTON: BitmapId = BitmapId(11);
const BUTTON_PRESSED: BitmapId = BitmapId(12);
const BARS: usize = 15;
const CLOSE: BitmapId = BitmapId(16);
const CLOSE_PRESSED: BitmapId = BitmapId(17);
const BUTTONS_UP: usize = 18;
const BUTTONS_DOWN: usize = 19;

/// `UMENU.TXT`'s texts: the title, the columns' headings (the name, how
/// full, the condition, the profit and its unit), the rent and its unit,
/// "Menükosten" (not used), and the two buttons' captions.
const TITLE: usize = 0;
const NAME: usize = 1;
const FULL: usize = 2;
const CONDITION: usize = 3;
const PROFIT: usize = 4;
const RENT: usize = 6;
const RENT_UNIT: usize = 7;
const RENOVATE_TEXT: usize = 9;
const JUMP_TEXT: usize = 10;

/// The kinds of building it lists (`1110:6fb8`, the `HAUSDATA.DAT` kind),
/// 0 to 11 but 7 (roads), each with the build menu's button picture of
/// its submenu (9 to 20 but the road's 16), 25 × 25 from the button bar's
/// pictures at 264. // 1080:0bfb
const KINDS: i32 = 12;
const ROADS: i32 = 7;
const KIND_PICTURES: (i32, i32) = (8, 0x11e);
const KIND_SIZE: i32 = 0x19;
const KIND_SOURCE: i32 = 0x108;

/// The list: 12 rows of 16 pixels from row 0x30, at most 0x1f5 buildings
/// (the list's 0x7d4 bytes). // 1080:02d9
const ROWS: i32 = 12;
const ROW_HEIGHT: i32 = 0x10;
const LIST_TOP: i32 = 0x30;
const LIST_CAPACITY: usize = 0x1f5;
/// The colours of a row and of the selected one. // 1110:337c
const COLOURS: [i32; 2] = [9, 7];
/// The text colour of the headings and buttons.
const COLOUR: i32 = 9;

/// The keys `FormKeyDown` knows. // 1080:202b
const VK_RETURN: u16 = 0x0d;
const VK_ESCAPE: u16 = 0x1b;
const VK_SPACE: u16 = 0x20;
const VK_LEFT: u16 = 0x25;
const VK_UP: u16 = 0x26;
const VK_RIGHT: u16 = 0x27;
const VK_DOWN: u16 = 0x28;
const VK_R: u16 = 0x52;
const VK_ADD: u16 = 0x6b;
const VK_SUBTRACT: u16 = 0x6d;
const VK_F4: u16 = 0x73;
const VK_PLUS: u16 = 0xbb;
const VK_MINUS: u16 = 0xbd;

/// What the form draws again after its caller applied its command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Redraw {
    /// The list (1080:02d9), after a button.
    List,
    /// The kind, from the list's top (1080:0bfb), after the keys `+`
    /// and `-`.
    Kind,
}

/// What the form tells its caller.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Response {
    /// A command for the human (player 1); after applying it the caller
    /// calls [`Umenuform::redraw`] with `redraw`.
    pub action: Option<Action>,
    pub redraw: Option<Redraw>,
    /// "Springe hin": the main screen's view jumps to this building's square
    /// ([`crate::hauptschirm::Hauptschirm::jump_to`]). // 1020:0787
    pub jump: Option<Pos>,
    /// The form closed.
    pub closed: bool,
    /// The picture or the bounds changed.
    pub refresh: bool,
    /// Sounds to play, in order (`play_sound`).
    pub sounds: Vec<u8>,
}

/// The widgets that take the mouse, in the order the form asks them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Which {
    Scroll,
    Renovate,
    Jump,
    Minus,
    Plus,
    Close,
}

const WIDGETS: [Which; 6] = [
    Which::Scroll,
    Which::Renovate,
    Which::Jump,
    Which::Minus,
    Which::Plus,
    Which::Close,
];

/// The overview's state and picture.
#[derive(Debug, Clone)]
pub struct Umenuform {
    bitmaps: Vec<Surface>,
    /// `UMENU.TXT`'s 11 lines. // *1110:6fba
    texts: Strings,
    /// `INFOBOX.TXT`'s short texts, for the prices' headings. // *1110:65f8
    short_texts: Strings,
    /// `HAUSNAME.TXT`'s short names. // 1110:7366
    short_names: Strings,
    /// `bild`'s picture.
    pub image: Surface,
    /// The form is shown. // +0x29
    pub visible: bool,
    /// The form's top left on the screen, and its height.
    pub left: i32,
    pub top: i32,
    pub height: i32,
    /// The kind of building listed. // 1110:6fb8
    pub kind: i32,
    /// The selected row of the list, from 0; it may be past the list.
    /// // 1110:6fbe
    pub selected: i32,
    /// The human's buildings of the kind, by their first square: the
    /// original keeps their records' pointers. // *1110:6fc0, 1110:6fc4
    pub list: Vec<Pos>,
    /// Its position is the list's top row. // +0x73a (+0x766)
    scroll: ScrollBar,
    renovate: Button, // +0x1bc
    jump: Button,     // +0x2d2
    minus: Button,    // +0x3e8
    plus: Button,     // +0x4fe
    close: Button,    // +0x614
    /// The title bar was pressed: the form follows the mouse. // +0x72a
    dragging: bool,
    /// Where in the form the drag took hold. // +0x72b
    grip: (i32, i32),
    /// The height before the drag. // +0x733
    saved_height: i32,
    /// The title bar was double-clicked: the form stays rolled up.
    /// // +0x735
    rolled_up: bool,
    /// The player's view enhancements (`overview_list`, `infobox_text`,
    /// `scroll_bars`).
    pub enhance: Enhancements,
}

/// `p` percent as one of the ten bars in `menuaus.bmp`: its left edge.
/// // 1080:02d9
fn bar(percent: i32) -> i32 {
    (9 - (percent - 1) / 10) * 0x20
}

impl Umenuform {
    /// The form, hidden: `bitmaps` are those of [`BITMAP_FILES`], `texts`
    /// `UMENU.TXT`'s lines (`formats::data::text_lines`), `short_texts`
    /// `INFOBOX.TXT`'s and `short_names` `HAUSNAME.TXT`'s first list.
    ///
    /// # Panics
    /// If there aren't as many bitmaps as files.
    #[must_use]
    pub fn new(bitmaps: Vec<Surface>, texts: Strings, short_texts: Strings, short_names: Strings) -> Self {
        assert_eq!(bitmaps.len(), BITMAP_FILES.len(), "one bitmap per file");
        let text = |n: usize| texts.get(n).cloned().unwrap_or_default();
        let (renovate, jump) = (text(RENOVATE_TEXT), text(JUMP_TEXT));
        Umenuform {
            bitmaps,
            texts,
            short_texts,
            short_names,
            image: Surface::new(
                WIDTH.unsigned_abs().try_into().unwrap_or(0),
                HEIGHT.unsigned_abs().try_into().unwrap_or(0),
                0,
            ),
            visible: false,
            left: 0,
            top: 0,
            height: HEIGHT,
            kind: 0,
            selected: 0,
            list: Vec::new(),
            // 1080:145f
            scroll: ScrollBar::new(Frame::new(299, 0x31, 0x10, 0xc5), SCROLL),
            renovate: Button::new(
                Frame::new(8, 0x105, 0x50, 0x14),
                BUTTON,
                BUTTON_PRESSED,
                &renovate,
                COLOUR,
            ),
            jump: Button::new(
                Frame::new(0x60, 0x105, 0x50, 0x14),
                BUTTON,
                BUTTON_PRESSED,
                &jump,
                COLOUR,
            ),
            minus: Button::new(Frame::new(0xb6, 0x109, 0x10, 0x10), MINUS, MINUS_PRESSED, b"", COLOUR),
            plus: Button::new(Frame::new(0x10b, 0x109, 0x10, 0x10), PLUS, PLUS_PRESSED, b"", COLOUR),
            close: Button::new(Frame::new(0, 0, 0x10, 0x10), CLOSE, CLOSE_PRESSED, b"", COLOUR),
            dragging: false,
            grip: (0, 0),
            saved_height: HEIGHT,
            rolled_up: false,
            enhance: Enhancements::ORIGINAL,
        }
    }

    fn text(&self, n: usize) -> Vec<u8> {
        self.texts.get(n).cloned().unwrap_or_default()
    }

    /// The list's top row: its scroll bar's position.
    #[must_use]
    pub fn top_row(&self) -> i32 {
        self.scroll.position
    }

    /// The last row of the list, -1 for none. // 1110:6fc4
    fn last(&self) -> i32 {
        i32::try_from(self.list.len()).unwrap_or(i32::MAX) - 1
    }

    /// The selected building's first square, if the selection is on the
    /// list.
    fn chosen(&self) -> Option<Pos> {
        usize::try_from(self.selected)
            .ok()
            .and_then(|n| self.list.get(n))
            .copied()
    }

    fn widget(&mut self, which: Which) -> &mut dyn Widget {
        match which {
            Which::Scroll => &mut self.scroll,
            Which::Renovate => &mut self.renovate,
            Which::Jump => &mut self.jump,
            Which::Minus => &mut self.minus,
            Which::Plus => &mut self.plus,
            Which::Close => &mut self.close,
        }
    }

    /// Draws a widget with the form's bitmaps.
    fn draw_widget(&mut self, which: Which, font: &Font, show: bool) {
        let mut image = std::mem::replace(&mut self.image, Surface::new(0, 0, 0));
        let bitmaps = std::mem::take(&mut self.bitmaps);
        let skin = Skin {
            font,
            bitmaps: &bitmaps,
        };
        let w = self.widget(which);
        if show {
            w.show(&mut image, &skin);
        } else {
            w.draw(&mut image, &skin);
        }
        self.bitmaps = bitmaps;
        self.image = image;
    }

    /// Opens the form: in the middle of a `screen` of that size, kind 0
    /// (the houses) from the top, the background, the widgets, the
    /// headings and the list. Each opening sets the form up afresh
    /// (1080:145f): the original's close button frees it, Escape only
    /// hides it. // 1080:0ea8, 1080:145f
    pub fn open(&mut self, world: &World, font: &Font, info: &InfoTexts, screen: (i32, i32)) {
        let fresh = Umenuform::new(
            std::mem::take(&mut self.bitmaps),
            std::mem::take(&mut self.texts),
            std::mem::take(&mut self.short_texts),
            std::mem::take(&mut self.short_names),
        );
        *self = fresh;
        self.scroll.min = 0;
        self.scroll.position = 0;
        self.top = (screen.1 - HEIGHT) / 2;
        self.left = (screen.0 - WIDTH) / 2;
        let all = self.image.bounds();
        self.image.draw(0, 0, &self.bitmaps[BACKGROUND], all);
        for w in WIDGETS {
            self.draw_widget(w, font, true);
        }
        self.select(self.kind, world, font, info); // 1080:0bfb
        let title = self.text(TITLE);
        font.draw(&mut self.image, &title, (0x10, 1), WIDTH, Align::Center, 1);
        let headings = [
            (self.text(NAME), (8, 0x20), 100, Align::Left),
            (concat(self.text(FULL), b"_%"), (0x50, 0x20), 0x46, Align::Left), // 1080:0ea3
            (concat(self.text(CONDITION), b"_%"), (0xaa, 0x20), 0x3c, Align::Left),
            (
                concat(concat(self.text(PROFIT), b"_"), &info.currency),
                (0xcd, 0x20),
                100,
                Align::Right,
            ), // 1080:0ea6
            (
                concat(concat(self.text(RENT), b"_"), &self.text(RENT_UNIT)),
                (0xb4, 0xf9),
                0x67,
                Align::Center,
            ),
        ];
        for (text, at, width, align) in headings {
            font.draw(&mut self.image, &text, at, width, align, COLOUR);
        }
        self.draw_list(world, font, info);
        self.visible = true;
    }

    /// Shows kind `kind`: the kinds' pictures with its own down, the
    /// heading of the price, and its list from the top. // 1080:0bfb
    pub fn select(&mut self, kind: i32, world: &World, font: &Font, info: &InfoTexts) {
        let all = self.image.bounds();
        let (x0, y) = KIND_PICTURES;
        let place = |k: i32| if k < ROADS { k } else { k - 1 };
        for n in 0..KINDS - 1 {
            let source = if n < ROADS { n } else { n + 1 };
            let from = Rect::new(source * KIND_SIZE + KIND_SOURCE, 0, KIND_SIZE, KIND_SIZE);
            self.image
                .blit(n * KIND_SIZE + x0, y, &self.bitmaps[BUTTONS_UP], from, Rop::Copy, all);
        }
        self.image.blit(
            0xb8,
            0xf9,
            &self.bitmaps[BACKGROUND],
            Rect::new(0xb8, 0xf9, 99, 0x10),
            Rop::Copy,
            all,
        );
        let heading = if kind == 0 || kind == 1 {
            concat(concat(self.text(RENT), b"_"), &self.text(RENT_UNIT)) // 1080:0bf9
        } else {
            concat(concat(self.price_heading(kind), b"_"), &info.currency)
        };
        font.draw(&mut self.image, &heading, (0xb4, 0xf9), 0x67, Align::Center, COLOUR);
        let from = Rect::new(kind * KIND_SIZE + KIND_SOURCE, 0, KIND_SIZE, KIND_SIZE);
        self.image.blit(
            place(kind) * KIND_SIZE + x0,
            y,
            &self.bitmaps[BUTTONS_DOWN],
            from,
            Rop::Copy,
            all,
        );
        self.scroll.position = 0;
        self.draw_list(world, font, info);
    }

    /// The heading of a facility kind's price: `INFOBOX.TXT`'s short text
    /// for its class, the energy's (11) the eighth. // 1030:0465
    fn price_heading(&self, kind: i32) -> Vec<u8> {
        let n = match kind {
            2..=6 | 8..=10 => usize::try_from(kind - 2).ok().and_then(|k| CLASSES.get(k)).copied(),
            11 => Some(7),
            // The original leaves the text as it was.
            _ => None,
        };
        n.and_then(|n| self.short_texts.get(usize::from(n)))
            .cloned()
            .unwrap_or_default()
    }

    /// The list of the human's buildings of the kind, the scroll bar, the
    /// rows from its position, and the selected one's rent or price.
    /// With `overview_list` the price's place is cleared with its own
    /// background. // 1080:02d9
    pub fn draw_list(&mut self, world: &World, font: &Font, info: &InfoTexts) {
        let all = self.image.bounds();
        let bg = &self.bitmaps[BACKGROUND];
        self.image
            .blit(0, 0x2f, bg, Rect::new(0, 0x2f, 0x122, 0xc9), Rop::Copy, all);
        // BUG(original): the price's place is cleared with the background from row 0x43, not its own.
        let from = if self.enhance.has(Enhancement::OverviewList) {
            0x109
        } else {
            0x43
        };
        self.image
            .blit(200, 0x109, bg, Rect::new(200, from, 0x43, 0x10), Rop::Copy, all);
        let t = &world.tables;
        // Single human: the human's buildings, player 1's.
        let ours =
            |pos: Pos, b: u8| world.island.owners.get(pos) == Some(1) && i32::from(t.hausdata(b, 5)) == self.kind;
        self.list = if self.kind == 0 || self.kind == 1 {
            world
                .houses
                .iter()
                .filter(|h| ours(h.pos(), h.building()))
                .map(crate::world::House::pos)
                .take(LIST_CAPACITY)
                .collect()
        } else {
            world
                .facilities
                .iter()
                .filter(|f| ours(f.pos(), f.building()))
                .map(crate::world::Facility::pos)
                .take(LIST_CAPACITY)
                .collect()
        };
        let n = self.last() + 1;
        self.scroll.max = (n - ROWS).max(0);
        self.draw_widget(Which::Scroll, font, false);
        for e in 0..ROWS {
            let row = e + self.scroll.position;
            if row > self.last() {
                continue;
            }
            // BUG(original): after the key Up with fewer than 12 buildings the list's top is before its start, and those rows are read from before the list; the port leaves them empty.
            let Some(&pos) = usize::try_from(row).ok().and_then(|r| self.list.get(r)) else {
                continue;
            };
            let colour = COLOURS[usize::from(row == self.selected)];
            self.draw_row(world, font, info, pos, e, colour);
        }
        if let Some(pos) = self.chosen() {
            let price = self.price_text(world, info, pos);
            font.draw(&mut self.image, &price, (200, 0x109), 0x43, Align::Center, COLOUR);
        }
    }

    /// The selected building's rent a week, or its price per visit.
    fn price_text(&self, world: &World, info: &InfoTexts, pos: Pos) -> Vec<u8> {
        if self.kind == 0 || self.kind == 1 {
            let rent = world
                .houses
                .iter()
                .find(|h| h.pos() == pos)
                .map_or(0, crate::world::House::price);
            format_money(rent / 4, info.currency_factor, info.land)
        } else {
            let Some(f) = world.facilities.iter().find(|f| f.pos() == pos) else {
                return Vec::new();
            };
            let capacity = i32::from(world.tables.hausdata(f.building(), 3));
            // BUG(original): a type with no capacity divides by zero; the port shows 0.
            let per_visit = (f.price().wrapping_mul(100) / 30).checked_div(capacity).unwrap_or(0);
            decimal_money(per_visit, info.currency_factor, info.land, self.enhance)
        }
    }

    /// One row of the list, `e` rows down: the short name, how full it is
    /// and its condition with their bars, and the profit a month (the
    /// takings less `HAUSPREI.DAT`'s running costs). // 1080:02d9
    fn draw_row(&mut self, world: &World, font: &Font, info: &InfoTexts, pos: Pos, e: i32, colour: i32) {
        let t = &world.tables;
        let (building, full, condition, takings) = if self.kind == 0 || self.kind == 1 {
            let Some(h) = world.houses.iter().find(|h| h.pos() == pos) else {
                return;
            };
            let capacity = i32::from(t.hausdata(h.building(), 3));
            // BUG(original): a type with no capacity divides by zero; the port shows 0.
            let full = (i32::from(h.guests()) * 100).checked_div(capacity).unwrap_or(0);
            (
                h.building(),
                full,
                h.condition(),
                i32::from(h.guests()).wrapping_mul(h.price()),
            )
        } else {
            let Some(f) = world.facilities.iter().find(|f| f.pos() == pos) else {
                return;
            };
            let fill = i32::from(f.fill());
            (f.building(), fill, f.condition(), fill.wrapping_mul(f.price()) / 100)
        };
        let y = e * ROW_HEIGHT + LIST_TOP;
        let all = self.image.bounds();
        let name = self.short_names.get(usize::from(building)).cloned().unwrap_or_default();
        font.draw(&mut self.image, &name, (8, y), 100, Align::Left, colour);
        let condition = i32::from(condition) * 100 / 0x7f;
        for (x, percent) in [(0x6c, full), (0xb4, condition)] {
            let from = Rect::new(bar(percent), 0, 0x20, 0x10);
            self.image.blit(x, y - 1, &self.bitmaps[BARS], from, Rop::Copy, all);
            font.draw(
                &mut self.image,
                percent.to_string().as_bytes(),
                (x, y),
                0x20,
                Align::Center,
                colour,
            );
        }
        let profit = takings.wrapping_sub(t.hausprei(building, 1));
        let profit = format_money(profit, info.currency_factor, info.land);
        font.draw(&mut self.image, &profit, (0xbe, y), 100, Align::Right, colour);
    }

    /// The form draws itself again after its caller applied the command
    /// it gave.
    pub fn redraw(&mut self, redraw: Redraw, world: &World, font: &Font, info: &InfoTexts) {
        match redraw {
            Redraw::List => self.draw_list(world, font, info),
            Redraw::Kind => self.select(self.kind, world, font, info),
        }
    }

    /// `umenu_refresh`: the list again, if the form is shown, after the
    /// world's tick ([`crate::simulation::Tick::umenu`]). Whether it
    /// changed. // 1080:0123
    pub fn refresh(&mut self, world: &World, font: &Font, info: &InfoTexts) -> bool {
        if self.visible {
            self.draw_list(world, font, info);
        }
        self.visible
    }

    /// The form hides. // 1080:0e92
    pub fn hide(&mut self) -> Response {
        self.visible = false;
        Response {
            closed: true,
            refresh: true,
            ..Response::default()
        }
    }

    /// A widget's `Click`. // 1080:0be9, 1080:014c, 1080:017f, 1080:01b2, 1080:0235, 1080:02bc
    fn click(&mut self, which: Which, world: &World, font: &Font, info: &InfoTexts, r: &mut Response) {
        let chosen = self.chosen();
        match which {
            Which::Scroll => self.draw_list(world, font, info),
            Which::Renovate => {
                if let Some(pos) = chosen {
                    r.action = Some(Action::Renovate(pos)); // 1010:1931
                    r.redraw = Some(Redraw::List);
                }
            }
            Which::Jump => r.jump = chosen, // 1080:017f
            Which::Minus | Which::Plus => {
                if let Some(at) = chosen {
                    r.action = Some(Action::Price {
                        at,
                        raise: which == Which::Plus,
                    });
                    r.redraw = Some(Redraw::List);
                }
            }
            Which::Close => {
                let h = self.hide();
                r.closed = h.closed;
            }
        }
    }

    /// A widget's outcome: drawn again, its sound, its `Click`.
    fn finish(&mut self, which: Which, out: Outcome, world: &World, font: &Font, info: &InfoTexts, r: &mut Response) {
        if out.repaint {
            self.draw_widget(which, font, false);
        }
        if let Some(s) = out.sound {
            r.sounds.push(s.number().to_le_bytes()[0]);
        }
        if out.clicked {
            self.click(which, world, font, info, r);
        }
        r.refresh |= out.handled || out.repaint;
    }

    /// `bildMouseDown` at `x`, `y` in the form, with the left button: a
    /// kind's picture, a row of the list, then the widgets until one takes
    /// it; else the title bar starts a drag. // 1080:1c3b
    pub fn mouse_down(
        &mut self,
        x: i32,
        y: i32,
        shift: ShiftState,
        world: &World,
        font: &Font,
        info: &InfoTexts,
    ) -> Response {
        let mut r = Response::default();
        self.dragging = false;
        if !shift.contains(ShiftState::LEFT) {
            return r;
        }
        if 7 < x && x < 0x11c && 0x11e < y && y < 0x139 {
            let mut kind = (x - 8) / KIND_SIZE;
            if kind >= ROADS {
                kind += 1;
            }
            self.kind = kind;
            self.select(kind, world, font, info);
            r.refresh = true;
        }
        // BUG(original): columns 299 and 300 are both the list's and the scroll bar's.
        let list_end = if self.enhance.has(Enhancement::OverviewList) {
            self.scroll.frame.left
        } else {
            0x12d
        };
        if 7 < x && x < list_end && 0x2f < y && y < 0xf1 {
            self.selected = (y - LIST_TOP) / ROW_HEIGHT + self.scroll.position;
            self.draw_list(world, font, info);
            r.refresh = true;
        }
        for which in WIDGETS {
            let out = match which {
                Which::Scroll => self.scroll.mouse_down(x, y, self.enhance),
                Which::Renovate => self.renovate.mouse_down(x, y),
                Which::Jump => self.jump.mouse_down(x, y),
                Which::Minus => self.minus.mouse_down(x, y),
                Which::Plus => self.plus.mouse_down(x, y),
                Which::Close => self.close.mouse_down(x, y),
            };
            if out.handled {
                self.finish(which, out, world, font, info, &mut r);
                return r;
            }
        }
        if 0xf < x && x <= WIDTH && -1 < y && y < TITLE_HEIGHT {
            self.dragging = true;
            self.grip = (x, y);
            if !self.rolled_up {
                self.saved_height = self.height;
                self.height = TITLE_HEIGHT;
            }
            self.rolled_up = shift.contains(ShiftState::DOUBLE);
            r.refresh = true;
        }
        r
    }

    /// `bildMouseMove`: the scroll bar's thumb with exactly the left button
    /// down, and the drag, the form kept on a `screen` of that size.
    /// // 1080:1e54
    #[allow(clippy::too_many_arguments)]
    pub fn mouse_move(
        &mut self,
        x: i32,
        y: i32,
        shift: ShiftState,
        world: &World,
        font: &Font,
        info: &InfoTexts,
        screen: (i32, i32),
    ) -> Response {
        let mut r = Response::default();
        if shift == ShiftState::LEFT {
            let out = self.scroll.mouse_move(x, y);
            self.finish(Which::Scroll, out, world, font, info, &mut r);
        }
        if self.dragging && shift.contains(ShiftState::LEFT) {
            let (sx, sy) = (x + self.left, y + self.top);
            self.left = (sx - self.grip.0).max(0).min(screen.0 - WIDTH);
            self.top = (sy - self.grip.1).max(0).min(screen.1 - self.height);
            r.refresh = true;
        }
        r
    }

    /// The list's scroll bar.
    #[must_use]
    pub fn scroll_bar(&self) -> &ScrollBar {
        &self.scroll
    }

    /// `scroll_bars`' repeat of the list's scroll arrow held down (a port
    /// extension, [`ScrollBar::repeat`]), `ms` after the last call.
    pub fn repeat(&mut self, ms: u32, world: &World, font: &Font, info: &InfoTexts) -> Response {
        let mut r = Response::default();
        let out = self.scroll.repeat(ms, self.enhance);
        if out.handled {
            self.finish(Which::Scroll, out, world, font, info, &mut r);
        }
        r
    }

    /// The mouse wheel over the form (`mouse_wheel`, a port extension):
    /// the list scrolls `notches` up, by its scroll bar
    /// ([`ScrollBar::wheel`]).
    pub fn wheel(&mut self, notches: i32, world: &World, font: &Font, info: &InfoTexts) -> Response {
        let mut r = Response::default();
        let out = self.scroll.wheel(notches);
        if out.handled {
            self.finish(Which::Scroll, out, world, font, info, &mut r);
        }
        r
    }

    /// `bildMouseUp`: the widgets until one takes it, their `Click`s; else
    /// the drag ends and the form unrolls, unless it was double-clicked.
    /// // 1080:1f18
    pub fn mouse_up(&mut self, x: i32, y: i32, world: &World, font: &Font, info: &InfoTexts) -> Response {
        let mut r = Response::default();
        for which in WIDGETS {
            let out = match which {
                Which::Scroll => self.scroll.mouse_up(x, y, self.enhance),
                Which::Renovate => self.renovate.mouse_up(x, y),
                Which::Jump => self.jump.mouse_up(x, y),
                Which::Minus => self.minus.mouse_up(x, y),
                Which::Plus => self.plus.mouse_up(x, y),
                Which::Close => self.close.mouse_up(x, y),
            };
            if out.handled {
                self.finish(which, out, world, font, info, &mut r);
                return r;
            }
        }
        if self.dragging && !self.rolled_up {
            self.height = self.saved_height;
            r.refresh = true;
        }
        r
    }

    /// `FormKeyDown`, alone or with Shift: Escape and Space close, R
    /// renovates, Left and Right go through the kinds, Up and Down through
    /// the list, `+` and `-` change the price, Enter jumps; Alt+F4 closes.
    /// // 1080:202b
    pub fn key_down(&mut self, key: u16, shift: ShiftState, world: &World, font: &Font, info: &InfoTexts) -> Response {
        let mut r = Response::default();
        if shift == ShiftState::ALT && key == VK_F4 {
            return self.hide();
        }
        if !(shift.0 == 0 || shift == ShiftState::SHIFT) {
            return r;
        }
        match key {
            VK_ESCAPE | VK_SPACE => return self.hide(),
            VK_R => self.click(Which::Renovate, world, font, info, &mut r),
            VK_LEFT => {
                self.kind = if self.kind == 0 { KINDS - 1 } else { self.kind - 1 };
                if self.kind == ROADS {
                    self.kind = ROADS - 1;
                }
                self.select(self.kind, world, font, info);
            }
            VK_RIGHT => {
                self.kind = if self.kind == KINDS - 1 { 0 } else { self.kind + 1 };
                if self.kind == ROADS {
                    self.kind = ROADS + 1;
                }
                self.select(self.kind, world, font, info);
            }
            VK_UP | VK_DOWN if self.enhance.has(Enhancement::OverviewList) => {
                // The selection moves, and the list only as far as it
                // needs to keep it in view.
                if key == VK_UP && self.selected > 0 {
                    self.selected -= 1;
                } else if key == VK_DOWN && self.selected < self.last() {
                    self.selected += 1;
                }
                let top = self.scroll.position;
                let top = if self.selected < top {
                    self.selected
                } else if self.selected > top + ROWS - 1 {
                    self.selected - (ROWS - 1)
                } else {
                    top
                };
                let top = top.min(self.scroll.max).max(self.scroll.min.max(0));
                if top != self.scroll.position {
                    self.scroll.position = top;
                    self.draw_widget(Which::Scroll, font, false);
                }
                self.draw_list(world, font, info);
            }
            VK_UP => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
                // BUG(original): the kind is drawn again from the list's top first; Up then puts the selection in the top row, Down in the bottom row.
                self.select(self.kind, world, font, info);
                if self.scroll.position < self.selected {
                    // BUG(original): with fewer than 12 buildings this is before the list's start.
                    self.scroll.position = (self.last() - (ROWS - 1)).min(self.selected);
                    self.draw_widget(Which::Scroll, font, false);
                }
                self.draw_list(world, font, info);
            }
            VK_DOWN => {
                if self.selected < self.last() {
                    self.selected += 1;
                }
                self.select(self.kind, world, font, info);
                if self.selected > ROWS - 2 {
                    self.scroll.position = self.last().min(self.selected - (ROWS - 1));
                    self.draw_widget(Which::Scroll, font, false);
                }
                self.draw_list(world, font, info);
            }
            VK_PLUS | VK_ADD | VK_MINUS | VK_SUBTRACT => {
                let which = if key == VK_PLUS || key == VK_ADD {
                    Which::Plus
                } else {
                    Which::Minus
                };
                self.click(which, world, font, info, &mut r);
                r.redraw = Some(Redraw::Kind);
            }
            VK_RETURN => self.click(Which::Jump, world, font, info, &mut r),
            _ => return r,
        }
        r.refresh = true;
        r
    }

    /// The form's bounds on the screen.
    #[must_use]
    pub fn bounds(&self) -> Rect {
        Rect::new(self.left, self.top, WIDTH, self.height)
    }

    /// `bild`'s bounds in the form, and its picture.
    #[must_use]
    pub fn images(&self) -> [(&'static str, Rect, &Surface); 1] {
        [("bild", Rect::new(0, 0, WIDTH, HEIGHT), &self.image)]
    }

    /// Whether `x`, `y` (on the screen) is on the form as it's shown.
    #[must_use]
    pub fn contains(&self, x: i32, y: i32) -> bool {
        self.visible && (self.left..self.left + WIDTH).contains(&x) && (self.top..self.top + self.height).contains(&y)
    }
}

/// Appends `b` to the Pascal string `a`, at most 255 characters.
fn concat(mut a: Vec<u8>, b: &[u8]) -> Vec<u8> {
    a.extend_from_slice(b);
    a.truncate(255);
    a
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bars() {
        assert_eq!(bar(100), 0);
        assert_eq!(bar(91), 0);
        assert_eq!(bar(90), 0x20);
        assert_eq!(bar(1), 9 * 0x20);
        // `(0 - 1) div 10` is 0.
        assert_eq!(bar(0), 9 * 0x20);
    }
}
