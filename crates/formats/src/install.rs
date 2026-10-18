//! The installer's archives (`_FILES.1`, `_SETUP.1`): a table of records
//! and one LZHUF stream per file ([docs/formats/install-archive.md]).
//! `_FILES.1` holds `HOLIDAY.EXE` and the rest of what the installer
//! copies that isn't loose on the CD. `tools/extract_install.py` and
//! `tools/lzhuf.py` do the same in Python.
//!
//! [docs/formats/install-archive.md]: ../../../docs/formats/install-archive.md

use std::fmt;

const HEADER: usize = 7;
const RECORD: usize = 135;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The table runs past the file, or a record's index or data is off.
    Layout(usize),
    /// A stream decoded to its size from too few or too many bytes.
    Damaged(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Layout(n) => write!(f, "installer archive: record {n} has an unexpected layout"),
            Error::Damaged(p) => write!(f, "installer archive: {p} is damaged"),
        }
    }
}

impl std::error::Error for Error {}

/// A file in the archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The source path on the build machine
    /// (`y:\global\holiday\sound\auf8b.wav`).
    pub source: String,
    /// The unpacked size.
    pub size: u32,
    /// The stream's offset in the archive and its length.
    pub offset: u32,
    pub packed: u32,
}

impl Entry {
    /// Where the installer puts it below the game's directory, upper case
    /// like the CD's names, `/` between the parts: `SOUND/AUF8B.WAV`; the
    /// installer's own files (`d:\holiday\install\x`) by their name only.
    #[must_use]
    pub fn install_path(&self) -> String {
        let parts: Vec<&str> = self.source.split('\\').collect();
        let low: Vec<String> = parts.iter().map(|p| p.to_ascii_lowercase()).collect();
        let rel = if low.iter().any(|p| p == "install") {
            &parts[parts.len() - 1..]
        } else if let Some(i) = low.iter().position(|p| p == "holiday") {
            &parts[i + 1..]
        } else {
            &parts[parts.len() - 1..]
        };
        rel.iter().map(|p| p.to_ascii_uppercase()).collect::<Vec<_>>().join("/")
    }
}

