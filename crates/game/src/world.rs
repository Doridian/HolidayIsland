//! The simulation's state: the island's map layers, the clock and date,
//! the players' money and the random numbers. The rest of the globals a
//! map file holds (`formats::savegame`) join as the code that uses them is
//! ported.
//!
//! The original keeps each layer in a 63001-byte block on the heap, one far
//! pointer each at `1110:5500` to `1110:5520`, indexed `[i * 251 + j]`.
//! `load_map` (1048:2a19) reads them in its first unit routine (1000:0c6a).
//! The map's two axes have no names in the code; `i` is the one the
//! drawing multiplies by 251.

use formats::savegame::{MAP_SIZE, SaveGame};
use vcl::random::Random;

use crate::monate::Date;
use crate::money::Finances;
use crate::simulation::{Accounting, Clock, Options};

/// A map position. Coordinates are `Integer`s in the original and may be
/// off the map while the drawing walks across it.
pub type Pos = (i16, i16);

/// One 251 × 251 byte layer.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Layer(Vec<u8>);

impl Layer {
    fn from_bytes(data: &[u8]) -> Option<Self> {
        (data.len() == MAP_SIZE * MAP_SIZE).then(|| Layer(data.to_vec()))
    }

    /// A layer filled with `value`.
    #[must_use]
    pub fn filled(value: u8) -> Self {
        Layer(vec![value; MAP_SIZE * MAP_SIZE])
    }

    /// The byte at `(i, j)`, which must be on the map.
    ///
    /// # Panics
    /// If it isn't.
    #[must_use]
    pub fn at(&self, (i, j): Pos) -> u8 {
        self.get((i, j)).expect("position off the map")
    }

    /// The byte at `(i, j)`, or `None` off the map.
    #[must_use]
    pub fn get(&self, (i, j): Pos) -> Option<u8> {
        let (i, j) = (usize::try_from(i).ok()?, usize::try_from(j).ok()?);
        (i < MAP_SIZE && j < MAP_SIZE).then(|| self.0[i * MAP_SIZE + j])
    }

    /// Sets the byte at `(i, j)`.
    ///
    /// # Panics
    /// If `(i, j)` is off the map.
    pub fn set(&mut self, (i, j): Pos, value: u8) {
        let (i, j) = (
            usize::try_from(i).expect("on the map"),
            usize::try_from(j).expect("on the map"),
        );
        self.0[i * MAP_SIZE + j] = value;
    }
}

/// No building on a square (`buildings`).
pub const NO_BUILDING: u8 = 0xff;
/// The building type of a palm. // 1050:0e68
pub const PALM: u8 = 12;

/// The island's map layers.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Island {
    /// The land's height at each corner of the squares, in steps of 9
    /// pixels; 0 is the sea. // *1110:5500
    pub heights: Layer,
    /// What the ground is: below 10 the kind of ground (`GROUNDNR.DAT`'s
    /// groups: 0 sand, 1 grass, 2 and 3 water, 4 and 5 sand to grass, 9 a
    /// white wire grid), 10–54 a road piece
    /// (`STRASPOS.DAT`), 55–94 a river piece (`FLUSSPOS.DAT`). // *1110:5504
    pub ground: Layer,
    /// The building type on the square (`HAUSNR.DAT` row), or
    /// [`NO_BUILDING`]. // *1110:5508
    pub buildings: Layer,
    /// The square's slope, 0–18: which corners are raised. Picks the ground
    /// tile and how far it's lifted. // *1110:550c
    pub forms: Layer,
    /// Which part of its building the square shows (for buildings bigger
    /// than one square), or the ground tile's variant. // *1110:5510
    pub parts: Layer,
    /// The player who owns the building, 1–8, or 0. // *1110:5514
    pub owners: Layer,
    /// How full the building is, in percent. // *1110:5518
    pub occupancy: Layer,
    /// Not known yet: 255 nearly everywhere. // *1110:551c
    pub layer_551c: Layer,
    /// Not known yet: 255 everywhere in the CD's maps. // *1110:5520
    pub layer_5520: Layer,
}

