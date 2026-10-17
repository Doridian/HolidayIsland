//! Sound output: the [`Engine`] renders what the game plays, and a `cpal`
//! stream (the `audio-device` feature) pulls it to the sound card. With no
//! device (or without the feature) [`Audio`] is silent and every call does
//! nothing, so the game runs the same headless.
//!
//! The game ships no `SoundFont` for its MIDI music: [`find_soundfont`]
//! looks for one (`tools/get-soundfont.sh` fetches `GeneralUser GS`), and
//! without it the music is silent.
//!
//! The music was written on a Roland GS Sound Canvas (`dub.mid` picks drum
//! kit 25, the TR-808): `rustysynth` takes a program change on channel 10
//! as a kit (bank 128, falling back to 128:0), as GS does, so a GS
//! `SoundFont` plays the right kits.

pub mod engine;
pub mod wavemix;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

pub use engine::Engine;
pub use rustysynth::MidiFile;
use rustysynth::SoundFont;

/// The environment variable that names a `SoundFont` (`.sf2`) for the music.
pub const SOUNDFONT_ENV: &str = "HOLIDAY_SOUNDFONT";

/// Where `SoundFont`s usually are, the GM ones first.
const SOUNDFONT_FILES: [&str; 6] = [
    "/usr/share/soundfonts/default.sf2",
    "/usr/share/soundfonts/FluidR3_GM.sf2",
    "/usr/share/sounds/sf2/default-GM.sf2",
    "/usr/share/sounds/sf2/FluidR3_GM.sf2",
    "/usr/share/sounds/sf2/TimGM6mb.sf2",
    "/usr/share/soundfonts/freepats-general-midi.sf2",
];
/// Directories searched for any `*.sf2` after those (sorted by name).
const SOUNDFONT_DIRS: [&str; 4] = [
    "/usr/share/soundfonts",
    "/usr/share/sounds/sf2",
    "/usr/local/share/soundfonts",
    "/usr/share/sf2",
];

/// Where `tools/get-soundfont.sh` installs a `SoundFont`:
/// `soundfont` in [`crate::storage::data_dir`].
#[must_use]
pub fn soundfont_dir() -> Option<PathBuf> {
    crate::storage::data_dir().map(|d| d.join("soundfont"))
}

/// The first `*.sf2` in `dir` by name.
fn first_sf2(dir: &Path) -> Option<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("sf2")) && p.is_file())
        .collect();
    found.sort();
    found.into_iter().next()
}

/// The `SoundFont` for the music, in this order: `explicit` (the
/// command-line option); [`SOUNDFONT_ENV`]; any `.sf2` in
/// [`soundfont_dir`] (where `tools/get-soundfont.sh` puts `GeneralUser GS`);
/// the common system ones ([`SOUNDFONT_FILES`], then any `.sf2` in
/// [`SOUNDFONT_DIRS`], `$XDG_DATA_HOME/soundfonts` and macOS's
/// `~/Library/Audio/Sounds/Banks`). `None` if there's none.
#[must_use]
pub fn find_soundfont(explicit: Option<&Path>) -> Option<PathBuf> {
    if let Some(p) = explicit {
        return Some(p.to_path_buf());
    }
    if let Some(p) = std::env::var_os(SOUNDFONT_ENV).filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(p));
    }
    if let Some(p) = soundfont_dir().as_deref().and_then(first_sf2) {
        return Some(p);
    }
    if let Some(p) = SOUNDFONT_FILES.iter().map(PathBuf::from).find(|p| p.is_file()) {
        return Some(p);
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| home.as_ref().map(|h| h.join(".local/share")));
    let mut dirs: Vec<PathBuf> = SOUNDFONT_DIRS.iter().map(PathBuf::from).collect();
    dirs.extend(data.map(|d| d.join("soundfonts")));
    dirs.extend(home.map(|h| h.join("Library/Audio/Sounds/Banks")));
    dirs.iter().find_map(|d| first_sf2(d))
}

/// Reads a `SoundFont`.
///
/// # Errors
/// If the file can't be read or isn't a `SoundFont` 2.
pub fn load_soundfont(path: &Path) -> Result<Arc<SoundFont>, String> {
    let mut f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    SoundFont::new(&mut f).map(Arc::new).map_err(|e| format!("{e:?}"))
}

