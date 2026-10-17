//! The sounds and the music, the view's part of them: which wave
//! `play_sound` plays and when it won't (10a0:30b6), the `WaveMix` session's
//! state the game keeps (`1110:733c`), and the MIDI music's player and
//! playlist (`MusikMediaPlayer`, 1018:35f0..1018:38f5). Nothing here makes
//! a sound: the caller hands what it asks for to the platform's mixer and
//! synthesizer. `docs/sound.md` has the whole picture.
//!
//! The players (`TMediaPlayer`, MCI) are modelled by [`Mci`]: a position
//! that runs with the caller's clock up to the file's length, which is all
//! the game reads of them (`Position = Length` means it has ended).

use crate::optionen::{Settings, TRACK_FILES};

/// `play_sound`'s waves, 1 to 42, in `SOUND\` or `SOUNDLO\` (`1110:51e0`).
/// `zum1.wav` to `zum3.wav` aren't there: they're in `SPRADEUn\`, so the
/// auctioneer's calls are silent. // 10a0:2824, names from 10a0:25e9
pub const WAVE_FILES: [&str; 42] = [
    "waal.wav",     // 1: a whale
    "chikli8b.wav", // 2: chickens (and 0x2c)
    "hupe.wav",     // 3: a horn (and 0x2b)
    "hupe3.wav",    // 4: a ship's horn
    "abris18b.wav", // 5: demolition
    "elektriz.wav", // 6: electricity
    "loewe8b.wav",  // 7: a lion
    "butthi8b.wav", // 8: a button goes down
    "buttmi8b.wav", // 9: a box ticked, a row picked
    "buttlo8b.wav", // 10: a button comes up
    "clicla8b.wav", // 11
    "duck8b.wav",   // 12: an error message
    "auf8b.wav",    // 13: a form opens
    "zu8b.wav",     // 14: a form closes
    "bike38b.wav",  // 15: a motorbike
    "brandung.wav", // 16: the surf
    "strasb8b.wav", // 17: a road built
    "brems8b.wav",  // 18: brakes
    "hammer.wav",   // 19: the builders' hammer
    "aukha8b.wav",  // 20: the auctioneer's hammer
    "crashku1.wav", // 21: a crash
    "exploku1.wav", // 22: an explosion
    "exploku2.wav", // 23: an explosion
    "fliegst2.wav", // 24: a plane
    "elefant.wav",  // 25: an elephant
    // BUG(original): the auctioneer's calls are looked for in `SOUND\`, but they're in `SPRADEUn\`: never heard.
    "zum1.wav",     // 26: "zum ersten"
    "zum2.wav",     // 27: "zum zweiten"
    "zum3.wav",     // 28: "zum dritten"
    "hublop8b.wav", // 29: a helicopter
    "klaerku.wav",  // 30: the sewage plant
    "klonga8b.wav", // 31: a tool did its work
    "kranku.wav",   // 32: a crane
    "kreisse.wav",  // 33: a circular saw
    "moew2.wav",    // 34: gulls
    "nebel48b.wav", // 35: a foghorn
    "newskurz.wav", // 36: news
    "payedit2.wav", // 37: the balance sheet
    "pressl.wav",   // 38: a pneumatic drill
    "saege.wav",    // 39: a saw
    "putt.wav",     // 40: minigolf
    "hole-in.wav",  // 41: a hole in one
    "disco.wav",    // 42: the disco
];

/// How many of the people's ticks (`people_tick`, 1090:5293) each wave
/// blocks itself after it started: a sound with a time doesn't start again
/// until it has passed. // 1110:3dd0 (`1110:3dce + 2n`)
pub const PLAY_TICKS: [i16; 42] = [
    50, 50, 50, 50, 0, 50, 50, 0, 0, 0, 0, 0, 0, 0, 50, 50, 0, 50, 25, 0, 0, 0, 0, 50, 50, 0, 0, 0, 50, 50, 0, 25, 50,
    50, 0, 0, 50, 25, 25, 25, 25, 25,
];

