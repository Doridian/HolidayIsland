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

/// `EREIGNIS.DAT`: 41 events, 12 `Longint` columns of 10 characters each, then
/// a name the game ignores. The original's records have room for a 13th
/// value that the game fills later. // 1010:3ab4, into 1110:6536
///
/// `EREIGN1.DAT` to `EREIGN6.DAT` have the same format: starting scenario
/// *n* reloads the table from `EREIGNn.DAT` (1010:36f8, called from 1000:2466).
///
/// # Errors
/// If the file is short or a cell isn't a number.
pub fn ereignis(data: &[u8]) -> Result<Vec<[i32; 12]>> {
    let mut c = Columns::new(data, true);
    (0..41).map(|_| c.row::<12>(10)).collect()
}

/// `ABRECHNE.DAT`: the accounting constants. // 1008:55ee
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Abrechnung {
    /// 38 rows of four values, each under a heading line naming the factor
    /// (`Sterne der Haeuser und Hotels: Verfall`, ...). Into the four arrays
    /// at 1110:4c6e, 1110:4c72, 1110:4c76, 1110:4c7a.
    pub factors: Vec<[i32; 4]>,
    /// `MinMiete`, rent per head per month. // 1110:4a8a
    pub min_rent: i32,
    /// `MaxMiete`. // 1110:4a8e
    pub max_rent: i32,
    /// `MinPreis: array[TKommerzsachen]`, price per customer per day in
    /// Groschen. The file has 11 values; the game reads the first 10.
    /// // 1110:4a96
    pub min_price: [i32; 10],
    /// `MaxPreis: array[TKommerzsachen]`. // 1110:4ac2
    pub max_price: [i32; 10],
}

impl Abrechnung {
    /// `MaxMiete - MinMiete`, which the loader stores too. // 1110:4a92
    #[must_use]
    pub fn rent_range(&self) -> i32 {
        self.max_rent.wrapping_sub(self.min_rent)
    }
}

/// Parses `ABRECHNE.DAT`. // 1008:55ee
///
/// # Errors
/// If a number is malformed.
pub fn abrechne(data: &[u8]) -> Result<Abrechnung> {
    let mut f = TextReader::new(data);
    let mut factors = Vec::with_capacity(38);
    for _ in 0..38 {
        f.readln();
        let mut row = [0; 4];
        for cell in &mut row {
            *cell = f.read_longint()?;
        }
        f.readln();
        factors.push(row);
    }
    f.readln();
    let min_rent = f.read_longint()?;
    f.readln();
    f.readln();
    let max_rent = f.read_longint()?;
    f.readln();
    f.readln();
    let mut min_price = [0; 10];
    for cell in &mut min_price {
        *cell = f.read_longint()?;
    }
    f.readln();
    f.readln();
    let mut max_price = [0; 10];
    for cell in &mut max_price {
        *cell = f.read_longint()?;
    }
    Ok(Abrechnung {
        factors,
        min_rent,
        max_rent,
        min_price,
        max_price,
    })
}

/// `COMPBAU.DAT`: 13 rows of 7 `Byte`s, read as one stream of numbers
/// without regard to lines; negative numbers become 255.
/// // 10a8:37da, into 1110:70ee
///
/// # Errors
/// If a number is malformed.
pub fn compbau(data: &[u8]) -> Result<Vec<[u8; 7]>> {
    let mut f = TextReader::new(data);
    (0..13)
        .map(|_| {
            let mut row = [0; 7];
            for cell in &mut row {
                let n = f.read_longint()?;
                *cell = if n < 0 { 0xff } else { byte(n) };
            }
            Ok(row)
        })
        .collect()
}

/// A `TMapersteumrechnung`: 5 rows of 5 `Integer`s.
pub type MapersteTable = [[i16; 5]; 5];

