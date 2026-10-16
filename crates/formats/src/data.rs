//! The game's tables in `DATA\*.DAT` and `TEXTE\*.TXT`.
//!
//! Each loader follows the original routine that reads the file (address in
//! its comment), including how many rows it takes: the files often carry
//! notes after the rows the game reads, which these loaders ignore too. Columns
//! whose meaning isn't known yet stay as raw arrays.

use crate::pascal::{Error, Result, TextReader, str_to_int};

/// A list of strings from a `TEXTE` file.
pub type Strings = Vec<Vec<u8>>;

/// Stores a `Longint` into an `Integer` the way the original does: without a
/// range check, keeping the low 16 bits.
#[allow(clippy::cast_possible_truncation)]
fn integer(n: i32) -> i16 {
    n as i16
}

/// Reads `rows` lines of `COLS` fixed-width integer columns, 5 characters each
/// (`StrToInt(Copy(line, 1, 5)); Delete(line, 1, 5)`), stored as `Integer`.
///
/// With `halt_on_eof`, a missing line halts the game. Without it the loader
/// keeps going with what's left of the previous line, which is usually nothing
/// and then fails in `StrToInt`.
fn fixed_columns<const COLS: usize>(data: &[u8], rows: usize, halt_on_eof: bool) -> Result<Vec<[i16; COLS]>> {
    const WIDTH: usize = 5;
    let mut f = TextReader::new(data);
    let mut line = Vec::new();
    let mut table = Vec::with_capacity(rows);
    for _ in 0..rows {
        if !f.eof() {
            line = f.read_line();
        } else if halt_on_eof {
            return Err(Error::UnexpectedEof { line: f.line() });
        }
        let mut row = [0; COLS];
        for cell in &mut row {
            let n = WIDTH.min(line.len());
            *cell = integer(str_to_int(&line[..n])?);
            line.drain(..n);
        }
        table.push(row);
    }
    Ok(table)
}

/// `HAUSPOS.DAT`: 673 rows of 12 columns. // 1048:2dff, into 1110:54f6
///
/// # Errors
/// If a cell isn't a number.
pub fn hauspos(data: &[u8]) -> Result<Vec<[i16; 12]>> {
    fixed_columns(data, 673, false)
}

/// `HAUSNR.DAT`: 127 rows of 8 columns, one per building type. // 1048:2ed8, into 1110:54f2
///
/// # Errors
/// If the file is short or a cell isn't a number.
pub fn hausnr(data: &[u8]) -> Result<Vec<[i16; 8]>> {
    fixed_columns(data, 127, true)
}

/// `GROUNPOS.DAT`: 141 rows of 6 columns. // 1048:2fbd, into 1110:54e2
///
/// # Errors
/// If a cell isn't a number.
pub fn grounpos(data: &[u8]) -> Result<Vec<[i16; 6]>> {
    fixed_columns(data, 141, false)
}

/// `GROUNDNR.DAT`: 95 rows of 2 columns. // 1048:3098, into 1110:54e6
///
/// # Errors
/// If a cell isn't a number.
pub fn groundnr(data: &[u8]) -> Result<Vec<[i16; 2]>> {
    fixed_columns(data, 95, false)
}

/// `GFXPOS.DAT`: 133 rows of 6 columns. // 1048:3172, into 1110:54de
///
/// # Errors
/// If the file is short or a cell isn't a number.
pub fn gfxpos(data: &[u8]) -> Result<Vec<[i16; 6]>> {
    fixed_columns(data, 133, true)
}

/// `STRASPOS.DAT`: 45 rows of 6 columns. // 1048:3256, into 1110:54ea
///
/// # Errors
/// If the file is short or a cell isn't a number.
pub fn straspos(data: &[u8]) -> Result<Vec<[i16; 6]>> {
    fixed_columns(data, 45, true)
}

/// `FLUSSPOS.DAT`: 40 rows of 6 columns. // 1048:3339, into 1110:54ee
///
/// # Errors
/// If the file is short or a cell isn't a number.
pub fn flusspos(data: &[u8]) -> Result<Vec<[i16; 6]>> {
    fixed_columns(data, 40, true)
}

/// Number of building types: rows in `HAUSDATA.DAT`, `HAUSPREI.DAT`, `HAUSNR.DAT`.
pub const BUILDING_TYPES: usize = 127;

/// `HAUSPREI.DAT`: per building type, three `Longint`s and the building's name
/// (ignored). The original adds a fourth column, a copy of the third.
/// // 1048:341c, into 1110:736e
///
/// # Errors
/// If a number is malformed.
pub fn hausprei(data: &[u8]) -> Result<Vec<[i32; 4]>> {
    let mut f = TextReader::new(data);
    let mut table = Vec::with_capacity(BUILDING_TYPES);
    for _ in 0..BUILDING_TYPES {
        let mut row = [0; 4];
        for cell in &mut row[..3] {
            *cell = f.read_longint()?;
        }
        row[3] = row[2];
        f.readln();
        table.push(row);
    }
    Ok(table)
}

