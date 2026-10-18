//! The simulation's state: the island's map layers, the clock and date,
//! the players' money and the random numbers. The rest of the globals a
//! map file holds (`formats::savegame`) are kept as they were read, in
//! [`World::unported`], and join as the code that uses them is ported;
//! `crate::savegame` writes them all back.
//!
//! The original keeps each layer in a 63001-byte block on the heap, one far
//! pointer each at `1110:5500` to `1110:5520`, indexed `[i * 251 + j]`.
//! `load_map` (1048:2a19) reads them in its first unit routine (1000:0c6a).
//! The map's two axes have no names in the code; `i` is the one the
//! drawing multiplies by 251.

use std::sync::Arc;

use formats::savegame::{MAP_SIZE, SaveGame};
use vcl::random::Random;

use crate::auction::Auction;
use crate::computer::Computers;
use crate::events::{Damage, Event, parse_events};
use crate::monate::Date;
use crate::money::Finances;
use crate::people::{DIRECTIONS_SIZE, ENTRIES_SIZE, PATHS_SIZE, People, TRACKS_SIZE, Viewpoint};
use crate::sabotage::{Championship, Sabotage, parse_championships, parse_sabotage};
use crate::savegame::Fields;
use crate::settlement::{FACTORS, SUMS, Settlement};
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

    /// The layer as the map file stores it.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.0
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

    /// The byte at the flat index `i * 251 + j`, 0 outside the layer: what
    /// the original's unchecked index arithmetic reads for a `j` just off
    /// a row (the next or previous row's end).
    #[must_use]
    pub fn flat(&self, (i, j): Pos) -> u8 {
        usize::try_from(i32::from(i) * 251 + i32::from(j))
            .ok()
            .and_then(|n| self.0.get(n))
            .copied()
            .unwrap_or(0)
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

    /// The map as the map file stores it.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.0
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

    /// The table as the map file stores it, `Word`s.
    #[must_use]
    pub fn bytes(&self) -> Vec<u8> {
        self.0.iter().flat_map(|w| w.to_le_bytes()).collect()
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

    /// Every entry `from` becomes `to`. // 1068:27a5, 1068:2a2b
    pub fn replace(&mut self, from: u16, to: u16) {
        for entry in &mut self.0 {
            if *entry == from {
                *entry = to;
            }
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

    /// The record as the map file stores it.
    pub(crate) fn record(&self) -> &[u8] {
        &self.record
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

    pub fn set_id(&mut self, id: i16) {
        self.set_word(0, id);
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

    /// The plot's quality, 0–100 (`Gesamtqualitaet einer Insel`), which
    /// the settlement works out from its greenery, its shops' prices and
    /// its facilities (1008:0f37); scenario 6 averages it over the plots,
    /// weighted by [`Plot::area`]. // +4
    #[must_use]
    pub fn quality(&self) -> u8 {
        self.record[4]
    }

    /// The plot's area: a new island counts its corners above the sea
    /// (1068:25fd), and raising and lowering the land count up and down.
    /// What its greenery is measured against (1008:0f37), and the weight
    /// of [`Plot::quality`]. // +0x352
    #[must_use]
    pub fn area(&self) -> u16 {
        self.word(0x352).cast_unsigned()
    }

    pub fn set_area(&mut self, area: u16) {
        self.set_word(0x352, area.cast_signed());
    }

    /// The byte at `at`, 0 past the record. The settlement keeps its
    /// ratings there (`game::settlement`).
    #[must_use]
    pub fn byte(&self, at: usize) -> u8 {
        self.record.get(at).copied().unwrap_or(0)
    }

    /// Sets the byte at `at`; nothing past the record.
    pub fn set_byte(&mut self, at: usize, value: u8) {
        if let Some(b) = self.record.get_mut(at) {
            *b = value;
        }
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

    /// The record as the map file stores it.
    pub(crate) fn record(&self) -> &[u8] {
        &self.0
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

    /// Its stars (`Sterne`), 0–100: `HAUSDATA.DAT`'s column 4 for its
    /// type when it was built (1068:1a08), then each month from that, its
    /// condition, connection and location (1008:1bf7 step 0xc). Its value
    /// follows them. // +1
    #[must_use]
    pub fn stars(&self) -> u8 {
        self.0[1]
    }

    pub fn set_stars(&mut self, value: u8) {
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

    /// Its rent per guest and month: `HAUSPREI.DAT`'s third column when
    /// it was built; the computer players change theirs each month
    /// (1008:1bf7 step 0x82). // +0x12, `Longint`
    #[must_use]
    pub fn price(&self) -> i32 {
        longint_at(&self.0, 0x12)
    }

    pub fn set_price(&mut self, price: i32) {
        set_longint_at(&mut self.0, 0x12, price);
    }

    /// How fair its rent is for its stars, 0–100 (`Mietniveau`): 0x50
    /// when it's built (1068:1a08), then each month 1008:0863. // +0x16
    #[must_use]
    pub fn rent_rating(&self) -> u8 {
        self.0[0x16]
    }

    pub fn set_rent_rating(&mut self, value: u8) {
        self.0[0x16] = value;
    }

    /// Its guests, at most its capacity (`HAUSDATA.DAT` column 3): the
    /// share of its plot's guests the settlement gives it each month
    /// (step 0x16). Its rent is paid per guest; how much beach life it
    /// gets follows them, and with its capacity how likely it sends
    /// someone out ([`World::building_slice`]). // +0x10
    #[must_use]
    pub fn guests(&self) -> i16 {
        self.word(0x10)
    }

    pub fn set_guests(&mut self, value: i16) {
        self.set_word(0x10, value);
    }

    /// How many guests would come, at most its capacity (1008:1bf7 step
    /// 0x10). // +0xc, `Longint`
    #[must_use]
    pub fn demand(&self) -> i32 {
        longint_at(&self.0, 0xc)
    }

    pub fn set_demand(&mut self, value: i32) {
        set_longint_at(&mut self.0, 0xc, value);
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

    /// The record as the map file stores it.
    pub(crate) fn record(&self) -> &[u8] {
        &self.0
    }

    /// The building type. // +0
    #[must_use]
    pub fn building(&self) -> u8 {
        self.0[0]
    }

    pub fn set_building(&mut self, building: u8) {
        self.0[0] = building;
    }

    /// Like [`House::stars`]. // +1
    #[must_use]
    pub fn stars(&self) -> u8 {
        self.0[1]
    }

    pub fn set_stars(&mut self, value: u8) {
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

    /// Its visitors, at most its capacity: the share of the guests who
    /// want what it offers that it gets each month (1008:1bf7 step
    /// 0x44); scenario 5 counts them for the surf schools (0x6d).
    /// // +0xc, `Longint`
    #[must_use]
    pub fn visitors(&self) -> i32 {
        i32::from_le_bytes([self.0[0xc], self.0[0xd], self.0[0xe], self.0[0xf]])
    }

    pub fn set_visitors(&mut self, value: i32) {
        self.0[0xc..0x10].copy_from_slice(&value.to_le_bytes());
    }

    /// How full it is, in percent of its capacity (1008:1bf7 step 0x46):
    /// its income, and the chance that it sends someone out
    /// ([`World::building_slice`]). A full championship's
    /// building gets [`Facility::visitors`] of its capacity
    /// ([`World::replace_building`]). // +0x15
    #[must_use]
    pub fn fill(&self) -> u8 {
        self.0[0x15]
    }

    pub fn set_fill(&mut self, percent: u8) {
        self.0[0x15] = percent;
    }

    /// How many visitors would come, at most its capacity (1008:1bf7
    /// step 0x3a). // +0x10, `Longint`
    #[must_use]
    pub fn demand(&self) -> i32 {
        longint_at(&self.0, 0x10)
    }

    pub fn set_demand(&mut self, value: i32) {
        set_longint_at(&mut self.0, 0x10, value);
    }

    /// Which tenth of its class's range of prices (`MinPreis` to
    /// `MaxPreis`) its price per visitor falls in, 0–9 (1008:0907).
    /// // +0x14
    #[must_use]
    pub fn price_class(&self) -> u8 {
        self.0[0x14]
    }

    pub fn set_price_class(&mut self, class: u8) {
        self.0[0x14] = class;
    }

    /// How fair its price is for its stars, 0–100 (`Preisniveau`,
    /// 1008:0776). // +0x1a
    #[must_use]
    pub fn price_rating(&self) -> u8 {
        self.0[0x1a]
    }

    pub fn set_price_rating(&mut self, rating: u8) {
        self.0[0x1a] = rating;
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

    /// The record as `save_map` writes it; the far pointer to the next
    /// one (`+9`) is 0. // 1068:30d0
    pub(crate) fn record(&self) -> [u8; BUILDING_SITE_SIZE] {
        let mut r = [0; BUILDING_SITE_SIZE];
        r[0] = self.byte_0;
        r[1..3].copy_from_slice(&self.pos.0.to_le_bytes());
        r[3..5].copy_from_slice(&self.pos.1.to_le_bytes());
        r[5..7].copy_from_slice(&self.days.to_le_bytes());
        r[7] = self.building;
        r[8] = self.budget.cast_unsigned();
        r
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

impl Books {
    /// The books of a new game: the four rings of the history (120 months
    /// of a `Longint` for each player, then two `Longint`s and two bytes)
    /// empty but the last, at 50 for everyone, with the headers `1, -1,
    /// 1, 0`, `1, -1, 1, 0`, `2, 0, 1, 0` and `100, 1, 1, 0`; this and
    /// last year's sums 0, and kinds 0x6c to 0x6e player 1's `money`,
    /// `-loan` and their difference. // 1070:2e6c
    #[must_use]
    pub fn new_game(money: i32, loan: i32) -> Self {
        const RING: usize = 0xf0a;
        const HEADERS: [(i32, i32, u8); 4] = [(1, -1, 1), (1, -1, 1), (2, 0, 1), (100, 1, 1)];
        let mut history = vec![0; HISTORY_SIZE];
        for (ring, (a, b, c)) in HEADERS.into_iter().enumerate() {
            let base = ring * RING;
            let fill: i32 = if ring == 3 { 50 } else { 0 };
            for month in 0..120 {
                for player in 0..8 {
                    set_longint_at(&mut history, base + 0x20 * month + 4 * player, fill);
                }
            }
            set_longint_at(&mut history, base + 0xf00, a);
            set_longint_at(&mut history, base + 0xf04, b);
            history[base + 0xf08] = c;
            history[base + 0xf09] = 0;
        }
        let mut year = [0; 29];
        year[26] = money;
        year[27] = loan.wrapping_neg();
        year[28] = money.wrapping_sub(loan);
        Books { year, history }
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
    /// `Strassenbaukosten` (`MODATYP.DAT`): what a road square costs. The
    /// map file doesn't save it (unit 1010:0d4c skips it), so it stays
    /// what the game's data gave it. // 1110:64ee
    pub road_cost: i32,
    /// `SABOTAGE.TXT`'s nine kinds of sabotage, as far as the world needs
    /// them: the price and the building types piranhas go into. The rest
    /// is the form's ([`crate::sabowin`]). // *1110:65bc, 1028:361f
    pub sabotage: Vec<crate::sabotage::SabotageKind>,
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
    /// Each player's name, `array[0..8] of string`: `keiner` (nobody)
    /// for player 0, then players 1 to 8. // 1110:0902
    pub player_names: Vec<Vec<u8>>,
    /// 0 (easy) to 2 (hard). // 1110:008c
    pub difficulty: i16,
    /// The start menu's settings the island was made from: its shape,
    /// relief, how much land and how many rivers. Only the start menu
    /// reads them, when the player goes back to it. // 1110:008a,
    /// 1110:008e, 1110:0090, 1110:0092
    pub island_settings: [i16; 4],
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
    /// The game went without disasters for a while, which costs 10 points
    /// of the score ([`World::score`], 1010:0faa, its only reader). A
    /// game starts with it where the difficulty switches disasters off,
    /// on easy (1078:3271, and state 1500 for a scenario), and clears it
    /// on the other levels; switching them off on the options form sets
    /// it for good (1060:0637). Cleared on the way back to the start menu
    /// (1010:0f96). // 1110:133c
    pub played_without_disasters: bool,
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
    /// Two periods, in days, set when the game's data is loaded
    /// (`Random(2000) + 2000` and `Random(200) + 300`, 1078:075f): the
    /// economy's two waves (1078:0692), which the original only ever
    /// works out for day 0. // 1110:6fb0, 1110:6fb2
    pub economy_periods: [i16; 2],
    /// The people and vehicles.
    pub people: People,
    /// The monthly settlement's tables and sums.
    pub settlement: Settlement,
    /// The computer players take their turns (the option; on at first).
    /// // 1110:0057
    pub computers_play: bool,
    /// The computer players' turns so far, which picks whose turn it is.
    /// // 1110:70c2
    pub computer_turns: i32,
    /// The computer players' state (segment `10a8`).
    pub computers: Computers,
    /// `Fundamentbaukosten` (`MODATYP.DAT`): what a building on any land
    /// costs more for each square that isn't level. // 1110:64f6
    pub foundation_cost: i32,
    /// What raising and lowering the land costs (`MODATYP.DAT`).
    pub land_costs: crate::terrain::LandCosts,
    /// The human is dragging out a road (the road tool, 1080:3195): the
    /// human's UI state, but the computers don't build roads meanwhile,
    /// so it's the world's and changes by commands. Single human: one
    /// flag for player 1's drag. // 1110:634c
    pub road_dragging: bool,
    /// The ground the human's last road drag started on (1080:3195): the
    /// road planner builds a bridge from water (2, 3), and the computers'
    /// roads read it too, so it's the world's. Single human. // 1110:704e
    pub road_drag_ground: u8,
    /// That square's form; saved, never read. // 1110:704f
    pub road_drag_form: u8,
    /// What the human's build menu offers (`crate::build_menu`).
    pub build_menu: crate::build_menu::BuildMenu,
    /// The human's view, which the people code reads (marked).
    pub viewpoint: Viewpoint,
    /// The tables from the CD the simulation reads; empty until the game's
    /// data is loaded.
    pub tables: Arc<Tables>,
    /// `RandSeed`. Not in the map file: the original seeds it from the
    /// clock (`Randomize`, 1108:1e21) when it loads the game's data
    /// (1030:38a3), so whoever starts a game picks the seed.
    pub random: Random,
    /// The globals the map file holds that the port doesn't use yet, as
    /// they were read, for `save_map` to write back. Fields of zeros
    /// aren't kept; one that's missing is written as zeros
    /// (`crate::savegame`).
    pub unported: Fields,
    /// The human's money menu is shown (its height above 0, 1070:2617):
    /// while it is, every booking works out this year's total
    /// ([`World::geldmenu_refresh`]). The view's state, which the books
    /// follow, so it's the world's and changes by command
    /// (`crate::geldmenu::Action::Shown`). A map file has it as the
    /// form's saved window, whose height loading gives the form
    /// (1010:08d7). Single human: player 1's form.
    pub money_menu_shown: bool,
    /// The news archive (`crate::news`). Which news open the TV is the
    /// player's view's (`crate::optionen::Settings::news`).
    /// // *1110:663a, unit 1048:25ea
    pub news: crate::news::News,
    /// The fixes of the original's bugs this game plays with
    /// (`crate::enhance`): the same for every player, so in the hash, and
    /// saved in the port's chunk after the original's fields. A command
    /// changes them ([`crate::command::Action::SetFix`]). The default,
    /// and a file without the chunk, is none: the original.
    pub fixes: crate::enhance::Fixes,
    /// The language of the text the world makes: the computers' names
    /// (`NAMEN.TXT`), the news archive's dates (`MONATE.TXT` and the
    /// language's date format) and its other strings. The language a
    /// game was created in, the same for every player, so in the hash;
    /// saved in the port's chunk (`language=`, [`crate::lang::language_in`]).
    /// A file without it (an original's, an older port's) takes
    /// [`World::folder_language`]. Each player's view shows the rest in
    /// their own language (`docs/language.md`).
    pub language: crate::lang::Language,
    /// The language of the game folder's `MODATYP.DAT` (`land`,
    /// `1110:64be`), which a file without a language gets. Not in the map
    /// file: kept when one is loaded, like [`World::tables`].
    pub folder_language: crate::lang::Language,
}

impl Default for World {
    /// An empty island and the globals as `init_game_defaults` (1000:1ea0)
    /// and `monate_create` (1030:000d) leave them.
    fn default() -> Self {
        World {
            island: Island::empty(),
            player_colours: [1, 2, 3, 4, 5, 6, 7, 8],
            player_count: 8,
            player_names: vec![Vec::new(); 9],
            difficulty: 0,
            island_settings: [1, 2, 2, 5],
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
            played_without_disasters: false,
            player_block: vec![0; PLAYER_BLOCK_SIZE],
            sabotage: Default::default(),
            championships: Default::default(),
            max_location_effect: 0,
            road_demolition_cost: 0,
            connection_values: [0; 6],
            totals: vec![0; TOTALS_SIZE],
            economy_periods: [0; 2],
            people: People::default(),
            settlement: Settlement::default(),
            computers_play: true,
            computer_turns: 0,
            computers: Computers::default(),
            foundation_cost: 0,
            land_costs: crate::terrain::LandCosts::default(),
            road_dragging: false,
            road_drag_ground: crate::roads::NO_DRAG_GROUND,
            road_drag_form: 0,
            build_menu: crate::build_menu::BuildMenu::default(),
            viewpoint: Viewpoint::default(),
            tables: Arc::default(),
            random: Random::default(),
            unported: Fields::default(),
            money_menu_shown: false,
            news: crate::news::News::default(),
            fixes: crate::enhance::Fixes::ORIGINAL,
            language: crate::lang::Language::default(),
            folder_language: crate::lang::Language::default(),
        }
    }
}

/// The field stored at `dest`; when the map stores one twice (some of
/// `MODATYP.DAT`'s values), the later read wins, as in the original.
fn field<'a>(save: &'a SaveGame, dest: &'static str) -> Result<&'a [u8], Error> {
    save.fields(dest).last().ok_or(Error::Field(dest))
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
    // BUG(original): `save_map` writes them from the list's head, so a saved and loaded game has them the other way round.
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

/// `n` `Longint`s.
fn longint_array<const N: usize>(save: &SaveGame, dest: &'static str) -> Result<[i32; N], Error> {
    let data = field(save, dest)?;
    if data.len() != 4 * N {
        return Err(Error::Field(dest));
    }
    Ok(std::array::from_fn(|i| {
        i32::from_le_bytes([data[4 * i], data[4 * i + 1], data[4 * i + 2], data[4 * i + 3]])
    }))
}

/// The settlement's tables and sums (units 1008:539f and 1010:0d4c).
fn settlement(save: &SaveGame) -> Result<Settlement, Error> {
    let columns: [[i32; FACTORS]; 4] = [
        longint_array(save, "*1110:4c6e")?,
        longint_array(save, "*1110:4c72")?,
        longint_array(save, "*1110:4c76")?,
        longint_array(save, "*1110:4c7a")?,
    ];
    // MaxPreis has 11 values in the map; the 11th is the sums' first.
    let max_price: [i32; 11] = longint_array(save, "1110:4ac2")?;
    let month: [i32; SUMS - 1] = longint_array(save, "1110:4aee")?;
    let mut sums = [0; SUMS];
    sums[0] = max_price[10];
    sums[1..].copy_from_slice(&month);
    let mix = field(save, "*1110:6502")?;
    if mix.len() != 200 {
        return Err(Error::Field("*1110:6502"));
    }
    Ok(Settlement {
        factors: (0..FACTORS).map(|n| columns.map(|c| c[n])).collect(),
        min_rent: longint(save, "1110:4a8a")?,
        max_rent: longint(save, "1110:4a8e")?,
        rent_range: longint(save, "1110:4a92")?,
        min_price: longint_array(save, "1110:4a96")?,
        max_price: std::array::from_fn(|i| max_price[i]),
        sums,
        base_demand: byte(save, "1110:004c")?,
        advertising_cost: longint(save, "1110:64c6")?,
        living_costs: longint(save, "1110:64fa")?,
        class_mix: std::array::from_fn(|k| {
            std::array::from_fn(|c| i16::from_le_bytes([mix[20 * k + 2 * c], mix[20 * k + 2 * c + 1]]))
        }),
        mix_divisor: longint(save, "1110:64fe")?,
        decay: bytes(save, "1110:650e")?,
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

/// The news archive (unit 1048:25ea).
fn news(save: &SaveGame) -> Result<crate::news::News, Error> {
    let entries =
        crate::news::News::from_archive_bytes(field(save, "*1110:663a")?).ok_or(Error::Field("*1110:663a"))?;
    Ok(crate::news::News {
        entries,
        last: word(save, "1110:6664")?.cast_signed(),
        nothing_yet: byte(save, "1110:667c")? != 0,
        speaker: crate::news::Speaker::Off,
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
        // BUG(original): a player outside 1..8 reads past the reputations; the port says 0 past the block.
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
            player_names: field(save, "1110:0902")?
                .chunks(256)
                .map(|s| s[1..=usize::from(s[0]).min(s.len() - 1)].to_vec())
                .collect(),
            difficulty: word(save, "1110:008c")?.cast_signed(),
            island_settings: [
                word(save, "1110:008a")?.cast_signed(),
                word(save, "1110:008e")?.cast_signed(),
                word(save, "1110:0090")?.cast_signed(),
                word(save, "1110:0092")?.cast_signed(),
            ],
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
                guest_count: longints(save, "1110:6f90")?,
                max_debt: longint(save, "1110:64ce")?,
                interest_rate: longint(save, "1110:64d2")?,
                start_money: longint(save, "1110:64d6")?,
                credit_size: longint(save, "1110:64ca")?,
                credit_refused: bytes::<8>(save, "1110:6f28")?.map(|b| b != 0),
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
            // Unit 1028:2506. The form's state (`1110:6598`, and `659a`, the
            // form rolled up) is the view's.
            auction: Auction {
                price: longint(save, "1110:654a")?,
                start_price: longint(save, "1110:659c")?,
                item: word(save, "1110:656e")?.cast_signed(),
                bidder: word(save, "1110:6548")?.cast_signed(),
                colour: word(save, "1110:6596")?,
                going: word(save, "1110:65a0")?,
                ticks: longint(save, "1110:65a2")?,
                wait: longint(save, "1110:65a6")?,
                held: byte(save, "1110:6599")? != 0,
                ..Auction::default()
            },
            options: Options {
                animations: byte(save, "1110:0054")? != 0,
                people_move: byte(save, "1110:0055")? != 0,
                people_spawn: byte(save, "1110:0056")? == 1,
                auction_waits: byte(save, "1110:0063")? != 0,
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
            played_without_disasters: byte(save, "1110:133c")? != 0,
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
            economy_periods: [
                word(save, "1110:6fb0")?.cast_signed(),
                word(save, "1110:6fb2")?.cast_signed(),
            ],
            people: people(save)?,
            settlement: settlement(save)?,
            computers_play: byte(save, "1110:0057")? != 0,
            computer_turns: longint(save, "1110:70c2")?,
            // Unit 10a8:32ec.
            computers: Computers::from_map(save)?,
            foundation_cost: longint(save, "1110:64f6")?,
            land_costs: crate::terrain::LandCosts {
                from_water: longint(save, "1110:64da")?,
                raise: longint(save, "1110:64de")?,
                into_water: longint(save, "1110:64e2")?,
                lower: longint(save, "1110:64e6")?,
                palm: longint(save, "1110:64f2")?,
            },
            road_dragging: byte(save, "1110:634c")? != 0,
            // Unit 1080:3c6d.
            road_drag_ground: byte(save, "1110:704e")?,
            road_drag_form: byte(save, "1110:704f")?,
            build_menu: crate::build_menu::BuildMenu::from_map(save)?,
            viewpoint: viewpoint(save)?,
            tables: Arc::default(),
            random: Random::default(),
            unported: Fields::default(),
            // `load_window_positions` gives the money menu its saved
            // height (1010:08d7).
            money_menu_shown: crate::savegame::money_menu_shown(save).unwrap_or(false),
            news: news(save)?,
            // The port's chunk, if the file has one; else the original.
            // Whoever starts the game from a file without it gives the
            // player's defaults instead (`crate::enhance::fixes_in`).
            fixes: crate::enhance::fixes_in(&save.trailing, crate::enhance::Fixes::ORIGINAL),
            // The port's chunk's, if it has one; whoever loads a file
            // without it gives the folder's ([`World::load_map`]).
            language: crate::lang::language_in(&save.trailing).unwrap_or_default(),
            folder_language: crate::lang::Language::default(),
        };
        world.unported = crate::savegame::unported_world_fields(save);
        world.link_plots();
        if world.fixes.has(crate::enhance::Fix::LoadOrder) {
            world.turn_loaded_lists();
        }
        Ok(world)
    }

    /// Gives a world just loaded from a file ([`World::from_map`]) the
    /// fixes `fixes`, where the file's chunk didn't decide them (an
    /// original file plays with the player's): what loading did that
    /// depends on them follows (`fix.load_order`).
    pub fn set_loaded_fixes(&mut self, fixes: crate::enhance::Fixes) {
        let order = crate::enhance::Fix::LoadOrder;
        if self.fixes.has(order) != fixes.has(order) {
            self.turn_loaded_lists();
        }
        self.fixes = fixes;
    }

    /// The building sites and the plots' lists of houses and facilities
    /// the other way round: loading turns them round (1068:304f,
    /// 1068:2d49), and `fix.load_order` turns them back to the order the
    /// game that was saved had.
    fn turn_loaded_lists(&mut self) {
        self.building_sites.reverse();
        for p in &mut self.plots {
            p.houses.reverse();
            p.facilities.reverse();
        }
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

    /// The plots of a new island, from the plot map's `count` numbers:
    /// each record goes in front of the list, so the last comes first,
    /// with nothing on it and its ratings at 50. Then each corner above
    /// the sea counts in its square's plot's area (1030:1e62's plot), and
    /// each palm on it in its palms. // 1068:2bba, 1068:25fd
    pub fn create_plots(&mut self, count: u16) {
        self.plots = (1..=count)
            .rev()
            .map(|id| {
                let mut plot = Plot::new(id.cast_signed());
                for at in 3..=0x19 {
                    plot.set_byte(at, 50);
                }
                plot
            })
            .collect();
        // The counts by plot number, added up as the original goes.
        let mut counts = vec![(0u16, 0i16); usize::from(count) + 1];
        for j in 0..=250 {
            for i in 0..=250 {
                let pos = (i, j);
                let plot = self.island.plot_at(pos);
                if plot == 0 || self.island.heights.at(pos) == 0 {
                    continue;
                }
                // The original counts through a nil pointer for a number
                // without a record; every number has one here.
                if let Some(c) = counts.get_mut(usize::from(plot)) {
                    c.0 = c.0.wrapping_add(1);
                    if self.island.buildings.at(pos) == PALM {
                        c.1 = c.1.wrapping_add(1);
                    }
                }
            }
        }
        for p in &mut self.plots {
            let (area, palms) = counts[usize::from(p.id().cast_unsigned())];
            p.set_area(area);
            p.set_palms(palms);
        }
    }

    /// Fills the plots' lists of houses and facilities from the lists,
    /// each in front of the ones before it, as loading a map does. The
    /// original writes through a nil pointer for a building on no plot.
    /// // 1068:2d49
    fn link_plots(&mut self) {
        // BUG(original): a building on no plot is written through a nil pointer; skipped.
        // BUG(original): the plots' lists come out the other way round from those of the game that was saved.
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
