//! `TOptionenform`, the options (segment `1060`): the button bar's third
//! button (button 2, or the keys `3` and `O`) creates it and shows it
//! (1010:2c8b, 1060:0789); its close button, "OK", Escape or Enter hide
//! and free it again (1060:0e80). While it's open the clock stands and the
//! other forms are disabled.
//!
//! The form is one `TImage`, `bild`, with the background `optionen.bmp`
//! and some forty widgets of the game's toolkit: the music on or off, which
//! of the seven tracks play and which one does, three volumes, the news the
//! TV shows, the pointers over the buildings, the year's balance, whether
//! auctions wait, the button help, the animations, the game's speed and
//! disasters. Its texts are `OPTIONEN.TXT`'s first 26 lines (1060:0ed0).
//!
//! Most options are the player's ([`Settings`], which the caller keeps and
//! hands to [`Optionenform::show`]); four change the world, and come back
//! as commands ([`Response::commands`]): the speed ([`Action::Speed`]),
//! the animations, "Auktion wartet" and disasters
//! ([`Action::SetOption`]). Single human: in the original they're the
//! one player's settings; in a multiplayer game whoever sets one sets it
//! for everybody.
//!
//! [`Action::Speed`]: crate::command::Action::Speed
//! [`Action::SetOption`]: crate::command::Action::SetOption

use formats::savegame::SaveGame;
use vcl::Rect;
use vcl::surface::{Rop, Surface};

use crate::command;
use crate::enhance::{Enhancement, Enhancements};
use crate::people::FIRST;
use crate::simulation::Tick;
use crate::text::{Align, Font};
use crate::widgets::{
    BitmapId, Button, CheckBox, Frame, Label, Outcome, RadioButton, ScrollBar, ScrollBitmaps, ShiftState, Skin, Sound,
    Widget,
};
use crate::world::World;

/// The bitmaps the form draws with, from `GRAFIK`, in the order
/// [`Optionenform::new`] takes them. // 1060:14f1
pub const BITMAP_FILES: [&str; 17] = [
    "optionen.bmp", // the background, +0x18c, 1060:13f3
    "optch.bmp",    // a check box, +0x190, 1060:1400
    "optchpr.bmp",  // ticked, +0x194, 1060:140a
    "optra.bmp",    // a track's radio button, +0x198, 1060:1416
    "optrapr.bmp",  // on, +0x19c, 1060:1420
    "opttop.bmp",   // "TOP" and "INFO", +0x1a0, 1060:1442
    "opttoppr.bmp", // held down, +0x1a4, 1060:144d
    "optkn.bmp",    // "OK", +0x1a8, 1060:142c
    "optknpr.bmp",  // held down, +0x1ac, 1060:1436
    "optli.bmp",    // a scroll bar's left arrow, +0x1b0, 1060:145a
    "optlipr.bmp",  // held down, +0x1b4, 1060:1464
    "optre.bmp",    // its right arrow, +0x1b8, 1060:1470
    "optrepr.bmp",  // held down, +0x1bc, 1060:147a
    "optscrol.bmp", // its thumb, +0x1c0, 1060:1486
    "optpanel.bmp", // its trough, +0x1c4, 1060:1493
    // `hintergrund`'s, which state 100 loads.
    "close.bmp",    // 1110:54b8
    "closepre.bmp", // 1110:54bc
];
const BACKGROUND: BitmapId = BitmapId(0);
const CHECK: BitmapId = BitmapId(1);
const CHECKED: BitmapId = BitmapId(2);
const RADIO: BitmapId = BitmapId(3);
const RADIO_ON: BitmapId = BitmapId(4);
const TOP_BUTTON: BitmapId = BitmapId(5);
const TOP_BUTTON_PRESSED: BitmapId = BitmapId(6);
const OK_BUTTON: BitmapId = BitmapId(7);
const OK_BUTTON_PRESSED: BitmapId = BitmapId(8);
const SCROLL: ScrollBitmaps = ScrollBitmaps {
    back: BitmapId(9),
    back_pressed: BitmapId(10),
    forward: BitmapId(11),
    forward_pressed: BitmapId(12),
    thumb: BitmapId(13),
    track: BitmapId(14),
};
const CLOSE: BitmapId = BitmapId(15);
const CLOSE_PRESSED: BitmapId = BitmapId(16);

/// The form's size, which 1060:14f1 sets over the DFM's.
pub const WIDTH: i32 = 0x156;
pub const HEIGHT: i32 = 0x179;
/// The form's height while it's dragged by its title bar, and after a
/// double click on it. // 1060:2404
const TITLE_HEIGHT: i32 = 0x10;

/// `bild`'s bounds in the form. // 1060:14f1
pub const LAYOUT: [(&str, Rect); 1] = [("bild", Rect::new(0, 0, WIDTH, HEIGHT))];

/// The seven tracks' texts, which the form gives their check boxes.
/// // 1060:14a1 .. 1060:14e2
pub const TRACK_NAMES: [&[u8]; 7] = [
    b"Anotrian",       // 1060:14a1
    b"Dub'o'What?",    // 1060:14aa
    b"Floatin'",       // 1060:14b6
    b"Samba do...",    // 1060:14bf
    b"Ukanchi",        // 1060:14cb
    b"Cuban Memories", // 1060:14d3
    b"Holiday Island", // 1060:14e2
];

/// The tracks' files in `MUSIK`, `string[12]`s. // 1110:31d6
pub const TRACK_FILES: [&str; 7] = [
    "anotrian.mid", // 1110:31d6
    "dub.mid",      // 1110:31e3
    "floatin'.mid", // 1110:31f0
    "samba.mid",    // 1110:31fd
    "ukanchi.mid",  // 1110:320a
    "cuban.mid",    // 1110:3217
    "holiday.mid",  // 1110:3224
];

/// The speed slider's range: 100 less the game's speed, 1 to 20 ticks an
/// hour. // 1060:0789
const SPEED_MIN: i32 = 0x50;
const SPEED_MAX: i32 = 99;

/// The keys `FormKeyDown` knows. // 1060:2c72
const VK_RETURN: u16 = 0x0d;
const VK_ESCAPE: u16 = 0x1b;
const VK_SPACE: u16 = 0x20;
const VK_LEFT: u16 = 0x25;
const VK_RIGHT: u16 = 0x27;
const VK_NUMPAD1: u16 = 0x61;
const VK_ADD: u16 = 0x6b;
const VK_SUBTRACT: u16 = 0x6d;
const VK_F1: u16 = 0x70;
const VK_F4: u16 = 0x73;
const VK_PLUS: u16 = 0xbb;
const VK_MINUS: u16 = 0xbd;

