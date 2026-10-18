//! `Thighscoreform`, the high scores (segment `1040`, the later build's
//! en `1068`): ten ranks for the normal game and each scenario. v1.0 has
//! seven tables, the normal game's and the six scenarios', in
//! `SOUND\HIGHSCOR.WAV` ([`formats::highscore`]); the later build keeps
//! fourteen in memory, `0x3aa` bytes a rank ([`ScoreTable`]): 0 to 6 in
//! that file as before, the scenarios 8 to 13 of its `UPGRADE\` folder
//! in `UPGRADE\HIGHSCO2.WAV`, and 7 (the tutorial's number) nowhere. The
//! port follows it (`docs/versions.md`). It opens from the end of a
//! game's "High-Score" button (1068:01b3) and from the options
//! (1060:0175), on the table of the scenario being played. Seven check
//! boxes pick the table (thirteen with `UPGRADE\`), "Löschen" clears it
//! (without asking) and "Schließen" or the close button close the form.
//!
//! A fair win enters the score at the end of the game (1008:0512,
//! [`Highscoreform::add`]), with the player's name, today's date and their
//! net worth. The table is a file, not part of the world: the form keeps it,
//! and whenever [`Response::save`] says so the caller writes it back
//! ([`HighScores::to_bytes`]). The original writes it on every sort, so on
//! every showing too.

use formats::highscore::{Entry, HighScores, RANKS};
use formats::pascal::PString;
use vcl::Rect;
use vcl::surface::{Rop, Surface};

use crate::lang::Language;
use crate::text::{Align, Font};
use crate::widgets::{BitmapId, Button, CheckBox, Frame, Label, ShiftState, Skin, Sound, Widget};

/// The bitmaps the form draws with, from `GRAFIK`, in the order
/// [`Highscoreform::new`] takes them.
pub const BITMAP_FILES: [&str; 7] = [
    "highscor.bmp", // the background, +0xc88, 1040:2ba4
    "highbu.bmp",   // a button, +0x180, 1040:2bb1
    "highbupr.bmp", // a button held down, +0x184, 1040:2bbc
    "strakn.bmp",   // a check box, +0x188, 1040:2bc9
    "straknpr.bmp", // a check box ticked, +0x18c, 1040:2bd4
    // hintergrund's
    "close.bmp",    // 1110:54b8
    "closepre.bmp", // 1110:54bc
];
const BACKGROUND: BitmapId = BitmapId(0);
const BUTTON: BitmapId = BitmapId(1);
const BUTTON_PRESSED: BitmapId = BitmapId(2);
const CHECK: BitmapId = BitmapId(3);
const CHECKED: BitmapId = BitmapId(4);
const CLOSE: BitmapId = BitmapId(5);
const CLOSE_PRESSED: BitmapId = BitmapId(6);

/// With the later build's `UPGRADE\` folder, its bitmaps after
/// [`BITMAP_FILES`]: the background with room for thirteen boxes and
/// their smaller check box. // en 1068:29ac
pub const UPGRADE_BITMAP_FILES: [&str; 3] = ["highsco2.bmp", "strakn2.bmp", "strakn2p.bmp"];
const BACKGROUND2: BitmapId = BitmapId(7);
const CHECK2: BitmapId = BitmapId(8);
const CHECKED2: BitmapId = BitmapId(9);

/// The games the later build keeps a table for: the normal game, the
/// scenarios 1 to 6, the tutorial's 7 and the scenarios 8 to 13.
/// // en 1068:0109 (`0x3aa` a rank)
pub const GAMES: usize = 14;
/// The games of each file: `SOUND\HIGHSCOR.WAV` and
/// `UPGRADE\HIGHSCO2.WAV`.
const FILE_GAMES: usize = formats::highscore::GAMES;
/// The first game the second file keeps, in its slot 1.
const SECOND_FILE_FIRST: usize = 8;

/// An empty entry: no texts, score 0.
const EMPTY: Entry = Entry {
    name: PString([0; 21]),
    date: PString([0; 21]),
    score: 0,
    money_text: PString([0; 21]),
};

/// The later build's table of [`GAMES`] games: `entries[rank][game]`, as
/// it lies in memory. // `*1110:6ee4`, en 1068:0109
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScoreTable {
    pub entries: [[Entry; GAMES]; RANKS],
}

