//! `TBaumenue`, the build menu (segment `1018`): the form a build tool's
//! button opens on the right of the screen, with the submenu's list of
//! building types, the selected type's picture, name and figures, the
//! buttons to scroll the list, to turn the building (its other looks),
//! "Schließen" and "Bauen", and a title bar to drag it by. Which types
//! the submenus offer is the world's ([`crate::build_menu`]); this is the
//! form, the selection and the scrolling, which the original saves with
//! the map too (unit 1018:34a0).
//!
//! The form is one `TImage`, `Baumenuebild`, the game draws into itself
//! from `BAUMENU.DAT`'s two layouts of 32 rectangles each (the first for
//! most submenus, the second, bigger one for hotels, leisure and
//! airports). The form is hidden by giving it no height.

// The original's names: `x`, `y`, the rectangle `r`.
#![allow(clippy::many_single_char_names)]

use formats::data::{BaumenuData, BaumenuTexts, Strings};
use vcl::Rect;
use vcl::surface::{Rop, Surface};

use crate::build_menu::{FIRST_MENU, MENUS, NONE};
use crate::darstellung::{self, Tiles};
use crate::enhance::{Enhancement, Enhancements};
use crate::hauptschirm::InfoTexts;
use crate::money::format_money;
use crate::savegame::Fields;
use crate::text::{Align, Font};
use crate::widgets::ShiftState;
use crate::world::World;

/// The bitmaps the form is drawn from, from `GRAFIK`, in the order
/// [`Baumenue::new`] takes them: `+0x188` to `+0x1a8` (1018:2d12) and
/// the close button held down (`1110:54bc`).
pub const BITMAP_FILES: [&str; 10] = [
    "backgras.BMP", // the picture's ground for buildings on grass, +0x188
    "backsand.BMP", // for those on sand, +0x18c
    "bauwin1.BMP",  // layout 1, +0x190
    "bauwin2.BMP",  // layout 2, +0x194
    "bmbutpre.BMP", // a button held down, +0x198
    "bmdrehbu.BMP", // the rotate button, +0x19c
    "bmdrbupr.BMP", // held down, +0x1a0
    "bmuppres.BMP", // the up arrow held down, +0x1a4
    "bmdownpr.BMP", // the down arrow held down, +0x1a8
    "closepre.bmp", // 1110:54bc
];
const BACKGRAS: usize = 0;
const BACKSAND: usize = 1;
const BAUWIN: [usize; 2] = [2, 3];
const BUTTON_PRESSED: usize = 4;
const ROTATE: usize = 5;
const ROTATE_PRESSED: usize = 6;
const UP_PRESSED: usize = 7;
const DOWN_PRESSED: usize = 8;
const CLOSE_PRESSED: usize = 9;

/// The layouts' rectangles (`BAUMENU.DAT`, each `x1, y1, x2, y2`,
/// inclusive) the form uses.
mod r {
    /// The whole form.
    pub const FORM: u8 = 0;
    pub const CLOSE: u8 = 1;
    /// The title bar to drag the form by.
    pub const TITLE: u8 = 2;
    pub const HEADING: u8 = 3;
    /// The up arrow's picture, and where it's clicked.
    pub const UP_PICTURE: u8 = 4;
    pub const UP: u8 = 5;
    pub const DOWN_PICTURE: u8 = 6;
    pub const DOWN: u8 = 7;
    /// The list of the submenu's types, rows of 15 pixels.
    pub const LIST: u8 = 8;
    /// The building's picture.
    pub const PICTURE: u8 = 9;
    /// The rotate button, for a type with other looks.
    pub const ROTATE: u8 = 10;
    pub const NAME: u8 = 0xb;
    /// "Schließen" ([`CANCEL`]) and "Bauen" ([`BUILD`]): the button's
    /// picture and, inside it, where it's clicked and its text drawn
    /// pressed.
    pub const CANCEL_PICTURE: u8 = 0x1c;
    pub const CANCEL: u8 = 0x1d;
    pub const BUILD_PICTURE: u8 = 0x1e;
    pub const BUILD: u8 = 0x1f;
}

/// The rows the list shows in each layout. // 1110:14ca
const VISIBLE_ROWS: [i16; 2] = [7, 11];
/// A row's height in the list.
const ROW_HEIGHT: i16 = 15;
/// The form's height while it's dragged, and after a double click on its
/// title bar: just the title. // 1018:0208
const TITLE_HEIGHT: i32 = 16;

