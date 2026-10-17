//! `TFernseher`, the TV (segment `1048`): the news on a TV set, with the
//! archive of the last 26 to browse. A message opens it if the player's
//! options for its kind of news say so ([`crate::news`]), and the button
//! bar's third button opens it on the archive's page it was last on
//! (1010:2c8b). While it's open the game stands (`1110:0067`): the world's
//! part is [`World::post_message`] and the commands
//! [`Action::ShowTv`] and [`Action::TvPause`], which this view asks for
//! in [`Response::actions`]; the rest is the view's.
//!
//! The form is two `TImage`s the game draws into itself, one [`Surface`]
//! each here, which the caller puts into the VCL form at the bounds
//! [`LAYOUT`] gives: `tvleiimage`, the title bar (the close button and the
//! bar to drag the form by), and `tvimage`, the TV with the message's
//! picture, date and text and four buttons: scroll the view to the news'
//! square and back, the page before, the page after, and off.
//!
//! The speaker (`SOUNNAME.DAT` from `SPRADEUn\`, `MediaPlayer2`) is the
//! caller's: the TV asks for it in [`Response::media`], with the `WaveMix`
//! session given up while it reads, and takes `Position = Length` from it
//! in [`Fernseher::timer`]; the world hears of it through
//! [`Action::Speaker`] (`docs/sound.md`).
//!
//! The news' film (`FILMNAME.DAT`, `MediaPlayer1` on `Panel1`) is the
//! caller's too: [`Media::PlayFilm`] in [`FILM_PANEL`], and its end
//! (`Position = Length`) back in [`Fernseher::timer`] (`docs/films.md`).

// The original's names: `x`, `y`, sizes `w`, `h`.
#![allow(clippy::many_single_char_names)]

use vcl::Rect;
use vcl::surface::{Rop, Surface};

use crate::command::Action;
use crate::enhance::{Enhancement, Enhancements};
use crate::hauptschirm::InfoTexts;
use crate::money::format_money;
use crate::news::{Entry, Speaker};
use crate::text::{Align, Font};
use crate::widgets::ShiftState;
use crate::world::{Pos, World};

/// The form's size while it's shown: `ClientWidth`, `ClientHeight` as
/// 1048:12e9 sets them.
pub const WIDTH: i32 = 352;
pub const HEIGHT: i32 = 335;
/// The title bar's height, which the form shrinks to while it's dragged
/// and after a double click on it. // 1048:288f
const TITLE_HEIGHT: i32 = 16;
/// The height `init_messages` centres the form by. // 1048:1ffd
const CENTRE_HEIGHT: i32 = 0x14e;

/// The images' bounds in the form. // 1048:12e9
pub const LAYOUT: [(&str, Rect); 2] = [
    ("tvleiimage", Rect::new(0, 0, WIDTH, TITLE_HEIGHT)),
    ("tvimage", Rect::new(0, TITLE_HEIGHT, WIDTH, 319)),
];

/// The bitmaps the form is drawn from, from `GRAFIK`, in the order
/// [`Fernseher::new`] takes them.
pub const BITMAP_FILES: [&str; 5] = [
    "tvbild.BMP",   // tvimage's background, 1110:6650, 1048:1fd0
    "tvanim.BMP",   // the TV's two lights, 1110:666c, 1048:1fdb
    "tvknopf.BMP",  // the buttons held down, 1110:6666, 1048:1fe6
    "tvleis.BMP",   // the title bar, 1110:6658, 1048:1ff2
    "closepre.bmp", // the close button held down, 1110:54bc
];
const BILD: usize = 0;
const ANIM: usize = 1;
const KNOPF: usize = 2;
const LEISTE: usize = 3;
const CLOSE_PRESSED: usize = 4;

/// The form's parts, left, top, right and bottom, inclusive: 0 the TV,
/// 1 to 4 its buttons in `tvimage` (to the news and back, back, on, off),
/// 5 the close button and 6 the bar to drag in `tvleiimage`. // 1110:1a52
const RECTS: [(i32, i32, i32, i32); 7] = [
    (0, 0, 352, 319),     // 1110:1a52
    (107, 278, 139, 310), // 1110:1a5a
    (143, 278, 175, 310), // 1110:1a62
    (179, 278, 211, 310), // 1110:1a6a
    (215, 278, 247, 310), // 1110:1a72
    (0, 0, 16, 16),       // 1110:1a7a
    (16, 0, 352, 16),     // 1110:1a82
];
const CENTRE: u8 = 1;
const BACK: u8 = 2;
const ON: u8 = 3;
const OFF: u8 = 4;
const CLOSE: u8 = 5;
const DRAG: u8 = 6;

/// The message's picture: 125 × 125 of `FILENAME.DAT`'s bitmap. // 1048:087f
const PICTURE: Rect = Rect::new(54, 72, 125, 125);
/// A button held down: 32 × 32 of `tvknopf.BMP`, button `n`'s at
/// `0x24 · (n − 1)` (1048:15e6); the first button's picture shows that
/// the view is at the news' square (1048:087f).
const BUTTON_SIZE: i32 = 32;
const BUTTON_STEP: i32 = 0x24;
/// The TV's two lights, frame 0 while no one speaks: 40 × 16 at (242,
/// 117) from row 0 and 40 × 20 at (243, 91) from row 16. // 1048:1b3e
const LIGHTS: [(i32, i32, Rect); 2] = [
    (0xf2, 0x75, Rect::new(0, 0, 0x28, 0x10)),
    (0xf3, 0x5b, Rect::new(0, 0x10, 0x28, 0x14)),
];

/// The ms after the TV opened for a message before the speaker starts
/// (1048:1b3e); with the speaker off, the film starts or the TV closes
/// after [`AUTO_CLOSE_MS`].
pub const SPEAKER_MS: u32 = 3000;
pub const AUTO_CLOSE_MS: u32 = 6000;

/// The media name for none. // 1048:0a45, 1048:1b39
const NONE: &[u8] = b"kein";

/// `VK_*` codes the form reads. // 1048:26f6
const VK_RETURN: u16 = 0x0d;
const VK_ESCAPE: u16 = 0x1b;
const VK_SPACE: u16 = 0x20;
const VK_LEFT: u16 = 0x25;
const VK_RIGHT: u16 = 0x27;
const VK_F4: u16 = 0x73;

/// `play_sound`'s numbers. // 10a0:30b6
pub const SOUND_SHOW: u8 = 0xd;
pub const SOUND_HIDE: u8 = 0xe;
/// After the TV opened for a message. // 1048:0a54
pub const SOUND_NEWS: u8 = 0x24;
const SOUND_PRESS: u8 = 8;
const SOUND_KEY: u8 = 9;
const SOUND_RELEASE: u8 = 10;

/// `Panel1`, where the film plays, in the form (1048:01bd: left 9, top
/// 0x19, width 0x14c, height 0x100). The films are 335 × 257, drawn at
/// their size from its top left, so their right 3 columns and bottom row
/// are cut off.
// BUG(original): `Panel1` is 332 × 256 and the films 335 × 257: their right 3 columns and bottom row are never seen.
pub const FILM_PANEL: Rect = Rect::new(9, 0x19, 0x14c, 0x100);

/// The film's state (`1110:6672`, a `Longint`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Film {
    /// Not started. // 0
    Waiting,
    /// Started (1048:01bd). // 1
    Started,
    /// Stopped (1048:034f). // 2
    Stopped,
    /// The player browsed: no film, no closing by the timer. // 3
    Browsed,
}

