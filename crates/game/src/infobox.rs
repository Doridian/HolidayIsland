//! `TInfobox2`, the pointer tool's info box (segment `1038`): what a
//! square of the island is. A press of the pointer on `Hauptbild` shows
//! it next to the mouse until the button comes up; a double click shows
//! it with buttons and keeps it open, and the other forms are disabled
//! until it closes (1020:1984, 1020:04bc, 1020:1b31, 1020:050b).
//! `docs/infobox.md` has the whole of it.
//!
//! The box is one `TImage`, `Infoimage`, drawn from `INFOBOX.DAT`'s
//! layout and `INFOBOX.TXT`'s texts: for each kind of square
//! (`TIBArt`: hotel, house, restaurant, ..., road, ground, building site,
//! island) the lines it shows (`WelcheZeilen`) and, with buttons, the
//! rows of buttons (`WelcheButtonzeile`). The buttons' world changes
//! (renovating, the rent or price, bribing the builders, selling) are
//! [`Action`]s, carried out by the `World` methods at the end of this
//! module.

// The original's names: `x`, `y`, sizes `w`, `h`.
#![allow(clippy::many_single_char_names)]

use formats::data::{IB_ARTEN, IB_ZEILEN, InfoboxData, InfoboxTexts, Strings, TextAlign};
use vcl::Rect;
use vcl::surface::{Rop, Surface};

use crate::accounts::BOOK_OTHER;
use crate::command::Action;
use crate::construction::{BUILDING_SITE, NOT_BUILDINGS, Record};
use crate::enhance::Fix;
use crate::hauptschirm::InfoTexts;
use crate::money::format_money;
use crate::settlement::{lower_price, lower_rent, raise_rent};
use crate::simulation::{Status, Tick};
use crate::text::{Align, Font};
use crate::widgets::ShiftState;
use crate::world::{BuildingSite, NO_BUILDING, Pos, World, on_map};

/// The bitmaps the box is drawn from, from `GRAFIK`, in the order
/// [`Infobox::new`] takes them. // 1038:1f69
pub const BITMAP_FILES: [&str; 7] = [
    "Infobox.bmp",  // the background and its frame, +0x1de
    "IBButton.bmp", // a button, +0x1be (and +0x1d6)
    "IBButpre.bmp", // held down, +0x1c2 (and +0x1da)
    "IBBlnk.bmp",   // the arrow left, +0x1c6
    "IBBlnkpr.bmp", // +0x1ca
    "IBBrts.bmp",   // the arrow right, +0x1ce
    "IBBrtspr.bmp", // +0x1d2
];
const BACKGROUND: usize = 0;

/// The box's eight button bitmaps (`+0x1be` on, 4 bytes each) as
/// [`BITMAP_FILES`]: 6 and 7 load `IBButton.bmp` and `IBButpre.bmp`
/// again. // 1038:1f69
const BUTTON_BITMAPS: [usize; 8] = [1, 2, 3, 4, 5, 6, 1, 2];

/// Each button's bitmaps up and held down (`+0x25`, `+0x26` of its
/// record): the buttons, the two arrows, the buttons. // 1038:17f3
const BUTTON_LOOKS: [(usize, usize); 9] = [(0, 1), (2, 3), (4, 5), (0, 1), (0, 1), (6, 7), (6, 7), (6, 7), (6, 7)];

/// The buttons (`TIBButton`, `INFOBOX.TXT`'s captions).
pub const RENOVATE: usize = 0;
pub const LOWER: usize = 1;
pub const RAISE: usize = 2;
pub const BRIBE: usize = 3;
pub const SELL: usize = 4;
pub const CANCEL: usize = 5;
pub const OK: usize = 6;
pub const ISLAND_INFO: usize = 7;
pub const OVERVIEW: usize = 8;
const BUTTON_COUNT: usize = 9;

/// The kinds of square (`TIBArt`) past the buildings'.
pub const IB_STRASSE: u8 = 0xb;
pub const IB_GROUND: u8 = 0xc;
pub const IB_BAUSTELLE: u8 = 0xd;
pub const IB_INSEL: u8 = 0xe;

/// A building's kind of square by its `HAUSDATA.DAT` kind (column 5), -1
/// to 11: the building site, house (`IBHaus`), hotel, restaurant, shop,
/// sport, attraction, nature, road, rental, airport, harbour, energy.
/// // 1110:177c (`array[-1..11]`, 1038:1899 reads it from 1110:177d)
const IB_KINDS: [u8; 13] = [IB_BAUSTELLE, 1, 0, 2, 3, 4, 5, 6, IB_STRASSE, 7, 8, 9, 10];

/// The button row (`TIBButtonzeilenart`) the box shows instead of its
/// kind's on a square that's not the human's or nobody's, on the water or
/// off the map: just OK. // 1038:1a75
const ROW_ONLY_OK: usize = 7;
/// The button rows with a text and a value: the rent and the price.
const ROW_RENT: usize = 1;
const ROW_PRICE: usize = 2;

/// The lines (`TIBZeilen`).
const LINE_PRICE: usize = 9;

/// Where the rows start, and the room under them. // 1038:1b92
const TOP: i32 = 3;
const BOTTOM: i32 = 2;
/// The frame's right and bottom edges in `Infobox.bmp`, four pixels each.
/// // 1038:1b92
const RIGHT_EDGE: i32 = 0x11b;
const BOTTOM_EDGE: i32 = 0xcb;
/// How far from the mouse the box goes. // 1038:1ccf
const OFFSET: i32 = 10;

