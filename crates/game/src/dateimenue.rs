//! `TDateimenue`, the file menu (segment `1040`): the button bar's first
//! button (or the keys `1` and `D`, 1020:2e09) opens it in a game
//! (1010:2c8b). It saves the game under a name, loads one, starts a new
//! one ("Neues Spiel", which saves `Last.map` first) or quits. Opening it
//! stops the clock and disables the other forms until it closes.
//!
//! The form is one `TImage`, `Dateimenuebild`, with the background
//! `datwin1c.BMP`. Its six buttons and the title bar are drawn on the
//! background and found by the rectangles of a table (`1110:1812`); the
//! list of saved games, its arrows and the name field are widgets of the
//! game's toolkit. Saving and loading come back as [`Action`]s: the
//! caller writes and reads the files, and hands the list of saved games
//! back with [`Dateimenue::set_games`].
//!
//! With the later build's `UPGRADE\` folder the form is its own
//! (en 1040:077e, `docs/versions.md`): `UPGRADE\datwin1d.BMP` has a fifth
//! button on the left, "Löschen" (`UPGRADE\SONSTIGE.TXT`'s 25th text),
//! above "Neues Spiel", "Beenden" moves down one, and the table of
//! rectangles has two more (en `1118:1812`, [`RECTS_UPGRADE`]).

use vcl::Rect;
use vcl::surface::{Rop, Surface};

use crate::enhance::{Enhancement, Enhancements};
use crate::lang::{Language, Text};
use crate::text::{Align, Font};
use crate::widgets::{BitmapId, Button, Edit, Frame, ListBox, Outcome, ShiftState, Skin, Sound, Widget};

/// The bitmaps the form draws with, from `GRAFIK`, in the order
/// [`Dateimenue::new`] takes them.
pub const BITMAP_FILES: [&str; 8] = [
    "datwin1c.BMP", // the background, 1110:6604, 1040:12b0
    // The rest are `hintergrund`'s, which state 100 loads.
    "closepre.bmp", // 1110:54bc
    "datB1pr.bmp",  // 1110:54c4, a button held down
    "datListe.bmp", // 1110:54c8
    "datDow.bmp",   // 1110:54cc
    "datDowPr.bmp", // 1110:54d0
    "datUp.bmp",    // 1110:54d4
    "datUpPr.bmp",  // 1110:54d8
];
const BACKGROUND: BitmapId = BitmapId(0);
const CLOSE_PRESSED: BitmapId = BitmapId(1);
const BUTTON_PRESSED: BitmapId = BitmapId(2);
const LIST: BitmapId = BitmapId(3);
const DOWN: BitmapId = BitmapId(4);
const DOWN_PRESSED: BitmapId = BitmapId(5);
const UP: BitmapId = BitmapId(6);
const UP_PRESSED: BitmapId = BitmapId(7);

/// The form's parts: left, top, right and bottom, inclusive. Entry 0 is
/// the form, 1 the close button, 2 the title bar, 3 the list; the
/// buttons have one entry to draw (`n`) and one to hit (`n + 1`), the
/// same rectangle. 12 (the name field's) isn't used. // 1110:1812
pub const RECTS: [(i32, i32, i32, i32); 21] = [
    (0, 0, 201, 209),     // 1110:1812
    (0, 0, 15, 15),       // 1110:181a
    (16, 0, 201, 15),     // 1110:1822
    (97, 46, 192, 146),   // 1110:182a
    (130, 25, 157, 41),   // 1110:1832
    (130, 25, 157, 41),   // 1110:183a
    (130, 159, 157, 175), // 1110:1842
    (130, 159, 157, 175), // 1110:184a
    (8, 49, 85, 67),      // 1110:1852
    (8, 49, 85, 67),      // 1110:185a
    (8, 71, 85, 89),      // 1110:1862
    (8, 71, 85, 89),      // 1110:186a
    (8, 90, 88, 108),     // 1110:1872
    (8, 114, 85, 132),    // 1110:187a
    (8, 114, 85, 132),    // 1110:1882
    (8, 136, 85, 154),    // 1110:188a
    (8, 136, 85, 154),    // 1110:1892
    (8, 186, 85, 204),    // 1110:189a
    (8, 186, 85, 204),    // 1110:18a2
    (106, 186, 183, 204), // 1110:18aa
    (106, 186, 183, 204), // 1110:18b2
];

/// The later build's parts: as [`RECTS`] up to 12, then "Löschen" (13,
/// 14), "Neues Spiel" (15, 16), "Beenden" (17, 18), "Abbruch" (19, 20)
/// and "OK" (21, 22). // en 1118:1812
pub const RECTS_UPGRADE: [(i32, i32, i32, i32); 23] = [
    (0, 0, 201, 209),
    (0, 0, 15, 15),
    (16, 0, 201, 15),
    (97, 46, 192, 146),
    (130, 25, 157, 41),
    (130, 25, 157, 41),
    (130, 159, 157, 175),
    (130, 159, 157, 175),
    (8, 49, 85, 67),
    (8, 49, 85, 67),
    (8, 71, 85, 89),
    (8, 71, 85, 89),
    (8, 90, 88, 108),
    (8, 114, 85, 132),
    (8, 114, 85, 132),
    (8, 136, 85, 154),
    (8, 136, 85, 154),
    (8, 158, 85, 176), // en 1118:189a
    (8, 158, 85, 176), // en 1118:18a2
    (8, 186, 85, 204),
    (8, 186, 85, 204),
    (106, 186, 183, 204),
    (106, 186, 183, 204),
];