/// What the TV asks of the sound, in order. // 1048:0a54, 1048:1b3e, 1048:034f, 1048:077a
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Media {
    /// The `WaveMix` session gives up the wave device (10a0:2518).
    EffectsOff,
    /// It takes it back (10a0:254f).
    EffectsOn,
    /// `fernseher_hide`'s end: with the options' "Geräusche" on
    /// (`1110:0061`), [`Media::EffectsOn`] and [`Media::SoundVolume`].
    EffectsOnWithSounds,
    /// The wave device's volume at the speech volume (10a0:24bc).
    SpeechVolume,
    /// At the sound volume again (10a0:2453).
    SoundVolume,
    /// `MediaPlayer2.FileName := SPRADEU<dir>\<file>; Open`.
    OpenSpeaker { dir: u8, file: Vec<u8> },
    /// `MediaPlayer2.Play`.
    PlaySpeaker,
    /// `MediaPlayer2.Close`.
    CloseSpeaker,
    /// `MediaPlayer1.FileName := <film dir>\<file>; Open; Play`, shown in
    /// [`FILM_PANEL`]. // 1048:01bd
    PlayFilm { file: Vec<u8> },
    /// `MediaPlayer1.Eject; Close`, `Panel1` hidden. // 1048:034f
    StopFilm,
}

/// What an event did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[must_use]
pub struct Response {
    /// The images changed.
    pub refresh: bool,
    /// The sounds played, in order (`play_sound`, 10a0:30b6).
    pub sounds: Vec<u8>,
    /// The main screen scrolls to this square (`Hauptschirm::jump_to`,
    /// 1020:0787), then the caller hands the world the new viewpoint.
    pub scroll: Option<Pos>,
    /// The TV opened: the button bar's third button goes down
    /// (1010:2b2e), and the TV takes the keys.
    pub shown: bool,
    /// The TV closed (1048:077a), or didn't open: the button bar's third
    /// button comes up (1010:2b60) and the main screen gets the focus back.
    pub hidden: bool,
    /// The commands the caller applies for player 1, in order: the pause
    /// while the TV is open, and what its speaker does. Single human.
    pub actions: Vec<Action>,
    /// What the sound does, in order, before [`Response::sounds`].
    pub media: Vec<Media>,
}

/// Which image took the mouse down: it keeps the mouse until it's up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Capture {
    None,
    Title,
    Tv,
}

/// The TV's state and images.
#[allow(clippy::struct_excessive_bools)] // One flag for each of the original's.
#[derive(Debug, Clone)]
pub struct Fernseher {
    font: Font,
    bitmaps: Vec<Surface>,
    /// `FILENAME.DAT`'s bitmaps by message, `None` where the file is
    /// missing (the original raises an exception). // 1048:087f
    pictures: Vec<Option<Surface>>,
    /// `NACHRICH.TXT`'s title and three texts per message.
    /// // *1110:6634, 1110:663e
    title: Vec<u8>,
    texts: Vec<[Vec<u8>; 3]>,
    /// `FILMNAME.DAT`, by message. // *1110:6630 + 0xd
    films: Vec<Vec<u8>>,
    /// `SOUNNAME.DAT`, by message: the speaker's file. // *1110:6630 + 0x1a
    speech: Vec<Vec<u8>>,
    /// The options' "Sprachausgabe" (`1110:0066`), the caller's.
    pub speech_option: bool,
    /// A message's speech is open (`tv_busy`). // 1110:662d
    speaker_open: bool,
    /// The speaker was started. // 1110:667a
    speaker_started: bool,
    /// The speaker has played to its end (`Position = Length`), as the
    /// caller last said.
    speaker_done: bool,
    /// What the world was last told of the speaker.
    reported: Speaker,
    /// The film gave up the `WaveMix` session and nothing took it back.
    effects_off: bool,
    /// [`Response::media`] so far.
    media: Vec<Media>,
    /// The form is shown. // +0x29
    pub visible: bool,
    /// The form's top left on the screen, kept while it's hidden.
    /// // 1110:664a, 1110:664c
    pub left: i32,
    pub top: i32,
    /// `ClientHeight`.
    pub height: i32,
    /// The archive's entry on the TV. // 1110:6638
    pub cursor: i16,
    /// The view is at the news' square (the first button). // 1110:662e
    pub centred: bool,
    /// The part the mouse went down on, kept after it's up. // 1110:664e
    pressed: u8,
    /// A button asked the TV to close. // 1110:6662
    closing: bool,
    /// The mouse is up. // 1110:6663
    ready: bool,
    /// The TV opened for a message: the timer shows the lights and closes
    /// it. // 1110:667d
    auto: bool,
    /// When it opened, in the caller's ms (`GetTickCount`). // 1110:6676
    opened_at: u32,
    film: Film,
    /// The film is playing (1048:01bd). // 1110:662c
    film_playing: bool,
    /// The title bar is held to drag the form. // +400
    dragging: bool,
    /// Where it was grabbed, from the form's top left. // +0x191
    grab: (i32, i32),
    /// The height before the drag. // +0x199
    saved_height: i32,
    /// The title bar was double-clicked: the form stays rolled up to it.
    /// // +0x19b
    rolled_up: bool,
    capture: Capture,
    /// `tvleiimage`'s and `tvimage`'s pictures.
    leiste: Surface,
    image: Surface,
    /// The player's view enhancements (`tv_form`, `tv_sound`).
    pub enhance: Enhancements,
}

fn copy(dst: &mut Surface, src: &Surface) {
    let all = dst.bounds();
    dst.blit(0, 0, src, all, Rop::Copy, all);
}

/// Whether `x`, `y` is in part `n`'s rectangle, edges included.
/// // 1048:1599, 1030:1517
fn hit(n: u8, x: i32, y: i32) -> bool {
    let (l, t, r, b) = RECTS[usize::from(n)];
    (l..=r).contains(&x) && (t..=b).contains(&y)
}

/// The text of `entry` as the TV shows it: `NACHRICH.TXT`'s line with
/// `&1`, `&2` the names of the players `a` and `b`, `}1`, `}2` the
/// amounts `x` and `y`, and `$` the currency, each followed by a space
/// in place of the character after it; the line ends at `*`.
/// `names` are the players' (`1110:0902`). // 1048:03c5
#[must_use]
pub fn message_text(line: &[u8], entry: &Entry, names: &[Vec<u8>], info: &InfoTexts) -> Vec<u8> {
    // BUG(original): a line without a `*` (or with one right after `&1`, `}1`, `$`...) reads on past its end; the port stops at the end.
    let at = |i: usize| line.get(i.wrapping_sub(1)).copied().unwrap_or(b'*');
    let name = |p: i16| {
        // BUG(original): a player outside 0..8 reads past the names; the port shows none.
        usize::try_from(p)
            .ok()
            .and_then(|p| names.get(p))
            .map_or(&[][..], |n| &n[..])
    };
    let mut out = Vec::new();
    let add = |out: &mut Vec<u8>, s: &[u8]| {
        out.extend_from_slice(s);
        out.truncate(255);
    };
    let mut i = 1;
    loop {
        match at(i) {
            b'&' => {
                i += 1;
                match at(i) {
                    b'1' => {
                        add(&mut out, name(entry.a));
                        i += 1;
                    }
                    b'2' => {
                        add(&mut out, name(entry.b));
                        i += 1;
                    }
                    _ => {}
                }
                add(&mut out, b" "); // 1048:03c3
            }
            b'}' => {
                i += 1;
                match at(i) {
                    b'1' => {
                        add(&mut out, &format_money(entry.x, info.currency_factor, info.land)); // 1030:11a0
                        i += 1;
                    }
                    b'2' => {
                        add(&mut out, &format_money(entry.y, info.currency_factor, info.land));
                        i += 1;
                    }
                    _ => {}
                }
                add(&mut out, b" ");
            }
            b'$' => {
                i += 1;
                add(&mut out, &info.currency); // *1110:7352
                add(&mut out, b" ");
            }
            c => add(&mut out, &[c]),
        }
        i += 1;
        if at(i) == b'*' || i > 256 {
            break;
        }
    }
    out
}

