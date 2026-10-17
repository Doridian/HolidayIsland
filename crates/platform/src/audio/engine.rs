//! What the sound card plays, rendered without a device: the `WaveMix`
//! session's mix ([`super::wavemix`]), the TV's speaker (MCI `waveaudio`,
//! on the same wave device) and the MIDI music (MCI `sequencer`, rendered
//! with `rustysynth` from a `SoundFont`). The game decides what plays when;
//! this only renders it, so tests can render to a buffer.

use std::sync::Arc;

use rustysynth::{MidiFile, MidiFileSequencer, SoundFont, Synthesizer, SynthesizerSettings};

use super::wavemix::{self, PlayError, SILENCE, WavFile, Wave, WaveMix};

/// Linear interpolation of an 8-bit stream from `src` Hz to `dst` Hz, in
/// whole numbers so a render is the same every time.
#[derive(Debug, Clone)]
struct Resampler {
    src: u64,
    dst: u64,
    /// How far between `prev` and `next`, in `dst`ths.
    phase: u64,
    prev: u8,
    next: u8,
}

impl Resampler {
    fn new(src: u32, dst: u32) -> Resampler {
        Resampler {
            src: u64::from(src.max(1)),
            dst: u64::from(dst.max(1)),
            phase: 0,
            prev: SILENCE,
            next: SILENCE,
        }
    }

    /// The next output sample, as -1.0..1.0, pulling input from `pull`.
    #[allow(clippy::cast_precision_loss)] // `phase` and `dst` are below 2^32.
    fn next(&mut self, mut pull: impl FnMut() -> u8) -> f32 {
        self.phase += self.src;
        while self.phase >= self.dst {
            self.phase -= self.dst;
            self.prev = self.next;
            self.next = pull();
        }
        let t = self.phase as f32 / self.dst as f32;
        let (a, b) = (f32::from(self.prev), f32::from(self.next));
        (a + (b - a) * t - 128.0) / 128.0
    }
}

/// The speaker's wave while it plays.
#[derive(Debug, Clone)]
struct Speaker {
    samples: Arc<[u8]>,
    at: usize,
    resampler: Resampler,
}

/// The mixer behind the output device.
pub struct Engine {
    rate: u32,
    wavemix: WaveMix,
    /// The game's waves by `play_sound` number less 1; `None` where the
    /// file didn't open (the original's null pointers). // 1110:728e
    waves: Vec<Option<Wave>>,
    mix: Resampler,
    speaker: Option<Speaker>,
    /// `waveOutSetVolume`: the wave device, `WaveMix` and the speaker.
    wave_volume: f32,
    /// `midiOutSetVolume`.
    music_volume: f32,
    sequencer: Option<MidiFileSequencer>,
    /// Which player's music the sequencer plays.
    music_owner: Option<u8>,
    left: Vec<f32>,
    right: Vec<f32>,
}

/// `waveOutSetVolume`'s and `midiOutSetVolume`'s 16-bit level as a factor.
fn level(volume: u16) -> f32 {
    f32::from(volume) / f32::from(u16::MAX)
}

impl Engine {
    /// An engine rendering at `rate` Hz, with MIDI from `soundfont` if
    /// there is one.
    ///
    /// # Errors
    /// If the synthesizer can't start with the `SoundFont` at that rate.
    pub fn new(rate: u32, soundfont: Option<&Arc<SoundFont>>) -> Result<Engine, String> {
        let sequencer = match soundfont {
            Some(sf) => {
                let settings = SynthesizerSettings::new(i32::try_from(rate).map_err(|e| e.to_string())?);
                let synth = Synthesizer::new(sf, &settings).map_err(|e| format!("{e:?}"))?;
                Some(MidiFileSequencer::new(synth))
            }
            None => None,
        };
        Ok(Engine {
            rate,
            wavemix: WaveMix::new(),
            waves: Vec::new(),
            mix: Resampler::new(wavemix::SAMPLE_RATE, rate),
            speaker: None,
            wave_volume: 1.0,
            music_volume: 1.0,
            sequencer,
            music_owner: None,
            left: Vec::new(),
            right: Vec::new(),
        })
    }

    #[must_use]
    pub fn rate(&self) -> u32 {
        self.rate
    }

    /// Whether MIDI renders (a `SoundFont` was loaded).
    #[must_use]
    pub fn has_synth(&self) -> bool {
        self.sequencer.is_some()
    }

    /// The opened waves, which [`Engine::play_wave`] numbers from 0.
    pub fn set_waves(&mut self, waves: Vec<Option<Wave>>) {
        self.waves = waves;
    }

    /// `WaveMixPlay` of wave `n`.
    ///
    /// # Errors
    /// As [`WaveMix::play`].
    pub fn play_wave(&mut self, n: usize) -> Result<usize, PlayError> {
        let wave = self.waves.get(n).and_then(Option::as_ref);
        self.wavemix.play(wave)
    }

