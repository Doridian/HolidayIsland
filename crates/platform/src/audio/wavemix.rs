//! A model of `WAVEMIX.DLL` ("Realtime Wave Mixing DLL by Angel Diaz,
//! (c) Microsoft 1993"), the mixer the game plays its effects through, as
//! `HOLIDAY.EXE` sets it up (10a0:2f5e): `WaveMixConfigureInit` with one
//! output channel (mono) and the rate from `WAVEMIX.INI` (11.025 kHz, the
//! default and what the shipped INI keeps for most devices), every wave
//! opened from a file (converted to that rate on open), all eight channels
//! opened at once (`WMIX_OPENALL`), and each wave played with
//! `WMIX_USELRUCHANNEL`, queued (no `WMIX_CLEARQUEUE`), once.
//!
//! Read from the DLL (Ghidra, `WAVEMIX.DLL`'s own segment `1000`):
//!
//! - `WaveMixOpenWave` (1000:227e) reads a RIFF `WAVE` file's `fmt ` and
//!   `data` chunks and converts the PCM to the mixer's format (1000:21a0):
//!   to a lower rate by the whole ratio `src / dst`, each output sample the
//!   average of `ratio` input samples, truncated towards zero, but the last,
//!   which is the first of its group (1000:1ef4, 1000:1c1c); to a higher
//!   rate by the whole ratio `dst / src`, stepping linearly towards the next
//!   sample by `(next - this) / ratio`, the last sample repeated (1000:1d3e).
//!   The game's waves are all 8-bit mono, which is all this model reads.
//! - `WaveMixPlay` (1000:1042) fails without a wave (the game passes the
//!   null pointer of a file that didn't open) or while the session isn't
//!   active (`WaveMixActivate`), silently: `WAVEMIX.INI`'s `ShowDevices` is
//!   0, so no message box. With `WMIX_USELRUCHANNEL` it takes the open
//!   channel played on least recently (a 32-bit count of plays; ties go to
//!   the lowest channel), whether or not it's still playing, so the eight
//!   channels take turns. Without `WMIX_CLEARQUEUE` the wave is queued on
//!   that channel: it starts when the channel's last wave ends, or now.
//! - The mix (`cmixit`, 1000:01b2, and its 386 twin 1000:31b4): one source
//!   is copied, more are summed around 0x80 and clipped to 0..=255; no
//!   source is silence (0x80).
//! - `WaveMixFlushChannel(WMIX_ALL)` empties every channel's queue.
//!
//! Assumed, not modelled: the device's block latency (`WaveBlocks`,
//! `WaveBlockLen`, the "remix" of blocks already written): a wave queued
//! on an idle channel starts at the next sample [`WaveMix::next_sample`]
//! mixes.

use std::collections::VecDeque;
use std::fmt;
use std::sync::Arc;

/// The mixer's rate: `WAVEMIX.INI`'s `SamplesPerSec=11`. // 1000:2b74
pub const SAMPLE_RATE: u32 = 11025;
/// `MAXCHANNELS`: the channels `WMIX_OPENALL` opens. // 1000:07d8
pub const CHANNELS: usize = 8;
/// The 8-bit PCM zero.
pub const SILENCE: u8 = 0x80;

/// Why a file isn't a wave the mixer plays (`WaveMixOpenWave`'s errors).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WaveError {
    /// "This is not a WAVE file."
    NotWave,
    /// "Failed to read format chunk."
    NoFormat,
    /// "The file is not a PCM file."
    NotPcm,
    /// "WAVE file has no data chunk."
    NoData,
    /// A PCM format other than the game's 8-bit mono.
    Unsupported { bits: u16, channels: u16 },
}

impl fmt::Display for WaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WaveError::NotWave => f.write_str("not a WAVE file"),
            WaveError::NoFormat => f.write_str("no format chunk"),
            WaveError::NotPcm => f.write_str("not PCM"),
            WaveError::NoData => f.write_str("no data chunk"),
            WaveError::Unsupported { bits, channels } => {
                write!(f, "unsupported PCM: {bits} bits, {channels} channels")
            }
        }
    }
}