impl Fernseher {
    /// The TV as `init_messages` leaves it (1048:1ffd): hidden, centred on
    /// a `screen` of that size. `bitmaps` are those of [`BITMAP_FILES`],
    /// `pictures` `FILENAME.DAT`'s by message (`crate::auktionsform::picture_file`),
    /// `nachrich` `NACHRICH.TXT` (`formats::data::nachrich`), `films`
    /// `FILMNAME.DAT` (`formats::data::message_media_names`).
    #[must_use]
    pub fn new(
        font: Font,
        bitmaps: Vec<Surface>,
        pictures: Vec<Option<Surface>>,
        nachrich: (Vec<u8>, Vec<[Vec<u8>; 3]>),
        films: Vec<Vec<u8>>,
        screen: (i32, i32),
    ) -> Self {
        let mut f = Fernseher {
            font,
            bitmaps,
            pictures,
            title: nachrich.0,
            texts: nachrich.1,
            films,
            speech: Vec::new(),
            speech_option: true,
            speaker_open: false,
            speaker_started: false,
            speaker_done: false,
            reported: Speaker::Off,
            effects_off: false,
            media: Vec::new(),
            visible: false,
            left: 0,
            top: 0,
            height: 0,
            cursor: 0,
            centred: false,
            pressed: 0,
            closing: false,
            ready: true,
            auto: false,
            opened_at: 0,
            film: Film::Waiting,
            film_playing: false,
            dragging: false,
            grab: (0, 0),
            saved_height: HEIGHT,
            rolled_up: false,
            capture: Capture::None,
            leiste: Surface::new(WIDTH as usize, TITLE_HEIGHT as usize, 0),
            image: Surface::new(WIDTH as usize, 319, 0),
            enhance: Enhancements::ORIGINAL,
        };
        f.new_game(screen);
        f
    }

    /// `SOUNNAME.DAT` (`formats::data::message_media_names`): the speaker's
    /// file of each message. Without it the TV has no speaker.
    pub fn set_speech(&mut self, speech: Vec<Vec<u8>>) {
        self.speech = speech;
    }

    /// The speaker's file of the message on archive entry `n`, unless it
    /// has none (`kein`).
    fn speech_of(&self, world: &World, n: i16) -> Option<&[u8]> {
        let number = world.news.entry(n).map_or(0, |e| e.number);
        usize::try_from(number)
            .ok()
            .and_then(|n| self.speech.get(n))
            .filter(|f| !f.eq_ignore_ascii_case(NONE))
            .map(Vec::as_slice)
    }

    /// The speaker's file didn't open: as if it had none. The original
    /// raises an exception in `post_message` before `1110:662d` is set,
    /// and the TV closes 6 s after it opened. Whether the world must hear
    /// of it ([`Action::Speaker`]).
    pub fn speaker_missing(&mut self) -> Option<Action> {
        // BUG(original): `TMediaPlayer.Open` raises an exception in the middle of `post_message`; the port goes on without the speaker.
        self.speaker_open = false;
        let state = self.speaker_state();
        (state != self.reported).then(|| {
            self.reported = state;
            Action::Speaker(state)
        })
    }

    /// The speaker's state as the world sees it.
    fn speaker_state(&self) -> Speaker {
        match (self.speaker_open, self.speaker_started && !self.speaker_done) {
            (false, _) => Speaker::Off,
            (true, false) => Speaker::Open,
            (true, true) => Speaker::Reading,
        }
    }

    /// Closes the speaker if it's open (1048:034f, 1048:077a, 1048:15e6,
    /// 1048:1939, 1048:26b7): the `WaveMix` session back, at the sound
    /// volume.
    fn close_speaker(&mut self) {
        if self.speaker_open {
            self.media.push(Media::CloseSpeaker);
            self.media.push(Media::EffectsOn);
            self.media.push(Media::SoundVolume);
            self.effects_off = false;
            self.speaker_open = false;
        }
    }

    /// Hands `r` the sound's requests so far, and tells the world if the
    /// speaker changed.
    fn finish(&mut self, mut r: Response) -> Response {
        r.media.append(&mut self.media);
        let state = self.speaker_state();
        if state != self.reported {
            self.reported = state;
            r.actions.push(Action::Speaker(state));
        }
        r
    }

    /// A new game's TV (1048:1ffd, which each game's set-up runs): hidden,
    /// centred, on the first page, its buttons up.
    pub fn new_game(&mut self, (w, h): (i32, i32)) {
        self.left = (w - WIDTH) / 2;
        self.top = (h - CENTRE_HEIGHT) / 2;
        self.cursor = 0;
        self.ready = true;
        self.closing = false;
        self.visible = false;
        self.height = 0;
    }

    /// The TV's fields a map file holds (unit 1048:25ea), from the view's
    /// `fields` (`crate::savegame::MapView::unported`): the page, the
    /// position, the first button and the state of the buttons.
    pub fn load_fields(&mut self, fields: &crate::savegame::Fields) {
        let word = |dest, at: usize| {
            fields
                .get(dest, 0)
                .and_then(|d| d.get(at..at + 2))
                .map_or(0, |d| i16::from_le_bytes([d[0], d[1]]))
        };
        let byte = |dest| fields.get(dest, 0).and_then(|d| d.first().copied()).unwrap_or(0);
        self.cursor = word("1110:6638", 0);
        self.left = i32::from(word("1110:664a", 0));
        self.top = i32::from(word("1110:664a", 2));
        self.centred = byte("1110:662e") != 0;
        self.pressed = byte("1110:664e");
        self.closing = byte("1110:6662") != 0;
        self.ready = byte("1110:6663") != 0;
    }

    /// Puts the TV's fields into the view's `fields` for `save_map`
    /// (1048:2524).
    pub fn store_fields(&self, fields: &mut crate::savegame::Fields) {
        let short = |v: i32| i16::try_from(v).unwrap_or(0).to_le_bytes();
        fields.set("1110:6638", 0, &self.cursor.to_le_bytes());
        fields.set("1110:664a", 0, &[short(self.left), short(self.top)].concat());
        fields.set("1110:662e", 0, &[u8::from(self.centred)]);
        fields.set("1110:664e", 0, &[self.pressed]);
        fields.set("1110:6662", 0, &[u8::from(self.closing)]);
        fields.set("1110:6663", 0, &[u8::from(self.ready)]);
    }

    /// The title bar; with `tv_form` the title is centred over the bar
    /// after the close button.
    fn draw_title(&mut self) {
        copy(&mut self.leiste, &self.bitmaps[LEISTE]);
        let (l, _, r, _) = RECTS[usize::from(DRAG)];
        // BUG(original): the title is centred over the bar's width from the form's left edge, not from the bar's.
        let x = if self.enhance.has(Enhancement::TvForm) { l } else { 0 };
        self.font
            .draw(&mut self.leiste, &self.title, (x, 0), r - l, Align::Center, 10);
    }

