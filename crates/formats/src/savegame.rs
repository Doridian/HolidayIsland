//! Maps, scenarios and saved games: `MAPS\*.MAP`, `SZENARIO\*.HIS` and the
//! player's `*.map` files (`Last.map` too) all have this format.
//!
//! The file is a dump of the game's state. `load_map` (1048:2a19) reads a
//! 6-byte version string (`1.000` as a `string[5]`, not checked), then calls
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

mod layout;

use std::fmt;

/// The version string at the start of every file: `string[5]` `1.000`.
pub const VERSION: [u8; 6] = *b"\x051.000";

/// Side length of the island's map layers: the 63001-byte blocks are
/// 251 × 251 bytes.
pub const MAP_SIZE: usize = 251;

/// The global holding the number of players, human included. // 1110:1479
const PLAYERS: &str = "1110:1479";

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
    /// The window positions of the 17 forms in `1110:646c`, except form 2.
    /// // 1010:08d7
    Windows,
    /// Two blocks of three 256-byte records per computer player, players 2 to
    /// the player count at `1110:1479`. // 10a8:32ec, 1030:0eab
    PerComputerPlayer,
}

/// One `BlockRead`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    /// Where the original stores the bytes, in the notation of the module
    /// documentation.
    pub dest: &'static str,
    pub data: Vec<u8>,
}

/// What one unit routine reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unit {
    /// The routine's address.
    pub routine: &'static str,
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveGame {
    /// The version string, as stored.
    pub version: Vec<u8>,
    pub units: Vec<Unit>,
    /// Bytes after the last read, which the game ignores.
    pub trailing: Vec<u8>,
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
                            r.read(1, "(form state)")?;
                            r.read(1, "[1110:646c[i]].Visible")?;
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

    /// The number of players, the human one included.
    #[must_use]
    pub fn players(&self) -> u8 {
        self.field(PLAYERS).map_or(0, |d| d[0])
    }
}