/// The world's options the form (and the overview map) sets, with
/// [`command::Action::SetOption`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WorldOption {
    /// "Animationen": people walk ([`crate::simulation::Options::people_move`]).
    /// // 1060:05cb, 1110:0055
    PeopleMove,
    /// "Auktion wartet". // 1060:05a7, 1110:0063
    AuctionWaits,
    /// "Katastrophen". // 1060:0637, 1110:0065
    Disasters,
    /// New people come ([`crate::simulation::Options::people_spawn`]). Not
    /// on this form: the overview map's scroll arrows stop them while
    /// they're held and let them come again when they're let go
    /// (10b8:21c7, 10b8:21fa, 10b8:293e, 10b8:2971,
    /// `crate::uebersichtsmap`). // 1110:0056
    PeopleSpawn,
}

/// The kinds of people 1088:7ee3 and 1088:7f4a leave alone: the tools'
/// pictures (6), the money (0x2a), oil (0x46) and kinds 1, 0x19, 0x25 and
/// 0x2b to 0x2d.
const FLAG_16_KEPT: [u8; 9] = [6, 0x2a, 0x46, 1, 0x19, 0x25, 0x2b, 0x2c, 0x2d];

impl World {
    /// The speed slider's `Click`: the game's speed, in ticks per hour, is
    /// 100 less its position. Single human: whoever moves it sets the
    /// speed for everybody. // 1060:0619
    pub(crate) fn set_speed(&mut self, speed: i32) {
        self.clock.speed = speed;
    }

    /// A check box for one of the world's options. Single human: the
    /// original's options are the one player's; here whoever ticks one
    /// sets it for everybody.
    pub(crate) fn set_option(&mut self, option: WorldOption, on: bool, tick: &mut Tick) {
        match option {
            WorldOption::PeopleMove => {
                // 1060:05cb
                self.options.people_move = on;
                self.set_people_flag_16(on); // 1088:7f4a, 1088:7ee3
                // The original then redraws the island without the people
                // (off, 1050:2700), or runs `people_refresh` (on), which
                // places them for the view and spawns new ones there. The
                // view the world has is the last one handed to it.
                if on {
                    self.people_refresh(tick); // 1088:7fb1
                }
                tick.redraw_all = true;
            }
            WorldOption::AuctionWaits => self.options.auction_waits = on, // 1060:05a7
            WorldOption::PeopleSpawn => self.options.people_spawn = on,
            WorldOption::Disasters => {
                // 1060:0637
                self.disasters = on;
                if !on {
                    // BUG(original): switched off once, disasters lower the score for good, even after they're switched on again.
                    self.played_without_disasters = true;
                }
            }
        }
    }

    /// `+0x16` of the people from [`FIRST`] to the last in use, but those
    /// of [`FLAG_16_KEPT`]'s kinds, set (1088:7f4a) or cleared (1088:7ee3).
    fn set_people_flag_16(&mut self, on: bool) {
        let last = self.people.last;
        if last < FIRST {
            return;
        }
        for n in FIRST..=last {
            let Some(p) = usize::try_from(n).ok().and_then(|n| self.people.records.get_mut(n)) else {
                break;
            };
            if !FLAG_16_KEPT.contains(&p.kind()) {
                let mut flags = p.flags();
                flags[2] = u8::from(on);
                p.set_flags(flags);
            }
        }
    }
}

/// The player's own options, which the form shows and changes: the
/// original's globals, which the map files save (but the playlist). The
/// defaults are `init_game_defaults` (1000:1ea0), 1018:37de and `DGROUP`'s
/// initial values.
#[allow(clippy::struct_excessive_bools)] // One flag for each of the original's options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Settings {
    /// "Musik": the music plays. // 1110:005b
    pub music: bool,
    /// Which tracks play, one after the other. // 1110:6ef2..1110:6ef8
    pub playlist: [bool; 7],
    /// The track playing, 0 to 6. // 1110:6efa
    pub track: u8,
    /// 0 to 100: the MIDI volume (1018:35f0). // 1110:6330
    pub music_volume: i32,
    /// "Geräusche": `play_sound` plays. // 1110:0061
    pub sounds: bool,
    /// 0 to 100: the wave volume (10a0:2453). // 1110:6334
    pub sound_volume: i32,
    /// "Sprachausgabe": the TV reads the news out. // 1110:0066
    pub speech: bool,
    /// 0 to 100: the wave volume while it speaks (10a0:24bc). // 1110:6338
    pub speech_volume: i32,
    /// "Nachrichten:": which news the TV shows (which pauses the game):
    /// "Auktionen", "Neue Gebäude", "Sabotage", "Gratulationen", news of
    /// kinds 0 to 3 (`1110:199e`, `post_message`, 1048:0a54).
    /// // 1110:005c..1110:005f
    pub news: [bool; 4],
    /// "Jahresbilanz": the money menu opens at the end of a year
    /// (`year_end`, 1008:4f64). // 1110:0064
    pub year_balance: bool,
    /// "Buttonhilfe": the button bar's tooltips. // 1110:0060
    pub button_help: bool,
    /// "Gebäudeumrandung:" "Besitzer": pointers in the owners' colours
    /// over the buildings. // 1110:0058
    pub owner_pointers: bool,
    /// Or "Auslastung": pointers for how full they are; neither is
    /// "Keine". // 1110:0059
    pub occupancy_pointers: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            music: true,
            playlist: [true; 7],
            track: 0,
            // The original reads the system's wave volume (10a0:2f5e).
            music_volume: 100,
            sounds: true,
            sound_volume: 100,
            speech: true,
            speech_volume: 100,
            news: [true; 4],
            year_balance: true,
            button_help: true,
            owner_pointers: false,
            occupancy_pointers: true,
        }
    }
}

impl Settings {
    /// The options after "Neues Spiel" (`back_to_start_menu`,
    /// en 1010:3582): `init_game_defaults` (en 1000:22b4) sets every one
    /// but the speech (`1110:0066`) back to its default, and `init_sound`
    /// (en 1040:3c33) the volumes to the system's (the port: 100) when
    /// there's sound (`sound_on`); the playlist is set when the next game
    /// starts (1018:37de).
    #[must_use]
    pub fn after_game(&self, sound_on: bool) -> Settings {
        let mut s = Settings {
            speech: self.speech,
            ..Settings::default()
        };
        if !sound_on {
            s.music_volume = self.music_volume;
            s.sound_volume = self.sound_volume;
            s.speech_volume = self.speech_volume;
        }
        s
    }

    /// The settings a map file holds, the rest as they are. // 1048:2a19
    #[must_use]
    pub fn with_map(&self, save: &SaveGame) -> Settings {
        let byte = |dest: &str, old: bool| {
            save.fields(dest)
                .last()
                .and_then(|d| d.first().copied())
                .map_or(old, |b| b != 0)
        };
        let mut s = *self;
        s.music = byte("1110:005b", s.music);
        s.sounds = byte("1110:0061", s.sounds);
        s.button_help = byte("1110:0060", s.button_help);
        for (n, dest) in ["1110:005c", "1110:005d", "1110:005e", "1110:005f"].iter().enumerate() {
            s.news[n] = byte(dest, s.news[n]);
        }
        s.year_balance = byte("1110:0064", s.year_balance);
        s.speech = byte("1110:0066", s.speech);
        s.owner_pointers = byte("1110:0058", s.owner_pointers);
        s.occupancy_pointers = byte("1110:0059", s.occupancy_pointers);
        s
    }