/// The form's size: entry 0 of [`RECTS`] (1040:12be).
pub const WIDTH: i32 = 202;
pub const HEIGHT: i32 = 210;
/// The height the form shrinks to while its title bar is held, its
/// title bar's (`ClientHeight := 16`). // 1040:073c, en 1040:077e
pub const TITLE_HEIGHT: i32 = 16;

const CLOSE: u8 = 1;
const TITLE: u8 = 2;
const LOAD: u8 = 9;
const SAVE: u8 = 0xb;

/// What a button does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Close,
    Load,
    Save,
    Delete,
    NewGame,
    Quit,
    Cancel,
    Ok,
}

/// A form's buttons: the part to hit (the one before is drawn), what
/// it does and its text, by `SONSTIGE.TXT`'s texts, in the order a press
/// tries them.
type Buttons = &'static [(u8, Kind, Option<usize>)];

/// v1.0's: 7 "Laden", 8 "Speichern", 9 "Neues Spiel", 10 "Beenden", 4
/// "Abbruch", 3 "OK". // 1040:073c, 1040:04d9
const BUTTONS: Buttons = &[
    (CLOSE, Kind::Close, None),
    (0x12, Kind::Cancel, Some(4)),
    (0x14, Kind::Ok, Some(3)),
    (0x10, Kind::Quit, Some(10)),
    (LOAD, Kind::Load, Some(7)),
    (SAVE, Kind::Save, Some(8)),
    (0xe, Kind::NewGame, Some(9)),
];

/// The later build's, with "Löschen" (`UPGRADE\SONSTIGE.TXT`'s 25th
/// text) and the ones below it moved. // en 1040:077e, en 1040:04d9
const BUTTONS_UPGRADE: Buttons = &[
    (CLOSE, Kind::Close, None),
    (0x14, Kind::Cancel, Some(4)),
    (0x16, Kind::Ok, Some(3)),
    (0x12, Kind::Quit, Some(10)),
    (LOAD, Kind::Load, Some(7)),
    (SAVE, Kind::Save, Some(8)),
    (0xe, Kind::Delete, Some(24)),
    (0x10, Kind::NewGame, Some(9)),
];

/// `VK_*` codes the form reads.
/// The message for a name that isn't one ([`Action::InvalidName`]):
/// v1.0 has only the German (CS:0df0, 1040:0e03); the later build picks
/// it by `land` (en 1040:0eef): German for `D` (en 1040:0eb0), French
/// for `F` (en 1040:0ec3), English for `A` and `E` (en 1040:0ede). The
/// port takes the view's language's (`file_menu.invalid_name`,
/// `crate::lang`); it has no French.
#[must_use]
pub fn invalid_name_text(language: Language) -> &'static [u8] {
    language.bytes(Text::InvalidName)
}

/// The later build's question before "Löschen" erases a game
/// ([`Action::Delete`]), by `land` as [`invalid_name_text`] (en 1040:10d7,
/// en 1040:10ea, en 1040:10f9); the port's in the view's language
/// (`file_menu.delete_question`). // en 1040:110c
#[must_use]
pub fn delete_question(language: Language) -> &'static [u8] {
    language.bytes(Text::DeleteQuestion)
}

const VK_RETURN: u16 = 0x0d;
const VK_ESCAPE: u16 = 0x1b;
const VK_SPACE: u16 = 0x20;
const VK_UP: u16 = 0x26;
const VK_DOWN: u16 = 0x28;
const VK_F4: u16 = 0x73;

/// What the file menu asks of the game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// "OK", "Abbruch", the close button, Escape, Space or Alt+F4: the
    /// menu hides and the clock runs again (1020:24b7).
    Close,
    /// "Laden" (or `L`, or a double click on the selected game) with a
    /// game selected: `load_map` of this file from `MAPS\` in the middle
    /// of the game, then `game_show_state` (1010:26fa). The menu stays
    /// open. // 1040:0f27
    Load(Vec<u8>),
    /// "Speichern" (or `S`, or Enter in the name field) while the field
    /// is focused, with a valid name: `save_map` to this file in `MAPS\`.
    /// The caller then lists the games again ([`Dateimenue::set_games`]).
    /// // 1040:0e03
    Save(Vec<u8>),
    /// The name isn't one ([`valid_name`]): a message box says
    /// "Dateiname ungültig" (`vcl_messagedlg`, CS:0df0), in the later
    /// build by `land` ([`invalid_name_text`]). // 1040:0e03, en 1040:0eef
    InvalidName,
    /// "Neues Spiel" (or `N`): `back_to_start_menu` saves `Last.map` and
    /// shows the start menu. // 1010:25dc
    NewGame,
    /// "Beenden" (or `Q`): the menu hides and `TEndeform` asks whether to
    /// quit.
    Quit,
    /// The later build's "Löschen" (or `D`) with a game selected: after a
    /// question ([`delete_question`], by `land`) the file is erased from
    /// `MAPS\` and the list filled again ([`Dateimenue::set_games`]).
    /// // en 1040:110c
    Delete(Vec<u8>),
}