    /// Draws `entry` on the TV: the background, the message's picture, the
    /// first button down if the view is at the news' square, the date and
    /// the text. // 1048:087f
    fn draw_entry(&mut self, entry: &Entry, world: &World, info: &InfoTexts) {
        copy(&mut self.image, &self.bitmaps[BILD]);
        let clip = self.image.bounds();
        if let Some(Some(picture)) = usize::try_from(entry.number).ok().and_then(|n| self.pictures.get(n)) {
            // BUG(original): a message whose picture file is missing (FILENAME.DAT's `Xpiranh`, `Xstrkapu`) raises an exception; the port leaves the picture out.
            let from = Rect::new(0, 0, PICTURE.width(), PICTURE.height());
            self.image
                .blit(PICTURE.left, PICTURE.top, picture, from, Rop::Copy, clip);
        }
        if self.centred {
            let (l, t, _, _) = RECTS[usize::from(CENTRE)];
            let from = Rect::new(0, 0, BUTTON_SIZE, BUTTON_SIZE);
            self.image.blit(l, t, &self.bitmaps[KNOPF], from, Rop::Copy, clip);
        }
        self.font
            .draw(&mut self.image, &entry.date, (0xc0, 0x26), 0x87, Align::Center, 9);
        let t = entry.text;
        let line = usize::try_from(t / 3)
            .ok()
            .and_then(|n| self.texts.get(n))
            .and_then(|l| usize::try_from(t % 3).ok().and_then(|v| l.get(v)));
        let text = line.map_or_else(Vec::new, |l| message_text(l, entry, &world.player_names, info));
        self.font
            .draw(&mut self.image, &text, (0x18, 0xd6), 0x131, Align::Center, 10);
    }

    /// Draws the archive's entry `n` (empty past the archive).
    fn draw(&mut self, n: i16, world: &World, info: &InfoTexts) {
        let entry = world.news.entry(n).cloned().unwrap_or_default();
        self.draw_entry(&entry, world, info);
    }

    /// The view's part of `fernseher_show` (1048:12e9) for `entry`, the
    /// archive's entry `n`: unless nothing was posted yet, the form is
    /// shown at its position (at the screen's left edge for the
    /// announcement of a building's sale), full size, with the title and
    /// the entry, and sound 0xd plays. The world's part is
    /// [`World::tv_shown`]. Nothing was posted yet: the button bar's
    /// button comes back up.
    fn show_entry(&mut self, n: i16, entry: &Entry, world: &World, info: &InfoTexts, now: u32) -> Response {
        let mut r = Response::default();
        if world.news.nothing_yet {
            r.hidden = true;
            return r;
        }
        self.film = Film::Waiting;
        self.auto = false;
        self.speaker_started = false;
        self.cursor = n;
        r.actions.push(Action::ShowTv(n));
        if entry.number == crate::news::BUILDING_SALE_NEWS {
            self.left = 0;
        }
        self.height = HEIGHT;
        self.draw_entry(entry, world, info);
        self.draw_title();
        self.visible = true;
        self.opened_at = now;
        r.refresh = true;
        r.shown = true;
        r.sounds.push(SOUND_SHOW);
        r
    }

    /// The button bar's third button (or its key) opens the TV on the page
    /// it was last on, with [`Action::ShowTv`], which pauses the game. The
    /// original also sets `1110:662e` to 0 first (1010:2c8b). `now` is the
    /// caller's clock in ms. // 1048:12e9
    pub fn show(&mut self, world: &World, info: &InfoTexts, now: u32) -> Response {
        self.centred = false;
        let n = self.cursor;
        let entry = world.news.entry(n).cloned().unwrap_or_default();
        let r = self.show_entry(n, &entry, world, info, now);
        self.finish(r)
    }

    /// The news of a tick ([`crate::simulation::Tick::news`]), in order:
    /// each moves the TV to its page and sets the first button by its
    /// `centre`; those the player's news options `options`
    /// (`1110:005c`..`005f`, [`crate::optionen::Settings::news`]) show
    /// scroll the view to their square ([`Posted::scrolls`]) and open the
    /// TV on them, with the timer that closes it, and sound 0x24, and pause
    /// the game ([`Action::ShowTv`], 1048:12e9's world part). // 1048:0a54
    ///
    /// [`Posted::scrolls`]: crate::news::Posted::scrolls
    pub fn news(
        &mut self,
        news: &[crate::news::Posted],
        options: [bool; 4],
        world: &World,
        info: &InfoTexts,
        now: u32,
    ) -> Response {
        let mut r = Response::default();
        for p in news {
            self.cursor = p.index;
            self.centred = p.centre;
            if !p.opens_tv(options) {
                continue;
            }
            if p.scrolls(options) {
                r.scroll = Some(p.entry.pos);
            }
            let shown = self.show_entry(p.index, &p.entry, world, info, now);
            r.refresh |= shown.refresh;
            r.shown |= shown.shown;
            r.sounds.extend(shown.sounds);
            r.actions.extend(shown.actions);
            self.auto = true;
            match self.speech_of(world, p.index).map(<[u8]>::to_vec) {
                Some(file) if self.speech_option => {
                    // The speech of the text read: `SPRADEU1` to `3` for
                    // the message's three texts.
                    let dir = u8::try_from(p.entry.text.rem_euclid(3)).unwrap_or(0) + 1;
                    self.media.push(Media::OpenSpeaker { dir, file });
                    self.speaker_open = true;
                }
                // BUG(original): a message without speech leaves a speech still open playing (and the effects off) until the TV closes, only forgetting it.
                _ if self.enhance.has(Enhancement::TvSound) => self.close_speaker(),
                _ => self.speaker_open = false,
            }
            self.speaker_done = false;
            r.sounds.push(SOUND_NEWS);
            self.opened_at = now;
        }
        self.finish(r)
    }

    /// The film of the entry on the TV, if it has one.
    fn film_of(&self, world: &World) -> Option<Vec<u8>> {
        let number = world.news.entry(self.cursor).map_or(0, |e| e.number);
        usize::try_from(number)
            .ok()
            .and_then(|n| self.films.get(n))
            .filter(|f| !f.eq_ignore_ascii_case(NONE))
            .cloned()
    }

    fn has_film(&self, world: &World) -> bool {
        self.film_of(world).is_some()
    }

    /// Whether the film plays, and so `Panel1` with it shows.
    #[must_use]
    pub fn film_playing(&self) -> bool {
        self.visible && self.film_playing
    }

    /// `fernseher_tick`, the clock's step 8 while the TV is shown, at
    /// `now` ms (`GetTickCount`), with `speaker_done` whether the speaker
    /// has played to its end (`Position = Length`) and `film_done`
    /// whether the film has (or didn't open). Only for a TV a
    /// message opened: 3 s after it opened, a message's speech starts,
    /// with the `WaveMix` session given up meanwhile; the two lights, which
    /// flicker while it reads (the world draws them,
    /// [`Fernseher::draw_lights`]); and when the speech ended, or without
    /// one 6 s after the TV opened, it closes, unless the page on it has a
    /// film, which plays first, and not at all once the player browsed
    /// (while a page without a film still closes it). // 1048:1b3e
    pub fn timer(&mut self, world: &World, now: u32, speaker_done: bool, film_done: bool) -> Response {
        let mut r = Response::default();
        if !self.auto {
            return r;
        }
        self.speaker_done = speaker_done;
        let since = now.wrapping_sub(self.opened_at);
        // BUG(original): the page's speech decides, though the speaker has the posted message's: browsing by key to a page without one keeps it from starting, and the TV open.
        let speech = if self.enhance.has(Enhancement::TvSound) {
            self.speaker_open
        } else {
            self.speech_of(world, self.cursor).is_some()
        };
        if since >= SPEAKER_MS && !self.speaker_started && speech {
            if self.speaker_open {
                self.media.push(Media::EffectsOff);
                self.media.push(Media::SpeechVolume);
                self.media.push(Media::PlaySpeaker);
                self.effects_off = true;
            }
            self.speaker_started = true;
        }
        if self.speaker_state() != Speaker::Reading {
            let clip = self.image.bounds();
            for (x, y, from) in LIGHTS {
                self.image.blit(x, y, &self.bitmaps[ANIM], from, Rop::Copy, clip);
            }
            r.refresh = true;
        }
        let ended = if self.speaker_open {
            self.speaker_done
        } else {
            since >= AUTO_CLOSE_MS
        };
        if ended {
            // BUG(original): a page without a film closes the TV 6 s after it opened even while the player browses.
            if self.film == Film::Browsed && self.enhance.has(Enhancement::TvForm) {
                // The player browses: the TV stays until they close it.
                return self.finish(r);
            }
            if !self.has_film(world) {
                let c = self.close_inner();
                return self.finish(c);
            }
            // A film started now has just begun (`Position` 0).
            let mut started = false;
            if self.film == Film::Waiting {
                self.play_film(world);
                started = true;
            }
            if !matches!(self.film, Film::Stopped | Film::Browsed) && self.film_playing && film_done && !started {
                self.stop_film(); // 1048:034f
                let c = self.close_inner();
                return self.finish(c);
            }
        }
        self.finish(r)
    }

