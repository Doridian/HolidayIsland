//! Maps, scenarios and saved games: `MAPS\*.MAP`, `SZENARIO\*.HIS` and the
//! player's `*.map` files (`Last.map` too) all have this format.
//!
//! The file is a dump of the game's state. `load_map` (1048:2a19) reads a
//! 6-byte version string (`1.000` as a `string[5]`, the later build's `1.100`, not checked), then calls
//! one routine per unit, each of which `BlockRead`s its globals one after the
//! other. `save_map` (1048:2bae) writes them back in the same order. There
//! are no tags or lengths, so the parser follows the same sequence of reads,
//! described by [`layout::LAYOUT`]. Most of the sizes are fixed; the rest
//! depend on counts read earlier in the file.
//!
//! Each read becomes a [`Field`] named after where the original puts the
//! bytes:
//! - `1110:0067`: the global at that address.
//! - `*1110:5500`: the memory block the far pointer at that address points to.
//! - `[1110:6516]+0x1ac`: a field of the object the global points to (a form).
//! - `*[1110:6516]+0x180`: the block a pointer in that field points to.
//!
//! The fields stay as bytes until the code that uses them is ported.
//! [`SaveGame::build`] goes the other way: it asks a [`Source`] for each
//! field in `save_map`'s order and works out the lists' counts itself.
//! `docs/savegame.md` describes the format and what each unit holds.

mod layout;

use std::fmt;

/// The version string the port writes at the start of a file: the later
/// build's `string[5]` `1.100` (en `1118:004e`). v1.0 writes
/// [`VERSION_V10`]; neither build checks it on loading, and the format
/// is the same (`docs/versions.md`).
pub const VERSION: [u8; 6] = *b"\x051.100";
/// v1.0's version string, `1.000` (`1110:004e`), also the scenarios'.
pub const VERSION_V10: [u8; 6] = *b"\x051.000";

/// Side length of the island's map layers: the 63001-byte blocks are
/// 251 × 251 bytes.
pub const MAP_SIZE: usize = 251;

/// The global holding the number of players, human included. // 1110:1479
const PLAYERS: &str = "1110:1479";

/// The most records a list holds: its count is a `Word`.
pub const MAX_RECORDS: usize = 0xffff;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The file ends inside a read.
    Truncated {
        routine: &'static str,
        dest: &'static str,
        offset: usize,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Truncated { routine, dest, offset } => {
                write!(f, "file ends at {offset:#x} while {routine} reads {dest}")
            }
        }
    }
}

impl std::error::Error for Error {}

/// One step of a unit routine's reads.
#[derive(Debug, Clone, Copy)]
enum Step {
    /// `BlockRead(f, dest, size)`.
    Read(usize, &'static str),
    /// A `Word` count at `count`, then that many records of `record` bytes.
    List {
        count: &'static str,
        record: usize,
        dest: &'static str,
    },
    /// The window positions of the 17 forms in `1110:646c`, except form 2:
    /// `Top`, `Left`, `Width`, `Height` and the bytes at `+0x29`
    /// (`Visible`) and `+0x2a` (`Enabled`). Forms 10 and 11 are saved as
    /// zeros. // 1010:08d7, 1010:0799
    Windows,
    /// Two blocks of three 256-byte records per computer player, players 2 to
    /// the player count at `1110:1479`. // 10a8:32ec, 1030:0eab
    PerComputerPlayer,
}

/// One `BlockRead`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Field {
    /// Where the original stores the bytes, in the notation of the module
    /// documentation.
    pub dest: &'static str,
    pub data: Vec<u8>,
}

/// What one unit routine reads.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Unit {
    /// The routine's address.
    pub routine: &'static str,
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SaveGame {
    /// The version string, as stored.
    pub version: Vec<u8>,
    pub units: Vec<Unit>,
    /// Bytes after the last read, which the game ignores.
    pub trailing: Vec<u8>,
}

