//! The simulation's state. So far that's the island's map layers, which is
//! what the main screen needs to draw it; the rest of the globals a map
//! file holds (`formats::savegame`) join as the code that uses them is
//! ported.
//!
//! The original keeps each layer in a 63001-byte block on the heap, one far
//! pointer each at `1110:5500` to `1110:5520`, indexed `[i * 251 + j]`.
//! `load_map` (1048:2a19) reads them in its first unit routine (1000:0c6a).
//! The map's two axes have no names in the code; `i` is the one the
//! drawing multiplies by 251.

use formats::savegame::{MAP_SIZE, SaveGame};

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
}

fn field<'a>(save: &'a SaveGame, dest: &'static str) -> Result<&'a [u8], Error> {
    save.field(dest).ok_or(Error::Field(dest))
}

fn layer(save: &SaveGame, dest: &'static str) -> Result<Layer, Error> {
    Layer::from_bytes(field(save, dest)?).ok_or(Error::Field(dest))
}

impl World {
    /// The state `load_map` leaves behind, as far as it's ported.
    /// // 1048:2a19, 1000:0c6a
    ///
    /// # Errors
    /// If a field is missing or has the wrong size.
    pub fn from_map(save: &SaveGame) -> Result<Self, Error> {
        let colours = field(save, "1110:0074")?;
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
            player_colours: colours.try_into().map_err(|_| Error::Field("1110:0074"))?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corners() {
        let mut island = Island {
            heights: Layer::filled(0),
            ground: Layer::filled(0),
            buildings: Layer::filled(NO_BUILDING),
            forms: Layer::filled(0),
            parts: Layer::filled(0),
            owners: Layer::filled(0),
            occupancy: Layer::filled(0),
            layer_551c: Layer::filled(0xff),
            layer_5520: Layer::filled(0xff),
        };
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
