//! The sound: `game::sound`'s logic on the platform's mixer and
//! synthesizer. `play_sound`'s waves (10a0:2f5e opens them from `SOUND\`
//! or `SOUNDLO\`), the background music (`hintergrund.MusikMediaPlayer`),
//! the end's music (`TSchlussform.MusikPlayer`), the TV's speaker
//! (`TFernseher.MediaPlayer2`) and the films (`hintergrund.FilmMediaPlayer`,
//! `TFernseher.MediaPlayer1`, `TSchlussform.Filmplayer`: one at a time).
//! `docs/sound.md` and `docs/films.md` have what plays when.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use formats::setup;
use game::enhance::{Enhancement, Enhancements};
use game::fernseher::Media;
use game::optionen::Settings;
use game::sound::{Effects, Mci, MciEvent, Music, WAVE_FILES, device_level};
use platform::audio::wavemix::{WavFile, Wave};
use platform::audio::{self, Audio, MidiFile};
use vcl::Rect;

use crate::film::Film;

/// The players the engine's one MIDI output plays for.
const BACKGROUND: u8 = 1;
const END: u8 = 2;

/// The sound, shared by the start menu and the game.
pub type Shared = Rc<RefCell<Sound>>;

pub struct Sound {
    audio: Audio,
    dir: PathBuf,
    pub effects: Effects,
    pub music: Music,
    /// `TSchlussform.MusikPlayer` (+0x18c).
    pub end_music: Mci,
    /// `TFernseher.MediaPlayer2` (+0x18c).
    pub speaker: Mci,
    speaker_wav: Option<WavFile>,
    /// The speaker's last file didn't open.
    speaker_failed: bool,
    /// The MIDI files read so far, `None` where one isn't there.
    midis: BTreeMap<String, Option<Arc<MidiFile>>>,
    /// Which films: `SETUP.DAT`'s fourth line (`film_dir`, 1110:502a).
    pub film_quality: setup::Film,
    /// The film that's open.
    film: Option<Film>,
    /// The waves' directory (`SOUND` or `SOUNDLO`), none without sound.
    wave_dir: Option<&'static str>,
    /// The player's view enhancements (`auction_calls`, `sound_times`,
    /// `music`).
    pub enhance: Enhancements,
}

/// `play_sound`'s waves from `dir`'s `sub` (`init_sound`, 10a0:2f5e); with
/// `auction_calls` those not there from `SPRADEU1\`, where the
/// auctioneer's calls are.
fn load_waves(dir: &Path, sub: &str, auction_calls: bool) -> Vec<Option<Wave>> {
    WAVE_FILES
        .iter()
        .map(|name| {
            let mut path = find(dir, sub, name);
            // BUG(original): the auctioneer's calls are looked for in `SOUND\`, but they're in `SPRADEUn\`.
            if auction_calls && !path.is_file() {
                path = find(dir, &speech_dir(dir, 1), name);
            }
            // A missing file is a null wave, which `WaveMixPlay`
            // refuses (the auctioneer's `zum1.wav` to `zum3.wav`).
            let bytes = std::fs::read(&path).ok()?;
            Wave::open(&bytes)
                .inspect_err(|e| eprintln!("{}: {e}", path.display()))
                .ok()
        })
        .collect()
}

/// `name` in `dir`'s subdirectory `sub`, whatever the case of the
/// letters (the German CD's names are upper case, the English install's
/// mixed: `Sprache1\`, `Abgeld.avi`), else as given.
fn find(dir: &Path, sub: &str, name: &str) -> PathBuf {
    let upper = dir.join(sub).join(name.to_ascii_uppercase());
    if upper.is_file() {
        return upper;
    }
    crate::forms::find(dir, &format!("{sub}/{name}")).unwrap_or_else(|| dir.join(sub).join(name))
}

