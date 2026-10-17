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

use std::sync::Arc;

use formats::savegame::{MAP_SIZE, SaveGame};
use vcl::random::Random;

use crate::events::{Auction, Damage, Event, parse_events};
use crate::monate::Date;
use crate::money::Finances;
use crate::people::{DIRECTIONS_SIZE, ENTRIES_SIZE, PATHS_SIZE, People, TRACKS_SIZE, Viewpoint};
use crate::sabotage::{Championship, Sabotage, parse_championships, parse_sabotage};
use crate::simulation::{Accounting, Clock, Options};

/// A map position. Coordinates are `Integer`s in the original and may be
/// off the map while the drawing walks across it.
pub type Pos = (i16, i16);

/// Whether `(i, j)` is on the map: both 0 to 250. // 1030:1f93
#[must_use]
pub fn on_map((i, j): Pos) -> bool {
    (0..=250).contains(&i) && (0..=250).contains(&j)
}

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

/// Side of the location quality map: one cell for each 2 × 2 squares.
pub const LOCATION_SIZE: usize = 126;

/// How good each 2 × 2 block of squares is as a place to build, around
/// 100 (`Lage`): buildings, palms, sabotage and disasters raise and lower
/// it around themselves (1030:185d). Indexed `[i div 2][j div 2]`.
/// // *1110:5524
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LocationMap(Vec<u8>);

impl LocationMap {
    fn from_bytes(data: &[u8]) -> Option<Self> {
        (data.len() == LOCATION_SIZE * LOCATION_SIZE).then(|| LocationMap(data.to_vec()))
    }

    /// A map filled with `value`.
    #[must_use]
    pub fn filled(value: u8) -> Self {
        LocationMap(vec![value; LOCATION_SIZE * LOCATION_SIZE])
    }

    /// The cell `(a, b)`, or `None` off the map.
    #[must_use]
    pub fn get(&self, (a, b): Pos) -> Option<u8> {
        let (a, b) = (usize::try_from(a).ok()?, usize::try_from(b).ok()?);
        (a < LOCATION_SIZE && b < LOCATION_SIZE).then(|| self.0[a * LOCATION_SIZE + b])
    }

    /// Sets the cell `(a, b)`; nothing off the map.
    pub fn set(&mut self, (a, b): Pos, value: u8) {
        if let (Ok(a), Ok(b)) = (usize::try_from(a), usize::try_from(b))
            && a < LOCATION_SIZE
            && b < LOCATION_SIZE
        {
            self.0[a * LOCATION_SIZE + b] = value;
        }
    }
}

/// Which plot (`Grundstück`, [`Plot`]) each square belongs to, 0 for
/// none, with one row for every two rows of squares: `[i div 2][j]`.
/// // *1110:5528
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PlotMap(Vec<u16>);

impl PlotMap {
    fn from_bytes(data: &[u8]) -> Option<Self> {
        (data.len() == LOCATION_SIZE * MAP_SIZE * 2)
            .then(|| PlotMap(data.as_chunks::<2>().0.iter().map(|w| u16::from_le_bytes(*w)).collect()))
    }

    /// No plots anywhere.
    #[must_use]
    pub fn empty() -> Self {
        PlotMap(vec![0; LOCATION_SIZE * MAP_SIZE])
    }

    /// The entry for the squares `(i, j)` and `(i ± 1, j)`, or `None` off
    /// the table. Like the original, a `j` just off a row reads the next
    /// or previous row's end.
    #[must_use]
    pub fn get(&self, (i, j): Pos) -> Option<u16> {
        let index = usize::try_from(i32::from(i / 2) * 251 + i32::from(j)).ok()?;
        self.0.get(index).copied()
    }

    /// Sets the entry for `(i, j)`; nothing off the table.
    pub fn set(&mut self, (i, j): Pos, plot: u16) {
        if let (Ok(a), Ok(j)) = (usize::try_from(i / 2), usize::try_from(j))
            && a < LOCATION_SIZE
            && j < MAP_SIZE
        {
            self.0[a * MAP_SIZE + j] = plot;
        }
    }
}

/// The size of a [`Plot`] record. // 1068:313e
pub const PLOT_SIZE: usize = 0x364;

/// A plot of land, the record as the map file stores it. Only the fields
/// the ported code uses are known; the list's far pointer to the next
/// record (`+0x360`) means nothing here, the `Vec`'s order is the list's.
/// // *1110:6f0c
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Plot {
    record: Vec<u8>,
    /// The first squares of the houses on the plot, newest first: the
    /// list at `+0x358` (1068:1d05), which loading a map rebuilds from the
    /// houses (1068:2d49) and isn't in the file.
    pub houses: Vec<Pos>,
    /// The same for the facilities, the list at `+0x35c` (1068:1d55).
    pub facilities: Vec<Pos>,
}