/// What an event did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[must_use]
pub struct Response {
    /// Something changed and the handler refreshes the image.
    pub refresh: bool,
    /// The sounds played, in order (`play_sound`, 10a0:30b6).
    pub sounds: Vec<Sound>,
    pub action: Option<Action>,
    /// The form moved (dragged by its title bar).
    pub moved: bool,
}

/// Whether `name` is one the file menu saves under: one to eight
/// characters, a letter first, then letters, digits and `_`. // 1040:0d0f
#[must_use]
pub fn valid_name(name: &[u8]) -> bool {
    match name.split_first() {
        Some((first, rest)) if name.len() <= 8 => {
            first.is_ascii_alphabetic() && rest.iter().all(|&c| c.is_ascii_alphanumeric() || c == b'_')
        }
        _ => false,
    }
}

/// The file menu.
#[derive(Debug, Clone)]
pub struct Dateimenue {
    font: Font,
    bitmaps: Vec<Surface>,
    empty: Surface,
    /// `SONSTIGE.TXT`'s texts, for the title and the buttons.
    texts: Vec<Vec<u8>>,
    /// `Dateimenuebild`'s picture.
    image: Surface,
    /// The form's top left on the screen.
    pub left: i32,
    pub top: i32,
    edit: Edit,    // +0x180
    up: Button,    // +0x2b4
    list: ListBox, // +0x293
    down: Button,  // +0x3ca
    /// The part the left button went down on, 0 for none. // 1110:6609
    pressed: u8,
    /// What Enter does: 0 load, 1 save, 2 nothing yet; the last of "Laden"
    /// and "Speichern" clicked sets it, but nothing reads it (the name
    /// field's `EditierfensterKeyDown`, 1040:0cd3, is never called).
    /// // 1110:6608
    mode: u8,
    /// Where the mouse grabbed the title bar, from the form's top left.
    /// // 1110:6046, 1110:6048
    grab: (i32, i32),
    /// The form's height: [`HEIGHT`], or [`TITLE_HEIGHT`] while its title
    /// bar is held. `Height`
    height: i32,
    /// The player's view enhancements (`edit_field`, `modifier_clicks`,
    /// `form_drags`).
    pub enhance: Enhancements,
    /// The parts and the buttons: v1.0's, or the later build's with
    /// `UPGRADE\` ([`Dateimenue::with_upgrade`]).
    rects: &'static [(i32, i32, i32, i32)],
    buttons: Buttons,
}

impl Dateimenue {
    /// The form as it was created, not shown, at `left`, `top` (the DFM's
    /// position). `bitmaps` are those of [`BITMAP_FILES`], `texts`
    /// `SONSTIGE.TXT`'s. // 1040:154d
    #[must_use]
    pub fn new(font: Font, bitmaps: Vec<Surface>, texts: Vec<Vec<u8>>, left: i32, top: i32) -> Self {
        Dateimenue {
            font,
            bitmaps,
            empty: Surface::new(0, 0, 0),
            texts,
            image: Surface::new(WIDTH as usize, HEIGHT as usize, 0),
            left,
            top,
            edit: Edit::new(Frame::new(8, 0x5a, 0x4f, 0x13), b"", 8, 9, 11),
            up: Button::new(Frame::new(0x82, 0x19, 0x1c, 0x11), UP, UP_PRESSED, b"", 9),
            list: ListBox::new(Frame::new(0x61, 0x2e, 0x60, 0x6c), LIST, 9, 10, 0xf),
            down: Button::new(Frame::new(0x82, 0x9f, 0x1c, 0x11), DOWN, DOWN_PRESSED, b"", 9),
            pressed: 0,
            mode: 2,
            grab: (0, 0),
            height: HEIGHT,
            enhance: Enhancements::ORIGINAL,
            rects: &RECTS,
            buttons: BUTTONS,
        }
    }

    /// The later build's form, with `UPGRADE\`: the background (the
    /// caller's `datwin1d.BMP` in place of `datwin1c.BMP`) has the button
    /// "Löschen", whose text is the 25th of `texts`. // en 1040:159e
    #[must_use]
    pub fn with_upgrade(mut self) -> Self {
        self.rects = &RECTS_UPGRADE;
        self.buttons = BUTTONS_UPGRADE;
        self
    }

    /// What button `id` does.
    fn kind(&self, id: u8) -> Option<Kind> {
        self.buttons.iter().find(|b| b.0 == id).map(|b| b.1)
    }

    /// Button `id`'s text.
    fn button_text(&self, id: u8) -> Option<usize> {
        self.buttons.iter().find(|b| b.0 == id).and_then(|b| b.2)
    }