impl std::error::Error for WaveError {}

/// A RIFF `WAVE` file's PCM: 8-bit mono, as the game's are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WavFile {
    pub rate: u32,
    /// `nAvgBytesPerSec`.
    pub bytes_per_second: u32,
    /// Unsigned 8-bit samples.
    pub samples: Vec<u8>,
}

impl WavFile {
    /// Reads `file`'s `fmt ` and `data` chunks, little-endian, with bounds
    /// checks. A `data` chunk longer than the file is cut at its end.
    ///
    /// # Errors
    /// If it isn't an 8-bit mono PCM wave.
    pub fn parse(file: &[u8]) -> Result<WavFile, WaveError> {
        let u16_at = |at: usize| file.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
        let u32_at = |at: usize| {
            file.get(at..at + 4)
                .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        };
        if file.get(0..4) != Some(b"RIFF") || file.get(8..12) != Some(b"WAVE") {
            return Err(WaveError::NotWave);
        }
        let mut format = None;
        let mut data = None;
        let mut at = 12;
        while let (Some(id), Some(len)) = (file.get(at..at + 4), u32_at(at + 4)) {
            let body = at + 8;
            let len = usize::try_from(len).unwrap_or(usize::MAX);
            match id {
                b"fmt " => {
                    let tag = u16_at(body).ok_or(WaveError::NoFormat)?;
                    let channels = u16_at(body + 2).ok_or(WaveError::NoFormat)?;
                    let rate = u32_at(body + 4).ok_or(WaveError::NoFormat)?;
                    let bytes_per_second = u32_at(body + 8).ok_or(WaveError::NoFormat)?;
                    let bits = u16_at(body + 14).ok_or(WaveError::NoFormat)?;
                    if tag != 1 {
                        return Err(WaveError::NotPcm);
                    }
                    if bits != 8 || channels != 1 {
                        return Err(WaveError::Unsupported { bits, channels });
                    }
                    format = Some((rate, bytes_per_second));
                }
                b"data" => {
                    let end = body.saturating_add(len).min(file.len());
                    data = Some(file.get(body..end).unwrap_or_default().to_vec());
                }
                _ => {}
            }
            // Chunks are word-aligned.
            at = body.saturating_add(len).saturating_add(len & 1);
        }
        let (rate, bytes_per_second) = format.ok_or(WaveError::NoFormat)?;
        let samples = data.ok_or(WaveError::NoData)?;
        Ok(WavFile {
            rate,
            bytes_per_second,
            samples,
        })
    }

    /// How long it plays, in ms, as MCI's `waveaudio` reports its length:
    /// the data at `nAvgBytesPerSec`.
    #[must_use]
    pub fn length_ms(&self) -> u32 {
        if self.bytes_per_second == 0 {
            return 0;
        }
        let ms = self.samples.len() as u64 * 1000 / u64::from(self.bytes_per_second);
        u32::try_from(ms).unwrap_or(u32::MAX)
    }
}

/// An opened wave: its samples at [`SAMPLE_RATE`]. // `WaveMixOpenWave`, 1000:227e
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wave {
    samples: Arc<[u8]>,
}

impl Wave {
    /// `WaveMixOpenWave` of a file's bytes.
    ///
    /// # Errors
    /// As [`WavFile::parse`].
    pub fn open(file: &[u8]) -> Result<Wave, WaveError> {
        let wav = WavFile::parse(file)?;
        Ok(Wave::from_pcm(wav.rate, &wav.samples))
    }

    /// 8-bit mono `samples` at `rate`, converted to [`SAMPLE_RATE`].
    #[must_use]
    pub fn from_pcm(rate: u32, samples: &[u8]) -> Wave {
        Wave {
            samples: convert_rate(samples, rate, SAMPLE_RATE).into(),
        }
    }