impl Plot {
    /// A record of zeros with the number `id`.
    #[must_use]
    pub fn new(id: i16) -> Self {
        let mut plot = Plot::from_record(vec![0; PLOT_SIZE]);
        plot.record[..2].copy_from_slice(&id.to_le_bytes());
        plot
    }

    fn from_record(record: Vec<u8>) -> Self {
        Plot {
            record,
            houses: Vec::new(),
            facilities: Vec::new(),
        }
    }

    fn word(&self, at: usize) -> i16 {
        i16::from_le_bytes([self.record[at], self.record[at + 1]])
    }

    fn set_word(&mut self, at: usize, value: i16) {
        self.record[at..at + 2].copy_from_slice(&value.to_le_bytes());
    }

    /// The plot's number in the [`PlotMap`]. // +0
    #[must_use]
    pub fn id(&self) -> i16 {
        self.word(0)
    }

    /// How many palms were planted on it, plus the squares of its parks
    /// (`HAUSDATA.DAT` kind 6, [`World::register_building`]); a palm that
    /// dies isn't taken off. // +0x354
    #[must_use]
    pub fn palms(&self) -> i16 {
        self.word(0x354)
    }

    pub fn set_palms(&mut self, palms: i16) {
        self.set_word(0x354, palms);
    }

    /// The squares of its buildings other than parks. // +0x356
    #[must_use]
    pub fn built(&self) -> i16 {
        self.word(0x356)
    }

    pub fn set_built(&mut self, squares: i16) {
        self.set_word(0x356, squares);
    }

    /// How many airports and harbours (`HAUSDATA.DAT` kinds 9 and 10) are
    /// on it. // +2
    #[must_use]
    pub fn ports(&self) -> u8 {
        self.record[2]
    }

    pub fn set_ports(&mut self, n: u8) {
        self.record[2] = n;
    }

    /// The `Longint` at `at`, 0 past the record. The sums
    /// [`World::register_building`] keeps are at `+0x22` (the houses'
    /// capacity), `+0x4e + 0x2c·class` and `+0xc6 + 4·class` (the
    /// facilities').
    #[must_use]
    pub fn longint(&self, at: usize) -> i32 {
        longint_at(&self.record, at)
    }

    /// Sets the `Longint` at `at`; nothing past the record.
    pub fn set_longint(&mut self, at: usize, value: i32) {
        set_longint_at(&mut self.record, at, value);
    }

    /// Not known yet: a value 0–255 that scenario 6 averages over the
    /// plots, weighted by [`Plot::weight`]. // +4
    #[must_use]
    pub fn value_4(&self) -> u8 {
        self.record[4]
    }

    /// Not known yet (its size?): the weight of [`Plot::value_4`].
    /// // +0x352
    #[must_use]
    pub fn weight(&self) -> u16 {
        self.word(0x352).cast_unsigned()
    }
}

/// The `Longint` at `at` in `record`, 0 past its end.
fn longint_at(record: &[u8], at: usize) -> i32 {
    record
        .get(at..at + 4)
        .and_then(|b| b.try_into().ok())
        .map_or(0, i32::from_le_bytes)
}

/// Sets the `Longint` at `at` in `record`; nothing past its end.
fn set_longint_at(record: &mut [u8], at: usize, value: i32) {
    if let Some(b) = record.get_mut(at..at + 4) {
        b.copy_from_slice(&value.to_le_bytes());
    }
}

/// The size of [`World::totals`]. // 1078:34ad
pub const TOTALS_SIZE: usize = 0x19b;

/// The size of a [`House`] record. // 1068:2e05
pub const HOUSE_SIZE: usize = 0x2c;

/// The most beach life a house gets.
pub const MAX_BEACH_LIFE: u8 = 5;

/// A finished building, the record as the map file stores it. Only the
/// fields the ported code uses are known; the list's far pointer to the
/// next record (`+8`) means nothing here. // *1110:6f10
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct House(Vec<u8>);

impl House {
    /// A record of zeros at `pos`.
    #[must_use]
    pub fn new(pos: Pos) -> Self {
        let mut house = House(vec![0; HOUSE_SIZE]);
        house.set_word(3, pos.0);
        house.set_word(5, pos.1);
        house
    }

    fn word(&self, at: usize) -> i16 {
        i16::from_le_bytes([self.0[at], self.0[at + 1]])
    }

    fn set_word(&mut self, at: usize, value: i16) {
        self.0[at..at + 2].copy_from_slice(&value.to_le_bytes());
    }

    /// The building type. // +0
    #[must_use]
    pub fn building(&self) -> u8 {
        self.0[0]
    }

    pub fn set_building(&mut self, building: u8) {
        self.0[0] = building;
    }