    /// `FormShow`: the background with the title and the buttons' texts,
    /// new widgets (an empty name field, the list of `games` from the
    /// top, both arrows). The original also stops the clock, hides the
    /// help, disables the other forms and lets the button bar's button
    /// up; that's the caller's. // 1040:12be
    pub fn show(&mut self, games: Vec<Vec<u8>>) {
        let clip = self.image.bounds();
        let background = self.bitmaps.get(BACKGROUND.0).unwrap_or(&self.empty);
        self.image.blit(0, 0, background, clip, Rop::Copy, clip);
        self.draw_texts();
        self.edit = Edit::new(Frame::new(8, 0x5a, 0x4f, 0x13), b"", 8, 9, 11);
        self.up = Button::new(Frame::new(0x82, 0x19, 0x1c, 0x11), UP, UP_PRESSED, b"", 9);
        self.list = ListBox::new(Frame::new(0x61, 0x2e, 0x60, 0x6c), LIST, 9, 10, 0xf);
        self.down = Button::new(Frame::new(0x82, 0x9f, 0x1c, 0x11), DOWN, DOWN_PRESSED, b"", 9);
        let Dateimenue {
            font,
            bitmaps,
            image,
            edit,
            up,
            list,
            down,
            ..
        } = self;
        let skin = Skin { font, bitmaps };
        edit.show(image, &skin);
        list.items = games;
        list.top = 1;
        list.show(image, &skin);
        up.show(image, &skin);
        down.show(image, &skin);
    }

    /// The image's picture.
    #[must_use]
    pub fn image(&self) -> &Surface {
        &self.image
    }

    /// Where the form is on the screen, as high as it is now.
    #[must_use]
    pub fn bounds(&self) -> Rect {
        Rect::new(self.left, self.top, WIDTH, self.height)
    }

    /// Empties the name field and erases it from the background, as the
    /// later build's "Löschen" does once the game is erased and the list
    /// filled again. Its cursor stays where it was (the field's keys keep
    /// it within the text), and a focused field shows its `|` again with
    /// the next key. // en 1040:110c
    pub fn clear_name(&mut self) {
        self.edit.text.clear();
        let background = self.bitmaps.get(BACKGROUND.0).unwrap_or(&self.empty);
        self.edit.frame.erase(&mut self.image, background);
    }

    /// The list of saved games after a save: the entries and the first row
    /// at the top again, redrawn. The selection stays. // 1040:1175
    pub fn set_games(&mut self, games: Vec<Vec<u8>>) {
        self.list.items = games;
        self.list.top = 1;
        let Dateimenue {
            font,
            bitmaps,
            image,
            list,
            ..
        } = self;
        list.draw(image, &Skin { font, bitmaps });
    }

    /// What Enter would do if anything read it: 0 load, 1 save, 2 neither
    /// yet. // 1110:6608
    #[must_use]
    pub fn mode(&self) -> u8 {
        self.mode
    }

    /// The name field's text.
    #[must_use]
    pub fn name(&self) -> &[u8] {
        &self.edit.text
    }

    fn text(&self, n: usize) -> &[u8] {
        self.texts.get(n).map_or(&[], |t| &t[..])
    }

    /// The title and the buttons' texts, up. // 1040:11f2, en 1040:14bc
    fn draw_texts(&mut self) {
        for &(id, _, text) in self.buttons {
            if let Some(n) = text {
                self.draw_text(id - 1, n, false);
            }
        }
        let (l, t, r, _) = self.rect(TITLE);
        let title = self.text(6).to_vec();
        self.font
            .draw(&mut self.image, &title, (l, t + 1), r - l, Align::Center, 1);
    }

    /// A button's text: pressed one pixel further right and down
    /// (1040:03e5), up (1040:0460).
    fn draw_text(&mut self, part: u8, n: usize, pressed: bool) {
        let (l, t, r, _) = self.rect(part);
        let shift = i32::from(pressed);
        let text = self.text(n).to_vec();
        self.font.draw(
            &mut self.image,
            &text,
            (l + 2 + shift, t + 1 + shift),
            r - l,
            Align::Center,
            9,
        );
    }

    /// Draws button `id` held down: `closepre.bmp` for the close button,
    /// `datB1pr.bmp` for the others, with the text. // 1040:05a4
    fn draw_pressed(&mut self, id: u8) {
        let part = if id == CLOSE { CLOSE } else { id - 1 };
        let (l, t, r, b) = self.rect(part);
        let bitmap = if id == CLOSE { CLOSE_PRESSED } else { BUTTON_PRESSED };
        let clip = self.image.bounds();
        if let Some(src) = self.bitmaps.get(bitmap.0) {
            self.image
                .blit(l, t, src, Rect::new(0, 0, r - l + 1, b - t + 1), Rop::Copy, clip);
        }
        if let Some(n) = self.button_text(id) {
            self.draw_text(part, n, true);
        }
    }

    /// Draws button `id` up: the background's part, with the text.
    /// // 1040:04d9, 1040:0327
    fn draw_up(&mut self, id: u8) {
        let part = if id == CLOSE { CLOSE } else { id - 1 };
        let (l, t, r, b) = self.rect(part);
        let clip = self.image.bounds();
        if let Some(src) = self.bitmaps.get(BACKGROUND.0) {
            self.image
                .blit(l, t, src, Rect::new(l, t, r - l + 1, b - t + 1), Rop::Copy, clip);
        }
        if let Some(n) = self.button_text(id) {
            self.draw_text(part, n, false);
        }
    }

    /// Part `n`'s rectangle.
    fn rect(&self, part: u8) -> (i32, i32, i32, i32) {
        self.rects.get(usize::from(part)).copied().unwrap_or_default()
    }