/// The archive's records.
///
/// # Errors
/// [`Error::Layout`] if the table doesn't fit the file.
pub fn entries(data: &[u8]) -> Result<Vec<Entry>, Error> {
    let u16_at = |p: usize| data.get(p..p + 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
    let u32_at = |p: usize| data.get(p..p + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    let count = u16_at(0).ok_or(Error::Layout(0))?;
    let mut out = Vec::with_capacity(usize::from(count));
    for k in 0..usize::from(count) {
        let p = HEADER + k * RECORD;
        let bad = Error::Layout(k + 1);
        let index = u16_at(p).ok_or_else(|| bad.clone())?;
        let name_len = usize::from(*data.get(p + 15).ok_or_else(|| bad.clone())?);
        let name = data.get(p + 16..p + 16 + name_len).ok_or_else(|| bad.clone())?;
        let (size, offset, packed) = (
            u32_at(p + 99).ok_or_else(|| bad.clone())?,
            u32_at(p + 103).ok_or_else(|| bad.clone())?,
            u32_at(p + 107).ok_or_else(|| bad.clone())?,
        );
        let end = u64::from(offset) + u64::from(packed);
        if usize::from(index) != k + 1 || end > data.len() as u64 {
            return Err(bad);
        }
        out.push(Entry {
            // Latin-1.
            source: name.iter().map(|&b| char::from(b)).collect(),
            size,
            offset,
            packed,
        });
    }
    Ok(out)
}

/// `entry`'s file, unpacked from `archive`.
///
/// # Errors
/// [`Error::Damaged`] if the stream doesn't end where the record says.
pub fn unpack(archive: &[u8], entry: &Entry) -> Result<Vec<u8>, Error> {
    let start = usize::try_from(entry.offset).map_err(|_| Error::Damaged(entry.source.clone()))?;
    let packed = usize::try_from(entry.packed).map_err(|_| Error::Damaged(entry.source.clone()))?;
    let stream = archive
        .get(start..start + packed)
        .ok_or_else(|| Error::Damaged(entry.source.clone()))?;
    let size = usize::try_from(entry.size).map_err(|_| Error::Damaged(entry.source.clone()))?;
    let (out, used) = lzhuf(stream, size);
    // The encoder's final flush can leave one byte the decoder never reads.
    if used + 1 < packed || used > packed {
        return Err(Error::Damaged(entry.source.clone()));
    }
    Ok(out)
}

const N: usize = 4096;
const F: usize = 60;
const THRESHOLD: usize = 2;
const N_CHAR: usize = 256 - THRESHOLD + F;
const T: usize = N_CHAR * 2 - 1;
const R: usize = T - 1;
const MAX_FREQ: u32 = 0x8000;

/// The upper 6 bits of a match's position by the next 8 input bits:
/// the value and the code's length.
fn position_codes() -> ([u8; 256], [u8; 256]) {
    let mut len = Vec::with_capacity(64);
    len.push(3u8);
    len.extend([4; 3]);
    len.extend([5; 8]);
    len.extend([6; 12]);
    len.extend([7; 24]);
    len.extend([8; 16]);
    let mut code: Vec<usize> = vec![0x00, 0x20, 0x30, 0x40, 0x50, 0x58, 0x60, 0x68, 0x70, 0x78, 0x80, 0x88];
    code.extend((0x90..0xc0).step_by(4));
    code.extend((0xc0..0xf0).step_by(2));
    code.extend(0xf0..0x100);
    let (mut d_code, mut d_len) = ([0u8; 256], [0u8; 256]);
    for (v, (&c, &l)) in code.iter().zip(&len).enumerate() {
        for j in 0..1usize << (8 - l) {
            d_code[c + j] = u8::try_from(v).unwrap_or(0);
            d_len[c + j] = l;
        }
    }
    (d_code, d_len)
}

struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
    buf: u8,
    n: u8,
}

impl Bits<'_> {
    fn bit(&mut self) -> usize {
        if self.n == 0 {
            self.buf = self.data.get(self.pos).copied().unwrap_or(0);
            self.pos += 1;
            self.n = 8;
        }
        self.n -= 1;
        usize::from((self.buf >> self.n) & 1)
    }

    fn bits(&mut self, k: u8) -> usize {
        (0..k).fold(0, |v, _| (v << 1) | self.bit())
    }
}

/// LZHUF's adaptive Huffman tree. The names are `LZHUF.C`'s.
struct Tree {
    freq: Vec<u32>,
    prnt: Vec<usize>,
    son: Vec<usize>,
}

impl Tree {
    fn new() -> Tree {
        let mut t = Tree {
            freq: vec![0; T + 1],
            prnt: vec![0; T + N_CHAR],
            son: vec![0; T],
        };
        for i in 0..N_CHAR {
            t.freq[i] = 1;
            t.son[i] = i + T;
            t.prnt[i + T] = i;
        }
        let (mut i, mut j) = (0, N_CHAR);
        while j <= R {
            t.freq[j] = t.freq[i] + t.freq[i + 1];
            t.son[j] = i;
            t.prnt[i] = j;
            t.prnt[i + 1] = j;
            i += 2;
            j += 1;
        }
        t.freq[T] = 0xffff;
        t.prnt[R] = 0;
        t
    }

    fn reconst(&mut self) {
        let mut j = 0;
        for i in 0..T {
            if self.son[i] >= T {
                self.freq[j] = self.freq[i].div_ceil(2);
                self.son[j] = self.son[i];
                j += 1;
            }
        }
        let (mut i, mut j) = (0, N_CHAR);
        while j < T {
            let f = self.freq[i] + self.freq[i + 1];
            self.freq[j] = f;
            let mut k = j - 1;
            while f < self.freq[k] {
                k -= 1;
            }
            k += 1;
            self.freq.copy_within(k..j, k + 1);
            self.freq[k] = f;
            self.son.copy_within(k..j, k + 1);
            self.son[k] = i;
            i += 2;
            j += 1;
        }
        for i in 0..T {
            let k = self.son[i];
            self.prnt[k] = i;
            if k < T {
                self.prnt[k + 1] = i;
            }
        }
    }

