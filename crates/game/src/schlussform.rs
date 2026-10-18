//! `TSchlussform`, the end (segment `1068`). Two ways in:
//!
//! - The scenario is won or lost on the 2nd of January (1008:0512, the
//!   world's [`Tick::game_over`](crate::simulation::Tick::game_over)):
//!   [`Schlussform::open_end`] (1068:1016) plays the scenario's film
//!   (`ABgeld.avi`, ..., `ABPleite.avi`) with `gewonn.mid` or
//!   `pleite.mid`, then shows the results page: `Schluss.txt`'s verdict,
//!   the net worth, the last year's guests, the score, and the buttons
//!   "Weiterspielen" (only after a fair win), "High-Score", "Beenden" and
//!   "Neues Spiel".
//! - "OK" on `TEndeform` (or "Beenden" on the results page):
//!   [`Schlussform::open_quit`] (1068:0f52, 1068:0e68) plays `ABEnde.avi`,
//!   after which the game exits (1068:17ad: everything freed, `game_done`,
//!   `Halt`).
//!
//! The films and the music (`TMediaPlayer`) are the caller's: the
//! [`Response`] names the file to play, and the caller reports the end
//! with [`Schlussform::film_ended`] (the original's `Timer1` sees the
//! film's position reach its length, 1068:187f). A key, a click on the
//! film or on the background form skip it (1068:198a, 1068:199b,
//! 1000:3d0c). Without a film player the caller ends it at once.
//!
//! The end of the game stops the clock and disables the other forms until
//! the player picks a button: the simulation waits on this dialog. Single
//! human: the original shows the human's (player 1's) results and stops
//! the game for everyone.

use vcl::Rect;
use vcl::surface::{Rop, Surface};

use crate::lang::Language;
use crate::money::format_money;
use crate::simulation::GameOver;
use crate::text::{Align, Font};
use crate::widgets::{BitmapId, Button, Frame, Label, Skin, Sound, Widget};
use crate::world::World;

/// The bitmaps the form draws with, from `GRAFIK`, in the order
/// [`Schlussform::new`] takes them.
pub const BITMAP_FILES: [&str; 6] = [
    "schluss.bmp",  // the results page, +0x194, 1068:1334
    "schlrahm.bmp", // the film's frame, +0x198, 1068:097f
    "schlkn.bmp",   // a button, +0x19c, 1068:098c
    "schlknpr.bmp", // a button held down, +0x1a0, 1068:0997
    // hintergrund's
    "close.bmp",    // 1110:54b8
    "closepre.bmp", // 1110:54bc
];
const PAGE: BitmapId = BitmapId(0);
const FRAME: BitmapId = BitmapId(1);
const BUTTON: BitmapId = BitmapId(2);
const BUTTON_PRESSED: BitmapId = BitmapId(3);
const CLOSE: BitmapId = BitmapId(4);
const CLOSE_PRESSED: BitmapId = BitmapId(5);

/// The form while the film plays: `Image1` with the frame fills it, the
/// film plays in `Panel1` inside. // 1068:09a5
pub const FILM_WIDTH: i32 = 0x1ba;
pub const FILM_HEIGHT: i32 = 0x14a;
/// `Panel1`, where the film plays. // 1068:09a5
pub const FILM_PANEL: Rect = Rect::new(5, 5, 0x1b0, 0x140);
/// The results page, `Bild`, and the form while it shows. // 1068:1340
pub const WIDTH: i32 = 0x1b0;
pub const HEIGHT: i32 = 0x140;

/// The film that ends a quit (`FILM` directory). // 1068:0e5d
pub const QUIT_FILM: &str = "ABEnde.avi";
/// The music after a fair win, and otherwise (`MUSIK`). // 1068:0f93, 1068:0ffe
pub const WON_MUSIC: &str = "gewonn.mid";
pub const LOST_MUSIC: &str = "pleite.mid";
/// The film when the game is lost (or won by cheating). // 1068:1009
pub const LOST_FILM: &str = "ABPleite.avi";
/// What plays when the end's music is over, over and over. // 1068:1872
pub const LOOP_MUSIC: &str = "endeloop.mid";