impl Default for ScoreTable {
    fn default() -> Self {
        ScoreTable {
            entries: [[EMPTY; GAMES]; RANKS],
        }
    }
}

impl ScoreTable {
    /// The table from the files: games 0 to 6 from `first`
    /// (`SOUND\HIGHSCOR.WAV`), 8 to 13 from the slots 1 to 6 of `second`
    /// (`UPGRADE\HIGHSCO2.WAV`) if there is one; the rest empty.
    /// // en 1068:1adf
    #[must_use]
    pub fn new(first: &HighScores, second: Option<&HighScores>) -> Self {
        let mut t = ScoreTable::default();
        for (rank, row) in t.entries.iter_mut().enumerate() {
            row[..FILE_GAMES].copy_from_slice(&first.entries[rank]);
            if let Some(second) = second {
                row[SECOND_FILE_FIRST..].copy_from_slice(&second.entries[rank][1..]);
            }
        }
        t
    }

    /// `SOUND\HIGHSCOR.WAV`: the games 0 to 6. // en 1068:1e08
    #[must_use]
    pub fn first_file(&self) -> HighScores {
        HighScores {
            entries: std::array::from_fn(|rank| std::array::from_fn(|g| self.entries[rank][g])),
        }
    }

    /// `UPGRADE\HIGHSCO2.WAV`: the games 8 to 13 in the slots 1 to 6, and
    /// in slot 0 game 0's, which the original's buffer still holds from
    /// writing the first file. Game 7 isn't saved. // en 1068:1e08
    #[must_use]
    pub fn second_file(&self) -> HighScores {
        HighScores {
            entries: std::array::from_fn(|rank| {
                std::array::from_fn(|slot| {
                    if slot == 0 {
                        self.entries[rank][0]
                    } else {
                        self.entries[rank][SECOND_FILE_FIRST - 1 + slot]
                    }
                })
            }),
        }
    }
}

/// The form's size, and its image's. // 1040:2bf0
pub const WIDTH: i32 = 0x280;
pub const HEIGHT: i32 = 300;
/// The image's name in the DFM.
pub const IMAGE: &str = "bild";

/// The table's first row and the rows' height. // 1040:1d49
const TABLE_TOP: i32 = 0x30;
const ROW_HEIGHT: i32 = 0x10;
/// The columns' left edges, each 128 wide: rank, name, date, score, money.
const COLUMNS: [i32; 5] = [0, 0x80, 0x100, 0x180, 0x200];
/// What the tenth rank shows instead of its number. // 1040:1d45
// BUG(original): the tenth rank shows "..." instead of 10.
const TENTH: &[u8] = b"...";

/// The check boxes' positions, by box: box `i` shows table `(i + 1) % 7`
/// (the normal game last) with `HIGHSCOR.TXT`'s text `4 + i`. // 1040:2bf0
const BOXES: [(i32, i32); 7] = [
    (8, 0xed),
    (8, 0x101),
    (8, 0x115),
    (0xc0, 0xed),
    (0xc0, 0x101),
    (0xc0, 0x115),
    (0x188, 0xed),
];

/// The later build's thirteen boxes: left, top (each 0x7e by 0xd), the
/// game and its text, by box: games 1 to 6 with `HIGHSCOR.TXT`'s texts 4
/// to 9, 8 to 13 with `HIGHSCO2.TXT`'s six (here 16 to 21), the normal
/// game with text 10. // en 1068:29ac
const UPGRADE_BOXES: [(i32, i32, u8, usize); 13] = [
    (3, 0xe6, 1, 4),
    (3, 0xf7, 2, 5),
    (3, 0x108, 3, 6),
    (3, 0x119, 4, 7),
    (0x81, 0xe6, 5, 8),
    (0x81, 0xf7, 6, 9),
    (0x81, 0x108, 8, 16),
    (0x81, 0x119, 9, 17),
    (0x109, 0xe6, 10, 18),
    (0x109, 0xf7, 11, 19),
    (0x109, 0x108, 12, 20),
    (0x109, 0x119, 13, 21),
    (0x1a5, 0xe6, 0, 10),
];
/// The table entry `rank` of game `game` counted the way the original
/// does, through the memory's layout: entry `rank * 14 + game`, so a game
/// past 13 reads the next rank's first tables. `None` past the table.
fn entry_mut(table: &mut ScoreTable, rank: usize, game: usize) -> Option<&mut Entry> {
    let i = rank * GAMES + game;
    table.entries.get_mut(i / GAMES).and_then(|r| r.get_mut(i % GAMES))
}