/// The text colour row of everything in the box.
const COLOUR: i32 = 9;

/// The keys `FormKeyDown` knows. // 1038:284b
const VK_RETURN: u16 = 0x0d;
const VK_ESCAPE: u16 = 0x1b;
const VK_F4: u16 = 0x73;

/// Event 9 sells a building at an auction (`Verkaufen`). // 1038:24d6
pub(crate) const SALE_EVENT: usize = 9;

/// `draw_text`'s alignment from `Textzentriert`.
fn align(a: TextAlign) -> Align {
    match a {
        TextAlign::Left => Align::Left,
        TextAlign::Center => Align::Center,
        TextAlign::Right => Align::Right,
    }
}

/// A Pascal `Str` of a number.
fn number(n: i32) -> Vec<u8> {
    n.to_string().into_bytes()
}

/// Appends `b` to the Pascal string `a`, at most 255 characters.
fn concat(mut a: Vec<u8>, b: &[u8]) -> Vec<u8> {
    a.extend_from_slice(b);
    a.truncate(255);
    a
}

/// An amount with two decimals, as the menu prices show: times the
/// currency factor, at least three digits, the decimal point before the
/// last two (`,` in Germany, `.` elsewhere) and the thousands marked (`.`
/// in Germany, `_` in France, `,` elsewhere). // 1030:13da, 1030:108b
#[must_use]
pub fn decimal_money(amount: i32, factor: i32, land: formats::data::Land) -> Vec<u8> {
    use formats::data::Land;
    let mut s = number(amount.wrapping_mul(factor));
    let (point, thousands) = match land {
        Land::Deutschland => (b',', b'.'),
        Land::Frankreich => (b'.', b'_'),
        Land::Amerika | Land::England => (b'.', b','),
    };
    let first = usize::from(s.first() == Some(&b'-'));
    while s.len() - first < 3 {
        s.insert(first, b'0'); // 1030:1089
    }
    // Pascal positions: the point goes in before the last two digits.
    let at = s.len() - 1;
    s.insert(at - 1, point);
    // BUG(original): the thousands marks start five places before the point's position, not four, so an amount of four digits or more before the point is marked one digit off ("12.34567,89").
    let mut at = at.wrapping_sub(5);
    while at >= first + 2 && at < s.len() {
        s.insert(at - 1, thousands);
        at = at.wrapping_sub(3);
    }
    s.truncate(255);
    s
}

/// What `1078:17a0` gathers about the building on a square from its
/// record: for a house its guests and rent, for a facility how full it is
/// and its price.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Figures {
    /// The condition (`+7`). // Self+0x1a6
    pub condition: u8,
    /// The takings a month: guests times the rent, or the price times how
    /// full it is in percent. // Self+0x1a7
    pub takings: i32,
    /// The rent or the price (`+0x12`, `+0x16`). // Self+0x1ab
    pub price: i32,
    /// The guests (`+0x10`), or how full it is (`+0x15`). // Self+0x1af
    pub occupancy: i16,
    /// How well it's connected (`+2`). // Self+0x1a5
    pub connection: u8,
    /// A house's rent rating (`+0x16`). // Self+0x1b1
    pub rent_rating: u8,
}

/// One line of the box as it was drawn: the text before the value, the
/// value and the text after it, each empty where the layout has none.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Line {
    pub label: Vec<u8>,
    pub value: Vec<u8>,
    pub unit: Vec<u8>,
}

/// A button of the box: where its row put it and whether it's shown or
/// held down (`TIBButton`'s `+0x29`, `+0x27`, `+0x28`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct ButtonState {
    y: i32,
    visible: bool,
    pressed: bool,
}

/// What the pointer tool asks of the box ([`crate::tools::Tools::info_box`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Request {
    /// A press: [`Infobox::show`] for `square`, with the mouse at `mouse`
    /// in the main screen's form, with buttons (`modal`) for a double
    /// click. // 1020:04bc
    Show {
        square: Pos,
        mouse: (i32, i32),
        modal: bool,
    },
    /// The button came up after a single press: [`Infobox::hide`].
    /// // 1020:050b
    Hide,
}

/// What the box tells its caller.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Response {
    /// A command for the human (player 1). After applying it, the caller
    /// draws the box again with [`Infobox::redraw`] (1038:1b92), as the
    /// original does right after the change.
    pub action: Option<Action>,
    /// The box closed (1038:1ee0): with buttons, the other forms are
    /// enabled again (`enable_forms`, 1010:29cc); the main screen takes
    /// the focus.
    pub closed: bool,
    /// "Übersicht": the buildings' overview opens (`TUmenuform`,
    /// 1080:0ea8).
    pub open_overview: bool,
    /// The picture or the box's bounds changed.
    pub refresh: bool,
    /// Sounds to play, in order (`play_sound`).
    pub sounds: Vec<u8>,
}