/// The submenus that use the second layout: hotels, leisure, airports.
/// // 1018:2b01
const BIG_MENUS: [u8; 3] = [10, 0x12, 0xe];

/// The submenu to the left and right of each (9..=20), for the arrow keys:
/// the button bar's order, round. // 1110:14ce, 1110:14da
/// The later build's scenario 12 and the submenu whose first row it
/// shows (`*(+0x1ed) + 10`). // en 1018:015f
const HURRY_UP: u8 = 12;
const HURRY_UP_SUBMENU: usize = 5;

const PREVIOUS: [u8; 12] = [0x14, 9, 0xa, 0xb, 0xc, 0xd, 0xe, 0xf, 0xf, 0x11, 0x12, 0x13];
const NEXT: [u8; 12] = [0xa, 0xb, 0xc, 0xd, 0xe, 0xf, 0x11, 0x11, 0x12, 0x13, 0x14, 9];

/// The keys the build menu passes on to the main screen: F1 to F12, `1`
/// to `9` and Pause. // 1018:3290
fn main_screen_key(key: u16) -> bool {
    (0x70..=0x7b).contains(&key) || (0x31..=0x39).contains(&key) || key == 0x13
}

/// What a click or key on the build menu does besides drawing it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Response {
    Nothing,
    /// The form closes, the tool stays (2c71).
    Close,
    /// The pointer tool takes over from the submenu's and the form closes
    /// (1010:2bc4, 1018:2c71).
    Cancel,
    /// Button `n` of the button bar is clicked (1010:2c8b): the arrow keys'
    /// next submenu.
    Button(u8),
    /// The key goes to the main screen's `FormKeyDown` (1020:2e09).
    MainScreenKey(u16),
    /// The main screen gets the focus (Space).
    FocusMainScreen,
}

/// The build menu's texts and pictures, and its state.
#[derive(Debug, Clone)]
pub struct Baumenue {
    texts: BaumenuTexts,
    layouts: [Vec<[i16; 4]>; 2],
    /// `HAUSNAME.TXT`'s lists: the short names for the list, the long
    /// ones for the name. // 1110:7366, 1110:736a
    short_names: Strings,
    long_names: Strings,
    /// `AUSWAHL.TXT`. // 1110:7362
    auswahl: Strings,
    bitmaps: Vec<Surface>,
    /// `Baumenuebild`.
    pub image: Surface,
    /// The submenu shown: the tool that opened it. // +0x1ac
    pub menu: u8,
    /// Its button. // +0x1ad
    pub button: u8,
    /// Each submenu's selected row. // +0x19c (+0x1ae for 9)
    selected: [i16; MENUS],
    /// Each submenu's look picked with the rotate button, 0–3. // +0x1d5
    looks: [u8; MENUS],
    /// The selected row has other looks. // +0x1ea
    has_looks: bool,
    /// The rectangle the mouse went down on, 0 for none. // +0x1eb
    pressed: u8,
    /// The layout, 1 or 2. // +0x1ec
    layout: u8,
    /// Each submenu's first row shown. // *+0x1ed
    scroll: [i16; MENUS],
    /// The press on the title bar was a double click. // +0x1f9
    double: bool,
    /// Open: the button bar's buttons reopen it when it's closed.
    /// // +0x1fe
    pub open: bool,
    /// The building type the build tool builds, from the selected row and
    /// look. // 1110:6058
    pub building: i16,
    /// The form's place and height (0 while hidden), and its width.
    pub left: i32,
    pub top: i32,
    pub height: i32,
    pub width: i32,
    /// Where on the form the title bar was grabbed. // 1110:6046, 6048
    grab: (i32, i32),
    /// Sounds to play, in order (`play_sound`).
    pub sounds: Vec<u8>,
    /// The player's view enhancements (`build_menu`, `form_positions`).
    pub enhance: Enhancements,
}

