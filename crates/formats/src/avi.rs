//! AVI files (RIFF `AVI `): the films in `FILMHI\` and `FILMLO\`. Only the
//! demuxing: the headers, the video stream's frames and the audio
//! stream's chunks in file order. [`crate::cinepak`] and
//! [`crate::msadpcm`] decode what the films hold (`docs/films.md`).
//!
//! The chunks are read from `movi` (and its `rec ` lists) in file order;
//! `idx1` isn't needed for playing from the start, which is all MCI's
//! `play` does here.

use std::fmt;
use std::ops::Range;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// No `RIFF`…`AVI ` header.
    NotAvi,
    /// A header the file says is there lies outside it.
    Truncated,
    /// No `hdrl`, `avih` or video stream.
    Missing(&'static str),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotAvi => write!(f, "not an AVI file"),
            Error::Truncated => write!(f, "AVI file is truncated"),
            Error::Missing(what) => write!(f, "AVI file has no {what}"),
        }
    }
}

impl std::error::Error for Error {}

/// `AVIMAINHEADER`'s fields the films need.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MainHeader {
    pub micro_sec_per_frame: u32,
    pub total_frames: u32,
    pub streams: u32,
    pub width: u32,
    pub height: u32,
}

/// `AVISTREAMHEADER`'s timing: `rate / scale` units a second, `length`
/// units long, from `start`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StreamHeader {
    pub kind: [u8; 4],
    pub handler: [u8; 4],
    pub scale: u32,
    pub rate: u32,
    pub start: u32,
    pub length: u32,
}

impl StreamHeader {
    /// The time of unit `n`, in ms, rounded down.
    #[must_use]
    pub fn ms(&self, n: u32) -> u64 {
        u64::from(n) * u64::from(self.scale) * 1000 / u64::from(self.rate.max(1))
    }
}

/// The video stream's `BITMAPINFOHEADER` (and palette, for 8 bits and
/// fewer).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoFormat {
    pub width: i32,
    /// Positive for bottom-up DIBs; the codecs here ignore it.
    pub height: i32,
    pub bit_count: u16,
    pub compression: [u8; 4],
    pub palette: Vec<[u8; 3]>,
}

/// The audio stream's `WAVEFORMATEX`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioFormat {
    pub format_tag: u16,
    pub channels: u16,
    pub samples_per_sec: u32,
    pub avg_bytes_per_sec: u32,
    pub block_align: u16,
    pub bits_per_sample: u16,
    /// `cbSize` bytes after the structure (for MS ADPCM: the samples per
    /// block and the coefficients).
    pub extra: Vec<u8>,
}

/// One stream's chunks, as ranges into [`Avi::data`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stream<F> {
    pub header: StreamHeader,
    pub format: F,
    pub chunks: Vec<Range<usize>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Avi {
    pub data: Vec<u8>,
    pub main: MainHeader,
    /// The first video stream: its frames (`##dc`, `##db`), one chunk each,
    /// in order; an empty one repeats the frame before.
    pub video: Stream<VideoFormat>,
    /// The first audio stream (`##wb`), if there is one.
    pub audio: Option<Stream<AudioFormat>>,
    /// The video stream's palette changes (`##pc`), which aren't applied:
    /// none of the films has one.
    pub palette_changes: usize,
}