    /// `WaveMixFlushChannel(WMIX_ALL)`.
    pub fn flush(&mut self) {
        self.wavemix.flush();
    }

    /// `WaveMixActivate`.
    pub fn activate(&mut self, on: bool) {
        self.wavemix.activate(on);
    }

    /// `waveOutSetVolume`'s left (and right) level.
    pub fn set_wave_volume(&mut self, volume: u16) {
        self.wave_volume = level(volume);
    }

    /// `midiOutSetVolume`'s left (and right) level.
    pub fn set_music_volume(&mut self, volume: u16) {
        self.music_volume = level(volume);
    }

    /// Player `owner`'s MIDI file plays from its start, once.
    pub fn play_music(&mut self, owner: u8, midi: &Arc<MidiFile>) {
        if let Some(s) = &mut self.sequencer {
            s.play(midi, false);
        }
        self.music_owner = Some(owner);
    }

    /// Player `owner`'s music stops, if it's the one playing.
    pub fn stop_music(&mut self, owner: u8) {
        if self.music_owner == Some(owner) {
            if let Some(s) = &mut self.sequencer {
                s.stop();
            }
            self.music_owner = None;
        }
    }

    /// The speaker's wave plays from its start at its own rate.
    pub fn play_speaker(&mut self, wav: &WavFile) {
        self.speaker = Some(Speaker {
            samples: wav.samples.clone().into(),
            at: 0,
            resampler: Resampler::new(wav.rate, self.rate),
        });
    }

    pub fn stop_speaker(&mut self) {
        self.speaker = None;
    }

    /// Renders the next `out.len() / channels` frames, interleaved, as
    /// -1.0..1.0: the wave device (`WaveMix` and the speaker) at its volume
    /// plus the music at its own.
    pub fn render(&mut self, out: &mut [f32], channels: usize) {
        let channels = channels.max(1);
        let frames = out.len() / channels;
        self.left.clear();
        self.left.resize(frames, 0.0);
        self.right.clear();
        self.right.resize(frames, 0.0);
        if let Some(s) = &mut self.sequencer
            && self.music_owner.is_some()
        {
            s.render(&mut self.left, &mut self.right);
        }
        for (f, frame) in out.chunks_exact_mut(channels).enumerate() {
            let wavemix = &mut self.wavemix;
            let mut wave = self.mix.next(|| wavemix.next_sample());
            if let Some(sp) = &mut self.speaker {
                let samples = &sp.samples;
                let at = &mut sp.at;
                wave += sp.resampler.next(|| {
                    let s = samples.get(*at).copied().unwrap_or(SILENCE);
                    *at += 1;
                    s
                });
            }
            let wave = wave * self.wave_volume;
            let l = (wave + self.left[f] * self.music_volume).clamp(-1.0, 1.0);
            let r = (wave + self.right[f] * self.music_volume).clamp(-1.0, 1.0);
            match frame {
                [mono] => *mono = f32::midpoint(l, r),
                [first, second, rest @ ..] => {
                    *first = l;
                    *second = r;
                    rest.fill(f32::midpoint(l, r));
                }
                [] => {}
            }
        }
        if self.speaker.as_ref().is_some_and(|s| s.at > s.samples.len() + 1) {
            self.speaker = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_the_mix_at_the_device_rate() {
        let mut e = Engine::new(22050, None).unwrap();
        e.set_waves(vec![Some(Wave::from_pcm(wavemix::SAMPLE_RATE, &[0xc0; 4])), None]);
        // Inactive: nothing plays.
        assert!(e.play_wave(0).is_err());
        e.activate(true);
        assert_eq!(e.play_wave(1), Err(PlayError::NoWave));
        assert_eq!(e.play_wave(0), Ok(0));
        let mut out = vec![0.0; 2 * 12];
        e.render(&mut out, 2);
        // Twice the rate, interpolated, a sample late: each sample of 0.5
        // twice, ramping in from the silence before it.
        let left: Vec<f32> = out.iter().step_by(2).copied().collect();
        assert_eq!(left[..5], [0.0, 0.0, 0.25, 0.5, 0.5]);
        assert!(left[11].abs() < 1e-6, "silence after: {left:?}");
        e.set_wave_volume(0);
        e.play_wave(0).unwrap();
        e.render(&mut out, 2);
        assert!(out.iter().all(|s| *s == 0.0));
    }

    #[test]
    fn plays_the_speaker_to_its_end() {
        let mut e = Engine::new(11025, None).unwrap();
        let wav = WavFile {
            rate: 11025,
            bytes_per_second: 11025,
            samples: vec![0x40; 3],
        };
        e.play_speaker(&wav);
        let mut out = vec![0.0; 6];
        e.render(&mut out, 1);
        assert_eq!(out, [0.0, -0.5, -0.5, -0.5, 0.0, 0.0]);
        assert!(e.speaker.is_none());
    }
}
