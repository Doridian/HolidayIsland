//! `TGeldmenuform`, the money menu (segment `1070`, the `Geldmenu` unit):
//! the button bar's fifth button (4, or the keys `5` and `G`, 1020:2e09)
//! opens and closes it (1010:2c8b). Six pages, picked by the tabs of its
//! menu bar: the desk (`Schreibtisch`: advertising, renovation, insurance,
//! the loan), the balance sheet (`Bilanz`: last and this year's books) and
//! four charts of the books' monthly history (money less the loan, net
//! worth, guests, reputation). The menu bar's other buttons close the menu
//! and open the building overview (`TUmenuform`).
//!
//! The books themselves are the world's ([`crate::world::Books`],
//! [`crate::accounts`]). What the form changes in the world goes through
//! [`Action`]s (`command::Action::MoneyMenu`), carried out by
//! [`World::money_menu_command`]; the view, [`Geldmenuform`], hands them
//! to the caller in its [`Response`]s.
//!
//! The form is three `TImage`s whose places and sizes, like every button
//! and text on them, come from `GELDMENU.DAT`'s rectangles ([`RECTS`]):
//! the title bar (`UeberschriftImage`), the page (`BildImage`) and the menu
//! bar (`MenuImage`). It isn't modal and doesn't stop the clock: while it's
//! shown every booking redraws the page ([`Tick::books`]).
//!
//! Single human: the form and the books are player 1's.

#![allow(clippy::many_single_char_names)]

use formats::data::Rect as DatRect;
use vcl::Rect;
use vcl::surface::{Rop, Surface};

use crate::command;
use crate::hauptschirm::InfoTexts;
use crate::money::format_money;
use crate::simulation::Tick;
use crate::text::{Align, Font};
use crate::widgets::{BitmapId, Button, CheckBox, Frame, Label, ShiftState, Skin, Sound, Widget};
use crate::world::{FIRST_BOOK, World};

/// What the form does to the world, for the player who issues it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    /// The form was shown (1070:2b3a, which works out this year's total
    /// and the balance at once) or hidden (1070:2bb4). While it's shown,
    /// every booking works the total out again (1070:2617,
    /// [`World::geldmenu_refresh`]). Single human: only player 1's form
    /// counts.
    Shown(bool),
    /// "Geld leihen": borrow `Kreditgroesse` (1078:05c6), unless the loan
    /// would pass `MaxSchulden`; then the bank says no
    /// ([`crate::money::Finances::credit_refused`]). // 1070:04d3
    TakeLoan,
    /// "Kredit tilgen": pay `Kreditgroesse` back if the money is there
    /// and anything is owed (1078:061b). // 1070:04d3
    RepayLoan,
    /// The advertising arrows: one level more (at most 7) or less (at
    /// least 0). // 1070:04d3
    Advertising { more: bool },
    /// The renovation arrows: the interval through [`RENOVATION_MORE`] or
    /// [`RENOVATION_LESS`]. // 1070:04d3
    Renovation { more: bool },
    /// The insurance check box: insured or not (`1110:0072`). Single
    /// human: one flag, player 1's. // 1070:01fc
    Insurance(bool),
}

/// The renovation interval the left arrow (0x12) gives for each interval
/// (0 never, else every so many years): a year more, after five never,
/// and never stays never. // 1110:329a
pub const RENOVATION_LESS: [u8; 6] = [0, 2, 3, 4, 5, 0];
/// The right arrow's (0x13): a year less, at least every year; from never,
/// every five years. // 1110:3294
pub const RENOVATION_MORE: [u8; 6] = [5, 1, 1, 2, 3, 4];

/// The highest advertising level. // 1070:04d3
const MAX_ADVERTISING: u8 = 7;

/// The kinds of the books the form shows as the balance: player 1's money,
/// minus their loan, and the difference. // 1070:2ab8
const BOOK_MONEY: u8 = 0x6c;
const BOOK_LOAN: u8 = 0x6d;
const BOOK_BALANCE: u8 = 0x6e;
/// This year's kinds the total adds up, and the total. // 1070:1452
const BOOK_THIS_YEAR: u8 = 0x5f;
const BOOK_THIS_YEAR_LAST: u8 = 0x6a;
const BOOK_THIS_YEAR_TOTAL: u8 = 0x6b;

impl World {
    /// Carries out the money menu's `action` for `player`. // 1070:04d3,
    /// 1070:01fc, 1070:2b3a, 1070:2bb4
    pub(crate) fn money_menu_command(&mut self, player: u8, action: Action) {
        let Some(i) = usize::from(player).checked_sub(1).filter(|&i| i < 8) else {
            return;
        };
        match action {
            // Single human: the form whose state the books follow is player 1's.
            Action::Shown(shown) => {
                if player == 1 {
                    self.money_menu_shown = shown;
                    if shown {
                        self.geldmenu_year_total(); // 1070:2b3a
                    }
                }
            }
            Action::TakeLoan => {
                self.take_loan(player, self.finances.credit_size); // 1078:05c6
                self.geldmenu_balance(player); // 1070:2ab8
            }
            Action::RepayLoan => {
                self.repay_loan(player, self.finances.credit_size); // 1078:061b
                self.geldmenu_balance(player);
            }
            Action::Advertising { more } => {
                // `*1110:6f24 + player - 1`: `max(level - 1, 0)`, `min(level + 1, 7)`.
                if let Some(level) = self.player_block.get_mut(i) {
                    *level = if more {
                        level.saturating_add(1).min(MAX_ADVERTISING)
                    } else {
                        level.saturating_sub(1)
                    };
                }
            }
            Action::Renovation { more } => {
                let table = if more { &RENOVATION_MORE } else { &RENOVATION_LESS };
                let years = &mut self.renovation_years[i];
                // BUG(original): an interval past 5 (from a map) reads the bytes after the table; the port leaves it.
                if let Some(&next) = table.get(usize::from(*years)) {
                    *years = next;
                }
            }
            Action::Insurance(on) => {
                // Single human: `1110:0072` is one flag, the insurance of player 1.
                if player == 1 {
                    self.insured = on;
                }
            }
        }
    }

    /// The books' total of this year (kind 0x6b) from its kinds 0x5f to
    /// 0x6a, then the balance ([`World::geldmenu_balance`]). The original
    /// then redraws the form's page (1070:1eeb), the view's part.
    /// // 1070:1452
    pub(crate) fn geldmenu_year_total(&mut self) {
        let first = usize::from(BOOK_THIS_YEAR - FIRST_BOOK);
        let last = usize::from(BOOK_THIS_YEAR_LAST - FIRST_BOOK);
        let total = self.books.year[first..=last]
            .iter()
            .fold(0i32, |s, &v| s.wrapping_add(v));
        self.books.year[usize::from(BOOK_THIS_YEAR_TOTAL - FIRST_BOOK)] = total;
        self.geldmenu_balance(1);
    }

    /// Kinds 0x6c to 0x6e: player 1's money (1078:0191, 99 999 999 while
    /// it's frozen), their loan negated and the difference. They're only
    /// shown and saved. Single human: the books are player 1's, so the
    /// port does nothing for another `player`. // 1070:2ab8
    fn geldmenu_balance(&mut self, player: u8) {
        if player != 1 {
            return;
        }
        let money = self.money_of(1);
        let loan = self.loan_of(1); // 1078:057a
        let y = &mut self.books.year;
        y[usize::from(BOOK_MONEY - FIRST_BOOK)] = money;
        y[usize::from(BOOK_LOAN - FIRST_BOOK)] = loan.wrapping_neg();
        y[usize::from(BOOK_BALANCE - FIRST_BOOK)] = money.wrapping_sub(loan);
    }