/// `MAPERSTE.DAT`: 11 tables for generating the first map (`IGMinLeere`,
/// `IGMinRadius`, ...), each after a heading line.
/// // 10b0:56ce with 10b0:5653, into 1110:71f2 to 1110:721a
///
/// # Errors
/// If a number is malformed.
pub fn maperste(data: &[u8]) -> Result<Vec<MapersteTable>> {
    let mut f = TextReader::new(data);
    f.readln();
    (0..11)
        .map(|_| {
            let mut table = [[0; 5]; 5];
            for row in &mut table {
                for cell in row.iter_mut() {
                    *cell = integer(f.read_longint()?);
                }
                f.readln();
            }
            f.readln();
            Ok(table)
        })
        .collect()
}

/// `MONATE.TXT`: the 12 month names, as `string[15]`. // 1030:000d
///
/// # Errors
/// If the file has fewer than 12 lines.
pub fn monate(data: &[u8]) -> Result<Strings> {
    let mut f = TextReader::new(data);
    (0..12)
        .map(|_| {
            if f.eof() {
                return Err(Error::UnexpectedEof { line: f.line() });
            }
            Ok(short_string(f.read_line(), 15))
        })
        .collect()
}

/// The country setting at the top of `MODATYP.DAT`. // 1110:64be
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Land {
    /// `D`, and anything else the game doesn't know.
    Deutschland = 0,
    /// `A`
    Amerika = 1,
    /// `E`
    England = 2,
    /// `F`
    Frankreich = 3,
}

/// `MODATYP.DAT`: game constants. The file labels each value; the labels
/// are in the doc comments. // 1010:000e, into 1110:64be to 1110:6510
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Modatyp {
    /// `Land`. // 1110:64be
    pub land: Land,
    /// `MaxLageeinfluss`: radius of an event's effect on the location quality map. // 1110:64bf
    pub max_lageeinfluss: u8,
    /// `MaxGrueneinfluss`. // 1110:64c0
    pub max_grueneinfluss: i16,
    /// `Renovierungskosten`. // 1110:64c2
    pub renovierungskosten: i32,
    /// `Werbungskosten`. // 1110:64c6
    pub werbungskosten: i32,
    /// `Kreditgroesse`. // 1110:64ca
    pub kreditgroesse: i32,
    /// `MaxSchulden`. // 1110:64ce
    pub max_schulden: i32,
    /// `Zinssatz in Prozent im Jahr`. // 1110:64d2
    pub zinssatz: i32,
    /// `Startgeld`. // 1110:64d6
    pub startgeld: i32,
    /// `LandausWasserhebekosten`. // 1110:64da
    pub land_aus_wasser_hebekosten: i32,
    /// `Landhebekosten`. // 1110:64de
    pub landhebekosten: i32,
    /// `LandinWassersenkkosten`. // 1110:64e2
    pub land_in_wasser_senkkosten: i32,
    /// `Landsenkkosten`. // 1110:64e6
    pub landsenkkosten: i32,
    /// `Strassenabrisskosten`. // 1110:64ea
    pub strassenabrisskosten: i32,
    /// `Strassenbaukosten`. // 1110:64ee
    pub strassenbaukosten: i32,
    /// `Palmenabrisskosten`. // 1110:64f2
    pub palmenabrisskosten: i32,
    /// `Fundamentbaukosten`. // 1110:64f6
    pub fundamentbaukosten: i32,
    /// `Lebenshaltungskosten im Monat`. // 1110:64fa
    pub lebenshaltungskosten: i32,
    /// `Klassenmischungdivisor`. // 1110:64fe
    pub klassenmischungdivisor: i32,
    /// `Klassenmischung: array[0..BesteKlasse, 0..BesteKlasse] of Integer`. // 1110:6502
    pub klassenmischung: [[i16; 10]; 10],
    /// `Angeschlossendivisor`. // 1110:6506
    pub angeschlossendivisor: i16,
    /// `Angeschlossenumrechnung: array[-1..AbwoistStrassezuweitweg-1] of Byte`. // 1110:6508
    pub angeschlossenumrechnung: [u8; 6],
    /// `Verfallwahrscheinlichkeit: array[GAASchlaf..GAAKommerz2] of Byte`, in percent. // 1110:650e
    pub verfallwahrscheinlichkeit: [u8; 3],
}