    /// The lights while the speaker reads, as the world drew them
    /// ([`crate::simulation::Tick::tv_lights`]): the lower one's frame,
    /// and the upper one's if it changes, else its frame 0. // 1048:1b3e
    pub fn draw_lights(&mut self, lower: u16, upper: Option<u16>) {
        let clip = self.image.bounds();
        let [(lx, ly, l), (ux, uy, u)] = LIGHTS;
        let frame = |r: Rect, n: u16| Rect::new(r.left + l.width() * i32::from(n), r.top, r.width(), r.height());
        self.image
            .blit(lx, ly, &self.bitmaps[ANIM], frame(l, lower), Rop::Copy, clip);
        self.image.blit(
            ux,
            uy,
            &self.bitmaps[ANIM],
            frame(u, upper.unwrap_or(0)),
            Rop::Copy,
            clip,
        );
    }

    /// `fernseher_play_film` (1048:01bd): the `WaveMix` effects give up
    /// the wave device, `Panel1` shows at (9, 0x19), 0x14c × 0x100, and
    /// the page's film plays in it. The original also repaints the main
    /// screen and the info box, with the main screen's timer (`+0x190`)
    /// off meanwhile, and runs `Application.ProcessMessages` twice.
    fn play_film(&mut self, world: &World) {
        // BUG(original): the film takes WaveMix's device, and only closing the TV with the sounds on gives it back.
        // BUG(original): after a speech the film plays at the speech volume: the sound volume only comes back when the speaker closes, after the film.
        if self.enhance.has(Enhancement::TvSound) {
            self.media.push(Media::SoundVolume);
        }
        self.media.push(Media::EffectsOff); // 10a0:2518
        self.effects_off = true;
        self.film = Film::Started;
        self.film_playing = true;
        let file = self.film_of(world).unwrap_or_default();
        self.media.push(Media::PlayFilm { file });
    }

    /// Stops the film, and the speaker. With `tv_sound` the `WaveMix`
    /// session takes the wave device back. // 1048:034f
    fn stop_film(&mut self) {
        self.film = Film::Stopped;
        self.film_playing = false;
        self.media.push(Media::StopFilm);
        self.close_speaker();
        if self.enhance.has(Enhancement::TvSound) && self.effects_off {
            self.media.push(Media::EffectsOn);
            self.media.push(Media::SoundVolume);
            self.effects_off = false;
        }
    }

    /// Hides the TV (1048:077a): the form keeps its position for the next
    /// time, the film stops, sound 0xe plays, and the game goes on
    /// (`Action::TvPause(false)`), which the original does even when the
    /// TV wasn't shown (from 1010:0ac1 and 1068:1058).
    pub fn close(&mut self) -> Response {
        let r = self.close_inner();
        self.finish(r)
    }

    /// [`Fernseher::close`] without handing over the sound's requests.
    fn close_inner(&mut self) -> Response {
        let mut r = Response {
            hidden: true,
            actions: vec![Action::TvPause(false)],
            ..Response::default()
        };
        if self.visible {
            self.visible = false;
            self.height = 0;
            self.auto = false;
            if self.film_playing {
                self.stop_film();
            }
            self.close_speaker();
            // Played while a film has the `WaveMix` session: not heard.
            if !self.effects_off {
                r.sounds.push(SOUND_HIDE);
            }
            r.refresh = true;
        }
        // The pause ends, and with the sounds on the session is back.
        if self.enhance.has(Enhancement::TvSound) {
            self.media.push(Media::EffectsOn);
        }
        self.media.push(Media::EffectsOnWithSounds);
        self.effects_off = false;
        r
    }

    /// Goes to the page before (`back`) or after, as far as the archive
    /// goes: the film doesn't start any more. // 1048:1794, 1048:26f6
    fn browse(&mut self, back: bool, world: &World, info: &InfoTexts) {
        self.cursor = if back {
            (self.cursor - 1).max(0)
        } else {
            (self.cursor + 1).min(world.news.last)
        };
        self.film = Film::Browsed;
        self.draw(self.cursor, world, info);
    }

    /// The first button: to the news' square, or back to where the view
    /// was when it was posted. // 1048:1794, 1048:26f6
    fn toggle_centre(&mut self, world: &World) -> Pos {
        let entry = world.news.entry(self.cursor).cloned().unwrap_or_default();
        self.centred = !self.centred;
        if self.centred {
            entry.pos
        } else {
            // BUG(original): back goes to the saved view origin, which 1020:0787 moves 10 rows further.
            if self.enhance.has(Enhancement::TvForm) {
                (entry.origin.0.wrapping_add(10), entry.origin.1)
            } else {
                entry.origin
            }
        }
    }

    /// `tvimageMouseDown` at `x`, `y` in `tvimage`: with the mouse up
    /// before, a button under it goes down (sound 8). The film stops.
    /// // 1048:15e6
    fn tv_mouse_down(&mut self, x: i32, y: i32) -> Response {
        let mut r = Response::default();
        if self.film_playing {
            self.stop_film();
        }
        self.close_speaker();
        self.closing = false;
        let clip = self.image.bounds();
        for n in [BACK, ON, CENTRE, OFF] {
            if hit(n, x, y) && self.ready {
                self.pressed = n;
                if n != CENTRE {
                    let (l, t, _, _) = RECTS[usize::from(n)];
                    let from = Rect::new(BUTTON_STEP * i32::from(n - 1), 0, BUTTON_SIZE, BUTTON_SIZE);
                    self.image.blit(l, t, &self.bitmaps[KNOPF], from, Rop::Copy, clip);
                }
                r.refresh = true;
                r.sounds.push(SOUND_PRESS);
            }
        }
        self.ready = false;
        r
    }