    /// What every booking does in the original (1070:2617), and loading a
    /// game (1070:2630): this year's total again if the money menu is
    /// shown. The original works it out at once, in the middle of the
    /// tick or command; the port does it at the end of each, for what it
    /// booked ([`Tick::books`]) or loaded ([`Tick::loaded`]). Only the
    /// saved kinds 0x6b–0x6e can tell the difference, and only when money
    /// changes after a booking in the same tick. // 1070:2617, 1070:2630
    pub(crate) fn geldmenu_refresh(&mut self, tick: &Tick) {
        if self.money_menu_shown && (tick.books || tick.loaded) {
            self.geldmenu_year_total();
        }
    }

    /// What `player` pays for advertising a month: their beds
    /// (`*1110:6f24 + 0x3c + 4·player`) times their advertising level
    /// times `Werbungskosten`. // 1008:05d9
    #[must_use]
    pub fn advertising_cost(&self, player: u8) -> i32 {
        let at = 0x3c + 4 * usize::from(player);
        let beds = self
            .player_block
            .get(at..at + 4)
            .map_or(0, |b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]));
        beds.wrapping_mul(i32::from(self.advertising_level(player)))
            .wrapping_mul(self.settlement.advertising_cost)
    }

    /// `player`'s advertising level, 0–7; 0 for player 0. // *1110:6f24 + player - 1
    #[must_use]
    pub fn advertising_level(&self, player: u8) -> u8 {
        usize::from(player)
            .checked_sub(1)
            .and_then(|i| self.player_block.get(i))
            .copied()
            .unwrap_or(0)
    }

    /// What the human's insurance costs a month: a 120th of their
    /// buildings' value if they're insured, else 0. Single human.
    /// // 1008:062c
    #[must_use]
    pub fn insurance_cost(&self) -> i32 {
        if self.insured { self.assets(1) / 120 } else { 0 }
    }
}

/// The bitmaps the form draws with, from `GRAFIK`, in the order
/// [`Geldmenuform::new`] takes them. // 1070:2e6c
pub const BITMAP_FILES: [&str; 16] = [
    "spieler.bmp",  // +0x20f, the players' colours for the charts
    "geldfin.bmp",  // +0x213, page 0's background
    "geldbil.bmp",  // +0x217, page 1's
    "geldstat.bmp", // +0x21b, the charts'
    "geldmenu.BMP", // +0x21f, the menu bar
    "geldueb.BMP",  // +0x223, the title bar
    "geldmepr.BMP", // +0x227, the menu bar's tabs pressed
    "geldb1pr.BMP", // +0x22b, the loan buttons pressed
    "geldb7pr.BMP", // +0x22f, the left arrows pressed
    "geldb3pr.BMP", // +0x233, "Schließen" pressed
    "geldb2pr.BMP", // +0x237, the right arrows pressed
    "geldch.BMP",   // +0x23b, the check box
    "geldchpr.BMP", // +0x23f, ticked
    "geldb5.BMP",   // +0x243, the overview's button
    "geldb5pr.BMP", // +0x247, pressed
    "closepre.bmp", // 1110:54bc, `hintergrund`'s (state 100)
];
const SPIELER: usize = 0;
const PAGE_BACKGROUNDS: [usize; 3] = [1, 2, 3];
const MENU_BACKGROUND: usize = 4;
const TITLE_BACKGROUND: usize = 5;
const TABS_PRESSED: usize = 6;
const LOAN_PRESSED: usize = 7;
const LEFT_PRESSED: usize = 8;
const CLOSE_BUTTON_PRESSED: usize = 9;
const RIGHT_PRESSED: usize = 10;
const CHECK: BitmapId = BitmapId(11);
const CHECKED: BitmapId = BitmapId(12);
const OVERVIEW: BitmapId = BitmapId(13);
const OVERVIEW_PRESSED: BitmapId = BitmapId(14);
const CLOSE_PRESSED: usize = 15;

/// How many rectangles `GELDMENU.DAT` has (`formats::data::geldmenu_dat`).
pub const RECTS: usize = 0x76;

/// The rectangles' meanings, by their index in `GELDMENU.DAT`: left, top,
/// right and bottom, inclusive, in the image they're on. 0 the form, 1–3
/// the three images' places, 4 the title bar, 8 the menu bar and 0x11 the
/// page as wholes; 9–0xe the tabs; 0x6f the charts. // 1070:2bfa, Self+0x692
const FORM: u8 = 0;
const TITLE_PLACE: u8 = 1;
const BILD_PLACE: u8 = 2;
const MENU_PLACE: u8 = 3;
const TITLE_BAR: u8 = 4;
const CLOSE: u8 = 5;
const DRAG: u8 = 6;
const TITLE_TEXT: u8 = 7;
const MENU_BAR: u8 = 8;
const FIRST_TAB: u8 = 9;
const LAST_TAB: u8 = 0xe;
const CLOSE_BUTTON: u8 = 0xf;
const CLOSE_BUTTON_TEXT: u8 = 0x10;
const PAGE: u8 = 0x11;
const RENOVATION_LEFT: u8 = 0x12;
const RENOVATION_RIGHT: u8 = 0x13;
const ADVERTISING_LEFT: u8 = 0x14;
const ADVERTISING_RIGHT: u8 = 0x15;
const TAKE_LOAN: u8 = 0x17;
const REPAY_LOAN: u8 = 0x18;
const ADVERTISING_TEXT: u8 = 0x19;
const ADVERTISING_COST: u8 = 0x1a;
const RENOVATION_TEXT: u8 = 0x1b;
/// The first element with a text of `GELDMENU.TXT` (its second line).
const FIRST_TEXT: u8 = 0x1c;
const TAKE_LOAN_TEXT: u8 = 0x20;
const REPAY_LOAN_TEXT: u8 = 0x21;
const CHART: u8 = 0x6f;
const CHART_TITLE: u8 = 0x70;
const CHART_TOP: u8 = 0x71;
const CHART_MIDDLE: u8 = 0x72;
const CHART_BOTTOM: u8 = 0x73;
const CHART_START: u8 = 0x74;
const CHART_END: u8 = 0x75;

/// The page buttons a press on page 0 tries, in order. // 1070:0300
const PAGE_BUTTONS: [u8; 6] = [
    TAKE_LOAN,
    REPAY_LOAN,
    RENOVATION_LEFT,
    RENOVATION_RIGHT,
    ADVERTISING_LEFT,
    ADVERTISING_RIGHT,
];

/// Each page's previous and next, for the arrow keys. // 1110:3232, 1110:3238
const PREVIOUS_PAGE: [u8; 6] = [5, 0, 1, 2, 3, 4];
const NEXT_PAGE: [u8; 6] = [1, 2, 3, 4, 5, 0];
/// The charts' titles, `SONSTIGE.TXT`'s short texts by page (pages 2–5:
/// "Guthaben", "Gesamtvermögen", "Urlauber", "Ruf"). // 1110:328e
const CHART_TITLES: [usize; 6] = [0x1d, 0x1e, 0x1f, 0x20, 0x21, 0x22];
/// The tabs' tooltips, short texts by tab 9–0xe ("Schreibtisch",
/// "Bilanz", then the charts'). // 1110:1630
const TAB_TOOLTIPS: [usize; 6] = [0x1d, 0x1e, 0x1f, 0x20, 0x21, 0x22];

/// The size of one of the four rings of the books' history, and where its
/// scale, months and first month are. // 1070:210a
const RING: usize = 0xf0a;
const RING_MAX: usize = 0xf00;
const RING_MIN: usize = 0xf04;
const RING_MONTHS: usize = 0xf08;
const RING_START: usize = 0xf09;
/// The months of the chart's first date: ten years. // 1070:16b7
const CHART_MONTHS: i32 = 120;
/// The first year of a game, for the chart's first date. // 1030:02ef
const FIRST_YEAR: i32 = 1996;

