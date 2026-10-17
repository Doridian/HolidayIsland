//! Cinepak (`cvid`), the films' video codec, decoded to 24-bit RGB as the
//! films are stored (`biBitCount` 24, no palette).
//!
//! A frame is a 10-byte header (flags, size, width, height, strip count;
//! big-endian) and strips of 4×4 blocks. Each strip has two codebooks of
//! 256 entries, kept from frame to frame: four luma samples of a 2×2
//! block plus, for colour entries, one `u`, `v` pair. A block is either
//! one "V1" entry scaled up to 4×4 or four "V4" entries, one per 2×2
//! quarter; in inter-coded strips a block can also keep the frame
//! before's pixels. The colour conversion is Cinepak's own:
//! `r = y + 2v`, `g = y - u/2 - v`, `b = y + 2u`, clamped.
//!
//! Like `FFmpeg`'s decoder this takes each strip's height from its header
//! and its width from the frame's, ignores the strips' x coordinates, and
//! with frame flag bit 0 clear starts each strip after the first with the
//! codebooks of the strip before. A frame whose width or height isn't a
//! multiple of 4 (the TV's films are 335 × 257) is decoded into a buffer
//! rounded up and cut: the extra columns are dropped, and the last row
//! holds what a block's top row has, as with `FFmpeg`.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// A header or chunk runs past the frame's data.
    Truncated,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Truncated => write!(f, "Cinepak frame is truncated"),
        }
    }
}

impl std::error::Error for Error {}

/// One codebook entry: a 2×2 block's four pixels as RGB, top left, top
/// right, bottom left, bottom right.
type Entry = [[u8; 3]; 4];

#[derive(Debug, Clone)]
struct Strip {
    v1: Box<[Entry; 256]>,
    v4: Box<[Entry; 256]>,
}

impl Default for Strip {
    fn default() -> Self {
        Strip {
            v1: Box::new([[[0; 3]; 4]; 256]),
            v4: Box::new([[[0; 3]; 4]; 256]),
        }
    }
}

/// The most strips a frame may have (`FFmpeg`'s limit).
const MAX_STRIPS: usize = 32;

/// A Cinepak stream's decoder: the picture and the strips' codebooks
/// carry over from frame to frame.
#[derive(Debug, Clone)]
pub struct Decoder {
    width: usize,
    height: usize,
    /// The size rounded up to whole blocks.
    stride: usize,
    rows: usize,
    /// RGB, `stride` pixels a row.
    pixels: Vec<u8>,
    strips: Vec<Strip>,
}

fn be16(d: &[u8], at: usize) -> Result<usize, Error> {
    d.get(at..at + 2)
        .map(|b| usize::from(u16::from_be_bytes([b[0], b[1]])))
        .ok_or(Error::Truncated)
}

fn be24(d: &[u8], at: usize) -> Result<usize, Error> {
    d.get(at..at + 3)
        .map(|b| (usize::from(b[0]) << 16) | (usize::from(b[1]) << 8) | usize::from(b[2]))
        .ok_or(Error::Truncated)
}

fn be32(d: &[u8], at: usize) -> Result<u32, Error> {
    d.get(at..at + 4)
        .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
        .ok_or(Error::Truncated)
}

fn clamp(v: i32) -> u8 {
    u8::try_from(v.clamp(0, 255)).unwrap_or(0)
}

/// Reads a codebook chunk (ids 0x20 to 0x27) into `book`: bit 2 set for
/// luma-only entries (4 bytes), clear for colour ones (6); bit 0 set for
/// a partial update, a 32-bit mask before each 32 entries saying which
/// change.
#[allow(clippy::many_single_char_names)] // Cinepak's `y`, `u`, `v`.
fn read_codebook(book: &mut [Entry; 256], id: u8, d: &[u8]) {
    let n = if id & 0x04 != 0 { 4 } else { 6 };
    let partial = id & 0x01 != 0;
    let mut at = 0;
    let mut flags = 0u32;
    let mut mask = 0u32;
    for entry in book.iter_mut() {
        if partial {
            mask >>= 1;
            if mask == 0 {
                let Ok(f) = be32(d, at) else { return };
                flags = f;
                at += 4;
                mask = 0x8000_0000;
            }
            if flags & mask == 0 {
                continue;
            }
        }
        let Some(b) = d.get(at..at + n) else { return };
        at += n;
        let (u, v) = if n == 6 {
            (i32::from(b[4].cast_signed()), i32::from(b[5].cast_signed()))
        } else {
            (0, 0)
        };
        for (px, &y) in entry.iter_mut().zip(&b[..4]) {
            let y = i32::from(y);
            *px = if n == 6 {
                // C's division: toward zero.
                [clamp(y + 2 * v), clamp(y - u / 2 - v), clamp(y + 2 * u)]
            } else {
                let y = clamp(y);
                [y; 3]
            };
        }
    }
}