    /// Whether `x`, `y` is in part `n`'s rectangle, edges included.
    /// // 1040:02da, 1030:1517
    fn hit(&self, part: u8, x: i32, y: i32) -> bool {
        let (left, top, right, bottom) = self.rect(part);
        (left..=right).contains(&x) && (top..=bottom).contains(&y)
    }

    /// `DateimenuebildMouseDown`, with `x`, `y` in the image, whether it's
    /// the left button, and the shift state. With the left button, a
    /// button under the mouse goes down (sound 8), or the title bar is
    /// grabbed. With any button, the list, the name field and the arrows
    /// are offered the press until one takes it; the name field
    /// unfocuses on any press, so "Speichern" can't save a name being
    /// typed. With `edit_field` a press on a button leaves the name field
    /// as it is, and a name picked from the list puts the cursor after
    /// it. // 1040:073c
    pub fn mouse_down(&mut self, x: i32, y: i32, left_button: bool, shift: ShiftState) -> Response {
        let mut r = Response::default();
        let fixed = self.enhance.has(Enhancement::EditField);
        let mut on_button = false;
        if left_button {
            if let Some(&(id, _, _)) = self.buttons.iter().find(|b| self.hit(b.0, x, y)) {
                self.pressed = id;
                self.draw_pressed(id);
                r.refresh = true;
                r.sounds.push(Sound::Press);
                on_button = true;
            } else if self.hit(TITLE, x, y) {
                self.pressed = TITLE;
                self.grab = (x, y);
                self.height = TITLE_HEIGHT;
                r.refresh = true;
            }
        }
        // 1040:0192: the list, which puts the selected name in the field.
        let out = self.list.mouse_down(x, y, shift, self.enhance);
        if out.handled || out.clicked {
            r.sounds.extend(out.sound);
            if out.handled {
                if self.list.has_selection() {
                    let n = usize::try_from(self.list.selected - 1).unwrap_or(0);
                    // BUG(original): the name goes into the field without moving its cursor.
                    self.edit.text = self.list.items.get(n).cloned().unwrap_or_default();
                    if fixed {
                        self.edit.cursor = u8::try_from(self.edit.text.len()).unwrap_or(u8::MAX);
                    }
                }
                self.redraw_list_and_name();
                r.refresh = true;
                return r;
            }
            // 1040:0180: a double click on the selected game loads it.
            r.action = self.load();
            return r;
        }
        // BUG(original): any press while the name is focused unfocuses it, so clicking "Speichern" then focuses it again instead of saving.
        if fixed && on_button {
            return r;
        }
        let out = self.edit.mouse_down(x, y, self.enhance);
        if out.handled {
            self.repaint_edit();
            r.refresh = true;
            return r;
        }
        for down in [true, false] {
            let out = if down {
                self.down.mouse_down(x, y)
            } else {
                self.up.mouse_down(x, y)
            };
            if out.handled {
                self.finish_arrow(down, out, &mut r);
                return r;
            }
        }
        r
    }

    /// `DateimenuebildMouseMove`: drags the form by its title bar while
    /// the left button is held, keeping it on the `screen` by its width
    /// and its height then, the title bar's. With `form_drags` by its
    /// full height, which it's let go with. // 1040:09ca, en 1040:0a45
    pub fn mouse_move(&mut self, x: i32, y: i32, shift: ShiftState, screen: (i32, i32)) -> Response {
        let mut r = Response::default();
        if self.pressed == TITLE && shift.contains(ShiftState::LEFT) {
            let (sx, sy) = (self.left + x, self.top + y);
            self.left = (sx - self.grab.0).max(0).min(screen.0 - WIDTH);
            let top = sy - self.grab.1;
            self.top = if self.enhance.has(Enhancement::FormDrags) {
                top.min(screen.1 - HEIGHT).max(0)
            } else {
                // BUG(original): the form is kept on the screen by its 16-pixel title bar, the height it has while dragged, so it can be dropped with all but the title bar off the bottom.
                top.max(0).min(screen.1 - self.height)
            };
            r.moved = true;
        }
        r
    }

    /// `DateimenuebildMouseUp`: with the left button, the button that
    /// went down comes up (sound 10) and does its part if the mouse is
    /// still on it, or the title bar is let go and the form gets its
    /// height back (`rect0.bottom + 1`). With any button, the arrows.
    /// // 1040:0a4c, en 1040:0ac7
    pub fn mouse_up(&mut self, x: i32, y: i32, left_button: bool) -> Response {
        let mut r = Response::default();
        if left_button {
            let id = std::mem::take(&mut self.pressed);
            if id == TITLE {
                self.height = RECTS[0].3 + 1;
                r.refresh = true;
            }
            if id != 0 && id != TITLE {
                self.draw_up(id);
                r.refresh = true;
                r.sounds.push(Sound::Release);
                let on = self.hit(id, x, y);
                r.action = match self.kind(id) {
                    Some(Kind::Close | Kind::Cancel | Kind::Ok) if on => Some(Action::Close),
                    Some(Kind::Quit) if on => Some(Action::Quit),
                    Some(Kind::NewGame) if on => Some(Action::NewGame),
                    Some(Kind::Delete) if on => self.delete(),
                    Some(Kind::Load) => {
                        self.mode = 0;
                        if on { self.load() } else { None }
                    }
                    Some(Kind::Save) => {
                        self.mode = 1;
                        if on { self.save(self.edit.focused) } else { None }
                    }
                    _ => None,
                };
            }
        }
        for down in [true, false] {
            let out = if down {
                self.down.mouse_up(x, y)
            } else {
                self.up.mouse_up(x, y)
            };
            if out.handled {
                self.finish_arrow(down, out, &mut r);
                break;
            }
        }
        r
    }