/// Parses `MODATYP.DAT`. // 1010:000e
///
/// # Errors
/// If a number is malformed.
pub fn modatyp(data: &[u8]) -> Result<Modatyp> {
    let mut f = TextReader::new(data);
    // Each value sits on the line after its label.
    let value = |f: &mut TextReader| -> Result<i32> {
        f.readln();
        let v = f.read_longint()?;
        f.readln();
        Ok(v)
    };
    f.readln();
    let land = match f.read_char() {
        b'A' => Land::Amerika,
        b'E' => Land::England,
        b'F' => Land::Frankreich,
        _ => Land::Deutschland,
    };
    f.readln();
    let max_lageeinfluss = byte(value(&mut f)?);
    let max_grueneinfluss = integer(value(&mut f)?);
    let mut longs = [0; 16];
    for v in &mut longs {
        *v = value(&mut f)?;
    }
    f.readln();
    let mut klassenmischung = [[0; 10]; 10];
    for row in &mut klassenmischung {
        for cell in row.iter_mut() {
            *cell = integer(f.read_longint()?);
        }
        f.readln();
    }
    let angeschlossendivisor = integer(value(&mut f)?);
    f.readln();
    let mut angeschlossenumrechnung = [0; 6];
    for cell in &mut angeschlossenumrechnung {
        *cell = byte(f.read_longint()?);
    }
    f.readln();
    f.readln();
    let mut verfallwahrscheinlichkeit = [0; 3];
    for cell in &mut verfallwahrscheinlichkeit {
        *cell = byte(f.read_longint()?);
    }
    let [
        renovierungskosten,
        werbungskosten,
        kreditgroesse,
        max_schulden,
        zinssatz,
        startgeld,
        land_aus_wasser_hebekosten,
        landhebekosten,
        land_in_wasser_senkkosten,
        landsenkkosten,
        strassenabrisskosten,
        strassenbaukosten,
        palmenabrisskosten,
        fundamentbaukosten,
        lebenshaltungskosten,
        klassenmischungdivisor,
    ] = longs;
    Ok(Modatyp {
        land,
        max_lageeinfluss,
        max_grueneinfluss,
        renovierungskosten,
        werbungskosten,
        kreditgroesse,
        max_schulden,
        zinssatz,
        startgeld,
        land_aus_wasser_hebekosten,
        landhebekosten,
        land_in_wasser_senkkosten,
        landsenkkosten,
        strassenabrisskosten,
        strassenbaukosten,
        palmenabrisskosten,
        fundamentbaukosten,
        lebenshaltungskosten,
        klassenmischungdivisor,
        klassenmischung,
        angeschlossendivisor,
        angeschlossenumrechnung,
        verfallwahrscheinlichkeit,
    })
}

/// `MODATYP.TXT`: `WerbungToText: array[0..7] of string[20]` (advertising
/// levels) and `RenovierungToText: array[0..5] of string[20]` (renovation
/// intervals), each after a heading. // 1010:0414, into 1110:64b0 and 1110:64b4
#[must_use]
pub fn modatyp_txt(data: &[u8]) -> (Strings, Strings) {
    let mut f = TextReader::new(data);
    f.readln();
    let first = (0..8).map(|_| f.read_line_max(20)).collect();
    f.readln();
    let second = (0..6).map(|_| f.read_line_max(20)).collect();
    (first, second)
}

/// How many names the original's stack array in [`namen`] holds.
const NAMEN_MAX: usize = 51;

/// `NAMEN.TXT`: the name for no player (`keiner`, stored at 1110:0902), the
/// human player's default name (`Spieler1`, 1110:0a02), then a list of
/// `string[12]` names up to an empty line, from which the game picks the
/// computer players' names at random. // 1010:0414
///
/// # Errors
/// If the list is longer than the original has room for (51).
pub fn namen(data: &[u8]) -> Result<(Vec<u8>, Vec<u8>, Strings)> {
    let mut f = TextReader::new(data);
    let first = f.read_line();
    let second = f.read_line();
    let mut names = Vec::new();
    loop {
        let line = f.read_line();
        if line.is_empty() {
            break;
        }
        if names.len() == NAMEN_MAX {
            return Err(Error::Missing("empty line after the names"));
        }
        names.push(short_string(line, 12));
    }
    Ok((first, second, names))
}