impl Decoder {
    /// A decoder for `width` × `height` frames, black to start with.
    #[must_use]
    pub fn new(width: usize, height: usize) -> Decoder {
        let stride = width.div_ceil(4) * 4;
        let rows = height.div_ceil(4) * 4;
        Decoder {
            width,
            height,
            stride,
            rows,
            pixels: vec![0; stride * rows * 3],
            strips: vec![Strip::default(); MAX_STRIPS],
        }
    }

    #[must_use]
    pub fn width(&self) -> usize {
        self.width
    }

    #[must_use]
    pub fn height(&self) -> usize {
        self.height
    }

    /// The picture as RGB, `width` × `height`, top row first.
    #[must_use]
    pub fn rgb(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.width * self.height * 3);
        for y in 0..self.height {
            let row = y * self.stride * 3;
            out.extend_from_slice(&self.pixels[row..row + self.width * 3]);
        }
        out
    }

    /// Decodes a frame onto the picture. An empty frame (a dropped one)
    /// leaves it as it was.
    ///
    /// # Errors
    /// If the frame's headers run past its data. What was decoded before
    /// stays.
    pub fn decode(&mut self, frame: &[u8]) -> Result<(), Error> {
        if frame.is_empty() {
            return Ok(());
        }
        let flags = *frame.first().ok_or(Error::Truncated)?;
        let strips = be16(frame, 8)?.min(MAX_STRIPS);
        let mut at = 10;
        let mut y0 = 0;
        for i in 0..strips {
            let header = frame.get(at..at + 12).ok_or(Error::Truncated)?;
            let size = be24(header, 1)?.checked_sub(12).ok_or(Error::Truncated)?;
            let y1 = y0;
            let y2 = (y0 + be16(header, 8)?).min(self.rows);
            at += 12;
            let end = (at + size).min(frame.len());
            if i > 0 && flags & 0x01 == 0 {
                let (before, after) = self.strips.split_at_mut(i);
                after[0].clone_from(&before[i - 1]);
            }
            self.decode_strip(i, y1, y2, &frame[at..end])?;
            at = end;
            y0 = y2;
        }
        Ok(())
    }

    fn decode_strip(&mut self, i: usize, y1: usize, y2: usize, d: &[u8]) -> Result<(), Error> {
        let mut at = 0;
        while at + 4 <= d.len() {
            let id = d[at];
            let size = be24(d, at + 1)?.checked_sub(4).ok_or(Error::Truncated)?;
            at += 4;
            let end = (at + size).min(d.len());
            let body = &d[at..end];
            match id {
                0x20 | 0x21 | 0x24 | 0x25 => read_codebook(&mut self.strips[i].v4, id, body),
                0x22 | 0x23 | 0x26 | 0x27 => read_codebook(&mut self.strips[i].v1, id, body),
                0x30..=0x32 => return self.decode_blocks(i, id, y1, y2, body),
                _ => {}
            }
            at = end;
        }
        Ok(())
    }

    /// The blocks of a strip: chunk 0x30 has a mask word before each 32
    /// blocks, one bit each, V4 if set; 0x32 is V1 only; 0x31 (inter) has
    /// a bit for whether a block changes and, if so, the next for V4.
    fn decode_blocks(&mut self, i: usize, id: u8, y1: usize, y2: usize, d: &[u8]) -> Result<(), Error> {
        let mut at = 0;
        let mut flags = 0u32;
        let mut mask = 0u32;
        let mut next_bit = |at: &mut usize, flags: &mut u32| -> Result<bool, Error> {
            mask >>= 1;
            if mask == 0 {
                *flags = be32(d, *at)?;
                *at += 4;
                mask = 0x8000_0000;
            }
            Ok(*flags & mask != 0)
        };
        let byte = |at: &mut usize| -> Result<usize, Error> {
            let b = *d.get(*at).ok_or(Error::Truncated)?;
            *at += 1;
            Ok(usize::from(b))
        };
        let strip = &self.strips[i];
        let stride = self.stride;
        let put = |pixels: &mut [u8], x: usize, y: usize, rgb: [u8; 3]| {
            let p = (y * stride + x) * 3;
            pixels[p..p + 3].copy_from_slice(&rgb);
        };
        for by in (y1..y2).step_by(4) {
            for bx in (0..self.width).step_by(4) {
                let coded = id & 0x01 == 0 || next_bit(&mut at, &mut flags)?;
                if !coded {
                    continue;
                }
                // 0x30 and 0x31 (after "changes"): the next bit.
                let v4 = id != 0x32 && next_bit(&mut at, &mut flags)?;
                if v4 {
                    let e = [byte(&mut at)?, byte(&mut at)?, byte(&mut at)?, byte(&mut at)?];
                    for (q, &n) in e.iter().enumerate() {
                        let (qx, qy) = (bx + 2 * (q % 2), by + 2 * (q / 2));
                        for (k, &rgb) in strip.v4[n].iter().enumerate() {
                            put(&mut self.pixels, qx + k % 2, qy + k / 2, rgb);
                        }
                    }
                } else {
                    let n = byte(&mut at)?;
                    for (k, &rgb) in strip.v1[n].iter().enumerate() {
                        let (qx, qy) = (bx + 2 * (k % 2), by + 2 * (k / 2));
                        for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                            put(&mut self.pixels, qx + dx, qy + dy, rgb);
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A frame of `strips`, each `(id, height, chunks)`.
    fn frame(flags: u8, w: u16, h: u16, strips: &[(u16, u16, Vec<u8>)]) -> Vec<u8> {
        let mut body = Vec::new();
        for (id, height, chunks) in strips {
            let size = u32::try_from(12 + chunks.len()).unwrap();
            body.extend(id.to_be_bytes()[..1].iter().chain(&size.to_be_bytes()[1..]));
            body.extend([0, 0, 0, 0]);
            body.extend(height.to_be_bytes());
            body.extend(w.to_be_bytes());
            body.extend(chunks);
        }
        let mut f = vec![flags];
        f.extend(&u32::try_from(10 + body.len()).unwrap().to_be_bytes()[1..]);
        f.extend(w.to_be_bytes());
        f.extend(h.to_be_bytes());
        f.extend(u16::try_from(strips.len()).unwrap().to_be_bytes());
        f.extend(body);
        f
    }

    fn chunk(id: u8, body: &[u8]) -> Vec<u8> {
        let mut c = vec![id];
        c.extend(&u32::try_from(4 + body.len()).unwrap().to_be_bytes()[1..]);
        c.extend(body);
        c
    }

    #[test]
    fn v1_and_v4_blocks() {
        // Grey codebooks: V1 entry 0 is 10, 20, 30, 40; V4 entry 1 is
        // 50, 60, 70, 80.
        let mut v1 = vec![10, 20, 30, 40];
        v1.extend([0; 4 * 255]);
        let mut v4 = vec![0; 4];
        v4.extend([50, 60, 70, 80]);
        v4.extend([0; 4 * 254]);
        // Two blocks: V1 0, then V4 1, 1, 1, 1.
        let blocks = [0x40, 0, 0, 0, 0, 1, 1, 1, 1];
        let chunks = [chunk(0x26, &v1), chunk(0x24, &v4), chunk(0x30, &blocks)].concat();
        let f = frame(0, 8, 4, &[(0x1000, 4, chunks)]);
        let mut d = Decoder::new(8, 4);
        d.decode(&f).unwrap();
        let rgb = d.rgb();
        let grey = |x: usize, y: usize| rgb[(y * 8 + x) * 3];
        // V1: each sample fills a 2×2 quarter.
        assert_eq!(
            [grey(0, 0), grey(1, 1), grey(2, 0), grey(0, 2), grey(3, 3)],
            [10, 10, 20, 30, 40]
        );
        // V4: each entry is a quarter's four pixels.
        assert_eq!(
            [grey(4, 0), grey(5, 0), grey(4, 1), grey(5, 1), grey(7, 3)],
            [50, 60, 70, 80, 80]
        );
    }

    #[test]
    fn colour_entries_and_inter_blocks() {
        // A colour V1 entry: y 100, u -10, v 20.
        let mut v1 = vec![100, 100, 100, 100, (-10i8).cast_unsigned(), 20];
        v1.extend([0; 6 * 255]);
        let f = frame(0, 4, 4, &[(0x1000, 4, [chunk(0x22, &v1), chunk(0x32, &[0])].concat())]);
        let mut d = Decoder::new(4, 4);
        d.decode(&f).unwrap();
        // r = 100 + 40, g = 100 + 5 - 20, b = 100 - 20.
        assert_eq!(d.rgb()[..3], [140, 85, 80]);
        // An inter frame that skips the block keeps it.
        let f = frame(1, 4, 4, &[(0x1100, 4, chunk(0x31, &[0, 0, 0, 0]))]);
        d.decode(&f).unwrap();
        assert_eq!(d.rgb()[..3], [140, 85, 80]);
        // A dropped frame changes nothing.
        d.decode(&[]).unwrap();
        assert_eq!(d.rgb()[..3], [140, 85, 80]);
        assert_eq!(d.decode(&[0, 0, 0]), Err(Error::Truncated));
    }
}