/// `waveOutSetVolume`'s and `midiOutSetVolume`'s level for a volume of 0
/// to 100: `volume * 655` for both channels (the low word).
/// // 10a0:2453, 1018:35f0
#[must_use]
pub fn device_level(volume: i32) -> u16 {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // The low word, as the original.
    let level = volume.wrapping_mul(655) as u16;
    level
}

/// `play_sound`'s state: the session and how long each wave still blocks
/// itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Effects {
    /// The installation has sound (`SETUP.DAT`'s fifth line isn't `N`):
    /// without it nothing here does anything. // 1110:0062
    pub enabled: bool,
    /// The `WaveMix` session is active. // 1110:733c
    active: bool,
    /// The people's ticks each wave still blocks itself. Not saved.
    /// // 1110:723e (`1110:723c + 2n`)
    busy: [i16; 42],
}

impl Effects {
    /// As `init_sound` (10a0:2f5e) leaves it, which also activates the
    /// session (10a0:254f): all waves free.
    #[must_use]
    pub fn new(enabled: bool) -> Effects {
        Effects {
            enabled,
            active: enabled,
            busy: [0; 42],
        }
    }

    /// `play_sound(n)` with the options' "Geräusche" `option`
    /// (`1110:0061`): unless sound is off or the wave still blocks itself,
    /// the index (0-based) of the wave to `WaveMixPlay`, which then blocks
    /// itself for its [`PLAY_TICKS`]. `WaveMixPlay` may still fail (the
    /// file is missing, the session is inactive); the time runs anyway.
    /// 0x2b plays wave 3 and 0x2c wave 2. // 10a0:30b6
    pub fn play(&mut self, n: i16, option: bool) -> Option<usize> {
        if !self.enabled || !option {
            return None;
        }
        let n = match n {
            0x2b => 3,
            0x2c => 2,
            n => n,
        };
        // BUG(original): `play_sound` doesn't check its number; the port ignores one outside 1..=42 rather than reading past the tables.
        let i = usize::try_from(n)
            .ok()?
            .checked_sub(1)
            .filter(|&i| i < WAVE_FILES.len())?;
        if self.busy[i] != 0 {
            return None;
        }
        self.busy[i] = PLAY_TICKS[i];
        Some(i)
    }

    /// The start of `people_tick` (1090:5293): each wave's time counts
    /// down by one tick.
    pub fn count_down(&mut self) {
        // BUG(original): only `people_tick` counts down, which runs with the animations on: with them off a sound with a time never plays again.
        for b in &mut self.busy {
            if *b > 0 {
                *b -= 1;
            }
        }
    }

    /// 10a0:2518: with sound and the session active, every channel is
    /// flushed and the session gives up the wave device (for a film or the
    /// TV's speaker). Whether the caller flushes and deactivates the mixer.
    pub fn deactivate(&mut self) -> bool {
        if self.enabled && self.active {
            self.active = false;
            return true;
        }
        false
    }

    /// 10a0:254f: with sound and the session inactive, it takes the wave
    /// device back. Whether the caller activates the mixer.
    pub fn activate(&mut self) -> bool {
        if self.enabled && !self.active {
            self.active = true;
            return true;
        }
        false
    }

    #[must_use]
    pub fn active(&self) -> bool {
        self.active
    }
}

/// What a player did that the caller's renderer follows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MciEvent {
    /// The open file plays from `from` ms on.
    Play { file: String, from: u32 },
    /// It stopped (`Stop`, `Close`, or another file opened).
    Stop,
}