    /// The square the building starts at. // +3, +5
    #[must_use]
    pub fn pos(&self) -> Pos {
        (self.word(3), self.word(5))
    }

    /// `HAUSDATA.DAT`'s column 4 for its type, copied when it was built
    /// (1068:1a08). // +1
    #[must_use]
    pub fn value_1(&self) -> u8 {
        self.0[1]
    }

    pub fn set_value_1(&mut self, value: u8) {
        self.0[1] = value;
    }

    /// Its condition: 127 when new or renovated ([`World::year_end`]),
    /// which its value follows. // +7
    #[must_use]
    pub fn condition(&self) -> u8 {
        self.0[7]
    }

    pub fn set_condition(&mut self, condition: u8) {
        self.0[7] = condition;
    }

    /// How well it's connected: `Angeschlossenumrechnung`
    /// ([`World::connection_values`]) for how far its nearest road is
    /// (1030:1706), when it was built. // +2
    #[must_use]
    pub fn connection(&self) -> u8 {
        self.0[2]
    }

    pub fn set_connection(&mut self, value: u8) {
        self.0[2] = value;
    }

    /// Probably its price per night: `HAUSPREI.DAT`'s third column when
    /// it was built. // +0x12, `Longint`
    #[must_use]
    pub fn price(&self) -> i32 {
        longint_at(&self.0, 0x12)
    }

    pub fn set_price(&mut self, price: i32) {
        set_longint_at(&mut self.0, 0x12, price);
    }

    /// Not known yet: 0x50 when it's built (1068:1a08). // +0x16
    #[must_use]
    pub fn value_16(&self) -> u8 {
        self.0[0x16]
    }

    pub fn set_value_16(&mut self, value: u8) {
        self.0[0x16] = value;
    }

    /// Not known yet, probably its guests: how much beach life it gets
    /// follows it, and with its capacity (`HAUSDATA.DAT` column 3) how
    /// likely it sends someone out ([`World::building_slice`]). // +0x10
    #[must_use]
    pub fn value_10(&self) -> i16 {
        self.word(0x10)
    }

    pub fn set_value_10(&mut self, value: i16) {
        self.set_word(0x10, value);
    }

    /// How many squares of beach life (building type 36) it has around
    /// it, up to [`MAX_BEACH_LIFE`]. // +0x2b
    #[must_use]
    pub fn beach_life(&self) -> u8 {
        self.0[0x2b]
    }

    pub fn set_beach_life(&mut self, n: u8) {
        self.0[0x2b] = n;
    }

    /// The square of its beach life number `n`, 1 to [`MAX_BEACH_LIFE`].
    /// // +0x13 + 4n
    ///
    /// # Panics
    /// If `n` is more than that.
    #[must_use]
    pub fn beach_life_square(&self, n: u8) -> Pos {
        let at = 0x13 + 4 * usize::from(n);
        (self.word(at), self.word(at + 2))
    }

    /// Sets the square of beach life number `n`.
    ///
    /// # Panics
    /// If `n` is more than [`MAX_BEACH_LIFE`].
    pub fn set_beach_life_square(&mut self, n: u8, (i, j): Pos) {
        let at = 0x13 + 4 * usize::from(n);
        self.set_word(at, i);
        self.set_word(at + 2, j);
    }
}

/// The size of a [`Facility`] record. // 1068:2ebc
pub const FACILITY_SIZE: usize = 0x1b;

/// A finished building of the kinds that aren't in the houses' list
/// (`1110:1341[HAUSDATA.DAT column 5]` is 1 or 2 for them, 0 for houses;
/// restaurants, shops and the like?), the record as the map file stores it.
/// Only the fields the ported code uses are known; `+8` is the list's far
/// pointer to the next record. // *1110:6f14
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Facility(Vec<u8>);

impl Facility {
    /// A record of zeros for `building` at `pos`, in the condition of a new
    /// one.
    #[must_use]
    pub fn new(building: u8, (i, j): Pos) -> Self {
        let mut r = vec![0; FACILITY_SIZE];
        r[0] = building;
        r[3..5].copy_from_slice(&i.to_le_bytes());
        r[5..7].copy_from_slice(&j.to_le_bytes());
        r[7] = 0x7f;
        Facility(r)
    }

    fn word(&self, at: usize) -> i16 {
        i16::from_le_bytes([self.0[at], self.0[at + 1]])
    }

    /// The building type. // +0
    #[must_use]
    pub fn building(&self) -> u8 {
        self.0[0]
    }

    pub fn set_building(&mut self, building: u8) {
        self.0[0] = building;
    }

    /// Like [`House::value_1`]. // +1
    #[must_use]
    pub fn value_1(&self) -> u8 {
        self.0[1]
    }

    pub fn set_value_1(&mut self, value: u8) {
        self.0[1] = value;
    }