/// The film for a fair win of each scenario (0 to 6). // 1068:0f9e..1068:0fe4
pub const WON_FILMS: [&str; 7] = [
    "ABrente.avi",  // 1068:0f9e
    "ABgeld.avi",   // 1068:0faa
    "ABleute.avi",  // 1068:0fc0
    "ABking.avi",   // 1068:0fb5
    "ABluxus.avi",  // 1068:0fe4
    "ABsurfer.avi", // 1068:0fd7
    "ABoeko.avi",   // 1068:0fcc
];

/// The won films of the later build's scenarios 8 to 13. // en 1070:121d
/// .. en 1070:125c
pub const UPGRADE_WON_FILMS: [&str; 6] = [
    "ABFries.avi",  // en 1070:121d
    "ABCamper.avi", // en 1070:1229
    "ABBBonly.avi", // en 1070:1236
    "ABPRWund.avi", // en 1070:1243
    "ABHurry.avi",  // en 1070:1250
    "ABChalle.avi", // en 1070:125c
];

/// The lines of `TEXTE\Schluss.txt`, and of the later build's
/// `UPGRADE\Schluss.txt`, whose blocks of reasons have a line for each of
/// fourteen games instead of eight.
pub const LINES: usize = 32;
pub const LINES_UPGRADE: usize = 44;

/// `Schluss.txt`'s 32 lines, as `schluss_load_texts` reads them
/// (`Readln` into `string`s); missing lines are empty. // 1068:0288
#[must_use]
pub fn schluss_lines(data: &[u8]) -> Vec<Vec<u8>> {
    schluss_lines_n(data, LINES)
}

/// The later build's `UPGRADE\Schluss.txt`: its 44 lines. // en 1070:0294
#[must_use]
pub fn schluss_lines_upgrade(data: &[u8]) -> Vec<Vec<u8>> {
    schluss_lines_n(data, LINES_UPGRADE)
}

fn schluss_lines_n(data: &[u8], n: usize) -> Vec<Vec<u8>> {
    let mut f = formats::pascal::TextReader::new(data);
    (0..n).map(|_| f.read_line()).collect()
}

/// What the results show: the globals `schluss_load_texts` reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Results {
    /// The scenario is won. // 1110:0071
    pub won: bool,
    /// The player cheated. // 1110:006f
    pub cheated: bool,
    /// // 1110:006e
    pub scenario: u8,
    /// The human's net worth, as money text. // 1008:0c6e
    pub net_worth: Vec<u8>,
    /// The human's guests in the last twelve months. // 1070:1345
    pub guests: i32,
    /// The score, if it went into the high scores. // 1110:0068, 1110:006c
    pub score: Option<i32>,
}

impl Results {
    /// The results of the game the world just ended, the human's
    /// (player 1's), with the money as `factor` and `language` format it.
    #[must_use]
    pub fn of(world: &World, over: &GameOver, factor: i32, language: Language) -> Self {
        // Single human: the results are player 1's.
        Results {
            won: over.won,
            cheated: world.cheated,
            scenario: world.scenario,
            net_worth: format_money(over.net_worth, factor, language),
            guests: world.guests_last_months(1),
            score: over.score,
        }
    }

    /// A fair win: "Weiterspielen", the won music and film.
    fn fair_win(&self) -> bool {
        self.won && !self.cheated
    }
}

/// The music and the film for the end of a game: `None` for the film of
/// the tutorial's 7 and past 13. // 1068:1016, en 1070:128d
#[must_use]
pub fn end_media(r: &Results) -> (&'static str, Option<&'static str>) {
    if r.fair_win() {
        // BUG(original): a won game of the tutorial's 7 (or past 13) sets no film, leaving the film player's file name as it was; the port shows the results at once.
        let film = match r.scenario {
            s @ 0..=6 => WON_FILMS.get(usize::from(s)).copied(),
            s @ 8..=13 => UPGRADE_WON_FILMS.get(usize::from(s - 8)).copied(),
            _ => None,
        };
        (WON_MUSIC, film)
    } else {
        (LOST_MUSIC, Some(LOST_FILM))
    }
}