/// The speech folder `n` (1 to 3): `SPRADEUn\` as v1.0 names it, else
/// `Sprache<n>\` as the later build does (`init_paths`, en 1000:196d).
fn speech_dir(dir: &Path, n: u8) -> String {
    let german = format!("SPRADEU{n}");
    if crate::forms::find(dir, &german).is_some_and(|p| p.is_dir()) {
        german
    } else {
        format!("SPRACHE{n}")
    }
}

impl Sound {
    /// `init_sound` (10a0:2f5e) for the game in `dir` with the
    /// installation's sound `quality` (`SETUP.DAT`): the waves opened
    /// from `SOUND\` or `SOUNDLO\`, none without sound, and the session
    /// active. `audio` plays them.
    #[must_use]
    pub fn new(dir: &Path, quality: setup::Sound, audio: Audio) -> Sound {
        let enabled = quality != setup::Sound::Off;
        let wave_dir = enabled.then_some(if quality == setup::Sound::Lo {
            "SOUNDLO"
        } else {
            "SOUND"
        });
        if let Some(sub) = wave_dir {
            let waves = load_waves(dir, sub, false);
            audio.with(|e| {
                e.set_waves(waves);
                e.activate(true);
            });
        }
        Sound {
            audio,
            dir: dir.to_path_buf(),
            effects: Effects::new(enabled),
            music: Music::default(),
            end_music: Mci::default(),
            speaker: Mci::default(),
            speaker_wav: None,
            speaker_failed: false,
            midis: BTreeMap::new(),
            film_quality: setup::Film::Hi,
            film: None,
            wave_dir,
            enhance: Enhancements::ORIGINAL,
        }
    }

    /// The player's view enhancements: the music's, and with
    /// `auction_calls` the auctioneer's calls loaded.
    pub fn set_view_enhancements(&mut self, view: Enhancements) {
        let calls = Enhancement::AuctionCalls;
        if let Some(sub) = self.wave_dir
            && view.has(calls) != self.enhance.has(calls)
        {
            let waves = load_waves(&self.dir, sub, view.has(calls));
            self.audio.with(|e| e.set_waves(waves));
        }
        self.enhance = view;
        self.music.enhance = view;
    }

    /// Each frame: with `music` a track that ended is followed at once
    /// (`Music::frame`).
    pub fn music_frame(&mut self, settings: &mut Settings) {
        let player = &self.music.player;
        if !(player.is_open() && player.at_end()) {
            return;
        }
        let lengths = self.lengths();
        self.music.frame(settings, lengths);
        self.sync();
    }

    /// `init_sound` again (10a0:2f5e, from 1030:38a3 when a new game
    /// starts from the file menu): no wave blocks itself, the session is
    /// active and empty.
    pub fn reset_effects(&mut self) {
        self.effects = Effects::new(self.effects.enabled);
        if self.effects.enabled {
            self.audio.with(|e| {
                e.flush();
                e.activate(true);
            });
        }
    }

    /// A shared [`Sound`].
    #[must_use]
    pub fn shared(self) -> Shared {
        Rc::new(RefCell::new(self))
    }

    /// The MIDI file `name` in `MUSIK\`.
    fn midi(&mut self, name: &str) -> Option<Arc<MidiFile>> {
        let dir = &self.dir;
        self.midis
            .entry(name.to_owned())
            .or_insert_with(|| {
                let path = find(dir, "MUSIK", name);
                let bytes = std::fs::read(&path).ok()?;
                audio::load_midi(&bytes)
                    .inspect_err(|e| eprintln!("{}: {e}", path.display()))
                    .ok()
            })
            .clone()
    }

    /// The length MCI gives `MUSIK\name`, `None` if it doesn't open.
    fn midi_length(&mut self, name: &str) -> Option<u32> {
        self.midi(name).map(|m| audio::midi_length_ms(&m))
    }

    /// The lengths of the MIDI files, for `game::sound::Music`.
    fn lengths(&mut self) -> impl Fn(&str) -> Option<u32> + use<> {
        let names: Vec<String> = game::optionen::TRACK_FILES
            .iter()
            .chain([&game::sound::INTRO_MUSIC])
            .map(|s| (*s).to_owned())
            .collect();
        let lengths: BTreeMap<String, Option<u32>> = names
            .into_iter()
            .map(|n| {
                let l = self.midi_length(&n);
                (n, l)
            })
            .collect();
        move |name: &str| lengths.get(name).copied().flatten()
    }