    /// Writes the options a map file saves with the view but
    /// [`crate::savegame::MapView`]'s own fields (the news, the pointers)
    /// into its other fields `view`: the music, the tooltips, the sounds,
    /// the year's balance and the speech. // 1048:2bae
    pub fn store(&self, view: &mut crate::savegame::Fields) {
        for (dest, on) in [
            ("1110:005b", self.music),
            ("1110:0060", self.button_help),
            ("1110:0061", self.sounds),
            ("1110:0064", self.year_balance),
            ("1110:0066", self.speech),
        ] {
            view.set(dest, 0, &[u8::from(on)]);
        }
    }
}

/// What the form asks of the game besides the world's commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// The close button, "OK", Escape, Enter or Alt+F4: the form hides,
    /// the other forms are enabled again, the button bar's button comes
    /// up, the clock runs again and the main screen gets the focus.
    /// // 1060:0151, 1060:0163, 1060:0e80
    Close,
    /// "TOP" (or `T`): the high scores (`Thighscoreform`, 1040:2579) open
    /// over the form, which is disabled until they close. // 1060:0175
    Highscores,
    /// "INFO" (or Space): `TCreditform` (`credits.bmp`, 1030:3c42) opens
    /// over the form, which is disabled until it closes. // 1060:01ab
    Credits,
    /// "INFO" with `enhancements_screen`: the list of the enhancements
    /// (`crate::enhanceform`) opens over the form instead, with the
    /// credits a button away. A port extension.
    Enhancements,
}

/// A change to [`Settings`] the caller carries out at once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// The music starts with [`Settings::track`] (1018:3850) or stops
    /// (1018:38c5).
    Music(bool),
    /// Another track: the one playing stops, the player is given
    /// [`TRACK_FILES`]' file, and with the music on it plays from the
    /// start. // 1060:02b7
    Track(u8),
    /// The MIDI volume (`midiOutSetVolume`, 1018:35f0).
    MusicVolume(i32),
    /// The wave volume (`waveOutSetVolume`, 10a0:2453), only with sound.
    SoundVolume(i32),
    /// The pointers over the buildings: the main screen is drawn again
    /// with [`Settings::owner_pointers`] and
    /// [`Settings::occupancy_pointers`]. // 1060:0656
    Pointers,
}

/// What an event did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[must_use]
pub struct Response {
    /// Something changed and the handler refreshes the image.
    pub refresh: bool,
    /// The widgets' sounds, in order (`play_sound`, 10a0:30b6).
    pub sounds: Vec<Sound>,
    /// The world's options that changed, for the caller to apply as the
    /// player's commands, in order.
    pub commands: Vec<command::Action>,
    /// The player's settings that changed.
    pub changes: Vec<Change>,
    pub action: Option<Action>,
    /// The form moved or changed its height (dragged by its title bar).
    pub moved: bool,
}