/// A `TMediaPlayer`'s device as the game sees it: a file, a length and a
/// position in ms (the default time format of MCI's `sequencer` and
/// `waveaudio`), which runs with [`Mci::advance`] while it plays.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Mci {
    /// The file (`FileName`) of the open device.
    file: Option<String>,
    /// `Length`; `None` while closed.
    length: Option<u32>,
    position: u32,
    playing: bool,
    /// `Error` (+0x1be): the last command failed.
    error: bool,
    events: Vec<MciEvent>,
}

impl Mci {
    /// `Open` (10c8:1810): closes the device if it's open, then opens
    /// `file`, whose length `length` is, at position 0; `None` (the file
    /// isn't there or isn't one the device plays) sets the error and leaves
    /// it closed. The original raises an exception then.
    pub fn open(&mut self, file: &str, length: Option<u32>) {
        self.close();
        self.error = length.is_none();
        if let Some(l) = length {
            self.file = Some(file.to_owned());
            self.length = Some(l);
            self.position = 0;
        }
    }

    /// `Close` (10c8:1b18).
    pub fn close(&mut self) {
        if self.playing {
            self.events.push(MciEvent::Stop);
        }
        self.file = None;
        self.length = None;
        self.playing = false;
    }

    /// `Play` (10c8:1c10): from the position on, if open.
    pub fn play(&mut self) {
        let (Some(file), Some(length)) = (&self.file, self.length) else {
            self.error = true;
            return;
        };
        self.error = false;
        if self.position < length {
            self.playing = true;
            self.events.push(MciEvent::Play {
                file: file.clone(),
                from: self.position,
            });
        }
    }

    /// `Stop` (10c8:1ee2): the position stays.
    pub fn stop(&mut self) {
        if self.playing {
            self.events.push(MciEvent::Stop);
        }
        self.playing = false;
        self.error = self.length.is_none();
    }

    /// `Position` (10c8:2fd7); 0 while closed.
    #[must_use]
    pub fn position(&self) -> u32 {
        self.position
    }

    /// `Length` (10c8:2ea9); 0 while closed.
    #[must_use]
    pub fn length(&self) -> u32 {
        self.length.unwrap_or(0)
    }

    /// `Position = Length`: what the game takes for "it has ended".
    #[must_use]
    pub fn at_end(&self) -> bool {
        self.position() == self.length()
    }

    #[must_use]
    pub fn is_open(&self) -> bool {
        self.length.is_some()
    }

    #[must_use]
    pub fn playing(&self) -> bool {
        self.playing
    }

    /// `Error <> 0`.
    #[must_use]
    pub fn error(&self) -> bool {
        self.error
    }

    /// `ms` pass: a playing device moves on, and stops at its end.
    pub fn advance(&mut self, ms: u32) {
        if let (true, Some(length)) = (self.playing, self.length) {
            self.position = self.position.saturating_add(ms).min(length);
            if self.position == length {
                self.playing = false;
            }
        }
    }

    /// What the renderer should follow since the last call.
    pub fn take_events(&mut self) -> Vec<MciEvent> {
        std::mem::take(&mut self.events)
    }
}

/// The music in the intro, on the start menu and at first in a game.
/// // 1000:232f (state 422), 1010:25cd
pub const INTRO_MUSIC: &str = "holiday.mid";
/// `1110:6efc` before a track has played: no position equals it.
/// // 1018:37de, 1018:3850
const NO_POSITION: u32 = 0x3039;

/// The background music (`hintergrund.MusikMediaPlayer`, +0x188) and what
/// `music_next_track` remembers. The playlist, the track and the option
/// are the player's [`Settings`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Music {
    pub player: Mci,
    /// The player's position at the end of the last day. // 1110:6efc
    last_position: u32,
}

impl Default for Music {
    fn default() -> Self {
        Music {
            player: Mci::default(),
            last_position: NO_POSITION,
        }
    }
}