fn entry(table: &ScoreTable, rank: usize, game: usize) -> Option<&Entry> {
    let i = rank * GAMES + game;
    table.entries.get(i / GAMES).and_then(|r| r.get(i % GAMES))
}

/// `s := text` into a `string[20]`: the length and the characters, the
/// bytes after them kept (the RTL's `PStrStore` with 20).
fn store(s: &mut PString<21>, text: &[u8]) {
    let n = text.len().min(20);
    s.0[0] = u8::try_from(n).unwrap_or(20);
    s.0[1..=n].copy_from_slice(&text[..n]);
}

/// Sorts every table by score, highest first: ten bubble passes, an
/// entry swapped with the next when its score is lower, so equal scores
/// keep their order. The names, dates and money texts are swapped as
/// strings, leaving the bytes after them. Then the original saves the
/// table. All fourteen games' in the later build. // 1040:1853, en 1068:0109
pub fn sort_highscores(table: &mut ScoreTable) {
    for game in 0..GAMES {
        for _ in 0..RANKS {
            for rank in 0..RANKS - 1 {
                let (a, b) = (table.entries[rank][game], table.entries[rank + 1][game]);
                if a.score < b.score {
                    let (mut x, mut y) = (a, b);
                    x.score = b.score;
                    y.score = a.score;
                    store(&mut x.name, b.name.as_bytes());
                    store(&mut y.name, a.name.as_bytes());
                    store(&mut x.money_text, b.money_text.as_bytes());
                    store(&mut y.money_text, a.money_text.as_bytes());
                    store(&mut x.date, b.date.as_bytes());
                    store(&mut y.date, a.date.as_bytes());
                    table.entries[rank][game] = x;
                    table.entries[rank + 1][game] = y;
                }
            }
        }
    }
}

/// `add_highscore`: the last rank of `game`'s table becomes the new
/// entry, whatever it held, and the tables are sorted. v1.0's table had
/// seven games, so a won game of the tutorial's 7 wrote 67 bytes past its
/// end (1040:1bdf); the later build's has fourteen, game 7's entry staying
/// in memory. // en 1068:0495
pub fn add_highscore(table: &mut ScoreTable, game: u8, name: &[u8], date: &[u8], money: &[u8], score: i32) {
    // BUG(original): a game past 13 writes past the end of the table; the port writes nothing.
    if let Some(e) = entry_mut(table, RANKS - 1, usize::from(game)) {
        e.score = score;
        store(&mut e.name, name);
        store(&mut e.date, date);
        store(&mut e.money_text, money);
    }
    sort_highscores(table);
}

/// `reset_highscores`: `game`'s ten entries get score 0 and empty
/// strings; the characters stay behind the lengths. // 1040:1ca7
pub fn reset_highscores(table: &mut ScoreTable, game: u8) {
    for rank in 0..RANKS {
        if let Some(e) = entry_mut(table, rank, usize::from(game)) {
            e.score = 0;
            e.name.0[0] = 0;
            e.date.0[0] = 0;
            e.money_text.0[0] = 0;
        }
    }
}

/// Today's date as Delphi 1's `DateToStr` writes it with Windows' short
/// date format, for a new entry. That's the Windows the game runs on,
/// not the game's: the port takes the German Windows' `dd.MM.yy` for
/// German and the British one's `dd/MM/yy` for English
/// (`docs/language.md`). The original takes the date from the clock
/// (1008:0512); the caller passes it.
#[must_use]
pub fn date_text(day: u32, month: u32, year: i32, language: Language) -> Vec<u8> {
    let sep = language.pick('.', '/');
    format!("{day:02}{sep}{month:02}{sep}{:02}", year.rem_euclid(100)).into_bytes()
}

/// `HIGHSCOR.TXT`'s 16 lines (`load_highscor_txt`); missing lines are
/// empty. // 1040:284c
#[must_use]
pub fn highscor_lines(data: &[u8]) -> Vec<Vec<u8>> {
    let mut f = formats::pascal::TextReader::new(data);
    (0..16).map(|_| f.read_line()).collect()
}