/// The results page's texts from `Schluss.txt`'s lines: the eight labels
/// and the four buttons. // 1068:0288
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Texts {
    pub labels: [Vec<u8>; 8],
    pub buttons: [Vec<u8>; 4],
}

/// `schluss_load_texts`: line 0 the title, 1 the heading, then the
/// verdict (2 won, 3 not; a cheater gets 3), then why: line 4 for a
/// cheater, else one of eight lines for a win (5–12) or a loss (13–20),
/// by the scenario (0 first, the tutorial 7 second, 1–6 after). Then the
/// net worth (21, the money, 22), the guests (23, 24) and the score (25,
/// 26), each joined with spaces, the high score note (27) and the
/// buttons (28–31). // 1068:0288
///
/// The later build's file ([`LINES_UPGRADE`] lines, [`schluss_lines_upgrade`])
/// has fourteen lines in each block, the scenarios 8 to 13 after 6, so
/// the rest is twelve lines further on. // en 1070:0294
#[must_use]
pub fn schluss_load_texts(lines: &[Vec<u8>], r: &Results) -> Texts {
    let games = if lines.len() >= LINES_UPGRADE { 14 } else { 8 };
    let line = |n: usize| lines.get(n).cloned().unwrap_or_default();
    let join = |a: usize, middle: &[u8], b: usize| {
        let mut s = line(a);
        s.push(b' '); // 1068:0286
        s.extend_from_slice(middle);
        s.push(b' ');
        s.extend_from_slice(&line(b));
        s.truncate(255);
        s
    };
    let verdict = if r.won && !r.cheated { line(2) } else { line(3) };
    let why = if r.cheated {
        line(4)
    } else {
        let block = if r.won { 5 } else { 5 + games };
        let row = match r.scenario {
            0 => Some(0),
            7 => Some(1),
            s @ 1..=6 => Some(usize::from(s) + 1),
            s @ 8..=13 if games == 14 => Some(usize::from(s)),
            // BUG(original): a scenario past the file's (7, or 13 in the later build's) leaves the text as it was; the port leaves it empty.
            _ => None,
        };
        row.map(|k| line(block + k)).unwrap_or_default()
    };
    let rest = 5 + 2 * games;
    let score = r.score.unwrap_or(0);
    Texts {
        labels: [
            line(0),
            line(1),
            verdict,
            why,
            join(rest, &r.net_worth, rest + 1),
            join(rest + 2, r.guests.to_string().as_bytes(), rest + 3),
            join(rest + 4, score.to_string().as_bytes(), rest + 5),
            line(rest + 6),
        ],
        buttons: [line(rest + 7), line(rest + 8), line(rest + 9), line(rest + 10)],
    }
}

/// What the form asks of the game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// "Weiterspielen": the form closes, the main screen comes to the
    /// front, the clock runs again (1020:24b7) and, if it played before,
    /// the background music starts again (1018:3850). // 1068:0180
    Continue { restart_music: bool },
    /// "High-Score": the high scores open
    /// ([`crate::highscoreform::Highscoreform::show`]) with this form
    /// disabled until they close. // 1068:01b3
    HighScores,
    /// "Neues Spiel": the form closes and `back_to_start_menu` saves
    /// `Last.map` and shows the start menu. // 1068:0201
    NewGame,
    /// The quit film is over: everything is freed, `game_done`, `Halt`.
    /// Nothing is saved first. // 1068:17ad, 1030:3974
    Exit,
}

/// What an event did.
#[allow(clippy::struct_excessive_bools)] // Independent requests.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[must_use]
pub struct Response {
    /// Something changed and the handler refreshes the images.
    pub refresh: bool,
    /// The sounds played, in order (`play_sound`, 10a0:30b6).
    pub sounds: Vec<Sound>,
    pub action: Option<Action>,
    /// A film to play from `FILM` in `Panel1` ([`Schlussform::film_panel`]),
    /// with the `WaveMix` session's wave device given up first (10a0:2518);
    /// report its end with [`Schlussform::film_ended`].
    pub film: Option<&'static str>,
    /// The film stops and closes (`Stop`, `Eject`, `Close`), `Panel1`
    /// hides and the session takes the wave device back (10a0:254f).
    /// // 1068:17ad
    pub stop_film: bool,
    /// Music to play from `MUSIK` (`MusikPlayer`).
    pub music: Option<&'static str>,
    /// The end's music stops (1068:1593).
    pub stop_music: bool,
    /// The background music stops (1018:38c5).
    pub stop_background_music: bool,
}