    #[must_use]
    pub fn samples(&self) -> &[u8] {
        &self.samples
    }
}

/// The sample-rate conversion of 8-bit mono PCM (1000:1ef4): by whole
/// ratios only, as the DLL does, so a rate that isn't a multiple keeps its
/// samples and plays at the wrong speed.
#[must_use]
pub fn convert_rate(samples: &[u8], src: u32, dst: u32) -> Vec<u8> {
    if src == dst || src == 0 || dst == 0 {
        return samples.to_vec();
    }
    if dst <= src {
        // 1000:1c1c: the average of each group of `ratio`, around 0x80.
        let ratio = usize::try_from(src / dst).unwrap_or(1);
        let frames = samples.len() / ratio;
        let mut out = Vec::with_capacity(frames);
        for k in 0..frames {
            let group = &samples[k * ratio..k * ratio + ratio];
            if k + 1 == frames {
                // The last one is copied, not averaged.
                out.push(group[0]);
            } else {
                let sum: i32 = group.iter().map(|&s| i32::from(s) - 0x80).sum();
                // A 16-bit `int` division in the original: truncated
                // towards zero, then stored as a `char` less 0x80.
                #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
                let q = (sum / ratio as i32) as i8;
                out.push(q.wrapping_sub(i8::MIN).cast_unsigned());
            }
        }
        out
    } else {
        // 1000:1d3e: steps of `(next - this) / ratio` from each sample.
        let ratio = usize::try_from(dst / src).unwrap_or(1);
        let mut out = Vec::with_capacity(samples.len() * ratio);
        for (i, &s) in samples.iter().enumerate() {
            match samples.get(i + 1) {
                Some(&next) => {
                    // A byte added with wraparound, as the original.
                    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap, clippy::cast_sign_loss)]
                    let step = ((i32::from(next) - i32::from(s)) / ratio as i32) as u8;
                    let mut v = s;
                    out.push(v);
                    for _ in 1..ratio {
                        v = v.wrapping_add(step);
                        out.push(v);
                    }
                }
                None => out.extend(std::iter::repeat_n(s, ratio)),
            }
        }
        out
    }
}

/// Why `WaveMixPlay` failed (its return value 5, `MMSYSERR_INVALPARAM`
/// and the like, which the game ignores).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayError {
    /// "NULL wave pointer passed to `WaveMixPlay`!"
    NoWave,
    /// The session isn't active (`WaveMixActivate(FALSE)`).
    Inactive,
}

/// A wave queued on a channel, from sample `start` of the mix on.
#[derive(Debug, Clone)]
struct Queued {
    wave: Wave,
    start: u64,
}

impl Queued {
    fn end(&self) -> u64 {
        self.start + self.wave.samples.len() as u64
    }
}

#[derive(Debug, Clone, Default)]
struct Channel {
    /// The play count when it was last chosen. // session +0x3e + 4n
    last_play: u32,
    queue: VecDeque<Queued>,
}

/// A `WaveMix` session with its eight channels open.
#[derive(Debug, Clone)]
pub struct WaveMix {
    channels: [Channel; CHANNELS],
    /// `WaveMixActivate`'s state: plays fail while inactive.
    active: bool,
    /// The plays so far, which pick the least recently used channel.
    /// // session +0x5e
    plays: u32,
    /// The next sample [`WaveMix::next_sample`] mixes.
    now: u64,
}

impl Default for WaveMix {
    fn default() -> Self {
        WaveMix::new()
    }
}

impl WaveMix {
    /// `WaveMixConfigureInit` and `WaveMixOpenChannel(0, WMIX_OPENALL)`:
    /// all channels open and idle, the session inactive until
    /// [`WaveMix::activate`].
    #[must_use]
    pub fn new() -> WaveMix {
        WaveMix {
            channels: Default::default(),
            active: false,
            plays: 0,
            now: 0,
        }
    }