/// The info box's state and picture.
#[derive(Debug, Clone)]
pub struct Infobox {
    data: InfoboxData,
    texts: InfoboxTexts,
    /// `HAUSNAME.TXT`'s long names. // 1110:736a
    long_names: Strings,
    /// `SONSTIGE.TXT`'s texts. // *1110:734e
    sonstige: Strings,
    bitmaps: Vec<Surface>,
    /// `Infoimage`'s picture, as big as the largest box
    /// (`MaxBoxbreite` × `MaxBoxHoehe`): the control shows its top left.
    pub image: Surface,
    /// The box is shown (its height isn't 0). // Self+0x1e6
    pub visible: bool,
    /// With buttons, after a double click: it stays open and the other
    /// forms are disabled (`disable_forms(7)`, 1010:2970). The main
    /// screen's `1110:64b8` as the box was shown.
    pub modal: bool,
    /// The box's bounds on the screen.
    pub left: i32,
    pub top: i32,
    pub width: i32,
    pub height: i32,
    /// The square it shows. // Self+0x1e2, Self+0x1e4
    pub square: Pos,
    /// "Insel-Info" was pressed: the square's plot is shown. // Self+0x180
    pub island_info: bool,
    /// The square's building, the highest corner, the ground and the
    /// kind (`TIBArt`). They keep the last square's where the box doesn't
    /// look them up. // Self+0x1a1 to Self+0x1a4
    building: u8,
    corner: u8,
    ground: u8,
    pub kind: u8,
    figures: Figures,
    /// The building site, a copy of its record. // Self+0x1b2
    site: Option<BuildingSite>,
    /// The building's record. // Self+0x1b6
    record: Option<Record>,
    /// The plot, an index into [`World::plots`]. // Self+0x1ba
    plot: Option<usize>,
    buttons: [ButtonState; BUTTON_COUNT],
    /// `Textzentriert`'s texts: the first of line 9 is overwritten by the
    /// facility's short text each time it's drawn. // Self+0x191
    zeilen: Vec<[Vec<u8>; 2]>,
    /// The lines as they were last drawn, for tests and tooltips.
    pub lines: Vec<Line>,
}

impl Infobox {
    /// The box, hidden (1038:1f69): `data` and `texts` are `INFOBOX.DAT`'s
    /// and `INFOBOX.TXT`'s (`formats::data::infobox_dat`, `infobox_txt`),
    /// `long_names` `HAUSNAME.TXT`'s second list, `sonstige`
    /// `SONSTIGE.TXT`'s texts, `bitmaps` those of [`BITMAP_FILES`].
    ///
    /// # Panics
    /// If there aren't as many bitmaps as files.
    #[must_use]
    pub fn new(
        data: InfoboxData,
        texts: InfoboxTexts,
        long_names: Strings,
        sonstige: Strings,
        bitmaps: Vec<Surface>,
    ) -> Self {
        assert_eq!(bitmaps.len(), BITMAP_FILES.len(), "one bitmap per file");
        let image = Surface::new(
            usize::try_from(data.max_boxbreite).unwrap_or(0),
            usize::try_from(data.max_box_hoehe).unwrap_or(0),
            0,
        );
        let zeilen = texts.zeilen.clone();
        Infobox {
            data,
            texts,
            long_names,
            sonstige,
            bitmaps,
            image,
            visible: false,
            modal: false,
            left: 0,
            top: 0,
            width: 0,
            height: 0,
            square: (0, 0),
            island_info: false,
            building: 0,
            corner: 0,
            ground: 0,
            kind: 0,
            figures: Figures::default(),
            site: None,
            record: None,
            plot: None,
            buttons: [ButtonState::default(); BUTTON_COUNT],
            zeilen,
            lines: Vec::new(),
        }
    }

    /// `HAUSNAME.TXT`'s long name for `building`.
    fn long_name(&self, building: u8) -> Vec<u8> {
        self.long_names.get(usize::from(building)).cloned().unwrap_or_default()
    }

    /// `INFOBOX.TXT`'s short text `n` (`string[25]`, 10 of them; the
    /// eleventh slot is never read in, so empty here).
    fn short_text(&self, n: u8) -> &[u8] {
        self.texts.short_texts.get(usize::from(n)).map_or(&[], |t| t)
    }

    /// The pointer's press: the box for `square` next to the mouse at
    /// `mouse` (in the main screen's form, whose bounds are `main`), with
    /// buttons for a double click. // 1020:04bc, 1038:1eac
    #[allow(clippy::too_many_arguments)]
    pub fn show(
        &mut self,
        world: &World,
        font: &Font,
        info: &InfoTexts,
        square: Pos,
        mouse: (i32, i32),
        main: Rect,
        modal: bool,
    ) {
        self.island_info = false;
        // `1110:64b8`, which the main screen set; with it the other forms
        // are disabled (`disable_forms(7)`), which is the caller's.
        self.modal = modal;
        self.layout(world, font, info, square, (mouse.1, mouse.0), main);
    }

    /// Shows and sizes the box for `square`: next to the mouse at `at`
    /// (`y`, `x` in the main screen's form), or where it is for the
    /// island's info. // 1038:1ccf
    fn layout(&mut self, world: &World, font: &Font, info: &InfoTexts, square: Pos, (y, x): (i32, i32), main: Rect) {
        self.visible = true;
        self.square = square;
        self.draw(world, font, info); // 1038:1b92
        let (w, h) = (self.width, self.height);
        if self.island_info {
            self.left = x;
            self.top = y;
        } else {
            // `ClientWidth`, `ClientHeight` of the borderless main screen.
            self.left = if x + 20 + w < main.width() {
                x + OFFSET + main.left
            } else {
                x - OFFSET - w + main.left
            };
            self.top = if y + 20 + h < main.height() {
                y + OFFSET + main.top
            } else {
                y - OFFSET - h + main.top
            };
        }
    }

    /// The box closes: its height goes to 0. // 1038:1ee0
    pub fn hide(&mut self) -> Response {
        self.visible = false;
        self.height = 0;
        Response {
            closed: true,
            refresh: true,
            ..Response::default()
        }
    }