/// A screen rectangle as the layout tables store it: one `Integer` per
/// `TFormRaender` (form edge), left, top, right, bottom.
pub type Rect = [i16; 4];

/// Reads the next line (halting at the end) and drops it: the loaders do
/// this for heading lines.
fn skip_heading(c: &mut Columns) -> Result<()> {
    c.next_line()
}

/// `n` `Readln(f, s)` with the halt at the end of the file.
fn lines(c: &mut Columns, n: usize) -> Result<Strings> {
    (0..n)
        .map(|_| {
            c.next_line()?;
            Ok(std::mem::take(&mut c.line))
        })
        .collect()
}

/// `BAUMENU.TXT`: the build menu's texts. Read by a form method at 1018:080f.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaumenuTexts {
    /// The 12 submenu headings (`HÄUSER`, `HOTELS`, ...), indices 9 to 20
    /// in the original. // Self+0x1f5
    pub headings: Strings,
    /// 10 other texts (`Preis:`, ...). // Self+0x1f1
    pub texts: Strings,
}

/// Parses `BAUMENU.TXT`. // 1018:080f
///
/// # Errors
/// If the file is short.
pub fn baumenu_txt(data: &[u8]) -> Result<BaumenuTexts> {
    let mut c = Columns::new(data, true);
    skip_heading(&mut c)?;
    let headings = lines(&mut c, 12)?;
    skip_heading(&mut c)?;
    let texts = lines(&mut c, 10)?;
    Ok(BaumenuTexts { headings, texts })
}

/// One of the build menu's 12 submenus in `BAUMENU.DAT`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaumenuEntries {
    /// 20 rows of 4 numbers; the first is a building type, or -1. // Self+0x184
    pub rows: [[i16; 4]; 20],
    /// How many rows don't start with -1, as the loader counts them. // Self+0x180
    pub count: u8,
}

/// `BAUMENU.DAT`: the build menu's contents and layout. // 1018:080f
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaumenuData {
    /// The 12 submenus, indices 9 to 20 in the original.
    pub menus: Vec<BaumenuEntries>,
    /// Two layouts of 32 rectangles. // Self+0x1fa
    pub layouts: [Vec<Rect>; 2],
}

/// Parses `BAUMENU.DAT`. // 1018:080f
///
/// # Errors
/// If the file is short or a number is malformed.
pub fn baumenu_dat(data: &[u8]) -> Result<BaumenuData> {
    let mut c = Columns::new(data, true);
    let mut menus = Vec::with_capacity(12);
    for _ in 0..12 {
        skip_heading(&mut c)?;
        let mut rows = [[0; 4]; 20];
        let mut count = 0u8;
        for row in &mut rows {
            for cell in row.iter_mut() {
                *cell = integer(c.f.read_longint()?);
            }
            if row[0] != -1 {
                count = count.wrapping_add(1);
            }
            c.f.readln();
        }
        menus.push(BaumenuEntries { rows, count });
    }
    skip_heading(&mut c)?;
    let mut layout = || {
        (0..32)
            .map(|_| Ok(c.row::<4>(5)?.map(integer)))
            .collect::<Result<Vec<_>>>()
    };
    let layouts = [layout()?, layout()?];
    Ok(BaumenuData { menus, layouts })
}

/// `GELDMENU.TXT`: a heading, 25 texts (indices 28 to 52 in the original,
/// Self+0x189), and one more text (Self+0x18d). // 1070:2bfa
///
/// # Errors
/// If the file is short.
pub fn geldmenu_txt(data: &[u8]) -> Result<(Strings, Vec<u8>)> {
    let mut c = Columns::new(data, true);
    skip_heading(&mut c)?;
    let texts = lines(&mut c, 25)?;
    // The last line is read without the end-of-file check.
    let last = c.f.read_line();
    Ok((texts, last))
}