    /// Like [`House::connection`]. // +2
    #[must_use]
    pub fn connection(&self) -> u8 {
        self.0[2]
    }

    pub fn set_connection(&mut self, value: u8) {
        self.0[2] = value;
    }

    /// The square the building starts at. // +3, +5
    #[must_use]
    pub fn pos(&self) -> Pos {
        (self.word(3), self.word(5))
    }

    /// Its condition, like [`House::condition`]. // +7
    #[must_use]
    pub fn condition(&self) -> u8 {
        self.0[7]
    }

    pub fn set_condition(&mut self, condition: u8) {
        self.0[7] = condition;
    }

    /// Not known yet: what scenario 5 counts for building type 0x6d.
    /// // +0xc, `Longint`
    #[must_use]
    pub fn value_c(&self) -> i32 {
        i32::from_le_bytes([self.0[0xc], self.0[0xd], self.0[0xe], self.0[0xf]])
    }

    pub fn set_value_c(&mut self, value: i32) {
        self.0[0xc..0x10].copy_from_slice(&value.to_le_bytes());
    }

    /// Probably how full it is, in percent: the chance that it sends
    /// someone out ([`World::building_slice`]). A full championship's
    /// building gets [`Facility::value_c`] of its capacity
    /// ([`World::replace_building`]). // +0x15
    #[must_use]
    pub fn fill(&self) -> u8 {
        self.0[0x15]
    }

    pub fn set_fill(&mut self, percent: u8) {
        self.0[0x15] = percent;
    }

    /// Like [`House::price`]. // +0x16, `Longint`
    #[must_use]
    pub fn price(&self) -> i32 {
        longint_at(&self.0, 0x16)
    }

    pub fn set_price(&mut self, price: i32) {
        set_longint_at(&mut self.0, 0x16, price);
    }
}

/// The size of a [`BuildingSite`] record. // 1068:304f
pub const BUILDING_SITE_SIZE: usize = 0xd;

/// A building under construction (building type 31 on its squares).
/// // *1110:6f08, 13 bytes each (1068:1c7d)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct BuildingSite {
    /// Not written by 1068:1c7d. // +0
    pub byte_0: u8,
    /// The building's first square. // +1, +3
    pub pos: Pos,
    /// Days left; it's finished when they reach 0. // +5
    pub days: i16,
    /// The building type it becomes. // +7
    pub building: u8,
    /// For a computer player's building, which of its budgets
    /// ([`World::computer_budgets`]) it comes out of. // +8
    pub budget: i8,
}

impl BuildingSite {
    fn parse(r: &[u8]) -> Self {
        let word = |at: usize| i16::from_le_bytes([r[at], r[at + 1]]);
        BuildingSite {
            byte_0: r[0],
            pos: (word(1), word(3)),
            days: word(5),
            building: r[7],
            budget: r[8].cast_signed(),
        }
    }
}

/// The number of [`World::computer_budgets`]: 13 for each of players 2
/// to 8. // *1110:6512, 0x16c bytes
pub const COMPUTER_BUDGETS: usize = 7 * 13;

/// The money menu's books (`TGeldmenuform`, `*1110:6f04`): the parts the
/// world reads.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Books {
    /// The human's sums by kind, `array[0x52..0x6e] of Longint`: kinds
    /// 0x52–0x5d last year's, 0x5e last year's total, 0x5f–0x6a this
    /// year's, 0x6b this year's total; 0x6c–0x6e not known yet.
    /// // [1110:6f04]+0x49 + 4·kind, saved from +0x191
    pub year: [i32; 29],
    /// The monthly history: 120 months of a `Longint` for each player at
    /// `+0x1e14`, a ring with the number of months at `+0x2d1c` and where
    /// it starts at `+0x2d1d`; the rest isn't known yet.
    /// // *[1110:6f04]+0x24b, 0x3c28 bytes
    pub history: Vec<u8>,
}

/// The first kind in [`Books::year`].
pub const FIRST_BOOK: u8 = 0x52;
/// The size of [`Books::history`].
pub const HISTORY_SIZE: usize = 0x3c28;

impl Default for Books {
    fn default() -> Self {
        Books {
            year: [0; 29],
            history: vec![0; HISTORY_SIZE],
        }
    }
}

/// The size of [`World::player_block`]. // 1078:34ad
pub const PLAYER_BLOCK_SIZE: usize = 0x88;