/// `HAUSDATA.DAT`: per building type, ten `Integer`s and the building's name
/// (ignored). // 1048:34ee, into 1110:7372
///
/// # Errors
/// If a number is malformed.
pub fn hausdata(data: &[u8]) -> Result<Vec<[i16; 10]>> {
    let mut f = TextReader::new(data);
    let mut table = Vec::with_capacity(BUILDING_TYPES);
    for _ in 0..BUILDING_TYPES {
        let mut row = [0; 10];
        for cell in &mut row {
            *cell = integer(f.read_longint()?);
        }
        f.readln();
        table.push(row);
    }
    Ok(table)
}

/// A line buffer that works like the original's `string` local: a line
/// shorter than three characters still has the previous lines' bytes behind
/// it, and the `***` check looks at those.
struct LineBuf([u8; 3]);

impl LineBuf {
    fn read(&mut self, f: &mut TextReader) -> Result<Vec<u8>> {
        if f.eof() {
            // The original would loop forever here.
            return Err(Error::Missing("*** end marker"));
        }
        let line = f.read_line();
        let n = line.len().min(3);
        self.0[..n].copy_from_slice(&line[..n]);
        Ok(line)
    }

    fn is_end(&self) -> bool {
        &self.0 == b"***"
    }
}

/// `HAUSNAME.TXT`: two lists of up to 127 names, each ended by a line
/// starting with `***`. // 1048:358f, into 1110:7366 and 1110:736a
///
/// # Errors
/// If an end marker is missing.
pub fn hausname(data: &[u8]) -> Result<(Strings, Strings)> {
    let mut f = TextReader::new(data);
    let mut buf = LineBuf([0; 3]);
    // The first list reads on to the marker and drops extra names.
    let mut first = Vec::new();
    loop {
        let line = buf.read(&mut f)?;
        if buf.is_end() {
            break;
        }
        if first.len() < BUILDING_TYPES {
            first.push(line);
        }
    }
    // The second list stops at the marker or after 127 names.
    let mut second = Vec::new();
    while second.len() < BUILDING_TYPES {
        let line = buf.read(&mut f)?;
        if buf.is_end() {
            break;
        }
        second.push(line);
    }
    Ok((first, second))
}

/// `AUSWAHL.TXT`: a heading line, then 8 strings. // 1048:36b2, into 1110:7362
#[must_use]
pub fn auswahl(data: &[u8]) -> Strings {
    let mut f = TextReader::new(data);
    f.readln();
    (0..8).map(|_| f.read_line()).collect()
}

/// `FEHLER.TXT`: 31 error messages, each line with a 3-character prefix that
/// the game drops. // 1048:387c, into 1110:734a
///
/// # Errors
/// If the file has fewer than 31 lines.
pub fn fehler(data: &[u8]) -> Result<Strings> {
    let mut f = TextReader::new(data);
    let mut table = Vec::with_capacity(31);
    for _ in 0..31 {
        if f.eof() {
            return Err(Error::UnexpectedEof { line: f.line() });
        }
        let mut line = f.read_line();
        line.drain(..line.len().min(3));
        table.push(line);
    }
    Ok(table)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns() {
        let t: Vec<[i16; 3]> = fixed_columns(b"    1   -2    3 note\r\n   10   20   30\r\n", 2, true).unwrap();
        assert_eq!(t, [[1, -2, 3], [10, 20, 30]]);
        assert!(fixed_columns::<3>(b"    1    2    3\r\n", 2, true).is_err());
        // Without the halt, the short file fails on the empty leftover line.
        assert!(fixed_columns::<3>(b"    1    2    3\r\n", 2, false).is_err());
        // ... unless the previous line had more columns than were read.
        let t: Vec<[i16; 1]> = fixed_columns(b"    1    2\r\n", 2, false).unwrap();
        assert_eq!(t, [[1], [2]]);
    }

    #[test]
    fn hausname_marker() {
        let (a, b) = hausname(b"one\r\n***\r\ntwo\r\n*** x\r\n").unwrap();
        assert_eq!(a, [b"one".to_vec()]);
        assert_eq!(b, [b"two".to_vec()]);
        // A short line after "***" still looks like the marker.
        let (a, b) = hausname(b"***\r\n*\r\n").unwrap();
        assert!(a.is_empty() && b.is_empty());
        assert!(hausname(b"one\r\n").is_err());
    }
}