    /// `play_sound(n)` (10a0:30b6) with the options' "Geräusche".
    pub fn play(&mut self, n: i16, settings: &Settings) {
        if let Some(i) = self.effects.play(n, settings.sounds) {
            self.audio.with(|e| {
                // A refusal is silent, as the original ignores it.
                let _ = e.play_wave(i);
            });
        }
    }

    /// Each of `sounds` in turn.
    pub fn play_all<N: Into<i16>>(&mut self, sounds: impl IntoIterator<Item = N>, settings: &Settings) {
        for n in sounds {
            self.play(n.into(), settings);
        }
    }

    /// 10a0:2518: the session gives up the wave device.
    pub fn effects_off(&mut self) {
        if self.effects.deactivate() {
            self.audio.with(|e| {
                e.flush();
                e.activate(false);
            });
        }
    }

    /// 10a0:254f: it takes it back.
    pub fn effects_on(&mut self) {
        if self.effects.activate() {
            self.audio.with(|e| e.activate(true));
        }
    }

    /// `waveOutSetVolume` at `volume` (0 to 100), with sound.
    /// // 10a0:2453, 10a0:24bc
    pub fn set_wave_volume(&mut self, volume: i32) {
        if self.effects.enabled {
            self.audio.with(|e| e.set_wave_volume(device_level(volume)));
        }
    }

    /// `midiOutSetVolume` at `volume` (0 to 100). // 1018:35f0
    pub fn set_music_volume(&mut self, volume: i32) {
        self.audio.with(|e| e.set_music_volume(device_level(volume)));
    }

    /// What the TV asks, in order.
    pub fn tv_media(&mut self, media: &[Media], settings: &Settings) {
        for m in media {
            match m {
                Media::EffectsOff => self.effects_off(),
                Media::EffectsOn => self.effects_on(),
                Media::EffectsOnWithSounds => {
                    if settings.sounds {
                        self.effects_on();
                        self.set_wave_volume(settings.sound_volume);
                    }
                }
                Media::SpeechVolume => self.set_wave_volume(settings.speech_volume),
                Media::SoundVolume => self.set_wave_volume(settings.sound_volume),
                Media::OpenSpeaker { dir, file } => {
                    let name = String::from_utf8_lossy(file).into_owned();
                    let path = find(&self.dir, &speech_dir(&self.dir, *dir), &name);
                    let wav = std::fs::read(&path).ok().and_then(|b| {
                        WavFile::parse(&b)
                            .inspect_err(|e| eprintln!("{}: {e}", path.display()))
                            .ok()
                    });
                    self.speaker.open(&name, wav.as_ref().map(WavFile::length_ms));
                    self.speaker_failed |= wav.is_none();
                    self.speaker_wav = wav;
                }
                Media::PlaySpeaker => self.speaker.play(),
                Media::CloseSpeaker => self.speaker.close(),
                Media::PlayFilm { file } => {
                    // A film that doesn't open is over at once.
                    let _ = self.play_film(&String::from_utf8_lossy(file), Rect::new(0, 0, 0, 0));
                }
                Media::StopFilm => self.stop_film(),
            }
        }
        self.sync();
    }

    /// Whether the speaker's file failed to open since the last call.
    pub fn take_speaker_failed(&mut self) -> bool {
        std::mem::take(&mut self.speaker_failed)
    }

    /// Whether the speaker has played to its end (`Position = Length`).
    #[must_use]
    pub fn speaker_done(&self) -> bool {
        self.speaker.at_end()
    }

    /// `music_close`: the music stops and closes, when the game is freed
    /// (en 1038:3657).
    pub fn close_music(&mut self) {
        self.music.player.close();
        self.sync();
    }