/// The `DATA\` tables the simulation reads. They come from the CD when the
/// game starts (1030:3932), not from the map file, and never change.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Tables {
    /// `HAUSNR.DAT`, one row per building type. // *1110:54f2
    pub hausnr: Vec<[i16; 8]>,
    /// `HAUSDATA.DAT`, one row per building type. // *1110:7372
    pub hausdata: Vec<[i16; 10]>,
    /// `HAUSPREI.DAT`, one row per building type; the first column is the
    /// building's price. // *1110:736e
    pub hausprei: Vec<[i32; 4]>,
    /// `GROUNDNR.DAT`, one row per kind of ground and form; the first
    /// column is how many variants its tile has. // *1110:54e6
    pub groundnr: Vec<[i16; 2]>,
    /// `ANIMDAT1.DAT`, the people's pictures: the first two columns are
    /// a picture's height and width. // *1110:6fea
    pub animdat1: Vec<[i16; 11]>,
    /// `ANIMDAT4.DAT`, 8 groups of 16 rows of 3 columns: the planes'
    /// routes. // *1110:6fc6
    pub animdat4: Vec<Vec<[i16; 3]>>,
    /// `DARSTELL.DAT`'s `AnsichtFormkorrektur`: a square's form as seen
    /// from each rotation. // *1110:667e
    pub form_korrektur: [[u8; 19]; 4],
}

impl Tables {
    /// A `HAUSNR.DAT` cell, 0 for a type the table doesn't have (the
    /// original reads past it).
    #[must_use]
    pub fn hausnr(&self, building: u8, column: usize) -> i16 {
        self.hausnr.get(usize::from(building)).map_or(0, |r| r[column])
    }

    /// A `HAUSDATA.DAT` cell, 0 for a type the table doesn't have.
    #[must_use]
    pub fn hausdata(&self, building: u8, column: usize) -> i16 {
        self.hausdata.get(usize::from(building)).map_or(0, |r| r[column])
    }

    /// A `HAUSPREI.DAT` cell, 0 for a type the table doesn't have.
    #[must_use]
    pub fn hausprei(&self, building: u8, column: usize) -> i32 {
        self.hausprei.get(usize::from(building)).map_or(0, |r| r[column])
    }

    /// A `GROUNDNR.DAT` cell, 0 for a row the table doesn't have.
    #[must_use]
    pub fn groundnr(&self, row: usize, column: usize) -> i16 {
        self.groundnr.get(row).map_or(0, |r| r[column])
    }

    /// An `ANIMDAT1.DAT` cell, 0 for a row the table doesn't have (the
    /// original reads past it).
    #[must_use]
    pub fn animdat1(&self, row: i16, column: usize) -> i16 {
        usize::try_from(row)
            .ok()
            .and_then(|r| self.animdat1.get(r))
            .map_or(0, |r| r[column])
    }