/// `GELDMENU.DAT`: `Lage: array[TGrundDing, TFormRaender] of Integer`, 118
/// rectangles. Each one is read as `Readln(f); Read(f, a, b, c, d)`, so the
/// first `Readln` skips the heading and the later ones end the previous
/// row's line. // 1070:2bfa, into Self+0x692
///
/// # Errors
/// If the file is short or a number is malformed.
pub fn geldmenu_dat(data: &[u8]) -> Result<Vec<Rect>> {
    let mut f = TextReader::new(data);
    (0..118)
        .map(|_| {
            if f.eof() {
                return Err(Error::UnexpectedEof { line: f.line() });
            }
            f.readln();
            let mut rect = [0; 4];
            for cell in &mut rect {
                *cell = integer(f.read_longint()?);
            }
            Ok(rect)
        })
        .collect()
}

/// `UEBERMAP.TXT`: a heading and the overview map's title. // 10b8:41ad, into Self+0x1ac
///
/// # Errors
/// If the file is short.
pub fn uebermap_txt(data: &[u8]) -> Result<Vec<u8>> {
    let mut c = Columns::new(data, true);
    skip_heading(&mut c)?;
    Ok(lines(&mut c, 1)?.remove(0))
}

/// `UEBERMAP.DAT`: the overview map's layout. // 10b8:41ad
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UebermapData {
    /// `PressedLage: array[UEBesitzermapbutton..UESchliessenbutton,
    /// TFormraender]`: 6 buttons (indices 16 to 21 in the original). // Self+0x1a8
    pub pressed: Vec<Rect>,
    /// Two layouts of 22 rectangles. // Self+0x20e
    pub layouts: [Vec<Rect>; 2],
}

/// Parses `UEBERMAP.DAT`. // 10b8:41ad
///
/// # Errors
/// If the file is short or a number is malformed.
pub fn uebermap_dat(data: &[u8]) -> Result<UebermapData> {
    let mut c = Columns::new(data, true);
    skip_heading(&mut c)?;
    let pressed = (0..6).map(|_| Ok(c.row::<4>(5)?.map(integer))).collect::<Result<_>>()?;
    skip_heading(&mut c)?;
    let mut layout = || {
        (0..22)
            .map(|_| Ok(c.row::<4>(5)?.map(integer)))
            .collect::<Result<Vec<_>>>()
    };
    let layouts = [layout()?, layout()?];
    Ok(UebermapData { pressed, layouts })
}

/// `SONSTIGE.TXT`: miscellaneous texts. // 1020:3dc4
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sonstige {
    /// `Waehrungsfaktor`, the currency factor. // 1110:7346
    pub waehrungsfaktor: i32,
    /// 24 labelled texts (`_TXTSpielname_`, `_TXTExitmenuueberschrift_`,
    /// ...); the labels are skipped. // 1110:734e
    pub texts: Strings,
    /// 35 `string[30]`s after a heading. // 1110:737a
    pub short_texts: Strings,
}

/// Parses `SONSTIGE.TXT`. // 1020:3dc4
///
/// # Errors
/// If the currency factor is malformed.
pub fn sonstige(data: &[u8]) -> Result<Sonstige> {
    let mut f = TextReader::new(data);
    f.readln();
    let waehrungsfaktor = f.read_longint()?;
    f.readln();
    let texts = (0..24)
        .map(|_| {
            f.readln();
            f.read_line()
        })
        .collect();
    f.readln();
    let short_texts = (0..35).map(|_| f.read_line_max(30)).collect();
    Ok(Sonstige {
        waehrungsfaktor,
        texts,
        short_texts,
    })
}

/// `AUKTION.TXT`: 12 `string[30]`s for the auction. // 1028:2449, into 1110:65b0
#[must_use]
pub fn auktion(data: &[u8]) -> Strings {
    let mut f = TextReader::new(data);
    (0..12).map(|_| short_string(f.read_line(), 30)).collect()
}

