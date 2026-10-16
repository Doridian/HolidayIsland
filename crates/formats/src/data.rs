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

/// Stores a `Longint` into a `Byte`: the low 8 bits.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn byte(n: i32) -> u8 {
    n as u8
}

/// A text file of fixed-width integer columns, read the way most loaders do:
/// `if not Eof(f) then Readln(f, line)`, then for each cell
/// `StrToInt(Copy(line, 1, width)); Delete(line, 1, width)`.
struct Columns<'a> {
    f: TextReader<'a>,
    line: Vec<u8>,
    halt_on_eof: bool,
}

impl<'a> Columns<'a> {
    /// With `halt_on_eof`, a missing line halts the game. Without it the
    /// loader keeps going with what's left of the previous line, which is
    /// usually nothing and then fails in `StrToInt`.
    fn new(data: &'a [u8], halt_on_eof: bool) -> Self {
        Self {
            f: TextReader::new(data),
            line: Vec::new(),
            halt_on_eof,
        }
    }

    fn next_line(&mut self) -> Result<()> {
        if !self.f.eof() {
            self.line = self.f.read_line();
        } else if self.halt_on_eof {
            return Err(Error::UnexpectedEof { line: self.f.line() });
        }
        Ok(())
    }

    fn cell(&mut self, width: usize) -> Result<i32> {
        let n = width.min(self.line.len());
        let v = str_to_int(&self.line[..n])?;
        self.line.drain(..n);
        Ok(v)
    }

    /// The next line's first `N` cells.
    fn row<const N: usize>(&mut self, width: usize) -> Result<[i32; N]> {
        self.next_line()?;
        let mut row = [0; N];
        for cell in &mut row {
            *cell = self.cell(width)?;
        }
        Ok(row)
    }
}

/// `rows` lines of `COLS` 5-character columns, stored as `Integer`.
fn fixed_columns<const COLS: usize>(data: &[u8], rows: usize, halt_on_eof: bool) -> Result<Vec<[i16; COLS]>> {
    let mut c = Columns::new(data, halt_on_eof);
    (0..rows).map(|_| Ok(c.row::<COLS>(5)?.map(integer))).collect()
}