impl Baumenue {
    /// The form as `Application.CreateForm` and 1018:2fa2 leave it for a
    /// `width` × `height` screen: hidden, at the second layout, every
    /// submenu at its first row. `bitmaps` are those of
    /// [`BITMAP_FILES`]; `names` `HAUSNAME.TXT`'s lists. // 1018:2fa2
    ///
    /// # Panics
    /// If there aren't as many bitmaps as files.
    #[must_use]
    pub fn new(
        texts: BaumenuTexts,
        data: &BaumenuData,
        names: (Strings, Strings),
        auswahl: Strings,
        bitmaps: Vec<Surface>,
        (width, height): (i32, i32),
    ) -> Self {
        assert_eq!(bitmaps.len(), BITMAP_FILES.len(), "one bitmap per file");
        let mut m = Baumenue {
            texts,
            layouts: data.layouts.clone(),
            short_names: names.0,
            long_names: names.1,
            auswahl,
            bitmaps,
            image: Surface::new(1, 1, 0),
            menu: 0,
            button: 0,
            selected: [0; MENUS],
            looks: [0; MENUS],
            has_looks: false,
            pressed: 0,
            layout: 2,
            scroll: [0; MENUS],
            double: false,
            open: false,
            building: 0,
            left: 0,
            top: 0,
            height: 0,
            // The DFM's `ClientWidth`, until a submenu opens.
            width: 410,
            grab: (0, 0),
            sounds: Vec::new(),
            enhance: Enhancements::ORIGINAL,
        };
        m.lay_out();
        m.left = width - m.width - 1;
        m.top = m.first_top(height);
        m
    }

    /// The menu's first top on a screen `height` high; with
    /// `form_positions` level with the island's top. // 1018:2fa2
    fn first_top(&self, height: i32) -> i32 {
        if self.enhance.has(Enhancement::FormPositions) {
            crate::hauptschirm::Hauptschirm::position(0, height).1 + 0x3b
        } else {
            // BUG(original): 480, not the main screen's 479 with each half divided, puts the menu one row above the island.
            (height - 480) / 2 + 0x3b
        }
    }

    /// The player's view enhancements, on a screen `height` high: a menu
    /// still where it was made goes where they put it.
    pub fn set_enhance(&mut self, enhance: Enhancements, height: i32) {
        let first = self.top == self.first_top(height);
        self.enhance = enhance;
        if first {
            self.top = self.first_top(height);
        }
    }

    /// A building type's short name (`HAUSNAME.TXT`'s first list), as the
    /// list and the status bar show it.
    #[must_use]
    pub fn short_name(&self, building: u8) -> Vec<u8> {
        self.short_names.get(usize::from(building)).cloned().unwrap_or_default()
    }

    /// Rectangle `n` of the layout.
    fn rect(&self, n: u8) -> [i16; 4] {
        let layout = usize::from(self.layout.clamp(1, 2) - 1);
        self.layouts[layout].get(usize::from(n)).copied().unwrap_or_default()
    }

    fn hit(&self, n: u8, x: i32, y: i32) -> bool {
        let [x1, y1, x2, y2] = self.rect(n).map(i32::from);
        (x1..=x2).contains(&x) && (y1..=y2).contains(&y)
    }

    /// The form's full height in this layout.
    #[must_use]
    pub fn full_height(&self) -> i32 {
        i32::from(self.rect(r::FORM)[3]) + 1
    }

    /// The submenu's index in the lists, if it's one.
    fn slot(&self) -> Option<usize> {
        usize::from(self.menu)
            .checked_sub(usize::from(FIRST_MENU))
            .filter(|&k| k < MENUS)
    }

    /// `Baumenuebild` sized to the layout. // 1018:2b01
    fn lay_out(&mut self) {
        let [_, _, x2, y2] = self.rect(r::FORM).map(i32::from);
        let size = |n: i32| usize::try_from(n + 1).unwrap_or(1);
        if self.image.width != size(x2) || self.image.height != size(y2) {
            self.image = Surface::new(size(x2), size(y2), 0);
        }
        self.width = x2 + 1;
    }

    /// Copies rectangle `n` of bitmap `b`'s top left (or of the same
    /// place, `same`) over rectangle `n`.
    fn blit_rect(&mut self, n: u8, b: usize, same: bool) {
        let [x1, y1, x2, y2] = self.rect(n).map(i32::from);
        let from = if same {
            Rect::new(x1, y1, x2 - x1 + 1, y2 - y1 + 1)
        } else {
            Rect::new(0, 0, x2 - x1 + 1, y2 - y1 + 1)
        };
        let all = self.image.bounds();
        self.image.blit(x1, y1, &self.bitmaps[b], from, Rop::Copy, all);
    }