    /// The box draws itself again for the same square, as after its
    /// buttons' commands; its bounds stay. // 1038:1b92
    pub fn redraw(&mut self, world: &World, font: &Font, info: &InfoTexts) {
        let (w, h) = (self.width, self.height);
        self.draw(world, font, info);
        // BUG(original): after a button's command the box is drawn again but keeps its size.
        self.width = w;
        self.height = h;
    }

    /// The world's tick: a finished building site under the box closes
    /// it. // 1008:525b
    pub fn world_changed(&mut self, world: &World, tick: &Tick) -> Option<Response> {
        let (i, j) = self.square;
        let t = &world.tables;
        let over = tick.finished_sites.iter().any(|s| {
            s.pos.0 <= i
                && s.pos.1 <= j
                && i < t.hausnr(s.building, 3).wrapping_add(s.pos.0)
                && j < t.hausnr(s.building, 4).wrapping_add(s.pos.1)
        });
        (self.visible && over).then(|| self.hide())
    }

    /// What the box looks up for its square: the kind of square and
    /// what it shows of it. // 1038:1899
    fn gather(&mut self, world: &World) {
        let pos = self.square;
        let island = &world.island;
        if !on_map(pos) {
            self.kind = IB_GROUND;
            self.ground = 2;
            self.corner = 0;
            return;
        }
        let plot = island.plot_at(pos);
        if self.island_info && plot != 0 {
            // BUG(original): a plot number without a record is used through a nil pointer; the port shows zeros.
            self.plot = world
                .plots
                .iter()
                .position(|p| p.id() >= 0 && p.id().cast_unsigned() == plot); // 1068:2591
            self.kind = IB_INSEL;
            return;
        }
        self.building = island.buildings.get(pos).unwrap_or(NO_BUILDING);
        self.ground = island.ground.get(pos).unwrap_or(0);
        self.corner = island.highest_corner(pos);
        if NOT_BUILDINGS.contains(&self.building) {
            // Roads, bridges and rivers, or the ground.
            self.kind = if (10..=0x3a).contains(&self.ground) || (0x5f..=100).contains(&self.ground) {
                IB_STRASSE
            } else {
                IB_GROUND
            };
            return;
        }
        let kind = world.tables.hausdata(self.building, 5);
        // BUG(original): a kind past 11 reads the bytes after the table; the port takes it for ground.
        self.kind = usize::try_from(kind.wrapping_add(1))
            .ok()
            .and_then(|k| IB_KINDS.get(k))
            .copied()
            .unwrap_or(IB_GROUND);
        if self.building == BUILDING_SITE {
            self.site = world
                .find_building_site(pos) // 1078:1995
                .and_then(|n| world.building_sites.get(n))
                .copied();
        } else {
            self.figures = world.building_figures(pos); // 1078:17a0
            self.record = world.find_record(pos); // 1078:169a
        }
    }

    /// The record's building type, stars, rent or price.
    fn record_building(&self, world: &World) -> Option<u8> {
        match self.record? {
            Record::House(n) => world.houses.get(n).map(crate::world::House::building),
            Record::Facility(n) => world.facilities.get(n).map(crate::world::Facility::building),
        }
    }

    fn record_stars(&self, world: &World) -> u8 {
        match self.record {
            Some(Record::House(n)) => world.houses.get(n).map_or(0, crate::world::House::stars),
            Some(Record::Facility(n)) => world.facilities.get(n).map_or(0, crate::world::Facility::stars),
            None => 0,
        }
    }

    /// A house's rent (`+0x12`).
    fn record_rent(&self, world: &World) -> i32 {
        match self.record {
            Some(Record::House(n)) => world.houses.get(n).map_or(0, crate::world::House::price),
            _ => 0,
        }
    }

    /// A facility's price (`+0x16`).
    fn record_price(&self, world: &World) -> i32 {
        match self.record {
            Some(Record::Facility(n)) => world.facilities.get(n).map_or(0, crate::world::Facility::price),
            _ => 0,
        }
    }

    /// A facility's price per visit as the box shows it: `price * 100 div
    /// 30 div capacity`, with two decimals. // 1038:014b, 1038:0c00
    fn price_text(&self, world: &World, info: &InfoTexts) -> Vec<u8> {
        let building = self.record_building(world).unwrap_or(0);
        let capacity = i32::from(world.tables.hausdata(building, 3));
        // BUG(original): a type with no capacity divides by zero; the port shows 0.
        let per_visit = (self.record_price(world).wrapping_mul(100) / 30)
            .checked_div(capacity)
            .unwrap_or(0);
        decimal_money(per_visit, info.currency_factor, info.land)
    }