/// Where [`SaveGame::build`] takes the fields from.
pub trait Source {
    /// The bytes of the `n`th field (from 0) stored at `dest`, which
    /// should be `size` long: shorter ones are padded with zeros, longer
    /// ones cut.
    fn field(&mut self, dest: &'static str, n: usize, size: usize) -> Vec<u8>;
    /// The records of the list stored at `dest`, in the file's order, at
    /// most [`MAX_RECORDS`]; each is padded or cut to the record's size.
    fn records(&mut self, dest: &'static str) -> Vec<Vec<u8>>;
}

/// What [`SaveGame::build`] writes, with each field's occurrence.
struct Writer<'a, S: Source> {
    source: &'a mut S,
    seen: Vec<(&'static str, usize)>,
    fields: Vec<Field>,
}

impl<S: Source> Writer<'_, S> {
    fn next(&mut self, dest: &'static str) -> usize {
        if let Some(e) = self.seen.iter_mut().find(|e| e.0 == dest) {
            e.1 += 1;
            e.1 - 1
        } else {
            self.seen.push((dest, 1));
            0
        }
    }

    fn push(&mut self, dest: &'static str, mut data: Vec<u8>, size: usize) -> &[u8] {
        data.resize(size, 0);
        self.fields.push(Field { dest, data });
        &self.fields.last().expect("just pushed").data
    }

    fn write(&mut self, size: usize, dest: &'static str) -> &[u8] {
        let n = self.next(dest);
        let data = self.source.field(dest, n, size);
        self.push(dest, data, size)
    }
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
    routine: &'static str,
    fields: Vec<Field>,
}

impl Reader<'_> {
    fn read(&mut self, size: usize, dest: &'static str) -> Result<&[u8], Error> {
        let end = self.pos + size;
        if end > self.data.len() {
            return Err(Error::Truncated {
                routine: self.routine,
                dest,
                offset: self.pos,
            });
        }
        self.fields.push(Field {
            dest,
            data: self.data[self.pos..end].to_vec(),
        });
        self.pos = end;
        Ok(&self.fields.last().expect("just pushed").data)
    }
}

impl SaveGame {
    /// Parses a map, scenario or saved game.
    ///
    /// # Errors
    /// If the file ends early.
    pub fn parse(data: &[u8]) -> Result<Self, Error> {
        let mut r = Reader {
            data,
            pos: 0,
            routine: "1048:2a19",
            fields: Vec::new(),
        };
        let version = r.read(VERSION.len(), "(version)")?.to_vec();
        r.fields.clear();
        let mut players = 0;
        let mut units = Vec::with_capacity(layout::LAYOUT.len());
        for &(routine, steps) in layout::LAYOUT {
            r.routine = routine;
            for &step in steps {
                match step {
                    Step::Read(size, dest) => {
                        let field = r.read(size, dest)?;
                        if dest == PLAYERS {
                            players = field[0];
                        }
                    }
                    Step::List { count, record, dest } => {
                        let n = r.read(2, count)?;
                        let n = u16::from_le_bytes([n[0], n[1]]);
                        for _ in 0..n {
                            r.read(record, dest)?;
                        }
                    }
                    Step::Windows => {
                        for form in 0..=16 {
                            if form == 2 {
                                continue;
                            }
                            // The setters are probably SetTop, SetLeft, SetWidth, SetHeight.
                            r.read(2, "[1110:646c[i]].Top")?;
                            r.read(2, "[1110:646c[i]].Left")?;
                            r.read(2, "[1110:646c[i]].Width")?;
                            r.read(2, "[1110:646c[i]].Height")?;
                            r.read(1, "[1110:646c[i]].Visible")?;
                            r.read(1, "[1110:646c[i]].Enabled")?;
                        }
                    }
                    Step::PerComputerPlayer => {
                        for _ in 2..=players {
                            for dest in ["*[1110:70e2[i]]", "*[1110:70e6[i]]"] {
                                for _ in 0..3 {
                                    r.read(0x100, dest)?;
                                }
                            }
                        }
                    }
                }
            }
            units.push(Unit {
                routine,
                fields: std::mem::take(&mut r.fields),
            });
        }
        let trailing = data[r.pos..].to_vec();
        Ok(SaveGame {
            version,
            units,
            trailing,
        })
    }