impl Island {
    /// An island under water: every layer as `init` would leave it, with no
    /// buildings.
    #[must_use]
    pub fn empty() -> Self {
        Island {
            heights: Layer::filled(0),
            ground: Layer::filled(0),
            buildings: Layer::filled(NO_BUILDING),
            forms: Layer::filled(0),
            parts: Layer::filled(0),
            owners: Layer::filled(0),
            occupancy: Layer::filled(0),
            layer_551c: Layer::filled(0xff),
            layer_5520: Layer::filled(0xff),
        }
    }

    /// The height at the corner `(i, j)` of the map. // 1110:5500
    #[must_use]
    pub fn height(&self, pos: Pos) -> u8 {
        self.heights.at(pos)
    }

    /// The highest of the four corners a building on `(i, j)` stands on:
    /// `(i, j)`, `(i-1, j)`, `(i-1, j-1)` and `(i, j-1)`. 0 within a square
    /// of the map's edge. // 1030:1fdb
    #[must_use]
    pub fn highest_corner(&self, pos: Pos) -> u8 {
        self.corners(pos).map_or(0, |c| c.into_iter().max().unwrap_or(0))
    }

    /// The lowest of the same four corners, 0 near the edge. // 1030:208d
    #[must_use]
    pub fn lowest_corner(&self, pos: Pos) -> u8 {
        self.corners(pos).map_or(0, |c| c.into_iter().min().unwrap_or(0))
    }

    fn corners(&self, (i, j): Pos) -> Option<[u8; 4]> {
        if !(1..=249).contains(&i) || !(1..=249).contains(&j) {
            return None;
        }
        let h = |i, j| self.heights.at((i, j));
        Some([h(i, j), h(i - 1, j), h(i - 1, j - 1), h(i, j - 1)])
    }
}

/// Why a map file can't start a game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// A field `load_map` reads is missing or has the wrong size.
    Field(&'static str),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Field(dest) => write!(f, "the map has no usable {dest}"),
        }
    }
}

impl std::error::Error for Error {}

/// Everyone's game: the island and the players.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct World {
    pub island: Island,
    /// Each player's colour, 1–8 (`START.TXT`'s order: yellow, orange,
    /// ...). Player 1 is the human. // 1110:0074
    pub player_colours: [u8; 8],
    /// The number of players, the human included. // 1110:1479
    pub player_count: u8,
    /// 0 (easy) to 2 (hard). // 1110:008c
    pub difficulty: i16,
    /// The simulation's tick counters and speed.
    pub clock: Clock,
    /// // [1110:65d6]+4
    pub date: Date,
    pub finances: Finances,
    /// The monthly settlement's schedule.
    pub accounting: Accounting,
    /// Days since the game started, for the events. // 1110:653a
    pub days: i32,
    /// Whether disasters may happen (events of kind 2): off on easy.
    /// // 1110:0065
    pub disasters: bool,
    /// An earthquake is going on. // 1110:0087
    pub earthquake: bool,
    pub options: Options,
    /// `RandSeed`. Not in the map file: the original seeds it from the
    /// clock (`Randomize`, 1108:1e21) when it loads the game's data
    /// (1030:38a3), so whoever starts a game picks the seed.
    pub random: Random,
}

impl Default for World {
    /// An empty island and the globals as `init_game_defaults` (1000:1ea0)
    /// and `monate_create` (1030:000d) leave them.
    fn default() -> Self {
        World {
            island: Island::empty(),
            player_colours: [1, 2, 3, 4, 5, 6, 7, 8],
            player_count: 8,
            difficulty: 0,
            clock: Clock::default(),
            date: Date::default(),
            finances: Finances::default(),
            accounting: Accounting::default(),
            days: 0,
            disasters: true,
            earthquake: false,
            options: Options::default(),
            random: Random::default(),
        }
    }
}

fn field<'a>(save: &'a SaveGame, dest: &'static str) -> Result<&'a [u8], Error> {
    save.field(dest).ok_or(Error::Field(dest))
}

fn layer(save: &SaveGame, dest: &'static str) -> Result<Layer, Error> {
    Layer::from_bytes(field(save, dest)?).ok_or(Error::Field(dest))
}

/// A field of exactly `N` bytes.
fn bytes<const N: usize>(save: &SaveGame, dest: &'static str) -> Result<[u8; N], Error> {
    field(save, dest)?.try_into().map_err(|_| Error::Field(dest))
}