/// What the form asks of the game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// "Schließen" or the close button: the form hides, and the form it
    /// came from is enabled again: the end of the game's, or the options'
    /// if [`Highscoreform::from_options`]. // 1040:1bc2, 1040:1ba5, 1040:1b5c
    Close,
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
    /// The table changed: the caller writes it to `SOUND\HIGHSCOR.WAV`
    /// (and with `UPGRADE\` to `HIGHSCO2.WAV`, [`ScoreTable::first_file`],
    /// [`ScoreTable::second_file`]; `save_highscores`, 1040:2517,
    /// en 1068:1e08).
    pub save: bool,
}

/// The high score form.
#[derive(Debug, Clone)]
pub struct Highscoreform {
    font: Font,
    bitmaps: Vec<Surface>,
    empty: Surface,
    /// `HIGHSCOR.TXT`'s 16 lines. // *1110:6edc..
    texts: Vec<Vec<u8>>,
    /// The table. // *1110:6ee4
    pub table: ScoreTable,
    /// The table shown. // 1110:6ee8
    pub game: u8,
    /// `bild`'s picture.
    image: Surface,
    pub visible: bool,
    /// The form's top left on the screen.
    pub left: i32,
    pub top: i32,
    /// The options opened it. // +0xc8c
    pub from_options: bool,
    /// "Schließen", the close button, "Löschen". // +0x190, +0x2a6, +0x3bc
    buttons: [Button; 3],
    /// The tables' check boxes, by [`BOXES`]; with `UPGRADE\` by
    /// [`UPGRADE_BOXES`]. // +0x4d2 .. +0xb6e, en +0x4d2 .. +0x120a
    boxes: Vec<CheckBox>,
    /// The later build's six scenario names from `UPGRADE\HIGHSCO2.TXT`,
    /// with its thirteen boxes; none for v1.0's seven. // en 1068:2493
    upgrade_names: Option<Vec<Vec<u8>>>,
}

impl Highscoreform {
    /// The form as it was created, not shown, with `HIGHSCOR.TXT`'s
    /// lines and the table (`load_highscor_txt`, `load_highscores`,
    /// 1040:3183). `bitmaps` are those of [`BITMAP_FILES`], then with
    /// `UPGRADE\` those of [`UPGRADE_BITMAP_FILES`] for
    /// [`Highscoreform::with_upgrade`].
    #[must_use]
    pub fn new(font: Font, bitmaps: Vec<Surface>, texts: Vec<Vec<u8>>, table: ScoreTable) -> Self {
        let mut f = Highscoreform {
            font,
            bitmaps,
            empty: Surface::new(0, 0, 0),
            texts,
            table,
            game: 0,
            image: Surface::new(WIDTH as usize, HEIGHT as usize, 0),
            visible: false,
            left: 0,
            top: 0,
            from_options: false,
            buttons: std::array::from_fn(|_| Button::new(Frame::new(0, 0, 0, 0), BUTTON, BUTTON_PRESSED, b"", 9)),
            boxes: Vec::new(),
            upgrade_names: None,
        };
        f.init_widgets();
        f
    }

    /// The later build's form, with `UPGRADE\`: `names` are
    /// `HIGHSCO2.TXT`'s six lines, and the form has thirteen boxes on
    /// `highsco2.bmp`. // en 1068:29ac, en 1068:2493
    #[must_use]
    pub fn with_upgrade(mut self, names: Vec<Vec<u8>>) -> Self {
        self.upgrade_names = Some(names);
        self.init_widgets();
        self
    }

    /// The game each box shows.
    fn box_game(&self, i: usize) -> u8 {
        if self.upgrade_names.is_some() {
            UPGRADE_BOXES.get(i).map_or(0, |b| b.2)
        } else {
            u8::try_from((i + 1) % 7).unwrap_or(0)
        }
    }

    /// The background picture: `highscor.bmp`, or with `UPGRADE\` its
    /// `highsco2.bmp`.
    fn background(&self) -> BitmapId {
        if self.upgrade_names.is_some() {
            BACKGROUND2
        } else {
            BACKGROUND
        }
    }

    fn text(&self, n: usize) -> Vec<u8> {
        if n >= 16
            && let Some(names) = &self.upgrade_names
        {
            return names.get(n - 16).cloned().unwrap_or_default();
        }
        self.texts.get(n).cloned().unwrap_or_default()
    }