    /// The layout's picture back over rectangle `n`. // 1018:11ce
    fn restore(&mut self, n: u8) {
        let b = BAUWIN[usize::from(self.layout.clamp(1, 2) - 1)];
        self.blit_rect(n, b, true);
    }

    /// `text` in rectangle `n`, over the layout's picture: from its left
    /// edge, one pixel above its top, as wide as it. // 1018:1502
    fn text_in(&mut self, font: &Font, text: &[u8], color: i32, align: Align, n: u8) {
        self.restore(n);
        let [x1, y1, x2, _] = self.rect(n).map(i32::from);
        font.draw(&mut self.image, text, (x1, y1 - 1), x2 - x1, align, color);
    }

    /// A button's text in rectangle `n`, centred, one pixel below its
    /// top. // 1018:15e1
    fn button_text(&mut self, font: &Font, text_index: usize, n: u8) {
        let text = self.texts.texts.get(text_index).cloned().unwrap_or_default();
        let [x1, y1, x2, _] = self.rect(n).map(i32::from);
        font.draw(&mut self.image, &text, (x1, y1 + 1), x2 - x1, Align::Center, 9);
    }

    fn other_text(&self, n: usize) -> Vec<u8> {
        self.texts.texts.get(n).cloned().unwrap_or_default()
    }

    /// Opens the submenu of `tool` (9..=20, but 16), whose button is
    /// `button`, at its layout: the whole form drawn, at the right of the
    /// `screen` (or further left where it was dragged), with the focus.
    /// // 1018:2b01
    #[allow(clippy::too_many_arguments)]
    pub fn open(
        &mut self,
        tool: u8,
        button: u8,
        world: &World,
        font: &Font,
        tiles: &Tiles,
        info: &InfoTexts,
        view: u8,
        screen: (i32, i32),
    ) {
        self.restore_menu(tool, button, world, font, tiles, info, view);
        self.open = true;
        self.left = (screen.0 - self.width - 1).min(self.left);
        self.height = self.full_height();
        self.sounds.push(0xd); // 10a0:30b6
    }

    /// The form for `tool`'s submenu, drawn, without opening it, as the
    /// start of a game does with the saved tool. // 1018:2a17
    ///
    /// For a tool that isn't a submenu's the original draws a heading
    /// from past the 12 it has and leaves the building; the port draws
    /// nothing.
    #[allow(clippy::too_many_arguments)]
    pub fn restore_menu(
        &mut self,
        tool: u8,
        button: u8,
        world: &World,
        font: &Font,
        tiles: &Tiles,
        info: &InfoTexts,
        view: u8,
    ) {
        self.menu = tool;
        self.button = button;
        self.layout = if BIG_MENUS.contains(&tool) { 2 } else { 1 };
        self.pressed = 0;
        self.lay_out();
        if self.slot().is_none() {
            // BUG(original): for a tool without a submenu it draws a heading from past its list; the port draws nothing.
            return;
        }
        self.restore(r::FORM);
        self.draw_texts(world, font, info); // 1018:1b5e
        self.draw_item(world, font, tiles, info, view); // 1018:16c8
    }

    /// Closes the form: no height, and the main screen gets the focus.
    /// // 1018:2c71
    pub fn close(&mut self) {
        self.sounds.push(0xe);
        self.height = 0;
        self.open = false;
    }

    /// The heading, the figures' labels, the buttons and the list.
    /// // 1018:1b5e
    fn draw_texts(&mut self, world: &World, font: &Font, info: &InfoTexts) {
        let heading = usize::from(self.menu)
            .checked_sub(usize::from(FIRST_MENU))
            .and_then(|k| self.texts.headings.get(k))
            .cloned()
            .unwrap_or_default();
        self.text_in(font, &heading, 1, Align::Center, r::HEADING);
        for (text, n) in [(0, 0xd), (1, 0x10), (2, 0x13), (3, 0x15), (4, 0x16), (5, 0x18)] {
            let t = self.other_text(text);
            self.text_in(font, &t, 9, Align::Left, n);
        }
        let currency = info.currency.clone(); // 1110:7352
        self.text_in(font, &currency, 9, Align::Left, 0xf);
        let rent = if self.menu == 9 || self.menu == 10 { 6 } else { 7 };
        let t = self.other_text(rent);
        self.text_in(font, &t, 9, Align::Left, 0x19);
        let t = self.other_text(3);
        self.text_in(font, &t, 9, Align::Left, 0x1b);
        self.button_text(font, 9, r::BUILD_PICTURE);
        self.button_text(font, 8, r::CANCEL_PICTURE);
        self.draw_list(world, font); // 1018:1a43
    }