/// `play_sound`'s numbers. // 10a0:30b6
const SOUND_PRESS: i16 = 8;
const SOUND_PAGE: i16 = 9;
const SOUND_RELEASE: i16 = 10;
const SOUND_OPEN: i16 = 0xd;
const SOUND_CLOSE: i16 = 0xe;
/// The balance sheet's opening sound.
const SOUND_OPEN_BALANCE: i16 = 0x25;

/// `VK_*` codes the form reads. // 1070:39bf
const VK_RETURN: u16 = 0x0d;
const VK_ESCAPE: u16 = 0x1b;
const VK_SPACE: u16 = 0x20;
const VK_LEFT: u16 = 0x25;
const VK_RIGHT: u16 = 0x27;
const VK_F4: u16 = 0x73;

/// The form's texts, from the CD.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Texts {
    /// `GELDMENU.TXT`'s 25 texts, for elements 0x1c to 0x34, and the last,
    /// "Schließen" (`formats::data::geldmenu_txt`). // Self+0x189, Self+0x18d
    pub geldmenu: Vec<Vec<u8>>,
    pub close: Vec<u8>,
    /// `SONSTIGE.TXT`'s texts and short texts (`formats::data::sonstige`):
    /// 0xe the title, 0x15 the overview's button, 0x16 and 0x17 the
    /// insurance's labels; the short texts for the charts and the tabs.
    /// // *1110:734e, *1110:737a
    pub sonstige: Vec<Vec<u8>>,
    pub short: Vec<Vec<u8>>,
    /// `MODATYP.TXT`'s `WerbungToText` and `RenovierungToText`
    /// (`formats::data::modatyp_txt`). // *1110:64b0, *1110:64b4
    pub advertising: Vec<Vec<u8>>,
    pub renovation: Vec<Vec<u8>>,
    /// The months, the money's format and the currency.
    pub info: InfoTexts,
}

impl Texts {
    fn get(list: &[Vec<u8>], n: usize) -> &[u8] {
        list.get(n).map_or(&[], |t| t)
    }
}

/// What an event did, for the caller.
#[allow(clippy::struct_excessive_bools)] // One flag for each thing the original does.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[must_use]
pub struct Response {
    /// The images changed and the original refreshes them.
    pub refresh: bool,
    /// The sounds played, in order (`play_sound`, 10a0:30b6).
    pub sounds: Vec<i16>,
    /// World changes, for player 1 (`command::Action::MoneyMenu`). Apply
    /// them, then call [`Geldmenuform::update`] if [`Response::update`].
    pub commands: Vec<command::Action>,
    /// The page shows the world: call [`Geldmenuform::update`] with the
    /// world (after the commands).
    pub update: bool,
    /// The form closed (1070:2bb4): let the button bar's button 4 up
    /// (1010:2ade) and give the main screen the focus.
    pub closed: bool,
    /// The form opened (1070:2b3a): it takes the focus, and the info box
    /// closes (1038:1ee0).
    pub opened: bool,
    /// The building overview's button: `TUmenuform` opens (1080:0ea8).
    /// // 1070:01e8
    pub overview: bool,
    /// Space: the main screen gets the focus (the form stays).
    pub focus_main: bool,
    /// A key the form doesn't use, for the main screen's `FormKeyDown`
    /// (1020:2e09).
    pub main_screen_key: Option<(u16, ShiftState)>,
    /// The form moved or changed its height (dragged by its title bar).
    pub moved: bool,
}

/// The image a press went down on, which keeps the mouse until it comes
/// up (the VCL's capture).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Image {
    Title,
    Bild,
    Menu,
}

/// The money menu.
#[derive(Debug, Clone)]
pub struct Geldmenuform {
    font: Font,
    bitmaps: Vec<Surface>,
    empty: Surface,
    rects: Vec<DatRect>,
    texts: Texts,
    /// The three images' pictures.
    pub title: Surface,
    pub bild: Surface,
    pub menu: Surface,
    /// The form's top left on the screen and its height: 0 while it's
    /// hidden (the form is always `Visible`; 1070:2617 asks for the
    /// height), the title bar's 16 while rolled up or dragged.
    pub left: i32,
    pub top: i32,
    pub height: i32,
    /// The page, 0–5. Saved with the map. // Self+0x188
    page: u8,
    /// The element the left button went down on, 6 for the title bar's
    /// drag, 0 for none. Saved. // Self+0x205
    pressed: u8,
    /// Where the drag took hold, from the form's top left. Saved.
    /// // Self+0x206
    grab: (i32, i32),
    /// The title bar was double-clicked: the form stays rolled up to it.
    /// Saved. // Self+0x696
    rolled_up: bool,
    /// The button bar's button that last opened the form; nothing reads
    /// it. Saved. // Self+0x20e
    opener: u8,
    captured: Option<Image>,
    /// The insurance's check box and its labels, on page 0.
    insurance: CheckBox, // +0x697
    insurance_label: Label, // +0x365
    cost_label: Label,      // +0x474
    cost: Label,            // +0x583
    /// "Gebäudeübersicht", on the menu bar.
    overview: Button, // +0x24f
}

impl Geldmenuform {
    /// The form as `geldmenu_init` makes it at the start (1070:2e6c): at
    /// the top right of a screen `screen_width` wide, its images placed and
    /// sized by `rects` (`GELDMENU.DAT`), the title and the menu bar drawn,
    /// page 0, hidden (height 0). `bitmaps` are those of [`BITMAP_FILES`].
    ///
    /// # Panics
    /// If there aren't as many bitmaps as files or rectangles as
    /// [`RECTS`].
    #[must_use]
    pub fn new(font: Font, bitmaps: Vec<Surface>, rects: Vec<DatRect>, texts: Texts, screen_width: i32) -> Self {
        assert_eq!(bitmaps.len(), BITMAP_FILES.len(), "one bitmap per file");
        assert_eq!(rects.len(), RECTS, "GELDMENU.DAT's rectangles");
        let size = |n: u8| {
            let r = rects[usize::from(n)];
            Surface::new(
                usize::try_from(i32::from(r[2]) + 1).unwrap_or(0),
                usize::try_from(i32::from(r[3]) + 1).unwrap_or(0),
                0,
            )
        };
        let text = |n: usize| Texts::get(&texts.sonstige, n).to_vec();
        let label = |left, top, width, n| Label::new(Frame::new(left, top, width, 0x12), &text(n), Align::Left, 9);
        let insurance = CheckBox::new(
            Label::new(
                Frame::new(0xf2, 0x7c, 0x82, 0x12),
                Texts::get(&texts.geldmenu, usize::from(0x1f - FIRST_TEXT)),
                Align::Left,
                9,
            ),
            CHECK,
            CHECKED,
            // `ui_radio_draw2` draws the text in the label's colour either way.
            9,
        );
        let mut f = Geldmenuform {
            title: size(TITLE_BAR),
            bild: size(PAGE),
            menu: size(MENU_BAR),
            font,
            empty: Surface::new(0, 0, 0),
            // BUG(original): placed at the screen's width less the form's right edge, one column further right than its width allows.
            left: screen_width - i32::from(rects[usize::from(FORM)][2]),
            top: 0,
            height: 0,
            page: 0,
            pressed: 0,
            grab: (0, 0),
            rolled_up: false,
            opener: 0,
            captured: None,
            insurance,
            insurance_label: label(10, 0x7c, 200, 0x16),
            cost_label: label(0x14, 0x91, 0xbe, 0x17),
            cost: Label::new(Frame::new(0x10a, 0x91, 0x68, 0x12), b"", Align::Right, 9),
            overview: Button::new(Frame::new(8, 5, 0x70, 0x20), OVERVIEW, OVERVIEW_PRESSED, &text(0x15), 9),
            rects,
            texts,
            bitmaps,
        };
        f.draw_title(); // 1070:1f3f
        let Geldmenuform {
            font,
            bitmaps,
            menu,
            overview,
            ..
        } = &mut f;
        overview.show(menu, &Skin { font, bitmaps });
        f.draw_menu_bar(); // 1070:1f79
        f
    }