    /// The intro's music, and "Neues Spiel"'s: `holiday.mid` from its
    /// start. // 1000:2466, 1010:25dc, en 1010:3690
    pub fn intro_music(&mut self) {
        let lengths = self.lengths();
        self.music.intro(lengths);
        self.sync();
    }

    /// After each day (1018:367e).
    pub fn next_day(&mut self, settings: &mut Settings) {
        let lengths = self.lengths();
        self.music.next_day(settings, lengths);
        self.sync();
    }

    /// The options' "Musik" (1060:01dc): `music_start` (1018:3850) or
    /// `music_stop` (1018:38c5).
    pub fn music_on(&mut self, on: bool, settings: &Settings) {
        if on {
            let lengths = self.lengths();
            self.music.start(settings, lengths);
        } else {
            self.music.stop();
        }
        self.sync();
    }

    /// The options' track (1060:02b7).
    pub fn select_track(&mut self, track: u8, settings: &mut Settings) {
        let lengths = self.lengths();
        self.music.select_track(settings, track, lengths);
        self.sync();
    }

    /// `game_show_state` (1010:26fa).
    pub fn game_loaded(&mut self, settings: &Settings) {
        let lengths = self.lengths();
        self.music.game_loaded(settings, lengths);
        self.sync();
    }

    /// `game_start_setup` (1010:2820) for a game with `settings`: the
    /// volumes, and the music stops if it's off.
    pub fn game_start(&mut self, settings: &Settings) {
        self.set_music_volume(settings.music_volume);
        self.set_wave_volume(settings.sound_volume);
        self.music.game_start(settings);
        self.sync();
    }

    /// The end's music `file` opens and plays (1068:1058, 1068:1872).
    pub fn play_end_music(&mut self, file: &str) {
        let length = self.midi_length(file);
        self.end_music.open(file, length);
        self.end_music.play();
        self.sync();
    }

    /// The end's music stops and closes (1068:1593).
    pub fn stop_end_music(&mut self) {
        self.end_music.stop();
        self.end_music.close();
        self.sync();
    }

    /// `ms` pass for the players.
    pub fn advance(&mut self, ms: u32) {
        self.music.player.advance(ms);
        self.end_music.advance(ms);
        self.speaker.advance(ms);
        if let Some(f) = &mut self.film {
            f.advance(ms);
        }
        self.sync();
    }

    /// A film player's `FileName := film_dir + name; Open; Play`, shown in
    /// `panel` on the screen, its sound on the wave device. `false` if it
    /// doesn't open; the original raises an exception then
    /// (`docs/films.md`). Another film that's open closes first.
    pub fn play_film(&mut self, name: &str, panel: Rect) -> bool {
        self.stop_film();
        let sub = match self.film_quality {
            setup::Film::Lo => "FILMLO",
            setup::Film::Hi => "FILMHI",
        };
        let Some(path) = crate::forms::find(&self.dir, &format!("{sub}/{name}")) else {
            eprintln!("film {sub}/{name} not found");
            return false;
        };
        match Film::open(&path, panel) {
            Ok(mut film) => {
                film.play();
                if let Some((samples, rate)) = &film.sound {
                    self.audio.with(|e| e.play_film(Arc::clone(samples), *rate));
                }
                self.film = Some(film);
                true
            }
            Err(e) => {
                // BUG(original): `TMediaPlayer.Open` raises an exception for a film that doesn't open (the TV then never closes by itself); the callers end the film at once.
                eprintln!("{}: {e}", path.display());
                false
            }
        }
    }

    /// Whether the film directory has the film `name` (`FileExists`).
    pub fn film_exists(&self, name: &str) -> bool {
        let sub = match self.film_quality {
            setup::Film::Lo => "FILMLO",
            setup::Film::Hi => "FILMHI",
        };
        crate::forms::find(&self.dir, &format!("{sub}/{name}")).is_some()
    }

    /// The film player's `Stop`/`Eject`/`Close`.
    pub fn stop_film(&mut self) {
        if self.film.take().is_some() {
            self.audio.with(platform::audio::Engine::stop_film);
        }
    }