    /// The list's visible rows: each type's short name, the selected one
    /// in colour 10. // 1018:1a43
    fn draw_list(&mut self, world: &World, font: &Font) {
        let Some(k) = self.slot() else {
            return;
        };
        let Some((rows, count, _)) = world.build_menu.menu(self.menu) else {
            return;
        };
        let visible = VISIBLE_ROWS[usize::from(self.layout.clamp(1, 2) - 1)];
        let mut line = 0;
        let mut n = self.scroll[k];
        while line < visible && n < i16::from(count) {
            let building = usize::try_from(n).ok().and_then(|n| rows.get(n)).map_or(NONE, |r| r[0]);
            // BUG(original): a row of -1 shows the name before the list's first; the port shows none.
            let name = usize::try_from(building)
                .ok()
                .and_then(|b| self.short_names.get(b))
                .cloned()
                .unwrap_or_default();
            self.draw_row(font, &name, self.selected[k] == n, line);
            line += 1;
            n += 1;
        }
    }

    /// Row `line` of the list. // 1018:1917
    fn draw_row(&mut self, font: &Font, name: &[u8], selected: bool, line: i16) {
        let [x1, y1, x2, _] = self.rect(r::LIST);
        let top = i32::from(y1) + 2 + i32::from(line * ROW_HEIGHT);
        // 1018:1468: the layout's picture behind the row.
        let b = BAUWIN[usize::from(self.layout.clamp(1, 2) - 1)];
        let (x1, x2) = (i32::from(x1), i32::from(x2));
        let from = Rect::new(x1, top + 1, x2 - x1 + 1, 15);
        let all = self.image.bounds();
        self.image.blit(x1, top + 1, &self.bitmaps[b], from, Rop::Copy, all);
        let color = if selected { 10 } else { 9 };
        font.draw(&mut self.image, name, (x1 + 2, top), 200, Align::Left, color);
    }

    /// The selected row's type, from its look, and its name, figures and
    /// picture. // 1018:16c8
    fn draw_item(&mut self, world: &World, font: &Font, tiles: &Tiles, info: &InfoTexts, view: u8) {
        self.has_looks = false;
        if let Some(k) = self.slot()
            && let Some((rows, _, _)) = world.build_menu.menu(self.menu)
        {
            let row = usize::try_from(self.selected[k])
                .ok()
                .and_then(|n| rows.get(n))
                .copied()
                .unwrap_or([NONE; 4]);
            self.has_looks = row[1] != NONE;
            self.building = if self.has_looks {
                row[usize::from(self.looks[k] & 3)]
            } else {
                row[0]
            };
        }
        // BUG(original): an empty submenu selects type -1, whose name and figures are read from before the tables; the port shows none and the tool builds nothing.
        let building = u8::try_from(self.building).ok();
        let t = &world.tables;
        let name = building
            .and_then(|b| self.long_names.get(usize::from(b)))
            .cloned()
            .unwrap_or_default();
        self.text_in(font, &name, 10, Align::Center, r::NAME);
        let money = |n: i32| format_money(n, info.currency_factor, info.language);
        let number = |n: i16| n.to_string().into_bytes();
        let b = building.unwrap_or(u8::MAX);
        let figures = [
            (money(t.hausprei(b, 0)), Align::Right, 0xe),
            (number(t.hausdata(b, 1)), Align::Right, 0x11),
            (
                usize::try_from(t.hausdata(b, 2))
                    .ok()
                    .and_then(|n| self.auswahl.get(n))
                    .cloned()
                    .unwrap_or_default(),
                Align::Left,
                0x12,
            ),
            (money(t.hausprei(b, 1)), Align::Right, 0x14),
            (money(t.hausprei(b, 2)), Align::Right, 0x1a),
            (number(t.hausdata(b, 3)), Align::Right, 0x17),
        ];
        for (text, align, n) in figures {
            self.text_in(font, &text, 9, align, n);
        }
        // 1018:10a9: the ground behind the picture.
        let ground = if t.hausdata(b, 7) == 0 { BACKSAND } else { BACKGRAS };
        self.blit_rect(r::PICTURE, ground, false);
        if let Some(b) = building {
            let rect = self.rect(r::PICTURE);
            darstellung::draw_building_picture(&mut self.image, tiles, b, view, rect); // 1018:0e4d
        }
        if self.has_looks {
            self.blit_rect(r::ROTATE, ROTATE, false); // 1018:1374
        }
    }