fn byte(save: &SaveGame, dest: &'static str) -> Result<u8, Error> {
    Ok(bytes::<1>(save, dest)?[0])
}

fn word(save: &SaveGame, dest: &'static str) -> Result<u16, Error> {
    Ok(u16::from_le_bytes(bytes(save, dest)?))
}

fn longint(save: &SaveGame, dest: &'static str) -> Result<i32, Error> {
    Ok(i32::from_le_bytes(bytes(save, dest)?))
}

/// An `array[1..8] of Longint`.
fn longints(save: &SaveGame, dest: &'static str) -> Result<[i32; 8], Error> {
    let data: [u8; 32] = bytes(save, dest)?;
    Ok(std::array::from_fn(|i| {
        i32::from_le_bytes([data[4 * i], data[4 * i + 1], data[4 * i + 2], data[4 * i + 3]])
    }))
}

impl World {
    /// The state `load_map` leaves behind, as far as it's ported.
    /// // 1048:2a19, 1000:0c6a
    ///
    /// # Errors
    /// If a field is missing or has the wrong size.
    pub fn from_map(save: &SaveGame) -> Result<Self, Error> {
        Ok(World {
            island: Island {
                heights: layer(save, "*1110:5500")?,
                ground: layer(save, "*1110:5504")?,
                buildings: layer(save, "*1110:5508")?,
                forms: layer(save, "*1110:550c")?,
                parts: layer(save, "*1110:5510")?,
                owners: layer(save, "*1110:5514")?,
                occupancy: layer(save, "*1110:5518")?,
                layer_551c: layer(save, "*1110:551c")?,
                layer_5520: layer(save, "*1110:5520")?,
            },
            player_colours: bytes(save, "1110:0074")?,
            player_count: byte(save, "1110:1479")?,
            difficulty: word(save, "1110:008c")?.cast_signed(),
            // Unit 1020:2b1d saves the form's counters.
            clock: Clock {
                hour_ticks: word(save, "[1110:6532]+0x1d6")?,
                day_ticks: word(save, "[1110:6532]+0x1d8")?,
                speed: longint(save, "1110:0096")?,
                slice: word(save, "1110:009e")?.cast_signed(),
                paused: byte(save, "1110:0067")? != 0,
            },
            // Unit 1030:2bc5.
            date: Date {
                day: byte(save, "[1110:65d6]+0x4")?,
                month: byte(save, "[1110:65d6]+0x5")?,
                year: word(save, "[1110:65d6]+0x6")?,
            },
            finances: Finances {
                money: longints(save, "1110:6f30")?,
                loan: longints(save, "1110:6f50")?,
                interest: longints(save, "1110:6f70")?,
                max_debt: longint(save, "1110:64ce")?,
                interest_rate: longint(save, "1110:64d2")?,
                start_money: longint(save, "1110:64d6")?,
                frozen: false,
            },
            // Unit 1008:539f.
            accounting: Accounting {
                step: longint(save, "1110:4a84")?,
                last_day: word(save, "1110:4a88")?,
            },
            days: longint(save, "1110:653a")?,
            disasters: byte(save, "1110:0065")? != 0,
            earthquake: byte(save, "1110:0087")? == 1,
            options: Options {
                animations: byte(save, "1110:0054")? != 0,
                people_move: byte(save, "1110:0055")? != 0,
                people_spawn: byte(save, "1110:0056")? == 1,
            },
            random: Random::default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corners() {
        let mut island = Island::empty();
        island.heights.set((10, 10), 3);
        island.heights.set((9, 9), 1);
        island.heights.set((10, 9), 2);
        island.heights.set((9, 10), 2);
        assert_eq!(island.highest_corner((10, 10)), 3);
        assert_eq!(island.lowest_corner((10, 10)), 1);
        assert_eq!(island.highest_corner((11, 11)), 3);
        assert_eq!(island.lowest_corner((11, 11)), 0);
        island.heights.set((0, 5), 7);
        assert_eq!(island.highest_corner((0, 5)), 0);
        assert_eq!(island.heights.get((-1, 0)), None);
        assert_eq!(island.heights.get((0, 251)), None);
    }
}