    /// `tvimageMouseUp`: the button that went down does its part if the
    /// mouse is on it (sound 10), and the page is drawn again, or the TV
    /// closes. // 1048:1794
    fn tv_mouse_up(&mut self, x: i32, y: i32, world: &World, info: &InfoTexts) -> Response {
        let mut r = Response::default();
        // BUG(original): the part that went down is never cleared, so letting go of a button elsewhere and later of any press on it still clicks it.
        let (ready, pressed) = (self.ready, self.pressed);
        let on = |n: u8| hit(n, x, y) && !ready && pressed == n;
        if on(BACK) {
            self.browse(true, world, info);
            r.sounds.push(SOUND_RELEASE);
        }
        if on(ON) {
            self.browse(false, world, info);
            r.sounds.push(SOUND_RELEASE);
        }
        if on(CENTRE) {
            r.scroll = Some(self.toggle_centre(world));
            r.sounds.push(SOUND_RELEASE);
        }
        if on(OFF) {
            self.closing = true;
            r.sounds.push(SOUND_RELEASE);
        }
        if self.enhance.has(Enhancement::TvForm) {
            self.pressed = 0;
        }
        self.ready = true;
        if self.closing {
            let c = self.close_inner();
            r.hidden = c.hidden;
            r.sounds.extend(c.sounds);
            r.actions = c.actions;
        } else {
            self.draw(self.cursor, world, info);
        }
        r.refresh = true;
        r
    }

    /// `tvleiimageMouseDown` at `x`, `y` in the title bar: on the bar, it's
    /// grabbed to drag the form, which shrinks to the bar until the mouse
    /// is up (and stays so after a double click, until the next press);
    /// else the close button goes down. // 1048:288f, 1048:1939
    fn title_mouse_down(&mut self, x: i32, y: i32, shift: ShiftState) -> Response {
        let mut r = Response::default();
        self.dragging = false;
        if !(0x10..=WIDTH).contains(&x) || !(0..=0xf).contains(&y) {
            if self.film_playing {
                self.stop_film();
            }
            self.close_speaker(); // 1048:1939
            if hit(CLOSE, x, y) && self.ready {
                self.pressed = CLOSE;
                let all = self.leiste.bounds();
                self.leiste.blit(
                    0,
                    0,
                    &self.bitmaps[CLOSE_PRESSED],
                    Rect::new(0, 0, 16, 16),
                    Rop::Copy,
                    all,
                );
                r.refresh = true;
            }
            if hit(DRAG, x, y) && self.ready {
                // Never reached: the bar's part of the rectangle is taken above.
                self.pressed = DRAG;
                self.grab = (x, y);
                self.height = TITLE_HEIGHT;
            }
            self.ready = false;
            return r;
        }
        self.dragging = true;
        self.grab = (x, y);
        if !self.rolled_up {
            self.saved_height = self.height;
            self.height = TITLE_HEIGHT;
        }
        self.rolled_up = shift.contains(ShiftState::DOUBLE);
        r.refresh = true;
        r
    }

    /// `tvleiimageMouseUp`: after a drag the form unrolls (unless it was
    /// double-clicked); else the close button comes up, closing the TV if
    /// the mouse is still on it, and the title and the page are drawn
    /// again. // 1048:2922, 1048:1a4b
    fn title_mouse_up(&mut self, x: i32, y: i32, world: &World, info: &InfoTexts) -> Response {
        let mut r = Response {
            refresh: true,
            ..Response::default()
        };
        if self.dragging {
            if !self.rolled_up {
                self.height = self.saved_height;
            }
            return r;
        }
        if hit(CLOSE, x, y) && !self.ready && self.pressed == CLOSE {
            self.closing = true;
        }
        if self.pressed == DRAG {
            self.height = HEIGHT;
        }
        if self.closing {
            let c = self.close_inner();
            r.hidden = c.hidden;
            r.sounds.extend(c.sounds);
            r.actions = c.actions;
        }
        self.draw_title();
        if !self.closing {
            self.draw(self.cursor, world, info);
        }
        self.ready = true;
        r
    }

    /// The mouse goes down at `x`, `y` in the form, with the buttons and
    /// keys `shift`: the image under it takes it.
    pub fn mouse_down(&mut self, x: i32, y: i32, shift: ShiftState) -> Response {
        let r = if y < TITLE_HEIGHT {
            self.capture = Capture::Title;
            self.title_mouse_down(x, y, shift)
        } else {
            self.capture = Capture::Tv;
            self.tv_mouse_down(x, y - TITLE_HEIGHT)
        };
        self.finish(r)
    }

    /// The mouse moves to `x`, `y` in the form on a `screen` of that size:
    /// while the title bar is held with the left button, the form
    /// follows, kept on the screen. Whether it moved. // 1048:2955
    pub fn mouse_move(&mut self, x: i32, y: i32, shift: ShiftState, screen: (i32, i32)) -> bool {
        if !self.dragging || !shift.contains(ShiftState::LEFT) {
            return false;
        }
        let (sx, sy) = (x + self.left, y + self.top);
        self.left = (sx - self.grab.0).max(0).min(screen.0 - WIDTH);
        self.top = (sy - self.grab.1).max(0).min(screen.1 - self.height);
        true
    }

    /// The mouse comes up at `x`, `y` in the form: the image that took it
    /// down handles it.
    pub fn mouse_up(&mut self, x: i32, y: i32, world: &World, info: &InfoTexts) -> Response {
        let r = match std::mem::replace(&mut self.capture, Capture::None) {
            Capture::Title => self.title_mouse_up(x, y, world, info),
            Capture::Tv => self.tv_mouse_up(x, y - TITLE_HEIGHT, world, info),
            Capture::None => Response::default(),
        };
        self.finish(r)
    }

    /// `FormKeyDown`: without Ctrl and Alt, Escape and Space switch the TV
    /// off, the arrows left and right go to the page before and after, and
    /// Enter is the first button (sound 9 each); Alt+F4 switches it off.
    /// Each stops the film first. // 1048:26f6
    pub fn key_down(&mut self, key: u16, shift: ShiftState, world: &World, info: &InfoTexts) -> Response {
        let mut r = Response::default();
        let mods = shift.0 & (ShiftState::SHIFT.0 | ShiftState::ALT.0 | ShiftState::CTRL.0);
        if mods == 0 || mods == ShiftState::SHIFT.0 {
            if matches!(key, VK_ESCAPE | VK_SPACE | VK_LEFT | VK_RIGHT | VK_RETURN) && self.film_playing {
                self.stop_film(); // 1048:26b7
            }
            match key {
                VK_ESCAPE | VK_SPACE => return self.close(),
                VK_LEFT | VK_RIGHT => {
                    self.browse(key == VK_LEFT, world, info);
                    r.sounds.push(SOUND_KEY);
                    r.refresh = true;
                }
                VK_RETURN => {
                    let scroll = self.toggle_centre(world);
                    self.draw(self.cursor, world, info);
                    r.scroll = Some(scroll);
                    r.sounds.push(SOUND_KEY);
                    r.refresh = true;
                }
                _ => {}
            }
        } else if mods == ShiftState::ALT.0 && key == VK_F4 {
            return self.close();
        }
        self.finish(r)
    }

    /// The form's bounds on the screen.
    #[must_use]
    pub fn bounds(&self) -> Rect {
        Rect::new(self.left, self.top, WIDTH, self.height)
    }