/// `rows` lines of `COLS` columns of `width` characters, stored as `Byte`.
fn byte_columns<const COLS: usize>(data: &[u8], rows: usize, width: usize) -> Result<Vec<[u8; COLS]>> {
    let mut c = Columns::new(data, true);
    (0..rows).map(|_| Ok(c.row::<COLS>(width)?.map(byte))).collect()
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

/// `s := line` into a `string[max]`: longer strings are cut.
fn short_string(mut line: Vec<u8>, max: usize) -> Vec<u8> {
    line.truncate(max);
    line
}

/// `FONTDAT1.DAT` and `FONTDAT2.DAT`: 32 rows of 8 `Byte`s, 5 characters
/// each. // 1030:2972, into 1110:65da and 1110:65de
///
/// # Errors
/// If the file is short or a cell isn't a number.
pub fn fontdat(data: &[u8]) -> Result<Vec<[u8; 8]>> {
    byte_columns(data, 32, 5)
}

/// `ANIMDAT1.DAT`: 683 rows of 11 columns. // 1088:80a7, into 1110:6fea
///
/// # Errors
/// If the file is short or a cell isn't a number.
pub fn animdat1(data: &[u8]) -> Result<Vec<[i16; 11]>> {
    fixed_columns(data, 683, true)
}

/// Groups in `ANIMDAT2.DAT`, and the most rows one group has room for.
const ANIMDAT2_GROUPS: usize = 40;
const ANIMDAT2_ROWS: usize = 47;

/// `ANIMDAT2.DAT`: 40 groups of rows of 4 `Byte`s, 5 characters each. Each
/// group ends with the row whose fourth byte is 255 (-1), which is kept.
/// // 1088:80a7, into 1110:6fda
///
/// # Errors
/// If the file is short, a cell isn't a number, or a group is longer than
/// the 47 rows the original has room for.
pub fn animdat2(data: &[u8]) -> Result<Vec<Vec<[u8; 4]>>> {
    let mut c = Columns::new(data, true);
    let mut groups = Vec::with_capacity(ANIMDAT2_GROUPS);
    for _ in 0..ANIMDAT2_GROUPS {
        let mut group = Vec::new();
        loop {
            if group.len() == ANIMDAT2_ROWS {
                // The original would run into the next group's memory.
                return Err(Error::Missing("ANIMDAT2 group end"));
            }
            let row = c.row::<4>(5)?.map(byte);
            group.push(row);
            if row[3] == 0xff {
                break;
            }
        }
        groups.push(group);
    }
    Ok(groups)
}

/// `NEINTRAG.DAT`: 101 rows of 4 `Byte`s, 3 characters each.
/// // 1088:80a7, into 1110:6fde
///
/// # Errors
/// If the file is short or a cell isn't a number.
pub fn neintrag(data: &[u8]) -> Result<Vec<[u8; 4]>> {
    byte_columns(data, 101, 3)
}

/// Groups in `ANIMDAT3.DAT`, and the most rows one group has room for.
pub const ANIMDAT3_GROUPS: usize = 16;
const ANIMDAT3_ROWS: usize = 31;

/// `ANIMDAT3.DAT`: 16 groups (`-schiene N` in the file's notes: track
/// pieces) of rows of 3 columns. Group `i` has `last_row[i] + 1` rows, or
/// none if `last_row[i]` is negative. // 1088:80a7, into 1110:6fe2
///
/// The original takes `last_row` from the first word of each 4-byte entry at
/// 1110:3620, which is uninitialized data. What fills it before the loader
/// runs isn't known yet.
///
/// # Errors
/// If the file is short, a cell isn't a number, or a group is longer than
/// the 31 rows the original has room for.
pub fn animdat3(data: &[u8], last_row: &[i16; ANIMDAT3_GROUPS]) -> Result<Vec<Vec<[i16; 3]>>> {
    let mut c = Columns::new(data, true);
    let mut groups = Vec::with_capacity(ANIMDAT3_GROUPS);
    for &last in last_row {
        let rows = usize::try_from(i32::from(last) + 1).unwrap_or(0);
        if rows > ANIMDAT3_ROWS {
            return Err(Error::Missing("room for ANIMDAT3 group"));
        }
        let group = (0..rows)
            .map(|_| Ok(c.row::<3>(5)?.map(integer)))
            .collect::<Result<_>>()?;
        groups.push(group);
    }
    Ok(groups)
}

/// `ANIMDAT4.DAT`: 8 groups of 16 rows of 3 columns. // 1088:80a7, into 1110:6fc6
///
/// # Errors
/// If the file is short or a cell isn't a number.
pub fn animdat4(data: &[u8]) -> Result<Vec<Vec<[i16; 3]>>> {
    let mut c = Columns::new(data, true);
    (0..8)
        .map(|_| (0..16).map(|_| Ok(c.row::<3>(5)?.map(integer))).collect())
        .collect()
}

/// Number of messages: entries in `NACHRICH.TXT` and the media name files.
pub const MESSAGES: usize = 90;

/// `NACHRICH.TXT`: a title (`string[30]`), then three lines of text per
/// message. // 1048:1ffd, into 1110:6634 and 1110:663e
#[must_use]
pub fn nachrich(data: &[u8]) -> (Vec<u8>, Vec<[Vec<u8>; 3]>) {
    let mut f = TextReader::new(data);
    let title = short_string(f.read_line(), 30);
    let messages = (0..MESSAGES)
        .map(|_| [f.read_line(), f.read_line(), f.read_line()])
        .collect();
    (title, messages)
}

/// `FILENAME.DAT`, `FILMNAME.DAT` and `SOUNNAME.DAT`: per message, the
/// bitmap, video and sound it shows or plays, as `string[12]`.
/// // 1048:1ffd, into the record array at 1110:6630
#[must_use]
pub fn message_media_names(data: &[u8]) -> Strings {
    let mut f = TextReader::new(data);
    (0..MESSAGES).map(|_| short_string(f.read_line(), 12)).collect()
}

/// `SABOTEXT.TXT`: six lines, of which the game keeps the 2nd, 4th and 6th.
/// // 1048:375e, into 1110:7356, 1110:735a, 1110:735e
#[must_use]
pub fn sabotext(data: &[u8]) -> [Vec<u8>; 3] {
    let mut f = TextReader::new(data);
    [(); 3].map(|()| {
        f.readln();
        f.read_line()
    })
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