/// Which widget, by what it's for. The instance offsets are the
/// original's. // 1060:14f1
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Id {
    Close,        // +0x1c8
    Ok,           // +0x2de
    Top,          // +0x3f4
    Info,         // +0x50a
    Music,        // +0x620
    Track(u8),    // +0x738 + 0x118 n
    Playing(u8),  // +0xee0 + 0x118 n
    MusicVolume,  // +0x1688
    Sounds,       // +0x16c6
    SoundVolume,  // +0x17de
    Speech,       // +0x181c
    SpeechVolume, // +0x1934
    News(u8),     // +0x1972 + 0x118 n
    YearBalance,  // +0x1dd2
    AuctionWaits, // +0x1eea
    ButtonHelp,   // +0x2002
    Animations,   // +0x211a
    Speed,        // +0x2232
    Disasters,    // +0x2270
    Pointers(u8), // +0x2c00 + 0x118 n: owner, occupancy, none
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Item {
    Button(Button),
    Check(CheckBox),
    Radio(RadioButton),
    Scroll(ScrollBar),
}

impl Item {
    fn widget(&self) -> &dyn Widget {
        match self {
            Item::Button(w) => w,
            Item::Check(w) => w,
            Item::Radio(w) => w,
            Item::Scroll(w) => w,
        }
    }

    fn widget_mut(&mut self) -> &mut dyn Widget {
        match self {
            Item::Button(w) => w,
            Item::Check(w) => w,
            Item::Radio(w) => w,
            Item::Scroll(w) => w,
        }
    }
}

fn label(left: i32, top: i32, width: i32, align: Align, colour: i32) -> Label {
    Label::new(Frame::new(left, top, width, 0x10), b"", align, colour)
}

/// A check box as the form makes it: `radio_init` with a check box's VMT,
/// so the colour for its text when ticked (+0x118) stays 0; its drawing
/// (`ui_radio_draw2`, 1010:16b5) doesn't read it, so it's the text's
/// colour here.
fn check(left: i32, top: i32, text: &[u8]) -> Item {
    Item::Check(CheckBox::new(
        Label::new(Frame::new(left, top, 0x7a, 0x10), text, Align::Left, 9),
        CHECK,
        CHECKED,
        9,
    ))
}

fn radio(left: i32, top: i32, width: i32, off: BitmapId, on: BitmapId) -> Item {
    Item::Radio(RadioButton::new(
        Label::new(Frame::new(left, top, width, 0x10), b"", Align::Left, 9),
        off,
        on,
    ))
}

fn scroll(top: i32) -> Item {
    Item::Scroll(ScrollBar::new(Frame::new(0xc, top, 0x97, 0x10), SCROLL))
}

/// The widgets in the order the form makes them, which is the order a
/// press tries them. // 1060:14f1, 1060:2404
fn widgets() -> Vec<(Id, Item)> {
    let mut w = vec![
        (
            Id::Close,
            Item::Button(Button::new(Frame::new(0, 0, 0x10, 0x10), CLOSE, CLOSE_PRESSED, b"", 9)),
        ),
        (
            Id::Ok,
            Item::Button(Button::new(
                Frame::new(0xb3, 0x158, 0x96, 0x16),
                OK_BUTTON,
                OK_BUTTON_PRESSED,
                b"",
                9,
            )),
        ),
        (
            Id::Top,
            Item::Button(Button::new(
                Frame::new(0xb3, 0x13e, 0x46, 0x16),
                TOP_BUTTON,
                TOP_BUTTON_PRESSED,
                b"",
                9,
            )),
        ),
        (
            Id::Info,
            Item::Button(Button::new(
                Frame::new(0x103, 0x13e, 0x46, 0x16),
                TOP_BUTTON,
                TOP_BUTTON_PRESSED,
                b"",
                9,
            )),
        ),
        (Id::Music, check(0xc, 0x18, b"")),
    ];
    for (n, name) in (0u8..).zip(TRACK_NAMES) {
        w.push((Id::Track(n), check(0xc, 0x2c + 0x12 * i32::from(n), name)));
    }
    for n in 0..7u8 {
        w.push((
            Id::Playing(n),
            radio(0x92, 0x2c + 0x12 * i32::from(n), 0x10, RADIO, RADIO_ON),
        ));
    }
    w.extend([
        (Id::MusicVolume, scroll(0xbe)),
        (Id::Sounds, check(0xc, 0xd7, b"")),
        (Id::SoundVolume, scroll(0xfb)),
        (Id::Speech, check(0xc, 0x113, b"")),
        (Id::SpeechVolume, scroll(0x137)),
    ]);
    for n in 0..4u8 {
        w.push((Id::News(n), check(0xb3, 0x2a + 0x12 * i32::from(n), b"")));
    }
    w.extend([
        (Id::YearBalance, check(0xb3, 0xd9, b"")),
        (Id::AuctionWaits, check(0xb3, 0xeb, b"")),
        (Id::ButtonHelp, check(0xb3, 0xfd, b"")),
        (Id::Animations, check(0xb3, 0x10f, b"")),
        (Id::Speed, scroll(0x161)),
        (Id::Disasters, check(0xb3, 0x127, b"")),
    ]);
    for n in 0..3u8 {
        w.push((
            Id::Pointers(n),
            radio(0xb3, 0x8b + 0x12 * i32::from(n), 0x7a, CHECK, CHECKED),
        ));
    }
    w
}

/// The labels, in the order the form makes them: the title, the three
/// "Lautstärke:", "Spielgeschwindigkeit:", "Nachrichten:",
/// "Gebäudeumrandung:" and "Sonstiges:". // 1060:14f1
fn labels() -> Vec<Label> {
    vec![
        label(0x10, 0, 0x146, Align::Center, 1),  // +0x2388
        label(0x18, 0xac, 0x96, Align::Left, 9),  // +0x2497
        label(0x18, 0xe9, 0x96, Align::Left, 9),  // +0x25a6
        label(0x18, 0x125, 0x96, Align::Left, 9), // +0x26b5
        label(0x18, 0x14f, 0x96, Align::Left, 9), // +0x27c4
        label(0xb3, 0x17, 0x96, Align::Left, 9),  // +0x28d3
        label(0xb3, 0x78, 0x96, Align::Left, 9),  // +0x29e2
        label(0xb3, 0xc6, 0x96, Align::Left, 9),  // +0x2af1
    ]
}

/// Where each of `OPTIONEN.TXT`'s first 26 lines goes: a label, by its
/// index in [`labels`], or a widget. // 1060:0ed0
#[derive(Debug, Clone, Copy)]
enum Text {
    Label(usize),
    Widget(Id),
}

const TEXTS: [Text; 26] = [
    Text::Label(0),           // "OPTIONEN"
    Text::Widget(Id::Music),  // "Musik"
    Text::Label(1),           // "Lautstärke:"
    Text::Widget(Id::Sounds), // "Geräusche"
    Text::Label(2),
    Text::Widget(Id::Speech), // "Sprachausgabe"
    Text::Label(3),
    Text::Label(4), // "Spielgeschwindigkeit:"
    Text::Label(5), // "Nachrichten:"
    Text::Widget(Id::News(0)),
    Text::Widget(Id::News(1)),
    Text::Widget(Id::News(2)),
    Text::Widget(Id::News(3)),
    Text::Label(6), // "Gebäudeumrandung:"
    Text::Widget(Id::Pointers(0)),
    Text::Widget(Id::Pointers(1)),
    Text::Widget(Id::Pointers(2)),
    Text::Label(7), // "Sonstiges:"
    Text::Widget(Id::YearBalance),
    Text::Widget(Id::AuctionWaits),
    Text::Widget(Id::ButtonHelp),
    Text::Widget(Id::Animations),
    Text::Widget(Id::Disasters),
    Text::Widget(Id::Ok), // "OK"
    Text::Widget(Id::Top),
    Text::Widget(Id::Info),
];

/// The keys without Ctrl and Alt that tick a check box, select a radio
/// button or press a button. // 1060:2c72
const KEYS: [(u16, Id); 33] = [
    (0x4d, Id::Music), // M
    (0x31, Id::Track(0)),
    (0x32, Id::Track(1)),
    (0x33, Id::Track(2)),
    (0x34, Id::Track(3)),
    (0x35, Id::Track(4)),
    (0x36, Id::Track(5)),
    (0x37, Id::Track(6)),
    (VK_NUMPAD1, Id::Playing(0)),
    (VK_NUMPAD1 + 1, Id::Playing(1)),
    (VK_NUMPAD1 + 2, Id::Playing(2)),
    (VK_NUMPAD1 + 3, Id::Playing(3)),
    (VK_NUMPAD1 + 4, Id::Playing(4)),
    (VK_NUMPAD1 + 5, Id::Playing(5)),
    (VK_NUMPAD1 + 6, Id::Playing(6)),
    (0x47, Id::Sounds), // G
    (0x53, Id::Speech), // S
    (VK_F1, Id::News(0)),
    (VK_F1 + 1, Id::News(1)),
    (VK_F1 + 2, Id::News(2)),
    (VK_F1 + 3, Id::News(3)),
    (VK_F1 + 4, Id::Pointers(0)),
    (VK_F1 + 5, Id::Pointers(1)),
    (VK_F1 + 6, Id::Pointers(2)),
    (VK_F1 + 8, Id::YearBalance),
    (VK_F1 + 9, Id::AuctionWaits),
    (VK_F1 + 10, Id::ButtonHelp),
    (VK_F1 + 11, Id::Animations),
    (0x4b, Id::Disasters), // K
    (VK_ESCAPE, Id::Close),
    (VK_RETURN, Id::Close),
    (VK_SPACE, Id::Info),
    (0x54, Id::Top), // T
];

/// The options form.
#[allow(clippy::struct_excessive_bools)] // The original's fields.
#[derive(Debug, Clone)]
pub struct Optionenform {
    font: Font,
    bitmaps: Vec<Surface>,
    empty: Surface,
    /// `OPTIONEN.TXT`'s lines.
    texts: Vec<Vec<u8>>,
    /// `bild`'s picture.
    image: Surface,
    /// The form is shown.
    pub visible: bool,
    /// The form's top left on the screen, and its height (16 while it's
    /// dragged or rolled up).
    pub left: i32,
    pub top: i32,
    pub height: i32,
    widgets: Vec<(Id, Item)>,
    labels: Vec<Label>,
    /// The player's options as the form has them.
    settings: Settings,
    /// The setup has sound (`1110:0062`).
    sound_enabled: bool,
    /// The title bar is held. // +0x180
    dragging: bool,
    /// Where in the form the drag took hold. // +0x181, +0x183
    grip: (i32, i32),
    /// The height to go back to. // +0x189
    full_height: i32,
    /// The title bar was double-clicked: the form stays rolled up to it.
    /// // +0x18b
    rolled_up: bool,
    /// The player's view enhancements (`mouse_buttons`, `scroll_bars`).
    pub enhance: Enhancements,
}

impl Optionenform {
    /// The form, not shown. `bitmaps` are those of [`BITMAP_FILES`],
    /// `texts` `OPTIONEN.TXT`'s lines (`formats::data::text_lines`).
    #[must_use]
    pub fn new(font: Font, bitmaps: Vec<Surface>, texts: Vec<Vec<u8>>) -> Self {
        Optionenform {
            font,
            bitmaps,
            empty: Surface::new(0, 0, 0),
            texts,
            image: Surface::new(0x156, 0x179, 0),
            visible: false,
            left: 0,
            top: 0,
            height: HEIGHT,
            widgets: widgets(),
            labels: labels(),
            settings: Settings::default(),
            sound_enabled: true,
            dragging: false,
            grip: (0, 0),
            full_height: HEIGHT,
            rolled_up: false,
            enhance: Enhancements::ORIGINAL,
        }
    }

    /// The button bar's button creates the form and shows it: at the top
    /// of a `screen` of that size, centred across; new widgets with
    /// `OPTIONEN.TXT`'s texts, set from `settings`, `world` and
    /// `sound_enabled` (`1110:0062`; without sound the sound volume shows
    /// 0); the background and all the widgets drawn. The original also
    /// disables the other forms, hides the button help and stops the
    /// clock; that's the caller's. // 1060:0789, 1060:14f1, 1060:0ed0
    pub fn show(&mut self, settings: &Settings, world: &World, sound_enabled: bool, screen: (i32, i32)) {
        self.settings = *settings;
        self.sound_enabled = sound_enabled;
        self.widgets = widgets();
        self.labels = labels();
        self.load_texts();
        self.left = (screen.0 - WIDTH) / 2;
        self.top = 0;
        self.height = HEIGHT;
        self.full_height = HEIGHT;
        self.dragging = false;
        self.rolled_up = false;
        let s = self.settings;
        let speed_position = 100i32.wrapping_sub(world.clock.speed);
        for (id, item) in &mut self.widgets {
            match (*id, item) {
                (Id::Music, Item::Check(c)) => c.radio.on = s.music,
                (Id::Track(n), Item::Check(c)) => c.radio.on = s.playlist[usize::from(n)],
                (Id::Playing(n), Item::Radio(r)) => r.on = s.track == n,
                (Id::MusicVolume, Item::Scroll(b)) => set_range(b, 0, 100, s.music_volume),
                (Id::Sounds, Item::Check(c)) => c.radio.on = s.sounds,
                (Id::SoundVolume, Item::Scroll(b)) => {
                    set_range(b, 0, 100, if sound_enabled { s.sound_volume } else { 0 });
                }
                (Id::Speech, Item::Check(c)) => c.radio.on = s.speech,
                (Id::SpeechVolume, Item::Scroll(b)) => set_range(b, 0, 100, s.speech_volume),
                (Id::News(n), Item::Check(c)) => c.radio.on = s.news[usize::from(n)],
                (Id::YearBalance, Item::Check(c)) => c.radio.on = s.year_balance,
                (Id::AuctionWaits, Item::Check(c)) => c.radio.on = world.options.auction_waits,
                (Id::ButtonHelp, Item::Check(c)) => c.radio.on = s.button_help,
                (Id::Animations, Item::Check(c)) => c.radio.on = world.options.people_move,
                (Id::Speed, Item::Scroll(b)) => set_range(b, SPEED_MIN, SPEED_MAX, speed_position),
                (Id::Disasters, Item::Check(c)) => c.radio.on = world.disasters,
                (Id::Pointers(n), Item::Radio(r)) => {
                    r.on = match n {
                        0 => s.owner_pointers,
                        1 => s.occupancy_pointers,
                        _ => !s.owner_pointers && !s.occupancy_pointers,
                    };
                }
                _ => {}
            }
        }
        let clip = self.image.bounds();
        let background = self.bitmaps.get(BACKGROUND.0).unwrap_or(&self.empty);
        self.image.blit(0, 0, background, clip, Rop::Copy, clip);
        let Optionenform {
            font,
            bitmaps,
            image,
            widgets,
            labels,
            ..
        } = self;
        let skin = Skin { font, bitmaps };
        for (_, item) in widgets.iter_mut() {
            item.widget_mut().show(image, &skin);
        }
        for l in labels.iter_mut() {
            l.show(image, &skin);
        }
        self.visible = true;
    }

    /// `optionen_load_texts`: `OPTIONEN.TXT`'s first 26 lines into the
    /// labels and widgets. A short file leaves the rest empty. // 1060:0ed0
    fn load_texts(&mut self) {
        for (n, target) in TEXTS.iter().enumerate() {
            let text = self.texts.get(n).cloned().unwrap_or_default();
            let text = text[..text.len().min(255)].to_vec();
            match *target {
                Text::Label(l) => {
                    if let Some(l) = self.labels.get_mut(l) {
                        l.text = text;
                    }
                }
                Text::Widget(id) => match self.item_mut(id) {
                    Some(Item::Button(b)) => b.text = text,
                    Some(Item::Check(c)) => c.radio.text = text,
                    Some(Item::Radio(r)) => r.text = text,
                    _ => {}
                },
            }
        }
    }

    /// The player's options as the form has them now.
    #[must_use]
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// `bild`'s picture.
    #[must_use]
    pub fn image(&self) -> &Surface {
        &self.image
    }

    /// Each image with the name of the `TImage` it goes into.
    #[must_use]
    pub fn images(&self) -> [(&'static str, &Surface); 1] {
        [("bild", &self.image)]
    }

    /// The form's bounds on the screen.
    #[must_use]
    pub fn bounds(&self) -> Rect {
        Rect::new(self.left, self.top, WIDTH, self.height)
    }

    /// Whether `x`, `y` (in the form) is on the form as it's shown.
    #[must_use]
    pub fn contains(&self, x: i32, y: i32) -> bool {
        self.visible && (0..WIDTH).contains(&x) && (0..self.height).contains(&y)
    }

    fn item_mut(&mut self, id: Id) -> Option<&mut Item> {
        self.widgets.iter_mut().find(|(i, _)| *i == id).map(|(_, w)| w)
    }

    fn checked(&self, id: Id) -> bool {
        match self.widgets.iter().find(|(i, _)| *i == id).map(|(_, w)| w) {
            Some(Item::Check(c)) => c.checked(),
            Some(Item::Radio(r)) => r.on,
            _ => false,
        }
    }

    fn position(&self, id: Id) -> i32 {
        match self.widgets.iter().find(|(i, _)| *i == id).map(|(_, w)| w) {
            Some(Item::Scroll(b)) => b.position,
            _ => 0,
        }
    }

    /// Draws widget `id` again over what's there.
    fn draw(&mut self, id: Id) {
        let Optionenform {
            font,
            bitmaps,
            image,
            widgets,
            ..
        } = self;
        if let Some((_, item)) = widgets.iter().find(|(i, _)| *i == id) {
            item.widget().draw(image, &Skin { font, bitmaps });
        }
    }

    /// What the original does with an event's [`Outcome`] for widget
    /// `id`: the sound, `Click`, then the drawing.
    fn finish(&mut self, id: Id, out: Outcome, r: &mut Response) {
        r.sounds.extend(out.sound);
        if out.clicked {
            self.click(id, r);
        }
        if out.repaint {
            self.draw(id);
        }
    }

    /// `radio_deselect` of radio button `id`, redrawn if it was on.
    /// // 10c0:06fe
    fn deselect(&mut self, id: Id) {
        if let Some(Item::Radio(radio)) = self.item_mut(id)
            && radio.deselect().repaint
        {
            self.draw(id);
        }
    }

    /// The widgets' `Click`s, the form's overrides of VMT+8.
    fn click(&mut self, id: Id, r: &mut Response) {
        let on = self.checked(id);
        let position = self.position(id);
        match id {
            Id::Close | Id::Ok => r.action = Some(Action::Close), // 1060:0151, 1060:0163
            Id::Top => r.action = Some(Action::Highscores),       // 1060:0175
            Id::Info => {
                // 1060:01ab; the port's list of enhancements in its place.
                r.action = Some(if self.enhance.has(Enhancement::EnhancementsScreen) {
                    Action::Enhancements
                } else {
                    Action::Credits
                });
            }
            Id::Music => {
                // 1060:01dc: 1018:3850 starts the music only if it's off,
                // 1018:38c5 stops it only if it's on.
                if self.settings.music != on {
                    self.settings.music = on;
                    r.changes.push(Change::Music(on));
                }
            }
            Id::Track(n) => {
                // 1060:01fa: the track playing can't be unticked.
                if n == self.settings.track {
                    if let Some(Item::Check(c)) = self.item_mut(id) {
                        c.radio.on = true;
                    }
                } else {
                    self.settings.playlist[usize::from(n)] = on;
                }
            }
            Id::Playing(n) => self.play(n, r),
            Id::MusicVolume => {
                self.settings.music_volume = position; // 1060:04e2, 1018:35f0
                r.changes.push(Change::MusicVolume(position));
            }
            Id::Sounds => self.settings.sounds = on, // 1060:04f9
            Id::SoundVolume => {
                // 1060:050b, 10a0:2453: only with sound.
                if self.sound_enabled {
                    self.settings.sound_volume = position;
                    r.changes.push(Change::SoundVolume(position));
                }
            }
            Id::Speech => self.settings.speech = on, // 1060:0522
            Id::SpeechVolume => self.settings.speech_volume = position, // 1060:0534
            Id::News(n) => self.settings.news[usize::from(n)] = on, // 1060:054d..1060:0583
            Id::YearBalance => self.settings.year_balance = on, // 1060:0595
            Id::ButtonHelp => self.settings.button_help = on, // 1060:05b9
            // 1060:05a7
            Id::AuctionWaits => r
                .commands
                .push(command::Action::SetOption(WorldOption::AuctionWaits, on)),
            // 1060:05cb
            Id::Animations => r.commands.push(command::Action::SetOption(WorldOption::PeopleMove, on)),
            // 1060:0637
            Id::Disasters => r.commands.push(command::Action::SetOption(WorldOption::Disasters, on)),
            // 1060:0619
            Id::Speed => r.commands.push(command::Action::Speed(100i32.wrapping_sub(position))),
            Id::Pointers(n) => {
                // 1060:0656, 1060:06ba, 1060:071e
                self.settings.owner_pointers = n == 0;
                self.settings.occupancy_pointers = n == 1;
                for other in (0..3).filter(|&o| o != n) {
                    self.deselect(Id::Pointers(other));
                }
                r.changes.push(Change::Pointers);
            }
        }
    }

    /// A track's radio button: a track that isn't ticked can't play, and
    /// its button goes off again; otherwise the others go off, the music
    /// stops and the track is the one to play. // 1060:02b7
    fn play(&mut self, n: u8, r: &mut Response) {
        if !self.settings.playlist[usize::from(n)] {
            if let Some(Item::Radio(radio)) = self.item_mut(Id::Playing(n)) {
                radio.on = false;
            }
            return;
        }
        for other in (0..7).filter(|&o| o != n) {
            self.deselect(Id::Playing(other));
        }
        self.settings.track = n;
        r.changes.push(Change::Track(n));
    }

    /// `bildMouseDown`, with `x`, `y` in the image and the shift state
    /// (whichever button it is). The widgets are offered the press in
    /// turn until one takes it; if none does, a press on the title bar
    /// right of the close button grabs the form, which rolls up to its
    /// title bar until the mouse comes up, and stays so after a double
    /// click. With `mouse_buttons` only the left button does anything.
    /// // 1060:2404
    pub fn mouse_down(&mut self, x: i32, y: i32, shift: ShiftState) -> Response {
        let mut r = Response::default();
        self.dragging = false;
        // BUG(original): the handler doesn't look at the mouse button, so the right button ticks the boxes too.
        if self.enhance.has(Enhancement::MouseButtons) && !shift.contains(ShiftState::LEFT) {
            return r;
        }
        for i in 0..self.widgets.len() {
            let (id, item) = &mut self.widgets[i];
            let id = *id;
            let out = match item {
                Item::Button(b) => b.mouse_down(x, y),
                Item::Check(c) => c.mouse_down(x, y),
                Item::Radio(radio) => radio.mouse_down(x, y),
                Item::Scroll(b) => b.mouse_down(x, y, self.enhance),
            };
            if out.handled {
                self.finish(id, out, &mut r);
                r.refresh = true;
                return r;
            }
        }
        if (0x10..=WIDTH).contains(&x) && (0..0x10).contains(&y) {
            self.dragging = true;
            self.grip = (x, y);
            if !self.rolled_up {
                self.full_height = self.height;
                self.height = TITLE_HEIGHT;
                r.moved = true;
            }
            self.rolled_up = shift.contains(ShiftState::DOUBLE);
        }
        r
    }

    /// `bildMouseMove`: a scroll bar's thumb follows the mouse; or, with
    /// the left button down on the title bar, the form does, kept on the
    /// `screen`. // 1060:29f0
    pub fn mouse_move(&mut self, x: i32, y: i32, shift: ShiftState, screen: (i32, i32)) -> Response {
        let mut r = Response::default();
        for i in 0..self.widgets.len() {
            let (id, item) = &mut self.widgets[i];
            let id = *id;
            if let Item::Scroll(b) = item {
                let out = b.mouse_move(x, y);
                if out.handled {
                    self.finish(id, out, &mut r);
                    r.refresh = true;
                    return r;
                }
            }
        }
        if self.dragging && shift.contains(ShiftState::LEFT) {
            let (sx, sy) = (self.left + x, self.top + y);
            self.left = (sx - self.grip.0).max(0).min(screen.0 - WIDTH);
            self.top = (sy - self.grip.1).max(0).min(screen.1 - self.height);
            r.moved = true;
        }
        r
    }

    /// `scroll_bars`' repeat of a scroll bar's arrow held down (a port
    /// extension, [`ScrollBar::repeat`]), `ms` after the last call.
    pub fn repeat(&mut self, ms: u32) -> Response {
        let mut r = Response::default();
        for i in 0..self.widgets.len() {
            let (id, item) = &mut self.widgets[i];
            let id = *id;
            if let Item::Scroll(b) = item {
                let out = b.repeat(ms, self.enhance);
                if out.handled {
                    self.finish(id, out, &mut r);
                    r.refresh = true;
                }
            }
        }
        r
    }

    /// `bildMouseUp`: the buttons, then the scroll bars, until one takes
    /// it; otherwise the form unrolls after a drag that wasn't a double
    /// click. // 1060:2b17
    pub fn mouse_up(&mut self, x: i32, y: i32) -> Response {
        let mut r = Response::default();
        for pass in [true, false] {
            for i in 0..self.widgets.len() {
                let (id, item) = &mut self.widgets[i];
                let id = *id;
                let out = match item {
                    Item::Button(b) if pass => b.mouse_up(x, y),
                    Item::Scroll(b) if !pass => b.mouse_up(x, y, self.enhance),
                    _ => continue,
                };
                if out.handled {
                    self.finish(id, out, &mut r);
                    r.refresh = true;
                    return r;
                }
            }
        }
        if self.dragging && !self.rolled_up {
            self.height = self.full_height;
            r.moved = true;
        }
        r
    }

    /// `FormKeyDown`. Without Ctrl and Alt: Escape and Enter close the
    /// form, Space is "INFO", `T` "TOP"; `M`, `1` to `7`, `G`, `S`, F1 to
    /// F4, F9 to F12 and `K` tick the check boxes, the numeric keypad's 1
    /// to 7 and F5 to F7 the radio buttons; the arrows move the music's
    /// volume, `-` and `+` the speed. With Ctrl the arrows move the
    /// sounds' volume, with Alt the speech's, and Alt+F4 closes the form.
    /// // 1060:2c72
    pub fn key_down(&mut self, key: u16, shift: ShiftState) -> Response {
        let mut r = Response {
            refresh: true,
            ..Response::default()
        };
        if shift.0 == 0 || shift == ShiftState::SHIFT {
            if let Some(&(_, id)) = KEYS.iter().find(|(k, _)| *k == key) {
                let out = match self.item_mut(id) {
                    // Buttons only `Click`.
                    Some(Item::Button(_)) => Outcome {
                        clicked: true,
                        ..Outcome::default()
                    },
                    Some(Item::Check(c)) => c.toggle(),
                    Some(Item::Radio(radio)) => radio.select(),
                    _ => Outcome::default(),
                };
                self.finish(id, out, &mut r);
            } else {
                match key {
                    VK_LEFT => self.step(Id::MusicVolume, -1, &mut r),
                    VK_RIGHT => self.step(Id::MusicVolume, 1, &mut r),
                    VK_MINUS | VK_SUBTRACT => self.step(Id::Speed, -1, &mut r),
                    VK_PLUS | VK_ADD => self.step(Id::Speed, 1, &mut r),
                    _ => {}
                }
            }
        } else if shift == ShiftState::CTRL {
            match key {
                VK_LEFT => self.step(Id::SoundVolume, -1, &mut r),
                VK_RIGHT => self.step(Id::SoundVolume, 1, &mut r),
                _ => {}
            }
        } else if shift == ShiftState::ALT {
            match key {
                VK_LEFT => self.step(Id::SpeechVolume, -1, &mut r),
                VK_RIGHT => self.step(Id::SpeechVolume, 1, &mut r),
                VK_F4 => self.click(Id::Close, &mut r),
                _ => {}
            }
        }
        r
    }

    /// A scroll bar a step towards its end, as far as it goes, with its
    /// `Click` and drawn; no sound.
    fn step(&mut self, id: Id, by: i32, r: &mut Response) {
        if let Some(Item::Scroll(b)) = self.item_mut(id) {
            b.position = if by < 0 {
                b.position.wrapping_add(by).max(b.min)
            } else {
                b.position.wrapping_add(by).min(b.max)
            };
            self.finish(
                id,
                Outcome {
                    handled: true,
                    repaint: true,
                    clicked: true,
                    sound: None,
                },
                r,
            );
        }
    }
}

fn set_range(b: &mut ScrollBar, min: i32, max: i32, position: i32) {
    b.min = min;
    b.max = max;
    b.position = position;
}

#[cfg(test)]
mod tests {
    use super::*;
    use formats::bmp::Bitmap;

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

    fn form(world: &World) -> Optionenform {
        let bitmaps = (0..BITMAP_FILES.len())
            .map(|i| Surface::new(400, 400, u8::try_from(i + 1).unwrap()))
            .collect();
        let texts = (0..26).map(|i| format!("t{i}").into_bytes()).collect();
        let mut f = Optionenform::new(font(), bitmaps, texts);
        f.show(&Settings::default(), world, true, (800, 600));
        f
    }

    fn click(f: &mut Optionenform, x: i32, y: i32) -> Response {
        let mut down = f.mouse_down(x, y, ShiftState::LEFT);
        let up = f.mouse_up(x, y);
        down.commands.extend(up.commands);
        down.changes.extend(up.changes);
        down.action = down.action.or(up.action);
        down
    }

    #[test]
    fn shows_the_world_and_the_settings() {
        let w = World::default();
        let f = form(&w);
        assert_eq!((f.left, f.top), ((800 - WIDTH) / 2, 0));
        assert_eq!(f.position(Id::Speed), 96);
        assert!(f.checked(Id::Animations) && f.checked(Id::AuctionWaits) && f.checked(Id::Disasters));
        assert!(f.checked(Id::Pointers(1)) && !f.checked(Id::Pointers(0)));
        assert!(f.checked(Id::Playing(0)) && !f.checked(Id::Playing(1)));
        // The title's label and a check box's text.
        assert_eq!(f.labels[0].text, b"t0");
        let Some((_, Item::Check(c))) = f.widgets.iter().find(|(i, _)| *i == Id::Disasters) else {
            panic!()
        };
        assert_eq!(c.radio.text, b"t22");
    }

    #[test]
    fn world_options_are_commands() {
        let w = World::default();
        let mut f = form(&w);
        // "Katastrophen" at (0xb3, 0x127).
        let r = click(&mut f, 0xb3 + 4, 0x127 + 4);
        assert_eq!(r.commands, [command::Action::SetOption(WorldOption::Disasters, false)]);
        assert_eq!(r.sounds, [Sound::Select]);
        // `+` speeds up: one more on the slider, one tick an hour fewer.
        let r = f.key_down(VK_PLUS, ShiftState::default());
        assert_eq!(r.commands, [command::Action::Speed(3)]);
        let r = f.key_down(VK_F1 + 11, ShiftState::default());
        assert_eq!(r.commands, [command::Action::SetOption(WorldOption::PeopleMove, false)]);
        // The slider stops at 99: speed 1.
        for _ in 0..10 {
            let _ = f.key_down(VK_ADD, ShiftState::default());
        }
        assert_eq!(f.position(Id::Speed), 99);
    }

    #[test]
    fn tracks_and_pointers() {
        let w = World::default();
        let mut f = form(&w);
        // The track playing can't be unticked.
        let r = f.key_down(0x31, ShiftState::default());
        assert!(f.checked(Id::Track(0)) && f.settings().playlist[0]);
        assert_eq!(r.changes, []);
        // Untick track 3, then it can't be picked.
        let _ = f.key_down(0x33, ShiftState::default());
        assert!(!f.settings().playlist[2]);
        let r = f.key_down(VK_NUMPAD1 + 2, ShiftState::default());
        assert!(r.changes.is_empty() && !f.checked(Id::Playing(2)) && f.checked(Id::Playing(0)));
        // Track 2 can.
        let r = f.key_down(VK_NUMPAD1 + 1, ShiftState::default());
        assert_eq!(r.changes, [Change::Track(1)]);
        assert!(f.checked(Id::Playing(1)) && !f.checked(Id::Playing(0)));
        assert_eq!(f.settings().track, 1);
        // "Besitzer".
        let r = f.key_down(VK_F1 + 4, ShiftState::default());
        assert_eq!(r.changes, [Change::Pointers]);
        assert!(f.settings().owner_pointers && !f.settings().occupancy_pointers);
        assert!(!f.checked(Id::Pointers(1)));
        // Music off, Ctrl and Alt for the other volumes.
        assert_eq!(f.key_down(0x4d, ShiftState::default()).changes, [Change::Music(false)]);
        assert_eq!(f.key_down(VK_LEFT, ShiftState::CTRL).changes, [Change::SoundVolume(99)]);
        let _ = f.key_down(VK_LEFT, ShiftState::ALT);
        assert_eq!(f.settings().speech_volume, 99);
    }

    #[test]
    fn closes_and_drags() {
        let w = World::default();
        let mut f = form(&w);
        assert_eq!(click(&mut f, 4, 4).action, Some(Action::Close));
        assert_eq!(click(&mut f, 0xb3 + 10, 0x158 + 5).action, Some(Action::Close));
        assert_eq!(click(&mut f, 0xb3 + 10, 0x13e + 5).action, Some(Action::Highscores));
        assert_eq!(click(&mut f, 0x103 + 10, 0x13e + 5).action, Some(Action::Credits));
        assert_eq!(f.key_down(VK_ESCAPE, ShiftState::SHIFT).action, Some(Action::Close));
        assert_eq!(f.key_down(VK_F4, ShiftState::ALT).action, Some(Action::Close));
        assert_eq!(
            f.key_down(VK_SPACE, ShiftState::default()).action,
            Some(Action::Credits)
        );
        assert_eq!(f.key_down(0x54, ShiftState::CTRL).action, None);
        // With `enhancements_screen`, "INFO" opens the list instead.
        f.enhance.set(Enhancement::EnhancementsScreen, true);
        assert_eq!(click(&mut f, 0x103 + 10, 0x13e + 5).action, Some(Action::Enhancements));
        f.enhance = Enhancements::ORIGINAL;
        // Dragged by the title bar: rolled up meanwhile.
        let left = f.left;
        let _ = f.mouse_down(100, 5, ShiftState::LEFT);
        assert_eq!(f.height, TITLE_HEIGHT);
        let r = f.mouse_move(90, 5, ShiftState::LEFT, (800, 600));
        assert!(r.moved);
        assert_eq!(f.left, left - 10);
        let _ = f.mouse_up(90, 5);
        assert_eq!(f.height, HEIGHT);
        // A double click leaves it rolled up.
        let _ = f.mouse_down(100, 5, ShiftState::LEFT | ShiftState::DOUBLE);
        let _ = f.mouse_up(100, 5);
        assert_eq!(f.height, TITLE_HEIGHT);
        let _ = f.mouse_down(100, 5, ShiftState::LEFT);
        let _ = f.mouse_up(100, 5);
        assert_eq!(f.height, HEIGHT);
    }

    #[test]
    fn people_flag_16() {
        let mut w = World::default();
        w.people.records = vec![crate::people::Person([0xff; crate::people::PERSON_SIZE]); 0x40];
        w.people.records[0x30].set_kind(6);
        w.people.records[0x31].set_kind(0x11);
        w.people.records[0x20].set_kind(0x11);
        w.people.last = 0x32;
        let mut tick = Tick::default();
        w.set_option(WorldOption::PeopleMove, true, &mut tick);
        assert!(tick.redraw_all);
        let flag = |w: &World, n: usize| w.people.records[n].flags()[2];
        assert_eq!(
            (flag(&w, 0x30), flag(&w, 0x31), flag(&w, 0x20), flag(&w, 0x33)),
            (0xff, 1, 0xff, 0xff)
        );
        w.set_option(WorldOption::PeopleMove, false, &mut tick);
        assert_eq!(flag(&w, 0x31), 0);
        assert!(!w.options.people_move);
    }

    #[test]
    fn right_button_with_the_fix() {
        let w = World::default();
        let mut f = form(&w);
        let animations = (0xb3 + 2, 0x10f + 2);
        let before = f.checked(Id::Animations);
        let _ = f.mouse_down(animations.0, animations.1, ShiftState::RIGHT);
        assert_ne!(f.checked(Id::Animations), before, "the original ticks it");
        let mut f = form(&w);
        f.enhance.set(Enhancement::MouseButtons, true);
        let _ = f.mouse_down(animations.0, animations.1, ShiftState::RIGHT);
        assert_eq!(f.checked(Id::Animations), before);
    }
}