/// What the form shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Hidden,
    /// A film plays. // +0xf8c
    Film,
    /// The results page.
    Results,
}

/// Why the form is up. // +0xf8b
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// The end of a game (0).
    End,
    /// Quitting (1).
    Quit,
}

/// The end form.
#[derive(Debug, Clone)]
pub struct Schlussform {
    font: Font,
    bitmaps: Vec<Surface>,
    empty: Surface,
    /// `Schluss.txt`'s lines.
    lines: Vec<Vec<u8>>,
    /// `Image1`'s picture: the film's frame.
    frame: Surface,
    /// `Bild`'s picture: the results page.
    page: Surface,
    pub stage: Stage,
    pub mode: Mode,
    /// The form's top left on the screen.
    pub left: i32,
    pub top: i32,
    results: Option<Results>,
    /// "Weiterspielen", "High-Score", "Beenden", "Neues Spiel".
    /// // +0x1a4, +0x2ba, +0x3d0, +0x4e6
    buttons: [Button; 4],
    /// // +0x5fc
    close: Button,
    /// // +0x712 .. +0xe7b
    labels: [Label; 8],
    /// The background music played when the game ended. // +0xf8a
    music_was_on: bool,
    /// The end's music plays. // +0xf8d
    music: bool,
}

/// The buttons' left edges: 90 × 22 at y 294. // 1068:09a5
const BUTTON_LEFTS: [i32; 4] = [0xf, 0x77, 0xdf, 0x147];
/// The labels' tops and heights, 400 wide from x 16 (the title 416, in
/// colour 1), centred, in colour 9. // 1068:09a5
const LABELS: [(i32, i32); 8] = [
    (0, 0x10),
    (0x20, 0x20),
    (0x4a, 0x20),
    (0x74, 0x20),
    (0x9e, 0x20),
    (200, 0x20),
    (0xf2, 0x10),
    (0x10c, 0x10),
];

impl Schlussform {
    /// The form as it was created, not shown. `bitmaps` are those of
    /// [`BITMAP_FILES`], `lines` `Schluss.txt`'s.
    #[must_use]
    pub fn new(font: Font, bitmaps: Vec<Surface>, lines: Vec<Vec<u8>>) -> Self {
        let mut f = Schlussform {
            font,
            bitmaps,
            empty: Surface::new(0, 0, 0),
            lines,
            frame: Surface::new(FILM_WIDTH as usize, FILM_HEIGHT as usize, 0),
            page: Surface::new(WIDTH as usize, HEIGHT as usize, 0),
            stage: Stage::Hidden,
            mode: Mode::End,
            left: 0,
            top: 0,
            results: None,
            buttons: std::array::from_fn(|_| Button::new(Frame::new(0, 0, 0, 0), BUTTON, BUTTON_PRESSED, b"", 9)),
            close: Button::new(Frame::new(0, 0, 16, 16), CLOSE, CLOSE_PRESSED, b"", 9),
            labels: std::array::from_fn(|_| Label::new(Frame::new(0, 0, 0, 0), b"", Align::Center, 9)),
            music_was_on: false,
            music: false,
        };
        f.init(None, (640, 480));
        f
    }