    /// A file with the fields `source` gives, in `save_map`'s order
    /// (1048:2bae): the version, then each unit's fields. A list's count
    /// is the number of records `source` has for it, and the computer
    /// players' lists are written for players 2 to the player count
    /// `source` gave before them.
    pub fn build(source: &mut impl Source) -> SaveGame {
        let mut w = Writer {
            source,
            seen: Vec::new(),
            fields: Vec::new(),
        };
        let mut players = 0;
        let mut units = Vec::with_capacity(layout::LAYOUT.len());
        for &(routine, steps) in layout::LAYOUT {
            for &step in steps {
                match step {
                    Step::Read(size, dest) => {
                        let field = w.write(size, dest);
                        if dest == PLAYERS {
                            players = field[0];
                        }
                    }
                    Step::List { count, record, dest } => {
                        let mut records = w.source.records(dest);
                        records.truncate(MAX_RECORDS);
                        let n = u16::try_from(records.len()).unwrap_or(u16::MAX);
                        w.next(count);
                        w.push(count, n.to_le_bytes().to_vec(), 2);
                        for r in records {
                            w.next(dest);
                            w.push(dest, r, record);
                        }
                    }
                    Step::Windows => {
                        for form in 0..=16 {
                            if form == 2 {
                                continue;
                            }
                            w.write(2, "[1110:646c[i]].Top");
                            w.write(2, "[1110:646c[i]].Left");
                            w.write(2, "[1110:646c[i]].Width");
                            w.write(2, "[1110:646c[i]].Height");
                            w.write(1, "[1110:646c[i]].Visible");
                            w.write(1, "[1110:646c[i]].Enabled");
                        }
                    }
                    Step::PerComputerPlayer => {
                        for _ in 2..=players {
                            for dest in ["*[1110:70e2[i]]", "*[1110:70e6[i]]"] {
                                for _ in 0..3 {
                                    w.write(0x100, dest);
                                }
                            }
                        }
                    }
                }
            }
            units.push(Unit {
                routine,
                fields: std::mem::take(&mut w.fields),
            });
        }
        SaveGame {
            version: VERSION.to_vec(),
            units,
            trailing: Vec::new(),
        }
    }

    /// Every field with its occurrence: how many fields before it in the
    /// file are stored at the same `dest`.
    pub fn numbered(&self) -> impl Iterator<Item = (usize, &Field)> {
        let mut seen: Vec<(&'static str, usize)> = Vec::new();
        self.units.iter().flat_map(|u| &u.fields).map(move |f| {
            let n = if let Some(e) = seen.iter_mut().find(|e| e.0 == f.dest) {
                e.1 += 1;
                e.1 - 1
            } else {
                seen.push((f.dest, 1));
                0
            };
            (n, f)
        })
    }

    /// The file's bytes, as `save_map` writes them.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = self.version.clone();
        for field in self.units.iter().flat_map(|u| &u.fields) {
            out.extend_from_slice(&field.data);
        }
        out.extend_from_slice(&self.trailing);
        out
    }

    /// The first field stored at `dest`.
    #[must_use]
    pub fn field(&self, dest: &str) -> Option<&[u8]> {
        self.units
            .iter()
            .flat_map(|u| &u.fields)
            .find(|f| f.dest == dest)
            .map(|f| &f.data[..])
    }

    /// Every field stored at `dest`, in the file's order: the records of a
    /// list.
    pub fn fields<'a>(&'a self, dest: &'a str) -> impl Iterator<Item = &'a [u8]> {
        self.units
            .iter()
            .flat_map(|u| &u.fields)
            .filter(move |f| f.dest == dest)
            .map(|f| &f.data[..])
    }

    /// The `n`th field (from 0) stored at `dest`.
    #[must_use]
    pub fn nth<'a>(&'a self, dest: &'a str, n: usize) -> Option<&'a [u8]> {
        self.fields(dest).nth(n)
    }

    /// The number of players, the human one included.
    #[must_use]
    pub fn players(&self) -> u8 {
        self.field(PLAYERS).map_or(0, |d| d[0])
    }

    /// The port's own chunk after the original's fields
    /// ([`port_chunk`]), if the file has one.
    #[must_use]
    pub fn port_chunk(&self) -> Option<Vec<(String, String)>> {
        port_chunk(&self.trailing)
    }
}

/// What starts the port's chunk: `HIPORT`, a 0 and the chunk's version.
/// Then comes a `Longint` length and that many bytes of text, one
/// `key=value` per line. `load_map` (1048:2a19) reads its units' fixed
/// sizes and closes the file, and nothing in the original looks at a
/// map's length, so the original loads a file with the chunk as it loads
/// one without (`docs/savegame.md`).
pub const PORT_CHUNK_MAGIC: &[u8; 8] = b"HIPORT\x00\x01";

