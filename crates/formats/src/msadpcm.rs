//! Microsoft ADPCM (`WAVE_FORMAT_ADPCM`, tag 2), the films' sound: 4-bit
//! samples in blocks of `nBlockAlign` bytes. Each block starts, per
//! channel, with a predictor number, the step (`iDelta`) and the two
//! samples before the first, which come out first (the older one
//! first); then a nibble per sample, high nibble first, channels
//! interleaved.
//!
//! The prediction is `(s1 * c1 + s2 * c2) / 256` with C's division
//! (toward zero), as `FFmpeg`'s and Wine's decoders have it. The
//! coefficients come from the format's extra bytes; the films have the
//! standard seven.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Not 4-bit MS ADPCM with one or two channels and a block size.
    Unsupported,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Unsupported => write!(f, "not a supported MS ADPCM format"),
        }
    }
}

impl std::error::Error for Error {}

/// `WAVE_FORMAT_ADPCM`.
pub const FORMAT_TAG: u16 = 2;

/// The standard coefficient pairs, for a format without its own.
pub const STANDARD_COEFFICIENTS: [(i16, i16); 7] = [
    (256, 0),
    (512, -256),
    (0, 0),
    (192, 64),
    (240, 0),
    (460, -208),
    (392, -232),
];

/// How the step changes after each nibble.
const ADAPTATION: [i32; 16] = [
    230, 230, 230, 230, 307, 409, 512, 614, 768, 614, 512, 409, 307, 230, 230, 230,
];

/// What decoding needs of a `WAVEFORMATEX`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Format {
    pub channels: usize,
    pub block_align: usize,
    pub samples_per_block: usize,
    pub coefficients: Vec<(i16, i16)>,
}

impl Format {
    /// From a `WAVEFORMATEX`'s fields: its tag, channels, block size,
    /// bits per sample and extra bytes (`wSamplesPerBlock`, `wNumCoef`,
    /// the coefficients).
    ///
    /// # Errors
    /// If it isn't 4-bit MS ADPCM with one or two channels.
    pub fn new(tag: u16, channels: u16, block_align: u16, bits: u16, extra: &[u8]) -> Result<Format, Error> {
        let channels = usize::from(channels);
        let block_align = usize::from(block_align);
        if tag != FORMAT_TAG || bits != 4 || !(1..=2).contains(&channels) || block_align < 7 * channels {
            return Err(Error::Unsupported);
        }
        let word = |i: usize| extra.get(2 * i..2 * i + 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
        let header = 7 * channels;
        let samples_per_block = word(0).map_or_else(|| (block_align - header) * 2 / channels + 2, usize::from);
        let coefficients = match word(1) {
            Some(n) if n > 0 => (0..usize::from(n))
                .map_while(|i| Some((word(2 + 2 * i)?.cast_signed(), word(3 + 2 * i)?.cast_signed())))
                .collect(),
            _ => STANDARD_COEFFICIENTS.to_vec(),
        };
        Ok(Format {
            channels,
            block_align,
            samples_per_block,
            coefficients,
        })
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct Channel {
    c1: i32,
    c2: i32,
    delta: i32,
    s1: i32,
    s2: i32,
}

impl Channel {
    fn expand(&mut self, nibble: u8) -> i16 {
        let signed = i32::from(nibble) - if nibble & 8 != 0 { 16 } else { 0 };
        let predicted = (self.s1 * self.c1 + self.s2 * self.c2) / 256 + signed * self.delta;
        let sample = predicted.clamp(i32::from(i16::MIN), i32::from(i16::MAX));
        self.s2 = self.s1;
        self.s1 = sample;
        self.delta = (ADAPTATION[usize::from(nibble & 0x0f)] * self.delta / 256).max(16);
        i16::try_from(sample).unwrap_or_default()
    }
}

fn le16(d: &[u8], at: usize) -> i32 {
    i32::from(i16::from_le_bytes([d[at], d[at + 1]]))
}

/// Decodes `data`, whole blocks of `format`, to 16-bit samples, channels
/// interleaved. A short last block gives the samples it has; one too
/// short for its header, none. A predictor number past the table uses
/// the last pair.
#[must_use]
pub fn decode(format: &Format, data: &[u8]) -> Vec<i16> {
    let ch = format.channels;
    let mut out = Vec::new();
    for block in data.chunks(format.block_align) {
        let header = 7 * ch;
        if block.len() < header || format.coefficients.is_empty() {
            break;
        }
        let mut channels = [Channel::default(); 2];
        for (c, state) in channels.iter_mut().enumerate().take(ch) {
            let n = usize::from(block[c]).min(format.coefficients.len() - 1);
            let (c1, c2) = format.coefficients[n];
            state.c1 = i32::from(c1);
            state.c2 = i32::from(c2);
            state.delta = le16(block, ch + 2 * c);
            state.s1 = le16(block, 3 * ch + 2 * c);
            state.s2 = le16(block, 5 * ch + 2 * c);
        }
        let start = out.len();
        let wanted = format.samples_per_block * ch;
        for state in &channels[..ch] {
            out.push(i16::try_from(state.s2).unwrap_or_default());
        }
        for state in &channels[..ch] {
            out.push(i16::try_from(state.s1).unwrap_or_default());
        }
        let mut c = 0;
        'block: for &byte in &block[header..] {
            for nibble in [byte >> 4, byte & 0x0f] {
                if out.len() - start >= wanted {
                    break 'block;
                }
                out.push(channels[c].expand(nibble));
                c = (c + 1) % ch;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_format() {
        let mut extra = vec![0xf4, 0x03, 7, 0];
        for (a, b) in STANDARD_COEFFICIENTS {
            extra.extend(a.to_le_bytes());
            extra.extend(b.to_le_bytes());
        }
        let f = Format::new(2, 1, 512, 4, &extra).unwrap();
        assert_eq!(f.samples_per_block, 1012);
        assert_eq!(f.coefficients, STANDARD_COEFFICIENTS);
        assert_eq!(Format::new(1, 1, 512, 8, &extra), Err(Error::Unsupported));
    }

    #[test]
    fn decodes_a_block() {
        let f = Format {
            channels: 1,
            block_align: 9,
            samples_per_block: 6,
            coefficients: STANDARD_COEFFICIENTS.to_vec(),
        };
        // Predictor 1 (2 s1 - s2), delta 16, s1 100, s2 50.
        let block = [1, 16, 0, 100, 0, 50, 0, 0x1f, 0x80];
        let out = decode(&f, &block);
        // s2, s1, then 2 * 100 - 50 + 16 = 166, 2 * 166 - 100 - 16 = 216
        // (the step 16 * 230 / 256, at least 16), 2 * 216 - 166 - 8 * 16 =
        // 138 (the step then 48), 2 * 138 - 216 = 60.
        assert_eq!(out, [50, 100, 166, 216, 138, 60]);
    }
}