/// Parses a Standard MIDI File.
///
/// # Errors
/// If it isn't one `rustysynth` reads.
pub fn load_midi(bytes: &[u8]) -> Result<Arc<MidiFile>, String> {
    let mut r = bytes;
    MidiFile::new(&mut r).map(Arc::new).map_err(|e| format!("{e:?}"))
}

/// A MIDI file's length in ms, as MCI's sequencer reports it in its
/// default time format.
#[must_use]
pub fn midi_length_ms(midi: &MidiFile) -> u32 {
    let ms = (midi.get_length() * 1000.0).round();
    if ms <= 0.0 {
        0
    } else if ms >= f64::from(u32::MAX) {
        u32::MAX
    } else {
        // In range, checked above.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let ms = ms as u32;
        ms
    }
}

/// The game's sound output.
pub struct Audio {
    engine: Option<Arc<Mutex<Engine>>>,
    #[cfg(feature = "audio-device")]
    _stream: Option<cpal::Stream>,
}

impl Audio {
    /// No output: every call does nothing.
    #[must_use]
    pub fn silent() -> Audio {
        Audio {
            engine: None,
            #[cfg(feature = "audio-device")]
            _stream: None,
        }
    }

    /// The default output device, with MIDI from the `SoundFont` at
    /// `soundfont` ([`find_soundfont`]'s). Without a device it's silent;
    /// without a `SoundFont` the music is. Each logs one line on stderr.
    #[must_use]
    pub fn open(soundfont: Option<&Path>) -> Audio {
        let font = if let Some(p) = soundfont {
            load_soundfont(p)
                .inspect_err(|e| eprintln!("music off: can't load the SoundFont {}: {e}", p.display()))
                .ok()
        } else {
            eprintln!(
                "music off: no SoundFont found (run tools/get-soundfont.sh, pass --soundfont=FILE.sf2 or set {SOUNDFONT_ENV})"
            );
            None
        };
        Audio::with_device(font.as_ref()).unwrap_or_else(|e| {
            eprintln!("sound off: {e}");
            Audio::silent()
        })
    }

    #[cfg(feature = "audio-device")]
    fn with_device(font: Option<&Arc<SoundFont>>) -> Result<Audio, String> {
        use cpal::traits::{DeviceTrait, HostTrait};

        let host = cpal::default_host();
        let device = host.default_output_device().ok_or("no audio output device")?;
        let supported = device.default_output_config().map_err(|e| e.to_string())?;
        let engine = Arc::new(Mutex::new(Engine::new(supported.config().sample_rate, font)?));
        // The device's default buffer can be smaller than the sound
        // server's period (PipeWire's ALSA plugin under a quantum of 1024),
        // and then every poll fails. Ask for a buffer that's more than
        // enough, and a bigger one while the stream reports errors.
        let mut last = String::new();
        for frames in BUFFER_FRAMES {
            let frames = match supported.buffer_size() {
                cpal::SupportedBufferSize::Range { min, max } => frames.clamp(*min, *max),
                cpal::SupportedBufferSize::Unknown => frames,
            };
            match open_stream(&device, &supported, frames, &engine) {
                Ok(stream) => {
                    return Ok(Audio {
                        engine: Some(engine),
                        _stream: Some(stream),
                    });
                }
                Err(e) => last = e,
            }
        }
        Err(last)
    }

    #[cfg(not(feature = "audio-device"))]
    #[allow(clippy::unnecessary_wraps)]
    fn with_device(_font: Option<&Arc<SoundFont>>) -> Result<Audio, String> {
        Err("built without the audio-device feature".into())
    }

    /// Whether anything is heard.
    #[must_use]
    pub fn has_output(&self) -> bool {
        self.engine.is_some()
    }

    /// Calls `f` on the engine, if there's output.
    pub fn with(&self, f: impl FnOnce(&mut Engine)) {
        if let Some(e) = &self.engine {
            f(&mut e.lock().unwrap_or_else(PoisonError::into_inner));
        }
    }
}

/// The buffer sizes to try, in frames, smallest first: 2048 is about
/// 46 ms at 44.1 kHz.
#[cfg(feature = "audio-device")]
const BUFFER_FRAMES: [u32; 3] = [2048, 4096, 8192];