    /// `WaveMixActivate`. Deactivating doesn't empty the queues; the game
    /// flushes them first (10a0:2518).
    pub fn activate(&mut self, on: bool) {
        self.active = on;
    }

    #[must_use]
    pub fn active(&self) -> bool {
        self.active
    }

    /// `WaveMixFlushChannel(0, WMIX_ALL)`: every channel's queue emptied.
    pub fn flush(&mut self) {
        for c in &mut self.channels {
            c.queue.clear();
        }
    }

    /// `WaveMixPlay` with `WMIX_USELRUCHANNEL`, queued, no loops: the
    /// channel it went on.
    ///
    /// # Errors
    /// Without a wave or while inactive; nothing changes then.
    pub fn play(&mut self, wave: Option<&Wave>) -> Result<usize, PlayError> {
        let wave = wave.ok_or(PlayError::NoWave)?;
        if !self.active {
            return Err(PlayError::Inactive);
        }
        // The least recently chosen channel: ties to the lowest.
        let mut lru = 0;
        for (n, c) in self.channels.iter().enumerate() {
            if c.last_play < self.channels[lru].last_play {
                lru = n;
            }
        }
        self.plays = self.plays.wrapping_add(1);
        let now = self.now;
        let channel = &mut self.channels[lru];
        channel.last_play = self.plays;
        let start = channel.queue.back().map_or(now, Queued::end).max(now);
        channel.queue.push_back(Queued {
            wave: wave.clone(),
            start,
        });
        Ok(lru)
    }

    /// Whether any channel has something to play.
    #[must_use]
    pub fn playing(&self) -> bool {
        self.channels.iter().any(|c| !c.queue.is_empty())
    }

    /// The next sample of the mix (`cmixit`, 1000:01b2): the channels'
    /// samples summed around 0x80 and clipped.
    pub fn next_sample(&mut self) -> u8 {
        let t = self.now;
        self.now += 1;
        let mut sum = i32::from(SILENCE);
        for c in &mut self.channels {
            while c.queue.front().is_some_and(|q| q.end() <= t) {
                c.queue.pop_front();
            }
            if let Some(q) = c.queue.front()
                && q.start <= t
            {
                let at = usize::try_from(t - q.start).unwrap_or(usize::MAX);
                sum += i32::from(q.wave.samples[at]) - i32::from(SILENCE);
            }
        }
        u8::try_from(sum.clamp(0, 0xff)).unwrap_or(SILENCE)
    }