    /// The widgets as `1040:2bf0` makes them.
    fn init_widgets(&mut self) {
        self.buttons = [
            Button::new(
                Frame::new(0x210, 0x109, 100, 0x14),
                BUTTON,
                BUTTON_PRESSED,
                &self.text(1),
                9,
            ),
            Button::new(Frame::new(0, 0, 0x10, 0x10), CLOSE, CLOSE_PRESSED, b"", 9),
            Button::new(
                Frame::new(0x1a5, 0x109, 100, 0x14),
                BUTTON,
                BUTTON_PRESSED,
                &self.text(2),
                9,
            ),
        ];
        self.boxes = if self.upgrade_names.is_some() {
            UPGRADE_BOXES
                .iter()
                .map(|&(left, top, _, text)| {
                    let label = Label::new(Frame::new(left, top, 0x7e, 0xd), &self.text(text), Align::Left, 9);
                    CheckBox::new(label, CHECK2, CHECKED2, 10)
                })
                .collect()
        } else {
            BOXES
                .iter()
                .enumerate()
                .map(|(i, &(left, top))| {
                    let label = Label::new(Frame::new(left, top, 0xa0, 0x10), &self.text(4 + i), Align::Left, 9);
                    CheckBox::new(label, CHECK, CHECKED, 10)
                })
                .collect()
        };
    }

    /// The image's picture.
    #[must_use]
    pub fn image(&self) -> &Surface {
        &self.image
    }

    /// The form on the screen.
    #[must_use]
    pub fn bounds(&self) -> Rect {
        Rect::new(self.left, self.top, WIDTH, HEIGHT)
    }

    /// The end of a fair win enters the score into `scenario`'s table and
    /// shows that table next. // 1040:1bdf
    pub fn add(&mut self, scenario: u8, name: &[u8], date: &[u8], money: &[u8], score: i32) -> Response {
        self.game = scenario;
        add_highscore(&mut self.table, scenario, name, date, money, score);
        Response {
            save: true,
            ..Response::default()
        }
    }

    /// `FormShow` (1040:2579, with 1040:2bf0): the tables sorted (and
    /// saved), new widgets, the form centred on the `screen`, `scenario`'s
    /// box ticked and its table shown (a scenario without a box, the
    /// tutorial's 7 or past the boxes, ticks none and keeps the table
    /// shown last), the background with the widgets, the texts and the
    /// table. `from_options` is the options' flag (1060:0175). The caller
    /// enables this form and disables the one it came from.
    /// // 1040:2579, en 1068:2117, en 1068:29ac
    pub fn show(&mut self, scenario: u8, from_options: bool, screen: (i32, i32)) -> Response {
        sort_highscores(&mut self.table);
        self.init_widgets();
        self.top = (screen.1 - HEIGHT) / 2;
        self.left = (screen.0 - WIDTH) / 2;
        if let Some(i) = (0..self.boxes.len()).find(|&i| self.box_game(i) == scenario) {
            self.boxes[i].radio.on = true;
            // Its `Click` before anything is drawn.
            self.game = scenario;
        }
        let clip = self.image.bounds();
        let background = self.bitmaps.get(self.background().0).unwrap_or(&self.empty);
        self.image.blit(0, 0, background, clip, Rop::Copy, clip);
        {
            let Highscoreform {
                font,
                bitmaps,
                image,
                buttons,
                boxes,
                ..
            } = self;
            let skin = Skin { font, bitmaps };
            for b in buttons.iter_mut() {
                b.show(image, &skin);
            }
            for b in boxes.iter_mut() {
                b.show(image, &skin);
            }
        }
        let font = &self.font;
        // The later build draws "Szenarien" six rows higher. // en 1068:2117
        let heading = if self.upgrade_names.is_some() { 0xd6 } else { 0xdc };
        let texts = [
            (0, 0x10, 0, 0x270, 1),
            (3, 0, heading, 0x280, 9),
            (11, COLUMNS[0], 0x20, 0x80, 0xb),
            (12, COLUMNS[1], 0x20, 0x80, 0xb),
            (13, COLUMNS[2], 0x20, 0x80, 0xb),
            (14, COLUMNS[3], 0x20, 0x80, 0xb),
            (15, COLUMNS[4], 0x20, 0x80, 0xb),
        ];
        for (n, x, y, width, colour) in texts {
            let text = self.texts.get(n).map_or(&[][..], |t| &t[..]);
            font.draw(&mut self.image, text, (x, y), width, Align::Center, colour);
        }
        self.draw_table();
        self.visible = true;
        self.from_options = from_options;
        Response {
            refresh: true,
            save: true,
            ..Response::default()
        }
    }