    /// `Position = Length` of the film, or no film open.
    #[must_use]
    pub fn film_done(&self) -> bool {
        self.film.as_ref().is_none_or(Film::at_end)
    }

    /// The film's panel moved.
    pub fn set_film_panel(&mut self, panel: Rect) {
        if let Some(f) = &mut self.film {
            f.panel = panel;
        }
    }

    /// What the film shows on a `screen`-sized screen.
    #[must_use]
    pub fn film_picture(&self, screen: (i32, i32)) -> Option<(Rect, Vec<u8>)> {
        self.film.as_ref()?.picture(screen)
    }

    /// The renderer follows the players.
    fn sync(&mut self) {
        let players = [
            (BACKGROUND, self.music.player.take_events()),
            (END, self.end_music.take_events()),
        ];
        for (owner, events) in players {
            for ev in events {
                match ev {
                    MciEvent::Play { file, .. } => {
                        if let Some(midi) = self.midi(&file) {
                            self.audio.with(|e| e.play_music(owner, &midi));
                        }
                    }
                    MciEvent::Stop => self.audio.with(|e| e.stop_music(owner)),
                }
            }
        }
        for ev in self.speaker.take_events() {
            match ev {
                MciEvent::Play { .. } => {
                    if let Some(wav) = &self.speaker_wav {
                        self.audio.with(|e| e.play_speaker(wav));
                    }
                }
                MciEvent::Stop => self.audio.with(platform::audio::Engine::stop_speaker),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// With the game's files: the waves open from `SOUND\` but the
    /// auctioneer's, the music has lengths, and a news speech plays to its
    /// end. Skipped without `orig/`.
    #[test]
    fn orig_sound_files() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../orig");
        if !dir.join("SOUND").is_dir() {
            eprintln!("skipped: no {}", dir.join("SOUND").display());
            return;
        }
        for quality in [setup::Sound::Hi, setup::Sound::Lo] {
            let sub = if quality == setup::Sound::Lo {
                "SOUNDLO"
            } else {
                "SOUND"
            };
            for (n, name) in WAVE_FILES.iter().enumerate() {
                let bytes = std::fs::read(find(&dir, sub, name)).ok();
                let wave = bytes.as_deref().map(Wave::open);
                if (25..=27).contains(&n) {
                    assert!(wave.is_none(), "{name} isn't in {sub}");
                } else {
                    // Both are 11 kHz in the mixer.
                    let wave = wave.unwrap().unwrap();
                    assert!(!wave.samples().is_empty(), "{name}");
                }
            }
            // `auction_calls`: the calls from `SPRADEU1\`.
            let original = load_waves(&dir, sub, false);
            let fixed = load_waves(&dir, sub, true);
            for n in 25..=27 {
                assert!(original[n].is_none());
                assert!(
                    fixed[n].as_ref().is_some_and(|w| !w.samples().is_empty()),
                    "{}",
                    WAVE_FILES[n]
                );
            }
            assert_eq!(original.iter().flatten().count(), 39);
            assert_eq!(fixed.iter().flatten().count(), 42);
        }
        let mut s = Sound::new(&dir, setup::Sound::Hi, Audio::silent());
        let mut settings = Settings::default();
        assert!(s.midi_length("floatin'.mid").is_some());
        s.intro_music();
        assert!(s.music.player.playing());
        s.music.init_playlist(&mut settings);
        s.tv_media(
            &[Media::OpenSpeaker {
                dir: 1,
                file: b"oster.wav".to_vec(),
            }],
            &settings,
        );
        assert!(s.speaker.is_open() && !s.speaker_done());
        s.tv_media(&[Media::PlaySpeaker], &settings);
        s.advance(60_000);
        assert!(s.speaker_done());
        // The sound plays (and blocks itself) though nothing is heard.
        s.play(1, &settings);
        assert_eq!(s.effects.play(1, true), None);
    }
}