    /// The view turned: the open form's picture turns too. // 1018:29f7
    pub fn view_turned(&mut self, world: &World, font: &Font, tiles: &Tiles, info: &InfoTexts, view: u8) {
        if self.open {
            self.draw_item(world, font, tiles, info, view);
        }
    }

    /// A row of the list was clicked `dy` pixels below its top: it's
    /// selected if the submenu has it. // 1018:237f
    fn select_row(&mut self, dy: i32, world: &World, font: &Font, tiles: &Tiles, info: &InfoTexts, view: u8) {
        let Some(k) = self.slot() else {
            return;
        };
        let length = world.build_menu.menu(self.menu).map_or(0, |m| m.2);
        let row = i32::from(self.scroll[k]) + dy / i32::from(ROW_HEIGHT);
        if (0..=i32::from(length) - 1).contains(&row) {
            self.selected[k] = i16::try_from(row).unwrap_or(0);
            self.draw_list(world, font);
            self.draw_item(world, font, tiles, info, view);
        }
    }

    /// The world took a type off submenu `menu`: its first row is
    /// selected again (the list itself is the world's). // 1018:26b9
    pub fn removed(&mut self, menu: u8) {
        if let Some(k) = usize::from(menu)
            .checked_sub(usize::from(FIRST_MENU))
            .filter(|&k| k < MENUS)
        {
            self.selected[k] = 0;
        }
    }

    /// `BaumenuebildMouseDown` at `x`, `y` in the form, `left` for the
    /// left button; `cursor` is the mouse on the screen. // 1018:0208
    #[allow(clippy::too_many_arguments)]
    pub fn mouse_down(
        &mut self,
        x: i32,
        y: i32,
        left: bool,
        shift: ShiftState,
        cursor: (i32, i32),
        world: &World,
        font: &Font,
        tiles: &Tiles,
        info: &InfoTexts,
        view: u8,
    ) -> Response {
        let mut response = Response::Nothing;
        if left {
            if self.hit(r::CLOSE, x, y) {
                self.pressed = r::CLOSE;
                self.blit_rect(r::CLOSE, CLOSE_PRESSED, false); // 1018:1d7d
                self.sounds.push(8);
            } else if self.has_looks && self.hit(r::ROTATE, x, y) {
                self.pressed = r::ROTATE;
                self.blit_rect(r::ROTATE, ROTATE_PRESSED, false); // 1018:1e6d
                self.sounds.push(8);
            } else if self.hit(r::UP, x, y) {
                self.pressed = r::UP;
                self.blit_rect(r::UP_PICTURE, UP_PRESSED, false); // 1018:2171
                self.sounds.push(8);
            } else if self.hit(r::DOWN, x, y) {
                self.pressed = r::DOWN;
                self.blit_rect(r::DOWN_PICTURE, DOWN_PRESSED, false); // 1018:2278
                self.sounds.push(8);
            } else if self.hit(r::LIST, x, y) {
                let dy = y - i32::from(self.rect(r::LIST)[1]);
                self.select_row(dy, world, font, tiles, info, view); // 1018:237f
                self.sounds.push(8);
                if shift.contains(ShiftState::DOUBLE) {
                    self.close();
                    response = Response::Close;
                }
            } else if self.hit(r::BUILD, x, y) {
                self.pressed = r::BUILD;
                self.blit_rect(r::BUILD_PICTURE, BUTTON_PRESSED, false); // 1018:1f61
                self.button_text(font, 9, r::BUILD);
                self.sounds.push(8);
            } else if self.hit(r::CANCEL, x, y) {
                self.pressed = r::CANCEL;
                self.blit_rect(r::CANCEL_PICTURE, BUTTON_PRESSED, false); // 1018:2069
                self.button_text(font, 8, r::CANCEL);
                self.sounds.push(8);
            }
        }
        if self.hit(r::TITLE, x, y) {
            // Any button: the form shrinks to its title while it's dragged.
            self.pressed = r::TITLE;
            self.grab = (cursor.0 - self.left, cursor.1 - self.top);
            self.height = TITLE_HEIGHT;
            self.double = shift.contains(ShiftState::DOUBLE);
        }
        response
    }