    /// Line `line`'s value. // 1038:014b
    fn value(&mut self, world: &World, info: &InfoTexts, line: usize) -> Vec<u8> {
        let t = &world.tables;
        let b = self.building;
        let money = |n: i32| format_money(n, info.currency_factor, info.land);
        let capacity = i32::from(t.hausdata(b, 3));
        let plot = self.plot.and_then(|p| world.plots.get(p));
        match line {
            0 => {
                let name = self.long_name(b);
                if (0..=2).contains(&t.hausdata(b, 5)) {
                    // BUG(original): a house, hotel or restaurant without a record reads its stars through a nil pointer; the port shows 0.
                    let stars = number(i32::from(self.record_stars(world)));
                    concat(concat(concat(name, b"   "), &stars), b"_*") // 1038:011e, 1038:0122
                } else {
                    name
                }
            }
            1 => {
                if b == BUILDING_SITE {
                    // BUG(original): off the map the building isn't looked up, so the building site of the square shown before names this one.
                    self.long_name(self.site.map_or(NO_BUILDING, |s| s.building))
                } else {
                    ground_text(self.ground, self.corner)
                        .and_then(|n| self.texts.texts.get(n))
                        .cloned()
                        .unwrap_or_default()
                }
            }
            3 => {
                let owner = world.island.owners.get(self.square).unwrap_or(0);
                world.player_names.get(usize::from(owner)).cloned().unwrap_or_default()
            }
            4 => money(self.figures.takings),
            5 => money(t.hausprei(b, 1)),
            6 => concat(
                concat(number(i32::from(self.figures.occupancy)), b"/"), // 1038:0125
                &number(capacity),
            ),
            7 => {
                let visitors = i32::from(self.figures.occupancy).wrapping_mul(capacity) / 100;
                concat(concat(number(visitors), b"/"), &number(capacity))
            }
            8 => money(self.record_rent(world) / 4),
            LINE_PRICE => {
                let class = self.record_building(world).map_or(10, |b| world.facility_class(b));
                let text = self.short_text(class).to_vec();
                if let Some(z) = self.zeilen.get_mut(LINE_PRICE) {
                    z[0] = text;
                }
                self.price_text(world, info)
            }
            10 => number(i32::from(self.figures.condition) * 100 / 0x7f),
            11 => {
                let days = self.site.map_or(0, |s| s.days);
                let text = |n: usize| self.sonstige.get(n).cloned().unwrap_or_default();
                let s = concat(text(0x13), b" "); // 1038:0127
                let s = concat(concat(s, &number(i32::from(days))), b" ");
                concat(s, &text(0x14))
            }
            12 => number(i32::from(self.corner) * 3),
            14 => number(i32::from(plot.map_or(0, crate::world::Plot::quality))),
            15 => number(plot.map_or(0, |p| p.longint(0xee))),
            16 => number(i32::from(plot.map_or(0, crate::world::Plot::area))),
            _ => Vec::new(),
        }
    }

    /// Draws line `line` at the current height and moves down.
    /// // 1038:014b
    fn draw_line(&mut self, world: &World, font: &Font, info: &InfoTexts, line: usize, y: &mut i32, width: &mut i32) {
        let value = self.value(world, info, line);
        let lage = self.data.zeilen_lage[line];
        let [a_label, a_unit, a_value] = self.data.textzentriert[line];
        let [label, unit] = self.zeilen.get(line).cloned().unwrap_or_default();
        let lage = |n: usize| i32::from(lage[n]);
        let mut shown = Line::default();
        if lage(2) != 0 {
            font.draw(
                &mut self.image,
                &label,
                (lage(1), *y),
                lage(2) - lage(1),
                align(a_label),
                COLOUR,
            );
            shown.label = label;
        }
        if lage(4) != 0 {
            font.draw(
                &mut self.image,
                &value,
                (lage(3), *y),
                lage(4) - lage(3),
                align(a_value),
                COLOUR,
            );
            shown.value = value;
        }
        if lage(6) != 0 {
            font.draw(
                &mut self.image,
                &unit,
                (lage(5), *y),
                lage(6) - lage(5),
                align(a_unit),
                COLOUR,
            );
            shown.unit = unit;
        }
        self.lines.push(shown);
        *y += lage(0);
        *width = (*width).max(lage(8));
    }

    /// Button `b` up (1038:098d) or held down (1038:0a44), with its caption.
    fn draw_button(&mut self, font: &Font, b: usize, pressed: bool) {
        let [x, w, h] = self.data.ib_button[b].map(i32::from);
        let y = self.buttons[b].y;
        let (up, down) = BUTTON_LOOKS[b];
        let look = BUTTON_BITMAPS[if pressed { down } else { up }];
        let all = self.image.bounds();
        self.image
            .blit(x, y, &self.bitmaps[look], Rect::new(0, 0, w, h), Rop::Copy, all);
        let caption = self.texts.buttons.get(b).cloned().unwrap_or_default();
        let shift = i32::from(pressed);
        font.draw(
            &mut self.image,
            &caption,
            (x + shift, y + 2 + shift),
            w,
            Align::Center,
            COLOUR,
        );
    }

    /// A row of buttons at the current height, its text and its value,
    /// and moves down. // 1038:0d3a, 1038:0afe, 1038:0b40, 1038:0c00
    fn draw_button_row(
        &mut self,
        world: &World,
        font: &Font,
        info: &InfoTexts,
        row: usize,
        y: &mut i32,
        width: &mut i32,
    ) {
        let Some(&zeile) = self.data.ib_buttonzeile.get(row) else {
            return;
        };
        for b in 0..BUTTON_COUNT {
            if self.data.welche_button.get(row).is_some_and(|r| r[b]) {
                self.buttons[b] = ButtonState {
                    y: *y,
                    visible: true,
                    pressed: false,
                };
                self.draw_button(font, b, false);
            }
        }
        if zeile.mit_text {
            let text = if row == ROW_PRICE {
                let class = self.record_building(world).map_or(10, |b| world.facility_class(b));
                self.short_text(class).to_vec()
            } else {
                self.texts.buttonzeilen.get(row).cloned().unwrap_or_default()
            };
            let (x, w) = (i32::from(zeile.textlinks), i32::from(zeile.textbreite));
            font.draw(&mut self.image, &text, (x, *y), w, Align::Left, COLOUR);
        }
        if zeile.mit_wert && (row == ROW_RENT || row == ROW_PRICE) {
            let amount = if row == ROW_RENT {
                format_money(self.record_rent(world) / 4, info.currency_factor, info.land)
            } else {
                self.price_text(world, info)
            };
            let text = concat(concat(amount, b" "), &info.currency); // 1038:0bfe
            let (x, w) = (i32::from(zeile.wertlinks), i32::from(zeile.wertbreite));
            font.draw(&mut self.image, &text, (x, *y), w, Align::Center, COLOUR);
        }
        *y += i32::from(zeile.hoehe);
        *width = (*width).max(i32::from(zeile.breite));
    }