    /// `FormShow`'s part (1068:09a5): new widgets, the film's frame drawn,
    /// the form centred at its film size, and the texts for `results`.
    fn init(&mut self, results: Option<Results>, screen: (i32, i32)) {
        self.buttons = std::array::from_fn(|i| {
            Button::new(
                Frame::new(BUTTON_LEFTS[i], 0x126, 0x5a, 0x16),
                BUTTON,
                BUTTON_PRESSED,
                b"",
                9,
            )
        });
        self.close = Button::new(Frame::new(0, 0, 16, 16), CLOSE, CLOSE_PRESSED, b"", 9);
        self.labels = std::array::from_fn(|i| {
            let (top, height) = LABELS[i];
            let (width, colour) = if i == 0 { (0x1a0, 1) } else { (400, 9) };
            Label::new(Frame::new(0x10, top, width, height), b"", Align::Center, colour)
        });
        let clip = self.frame.bounds();
        let frame = self.bitmaps.get(FRAME.0).unwrap_or(&self.empty);
        self.frame.blit(0, 0, frame, clip, Rop::Copy, clip);
        self.top = (screen.1 - FILM_HEIGHT) / 2;
        self.left = (screen.0 - FILM_WIDTH) / 2;
        if let Some(r) = &results {
            let texts = schluss_load_texts(&self.lines, r);
            for (label, text) in self.labels.iter_mut().zip(texts.labels) {
                label.text = text;
            }
            for (button, text) in self.buttons.iter_mut().zip(texts.buttons) {
                button.text = text;
            }
        }
        self.results = results;
    }

    /// `TCustomForm.Show` without anything else, as loading a game whose
    /// file has the form shown does (1010:08d7): the form has no
    /// `OnShow`, so it shows the results page as it was last drawn, the
    /// background's black in a session that hasn't drawn it, where it is.
    /// The original's buttons on it are another game's or were never made
    /// (a click on them goes through their freed or nil fields); the
    /// port's are the last ones made, which click as they did.
    // BUG(original): a Last.map saved by the end form's "Neues Spiel" has it shown, and loading it shows the form again over the game, without its results.
    pub fn show_saved(&mut self) {
        self.stage = Stage::Results;
    }

    /// The results the form shows.
    #[must_use]
    pub fn results(&self) -> Option<&Results> {
        self.results.as_ref()
    }

    /// Whether the form is shown.
    #[must_use]
    pub fn visible(&self) -> bool {
        self.stage != Stage::Hidden
    }

    /// The form on the screen.
    #[must_use]
    pub fn bounds(&self) -> Rect {
        match self.stage {
            Stage::Results => Rect::new(self.left, self.top, WIDTH, HEIGHT),
            _ => Rect::new(self.left, self.top, FILM_WIDTH, FILM_HEIGHT),
        }
    }

    /// The form's components that show, with their bounds in the form and
    /// their pictures: `Image1` and the black `Panel1` during a film,
    /// `Bild` with the results.
    #[must_use]
    pub fn components(&self) -> Vec<(&'static str, Rect, Option<&Surface>)> {
        match self.stage {
            Stage::Hidden => Vec::new(),
            Stage::Film => vec![
                ("Image1", Rect::new(0, 0, FILM_WIDTH, FILM_HEIGHT), Some(&self.frame)),
                ("Panel1", FILM_PANEL, None),
            ],
            Stage::Results => vec![("Bild", Rect::new(0, 0, WIDTH, HEIGHT), Some(&self.page))],
        }
    }

    /// The end of a game (1068:1016): the clock stops (and the help, the
    /// TV and the background music: `music_on` says whether it played),
    /// the other forms are disabled but this one (`disable_forms(0xe)`),
    /// and the film plays with the music; at its end the results. The
    /// clock, the help, the TV and the forms are the caller's.
    pub fn open_end(&mut self, results: Results, music_on: bool, screen: (i32, i32)) -> Response {
        // Single human: the clock stands until the one player picks a
        // button; the simulation waits on this dialog.
        self.mode = Mode::End;
        self.music_was_on = music_on;
        let (music, film) = end_media(&results);
        self.init(Some(results), screen);
        self.music = true;
        let mut r = Response {
            refresh: true,
            music: Some(music),
            stop_background_music: music_on,
            ..Response::default()
        };
        match film {
            Some(film) => {
                self.stage = Stage::Film;
                r.film = Some(film);
            }
            None => self.show_results(screen),
        }
        r
    }