/// The port's chunk with `pairs`, to go after the original's fields
/// ([`SaveGame::trailing`]).
#[must_use]
pub fn make_port_chunk<'a>(pairs: impl IntoIterator<Item = (&'a str, &'a str)>) -> Vec<u8> {
    let text: String = pairs.into_iter().flat_map(|(k, v)| [k, "=", v, "\n"]).collect();
    let mut out = PORT_CHUNK_MAGIC.to_vec();
    out.extend_from_slice(&u32::try_from(text.len()).unwrap_or(u32::MAX).to_le_bytes());
    out.extend_from_slice(text.as_bytes());
    out
}

/// The `key=value` pairs of the port's chunk at the start of `trailing`,
/// in order; none if it doesn't start with one (an original file, or one
/// the port didn't write). Lines that aren't `key=value` are skipped.
#[must_use]
pub fn port_chunk(trailing: &[u8]) -> Option<Vec<(String, String)>> {
    let rest = trailing.strip_prefix(&PORT_CHUNK_MAGIC[..])?;
    let len = usize::try_from(u32::from_le_bytes(rest.get(..4)?.try_into().ok()?)).ok()?;
    let text = std::str::from_utf8(rest.get(4..4 + len)?).ok()?;
    Some(
        text.lines()
            .filter_map(|l| l.split_once('='))
            .map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned()))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Gives back a parsed file's fields.
    struct Copy<'a>(&'a SaveGame);

    impl Source for Copy<'_> {
        fn field(&mut self, dest: &'static str, n: usize, _size: usize) -> Vec<u8> {
            self.0.nth(dest, n).map(<[u8]>::to_vec).unwrap_or_default()
        }

        fn records(&mut self, dest: &'static str) -> Vec<Vec<u8>> {
            self.0.fields(dest).map(<[u8]>::to_vec).collect()
        }
    }

    /// Zeros, with `players` players and a list of two plots.
    struct Blank(u8);

    impl Source for Blank {
        fn field(&mut self, dest: &'static str, _n: usize, _size: usize) -> Vec<u8> {
            if dest == PLAYERS { vec![self.0] } else { Vec::new() }
        }

        fn records(&mut self, dest: &'static str) -> Vec<Vec<u8>> {
            if dest == "*1110:6f0c" {
                vec![vec![1], vec![2; 0x400]]
            } else {
                Vec::new()
            }
        }
    }

    #[test]
    fn build_and_parse() {
        let blank = SaveGame::build(&mut Blank(3));
        let bytes = blank.to_bytes();
        let parsed = SaveGame::parse(&bytes).unwrap();
        assert_eq!(parsed, blank);
        assert_eq!(parsed.players(), 3);
        assert_eq!(parsed.nth("1110:552c", 0), Some(&[0, 0][..]));
        assert_eq!(parsed.nth("1110:552c", 1), Some(&[2, 0][..]));
        let plots: Vec<_> = parsed.fields("*1110:6f0c").collect();
        assert_eq!(plots.len(), 2);
        assert_eq!(plots[0].len(), 0x364);
        assert_eq!(plots[0][0], 1);
        assert!(plots[1].iter().all(|&b| b == 2));
        // Two computer players, two blocks of three lists each.
        assert_eq!(parsed.fields("*[1110:70e2[i]]").count(), 6);
        assert_eq!(SaveGame::build(&mut Copy(&parsed)), parsed);
        let numbered: Vec<_> = parsed
            .numbered()
            .filter(|(_, f)| f.dest == "1110:5530")
            .map(|(n, _)| n)
            .collect();
        assert_eq!(numbered, [0, 1]);
    }

    #[test]
    fn port_chunks() {
        let chunk = make_port_chunk([("fix.a", "1"), ("fix.b", "0")]);
        assert!(chunk.starts_with(PORT_CHUNK_MAGIC));
        let pairs = port_chunk(&chunk).unwrap();
        assert_eq!(pairs, [("fix.a".into(), "1".into()), ("fix.b".into(), "0".into())]);
        assert_eq!(port_chunk(&make_port_chunk([])), Some(Vec::new()));
        assert_eq!(port_chunk(b""), None);
        assert_eq!(port_chunk(b"something else"), None);
        // Cut short: no chunk.
        assert_eq!(port_chunk(&chunk[..chunk.len() - 1]), None);
    }
}
