//! `Thighscoreform`, the high scores (segment `1040`): ten ranks for the
//! normal game and each of the six scenarios, from `SOUND\HIGHSCOR.WAV`
//! ([`formats::highscore`]). It opens from the end of a game's
//! "High-Score" button (1068:01b3) and from the options (1060:0175), on the
//! table of the scenario being played. Seven check boxes pick the table,
//! "Löschen" clears it (without asking) and "Schließen" or the close button
//! close the form.
//!
//! A fair win enters the score at the end of the game (1008:0512,
//! [`Highscoreform::add`]), with the player's name, today's date and their
//! net worth. The table is a file, not part of the world: the form keeps it,
//! and whenever [`Response::save`] says so the caller writes it back
//! ([`HighScores::to_bytes`]). The original writes it on every sort, so on
//! every showing too.

use formats::highscore::{Entry, GAMES, HighScores, RANKS};
use formats::pascal::PString;
use vcl::Rect;
use vcl::surface::{Rop, Surface};

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

/// The table entry `rank` of game `game` counted the way the original
/// does, through the file's layout: entry `rank * 7 + game`, so a game
/// past 6 reads the next rank's first tables. `None` past the file.
fn entry_mut(table: &mut HighScores, rank: usize, game: usize) -> Option<&mut Entry> {
    let i = rank * GAMES + game;
    table.entries.get_mut(i / GAMES).and_then(|r| r.get_mut(i % GAMES))
}

fn entry(table: &HighScores, rank: usize, game: usize) -> Option<&Entry> {
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
/// table. // 1040:1853
pub fn sort_highscores(table: &mut HighScores) {
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
/// entry, whatever it held, and the tables are sorted. // 1040:1bdf
pub fn add_highscore(table: &mut HighScores, game: u8, name: &[u8], date: &[u8], money: &[u8], score: i32) {
    // BUG(original): a game past 6 writes past the end of the table (game 7's last rank is 67 bytes after it); the port writes nothing.
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
pub fn reset_highscores(table: &mut HighScores, game: u8) {
    for rank in 0..RANKS {
        if let Some(e) = entry_mut(table, rank, usize::from(game)) {
            e.score = 0;
            e.name.0[0] = 0;
            e.date.0[0] = 0;
            e.money_text.0[0] = 0;
        }
    }
}

/// Today's date as Delphi 1's `DateToStr` writes it with Windows'
/// German short date format (`dd.MM.yy`), for a new entry. The original
/// takes the date from the clock (1008:0512); the caller passes it.
#[must_use]
pub fn date_text(day: u32, month: u32, year: i32) -> Vec<u8> {
    format!("{day:02}.{month:02}.{:02}", year.rem_euclid(100)).into_bytes()
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
    /// (`save_highscores`, 1040:2517).
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
    pub table: HighScores,
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
    /// The tables' check boxes, by [`BOXES`]. // +0x4d2 .. +0xb6e
    boxes: [CheckBox; 7],
}

impl Highscoreform {
    /// The form as it was created, not shown, with `HIGHSCOR.TXT`'s
    /// lines and the table (`load_highscor_txt`, `load_highscores`,
    /// 1040:3183). `bitmaps` are those of [`BITMAP_FILES`].
    #[must_use]
    pub fn new(font: Font, bitmaps: Vec<Surface>, texts: Vec<Vec<u8>>, table: HighScores) -> Self {
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
            boxes: std::array::from_fn(|_| {
                CheckBox::new(
                    Label::new(Frame::new(0, 0, 0, 0), b"", Align::Left, 9),
                    CHECK,
                    CHECKED,
                    10,
                )
            }),
        };
        f.init_widgets();
        f
    }

    fn text(&self, n: usize) -> Vec<u8> {
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
        self.boxes = std::array::from_fn(|i| {
            let (left, top) = BOXES[i];
            let label = Label::new(Frame::new(left, top, 0xa0, 0x10), &self.text(4 + i), Align::Left, 9);
            CheckBox::new(label, CHECK, CHECKED, 10)
        });
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
    /// box ticked and its table shown (a scenario past 6 ticks none and
    /// keeps the table shown last), the background with the widgets, the
    /// texts and the table. `from_options` is the options' flag
    /// (1060:0175). The caller enables this form and disables the one it
    /// came from.
    pub fn show(&mut self, scenario: u8, from_options: bool, screen: (i32, i32)) -> Response {
        sort_highscores(&mut self.table);
        self.init_widgets();
        self.top = (screen.1 - HEIGHT) / 2;
        self.left = (screen.0 - WIDTH) / 2;
        if scenario < 7 {
            let i = (usize::from(scenario) + 6) % 7;
            self.boxes[i].radio.on = true;
            // Its `Click` before anything is drawn.
            self.game = scenario;
        }
        let clip = self.image.bounds();
        let background = self.bitmaps.get(BACKGROUND.0).unwrap_or(&self.empty);
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
        let texts = [
            (0, 0x10, 0, 0x270, 1),
            (3, 0, 0xdc, 0x280, 9),
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
        if let Some(src) = self.bitmaps.get(BACKGROUND.0) {
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
                // BUG(original): a game past 6 reads the next ranks' tables and, for the last rank, past the end of the table; the port shows nothing there.
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
    /// drawn ticked. // 1040:1f38 .. 1040:23d6
    fn select_box(&mut self, i: usize) {
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
        self.game = u8::try_from((i + 1) % 7).unwrap_or(0);
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

    fn table() -> HighScores {
        HighScores::parse(&vec![0; formats::highscore::SIZE]).unwrap()
    }

    fn scores(t: &HighScores, game: usize) -> Vec<i32> {
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
        // Past the tables: nothing is written.
        add_highscore(&mut t, 7, b"tut", b"", b"", 99);
        assert!(t.entries.iter().flatten().all(|e| e.name.as_bytes() != b"tut"));
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
        assert_eq!(date_text(4, 9, 1996), b"04.09.96");
    }
}