    /// `highscoreform_draw_table`: the background's rows under the table,
    /// then each rank's number (the tenth "..."), name, date, score and
    /// money, centred in their columns. // 1040:1d49
    fn draw_table(&mut self) {
        let clip = self.image.bounds();
        if let Some(src) = self.bitmaps.get(self.background().0) {
            let rows = Rect::new(0, TABLE_TOP, WIDTH, 0xa0);
            self.image.blit(0, TABLE_TOP, src, rows, Rop::Copy, clip);
        }
        for rank in 0..RANKS {
            let y = i32::try_from(rank).unwrap_or(0) * ROW_HEIGHT + TABLE_TOP;
            let number = if rank == RANKS - 1 {
                TENTH.to_vec()
            } else {
                (rank + 1).to_string().into_bytes()
            };
            let (name, date, score, money) = match entry(&self.table, rank, usize::from(self.game)) {
                Some(e) => (
                    e.name.as_bytes().to_vec(),
                    e.date.as_bytes().to_vec(),
                    e.score.to_string().into_bytes(),
                    e.money_text.as_bytes().to_vec(),
                ),
                // BUG(original): a game past 13 (v1.0: past 6) reads the next ranks' tables and, for the last rank, past the end of the table; the port shows nothing there.
                None => Default::default(),
            };
            for (x, text) in COLUMNS.into_iter().zip([number, name, date, score, money]) {
                self.font.draw(&mut self.image, &text, (x, y), 0x80, Align::Center, 9);
            }
        }
    }

    /// `bildMouseDown`: with the left button, the buttons, then the check
    /// boxes (as radio buttons: a ticked one ignores the click) are offered
    /// the press until one takes it. A box that ticks shows its table and
    /// unticks the others. // 1040:31bc
    pub fn mouse_down(&mut self, x: i32, y: i32, shift: ShiftState) -> Response {
        let mut r = Response::default();
        if !shift.contains(ShiftState::LEFT) {
            return r;
        }
        for i in 0..self.buttons.len() {
            let out = self.buttons[i].mouse_down(x, y);
            if out.handled {
                self.redraw_button(i);
                r.sounds.extend(out.sound);
                r.refresh = true;
                return r;
            }
        }
        for i in 0..self.boxes.len() {
            let out = self.boxes[i].radio.mouse_down(x, y);
            if out.handled {
                r.sounds.extend(out.sound);
                r.refresh = true;
                self.select_box(i);
                return r;
            }
        }
        r
    }

    /// A box's `Click`: the others unticked, its table shown. Then it's
    /// drawn ticked. // 1040:1f38 .. 1040:23d6, en 1068:07ee .. en 1068:1952
    fn select_box(&mut self, i: usize) {
        let game = self.box_game(i);
        let Highscoreform {
            font,
            bitmaps,
            image,
            boxes,
            ..
        } = self;
        let skin = Skin { font, bitmaps };
        for (j, b) in boxes.iter_mut().enumerate() {
            if j != i && b.radio.deselect().repaint {
                b.draw(image, &skin);
            }
        }
        self.game = game;
        self.draw_table();
        let Highscoreform {
            font,
            bitmaps,
            image,
            boxes,
            ..
        } = self;
        boxes[i].draw(image, &Skin { font, bitmaps });
    }

    /// `bildMouseUp`: the buttons are offered the release until one takes
    /// it; "Schließen" and the close button close the form, "Löschen"
    /// clears the table shown. // 1040:3347
    pub fn mouse_up(&mut self, x: i32, y: i32) -> Response {
        let mut r = Response::default();
        for i in 0..self.buttons.len() {
            let out = self.buttons[i].mouse_up(x, y);
            if !out.handled {
                continue;
            }
            self.redraw_button(i);
            r.refresh = true;
            if out.clicked {
                if i == 2 {
                    // BUG(original): "Löschen" clears the table without asking.
                    reset_highscores(&mut self.table, self.game);
                    self.draw_table();
                    r.save = true;
                } else {
                    self.visible = false;
                    r.action = Some(Action::Close);
                }
            }
            r.sounds.extend(out.sound);
            break;
        }
        r
    }