    /// `BaumenuebildMouseMove`: with the title bar held by the left button
    /// the form follows the mouse (`cursor`, on the screen), on the
    /// `screen`. // 1018:0461
    pub fn mouse_move(&mut self, shift: ShiftState, cursor: (i32, i32), screen: (i32, i32)) {
        if self.pressed == r::TITLE && shift.contains(ShiftState::LEFT) {
            self.left = (cursor.0 - self.grab.0).max(0).min(screen.0 - self.width);
            self.top = (cursor.1 - self.grab.1).max(0).min(screen.1 - self.height);
        }
    }

    /// `BaumenuebildMouseUp` at `x`, `y` in the form. // 1018:04e7
    #[allow(clippy::too_many_arguments)]
    pub fn mouse_up(
        &mut self,
        x: i32,
        y: i32,
        left: bool,
        world: &World,
        font: &Font,
        tiles: &Tiles,
        info: &InfoTexts,
        view: u8,
    ) -> Response {
        let mut response = Response::Nothing;
        let k = self.slot();
        if left {
            match self.pressed {
                r::CLOSE => {
                    self.restore(r::CLOSE); // 1018:1cff
                    self.sounds.push(10);
                    if self.hit(r::CLOSE, x, y) {
                        self.close();
                        response = Response::Cancel;
                    }
                }
                r::CANCEL => {
                    self.restore(r::CANCEL_PICTURE); // 1018:1d50
                    self.button_text(font, 8, r::CANCEL_PICTURE);
                    self.sounds.push(10);
                    if self.hit(r::CANCEL, x, y) {
                        self.close();
                        response = Response::Cancel;
                    }
                }
                r::BUILD => {
                    self.restore(r::BUILD_PICTURE); // 1018:1d23
                    self.button_text(font, 9, r::BUILD_PICTURE);
                    self.sounds.push(10);
                    if self.hit(r::BUILD, x, y) {
                        self.close();
                        response = Response::Close;
                    }
                }
                r::ROTATE => {
                    self.blit_rect(r::ROTATE, ROTATE, false); // 1018:1d12
                    self.sounds.push(10);
                    if self.hit(r::ROTATE, x, y)
                        && let Some(k) = k
                    {
                        self.looks[k] = (self.looks[k] + 1) % 4;
                        self.draw_item(world, font, tiles, info, view);
                    }
                }
                r::UP => {
                    self.restore(r::UP_PICTURE); // 1018:2265
                    self.sounds.push(10);
                    if self.hit(r::UP, x, y)
                        && let Some(k) = k
                    {
                        self.scroll[k] = (self.scroll[k] - 1).max(0);
                        self.draw_list(world, font);
                    }
                }
                r::DOWN => {
                    self.restore(r::DOWN_PICTURE); // 1018:236c
                    self.sounds.push(10);
                    if self.hit(r::DOWN, x, y)
                        && let Some(k) = k
                    {
                        let count = world.build_menu.menu(self.menu).map_or(0, |m| m.1);
                        let visible = VISIBLE_ROWS[usize::from(self.layout.clamp(1, 2) - 1)];
                        let last = (i16::from(count) - visible).max(0);
                        self.scroll[k] = (self.scroll[k] + 1).min(last);
                        self.draw_list(world, font);
                    }
                }
                _ => {}
            }
        }
        if self.pressed == r::TITLE && !self.double {
            self.height = self.full_height();
        }
        self.pressed = 0;
        response
    }

    /// `FormKeyDown`: the arrow keys switch submenus, Escape cancels,
    /// Enter closes, Space gives the main screen the focus; F1 to F12, the
    /// digits but 0 and Pause go to the main screen; with `build_menu`
    /// every key the menu doesn't use. In the later build's scenario 10
    /// (`scenario`) the arrows skip submenu 10, the hotels
    /// ([`crate::hauptschirm::button_blocked`]). // 1018:3290, en 1018:32a5
    #[must_use]
    pub fn key_down(&self, key: u16, scenario: u8) -> Response {
        let skip = |b: u8, table: &[u8; 12]| {
            if crate::hauptschirm::button_blocked(scenario, b) {
                table.get(usize::from(b - FIRST_MENU)).copied().unwrap_or(b)
            } else {
                b
            }
        };
        let k = usize::from(self.menu).checked_sub(usize::from(FIRST_MENU));
        // BUG(original): only these keys reach the main screen: the arrow keys and 0 do nothing while the build menu has the focus.
        let own = matches!(key, 0x25 | 0x27 | 0x1b | 0xd | 0x20);
        if main_screen_key(key) || (!own && self.enhance.has(Enhancement::BuildMenu)) {
            return Response::MainScreenKey(key);
        }
        match key {
            0x27 => k
                .and_then(|k| NEXT.get(k))
                .map_or(Response::Nothing, |&b| Response::Button(skip(b, &NEXT))),
            0x25 => k
                .and_then(|k| PREVIOUS.get(k))
                .map_or(Response::Nothing, |&b| Response::Button(skip(b, &PREVIOUS))),
            0x1b => Response::Cancel,
            0xd => Response::Close,
            0x20 => Response::FocusMainScreen,
            _ => Response::Nothing,
        }
    }