    /// Mixes the next `out.len()` samples.
    pub fn mix(&mut self, out: &mut [u8]) {
        for s in out {
            *s = self.next_sample();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wav(rate: u32, samples: &[u8]) -> Vec<u8> {
        let mut f = Vec::new();
        f.extend_from_slice(b"RIFF");
        f.extend_from_slice(&u32::try_from(36 + samples.len()).unwrap().to_le_bytes());
        f.extend_from_slice(b"WAVEfmt ");
        f.extend_from_slice(&16u32.to_le_bytes());
        f.extend_from_slice(&1u16.to_le_bytes());
        f.extend_from_slice(&1u16.to_le_bytes());
        f.extend_from_slice(&rate.to_le_bytes());
        f.extend_from_slice(&rate.to_le_bytes());
        f.extend_from_slice(&1u16.to_le_bytes());
        f.extend_from_slice(&8u16.to_le_bytes());
        f.extend_from_slice(b"data");
        f.extend_from_slice(&u32::try_from(samples.len()).unwrap().to_le_bytes());
        f.extend_from_slice(samples);
        f
    }

    #[test]
    fn parses_and_times_a_wave() {
        let w = WavFile::parse(&wav(22050, &vec![0x80; 22050])).unwrap();
        assert_eq!((w.rate, w.samples.len(), w.length_ms()), (22050, 22050, 1000));
        assert_eq!(WavFile::parse(b"RIFF\0\0\0\0WAVX"), Err(WaveError::NotWave));
        let mut stereo = wav(11025, &[0; 4]);
        stereo[22] = 2;
        assert_eq!(
            WavFile::parse(&stereo),
            Err(WaveError::Unsupported { bits: 8, channels: 2 })
        );
    }

    #[test]
    fn halves_22_khz_by_averaging_pairs() {
        // (0x90 + 0x70) / 2 = 0x80; (0x81 + 0x80) / 2 truncates to 0x80;
        // (0x7f + 0x7e) / 2 = -1.5 -> -1 = 0x7f; the last pair's first.
        let w = Wave::open(&wav(22050, &[0x90, 0x70, 0x81, 0x80, 0x7f, 0x7e, 0x10, 0xf0])).unwrap();
        assert_eq!(w.samples(), [0x80, 0x80, 0x7f, 0x10]);
        // An odd sample at the end is dropped.
        assert_eq!(convert_rate(&[0xa0, 0xa0, 0x30], 22050, 11025), [0x30 + 0x70]);
        assert_eq!(convert_rate(&[1, 2, 3], 11025, 11025), [1, 2, 3]);
    }

    #[test]
    fn doubles_by_stepping_towards_the_next() {
        // (0x90 - 0x80) / 2 = 8; the last sample twice.
        assert_eq!(
            convert_rate(&[0x80, 0x90, 0x85], 11025, 22050),
            [0x80, 0x88, 0x90, 0x8b, 0x85, 0x85]
        );
    }

    #[test]
    fn mixes_around_the_zero_and_clips() {
        let mut m = WaveMix::new();
        m.activate(true);
        let a = Wave::from_pcm(SAMPLE_RATE, &[0xf0, 0x90, 0x10]);
        let b = Wave::from_pcm(SAMPLE_RATE, &[0xf0, 0x70]);
        assert_eq!(m.play(Some(&a)), Ok(0));
        assert_eq!(m.play(Some(&b)), Ok(1));
        let mut out = [0; 4];
        m.mix(&mut out);
        assert_eq!(out, [0xff, 0x80, 0x10, SILENCE]);
        assert!(!m.playing());
    }

    #[test]
    fn channels_take_turns_and_queue() {
        let mut m = WaveMix::new();
        assert_eq!(
            m.play(Some(&Wave::from_pcm(SAMPLE_RATE, &[1]))),
            Err(PlayError::Inactive)
        );
        m.activate(true);
        assert_eq!(m.play(None), Err(PlayError::NoWave));
        let long = Wave::from_pcm(SAMPLE_RATE, &[0x90; 3]);
        let click = Wave::from_pcm(SAMPLE_RATE, &[0xa0]);
        let picked: Vec<usize> = (0..CHANNELS).map(|_| m.play(Some(&long)).unwrap()).collect();
        assert_eq!(picked, (0..CHANNELS).collect::<Vec<_>>());
        // The ninth goes on channel 0 again, after its wave.
        assert_eq!(m.play(Some(&click)), Ok(0));
        let mut out = [0; 5];
        m.mix(&mut out);
        assert_eq!(out, [0xff, 0xff, 0xff, 0xa0, SILENCE]);
        // Flushing stops everything.
        m.play(Some(&long)).unwrap();
        m.flush();
        assert_eq!(m.next_sample(), SILENCE);
    }

    #[test]
    fn a_queued_wave_waits_only_while_its_channel_plays() {
        let mut m = WaveMix::new();
        m.activate(true);
        let w = Wave::from_pcm(SAMPLE_RATE, &[0x90, 0x90]);
        for _ in 0..CHANNELS {
            m.play(Some(&w)).unwrap();
        }
        let mut out = [0; 10];
        m.mix(&mut out);
        // Channel 0 is idle again: the next wave starts at once.
        assert_eq!(m.play(Some(&w)), Ok(0));
        assert_eq!(m.next_sample(), 0x90);
    }
}