impl Music {
    /// States 422 and 424 of the intro (1000:2466), and "Neues Spiel"
    /// (1010:25dc): `holiday.mid` opens and plays, whatever the options
    /// say. `length` gives a file's length in ms ([`Mci::open`]).
    pub fn intro(&mut self, length: impl Fn(&str) -> Option<u32>) {
        self.player.open(INTRO_MUSIC, length(INTRO_MUSIC));
        if !self.player.error() {
            self.player.play();
        }
    }

    /// Each game's set-up (`music_init_playlist`, 1018:37de, from
    /// 1030:36e5): every track on the playlist, the first track, no
    /// position. The music that plays (the intro's) goes on.
    pub fn init_playlist(&mut self, s: &mut Settings) {
        // BUG(original): track 0 while the intro's `holiday.mid` plays: the first track after it is `dub.mid`.
        s.playlist = [true; 7];
        s.track = 0;
        self.last_position = NO_POSITION;
    }

    /// The file of track `track`.
    fn track_file(track: u8) -> &'static str {
        TRACK_FILES.get(usize::from(track)).copied().unwrap_or(TRACK_FILES[0])
    }

    /// `music_start`'s work (1018:3850), which only runs with the music off
    /// (the caller checks and sets [`Settings::music`]): track `s.track`
    /// opens and plays from its start.
    pub fn start(&mut self, s: &Settings, length: impl Fn(&str) -> Option<u32>) {
        self.last_position = NO_POSITION;
        let file = Music::track_file(s.track);
        self.player.open(file, length(file));
        self.player.play();
    }

    /// `music_stop`'s work (1018:38c5), which only runs with the music on:
    /// it stops and closes.
    pub fn stop(&mut self) {
        self.player.stop();
        self.player.close();
    }

    /// The options' track (1060:02b7), once it's ticked: with the music on
    /// the player stops and closes (`Eject` does nothing to a sequencer),
    /// the track changes, and with the music on its file opens and plays.
    pub fn select_track(&mut self, s: &mut Settings, track: u8, length: impl Fn(&str) -> Option<u32>) {
        if s.music {
            self.player.stop();
            self.player.close();
        }
        s.track = track;
        if s.music {
            let file = Music::track_file(track);
            self.player.open(file, length(file));
            self.player.play();
        }
    }

    /// After each day (`music_next_track`, 1018:367e): with the music on,
    /// if the player hasn't moved since the last day (its file ended, or
    /// it stopped), the next track on the playlist after `s.track`
    /// (wrapping, the same one if none is ticked) opens and plays; then the
    /// position is kept for the next day.
    pub fn next_day(&mut self, s: &mut Settings, length: impl Fn(&str) -> Option<u32>) {
        if !s.music {
            return;
        }
        // BUG(original): a track's end shows only after a whole day without the position moving.
        if self.player.position() == self.last_position {
            let first = s.track;
            let mut track = first;
            loop {
                track = if track >= 6 { 0 } else { track + 1 };
                if s.playlist.get(usize::from(track)).copied().unwrap_or(false) || track == first {
                    break;
                }
            }
            s.track = track;
            // `Stop` and `Eject` only without an error.
            if !self.player.error() {
                self.player.stop();
            }
            self.player.close();
            let file = Music::track_file(track);
            self.player.open(file, length(file));
            if !self.player.error() {
                self.player.play();
            }
        }
        self.last_position = self.player.position();
    }

    /// `game_start_setup` (1010:2820): with the music off it stops and
    /// closes.
    pub fn game_start(&mut self, s: &Settings) {
        if !s.music {
            self.player.stop();
            self.player.close();
        }
    }

    /// `game_show_state` (1010:26fa), after the file menu loaded a game:
    /// with the music off the player closes, else its file opens again,
    /// which stops it at its start.
    // BUG(original): loading a game with the music on stops it, until a day or two later the next track starts.
    pub fn game_loaded(&mut self, s: &Settings, length: impl Fn(&str) -> Option<u32>) {
        if s.music {
            if let Some(file) = self.player.file.clone() {
                self.player.open(&file, length(&file));
            }
        } else {
            self.player.close();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::unnecessary_wraps)] // As `Music` takes it.
    fn len(_: &str) -> Option<u32> {
        Some(1000)
    }

    #[test]
    fn a_wave_blocks_itself_for_its_ticks() {
        let mut e = Effects::new(true);
        assert_eq!(e.play(1, true), Some(0));
        assert_eq!(e.play(1, true), None);
        for _ in 0..49 {
            e.count_down();
        }
        assert_eq!(e.play(1, true), None);
        e.count_down();
        assert_eq!(e.play(1, true), Some(0));
        // No time: again at once. 0x2b and 0x2c are 3 and 2.
        assert_eq!((e.play(8, true), e.play(8, true)), (Some(7), Some(7)));
        assert_eq!(e.play(0x2b, true), Some(2));
        assert_eq!(e.play(3, true), None);
        assert_eq!(e.play(0x2c, true), Some(1));
        // The option, the installation and bad numbers.
        assert_eq!(e.play(9, false), None);
        assert_eq!(Effects::new(false).play(9, true), None);
        assert_eq!((e.play(0, true), e.play(43, true)), (None, None));
    }

    #[test]
    fn the_session_flag() {
        let mut e = Effects::new(true);
        assert!(!e.activate());
        assert!(e.deactivate());
        assert!(!e.deactivate());
        assert!(e.activate());
        let mut off = Effects::new(false);
        assert!(!off.deactivate() && !off.activate());
    }

    #[test]
    fn mci_runs_to_its_end() {
        let mut m = Mci::default();
        m.play();
        assert!(m.error() && !m.playing());
        m.open("a.mid", Some(250));
        m.play();
        assert_eq!(
            m.take_events(),
            [MciEvent::Play {
                file: "a.mid".into(),
                from: 0
            }]
        );
        m.advance(100);
        assert!(!m.at_end());
        m.advance(200);
        assert!(m.at_end() && !m.playing());
        m.open("b.mid", None);
        assert!(m.error() && !m.is_open());
    }

    #[test]
    fn the_next_track_once_a_day_has_passed_without_music() {
        let mut s = Settings::default();
        let mut music = Music::default();
        music.intro(len);
        music.init_playlist(&mut s);
        // Day 1: the intro's music has moved from the start value.
        music.player.advance(400);
        music.next_day(&mut s, len);
        assert_eq!(s.track, 0);
        // Day 2: still playing; day 3: it ended at 1000 on day 2.
        music.player.advance(600);
        music.next_day(&mut s, len);
        assert_eq!(s.track, 0);
        music.next_day(&mut s, len);
        // BUG(original) kept: the first track after the intro's is the
        // second, `dub.mid`.
        assert_eq!(s.track, 1);
        assert!(music.player.playing());
        // Only the fifth track on the list: it follows itself.
        s.playlist = [false, false, false, false, true, false, false];
        music.player.advance(1000);
        music.next_day(&mut s, len);
        music.next_day(&mut s, len);
        assert_eq!(s.track, 4);
        music.player.advance(1000);
        music.next_day(&mut s, len);
        music.next_day(&mut s, len);
        assert_eq!(s.track, 4);
        // None on the list: the same track again.
        s.playlist = [false; 7];
        music.player.advance(1000);
        music.next_day(&mut s, len);
        music.next_day(&mut s, len);
        assert_eq!(s.track, 4);
        assert!(music.player.playing());
        // The music off: nothing.
        s.music = false;
        music.stop();
        music.next_day(&mut s, len);
        assert!(!music.player.is_open());
    }

    #[test]
    fn a_loaded_game_stops_the_music() {
        let s = Settings::default();
        let mut music = Music::default();
        music.intro(len);
        music.player.advance(300);
        music.game_loaded(&s, len);
        assert!(music.player.is_open() && !music.player.playing());
        assert_eq!(music.player.position(), 0);
    }
}