    /// `FormKeyDown`. The name field gets the key first. Without Ctrl and
    /// Alt: Escape and Space close the menu, Enter focuses the name, `L`
    /// loads, `S` saves, `Q` quits, `N` starts a new game, the later
    /// build's `D` deletes, and the arrows move the list's selection
    /// (without putting the name in the field). Alt+F4 closes the menu.
    /// // 1040:1572, en 1040:1852
    pub fn key_down(&mut self, key: u16, shift: ShiftState) -> Response {
        let mut r = Response {
            refresh: true,
            ..Response::default()
        };
        let out = self.edit.key_down(key, shift);
        if out.handled {
            self.repaint_edit();
            if out.clicked {
                // 1040:02c8: the field's `Click` is the save, which runs
                // while the field is still focused.
                r.action = self.save(true);
            }
            return r;
        }
        let mods = shift.0 & (ShiftState::SHIFT.0 | ShiftState::ALT.0 | ShiftState::CTRL.0);
        if mods == 0 || mods == ShiftState::SHIFT.0 {
            r.action = match key {
                VK_ESCAPE | VK_SPACE => Some(Action::Close),
                VK_RETURN => {
                    self.edit.focused = true;
                    self.repaint_edit();
                    None
                }
                0x4c => self.load(),
                0x53 => self.save(self.edit.focused),
                0x51 => Some(Action::Quit),
                0x44 if self.buttons.iter().any(|b| b.1 == Kind::Delete) => self.delete(),
                0x4e => Some(Action::NewGame),
                VK_UP => {
                    let l = &mut self.list;
                    if l.selected > 1 {
                        l.selected -= 1;
                        if l.selected < l.top {
                            l.top -= 1;
                        }
                        self.redraw_list();
                    }
                    None
                }
                VK_DOWN => {
                    let l = &mut self.list;
                    if l.selected < i32::try_from(l.items.len()).unwrap_or(i32::MAX) {
                        l.selected += 1;
                        if l.top + l.rows <= l.selected {
                            l.top += 1;
                        }
                        self.redraw_list();
                    }
                    None
                }
                _ => None,
            };
        } else if shift == ShiftState::ALT && key == VK_F4 {
            r.action = Some(Action::Close);
        }
        r
    }

    /// "Löschen": the selected game, if one is (the selection within the
    /// list). // en 1040:110c
    fn delete(&self) -> Option<Action> {
        match self.load()? {
            Action::Load(name) => Some(Action::Delete(name)),
            _ => None,
        }
    }

    /// "Laden": the selected game, if one is. // 1040:0f27
    fn load(&self) -> Option<Action> {
        let l = &self.list;
        if !l.has_selection() {
            return None;
        }
        let mut name = l.items.get(usize::try_from(l.selected - 1).unwrap_or(0))?.clone();
        name.extend_from_slice(crate::ladenform::EXTENSION);
        Some(Action::Load(name))
    }

    /// "Speichern": with the name field not `focused`, it focuses it;
    /// otherwise the name is checked and the game saved under it.
    /// // 1040:0e03
    fn save(&mut self, focused: bool) -> Option<Action> {
        if !focused {
            self.edit.focused = true;
            self.repaint_edit();
            return None;
        }
        if valid_name(&self.edit.text) {
            let mut name = self.edit.text.clone();
            name.extend_from_slice(b".map"); // 1040:0deb
            Some(Action::Save(name))
        } else {
            Some(Action::InvalidName)
        }
    }

    fn repaint_edit(&mut self) {
        let Dateimenue {
            font,
            bitmaps,
            empty,
            image,
            edit,
            ..
        } = self;
        let background = bitmaps.get(BACKGROUND.0).unwrap_or(empty);
        edit.repaint(image, background, &Skin { font, bitmaps });
    }

    fn redraw_list(&mut self) {
        let Dateimenue {
            font,
            bitmaps,
            image,
            list,
            ..
        } = self;
        list.draw(image, &Skin { font, bitmaps });
    }

    fn redraw_list_and_name(&mut self) {
        self.repaint_edit();
        self.redraw_list();
    }

    /// The mouse wheel over the form (`mouse_wheel`, a port extension):
    /// the list moves [`crate::widgets::WHEEL_STEPS`] rows a notch, `notches` up,
    /// as far as the arrows would take it.
    pub fn wheel(&mut self, notches: i32) -> Response {
        let mut r = Response::default();
        let top = self.list.top;
        for _ in 0..notches
            .unsigned_abs()
            .saturating_mul(crate::widgets::WHEEL_STEPS.unsigned_abs())
        {
            if notches < 0 && self.list.can_scroll_down() {
                self.list.top += 1;
            } else if notches > 0 && self.list.can_scroll_up() {
                self.list.top -= 1;
            }
        }
        if self.list.top != top {
            self.redraw_list();
            r.refresh = true;
        }
        r
    }