    /// State 1500 of the later build for `scenario` 12 (hurry up): the
    /// submenu at index 5 (menu 14) starts at its first row, after the
    /// world took [`crate::hintergrund::SCENARIO_BANNED`] off the menu.
    /// // en 1018:015f
    pub fn start_scenario(&mut self, scenario: u8) {
        if scenario == HURRY_UP {
            self.scroll[HURRY_UP_SUBMENU] = 0;
        }
    }

    /// The form's bounds on the screen.
    #[must_use]
    pub fn bounds(&self) -> Rect {
        Rect::new(self.left, self.top, self.width, self.height)
    }

    /// Whether `x`, `y` on the screen is on the form.
    #[must_use]
    pub fn contains(&self, x: i32, y: i32) -> bool {
        self.height > 0
            && (self.left..self.left + self.width).contains(&x)
            && (self.top..self.top + self.height).contains(&y)
    }

    /// The form's fields as the map file saves them (unit 1018:3350),
    /// into `fields`; the lists are the world's.
    pub fn save(&self, fields: &mut Fields) {
        let words = |v: &[i16]| v.iter().flat_map(|n| n.to_le_bytes()).collect::<Vec<u8>>();
        fields.set("[1110:6516]+0x1ac", 0, &[self.menu]);
        fields.set("[1110:6516]+0x1ad", 0, &[self.button]);
        fields.set("[1110:6516]+0x1ae", 0, &words(&self.selected));
        fields.set("[1110:6516]+0x1de", 0, &self.looks);
        fields.set("[1110:6516]+0x1ea", 0, &[u8::from(self.has_looks)]);
        fields.set("[1110:6516]+0x1eb", 0, &[self.pressed]);
        fields.set("[1110:6516]+0x1ec", 0, &[self.layout]);
        fields.set("*[1110:6516]+0x1ed", 0, &words(&self.scroll));
        fields.set("[1110:6516]+0x1f9", 0, &[u8::from(self.double)]);
        fields.set("[1110:6516]+0x1fe", 0, &[u8::from(self.open)]);
    }

    /// The form's fields from a map file (unit 1018:34a0). The form's
    /// height isn't saved: it stays as it was, and for the port, which
    /// sets up a new form for each game, hidden.
    pub fn load(&mut self, fields: &Fields) {
        let byte = |dest: &str| fields.get(dest, 0).and_then(|d| d.first().copied()).unwrap_or(0);
        let words = |dest: &str, out: &mut [i16; MENUS]| {
            if let Some(d) = fields.get(dest, 0) {
                for (o, w) in out.iter_mut().zip(d.as_chunks::<2>().0) {
                    *o = i16::from_le_bytes(*w);
                }
            } else {
                *out = [0; MENUS];
            }
        };
        self.menu = byte("[1110:6516]+0x1ac");
        self.button = byte("[1110:6516]+0x1ad");
        words("[1110:6516]+0x1ae", &mut self.selected);
        self.looks = [0; MENUS];
        if let Some(d) = fields.get("[1110:6516]+0x1de", 0) {
            for (o, &v) in self.looks.iter_mut().zip(d) {
                *o = v;
            }
        }
        self.has_looks = byte("[1110:6516]+0x1ea") != 0;
        self.pressed = byte("[1110:6516]+0x1eb");
        self.layout = byte("[1110:6516]+0x1ec").clamp(1, 2);
        words("*[1110:6516]+0x1ed", &mut self.scroll);
        self.double = byte("[1110:6516]+0x1f9") != 0;
        self.open = byte("[1110:6516]+0x1fe") != 0;
    }
}