    /// The lines of the square's kind and, with buttons, its rows of
    /// buttons. // 1038:1a75
    fn draw_rows(&mut self, world: &World, font: &Font, info: &InfoTexts, y: &mut i32, width: &mut i32) {
        for b in &mut self.buttons {
            b.visible = false;
        }
        self.lines.clear();
        let kind = usize::from(self.kind);
        for line in 0..IB_ZEILEN {
            if self.data.welche_zeilen.get(kind).is_some_and(|r| r[line]) {
                self.draw_line(world, font, info, line, y, width);
            }
        }
        if !self.modal {
            return;
        }
        let pos = self.square;
        let island = &world.island;
        // Single human: the buttons are for player 1's squares and nobody's.
        // BUG(original): a building of nobody's (owner 0) gets the buttons too: the human renovates, prices or sells it.
        let lowest = u8::from(world.fixes.has(Fix::NobodysBuildings));
        let owner = island.owners.get(pos).unwrap_or(0);
        let theirs = on_map(pos) && (lowest..2).contains(&owner) && island.ground.get(pos) != Some(2);
        if theirs {
            for row in 0..8 {
                if kind < IB_ARTEN && self.data.welche_buttonzeile[kind][row] {
                    self.draw_button_row(world, font, info, row, y, width);
                }
            }
        } else {
            self.draw_button_row(world, font, info, ROW_ONLY_OK, y, width);
        }
    }

    /// Draws the box for its square: the background, the rows, the frame's
    /// right and bottom edges where the box ends. // 1038:1b92
    fn draw(&mut self, world: &World, font: &Font, info: &InfoTexts) {
        let all = self.image.bounds();
        let (mw, mh) = (i32::from(self.data.max_boxbreite), i32::from(self.data.max_box_hoehe));
        self.image
            .blit(0, 0, &self.bitmaps[BACKGROUND], Rect::new(0, 0, mw, mh), Rop::Copy, all);
        self.gather(world);
        let (mut y, mut width) = (TOP, 0);
        self.draw_rows(world, font, info, &mut y, &mut width);
        y += BOTTOM;
        let bg = &self.bitmaps[BACKGROUND];
        self.image
            .blit(width - 3, 0, bg, Rect::new(RIGHT_EDGE, 0, 4, y), Rop::Copy, all);
        self.image
            .blit(0, y - 3, bg, Rect::new(0, BOTTOM_EDGE, width - 1, 4), Rop::Copy, all);
        self.width = width;
        self.height = y;
    }

    /// Whether `x`, `y` (in the box) is on a shown button `b`, edges
    /// included. // 1038:2478
    fn hit(&self, b: usize, x: i32, y: i32) -> bool {
        let [left, w, h] = self.data.ib_button[b].map(i32::from);
        let top = self.buttons[b].y;
        !(x < left || y < top || left + w < x || top + h < y)
    }

    /// Where button `b` is in the box, if it's shown.
    #[must_use]
    pub fn button(&self, b: usize) -> Option<Rect> {
        let [left, w, h] = self.data.ib_button.get(b)?.map(i32::from);
        let s = self.buttons.get(b)?;
        s.visible.then(|| Rect::new(left, s.y, w, h))
    }

    /// `InfoimageMouseDown` at `x`, `y` in the box, any button: the shown
    /// buttons under it go down, with sound 8. // 1038:2742
    pub fn mouse_down(&mut self, x: i32, y: i32, font: &Font) -> Response {
        let mut r = Response::default();
        for b in 0..BUTTON_COUNT {
            if self.buttons[b].visible && self.hit(b, x, y) {
                self.draw_button(font, b, true);
                r.refresh = true;
                r.sounds.push(8);
                self.buttons[b].pressed = true;
            }
        }
        r
    }

    /// `InfoimageMouseUp`: the buttons held down come up, with sound 10,
    /// and the one under the mouse does its work. // 1038:27b4
    pub fn mouse_up(&mut self, x: i32, y: i32, world: &World, font: &Font, info: &InfoTexts, main: Rect) -> Response {
        let mut r = Response::default();
        for b in 0..BUTTON_COUNT {
            if self.buttons[b].visible && self.buttons[b].pressed {
                self.draw_button(font, b, false);
                r.refresh = true;
                r.sounds.push(10);
                if self.hit(b, x, y) {
                    self.click(b, world, font, info, main, &mut r);
                }
                self.buttons[b].pressed = false;
            }
        }
        r
    }

    /// A button's work. // 1038:24d6
    fn click(&mut self, b: usize, world: &World, font: &Font, info: &InfoTexts, main: Rect, r: &mut Response) {
        let pos = self.square;
        match b {
            // The record's square in the original; the world finds it
            // again from this one.
            RENOVATE => r.action = Some(Action::Renovate(pos)), // 1010:1931
            LOWER | RAISE => {
                r.action = Some(Action::Price {
                    at: pos,
                    raise: b == RAISE,
                });
            }
            BRIBE => r.action = Some(Action::Bribe(pos)), // 1010:19f1
            SELL => {
                // Nothing while a sale is on.
                if world.events.get(SALE_EVENT).is_none_or(|e| e.running == 0) {
                    r.action = Some(Action::Sell(pos));
                }
            }
            CANCEL | OK => {
                let closed = self.hide();
                r.closed = closed.closed;
            }
            ISLAND_INFO => {
                // 1038:1e83
                self.island_info = true;
                let at = (self.top, self.left);
                self.layout(world, font, info, pos, at, main);
            }
            _ => {
                // 1038:1ee0, 1080:0ea8
                let closed = self.hide();
                r.closed = closed.closed;
                r.open_overview = true;
            }
        }
    }