/// Opens a stream on `device` with a buffer of `frames` that pulls from
/// `engine`, plays it, and gives it back if it runs without errors for a
/// moment. Later errors are logged, each kind once.
#[cfg(feature = "audio-device")]
fn open_stream(
    device: &cpal::Device,
    supported: &cpal::SupportedStreamConfig,
    frames: u32,
    engine: &Arc<Mutex<Engine>>,
) -> Result<cpal::Stream, String> {
    use cpal::traits::{DeviceTrait, StreamTrait};
    use std::sync::atomic::{AtomicUsize, Ordering};

    let mut config = supported.config();
    config.buffer_size = cpal::BufferSize::Fixed(frames);
    let channels = usize::from(config.channels);
    let errors = Arc::new(AtomicUsize::new(0));
    let err = {
        let errors = Arc::clone(&errors);
        let mut logged: Vec<String> = Vec::new();
        move |e: cpal::StreamError| {
            errors.fetch_add(1, Ordering::Relaxed);
            let text = e.to_string();
            if !logged.contains(&text) {
                eprintln!("audio: {text}");
                logged.push(text);
            }
        }
    };
    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => {
            let engine = Arc::clone(engine);
            device.build_output_stream(
                &config,
                move |out: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    engine
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .render(out, channels);
                },
                err,
                None,
            )
        }
        cpal::SampleFormat::I16 => {
            let engine = Arc::clone(engine);
            let mut buf = Vec::new();
            device.build_output_stream(
                &config,
                move |out: &mut [i16], _: &cpal::OutputCallbackInfo| {
                    buf.resize(out.len(), 0.0);
                    engine
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .render(&mut buf, channels);
                    for (o, s) in out.iter_mut().zip(&buf) {
                        // -1.0..1.0, clamped by the engine.
                        #[allow(clippy::cast_possible_truncation)]
                        let v = (s * f32::from(i16::MAX)) as i16;
                        *o = v;
                    }
                },
                err,
                None,
            )
        }
        f => return Err(format!("unsupported sample format {f}")),
    }
    .map_err(|e| e.to_string())?;
    stream.play().map_err(|e| e.to_string())?;
    // A broken buffer size fails at once and on every poll.
    std::thread::sleep(std::time::Duration::from_millis(150));
    match errors.load(Ordering::Relaxed) {
        0 => Ok(stream),
        n => Err(format!("{n} stream errors with a buffer of {frames} frames")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The original's MIDI files parse and have a length. Skipped without
    /// the game's files in `orig/`.
    #[test]
    fn orig_midi_lengths() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../orig/MUSIK");
        let Ok(entries) = std::fs::read_dir(&dir) else {
            eprintln!("skipped: no {}", dir.display());
            return;
        };
        let mut n = 0;
        for e in entries.flatten() {
            let bytes = std::fs::read(e.path()).unwrap();
            let midi = load_midi(&bytes).unwrap_or_else(|err| panic!("{}: {err}", e.path().display()));
            assert!(midi_length_ms(&midi) > 10_000, "{}", e.path().display());
            n += 1;
        }
        assert_eq!(n, 11);
    }

    /// With a `SoundFont` (`HOLIDAY_SOUNDFONT` or a system one) and the
    /// game's files, `holiday.mid` renders the same twice and isn't silent.
    #[test]
    fn renders_midi_deterministically() {
        let midi = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../orig/MUSIK/HOLIDAY.MID");
        let (Some(sf), Ok(bytes)) = (find_soundfont(None), std::fs::read(&midi)) else {
            eprintln!("skipped: no SoundFont or no {}", midi.display());
            return;
        };
        let font = load_soundfont(&sf).unwrap();
        let midi = load_midi(&bytes).unwrap();
        let render = || {
            let mut e = Engine::new(22050, Some(&font)).unwrap();
            e.play_music(1, &midi);
            // `holiday.mid` is quiet for its first 4 s.
            let mut out = vec![0.0; 2 * 22050 * 8];
            e.render(&mut out, 2);
            out
        };
        let a = render();
        assert_eq!(a, render());
        assert!(a.iter().any(|s| s.abs() > 0.01));
    }
}