    /// The form's width: `GELDMENU.DAT`'s first rectangle's. // 1070:2e6c
    #[must_use]
    pub fn width(&self) -> i32 {
        self.right(FORM) + 1
    }

    /// The form's bounds on the screen.
    #[must_use]
    pub fn bounds(&self) -> Rect {
        Rect::new(self.left, self.top, self.width(), self.height)
    }

    /// Whether the form is shown (its height above 0). // 1070:2617
    #[must_use]
    pub fn shown(&self) -> bool {
        self.height > 0
    }

    /// The page, 0–5.
    #[must_use]
    pub fn page(&self) -> u8 {
        self.page
    }

    /// The images' names and places in the form (`GELDMENU.DAT`'s 1–3, and
    /// the sizes of its 4, 0x11 and 8). // 1070:2e6c
    #[must_use]
    pub fn layout(&self) -> [(&'static str, Rect); 3] {
        let place = |n: u8, s: &Surface| {
            let r = self.rect(n);
            Rect::new(r.0, r.1, dim(s.width), dim(s.height))
        };
        [
            ("UeberschriftImage", place(TITLE_PLACE, &self.title)),
            ("BildImage", place(BILD_PLACE, &self.bild)),
            ("MenuImage", place(MENU_PLACE, &self.menu)),
        ]
    }

    /// Each image with the name of the `TImage` it goes into.
    #[must_use]
    pub fn images(&self) -> [(&'static str, &Surface); 3] {
        [
            ("UeberschriftImage", &self.title),
            ("BildImage", &self.bild),
            ("MenuImage", &self.menu),
        ]
    }

    fn rect(&self, n: u8) -> (i32, i32, i32, i32) {
        let r = self.rects.get(usize::from(n)).copied().unwrap_or_default();
        (i32::from(r[0]), i32::from(r[1]), i32::from(r[2]), i32::from(r[3]))
    }

    fn right(&self, n: u8) -> i32 {
        self.rect(n).2
    }

    /// Whether `x`, `y` is in element `n`, edges included. // 1070:0294, 1030:1517
    fn hit(&self, n: u8, x: i32, y: i32) -> bool {
        let (l, t, r, b) = self.rect(n);
        (l..=r).contains(&x) && (t..=b).contains(&y)
    }

    /// The image element `n` is on. // 1070:0b76
    fn image_of(&mut self, n: u8) -> Option<&mut Surface> {
        match n {
            0x11..=0x75 => Some(&mut self.bild),
            8..=0x10 => Some(&mut self.menu),
            4..=7 => Some(&mut self.title),
            _ => None,
        }
    }

    /// The background element `n` is erased from: the page's for the
    /// page, the bars' for theirs. // 1070:0be5
    fn background_of(&self, n: u8) -> Option<usize> {
        match n {
            0x11..=0x75 => Some(PAGE_BACKGROUNDS[usize::from(self.page.min(2))]),
            8..=0x10 => Some(MENU_BACKGROUND),
            4..=7 => Some(TITLE_BACKGROUND),
            _ => None,
        }
    }

    /// Copies `bitmap`'s `from` to element `n`'s top left on its image.
    fn blit_to(&mut self, n: u8, bitmap: usize, from: (i32, i32)) {
        let (l, t, r, b) = self.rect(n);
        let Geldmenuform {
            bitmaps,
            empty,
            title,
            bild,
            menu,
            ..
        } = self;
        let image = match n {
            0x11..=0x75 => bild,
            8..=0x10 => menu,
            4..=7 => title,
            _ => return,
        };
        let src = bitmaps.get(bitmap).unwrap_or(empty);
        let clip = image.bounds();
        image.blit(
            l,
            t,
            src,
            Rect::new(from.0, from.1, r - l + 1, b - t + 1),
            Rop::Copy,
            clip,
        );
    }

    /// Element `n` erased: its rectangle copied from the same place of its
    /// background. // 1070:0cef
    fn erase(&mut self, n: u8) {
        if let Some(bg) = self.background_of(n) {
            let (l, t, _, _) = self.rect(n);
            self.blit_to(n, bg, (l, t));
        }
    }

    /// `text` in element `n` in one of the form's styles: 0 right, 1 and 9
    /// centred, 2 left, all in colour 9; 3 centred in colour 1; 4–7 the
    /// same as 0–2 and a red right-aligned 7 after erasing the element; 8
    /// centred a pixel right and down (a button held). // 1070:0dea
    fn put(&mut self, text: &[u8], style: u8, n: u8) {
        if matches!(style, 4..=7) {
            self.erase(n);
        }
        let (align, colour, shift) = match style {
            0 | 4 => (Align::Right, 9, 0),
            1 | 5 | 9 => (Align::Center, 9, 0),
            2 | 6 => (Align::Left, 9, 0),
            3 => (Align::Center, 1, 0),
            7 => (Align::Right, 0xb, 0),
            8 => (Align::Center, 9, 1),
            _ => return,
        };
        let (l, t, r, _) = self.rect(n);
        let font = self.font.clone();
        if let Some(image) = self.image_of(n) {
            font.draw(image, text, (l + shift, t + shift), r - l + 1, align, colour);
        }
    }

    /// `GELDMENU.TXT`'s text for element `n` (0x1c to 0x34).
    fn element_text(&self, n: u8) -> Vec<u8> {
        let k = usize::from(n.wrapping_sub(FIRST_TEXT));
        Texts::get(&self.texts.geldmenu, k).to_vec()
    }

    fn currency(&self) -> Vec<u8> {
        self.texts.info.currency.clone()
    }

    /// An amount with `separator` and the currency. // 1030:11a0
    fn money(&self, amount: i32, separator: u8) -> Vec<u8> {
        let info = &self.texts.info;
        let mut text = format_money(amount, info.currency_factor, info.land);
        text.push(separator);
        text.extend_from_slice(&info.currency);
        text.truncate(255);
        text
    }

    /// The title bar: its background and "FINANZEN". // 1070:1f3f
    fn draw_title(&mut self) {
        self.erase(TITLE_BAR);
        let title = Texts::get(&self.texts.sonstige, 0xe).to_vec();
        self.put(&title, 3, TITLE_TEXT);
    }

    /// The menu bar: its background, the page's tab pressed (its part of
    /// `geldmepr.BMP`, which starts at the first tab), "Schließen" and the
    /// overview's button. // 1070:1f79
    fn draw_menu_bar(&mut self) {
        self.erase(MENU_BAR);
        let tab = FIRST_TAB + self.page;
        let (fl, ft, _, _) = self.rect(FIRST_TAB);
        let (l, t, _, _) = self.rect(tab);
        self.blit_to(tab, TABS_PRESSED, (l - fl, t - ft));
        self.draw_up(CLOSE_BUTTON); // 1070:1100
        let Geldmenuform {
            font,
            bitmaps,
            menu,
            overview,
            ..
        } = self;
        overview.draw(menu, &Skin { font, bitmaps });
    }

    /// Element `n` (a button) let go: erased, and the loan buttons' and
    /// "Schließen"'s captions. // 1070:1100
    fn draw_up(&mut self, n: u8) {
        self.erase(n);
        match n {
            TAKE_LOAN => self.put(&self.element_text(TAKE_LOAN_TEXT), 9, TAKE_LOAN_TEXT),
            REPAY_LOAN => self.put(&self.element_text(REPAY_LOAN_TEXT), 9, REPAY_LOAN_TEXT),
            CLOSE_BUTTON => {
                let close = self.texts.close.clone();
                self.put(&close, 9, CLOSE_BUTTON_TEXT);
            }
            _ => {}
        }
    }

    /// Element `n` (a button) held down: its pressed bitmap, and the
    /// captions a pixel right and down. // 1070:1178
    fn draw_down(&mut self, n: u8) {
        let bitmap = match n {
            CLOSE => CLOSE_PRESSED,
            TAKE_LOAN | REPAY_LOAN => LOAN_PRESSED,
            ADVERTISING_LEFT | RENOVATION_LEFT => LEFT_PRESSED,
            ADVERTISING_RIGHT | RENOVATION_RIGHT => RIGHT_PRESSED,
            CLOSE_BUTTON => CLOSE_BUTTON_PRESSED,
            // BUG(original): another element blits from an unset bitmap handle; the port draws nothing.
            _ => return,
        };
        self.blit_to(n, bitmap, (0, 0));
        match n {
            TAKE_LOAN => self.put(&self.element_text(TAKE_LOAN_TEXT), 8, TAKE_LOAN_TEXT),
            REPAY_LOAN => self.put(&self.element_text(REPAY_LOAN_TEXT), 8, REPAY_LOAN_TEXT),
            CLOSE_BUTTON => {
                let close = self.texts.close.clone();
                self.put(&close, 8, CLOSE_BUTTON_TEXT);
            }
            _ => {}
        }
    }

    /// Redraws the page from `world`, as every refresh of the original
    /// does (1070:1452 calls it): page 0's widgets hidden on the other
    /// pages, the background, the labels, the values, the chart, page 0's
    /// widgets. Call it after applying a [`Response`]'s commands when it
    /// asks ([`Response::update`]), and for each tick that booked
    /// ([`Tick::books`]) or loaded a game while the form is shown. Single
    /// human: the books and values are player 1's. // 1070:1eeb
    pub fn update(&mut self, world: &World) {
        if self.page != 0 {
            // 1070:1ba5: both of its branches hide them.
            self.insurance.radio.frame.visible = false;
            self.insurance_label.frame.visible = false;
            self.cost_label.frame.visible = false;
            self.cost.frame.visible = false;
        }
        self.erase(PAGE);
        self.draw_labels();
        self.draw_values(world);
        self.draw_chart(world);
        self.draw_widgets(world);
    }

    /// After a game was loaded (`game_show_state`, 1010:26fa): the check
    /// box follows the loaded insurance (toggled, with its sound and its
    /// `Click`, which sets the same flag again), then, if the form is
    /// shown, the page is redrawn (the world worked the total out,
    /// 1070:2630), and the menu bar always. // 1010:26fa, 1070:2630
    pub fn loaded(&mut self, world: &World) -> Response {
        let mut r = Response::default();
        if self.insurance.checked() != world.insured {
            let out = self.insurance.toggle(); // 10c0:07fa
            r.sounds.extend(out.sound.map(Sound::number));
        }
        if self.shown() {
            self.update(world);
        }
        self.draw_menu_bar();
        r.refresh = true;
        r
    }

    /// The pages' labels. // 1070:28d3
    fn draw_labels(&mut self) {
        match self.page {
            0 => {
                for n in [0x1c, 0x1e, 0x1d, 0x33, 0x34] {
                    self.put(&self.element_text(n), 2, n);
                }
                let currency = self.currency();
                for n in 0x4f..=0x51 {
                    self.put(&currency, 2, n);
                }
                for n in [TAKE_LOAN_TEXT, REPAY_LOAN_TEXT] {
                    self.put(&self.element_text(n), 1, n);
                }
            }
            1 => {
                for n in 0x22..=0x34 {
                    self.put(&self.element_text(n), 2, n);
                }
                let currency = self.currency();
                for n in 0x35..=0x51 {
                    self.put(&currency, 2, n);
                }
            }
            2..=5 => {
                let title = Texts::get(&self.texts.short, CHART_TITLES[usize::from(self.page)]).to_vec();
                self.put(&title, 1, CHART_TITLE);
            }
            _ => {}
        }
    }

    /// The books' `kind` in element `kind`, red when it's negative.
    /// // 1070:270f
    fn put_book(&mut self, world: &World, kind: u8) {
        let value = world.book_value(kind);
        let info = &self.texts.info;
        let text = format_money(value, info.currency_factor, info.land); // 1030:11a0
        self.put(&text, if value < 0 { 7 } else { 4 }, kind);
    }

    /// The pages' values: page 0 the advertising level and its cost, the
    /// renovation interval and the balance (kinds 0x6c to 0x6e), page 1
    /// all of the books' kinds 0x52 to 0x6e. // 1070:270f
    fn draw_values(&mut self, world: &World) {
        match self.page {
            0 => {
                let level = usize::from(world.advertising_level(1));
                let text = Texts::get(&self.texts.advertising, level).to_vec();
                self.put(&text, 5, ADVERTISING_TEXT);
                let cost = self.money(world.advertising_cost(1), b' ');
                self.put(&cost, 4, ADVERTISING_COST);
                // BUG(original): an interval past 5 reads past `RenovierungToText`; the port shows nothing.
                let years = usize::from(world.renovation_years[0]);
                let text = Texts::get(&self.texts.renovation, years).to_vec();
                self.put(&text, 5, RENOVATION_TEXT);
                for kind in BOOK_MONEY..=BOOK_BALANCE {
                    self.put_book(world, kind);
                }
            }
            1 => {
                for kind in FIRST_BOOK..=BOOK_BALANCE {
                    self.put_book(world, kind);
                }
            }
            _ => {}
        }
    }

    /// Page 0's insurance: the check box and its labels shown, erased and
    /// drawn again, the cost (with a `_`, the font's narrow space) worked
    /// out. // 1070:1d1f
    fn draw_widgets(&mut self, world: &World) {
        if self.page != 0 {
            return;
        }
        self.cost.text = self.money(world.insurance_cost(), b'_');
        let Geldmenuform {
            font,
            bitmaps,
            empty,
            bild,
            insurance,
            insurance_label,
            cost_label,
            cost,
            ..
        } = self;
        let skin = Skin { font, bitmaps };
        let background = bitmaps.get(PAGE_BACKGROUNDS[0]).unwrap_or(empty);
        let widgets: [&mut dyn Widget; 4] = [insurance, insurance_label, cost_label, cost];
        for w in widgets {
            w.show(bild, &skin);
            w.frame().erase(bild, background);
            w.draw(bild, &skin);
        }
    }

    /// Pages 2–5: the chart of the history's ring for the page, each
    /// player's line from the last player's to the human's, the scale's
    /// top, middle and bottom, and the dates of its ends: ten years ago (or
    /// January 1996) and now. The original works the lines out in
    /// `Double`s on the FPU; the port uses `f64`, which can put a point a
    /// pixel off where the FPU's 64-bit mantissa rounds differently.
    /// // 1070:16b7
    fn draw_chart(&mut self, world: &World) {
        if !(2..=5).contains(&self.page) {
            return;
        }
        let ring = usize::from(self.page - 2) * RING;
        let h = &world.books.history;
        let longint = |at: usize| {
            h.get(at..at + 4)
                .map_or(0, |b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        };
        let (max, min) = (longint(ring + RING_MAX), longint(ring + RING_MIN));
        let months = i32::from(h.get(ring + RING_MONTHS).copied().unwrap_or(0));
        let start = i32::from(h.get(ring + RING_START).copied().unwrap_or(0));
        let range = (max / 2).wrapping_sub(min / 2);
        if range != 0 {
            let (x0, y0, x1, y1) = self.rect(CHART);
            let (top, bottom) = (f64::from(y0), f64::from(y1 - 1));
            let range = f64::from(range);
            let scale = (top - bottom) / range / 2.0;
            let offset = (f64::from(max) * bottom - f64::from(min) * top) / range / 2.0;
            let width = (x1 - 1) - x0;
            let n = months - 1;
            let y = |month: i32, player: u8| {
                let m = usize::try_from(month.rem_euclid(120)).unwrap_or(0);
                let v = longint((ring + m * 0x20 + usize::from(player) * 4).wrapping_sub(4));
                low_word(round(f64::from(v) * scale + offset)) // 1108:0ea0
            };
            for player in (1..=world.player_count).rev() {
                let colour = world.player_colours.get(usize::from(player) - 1).copied().unwrap_or(0);
                for m in 1..=n {
                    let xa = low_word(x0.wrapping_add((m - 1).wrapping_mul(width) / n));
                    let ya = y(start + m - 1, player);
                    let xb = low_word(x0.wrapping_add(m.wrapping_mul(width) / n));
                    let yb = y(start + m, player);
                    self.line(colour, (xa, ya), (xb, yb)); // 1070:151b
                }
            }
        }
        let start_date = {
            let mut t = self.month_name_ago(world, CHART_MONTHS); // 1030:0263
            t.push(b' ');
            t.extend_from_slice((i32::from(world.date.year) - 10).max(FIRST_YEAR).to_string().as_bytes()); // 1030:02ef
            t
        };
        self.put(&start_date, 2, CHART_START);
        let end_date = {
            let mut t = Texts::get(&self.texts.info.months, usize::from(world.date.month.max(1) - 1)).to_vec(); // 1030:032c
            t.push(b' ');
            t.extend_from_slice(world.date.year.to_string().as_bytes()); // 1030:0357
            t
        };
        self.put(&end_date, 0, CHART_END);
        let middle = max.wrapping_add(min) / 2;
        if self.page <= 3 {
            for (v, n) in [(max, CHART_TOP), (middle, CHART_MIDDLE), (min, CHART_BOTTOM)] {
                let text = self.money(v, b' ');
                self.put(&text, 2, n);
            }
        } else {
            for (v, n) in [(max, CHART_TOP), (middle, CHART_MIDDLE), (min, CHART_BOTTOM)] {
                self.put(v.to_string().as_bytes(), 0, n);
            }
        }
    }

    /// The name of the month `n` months before the date, or the first
    /// month's if the game (from January 1996) is younger. // 1030:0263
    fn month_name_ago(&self, world: &World, n: i32) -> Vec<u8> {
        let d = &world.date;
        let elapsed = (i32::from(d.year) - FIRST_YEAR)
            .wrapping_mul(12)
            .wrapping_add(i32::from(d.month));
        let k = if n >= 0 && elapsed <= n {
            0
        } else {
            usize::try_from((i32::from(d.month) + 119_999 - n).rem_euclid(12)).unwrap_or(0)
        };
        Texts::get(&self.texts.info.months, k).to_vec()
    }

    /// A chart's line from `a` to `b` in the player's `colour`, a 2 × 2
    /// dot of `spieler.bmp` per step along the longer axis; a shallow
    /// line is drawn from left to right only (from `a`), a steep one from
    /// its top. // 1070:151b
    fn line(&mut self, colour: u8, (xa, ya): (i32, i32), (xb, yb): (i32, i32)) {
        if (ya - yb).abs() > (xa - xb).abs() {
            let ((xa, ya), (xb, yb)) = if ya > yb {
                ((xb, yb), (xa, ya))
            } else {
                ((xa, ya), (xb, yb))
            };
            let slope = f64::from(xb - xa) / f64::from(yb - ya); // 1108:055f
            let mut x = f64::from(xa);
            self.dot(colour, xa, ya);
            for y in ya + 1..=yb {
                x += slope;
                self.dot(colour, low_word(round(x)), y);
            }
        } else {
            // BUG(original): a shallow line from right to left draws only its first dot (never in the charts).
            let slope = if xb == xa {
                0.0
            } else {
                f64::from(yb - ya) / f64::from(xb - xa)
            };
            let mut y = f64::from(ya);
            self.dot(colour, xa, ya);
            for x in xa + 1..=xb {
                y += slope;
                self.dot(colour, x, low_word(round(y)));
            }
        }
    }

    /// A 2 × 2 dot of `spieler.bmp`'s `colour` at `x`, `y` on the page.
    /// // 1070:14b4
    fn dot(&mut self, colour: u8, x: i32, y: i32) {
        let src = self.bitmaps.get(SPIELER).unwrap_or(&self.empty);
        let clip = self.bild.bounds();
        let from = Rect::new((i32::from(colour) - 1) * 2, 0, 2, 2);
        self.bild.blit(x, y, src, from, Rop::Copy, clip);
    }

    /// `FUN_1070_20e7`: the page becomes `page`, redrawn (the caller's
    /// [`Geldmenuform::update`]) with the menu bar's tab. // 1070:20e7
    fn set_page(&mut self, page: u8, r: &mut Response) {
        self.page = page;
        self.draw_menu_bar(); // 1070:1f79
        r.update = true;
        r.refresh = true;
    }

    /// Shows the form at `page` (1070:2b11): the button bar's button
    /// 4, the year's end with the option `1110:0064` (page 1). The page
    /// changes if it isn't the form's; then (1070:2b3a) the info box
    /// closes, the form takes the focus, remembers the button bar's
    /// `opener` (`1110:6519`), and opens two pixels short of its full
    /// height with sound 0xd, then 0x25 for the balance sheet or 0xd again.
    /// The world works out this year's total ([`Action::Shown`]): apply the
    /// command, then [`Geldmenuform::update`]. // 1070:2b11, 1070:2b3a
    pub fn show(&mut self, page: u8, opener: u8) -> Response {
        // BUG(original): the year's end shows the form without pressing the button bar's button 4.
        let mut r = Response::default();
        if page != self.page {
            self.set_page(page.min(5), &mut r);
        }
        r.opened = true;
        self.opener = opener;
        self.pressed = PAGE;
        r.commands.push(command::Action::MoneyMenu(Action::Shown(true)));
        r.update = true;
        // BUG(original): the form opens with r0's bottom - 1 as its height, two pixels short of the menu bar's bottom that a drag gives it.
        self.height = self.rect(FORM).3 - 1;
        r.sounds.push(SOUND_OPEN);
        r.sounds
            .push(if self.page == 1 { SOUND_OPEN_BALANCE } else { SOUND_OPEN });
        r.refresh = true;
        r.moved = true;
        r
    }

    /// Hides the form (1070:2bb4): sound 0xe, the button bar's button 4
    /// comes up, height 0, and the main screen gets the focus.
    /// // 1070:2bb4
    pub fn close(&mut self) -> Response {
        self.height = 0;
        Response {
            sounds: vec![SOUND_CLOSE],
            commands: vec![command::Action::MoneyMenu(Action::Shown(false))],
            closed: true,
            moved: true,
            ..Response::default()
        }
    }

    /// Which image `x`, `y` (in the form) is on.
    fn image_at(&self, x: i32, y: i32) -> Option<(Image, i32, i32)> {
        if !self.shown() || y >= self.height {
            return None;
        }
        let [title, bild, menu] = self.layout();
        [(Image::Title, title.1), (Image::Bild, bild.1), (Image::Menu, menu.1)]
            .into_iter()
            .find(|(_, r)| (r.left..r.right).contains(&x) && (r.top..r.bottom).contains(&y))
            .map(|(i, r)| (i, x - r.left, y - r.top))
    }

    fn offset_in(&self, image: Image, x: i32, y: i32) -> (i32, i32) {
        let n = match image {
            Image::Title => TITLE_PLACE,
            Image::Bild => BILD_PLACE,
            Image::Menu => MENU_PLACE,
        };
        let (l, t, _, _) = self.rect(n);
        (x - l, y - t)
    }

    /// A mouse button goes down at `x`, `y` in the form, the left one if
    /// `left`: the image under it takes it (and keeps the mouse until it
    /// comes up). // 1070:0820, 1070:0300, 1070:09f7
    pub fn mouse_down(&mut self, x: i32, y: i32, left: bool, shift: ShiftState) -> Response {
        let Some((image, ix, iy)) = self.image_at(x, y) else {
            return Response::default();
        };
        self.captured = Some(image);
        match image {
            Image::Title => self.title_mouse_down(ix, iy, left, shift),
            Image::Bild => self.bild_mouse_down(ix, iy, left),
            Image::Menu => self.menu_mouse_down(ix, iy, left),
        }
    }

    /// The mouse moves to `x`, `y` in the form, on a `screen` of that
    /// size. // 1070:08e0 (`MenuImageMouseMove`, 1070:0ad5, does nothing)
    pub fn mouse_move(&mut self, x: i32, y: i32, shift: ShiftState, screen: (i32, i32)) -> Response {
        let mut r = Response::default();
        if self.captured == Some(Image::Title) && self.pressed == DRAG && shift.contains(ShiftState::LEFT) {
            let (sx, sy) = (self.left + x, self.top + y);
            self.left = (sx - self.grab.0).max(0).min(screen.0 - self.width());
            self.top = (sy - self.grab.1).max(0).min(screen.1 - self.height);
            r.moved = true;
        }
        r
    }

    /// A mouse button comes up at `x`, `y` in the form, the left one if
    /// `left`; the image the press went down on gets it.
    /// // 1070:096d, 1070:04d3, 1070:0add
    pub fn mouse_up(&mut self, x: i32, y: i32, left: bool) -> Response {
        let Some(image) = self.captured.take() else {
            return Response::default();
        };
        let (ix, iy) = self.offset_in(image, x, y);
        match image {
            Image::Title => self.title_mouse_up(ix, iy, left),
            Image::Bild => self.bild_mouse_up(ix, iy, left),
            Image::Menu => self.menu_mouse_up(ix, iy, left),
        }
    }

    /// `UeberschriftImageMouseDown`: the left button presses the close
    /// button (sound 8); any button on the bar right of it starts a drag,
    /// rolling the form up to its title bar, for good with a double
    /// click. // 1070:0820
    fn title_mouse_down(&mut self, x: i32, y: i32, left: bool, shift: ShiftState) -> Response {
        let mut r = Response::default();
        if left && self.hit(CLOSE, x, y) {
            self.pressed = CLOSE;
            self.draw_down(CLOSE); // 1070:1178
            r.refresh = true;
            r.sounds.push(SOUND_PRESS);
        }
        // BUG(original): the drag starts with any mouse button.
        if self.hit(DRAG, x, y) {
            self.pressed = DRAG;
            let (tl, tt, _, _) = self.rect(TITLE_PLACE);
            self.grab = (x + tl, y + tt);
            self.height = 0x10;
            self.rolled_up = shift.contains(ShiftState::DOUBLE);
            r.moved = true;
        }
        r
    }

    /// `UeberschriftImageMouseUp`: the close button comes up (sound 10)
    /// and closes the form if the mouse is still on it; a drag ends, the
    /// form back at its full height unless it was double-clicked.
    /// // 1070:096d
    fn title_mouse_up(&mut self, x: i32, y: i32, left: bool) -> Response {
        let mut r = Response::default();
        if left && self.pressed == CLOSE {
            self.draw_up(CLOSE); // 1070:1100
            r.refresh = true;
            r.sounds.push(SOUND_RELEASE);
            if self.hit(CLOSE, x, y) {
                let closed = self.close();
                r.sounds.extend(closed.sounds);
                r.commands.extend(closed.commands);
                r.closed = true;
                r.moved = true;
            }
        }
        if self.pressed == DRAG && !self.rolled_up {
            self.height = self.rect(FORM).3 + 1;
            r.moved = true;
        }
        self.pressed = 0;
        r
    }

    /// `BildImageMouseDown`, the left button: the insurance's check box
    /// (toggled with sound 9; its `Click` insures or not, 1070:01fc), or on
    /// page 0 a button goes down (sound 8). // 1070:0300
    fn bild_mouse_down(&mut self, x: i32, y: i32, left: bool) -> Response {
        let mut r = Response::default();
        if !left {
            return r;
        }
        let out = self.insurance.mouse_down(x, y); // 10c0:0797
        if out.handled {
            let Geldmenuform {
                font,
                bitmaps,
                bild,
                insurance,
                ..
            } = self;
            insurance.draw(bild, &Skin { font, bitmaps });
            r.sounds.extend(out.sound.map(Sound::number));
            if out.clicked {
                // 1070:01fc: `1110:0072 := checked`, and the cost label anew.
                r.commands
                    .push(command::Action::MoneyMenu(Action::Insurance(self.insurance.checked())));
                r.update = true;
            }
            r.refresh = true;
            return r;
        }
        if self.page != 0 {
            return r;
        }
        if let Some(&n) = PAGE_BUTTONS.iter().find(|&&n| self.hit(n, x, y)) {
            self.pressed = n;
            self.draw_down(n); // 1070:1178
            r.refresh = true;
            r.sounds.push(SOUND_PRESS);
        }
        r
    }

    /// `BildImageMouseUp`, the left button, on page 0: the button that
    /// went down comes up (sound 10) and does its part if the mouse is
    /// still on it. // 1070:04d3
    fn bild_mouse_up(&mut self, x: i32, y: i32, left: bool) -> Response {
        let mut r = Response::default();
        if !left {
            return r;
        }
        let n = std::mem::take(&mut self.pressed);
        if self.page == 0 && PAGE_BUTTONS.contains(&n) {
            self.draw_up(n); // 1070:1100
            if self.hit(n, x, y) {
                let action = match n {
                    TAKE_LOAN => Action::TakeLoan,
                    REPAY_LOAN => Action::RepayLoan,
                    RENOVATION_LEFT => Action::Renovation { more: false },
                    RENOVATION_RIGHT => Action::Renovation { more: true },
                    ADVERTISING_LEFT => Action::Advertising { more: false },
                    _ => Action::Advertising { more: true },
                };
                r.commands.push(command::Action::MoneyMenu(action));
                r.update = true;
            }
            r.refresh = true;
            r.sounds.push(SOUND_RELEASE);
        }
        r
    }

    /// `MenuImageMouseDown`, the left button: the overview's button, or
    /// "Schließen" goes down (sound 8), or a tab picks its page (sound 9).
    /// // 1070:09f7
    fn menu_mouse_down(&mut self, x: i32, y: i32, left: bool) -> Response {
        let mut r = Response::default();
        if !left {
            return r;
        }
        let out = self.overview.mouse_down(x, y); // 1010:14c1
        if out.handled {
            self.repaint_overview();
            r.sounds.extend(out.sound.map(Sound::number));
            r.refresh = true;
            return r;
        }
        if self.hit(CLOSE_BUTTON, x, y) {
            self.pressed = CLOSE_BUTTON;
            self.draw_down(CLOSE_BUTTON);
            r.refresh = true;
            r.sounds.push(SOUND_PRESS);
            return r;
        }
        for tab in FIRST_TAB..=LAST_TAB {
            let page = tab - FIRST_TAB; // 1110:323e
            if self.hit(tab, x, y) && page != self.page {
                self.set_page(page, &mut r);
                r.sounds.push(SOUND_PAGE);
            }
        }
        r
    }

    /// `MenuImageMouseUp`, the left button: the overview's button comes up
    /// and opens the overview if the mouse is on it (sound 10); else
    /// "Schließen" comes up (sound 10) and closes the form if the mouse
    /// is still on it. // 1070:0add
    fn menu_mouse_up(&mut self, x: i32, y: i32, left: bool) -> Response {
        let mut r = Response::default();
        if !left {
            return r;
        }
        let out = self.overview.mouse_up(x, y); // 1010:14f7
        if out.handled {
            self.repaint_overview();
            r.overview = out.clicked; // 1070:01e8
            r.sounds.extend(out.sound.map(Sound::number));
            r.refresh = true;
        } else if self.pressed == CLOSE_BUTTON {
            self.draw_up(CLOSE_BUTTON);
            r.refresh = true;
            r.sounds.push(SOUND_RELEASE);
            if self.hit(CLOSE_BUTTON, x, y) {
                let closed = self.close();
                r.sounds.extend(closed.sounds);
                r.commands.extend(closed.commands);
                r.closed = true;
                r.moved = true;
            }
        }
        self.pressed = 0;
        r
    }

    fn repaint_overview(&mut self) {
        let Geldmenuform {
            font,
            bitmaps,
            menu,
            overview,
            ..
        } = self;
        overview.draw(menu, &Skin { font, bitmaps });
    }

    /// `FormKeyDown`, without modifiers or with Shift: Enter and Escape
    /// close the form, Space gives the main screen the focus, the arrows
    /// turn the pages round, `1` to `6` pick one; any other key goes to the
    /// main screen. Alt+F4 closes it. // 1070:39bf
    pub fn key_down(&mut self, key: u16, shift: ShiftState) -> Response {
        let mut r = Response::default();
        let mods = shift.0 & (ShiftState::SHIFT.0 | ShiftState::ALT.0 | ShiftState::CTRL.0);
        if mods == 0 || mods == ShiftState::SHIFT.0 {
            let page = usize::from(self.page.min(5));
            match key {
                VK_RETURN | VK_ESCAPE => return self.close(),
                VK_SPACE => r.focus_main = true,
                VK_LEFT => self.set_page(PREVIOUS_PAGE[page], &mut r),
                VK_RIGHT => self.set_page(NEXT_PAGE[page], &mut r),
                // BUG(original): the pages' keys take `1`-`6` from the button bar's while the form has the focus.
                0x31..=0x36 => {
                    let n = u8::try_from(key - 0x31).unwrap_or(0);
                    if n != self.page {
                        self.set_page(n, &mut r);
                    }
                }
                _ => r.main_screen_key = Some((key, shift)),
            }
        } else if mods == ShiftState::ALT.0 && key == VK_F4 {
            return self.close();
        }
        r
    }

    /// The tooltip for a tab under the mouse at `x`, `y` on the screen,
    /// as the main screen's timer shows it (1020:1e9b, with the option
    /// `1110:0060`): while the form is `focused` (or the help is) and not
    /// rolled up, the tab's short text (`SONSTIGE.TXT`) and where the help
    /// goes (its left, top: under the tab). // 1020:1e9b, 1070:0294
    #[must_use]
    pub fn tooltip(&self, x: i32, y: i32, focused: bool) -> Option<(usize, i32, i32)> {
        if !focused || self.rolled_up {
            return None;
        }
        let (ml, mt, _, _) = self.rect(MENU_PLACE);
        let (mx, my) = (x - self.left - ml, y - self.top - mt);
        (FIRST_TAB..=LAST_TAB).find(|&t| self.hit(t, mx, my)).map(|t| {
            let (l, _, _, b) = self.rect(t);
            (
                TAB_TOOLTIPS[usize::from(t - FIRST_TAB)],
                l + ml + self.left,
                b + self.top + mt,
            )
        })
    }
}

fn dim(n: usize) -> i32 {
    i32::try_from(n).unwrap_or(i32::MAX)
}

/// The low word of a `Longint`, as an `Integer` parameter takes it.
fn low_word(n: i32) -> i32 {
    let [a, b, _, _] = n.to_le_bytes();
    i32::from(i16::from_le_bytes([a, b]))
}

/// `Round` (1108:0ea0: `FRNDINT` to nearest even, `FISTP` of a
/// `Longint`); out of range, the FPU's integer indefinite.
#[allow(clippy::cast_possible_truncation)] // Checked against the range first.
fn round(x: f64) -> i32 {
    let r = x.round_ties_even();
    if r.is_nan() || r < f64::from(i32::MIN) || r > f64::from(i32::MAX) {
        i32::MIN
    } else {
        r as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renovation_and_advertising_steps() {
        let mut w = World {
            player_count: 2,
            ..World::default()
        };
        w.renovation_years = [0, 1, 1, 1, 1, 1, 1, 1];
        let mut more = Vec::new();
        for _ in 0..7 {
            w.money_menu_command(1, Action::Renovation { more: true });
            more.push(w.renovation_years[0]);
        }
        assert_eq!(more, [5, 4, 3, 2, 1, 1, 1]);
        let mut less = Vec::new();
        for _ in 0..7 {
            w.money_menu_command(1, Action::Renovation { more: false });
            less.push(w.renovation_years[0]);
        }
        assert_eq!(less, [2, 3, 4, 5, 0, 0, 0]);
        w.player_block = vec![0; crate::world::PLAYER_BLOCK_SIZE];
        w.player_block[1] = 6;
        for _ in 0..3 {
            w.money_menu_command(2, Action::Advertising { more: true });
        }
        assert_eq!(w.advertising_level(2), 7);
        w.money_menu_command(1, Action::Advertising { more: false });
        assert_eq!(w.advertising_level(1), 0);
        // Nobody's.
        w.money_menu_command(0, Action::Advertising { more: true });
        w.money_menu_command(9, Action::Advertising { more: true });
        assert_eq!(w.advertising_level(1), 0);
    }

    #[test]
    fn total_and_balance() {
        let mut w = World {
            player_count: 1,
            ..World::default()
        };
        w.finances.money[0] = 500;
        w.finances.loan[0] = 2000;
        w.book(0x5f, 100);
        w.book(0x6a, -30);
        w.book(0x5e, 7);
        w.money_menu_command(1, Action::Shown(true));
        assert_eq!(w.book_value(0x6b), 70);
        assert_eq!(
            [w.book_value(0x6c), w.book_value(0x6d), w.book_value(0x6e)],
            [500, -2000, -1500]
        );
        // Shown, a booking works the total out again.
        w.book(0x60, 5);
        let tick = Tick {
            books: true,
            ..Tick::default()
        };
        w.geldmenu_refresh(&tick);
        assert_eq!(w.book_value(0x6b), 75);
        // Hidden, it doesn't.
        w.money_menu_command(1, Action::Shown(false));
        w.book(0x60, 5);
        w.geldmenu_refresh(&tick);
        assert_eq!(w.book_value(0x6b), 75);
        // A computer's form doesn't count.
        w.money_menu_command(2, Action::Shown(true));
        assert!(!w.money_menu_shown);
    }

    #[test]
    fn lines() {
        assert_eq!(low_word(0x1_0005), 5);
        assert_eq!(low_word(0xffff), -1);
        assert_eq!(round(2.5), 2);
        assert_eq!(round(3.5), 4);
        assert_eq!(round(-0.5), 0);
        assert_eq!(round(f64::NAN), i32::MIN);
    }
}