    /// Quitting (1068:0f52, 1068:0e68): the clock stops, the other forms
    /// are disabled, and `ABEnde.avi` plays; at its end the game exits.
    pub fn open_quit(&mut self, screen: (i32, i32)) -> Response {
        self.init(self.results.clone(), screen);
        self.start_quit_film()
    }

    /// `ABEnde.avi` in the frame. // 1068:0e68
    fn start_quit_film(&mut self) -> Response {
        self.mode = Mode::Quit;
        self.stage = Stage::Film;
        Response {
            refresh: true,
            film: Some(QUIT_FILM),
            ..Response::default()
        }
    }

    /// The film is over or skipped (`Timer1`, a key, a click on it or on
    /// the background form): after the end of a game the results show;
    /// after the quit film the game exits. Nothing while no film plays.
    /// // 1068:17ad
    pub fn film_ended(&mut self, screen: (i32, i32)) -> Response {
        let mut r = Response::default();
        if self.stage != Stage::Film {
            return r;
        }
        r.refresh = true;
        r.stop_film = true;
        match self.mode {
            Mode::End => self.show_results(screen),
            // BUG(original): quitting from the results page ("Beenden") saves nothing, unlike TEndeform's "OK", which saves Last.map first.
            Mode::Quit => {
                r.stop_music = self.close_form();
                r.action = Some(Action::Exit);
            }
        }
        r
    }

    /// `Panel1` on the screen while a film plays: the film is drawn at its
    /// size from its top left.
    #[must_use]
    pub fn film_panel(&self) -> Option<Rect> {
        (self.stage == Stage::Film).then(|| {
            Rect::new(
                self.left + FILM_PANEL.left,
                self.top + FILM_PANEL.top,
                FILM_PANEL.width(),
                FILM_PANEL.height(),
            )
        })
    }

    /// `FormKeyDown`: any key skips the film. // 1068:198a
    pub fn key_down(&mut self, screen: (i32, i32)) -> Response {
        self.film_ended(screen)
    }