/// One kind of sabotage in `SABOTAGE.TXT` (a 0x232-byte record).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sabotage {
    /// `string[20]`. // +0x00
    pub name: Vec<u8>,
    /// The picture, `string[12]`. // +0x15
    pub bitmap: Vec<u8>,
    /// // +0x22
    pub description: Vec<u8>,
    /// What it can be used on, as text. // +0x122
    pub targets_text: Vec<u8>,
    /// // +0x222
    pub cost: i32,
    /// Six building types it can be used on, 255 for none. // +0x226
    pub targets: [i16; 6],
}

/// `SABOTAGE.TXT`: two labels (`Anwendbar auf:`, `Kosten:`, `string[20]`,
/// into 1110:65d2) and 9 kinds of sabotage (1110:65bc). The same routine
/// then loads `SABOTEXT.TXT` ([`sabotext`]). // 1028:361f
///
/// # Errors
/// If the file is short or a number is malformed.
pub fn sabotage(data: &[u8]) -> Result<([Vec<u8>; 2], Vec<Sabotage>)> {
    let mut c = Columns::new(data, true);
    let labels = [short_string(c.f.read_line(), 20), short_string(c.f.read_line(), 20)];
    let kinds = (0..9)
        .map(|_| {
            if c.f.eof() {
                return Err(Error::UnexpectedEof { line: c.f.line() });
            }
            let name = short_string(c.f.read_line(), 20);
            let bitmap = short_string(c.f.read_line(), 12);
            let description = c.f.read_line();
            let targets_text = c.f.read_line();
            let cost = str_to_int(&c.f.read_line())?;
            c.line = c.f.read_line();
            let mut targets = [0; 6];
            for cell in &mut targets {
                *cell = integer(c.cell(5)?);
            }
            Ok(Sabotage {
                name,
                bitmap,
                description,
                targets_text,
                cost,
                targets,
            })
        })
        .collect::<Result<_>>()?;
    Ok((labels, kinds))
}

/// `UMENU.TXT`: 11 texts. // 1080:1141, into 1110:6fba
#[must_use]
pub fn umenu(data: &[u8]) -> Strings {
    let mut f = TextReader::new(data);
    (0..11).map(|_| f.read_line()).collect()
}

/// `WASNUN.TXT`: 4 texts. Read by a form method at 1040:34af into Self+0x282.
#[must_use]
pub fn wasnun(data: &[u8]) -> Strings {
    let mut f = TextReader::new(data);
    (0..4).map(|_| f.read_line()).collect()
}

/// `if not Eof(f) then Readln(f) else Halt`, for heading lines.
fn heading(f: &mut TextReader) -> Result<()> {
    if f.eof() {
        return Err(Error::UnexpectedEof { line: f.line() });
    }
    f.readln();
    Ok(())
}

/// `N` `Read(f, n)` in a row.
fn read_longints<const N: usize>(f: &mut TextReader) -> Result<[i32; N]> {
    let mut row = [0; N];
    for cell in &mut row {
        *cell = f.read_longint()?;
    }
    Ok(row)
}

/// One line of `N` numbers read with `Read`, then `Readln`.
fn read_row<const N: usize>(f: &mut TextReader) -> Result<[i32; N]> {
    let row = read_longints(f)?;
    f.readln();
    Ok(row)
}

/// Views of the island (`TAnsichtart`): the four rotations.
pub const VIEWS: usize = 4;