    /// `FormKeyDown`: Enter and Escape (alone or with Shift), and Alt+F4,
    /// close the box. The original first hands the keys to the TV
    /// (`TFernseher`, 1048:26f6) when it's shown; that's the caller's.
    /// Whether the key was taken (Alt+F4 is swallowed). // 1038:284b
    pub fn key_down(&mut self, key: u16, shift: ShiftState) -> Option<Response> {
        let plain = shift.0 == 0 || shift == ShiftState::SHIFT;
        if (plain && (key == VK_RETURN || key == VK_ESCAPE)) || (shift == ShiftState::ALT && key == VK_F4) {
            Some(self.hide())
        } else {
            None
        }
    }

    /// The box's bounds on the screen.
    #[must_use]
    pub fn bounds(&self) -> Rect {
        Rect::new(self.left, self.top, self.width, self.height)
    }

    /// `Infoimage`'s bounds in the form, and its picture.
    #[must_use]
    pub fn images(&self) -> [(&'static str, Rect, &Surface); 1] {
        [("Infoimage", Rect::new(0, 0, self.width, self.height), &self.image)]
    }

    /// Whether `x`, `y` (on the screen) is on the box as it's shown.
    #[must_use]
    pub fn contains(&self, x: i32, y: i32) -> bool {
        self.visible
            && (self.left..self.left + self.width).contains(&x)
            && (self.top..self.top + self.height).contains(&y)
    }
}

/// `INFOBOX.TXT`'s description of a square's ground: the ocean (in the
/// sea) or the shore for water, grass, sand, grass and sand, a road on
/// sand, on grass, on both, a bridge, a river; none for the rest.
/// // 1038:014b
fn ground_text(ground: u8, corner: u8) -> Option<usize> {
    match ground {
        2 => Some(usize::from(corner != 0)),
        1 => Some(2),
        0 => Some(3),
        4 => Some(4),
        10..=0x18 => Some(5),
        0x19..=0x27 => Some(6),
        0x28..=0x36 => Some(7),
        0x37..=0x3a | 0x5f..=100 => Some(8),
        0x3b..=0x5e => Some(9),
        _ => None,
    }
}

impl World {
    /// What the box shows of the building on `pos`, from its record by
    /// its first square. // 1078:17a0
    #[must_use]
    pub fn building_figures(&self, pos: Pos) -> Figures {
        let mut f = Figures::default();
        match self.find_record(pos) {
            Some(Record::House(n)) => {
                let h = &self.houses[n];
                f.condition = h.condition();
                f.takings = i32::from(h.guests()).wrapping_mul(h.price());
                f.price = h.price();
                f.occupancy = h.guests();
                f.connection = h.connection();
                f.rent_rating = h.rent_rating();
            }
            Some(Record::Facility(n)) => {
                let r = &self.facilities[n];
                f.condition = r.condition();
                f.takings = i32::from(r.fill()).wrapping_mul(r.price()) / 100;
                f.price = r.price();
                f.occupancy = i16::from(r.fill());
                f.connection = r.connection();
            }
            None => {}
        }
        f
    }

    /// Whether `player` may change the building on `pos`: the info box
    /// offers the buttons on the human's squares and nobody's
    /// (1038:1a75), the overview lists only the human's (1080:02d9).
    /// With `fix.nobodys_buildings` only their own.
    fn may_change(&self, player: u8, pos: Pos) -> bool {
        let owner = self.island.owners.get(pos).unwrap_or(0);
        // BUG(original): nobody's buildings (owner 0) get the buttons too.
        (owner == 0 && !self.fixes.has(Fix::NobodysBuildings)) || owner == player
    }

    /// "Renovieren": `player` renovates the building on `pos` if they
    /// have the money (else error 0xe for player 1); its condition is as
    /// new. Single human: the books and the money rising are player 1's.
    /// // 1010:1931
    pub(crate) fn renovate_building(&mut self, player: u8, pos: Pos, tick: &mut Tick) {
        if !self.may_change(player, pos) {
            return;
        }
        let (building, condition, at) = match self.find_record(pos) {
            Some(Record::House(n)) => {
                let h = &self.houses[n];
                (h.building(), h.condition(), h.pos())
            }
            Some(Record::Facility(n)) => {
                let f = &self.facilities[n];
                (f.building(), f.condition(), f.pos())
            }
            None => return,
        };
        let price = self.renovation_price(building, condition); // 1008:0660
        if self.money_of(player) < price {
            if player == 1 {
                tick.status.push((player, Status::Error(0xe))); // 1030:232d
            }
            return;
        }
        self.add_money(player, price.wrapping_neg()); // 1078:01e7
        if player == 1 {
            self.book(BOOK_OTHER, price.wrapping_neg()); // 1070:1419
            tick.books = true;
            self.float_money(price, at); // 1088:0153
        }
        match self.find_record(pos) {
            Some(Record::House(n)) => self.houses[n].set_condition(0x7f),
            Some(Record::Facility(n)) => self.facilities[n].set_condition(0x7f),
            None => {}
        }
    }