    /// `Timer1`'s second half: when the end's music is over, the loop
    /// plays (again). // 1068:187f
    #[must_use]
    pub fn music_ended(&self) -> Option<&'static str> {
        self.music.then_some(LOOP_MUSIC)
    }

    /// The results page: the form at its size, centred, `schluss.bmp` with
    /// the buttons (but "Weiterspielen" after a fair win only), the
    /// labels, and the high score note if the score went in. // 1068:1340
    fn show_results(&mut self, screen: (i32, i32)) {
        self.stage = Stage::Results;
        self.top = (screen.1 - HEIGHT) / 2;
        self.left = (screen.0 - WIDTH) / 2;
        let clip = self.page.bounds();
        let page = self.bitmaps.get(PAGE.0).unwrap_or(&self.empty);
        self.page.blit(0, 0, page, clip, Rop::Copy, clip);
        let fair = self.results.as_ref().is_some_and(Results::fair_win);
        let entered = self.results.as_ref().is_some_and(|r| r.score.is_some());
        let Schlussform {
            font,
            bitmaps,
            page,
            buttons,
            close,
            labels,
            ..
        } = self;
        let skin = Skin { font, bitmaps };
        for (i, b) in buttons.iter_mut().enumerate() {
            if i > 0 || fair {
                b.show(page, &skin);
            }
        }
        close.show(page, &skin);
        for (i, l) in labels.iter_mut().enumerate() {
            if i < 7 || entered {
                l.show(page, &skin);
            }
        }
    }

    /// `BildMouseDown`: the buttons are offered the press until one takes
    /// it. // 1068:1613
    pub fn mouse_down(&mut self, x: i32, y: i32) -> Response {
        let mut r = Response::default();
        if self.stage != Stage::Results {
            return r;
        }
        for i in 0..5 {
            let out = self.button(i).mouse_down(x, y);
            if out.handled {
                self.repaint_button(i);
                r.sounds.extend(out.sound);
                r.refresh = true;
                break;
            }
        }
        r
    }

    /// `BildMouseUp`: the buttons are offered the release until one takes
    /// it, and a click does its part: "Weiterspielen", "High-Score",
    /// "Beenden" (the quit film), "Neues Spiel", the close button (the
    /// quit film too). // 1068:16e0
    pub fn mouse_up(&mut self, x: i32, y: i32) -> Response {
        let mut r = Response::default();
        if self.stage != Stage::Results {
            return r;
        }
        for i in 0..5 {
            let out = self.button(i).mouse_up(x, y);
            if !out.handled {
                continue;
            }
            self.repaint_button(i);
            r.refresh = true;
            if out.clicked {
                r = self.click(i, r);
            }
            r.sounds.extend(out.sound);
            break;
        }
        r
    }

    fn button(&mut self, i: usize) -> &mut Button {
        if i < 4 { &mut self.buttons[i] } else { &mut self.close }
    }

    fn repaint_button(&mut self, i: usize) {
        let Schlussform {
            font,
            bitmaps,
            page,
            buttons,
            close,
            ..
        } = self;
        let skin = Skin { font, bitmaps };
        let b = if i < 4 { &buttons[i] } else { &*close };
        b.draw(page, &skin);
    }

    /// The buttons' `Click`s. // 1068:0180, 1068:01b3, 1068:01df, 1068:0201, 1068:0218
    fn click(&mut self, i: usize, mut r: Response) -> Response {
        match i {
            0 => {
                r.stop_music = self.close_form();
                r.action = Some(Action::Continue {
                    restart_music: self.music_was_on,
                });
            }
            1 => r.action = Some(Action::HighScores),
            3 => {
                r.stop_music = self.close_form();
                r.action = Some(Action::NewGame);
            }
            // "Beenden" and the close button: `Bild` hides and the quit
            // film plays.
            _ => {
                let film = self.start_quit_film();
                r.film = film.film;
            }
        }
        r
    }

    /// `schluss_close`: hides the form, stops the end's music if it plays
    /// (whether it did); the other forms are enabled again (the caller's).
    /// // 1068:1593
    fn close_form(&mut self) -> bool {
        self.stage = Stage::Hidden;
        std::mem::take(&mut self.music)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines() -> Vec<Vec<u8>> {
        (0..32).map(|i| format!("L{i}").into_bytes()).collect()
    }

    fn results(won: bool, cheated: bool, scenario: u8) -> Results {
        Results {
            won,
            cheated,
            scenario,
            net_worth: b"1.234".to_vec(),
            guests: 56,
            score: (won && !cheated).then_some(78),
        }
    }

    #[test]
    fn texts() {
        let t = schluss_load_texts(&lines(), &results(true, false, 0));
        assert_eq!(t.labels[2], b"L2");
        assert_eq!(t.labels[3], b"L5");
        assert_eq!(t.labels[4], b"L21 1.234 L22");
        assert_eq!(t.labels[5], b"L23 56 L24");
        assert_eq!(t.labels[6], b"L25 78 L26");
        assert_eq!(t.buttons[3], b"L31");
        assert_eq!(schluss_load_texts(&lines(), &results(true, false, 3)).labels[3], b"L9");
        assert_eq!(schluss_load_texts(&lines(), &results(true, false, 7)).labels[3], b"L6");
        let lost = schluss_load_texts(&lines(), &results(false, false, 6));
        assert_eq!((&lost.labels[2][..], &lost.labels[3][..]), (&b"L3"[..], &b"L20"[..]));
        assert_eq!(lost.labels[6], b"L25 0 L26");
        let cheat = schluss_load_texts(&lines(), &results(true, true, 2));
        assert_eq!((&cheat.labels[2][..], &cheat.labels[3][..]), (&b"L3"[..], &b"L4"[..]));
    }

    #[test]
    fn media() {
        assert_eq!(end_media(&results(true, false, 1)), (WON_MUSIC, Some("ABgeld.avi")));
        assert_eq!(end_media(&results(true, true, 1)), (LOST_MUSIC, Some(LOST_FILM)));
        assert_eq!(end_media(&results(false, false, 0)), (LOST_MUSIC, Some(LOST_FILM)));
        assert_eq!(end_media(&results(true, false, 7)).1, None);
    }
}