/// `DARSTELL.DAT`: drawing corrections per view, and pointer positions. Each
/// table follows a heading line with its Pascal declaration, whose names
/// the fields keep. // 1050:271b
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Darstell {
    /// `AnsichtFormkorrektur: array[TAnsichtart, 0..18] of Byte`. // 1110:667e
    pub form_korrektur: [[u8; 19]; VIEWS],
    /// `AnsichtFlusskorrektur: array[TAnsichtart, 0..GfxPosFluesseAnzahl-1] of Byte`. // 1110:6682
    pub fluss_korrektur: [[u8; 40]; VIEWS],
    /// `AnsichtStrassenkorrektur: array[TAnsichtart, 0..GfxPosStrasseAnzahl-1] of Byte`. // 1110:6686
    pub strassen_korrektur: [[u8; 45]; VIEWS],
    /// `AnsichtExtratabxkorrektur: array[TAnsichtart, 0..1] of Integer`. // 1110:668a
    pub extratab_x_korrektur: [[i16; 2]; VIEWS],
    /// `AnsichtExtratabykorrektur: array[TAnsichtart, 0..1] of Integer`. // 1110:668e
    pub extratab_y_korrektur: [[i16; 2]; VIEWS],
    /// `AnsichtSpaltenxKorrektur: array[TAnsichtart, 0..1, 0..19] of Integer`. // 1110:6692
    pub spalten_x_korrektur: [[[i16; 20]; 2]; VIEWS],
    /// `AnsichtZeilenxKorrektur: array[TAnsichtart, 0..1, 0..Halbblockanzahly] of Integer`. // 1110:6696
    pub zeilen_x_korrektur: [[[i16; 27]; 2]; VIEWS],
    /// `AnsichtSpaltenyKorrektur: array[TAnsichtart, 0..1, 0..19] of Integer`. // 1110:669a
    pub spalten_y_korrektur: [[[i16; 20]; 2]; VIEWS],
    /// `AnsichtZeilenyKorrektur: array[TAnsichtart, 0..1, 0..Halbblockanzahly] of Integer`. // 1110:669e
    pub zeilen_y_korrektur: [[[i16; 27]; 2]; VIEWS],
    /// `gfxAuslastungszeigerpos: array[1..3, 0..10, Tgfxhausposindex] of Integer`. // 1110:66c2
    pub auslastungszeiger_pos: [[[i16; 12]; 11]; 3],
    /// `gfxspielerzeigerpos: array[1..3, 1..MaxSpielerzahl, Tgfxhausposindex] of Integer`. // 1110:66c6
    pub spielerzeiger_pos: [[[i16; 12]; 8]; 3],
}

/// Parses `DARSTELL.DAT`. // 1050:271b
///
/// # Errors
/// If a heading is missing or a number is malformed.
pub fn darstell(data: &[u8]) -> Result<Darstell> {
    fn bytes<const N: usize>(f: &mut TextReader) -> Result<[[u8; N]; VIEWS]> {
        heading(f)?;
        let mut t = [[0; N]; VIEWS];
        for row in &mut t {
            *row = read_row::<N>(f)?.map(byte);
        }
        Ok(t)
    }
    // Two numbers per view, all four views read as one stream.
    fn pairs(f: &mut TextReader) -> Result<[[i16; 2]; VIEWS]> {
        heading(f)?;
        let mut t = [[0; 2]; VIEWS];
        for row in &mut t {
            *row = read_longints::<2>(f)?.map(integer);
        }
        f.readln();
        Ok(t)
    }
    fn grid<const N: usize, const M: usize, const O: usize>(f: &mut TextReader) -> Result<[[[i16; N]; M]; O]> {
        heading(f)?;
        let mut t = [[[0; N]; M]; O];
        for rows in &mut t {
            for row in rows.iter_mut() {
                *row = read_row::<N>(f)?.map(integer);
            }
        }
        Ok(t)
    }
    let f = &mut TextReader::new(data);
    Ok(Darstell {
        form_korrektur: bytes(f)?,
        fluss_korrektur: bytes(f)?,
        strassen_korrektur: bytes(f)?,
        extratab_x_korrektur: pairs(f)?,
        extratab_y_korrektur: pairs(f)?,
        spalten_x_korrektur: grid(f)?,
        zeilen_x_korrektur: grid(f)?,
        spalten_y_korrektur: grid(f)?,
        zeilen_y_korrektur: grid(f)?,
        auslastungszeiger_pos: grid(f)?,
        spielerzeiger_pos: grid(f)?,
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