    /// An arrow's press or release: repaint, and its `Click` moves the
    /// list a row (down, 1040:0286, or up, 1040:0244). The arrows stay
    /// shown at either end.
    fn finish_arrow(&mut self, down: bool, out: Outcome, r: &mut Response) {
        let Dateimenue {
            font,
            bitmaps,
            image,
            up,
            list,
            down: down_button,
            ..
        } = self;
        let skin = Skin { font, bitmaps };
        if out.repaint {
            if down {
                down_button.draw(image, &skin);
            } else {
                up.draw(image, &skin);
            }
        }
        if out.clicked {
            if down && list.can_scroll_down() {
                list.top += 1;
                list.draw(image, &skin);
            } else if !down && list.can_scroll_up() {
                list.top -= 1;
                list.draw(image, &skin);
            }
        }
        r.refresh = true;
        r.sounds.extend(out.sound);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use formats::bmp::Bitmap;

    #[test]
    fn messages_by_land() {
        assert_eq!(invalid_name_text(Language::De), b"Dateiname ung\xfcltig");
        assert_eq!(invalid_name_text(Language::En), b"Invalid Filename");
        assert_eq!(delete_question(Language::En), b"Are you sure?");
        assert_eq!(delete_question(Language::De), b"Wirklich loeschen?");
    }

    fn font() -> Font {
        let (width, height) = (12, 14 * 12);
        let mut pixels = vec![0; width * height];
        pixels[..width * 14].fill(0xff);
        let bitmap = Bitmap {
            width,
            height,
            palette: [vcl::Rgb::default(); 256],
            pixels,
        };
        Font::new(&bitmap, &[[0; 8]; 32], &[[1; 8]; 32])
    }

    fn menu(games: usize) -> Dateimenue {
        let bitmaps = (0..BITMAP_FILES.len())
            .map(|i| Surface::new(210, 220, u8::try_from(i + 1).unwrap()))
            .collect();
        let texts = (0..24).map(|i| format!("t{i}").into_bytes()).collect();
        let mut m = Dateimenue::new(font(), bitmaps, texts, 305, 139);
        m.show((0..games).map(|i| format!("SPIEL{i}").into_bytes()).collect());
        m
    }

    fn click(m: &mut Dateimenue, x: i32, y: i32) -> Response {
        let down = m.mouse_down(x, y, true, ShiftState::LEFT);
        let up = m.mouse_up(x, y, true);
        if up.action.is_some() { up } else { down }
    }

    /// The title bar's drag: the form shrinks to it while held and is
    /// kept on the screen by it, so it can go off the bottom; with
    /// `form_drags` by its full height. 1040:073c, 1040:09ca, 1040:0a4c
    #[test]
    fn drags_by_the_title_bar() {
        for fixed in [false, true] {
            let mut m = menu(3);
            m.enhance.set(Enhancement::FormDrags, fixed);
            let screen = (800, 600);
            let _ = m.mouse_down(50, 5, true, ShiftState::LEFT);
            assert_eq!(m.bounds().height(), TITLE_HEIGHT);
            let r = m.mouse_move(50, 5 + 1000, ShiftState::LEFT, screen);
            assert!(r.moved);
            let bottom = if fixed { 600 - HEIGHT } else { 600 - TITLE_HEIGHT };
            assert_eq!(m.top, bottom, "fixed {fixed}");
            // A right release doesn't let it go; the left one does.
            let _ = m.mouse_up(50, 5, false);
            assert_eq!(m.bounds().height(), TITLE_HEIGHT);
            let _ = m.mouse_up(50, 5, true);
            assert_eq!(m.bounds(), Rect::new(m.left, bottom, WIDTH, HEIGHT));
        }
    }

    /// The later build's "Löschen" empties the name field. en 1040:110c
    #[test]
    fn clear_name_empties_the_field() {
        let mut m = menu(3);
        m.edit.text = b"SPIEL1".to_vec();
        m.repaint_edit();
        m.clear_name();
        assert_eq!(m.name(), b"");
        // The field shows the background there again.
        let f = m.edit.frame;
        let background = &m.bitmaps[BACKGROUND.0];
        for y in f.top..f.top + f.height {
            for x in f.left..f.left + f.width {
                let at = |s: &Surface| s.pixels[usize::try_from(y).unwrap() * s.width + usize::try_from(x).unwrap()];
                assert_eq!(at(&m.image), at(background), "{x}, {y}");
            }
        }
    }

    #[test]
    fn wheel_scrolls_the_list() {
        let mut m = menu(30);
        let rows = m.list.rows;
        assert!(m.wheel(-1).refresh);
        assert_eq!(m.list.top, 4);
        let _ = m.wheel(-100);
        assert_eq!(m.list.top, 30 - rows + 1);
        assert!(!m.wheel(-1).refresh, "at the end");
        let _ = m.wheel(100);
        assert_eq!(m.list.top, 1);
    }

    #[test]
    fn names() {
        for ok in [&b"a"[..], b"Spiel_1", b"ABCDEFGH"] {
            assert!(valid_name(ok), "{ok:?}");
        }
        for bad in [&b""[..], b"1abc", b"_x", b"ABCDEFGHI", b"a b", b"m\xfcll"] {
            assert!(!valid_name(bad), "{bad:?}");
        }
    }

    #[test]
    fn saves_only_with_enter() {
        let mut m = menu(2);
        assert_eq!(m.image.pixel(150, 100), 4, "the list's background");
        // "Speichern" focuses the name field.
        let save = (20, 75);
        let r = m.mouse_down(save.0, save.1, true, ShiftState::LEFT);
        assert_eq!(r.sounds, [Sound::Press]);
        assert_eq!(m.mouse_up(save.0, save.1, true).action, None);
        assert!(m.edit.focused);
        for key in [0x41, 0x42] {
            let _ = m.key_down(key, ShiftState::default());
        }
        assert_eq!(m.name(), b"ab");
        // Clicking it again: the press unfocuses, the release focuses again.
        assert_eq!(click(&mut m, save.0, save.1).action, None);
        assert!(m.edit.focused);
        // Enter saves.
        let r = m.key_down(VK_RETURN, ShiftState::default());
        assert_eq!(r.action, Some(Action::Save(b"ab.map".to_vec())));
        assert!(!m.edit.focused);
        // `S` with the field not focused focuses it.
        assert_eq!(m.key_down(0x53, ShiftState::default()).action, None);
        assert!(m.edit.focused);
        let _ = m.key_down(0xdb, ShiftState::default());
        assert_eq!(
            m.key_down(VK_RETURN, ShiftState::default()).action,
            Some(Action::InvalidName)
        );
    }

    #[test]
    fn loads_the_selected_game() {
        let mut m = menu(3);
        assert_eq!(m.key_down(0x4c, ShiftState::default()).action, None, "nothing selected");
        // The second row: its name goes into the field.
        let _ = m.mouse_down(130, 0x2e + 0xf + 2, true, ShiftState::LEFT);
        assert_eq!(m.name(), b"SPIEL1");
        assert_eq!(click(&mut m, 20, 55).action, Some(Action::Load(b"SPIEL1.map".to_vec())));
        // The arrow keys move the selection.
        let _ = m.key_down(VK_DOWN, ShiftState::default());
        assert_eq!(
            m.key_down(0x4c, ShiftState::default()).action,
            Some(Action::Load(b"SPIEL2.map".to_vec()))
        );
        assert_eq!(m.name(), b"SPIEL1");
        let r = m.mouse_down(130, 0x2e + 2 * 0xf + 2, true, ShiftState::LEFT | ShiftState::DOUBLE);
        assert_eq!(r.action, Some(Action::Load(b"SPIEL2.map".to_vec())));
    }

    #[test]
    fn buttons_and_keys() {
        let mut m = menu(0);
        assert_eq!(click(&mut m, 4, 4).action, Some(Action::Close));
        assert_eq!(click(&mut m, 20, 190).action, Some(Action::Close));
        assert_eq!(click(&mut m, 120, 190).action, Some(Action::Close));
        assert_eq!(click(&mut m, 20, 140).action, Some(Action::Quit));
        assert_eq!(click(&mut m, 20, 120).action, Some(Action::NewGame));
        // Let go elsewhere: nothing.
        let _ = m.mouse_down(20, 140, true, ShiftState::LEFT);
        assert_eq!(m.mouse_up(150, 10, true).action, None);
        assert_eq!(m.key_down(VK_ESCAPE, ShiftState::default()).action, Some(Action::Close));
        assert_eq!(m.key_down(VK_F4, ShiftState::ALT).action, Some(Action::Close));
        assert_eq!(m.key_down(0x4e, ShiftState::SHIFT).action, Some(Action::NewGame));
        assert_eq!(m.key_down(0x4e, ShiftState::CTRL).action, None);
        // Dragged by the title bar, and kept on the screen.
        let _ = m.mouse_down(50, 5, true, ShiftState::LEFT);
        let r = m.mouse_move(40, 0, ShiftState::LEFT, (800, 600));
        assert!(r.moved);
        assert_eq!((m.left, m.top), (295, 134));
        let _ = m.mouse_move(-2000, -2000, ShiftState::LEFT, (800, 600));
        assert_eq!((m.left, m.top), (0, 0));
    }

    #[test]
    fn saves_a_typed_name_with_the_fix() {
        let mut m = menu(3);
        m.enhance.set(Enhancement::EditField, true);
        let save = (20, 75);
        let _ = click(&mut m, save.0, save.1);
        for key in [0x41, 0x42] {
            let _ = m.key_down(key, ShiftState::default());
        }
        // "Speichern" again saves the typed name.
        assert_eq!(
            click(&mut m, save.0, save.1).action,
            Some(Action::Save(b"ab.map".to_vec()))
        );
        // A picked name puts the cursor after it: typing goes on at its end.
        let _ = m.mouse_down(130, 0x2e + 0xf + 2, true, ShiftState::LEFT);
        assert_eq!(m.name(), b"SPIEL1");
        m.edit.focused = true;
        let _ = m.key_down(0x43, ShiftState::default());
        assert_eq!(m.name(), b"SPIEL1c");
    }
}