fn u16_at(d: &[u8], at: usize) -> Result<u16, Error> {
    d.get(at..at + 2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .ok_or(Error::Truncated)
}

fn u32_at(d: &[u8], at: usize) -> Result<u32, Error> {
    d.get(at..at + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .ok_or(Error::Truncated)
}

fn fourcc(d: &[u8], at: usize) -> Result<[u8; 4], Error> {
    d.get(at..at + 4)
        .map(|b| [b[0], b[1], b[2], b[3]])
        .ok_or(Error::Truncated)
}

/// The chunks in `d[range]`: id and body, the body cut at the range's end.
fn chunks(d: &[u8], range: Range<usize>) -> Vec<([u8; 4], Range<usize>)> {
    let mut out = Vec::new();
    let mut at = range.start;
    while at + 8 <= range.end {
        let (Ok(id), Ok(size)) = (fourcc(d, at), u32_at(d, at + 4)) else {
            break;
        };
        let body = at + 8;
        let size = usize::try_from(size).unwrap_or(usize::MAX);
        let end = body.saturating_add(size).min(range.end);
        out.push((id, body..end));
        // Bodies are padded to an even length.
        at = body.saturating_add(size).saturating_add(size & 1);
    }
    out
}

/// A `LIST` body's type and contents.
fn list(d: &[u8], body: &Range<usize>) -> Option<([u8; 4], Range<usize>)> {
    let kind = fourcc(d, body.start).ok()?;
    (body.start + 4 <= body.end).then(|| (kind, body.start + 4..body.end))
}

/// What a stream's `strl` says.
enum Format {
    Video(VideoFormat),
    Audio(AudioFormat),
    Other,
}

fn video_format(d: &[u8], r: &Range<usize>) -> Result<VideoFormat, Error> {
    let at = r.start;
    let header_size = usize::try_from(u32_at(d, at)?).map_err(|_| Error::Truncated)?;
    let bit_count = u16_at(d, at + 14)?;
    let mut colours = usize::try_from(u32_at(d, at + 32)?).map_err(|_| Error::Truncated)?;
    if colours == 0 && bit_count <= 8 {
        colours = 1 << bit_count;
    }
    let mut palette = Vec::new();
    if bit_count <= 8 {
        let start = at + header_size;
        for i in 0..colours.min(256) {
            let p = start + 4 * i;
            if p + 4 > r.end {
                break;
            }
            palette.push([d[p + 2], d[p + 1], d[p]]);
        }
    }
    Ok(VideoFormat {
        width: u32_at(d, at + 4)?.cast_signed(),
        height: u32_at(d, at + 8)?.cast_signed(),
        bit_count,
        compression: fourcc(d, at + 16)?,
        palette,
    })
}

fn audio_format(d: &[u8], r: &Range<usize>) -> Result<AudioFormat, Error> {
    let at = r.start;
    let extra = if r.len() >= 18 {
        let n = usize::from(u16_at(d, at + 16)?);
        d.get(at + 18..(at + 18 + n).min(r.end)).unwrap_or_default().to_vec()
    } else {
        Vec::new()
    };
    Ok(AudioFormat {
        format_tag: u16_at(d, at)?,
        channels: u16_at(d, at + 2)?,
        samples_per_sec: u32_at(d, at + 4)?,
        avg_bytes_per_sec: u32_at(d, at + 8)?,
        block_align: u16_at(d, at + 12)?,
        bits_per_sample: if r.len() >= 16 { u16_at(d, at + 14)? } else { 8 },
        extra,
    })
}

fn stream(d: &[u8], body: Range<usize>) -> Result<(StreamHeader, Format), Error> {
    let mut header = None;
    let mut format = Format::Other;
    for (id, r) in chunks(d, body) {
        match &id {
            b"strh" => {
                header = Some(StreamHeader {
                    kind: fourcc(d, r.start)?,
                    handler: fourcc(d, r.start + 4)?,
                    scale: u32_at(d, r.start + 20)?,
                    rate: u32_at(d, r.start + 24)?,
                    start: u32_at(d, r.start + 28)?,
                    length: u32_at(d, r.start + 32)?,
                });
            }
            b"strf" => {
                format = match header.map(|h| h.kind) {
                    Some(k) if &k == b"vids" => Format::Video(video_format(d, &r)?),
                    Some(k) if &k == b"auds" => Format::Audio(audio_format(d, &r)?),
                    _ => Format::Other,
                };
            }
            _ => {}
        }
    }
    Ok((header.ok_or(Error::Missing("strh"))?, format))
}

/// The stream number of a `movi` chunk id (`00dc`: 0), and its type.
fn chunk_stream(id: [u8; 4]) -> Option<(usize, [u8; 2])> {
    let digit = |c: u8| match c {
        b'0'..=b'9' => Some(usize::from(c - b'0')),
        b'a'..=b'f' => Some(usize::from(c - b'a' + 10)),
        b'A'..=b'F' => Some(usize::from(c - b'A' + 10)),
        _ => None,
    };
    Some((digit(id[0])? * 16 + digit(id[1])?, [id[2], id[3]]))
}

/// Every data chunk in `movi`, descending into `rec ` lists.
fn movi_chunks(d: &[u8], r: Range<usize>, out: &mut Vec<([u8; 4], Range<usize>)>) {
    for (id, body) in chunks(d, r) {
        if &id == b"LIST" {
            if let Some((_, inner)) = list(d, &body) {
                movi_chunks(d, inner, out);
            }
        } else {
            out.push((id, body));
        }
    }
}

impl Avi {
    /// Reads an AVI file's headers and finds its chunks.
    ///
    /// # Errors
    /// If it isn't a RIFF AVI, or has no main header or video stream.
    pub fn parse(data: Vec<u8>) -> Result<Avi, Error> {
        let d = &data;
        if d.get(0..4) != Some(b"RIFF") || d.get(8..12) != Some(b"AVI ") {
            return Err(Error::NotAvi);
        }
        let riff_end = usize::try_from(u32_at(d, 4)?)
            .map_or(d.len(), |n| n.saturating_add(8))
            .min(d.len());
        let mut main = None;
        let mut streams = Vec::new();
        let mut movi = None;
        for (id, body) in chunks(d, 12..riff_end) {
            if &id != b"LIST" {
                continue;
            }
            match list(d, &body) {
                Some((kind, inner)) if &kind == b"hdrl" => {
                    for (id, r) in chunks(d, inner) {
                        match &id {
                            b"avih" => {
                                main = Some(MainHeader {
                                    micro_sec_per_frame: u32_at(d, r.start)?,
                                    total_frames: u32_at(d, r.start + 16)?,
                                    streams: u32_at(d, r.start + 24)?,
                                    width: u32_at(d, r.start + 32)?,
                                    height: u32_at(d, r.start + 36)?,
                                });
                            }
                            b"LIST" => {
                                if let Some((kind, s)) = list(d, &r)
                                    && &kind == b"strl"
                                {
                                    streams.push(stream(d, s)?);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Some((kind, inner)) if &kind == b"movi" => movi = Some(inner),
                _ => {}
            }
        }
        let main = main.ok_or(Error::Missing("avih"))?;
        let video_index = streams
            .iter()
            .position(|(_, f)| matches!(f, Format::Video(_)))
            .ok_or(Error::Missing("video stream"))?;
        let audio_index = streams.iter().position(|(_, f)| matches!(f, Format::Audio(_)));
        let mut all = Vec::new();
        movi_chunks(d, movi.ok_or(Error::Missing("movi"))?, &mut all);
        let mut frames = Vec::new();
        let mut audio_chunks = Vec::new();
        let mut palette_changes = 0;
        for (id, r) in all {
            match chunk_stream(id) {
                Some((n, kind)) if n == video_index => match &kind {
                    b"dc" | b"db" => frames.push(r),
                    b"pc" => palette_changes += 1,
                    _ => {}
                },
                Some((n, kind)) if Some(n) == audio_index && &kind == b"wb" => audio_chunks.push(r),
                _ => {}
            }
        }
        let mut video = None;
        let mut audio = None;
        for (i, (header, format)) in streams.into_iter().enumerate() {
            match format {
                Format::Video(format) if i == video_index => {
                    video = Some(Stream {
                        header,
                        format,
                        chunks: std::mem::take(&mut frames),
                    });
                }
                Format::Audio(format) if Some(i) == audio_index => {
                    audio = Some(Stream {
                        header,
                        format,
                        chunks: std::mem::take(&mut audio_chunks),
                    });
                }
                _ => {}
            }
        }
        Ok(Avi {
            main,
            video: video.ok_or(Error::Missing("video stream"))?,
            audio,
            palette_changes,
            data,
        })
    }

    /// Frame `n`'s data (empty for a dropped frame).
    #[must_use]
    pub fn frame(&self, n: usize) -> &[u8] {
        self.video
            .chunks
            .get(n)
            .and_then(|r| self.data.get(r.clone()))
            .unwrap_or_default()
    }

    /// The number of frames: the stream header's length, or the frames
    /// found if fewer.
    #[must_use]
    pub fn frames(&self) -> usize {
        usize::try_from(self.video.header.length)
            .unwrap_or(usize::MAX)
            .min(self.video.chunks.len())
    }

    /// The audio stream's bytes, its chunks joined.
    #[must_use]
    pub fn audio_bytes(&self) -> Vec<u8> {
        self.audio.as_ref().map_or_else(Vec::new, |a| {
            a.chunks
                .iter()
                .filter_map(|r| self.data.get(r.clone()))
                .flatten()
                .copied()
                .collect()
        })
    }

    /// The film's length in ms: its frames at the video stream's rate.
    #[must_use]
    pub fn duration_ms(&self) -> u64 {
        self.video.header.ms(self.video.header.length)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(id: [u8; 4], body: &[u8]) -> Vec<u8> {
        let mut v = id.to_vec();
        v.extend(u32::try_from(body.len()).unwrap().to_le_bytes());
        v.extend(body);
        if body.len() % 2 == 1 {
            v.push(0);
        }
        v
    }

    fn list_of(kind: [u8; 4], parts: &[Vec<u8>]) -> Vec<u8> {
        let mut body = kind.to_vec();
        for p in parts {
            body.extend(p);
        }
        chunk(*b"LIST", &body)
    }

    fn le(words: &[u32]) -> Vec<u8> {
        words.iter().flat_map(|w| w.to_le_bytes()).collect()
    }

    #[test]
    fn finds_the_streams_and_chunks() {
        let avih = chunk(*b"avih", &le(&[40_000, 0, 0, 0x10, 2, 0, 2, 0, 8, 4, 0, 0, 0, 0]));
        let mut strh_v = b"vidscvid".to_vec();
        strh_v.extend(le(&[0, 0, 0, 1, 25, 0, 2, 0, 0, 0, 0, 0]));
        let mut bih = le(&[40, 8, 4]);
        bih.extend([1, 0, 24, 0]);
        bih.extend(b"cvid");
        bih.extend(le(&[0, 0, 0, 0, 0]));
        let mut strh_a = b"auds\0\0\0\0".to_vec();
        strh_a.extend(le(&[0, 0, 0, 512, 11155, 0, 1, 0, 0, 512, 0, 0]));
        let mut wfx = vec![2, 0, 1, 0];
        wfx.extend(le(&[22050, 11155]));
        wfx.extend([0, 2, 4, 0, 2, 0, 0xf4, 0x03]);
        let hdrl = list_of(
            *b"hdrl",
            &[
                avih,
                list_of(*b"strl", &[chunk(*b"strh", &strh_v), chunk(*b"strf", &bih)]),
                list_of(*b"strl", &[chunk(*b"strh", &strh_a), chunk(*b"strf", &wfx)]),
            ],
        );
        let movi = list_of(
            *b"movi",
            &[
                chunk(*b"JUNK", &[0; 3]),
                list_of(*b"rec ", &[chunk(*b"00dc", &[1, 2, 3]), chunk(*b"01wb", &[9; 4])]),
                chunk(*b"00dc", &[]),
                chunk(*b"00pc", &[0; 4]),
            ],
        );
        let mut body = b"AVI ".to_vec();
        body.extend(hdrl);
        body.extend(movi);
        let avi = Avi::parse(chunk(*b"RIFF", &body)).unwrap();
        assert_eq!((avi.main.width, avi.main.height, avi.main.streams), (8, 4, 2));
        assert_eq!(avi.video.format.compression, *b"cvid");
        assert_eq!(avi.video.format.bit_count, 24);
        assert_eq!(avi.frames(), 2);
        assert_eq!(avi.frame(0), [1, 2, 3]);
        assert_eq!(avi.frame(1), []);
        assert_eq!(avi.palette_changes, 1);
        assert_eq!(avi.duration_ms(), 80);
        let audio = avi.audio.as_ref().unwrap();
        assert_eq!((audio.format.format_tag, audio.format.block_align), (2, 512));
        assert_eq!(audio.format.extra, [0xf4, 0x03]);
        assert_eq!(avi.audio_bytes(), [9; 4]);
    }

    #[test]
    fn rejects_other_files() {
        assert_eq!(Avi::parse(b"RIFF\0\0\0\0WAVE".to_vec()), Err(Error::NotAvi));
        assert_eq!(Avi::parse(b"RIFF\x04\0\0\0AVI ".to_vec()), Err(Error::Missing("avih")));
    }
}