    #[allow(clippy::many_single_char_names)]
    fn update(&mut self, c: usize) {
        if self.freq[R] == MAX_FREQ {
            self.reconst();
        }
        let mut c = self.prnt[c + T];
        loop {
            self.freq[c] += 1;
            let k = self.freq[c];
            let mut l = c + 1;
            if k > self.freq[l] {
                while k > self.freq[l + 1] {
                    l += 1;
                }
                self.freq[c] = self.freq[l];
                self.freq[l] = k;
                let i = self.son[c];
                self.prnt[i] = l;
                if i < T {
                    self.prnt[i + 1] = l;
                }
                let j = self.son[l];
                self.son[l] = i;
                self.prnt[j] = c;
                if j < T {
                    self.prnt[j + 1] = c;
                }
                self.son[c] = j;
                c = l;
            }
            c = self.prnt[c];
            if c == 0 {
                break;
            }
        }
    }
}

/// Decodes `size` bytes of the LZHUF stream `data` (Okumura and
/// Yoshizaki, 1988, `LZHUF.C`, without its size prefix): the bytes and
/// how many of `data` it read.
#[must_use]
#[allow(clippy::many_single_char_names)]
pub fn lzhuf(data: &[u8], size: usize) -> (Vec<u8>, usize) {
    let (d_code, d_len) = position_codes();
    let mut tree = Tree::new();
    let mut bits = Bits {
        data,
        pos: 0,
        buf: 0,
        n: 0,
    };
    let mut text = [b' '; N];
    let mut r = N - F;
    let mut out = Vec::with_capacity(size);
    while out.len() < size {
        let mut c = tree.son[R];
        while c < T {
            c = tree.son[c + bits.bit()];
        }
        c -= T;
        tree.update(c);
        if let Ok(byte) = u8::try_from(c) {
            out.push(byte);
            text[r] = byte;
            r = (r + 1) & (N - 1);
            continue;
        }
        let i = bits.bits(8);
        let hi = usize::from(d_code[i]);
        let extra = d_len[i] - 2;
        let i = ((i << extra) | bits.bits(extra)) & 0xff;
        let pos = (r.wrapping_sub((hi << 6) | (i & 0x3f)).wrapping_sub(1)) & (N - 1);
        for k in 0..c - 255 + THRESHOLD {
            let ch = text[(pos + k) & (N - 1)];
            out.push(ch);
            text[r] = ch;
            r = (r + 1) & (N - 1);
        }
    }
    out.truncate(size);
    (out, bits.pos)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_paths() {
        let e = |s: &str| Entry {
            source: s.to_owned(),
            size: 0,
            offset: 0,
            packed: 0,
        };
        assert_eq!(
            e(r"y:\global\holiday\sound\auf8b.wav").install_path(),
            "SOUND/AUF8B.WAV"
        );
        assert_eq!(e(r"y:\global\holiday\holiday.exe").install_path(), "HOLIDAY.EXE");
        assert_eq!(e(r"d:\holiday\install\hinstall.exe").install_path(), "HINSTALL.EXE");
    }

    #[test]
    fn same_as_lzhuf_py() {
        // What tools/lzhuf.py decodes from the same bytes: literals,
        // matches into the ring buffer's spaces and the tree's rebuilds.
        let (out, used) = lzhuf(&[0; 64], 16);
        assert_eq!((out, used), (vec![116; 16], 4));
        let data: Vec<u8> = (0..200u32).map(|i| ((i * 37 + 11) & 0xff) as u8).collect();
        let (out, used) = lzhuf(&data, 300);
        assert_eq!((out.len(), used), (300, 64));
        assert_eq!(
            out[..40],
            [
                127, 164, 201, 238, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32,
                32, 149, 0, 32, 178, 22, 3, 36, 184, 38, 44, 126, 208, 32, 32
            ]
        );
    }

    #[test]
    fn a_bad_table() {
        assert_eq!(entries(&[1, 0]), Err(Error::Layout(1)));
        assert_eq!(entries(&[0, 0, 0, 0, 0, 0, 0]), Ok(Vec::new()));
    }
}