    /// Each image with the name of the `TImage` it goes into.
    #[must_use]
    pub fn images(&self) -> [(&'static str, &Surface); 2] {
        [("tvleiimage", &self.leiste), ("tvimage", &self.image)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::news::Message;
    use crate::simulation::Tick;
    use formats::data::Land;

    fn info() -> InfoTexts {
        InfoTexts {
            months: Vec::new(),
            land: Land::Deutschland,
            currency_factor: 1,
            currency: b"DM".to_vec(),
        }
    }

    fn font() -> Font {
        let (width, height) = (12, 14 * 12);
        let bitmap = formats::bmp::Bitmap {
            width,
            height,
            palette: [vcl::Rgb::default(); 256],
            pixels: vec![0; width * height],
        };
        Font::new(&bitmap, &[[0; 8]; 32], &[[1; 8]; 32])
    }

    fn tv() -> Fernseher {
        let bitmaps = (0..BITMAP_FILES.len())
            .map(|i| Surface::new(400, 400, u8::try_from(i + 1).unwrap()))
            .collect();
        let texts = (0..90)
            .map(|n| [0, 1, 2].map(|v| format!("m{n} v{v}*").into_bytes()))
            .collect();
        let films = (0..90)
            .map(|n| {
                if n == 4 {
                    b"haitauch.avi".to_vec()
                } else {
                    NONE.to_vec()
                }
            })
            .collect();
        Fernseher::new(
            font(),
            bitmaps,
            vec![None; 90],
            (b"Nachrichten".to_vec(), texts),
            films,
            (800, 600),
        )
    }

    fn post(w: &mut World, number: i16, centre: bool) -> Tick {
        let mut tick = Tick::default();
        w.post_message(
            Message {
                number,
                pos: Some((30, 40)),
                centre,
                ..Message::default()
            },
            &mut tick,
        );
        tick
    }

    #[test]
    fn texts() {
        let entry = Entry {
            a: 1,
            b: 2,
            x: 12_345,
            ..Entry::default()
        };
        let names = [b"keiner".to_vec(), b"Anna".to_vec(), b"Bert".to_vec()];
        let line = b"&1 kauft von &2 f\xfcr }1 $. Ende*";
        assert_eq!(
            message_text(line, &entry, &names, &info()),
            b"Anna kauft von Bert f\xfcr 12.345 DM  Ende".to_vec()
        );
        assert_eq!(
            message_text(b"Ohne Stern", &entry, &names, &info()),
            b"Ohne Stern".to_vec()
        );
    }

    #[test]
    fn opens_for_news_and_closes() {
        let mut w = World::default();
        let mut f = tv();
        assert_eq!((f.left, f.top), (224, 133));
        // Nothing posted yet: the button only comes back up.
        let r = f.show(&w, &info(), 0);
        assert!(r.hidden && !f.visible && r.actions.is_empty());
        let tick = post(&mut w, 0, true);
        let r = f.news(&tick.news, [true; 4], &w, &info(), 1000);
        assert!(f.visible && r.shown);
        assert_eq!(r.actions, [Action::ShowTv(0)]);
        assert_eq!(r.scroll, Some((30, 40)));
        assert_eq!(r.sounds, [SOUND_SHOW, SOUND_NEWS]);
        assert!(f.centred);
        assert_eq!(f.bounds(), Rect::new(224, 133, WIDTH, HEIGHT));
        assert!(!f.timer(&w, 6999, false, false).hidden);
        let r = f.timer(&w, 7000, false, false);
        assert!(r.hidden && !f.visible);
        assert_eq!(r.sounds, [SOUND_HIDE]);
        assert_eq!(r.actions, [Action::TvPause(false)]);
    }

    #[test]
    fn the_speaker_reads_the_news() {
        let mut w = World::default();
        let mut f = tv();
        let mut speech = vec![NONE.to_vec(); 90];
        speech[0] = b"oster.wav".to_vec();
        f.set_speech(speech);
        let tick = post(&mut w, 0, false);
        let text = tick.news[0].entry.text;
        let r = f.news(&tick.news, [true; 4], &w, &info(), 1000);
        let dir = u8::try_from(text.rem_euclid(3)).unwrap() + 1;
        assert_eq!(
            r.media,
            [Media::OpenSpeaker {
                dir,
                file: b"oster.wav".to_vec()
            }]
        );
        assert_eq!(r.actions, [Action::ShowTv(0), Action::Speaker(Speaker::Open)]);
        // 3 s later it reads, with the effects off.
        let r = f.timer(&w, 3999, false, false);
        assert!(r.media.is_empty() && r.actions.is_empty());
        let r = f.timer(&w, 4000, false, false);
        assert_eq!(r.media, [Media::EffectsOff, Media::SpeechVolume, Media::PlaySpeaker]);
        assert_eq!(r.actions, [Action::Speaker(Speaker::Reading)]);
        // It reads on past 6 s; the world draws the lights.
        assert!(!f.timer(&w, 20_000, false, false).hidden);
        f.draw_lights(4, Some(2));
        // At its end the TV closes and the effects come back.
        let r = f.timer(&w, 20_100, true, false);
        assert!(r.hidden);
        assert_eq!(
            r.media,
            [
                Media::CloseSpeaker,
                Media::EffectsOn,
                Media::SoundVolume,
                Media::EffectsOnWithSounds
            ]
        );
        assert_eq!(r.actions, [Action::TvPause(false), Action::Speaker(Speaker::Off)]);
        assert_eq!(r.sounds, [SOUND_HIDE]);
        // Without the option there's no speaker.
        f.speech_option = false;
        let tick = post(&mut w, 0, false);
        let r = f.news(&tick.news, [true; 4], &w, &info(), 0);
        assert_eq!(r.media, []);
        assert!(f.timer(&w, AUTO_CLOSE_MS, false, false).hidden);
    }

    #[test]
    fn a_press_stops_the_speaker() {
        let mut w = World::default();
        let mut f = tv();
        f.set_speech(vec![b"oster.wav".to_vec(); 90]);
        let _ = f.news(&post(&mut w, 0, false).news, [true; 4], &w, &info(), 0);
        let r = f.mouse_down(0, TITLE_HEIGHT, ShiftState::LEFT);
        assert_eq!(r.media, [Media::CloseSpeaker, Media::EffectsOn, Media::SoundVolume]);
        assert_eq!(r.actions, [Action::Speaker(Speaker::Off)]);
        // Then the TV closes 6 s after it opened, as without one.
        let _ = f.mouse_up(0, TITLE_HEIGHT, &w, &info());
        assert!(f.timer(&w, AUTO_CLOSE_MS, false, false).hidden);
    }

    #[test]
    fn browses_the_archive() {
        let mut w = World::default();
        for _ in 0..3 {
            let _ = post(&mut w, 0, false);
        }
        let mut f = tv();
        let _ = f.news(&post(&mut w, 4, false).news, [false; 4], &w, &info(), 0);
        assert_eq!(f.cursor, 3);
        let r = f.show(&w, &info(), 0);
        assert!(r.shown);
        assert_eq!(r.actions, [Action::ShowTv(3)]);
        // Back with the second button, with the mouse; it goes no further
        // than the first page.
        let back = (150, 290 + TITLE_HEIGHT);
        for want in [2, 1, 0, 0] {
            let d = f.mouse_down(back.0, back.1, ShiftState::LEFT);
            assert_eq!(d.sounds, [SOUND_PRESS]);
            let u = f.mouse_up(back.0, back.1, &w, &info());
            assert_eq!(u.sounds, [SOUND_RELEASE]);
            assert_eq!(f.cursor, want);
        }
        // On with the arrow key, no further than the last.
        for want in [1, 2, 3, 3] {
            let _ = f.key_down(VK_RIGHT, ShiftState::default(), &w, &info());
            assert_eq!(f.cursor, want);
        }
        // Enter: to the news' square, and back to the view's origin.
        let r = f.key_down(VK_RETURN, ShiftState::default(), &w, &info());
        assert_eq!(r.scroll, Some((30, 40)));
        assert!(f.centred);
        let r = f.key_down(VK_RETURN, ShiftState::default(), &w, &info());
        assert_eq!(r.scroll, Some(w.viewpoint.origin));
        // Off with the fourth button.
        let off = (230, 290 + TITLE_HEIGHT);
        let _ = f.mouse_down(off.0, off.1, ShiftState::LEFT);
        let r = f.mouse_up(off.0, off.1, &w, &info());
        assert!(r.hidden && !f.visible);
    }

    #[test]
    fn a_film_keeps_it_open_after_browsing() {
        let mut w = World::default();
        let mut f = tv();
        // Message 4 has a film: it plays, and the TV closes after it,
        // unless the player browsed.
        let _ = f.news(&post(&mut w, 4, false).news, [true; 4], &w, &info(), 0);
        let r = f.timer(&w, AUTO_CLOSE_MS, false, true);
        assert!(!r.hidden && f.film_playing());
        assert_eq!(
            r.media,
            [
                Media::EffectsOff,
                Media::PlayFilm {
                    file: b"haitauch.avi".to_vec()
                }
            ]
        );
        assert!(!f.timer(&w, AUTO_CLOSE_MS + 100, false, false).hidden);
        let r = f.timer(&w, AUTO_CLOSE_MS + 200, false, true);
        assert!(r.hidden && !f.film_playing());
        assert_eq!(r.media, [Media::StopFilm, Media::EffectsOnWithSounds]);
        let _ = f.news(&post(&mut w, 4, false).news, [true; 4], &w, &info(), 0);
        let _ = f.key_down(VK_RIGHT, ShiftState::default(), &w, &info());
        assert!(!f.timer(&w, AUTO_CLOSE_MS, false, false).hidden);
        assert!(f.visible);
        // Browsing to a page without a film closes it after all.
        let _ = f.key_down(VK_RIGHT, ShiftState::default(), &w, &info());
        let _ = f.news(&post(&mut w, 0, false).news, [true; 4], &w, &info(), 0);
        let _ = f.key_down(VK_RIGHT, ShiftState::default(), &w, &info());
        assert_eq!(f.cursor, 2);
        assert!(f.timer(&w, AUTO_CLOSE_MS, false, false).hidden);
    }

    #[test]
    fn drags_by_the_title_bar() {
        let w = World::default();
        let mut f = tv();
        f.visible = true;
        f.height = HEIGHT;
        let _ = f.mouse_down(100, 5, ShiftState::LEFT);
        assert_eq!(f.height, TITLE_HEIGHT);
        assert!(f.mouse_move(90, 0, ShiftState::LEFT, (800, 600)));
        assert_eq!((f.left, f.top), (214, 128));
        let _ = f.mouse_up(90, 0, &w, &info());
        assert_eq!(f.height, HEIGHT);
        // A double click leaves it rolled up until the next press.
        let _ = f.mouse_down(100, 5, ShiftState::LEFT | ShiftState::DOUBLE);
        let _ = f.mouse_up(100, 5, &w, &info());
        assert_eq!(f.height, TITLE_HEIGHT);
        let _ = f.mouse_down(100, 5, ShiftState::LEFT);
        let _ = f.mouse_up(100, 5, &w, &info());
        assert_eq!(f.height, HEIGHT);
        // The close button.
        let _ = f.mouse_down(5, 5, ShiftState::LEFT);
        assert!(f.mouse_up(5, 5, &w, &info()).hidden);
    }

    fn fixed(e: Enhancement) -> Fernseher {
        let mut f = tv();
        f.enhance.set(e, true);
        f
    }

    #[test]
    fn browsing_keeps_it_open_with_the_fix() {
        for fix in [false, true] {
            let mut w = World::default();
            let _ = post(&mut w, 0, false);
            let mut f = if fix { fixed(Enhancement::TvForm) } else { tv() };
            let _ = f.news(&post(&mut w, 0, false).news, [true; 4], &w, &info(), 0);
            let _ = f.key_down(VK_LEFT, ShiftState::default(), &w, &info());
            assert_eq!(f.timer(&w, AUTO_CLOSE_MS, false, false).hidden, !fix);
        }
    }

    #[test]
    fn back_and_buttons_with_the_fix() {
        for fix in [false, true] {
            let mut w = World::default();
            let _ = post(&mut w, 0, false);
            let mut f = if fix { fixed(Enhancement::TvForm) } else { tv() };
            let _ = f.news(&post(&mut w, 0, false).news, [false; 4], &w, &info(), 0);
            let _ = f.show(&w, &info(), 0);
            // Back to where the view was: the origin 1020:0787 takes
            // 10 rows from again.
            let _ = f.key_down(VK_RETURN, ShiftState::default(), &w, &info());
            let r = f.key_down(VK_RETURN, ShiftState::default(), &w, &info());
            let (i, j) = w.viewpoint.origin;
            assert_eq!(r.scroll, Some(if fix { (i + 10, j) } else { (i, j) }));
            // The back button let go elsewhere; a later press elsewhere
            // let go on it clicks it only in the original.
            let back = (150, 290 + TITLE_HEIGHT);
            let _ = f.mouse_down(back.0, back.1, ShiftState::LEFT);
            let _ = f.mouse_up(5, 100, &w, &info());
            assert_eq!(f.cursor, 1);
            let _ = f.mouse_down(5, 100, ShiftState::LEFT);
            let _ = f.mouse_up(back.0, back.1, &w, &info());
            assert_eq!(f.cursor, i16::from(fix));
        }
    }

    #[test]
    fn the_title_with_the_fix() {
        // The test font's glyphs are black boxes: the title starts `l`
        // further right, over the bar after the close button.
        let first_black = |f: &Fernseher| (0..f.leiste.width).find(|&x| f.leiste.pixel(x, 5) == 0).unwrap();
        let mut f = fixed(Enhancement::TvForm);
        f.draw_title();
        let mut o = tv();
        o.draw_title();
        let (l, _, _, _) = RECTS[usize::from(DRAG)];
        assert!(l > 0);
        assert_eq!(first_black(&f), first_black(&o) + usize::try_from(l).unwrap());
    }

    #[test]
    fn the_speech_with_the_fix() {
        let mut w = World::default();
        let mut f = fixed(Enhancement::TvSound);
        let mut speech = vec![NONE.to_vec(); 90];
        speech[0] = b"oster.wav".to_vec();
        f.set_speech(speech);
        let _ = f.news(&post(&mut w, 0, false).news, [true; 4], &w, &info(), 1000);
        // A message without speech closes the one still open.
        let r = f.news(&post(&mut w, 1, false).news, [true; 4], &w, &info(), 1000);
        assert_eq!(r.media, [Media::CloseSpeaker, Media::EffectsOn, Media::SoundVolume]);
        // The film of message 4 after a speech plays at the sounds' volume
        // and gives the sounds back when it stops.
        let mut speech = vec![b"oster.wav".to_vec(); 90];
        speech[1] = NONE.to_vec();
        f.set_speech(speech);
        let _ = f.news(&post(&mut w, 4, false).news, [true; 4], &w, &info(), 0);
        let _ = f.timer(&w, SPEAKER_MS, false, false);
        let r = f.timer(&w, SPEAKER_MS + 100, true, false);
        assert_eq!(r.media[..2], [Media::SoundVolume, Media::EffectsOff]);
        let r = f.key_down(VK_LEFT, ShiftState::default(), &w, &info());
        assert!(r.media.contains(&Media::StopFilm));
        assert_eq!(r.media.iter().filter(|m| **m == Media::EffectsOn).count(), 1);
        // Browsing to a page without speech doesn't keep the posted
        // message's from starting.
        let _ = f.news(&post(&mut w, 0, false).news, [true; 4], &w, &info(), 0);
        let _ = f.key_down(VK_LEFT, ShiftState::default(), &w, &info());
        let _ = f.key_down(VK_LEFT, ShiftState::default(), &w, &info());
        let r = f.timer(&w, SPEAKER_MS, false, false);
        assert!(r.media.contains(&Media::PlaySpeaker));
    }
}