    /// An `ANIMDAT4.DAT` cell, 0 outside the table.
    #[must_use]
    pub fn animdat4(&self, group: i16, row: i16, column: usize) -> i16 {
        let (Ok(g), Ok(r)) = (usize::try_from(group), usize::try_from(row)) else {
            return 0;
        };
        self.animdat4.get(g).and_then(|g| g.get(r)).map_or(0, |r| r[column])
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
    pub location: LocationMap,
    pub plot_map: PlotMap,
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
            location: LocationMap::filled(100),
            plot_map: PlotMap::empty(),
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

    /// The plot `(i, j)` belongs to: the [`PlotMap`]'s entry, or for a
    /// square without one, its neighbour's in the direction its form
    /// (0–3) gives. 0 off the map. // 1030:1e62
    #[must_use]
    pub fn plot_at(&self, (i, j): Pos) -> u16 {
        if !(0..=250).contains(&i) || !(0..=250).contains(&j) {
            return 0;
        }
        let map = &self.plot_map;
        match map.get((i, j)) {
            Some(0) => match self.forms.at((i, j)) {
                // `(i - 1) div 2` rounds towards zero, like `i div 2` for 0.
                0 => map.get((i - 1, j)),
                1 => map.get((i, j + 1)),
                2 => map.get((i + 1, j)),
                3 => map.get((i, j - 1)),
                _ => Some(0),
            }
            .unwrap_or(0),
            plot => plot.unwrap_or(0),
        }
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
#[allow(clippy::struct_excessive_bools)] // The original's flags, one global each.
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
    /// A flood is going on: the drawing lowers the land by a step, and
    /// squares at sea level lose their buildings (but not palms) and turn
    /// to water. // 1110:0086
    pub flood: bool,
    /// An oil slick is going on. Not in the map file. // 1110:0088
    pub oil_slick: bool,
    /// Probably whether the player is insured against disasters: a
    /// disaster that begins while it's set pays its damage back at the end.
    /// // 1110:0072
    pub insured: bool,
    /// The damage of the disaster going on.
    pub damage: Damage,
    /// The scenario's events. // *1110:6536
    pub events: Vec<Event>,
    /// The auction an event started last.
    pub auction: Auction,
    pub options: Options,
    /// The plots of land, in the list's order. // *1110:6f0c
    pub plots: Vec<Plot>,
    /// The finished buildings, in the list's order. // *1110:6f10
    pub houses: Vec<House>,
    /// The other finished buildings, in the list's order. // *1110:6f14
    pub facilities: Vec<Facility>,
    /// The buildings under construction, in the list's order: new ones
    /// come first (1068:1c7d), and since loading a map puts each record in
    /// front of the ones before it (1068:304f), the file has them the other
    /// way round. // *1110:6f08
    pub building_sites: Vec<BuildingSite>,
    /// Each computer player's budgets, 13 for each of players 2 to 8,
    /// which their buildings' costs come out of. // *1110:6512
    pub computer_budgets: Vec<i32>,
    /// The money menu's books.
    pub books: Books,
    /// Every how many years each player's buildings are renovated, 0 for
    /// never ([`World::year_end`]). `array[1..8] of Byte`. // 1110:1334
    pub renovation_years: [u8; 8],
    /// `Renovierungskosten` (`MODATYP.DAT`): what renovating costs, per
    /// 1000 of a building's price and step of condition. // 1110:64c2
    pub renovation_cost: i32,
    /// The scenario, 1–6, 0 for a normal game: what wins and loses it
    /// ([`World::january_second`]). // 1110:006e
    pub scenario: u8,
    /// Probably that the player cheated: a key on the main screen sets it
    /// (`hauptschirm_formkeydown`), and a won game then gets no score.
    /// // 1110:006f
    pub cheated: bool,
    /// The scenario is won. // 1110:0071
    pub won: bool,
    /// Not known yet: set on easy when a scenario starts, and by the
    /// options (1060:0637); it lowers the score. // 1110:133c
    pub flag_133c: bool,
    /// A block of per-player values, mostly not known yet; `+0x2f +
    /// player` is the player's reputation ([`World::reputation`]).
    /// // *1110:6f24, 0x88 bytes
    pub player_block: Vec<u8>,
    /// Sabotage under way. // *1110:705a
    pub sabotage: [Sabotage; 5],
    /// Championships under way. // *1110:705e
    pub championships: [Championship; 3],
    /// `MaxLageeinfluss` (`MODATYP.DAT`): twice the largest radius a
    /// building's effect on the location map can have. // 1110:64bf
    pub max_location_effect: u8,
    /// `Strassenabrisskosten` (`MODATYP.DAT`): what demolishing a road
    /// square costs. // 1110:64ea
    pub road_demolition_cost: i32,
    /// `Angeschlossenumrechnung` (`MODATYP.DAT`, `array[-1..4] of Byte`):
    /// a house's [`House::connection`] by how far its nearest road is, 1
    /// to 4 squares, or -1 for none that close. // 1110:6508
    pub connection_values: [u8; 6],
    /// Sums over the whole island, mostly not known yet: `+0` (`Word`) the
    /// plots with an airport or harbour, `+2` the houses' capacity
    /// (`HAUSDATA.DAT` column 3), `+0x2e + 0x2c·class` and `+0xa6 +
    /// 4·class` the facilities', by 1010:2493's class (`Longint`s).
    /// // *1110:6f20, 0x19b bytes
    pub totals: Vec<u8>,
    /// The people and vehicles.
    pub people: People,
    /// The human's view, which the people code reads (marked).
    pub viewpoint: Viewpoint,
    /// The tables from the CD the simulation reads; empty until the game's
    /// data is loaded.
    pub tables: Arc<Tables>,
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
            flood: false,
            oil_slick: false,
            insured: false,
            damage: Damage::default(),
            events: Vec::new(),
            auction: Auction::default(),
            options: Options::default(),
            plots: Vec::new(),
            houses: Vec::new(),
            facilities: Vec::new(),
            building_sites: Vec::new(),
            computer_budgets: vec![0; COMPUTER_BUDGETS],
            books: Books::default(),
            renovation_years: [0, 1, 1, 1, 1, 1, 1, 1],
            renovation_cost: 0,
            scenario: 0,
            cheated: false,
            won: false,
            flag_133c: false,
            player_block: vec![0; PLAYER_BLOCK_SIZE],
            sabotage: Default::default(),
            championships: Default::default(),
            max_location_effect: 0,
            road_demolition_cost: 0,
            connection_values: [0; 6],
            totals: vec![0; TOTALS_SIZE],
            people: People::default(),
            viewpoint: Viewpoint::default(),
            tables: Arc::default(),
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

/// The other buildings (unit 1068:32b5).
fn facilities(save: &SaveGame) -> Result<Vec<Facility>, Error> {
    save.fields("*1110:6f14")
        .map(|r| (r.len() == FACILITY_SIZE).then(|| Facility(r.to_vec())))
        .collect::<Option<_>>()
        .ok_or(Error::Field("*1110:6f14"))
}

/// The computer players' budgets, `Longint`s.
fn computer_budgets(save: &SaveGame) -> Result<Vec<i32>, Error> {
    Ok(bytes::<{ COMPUTER_BUDGETS * 4 }>(save, "*1110:6512")?
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| i32::from_le_bytes(*b))
        .collect())
}

/// The building sites in the list's order: loading puts each record in
/// front of the ones before it. // 1068:304f
fn building_sites(save: &SaveGame) -> Result<Vec<BuildingSite>, Error> {
    let mut sites = save
        .fields("*1110:6f08")
        .map(|r| (r.len() == BUILDING_SITE_SIZE).then(|| BuildingSite::parse(r)))
        .collect::<Option<Vec<_>>>()
        .ok_or(Error::Field("*1110:6f08"))?;
    sites.reverse();
    Ok(sites)
}

/// The money menu's books (unit 1070:3bf8).
fn books(save: &SaveGame) -> Result<Books, Error> {
    let data: [u8; 29 * 4] = bytes(save, "[1110:6f04]+0x191")?;
    Ok(Books {
        year: std::array::from_fn(|n| {
            i32::from_le_bytes([data[4 * n], data[4 * n + 1], data[4 * n + 2], data[4 * n + 3]])
        }),
        history: bytes::<HISTORY_SIZE>(save, "*[1110:6f04]+0x24b")?.to_vec(),
    })
}

/// The people and their tables (unit 1088:8a07).
fn people(save: &SaveGame) -> Result<People, Error> {
    let records = People::parse_records(field(save, "*1110:6fd6")?).ok_or(Error::Field("*1110:6fd6"))?;
    let signed = |dest| Ok::<_, Error>(word(save, dest)?.cast_signed());
    Ok(People {
        records,
        paths: bytes::<PATHS_SIZE>(save, "*1110:6fda")?.to_vec(),
        entries: bytes::<ENTRIES_SIZE>(save, "*1110:6fde")?.to_vec(),
        tracks: bytes::<TRACKS_SIZE>(save, "*1110:6fe2")?.to_vec(),
        directions: bytes::<DIRECTIONS_SIZE>(save, "1110:3c2e")?,
        last: signed("1110:6ff4")?,
        ships: signed("1110:6fee")?,
        boats: signed("1110:6ff0")?,
        ..People::default()
    })
}

/// The human's view as the map saved it (units 1000:0c6a and 1048:2a19).
fn viewpoint(save: &SaveGame) -> Result<Viewpoint, Error> {
    let origin: [u8; 4] = bytes(save, "1110:5530")?;
    Ok(Viewpoint {
        rotation: byte(save, "1110:007c")?,
        origin: (
            i16::from_le_bytes([origin[0], origin[1]]),
            i16::from_le_bytes([origin[2], origin[3]]),
        ),
        parity: word(save, "1110:5534")?.cast_signed(),
    })
}

/// An `array[1..8] of Longint`.
fn longints(save: &SaveGame, dest: &'static str) -> Result<[i32; 8], Error> {
    let data: [u8; 32] = bytes(save, dest)?;
    Ok(std::array::from_fn(|i| {
        i32::from_le_bytes([data[4 * i], data[4 * i + 1], data[4 * i + 2], data[4 * i + 3]])
    }))
}

impl World {
    /// `player`'s reputation, 0–100, 50 at first. The original reads any
    /// byte from `+0x2f` on for players outside 1..8; the port says 0 past
    /// the block. // *1110:6f24 + 0x2f + player
    #[must_use]
    pub fn reputation(&self, player: u8) -> u8 {
        self.player_block.get(0x2f + usize::from(player)).copied().unwrap_or(0)
    }

    /// Sets `player`'s reputation; nothing past the block.
    pub fn set_reputation(&mut self, player: u8, value: u8) {
        if let Some(r) = self.player_block.get_mut(0x2f + usize::from(player)) {
            *r = value;
        }
    }

    /// The state `load_map` leaves behind, as far as it's ported.
    /// // 1048:2a19, 1000:0c6a
    ///
    /// # Errors
    /// If a field is missing or has the wrong size.
    #[allow(clippy::too_many_lines)] // One field after the other.
    pub fn from_map(save: &SaveGame) -> Result<Self, Error> {
        let mut world = World {
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
                location: LocationMap::from_bytes(field(save, "*1110:5524")?).ok_or(Error::Field("*1110:5524"))?,
                plot_map: PlotMap::from_bytes(field(save, "*1110:5528")?).ok_or(Error::Field("*1110:5528"))?,
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
            flood: byte(save, "1110:0086")? != 0,
            oil_slick: false,
            insured: byte(save, "1110:0072")? != 0,
            // Unit 10a0:23c9.
            damage: Damage {
                amount: longint(save, "1110:7050")?,
                insured: byte(save, "1110:7054")? != 0,
                slick: {
                    let d: [u8; 4] = bytes(save, "1110:7056")?;
                    (i16::from_le_bytes([d[0], d[1]]), i16::from_le_bytes([d[2], d[3]]))
                },
            },
            // Unit 1010:3e8a.
            events: parse_events(field(save, "*1110:6536")?).ok_or(Error::Field("*1110:6536"))?,
            // Unit 1028:2506.
            auction: Auction {
                price: longint(save, "1110:654a")?,
                start_price: longint(save, "1110:659c")?,
                item: word(save, "1110:656e")?.cast_signed(),
            },
            options: Options {
                animations: byte(save, "1110:0054")? != 0,
                people_move: byte(save, "1110:0055")? != 0,
                people_spawn: byte(save, "1110:0056")? == 1,
            },
            // Unit 1068:313e.
            plots: save
                .fields("*1110:6f0c")
                .map(|r| (r.len() == PLOT_SIZE).then(|| Plot::from_record(r.to_vec())))
                .collect::<Option<_>>()
                .ok_or(Error::Field("*1110:6f0c"))?,
            // Unit 1068:2e05.
            houses: save
                .fields("*1110:6f10")
                .map(|r| (r.len() == HOUSE_SIZE).then(|| House(r.to_vec())))
                .collect::<Option<_>>()
                .ok_or(Error::Field("*1110:6f10"))?,
            facilities: facilities(save)?,
            building_sites: building_sites(save)?,
            // Unit 1010:0d4c.
            computer_budgets: computer_budgets(save)?,
            books: books(save)?,
            renovation_years: bytes(save, "1110:1334")?,
            renovation_cost: longint(save, "1110:64c2")?,
            scenario: byte(save, "1110:006e")?,
            cheated: byte(save, "1110:006f")? != 0,
            won: byte(save, "1110:0071")? != 0,
            flag_133c: byte(save, "1110:133c")? != 0,
            // Unit 1078:34ad.
            player_block: bytes::<PLAYER_BLOCK_SIZE>(save, "*1110:6f24")?.to_vec(),
            // Unit 10a0:23c9.
            sabotage: parse_sabotage(field(save, "*1110:705a")?).ok_or(Error::Field("*1110:705a"))?,
            championships: parse_championships(field(save, "*1110:705e")?).ok_or(Error::Field("*1110:705e"))?,
            max_location_effect: byte(save, "1110:64bf")?,
            // Unit 1010:000e's values, saved with the map.
            road_demolition_cost: longint(save, "1110:64ea")?,
            connection_values: bytes(save, "1110:6508")?,
            // Unit 1078:34ad.
            totals: bytes::<TOTALS_SIZE>(save, "*1110:6f20")?.to_vec(),
            people: people(save)?,
            viewpoint: viewpoint(save)?,
            tables: Arc::default(),
            random: Random::default(),
        };
        world.link_plots();
        Ok(world)
    }

    /// The plot with the number `plot`, if there is one. // 1068:2591
    pub(crate) fn find_plot(&mut self, plot: u16) -> Option<&mut Plot> {
        self.plots
            .iter_mut()
            .find(|p| p.id() >= 0 && p.id().cast_unsigned() == plot)
    }

    /// The plot of the square `pos`, if there is one. // 1030:1e62, 1068:2591
    pub(crate) fn plot_of(&mut self, pos: Pos) -> Option<&mut Plot> {
        let plot = self.island.plot_at(pos);
        self.find_plot(plot)
    }

    /// Fills the plots' lists of houses and facilities from the lists,
    /// each in front of the ones before it, as loading a map does. The
    /// original writes through a nil pointer for a building on no plot.
    /// // 1068:2d49
    fn link_plots(&mut self) {
        for n in 0..self.houses.len() {
            let pos = self.houses[n].pos();
            if let Some(p) = self.plot_of(pos) {
                p.houses.insert(0, pos);
            }
        }
        for n in 0..self.facilities.len() {
            let pos = self.facilities[n].pos();
            if let Some(p) = self.plot_of(pos) {
                p.facilities.insert(0, pos);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plots_list_their_buildings() {
        let mut w = World::default();
        for i in 0..20 {
            for j in 0..20 {
                w.island.plot_map.set((i, j), 3);
            }
        }
        w.plots = vec![Plot::new(3)];
        w.houses = vec![House::new((1, 1)), House::new((2, 2)), House::new((30, 30))];
        w.facilities = vec![Facility::new(41, (5, 5))];
        w.link_plots();
        // Each in front of the ones before it; (30, 30) has no plot.
        assert_eq!(w.plots[0].houses, [(2, 2), (1, 1)]);
        assert_eq!(w.plots[0].facilities, [(5, 5)]);
    }

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