    fn redraw_button(&mut self, i: usize) {
        let Highscoreform {
            font,
            bitmaps,
            image,
            buttons,
            ..
        } = self;
        buttons[i].draw(image, &Skin { font, bitmaps });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> ScoreTable {
        ScoreTable::default()
    }

    fn scores(t: &ScoreTable, game: usize) -> Vec<i32> {
        (0..RANKS).map(|r| t.entries[r][game].score).collect()
    }

    #[test]
    fn adds_in_order() {
        let mut t = table();
        for (n, s) in [(b"a", 50), (b"b", 70), (b"c", 50), (b"d", 0)] {
            add_highscore(&mut t, 2, n, b"01.02.96", b"1.000", s);
        }
        assert_eq!(scores(&t, 2), [70, 50, 50, 0, 0, 0, 0, 0, 0, 0]);
        let names: Vec<_> = (0..3).map(|r| t.entries[r][2].name.as_bytes().to_vec()).collect();
        assert_eq!(
            names,
            [b"b".to_vec(), b"a".to_vec(), b"c".to_vec()],
            "equal scores keep their order"
        );
        // A full table: the last rank goes, even for a lower score.
        for s in 1..=10 {
            add_highscore(&mut t, 2, b"x", b"", b"", s * 10);
        }
        assert_eq!(scores(&t, 2), [100, 90, 80, 70, 70, 60, 50, 50, 50, 40]);
        add_highscore(&mut t, 2, b"low", b"", b"", 5);
        assert_eq!(t.entries[9][2].name.as_bytes(), b"low");
        assert_eq!(scores(&t, 0), [0; 10]);
        // The tutorial's 7 has a table of its own in the later build.
        add_highscore(&mut t, 7, b"tut", b"", b"", 99);
        assert_eq!(t.entries[0][7].name.as_bytes(), b"tut");
        // Past the tables: nothing is written.
        add_highscore(&mut t, 14, b"far", b"", b"", 99);
        assert!(t.entries.iter().flatten().all(|e| e.name.as_bytes() != b"far"));
    }

    #[test]
    fn files() {
        let mut t = table();
        for (game, name) in [
            (0u8, &b"normal"[..]),
            (6, b"oeko"),
            (7, b"tutorial"),
            (8, b"north"),
            (13, b"pro"),
        ] {
            add_highscore(&mut t, game, name, b"", b"", 50);
        }
        let (first, second) = (t.first_file(), t.second_file());
        assert_eq!(first.entries[0][6].name.as_bytes(), b"oeko");
        assert_eq!(second.entries[0][0].name.as_bytes(), b"normal");
        assert_eq!(second.entries[0][1].name.as_bytes(), b"north");
        assert_eq!(second.entries[0][6].name.as_bytes(), b"pro");
        let back = ScoreTable::new(&first, Some(&second));
        assert_eq!(back.entries[0][13].name.as_bytes(), b"pro");
        // Game 7 isn't saved.
        assert_eq!(back.entries[0][7].score, 0);
        // Without the second file, 8 to 13 are empty.
        let v10 = ScoreTable::new(&first, None);
        assert_eq!((v10.entries[0][0].score, v10.entries[0][8].score), (50, 0));
    }

    #[test]
    fn swaps_keep_the_bytes_behind() {
        let mut t = table();
        t.entries[0][1].name = PString::new(b"longername");
        t.entries[1][1].name = PString::new(b"ab");
        t.entries[1][1].score = 5;
        sort_highscores(&mut t);
        assert_eq!(t.entries[0][1].name.as_bytes(), b"ab");
        assert_eq!(&t.entries[0][1].name.0[..6], b"\x02abnge".as_ref());
    }

    #[test]
    fn resets_one_table() {
        let mut t = table();
        add_highscore(&mut t, 3, b"me", b"d", b"m", 42);
        add_highscore(&mut t, 4, b"you", b"d", b"m", 41);
        reset_highscores(&mut t, 3);
        assert_eq!(t.entries[0][3].score, 0);
        assert_eq!(t.entries[0][3].name.as_bytes(), b"");
        assert_eq!(t.entries[0][3].name.0[1..3], *b"me", "the characters stay");
        assert_eq!(t.entries[0][4].score, 41);
        assert_eq!(date_text(4, 9, 1996, Language::De), b"04.09.96");
        assert_eq!(date_text(4, 9, 2001, Language::En), b"04/09/01");
    }
}