    /// The minus and plus buttons: the rent of a house a step down or up
    /// (1010:1e05, 1010:1d62), or the price of a facility (1010:1c86,
    /// 1010:1ae2). // 1038:24d6, 1080:01b2, 1080:0235
    pub(crate) fn step_price(&mut self, player: u8, pos: Pos, raise: bool) {
        if !self.may_change(player, pos) {
            return;
        }
        match self.find_record(pos) {
            Some(Record::House(n)) => {
                let rent = self.houses[n].price();
                let rent = if raise { raise_rent(rent) } else { lower_rent(rent) };
                self.houses[n].set_price(rent);
            }
            Some(Record::Facility(n)) => {
                let f = &self.facilities[n];
                let price = if raise {
                    self.raise_price(f.building(), f.price())
                } else {
                    lower_price(f.price())
                };
                self.facilities[n].set_price(price);
            }
            None => {}
        }
    }

    /// "Bestechen": for 5000 (else error 0xe for player 1) the builders of
    /// the site on `pos` hurry: its days left go down to half of those
    /// past a quarter of the building time, plus that quarter, but by at
    /// most ten. Single human: the books and the money rising are player
    /// 1's. // 1010:19f1
    pub(crate) fn bribe_builders(&mut self, player: u8, pos: Pos, tick: &mut Tick) {
        if !self.may_change(player, pos) {
            return;
        }
        let Some(n) = self.find_building_site(pos) else {
            return;
        };
        if self.money_of(player) < 5000 {
            if player == 1 {
                tick.status.push((player, Status::Error(0xe))); // 1030:232d
            }
            return;
        }
        let site = &mut self.building_sites[n];
        let days = i32::from(site.days);
        let quarter = i32::from(self.tables.hausdata(site.building, 0) / 4);
        let half = ((days - quarter) / 2).max(0); // 1030:0ee7
        let days = (half + quarter).max(days - 10).min(days);
        site.days = i16::try_from(days).unwrap_or(site.days);
        let at = site.pos;
        self.add_money(player, -5000); // 1078:01e7
        if player == 1 {
            self.book(BOOK_OTHER, -5000); // 1070:1419
            tick.books = true;
            self.float_money(5000, at); // 1088:0153
        }
    }

    /// "Verkaufen": unless a sale is on, the building on `pos` goes to
    /// auction: event 9 becomes certain with the price, twice
    /// `HAUSPREI.DAT` column 2 for a type of price 10 or less, else a
    /// quarter of the price, for the seller `player`. // 1038:24d6
    pub(crate) fn sell_building(&mut self, player: u8, pos: Pos) {
        if !self.may_change(player, pos) || self.events.get(SALE_EVENT).is_some_and(|e| e.running != 0) {
            return;
        }
        let at = match self.find_record(pos) {
            Some(Record::House(n)) => self.houses[n].pos(),
            Some(Record::Facility(n)) => self.facilities[n].pos(),
            // BUG(original): a building without a record is sold from a nil pointer's square; the port sells nothing.
            None => return,
        };
        let b = self.island.buildings.get(pos).unwrap_or(NO_BUILDING);
        let price = self.tables.hausprei(b, 0);
        let value = if price <= 10 {
            self.tables.hausprei(b, 2).wrapping_mul(2)
        } else {
            price / 4
        };
        self.schedule_event(SALE_EVENT, value, i32::from(player), at); // 1010:3006
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use formats::data::Land;

    #[test]
    fn decimals() {
        let de = Land::Deutschland;
        assert_eq!(decimal_money(5, 10, de), b"0,50");
        assert_eq!(decimal_money(0, 10, de), b"0,00");
        assert_eq!(decimal_money(-1, 1, de), b"-0,01");
        assert_eq!(decimal_money(123, 10, de), b"12,30");
        assert_eq!(decimal_money(1234, 10, de), b"123,40");
        // The thousands mark one place off.
        assert_eq!(decimal_money(12_345, 10, de), b"1234,50");
        assert_eq!(decimal_money(1_234_567, 100, de), b"12.34567,00");
        assert_eq!(decimal_money(500, 1, Land::England), b"5.00");
    }

    #[test]
    fn fix_nobodys_buildings() {
        for fixed in [false, true] {
            let mut w = World {
                tables: std::sync::Arc::new(crate::world::Tables {
                    hausnr: vec![[1, 0, 0, 1, 1, 1, 0, 0]; 0x80],
                    hausdata: vec![[0; 10]; 0x80],
                    ..crate::world::Tables::default()
                }),
                ..World::default()
            };
            w.fixes.set(Fix::NobodysBuildings, fixed);
            w.island.buildings.set((20, 20), 40);
            let mut house = crate::world::House::new((20, 20));
            house.set_building(40);
            house.set_price(100);
            w.houses = vec![house];
            // Nobody's house: the original lets player 1 set its rent.
            w.step_price(1, (20, 20), true);
            assert_eq!(w.houses[0].price() == 100, fixed);
            // Their own, either way.
            w.island.owners.set((20, 20), 1);
            let rent = w.houses[0].price();
            w.step_price(1, (20, 20), true);
            assert_ne!(w.houses[0].price(), rent);
        }
    }

    #[test]
    fn grounds() {
        assert_eq!(ground_text(2, 0), Some(0));
        assert_eq!(ground_text(2, 1), Some(1));
        assert_eq!(ground_text(0x20, 1), Some(6));
        assert_eq!(ground_text(3, 1), None);
    }
}
