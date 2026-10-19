//! The people and vehicles on the island (segments `1088`, `1090` and
//! `1098`): guests, planes, ships, builders, surfers, spectators, the money
//! rising from a square. They're 1601 records of 0x28 bytes at
//! `*1110:6fd6`, saved with the map (unit 1088:8a07), in fixed ranges:
//!
//! | Records | What |
//! |---|---|
//! | 0–0x23 | The tools' pictures (1090:124f), the view's in the port (`crate::tools`); [`World::people_map_changed`] clears their kinds |
//! | 0x24–0x2e | The money rising from a square ([`World::float_money`]) |
//! | 0x2f–0x577 | Everyone else, up to [`People::last`] |
//! | 0x578–0x5dc | Planes and the helicopter ([`PLANE`], [`HELICOPTER`]) |
//! | 0x5dc–0x640 | The second half of each one's picture, 100 records on |
//!
//! `docs/people.md` has what each kind is, as far as it's known. The
//! original works out where the pictures go when it adds someone, so
//! several fields hold pixel offsets and sprite numbers (`ANIMDAT1.DAT`
//! rows); those depend on the human's view ([`Viewpoint`]), which is why
//! the world keeps a copy of it.
//!
//! The per-tick movement (`people_tick`, 1090:5293, with `person_animate`,
//! 1098:109f, and `person_step`, 1090:2ce5) is in [`movement`].
//!
//! With `fix.view_people` the people only a screen needs (spectators,
//! builders, taxiing planes, jetty boats, [`is_view_kind`]) are each
//! view's own ([`view`]), and the world's go by no view
//! ([`World::people_view`]); `docs/people.md`, "Who is whose".

// The map's `i` and `j`, and the record's number `n`, as everywhere else.
#![allow(clippy::many_single_char_names)]

mod movement;
pub mod view;

use std::sync::Arc;

use crate::enhance::Fix;
use crate::simulation::Tick;
use crate::world::{NO_BUILDING, Pos, World};

/// The number of records. // *1110:6fd6, 0xfa28 bytes
pub const PEOPLE: usize = 1601;
/// The size of a record.
pub const PERSON_SIZE: usize = 0x28;
/// The first record of the general range, which [`People::last`] closes.
pub const FIRST: i16 = 0x2f;
/// The end of the general range: [`People::last`] stays below it.
pub const GENERAL_END: i16 = 0x578;
/// The first plane record; its second half is [`PLANE_HALF`] records on.
pub const FIRST_PLANE: i16 = 0x578;
/// The last plane record.
pub const LAST_PLANE: i16 = 0x5dc;
/// How far a plane's second half is from the plane.
pub const PLANE_HALF: i16 = 100;
/// The money rising from a square ([`World::float_money`]) uses these.
pub const FIRST_MONEY: i16 = 0x24;
/// The last record of the money.
pub const LAST_MONEY: i16 = 0x2e;
/// The last record of all.
pub const LAST: i16 = 0x640;

/// The size of [`People::paths`] (`ANIMDAT2.DAT`'s 40 groups of 47 rows
/// of 4 bytes). // *1110:6fda
pub const PATHS_SIZE: usize = 0x1d60;
/// The size of [`People::entries`] (`NEINTRAG.DAT`). // *1110:6fde
pub const ENTRIES_SIZE: usize = 0x194;
/// The size of [`People::tracks`] (`ANIMDAT3.DAT`'s 16 groups of 31 rows
/// of 3 `Integer`s). // *1110:6fe2
pub const TRACKS_SIZE: usize = 0xba0;
/// The size of [`People::directions`]. // 1110:3c2e
pub const DIRECTIONS_SIZE: usize = 0x14;

/// A person's kind (`+0x17`): nobody.
pub const NOBODY: u8 = 0;
/// The money rising from a square.
pub const MONEY: u8 = 1;
/// A ship and a boat on the sea, which [`World::spawn_people`] adds.
pub const SHIP: u8 = 2;
pub const BOAT: u8 = 3;
/// The first of the three kinds of plane (7, 8, 9) landing at an airport
/// (building types 0x28–0x33); each has its second half at kind + 3.
pub const PLANE: u8 = 7;
/// Walkers along `ANIMDAT2.DAT` paths: kinds 0x11 to 0x16.
pub const WALKERS: std::ops::RangeInclusive<u8> = 0x11..=0x16;
/// Surfers (kinds 0x20 and 0x21): their home is the beach they came from.
pub const SURFERS: [u8; 2] = [0x20, 0x21];
/// Builders on a building site.
pub const BUILDER: u8 = 0x26;
/// Someone leaving a demolished building's square.
pub const LEAVING: u8 = 0x25;
/// The sightseeing helicopter (from building type 0x17, `H-Rundflüge`), in
/// a plane record, with kind 0x2f as its second half.
pub const HELICOPTER: u8 = 0x2e;
/// Spectators at a full tennis court.
pub const TENNIS_SPECTATORS: std::ops::RangeInclusive<u8> = 0x4d..=0x50;
/// Spectators at Miss Holiday Island's stage.
pub const STAGE_SPECTATORS: u8 = 0x41;
/// A boat at the `Bigport`'s jetties.
pub const JETTY_BOAT: u8 = 0x33;
/// Planes taxiing along an airport's runway.
pub const TAXIING: std::ops::RangeInclusive<u8> = 0x35..=0x3a;

/// Whether people of `kind` are a view's own with `fix.view_people`
/// ([`view`]): those [`World::spawn_people`] sends only where the view
/// looks, which nothing in the world reads but the spawning itself. The
/// rest stay in the world (`docs/people.md`, "Who is whose").
#[must_use]
pub fn is_view_kind(kind: u8) -> bool {
    kind == BUILDER
        || kind == JETTY_BOAT
        || kind == STAGE_SPECTATORS
        || TAXIING.contains(&kind)
        || TENNIS_SPECTATORS.contains(&kind)
}

/// One person or vehicle, the record as the map file stores it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Person(pub [u8; PERSON_SIZE]);

impl Default for Person {
    fn default() -> Self {
        Person([0; PERSON_SIZE])
    }
}

/// What 1088:0025 writes into a record: all of it but `+0xc`..`+0x13`.
#[derive(Debug, Clone, Copy, Default)]
struct Fill {
    pos: Pos,
    offset: (i16, i16),
    heading: i16,
    sprite: i16,
    flags: [u8; 3],
    kind: u8,
    path: i16,
    step: i32,
    target: Pos,
    home: Pos,
    word_26: i16,
}

impl Person {
    fn word(&self, at: usize) -> i16 {
        i16::from_le_bytes([self.0[at], self.0[at + 1]])
    }

    fn set_word(&mut self, at: usize, value: i16) {
        self.0[at..at + 2].copy_from_slice(&value.to_le_bytes());
    }

    /// Who or what it is, [`NOBODY`] for a free record. // +0x17
    #[must_use]
    pub fn kind(&self) -> u8 {
        self.0[0x17]
    }

    pub fn set_kind(&mut self, kind: u8) {
        self.0[0x17] = kind;
    }

    /// The square it's on. // +0, +2
    #[must_use]
    pub fn pos(&self) -> Pos {
        (self.word(0), self.word(2))
    }

    pub fn set_pos(&mut self, (i, j): Pos) {
        self.set_word(0, i);
        self.set_word(2, j);
    }

    /// Where its picture goes, in pixels from the square's. // +4, +6
    #[must_use]
    pub fn offset(&self) -> (i16, i16) {
        (self.word(4), self.word(6))
    }

    pub fn set_offset(&mut self, (x, y): (i16, i16)) {
        self.set_word(4, x);
        self.set_word(6, y);
    }

    /// Which way it goes, or which of its kind's routes it takes: 2 for a
    /// walker, 0x19 or 0x1f for a plane, 0–7 for the helicopter.
    /// // +8
    #[must_use]
    pub fn heading(&self) -> i16 {
        self.word(8)
    }

    pub fn set_heading(&mut self, heading: i16) {
        self.set_word(8, heading);
    }

    /// Its picture: an `ANIMDAT1.DAT` row. // +0xa
    #[must_use]
    pub fn sprite(&self) -> i16 {
        self.word(0xa)
    }

    pub fn set_sprite(&mut self, sprite: i16) {
        self.set_word(0xa, sprite);
    }

    /// Three flags, not known yet. `+0x15` is 0 for a plane that may be
    /// cleared off a demolished building's squares; `+0x16` is cleared and
    /// set again for most kinds by 1088:7ee3 and 1088:7f4a (drawn?).
    /// // +0x14, +0x15, +0x16
    #[must_use]
    pub fn flags(&self) -> [u8; 3] {
        [self.0[0x14], self.0[0x15], self.0[0x16]]
    }

    pub fn set_flags(&mut self, flags: [u8; 3]) {
        self.0[0x14..0x17].copy_from_slice(&flags);
    }

    /// For a walker, its `ANIMDAT2.DAT` path; for a ship, where it is in
    /// its animation. // +0x18
    #[must_use]
    pub fn path(&self) -> i16 {
        self.word(0x18)
    }

    pub fn set_path(&mut self, path: i16) {
        self.set_word(0x18, path);
    }

    /// A walker's step along its path, a plane's along its track; for most
    /// others a countdown; the money's amount. // +0x1a, `Longint`
    #[must_use]
    pub fn step(&self) -> i32 {
        i32::from_le_bytes([self.0[0x1a], self.0[0x1b], self.0[0x1c], self.0[0x1d]])
    }

    pub fn set_step(&mut self, step: i32) {
        self.0[0x1a..0x1e].copy_from_slice(&step.to_le_bytes());
    }

    /// Where it's going. // +0x1e, +0x20
    #[must_use]
    pub fn target(&self) -> Pos {
        (self.word(0x1e), self.word(0x20))
    }

    pub fn set_target(&mut self, (i, j): Pos) {
        self.set_word(0x1e, i);
        self.set_word(0x20, j);
    }

    /// Where it came from: a surfer's beach, a walker's square, a ship's
    /// corner of the map. // +0x22, +0x24
    #[must_use]
    pub fn home(&self) -> Pos {
        (self.word(0x22), self.word(0x24))
    }

    pub fn set_home(&mut self, (i, j): Pos) {
        self.set_word(0x22, i);
        self.set_word(0x24, j);
    }

    /// Not known yet: 0xdc for a new plane, its height?. // +0x26
    #[must_use]
    pub fn word_26(&self) -> i16 {
        self.word(0x26)
    }

    pub fn set_word_26(&mut self, value: i16) {
        self.set_word(0x26, value);
    }

    /// 1088:0025's stores, in its order.
    fn fill(&mut self, f: &Fill) {
        self.set_pos(f.pos);
        self.set_offset(f.offset);
        self.set_heading(f.heading);
        self.set_sprite(f.sprite);
        self.set_flags(f.flags);
        self.set_kind(f.kind);
        self.set_path(f.path);
        self.set_step(f.step);
        self.set_target(f.target);
        self.set_home(f.home);
        self.set_word_26(f.word_26);
    }
}

/// The people's state: the records and the tables the movement changes,
/// all but the last few saved with the map (unit 1088:8a07).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct People {
    /// The records. // *1110:6fd6
    pub records: Vec<Person>,
    /// `ANIMDAT2.DAT`, the walkers' paths: 40 groups of 47 rows of 4
    /// bytes (x, y, the way it faces, what happens there). The CD's file
    /// is loaded at the start (1088:80a7), then the map's copy over it.
    /// // *1110:6fda
    pub paths: Vec<u8>,
    /// `NEINTRAG.DAT`, 101 rows of 4 bytes. // *1110:6fde
    pub entries: Vec<u8>,
    /// `ANIMDAT3.DAT`, the planes' tracks: 16 groups of 31 rows of three
    /// `Integer`s (x, y, ?). // *1110:6fe2
    pub tracks: Vec<u8>,
    /// Ten `Integer`s, `1, 0, -1, 0, 0, -1, 0, 1, 0, 0` at first: the four
    /// directions? // 1110:3c2e
    pub directions: [u8; DIRECTIONS_SIZE],
    /// The last record of the general range in use: new people go into
    /// the first free record from [`FIRST`] up to it, and it grows when
    /// that's the last one, to at most [`GENERAL_END`]. Nobody is added
    /// while it's below [`FIRST`]. // 1110:6ff4
    pub last: i16,
    /// How many ships ([`SHIP`]) are on the sea, at most 10.
    /// // 1110:6fee
    pub ships: i16,
    /// How many boats ([`BOAT`]). // 1110:6ff0
    pub boats: i16,
    /// Not in the map: the runway of the next plane an airport sends out,
    /// set by [`World::spawn_people`] for the kinds 0x35–0x3a. // 1110:6fcc
    pub lane: i16,
    /// Not in the map, not known yet: cleared when the map changes.
    /// // 1110:6fce
    pub word_6fce: i16,
    /// Not in the map: the building [`World::remove_people_from`] last
    /// found, cleared when the map changes. // 1110:6ff2
    pub last_building: i16,
    /// Not in the map: the beach the next surfer comes from, -1 once a
    /// surfer took it. // 1110:7040, 1110:7042
    pub beach: Pos,
    /// Not in the map, not known yet: set when stage spectators come
    /// (kind 0x41), cleared by 1088:0002. // 1110:3c42
    pub flag_3c42: bool,
}

/// `1110:3c2e` as the program starts.
const DIRECTIONS: [i16; 10] = [1, 0, -1, 0, 0, -1, 0, 1, 0, 0]; // 1110:3c2e

impl Default for People {
    fn default() -> Self {
        let mut directions = [0; DIRECTIONS_SIZE];
        for (d, v) in directions.as_chunks_mut::<2>().0.iter_mut().zip(DIRECTIONS) {
            *d = v.to_le_bytes();
        }
        People {
            records: vec![Person::default(); PEOPLE],
            paths: vec![0; PATHS_SIZE],
            entries: vec![0; ENTRIES_SIZE],
            tracks: vec![0; TRACKS_SIZE],
            directions,
            last: 0,
            ships: 0,
            boats: 0,
            lane: 0,
            word_6fce: 0,
            last_building: 0,
            beach: (0, 0),
            flag_3c42: false,
        }
    }
}

/// The last row of each of `ANIMDAT3.DAT`'s tracks, which its loader
/// reads that many rows of (the first words of `1110:3620`).
/// // 1088:80a7
#[must_use]
pub fn track_last_rows() -> [i16; 16] {
    movement::TRACK_ENDS.map(|(last, _)| last)
}

impl People {
    /// No records and no tables, without allocating: what's left in a
    /// world while its people are lent out ([`World::placing`]), and the
    /// start of a view's own ([`view::ViewPeople`]).
    #[must_use]
    pub fn empty() -> Self {
        People {
            records: Vec::new(),
            paths: Vec::new(),
            entries: Vec::new(),
            tracks: Vec::new(),
            directions: [0; DIRECTIONS_SIZE],
            last: 0,
            ships: 0,
            boats: 0,
            lane: 0,
            word_6fce: 0,
            last_building: 0,
            beach: (0, 0),
            flag_3c42: false,
        }
    }

    /// The people as the game's data leaves them for a new game
    /// (1088:868c): the records 1 to 0x640 cleared, `+0x1a` 100, records
    /// 1 to 0x23 of kind 6 and every record's `+0x14` 1; nobody else
    /// yet ([`People::last`] 0x2f, no ships or boats). The tables come
    /// from `ANIMDAT2.DAT`, `NEINTRAG.DAT` and `ANIMDAT3.DAT` (1088:80a7),
    /// at 47 rows of 4 bytes, 4 bytes and 31 rows of 3 `Integer`s apiece.
    /// The original leaves the rest of the records (`+0x1e` on, and all
    /// of record 0 but `+0x14`) and the tracks' unused rows as the heap
    /// had them; the port has 0.
    #[must_use]
    pub fn new_game(paths: &[Vec<[u8; 4]>], entries: &[[u8; 4]], tracks: &[Vec<[i16; 3]>]) -> Self {
        // BUG(original): the rest of the records and the tracks' unused rows are left as the heap had them; the port has 0.
        let mut people = People::default();
        for (n, record) in people.records.iter_mut().enumerate() {
            let r = &mut record.0;
            if n >= 1 {
                r[0x1a..0x1c].copy_from_slice(&100i16.to_le_bytes());
                if n <= 0x23 {
                    r[0x17] = 6;
                }
            }
            r[0x14] = 1;
            r[0x15] = 0;
        }
        for (g, group) in paths.iter().enumerate().take(40) {
            for (row, cells) in group.iter().enumerate().take(47) {
                let at = g * 0xbc + row * 4;
                people.paths[at..at + 4].copy_from_slice(cells);
            }
        }
        for (row, cells) in entries.iter().enumerate().take(101) {
            people.entries[row * 4..row * 4 + 4].copy_from_slice(cells);
        }
        for (g, group) in tracks.iter().enumerate().take(16) {
            for (row, cells) in group.iter().enumerate().take(31) {
                for (k, v) in cells.iter().enumerate() {
                    let at = g * 0xba + row * 6 + k * 2;
                    people.tracks[at..at + 2].copy_from_slice(&v.to_le_bytes());
                }
            }
        }
        people.last = FIRST;
        people
    }

    /// The records from the map's block, 0x28 bytes each.
    #[must_use]
    pub fn parse_records(data: &[u8]) -> Option<Vec<Person>> {
        (data.len() == PEOPLE * PERSON_SIZE)
            .then(|| data.as_chunks::<PERSON_SIZE>().0.iter().map(|r| Person(*r)).collect())
    }

    /// Record `n`, if there is one.
    #[must_use]
    pub fn get(&self, n: i16) -> Option<&Person> {
        usize::try_from(n).ok().and_then(|n| self.records.get(n))
    }

    fn get_mut(&mut self, n: i16) -> Option<&mut Person> {
        usize::try_from(n).ok().and_then(|n| self.records.get_mut(n))
    }

    /// The kind of record `n`, [`NOBODY`] past the records.
    pub(crate) fn kind(&self, n: i16) -> u8 {
        self.get(n).map_or(NOBODY, Person::kind)
    }

    /// Byte 3 of row `row` of path `path` (what happens there), 0 past
    /// the table.
    fn path_byte(&self, path: i16, row: i32, at: usize) -> u8 {
        let index = i32::from(path) * 0xbc + row * 4 + i32::try_from(at).unwrap_or(0);
        usize::try_from(index)
            .ok()
            .and_then(|i| self.paths.get(i))
            .copied()
            .unwrap_or(0)
    }

    /// The `Integer` `at` of row `row` of track `track`, 0 past the table.
    fn track_word(&self, track: i16, row: i32, at: usize) -> i16 {
        let index = i32::from(track) * 0xba + row * 6 + i32::try_from(at).unwrap_or(0);
        usize::try_from(index)
            .ok()
            .and_then(|i| self.tracks.get(i..i + 2))
            .map_or(0, |b| i16::from_le_bytes([b[0], b[1]]))
    }
}

/// What the human's view looks at, as the original's people code sees it
/// (`TAnsichtart`, the map origin, the parity of the columns). The
/// original reads the player's view in the simulation: where new people
/// appear, which way a building's door faces, which pictures they get,
/// and whether a sound plays (with a `Random`) depend on it. Marked: it's
/// player 1's view, which the view hands the world with
/// [`World::set_viewpoint`]. Saved with the map. // 1110:007c, 1110:5530,
/// 1110:5534
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Viewpoint {
    /// The rotation, 0–3. // 1110:007c
    pub rotation: u8,
    /// The map position of the view's top left. // 1110:5530, 1110:5532
    pub origin: Pos,
    /// Whether the columns' halves start the other way round. // 1110:5534
    pub parity: i16,
}

impl Viewpoint {
    fn rotation(self) -> usize {
        usize::from(self.rotation & 3)
    }
}

/// Whether a view sees `(i, j)` on the main screen: the square's column
/// and half row, from the view's origin and rotation, against the 20
/// columns and the rows the square's height leaves (1050:21b7). The world
/// asks it of the human's view ([`World::viewpoint`]), the view of its own
/// camera. // 1050:245b
#[must_use]
pub fn sees(v: &Viewpoint, island: &crate::world::Island, pos: Pos) -> bool {
    sees_in(
        v,
        island,
        pos,
        crate::darstellung::COLUMNS,
        crate::darstellung::HALF_ROWS,
    )
}

/// [`sees`] for a view `columns` wide and `half_rows` high: the original's
/// 20 and 27 (1050:21b7 has `column_low <= 0x13` and `+ 0x2a`, 27 and the
/// 15 squares tall buildings reach up from below), or `big_map`'s bigger
/// view, which only the view's drawing asks about. The rotations' other
/// constants are where `DARSTELL.DAT`'s tables start, which a bigger
/// view's camera shares. // 1050:245b, 1050:21b7
#[must_use]
pub fn sees_in(v: &Viewpoint, island: &crate::world::Island, (i, j): Pos, columns: i16, half_rows: i16) -> bool {
    let (di, dj) = (i.wrapping_sub(v.origin.0), j.wrapping_sub(v.origin.1));
    let p = v.parity;
    let (column_low, column, row_low, row_high) = match v.rotation {
        0 => {
            let a = di.wrapping_sub(dj).wrapping_add(p);
            let r = di.wrapping_add(dj).wrapping_add(1);
            (a.wrapping_sub(1), a, r, r)
        }
        2 => {
            let t = di.wrapping_sub(dj).wrapping_add(p);
            let r = 0x1a_i16.wrapping_sub(di.wrapping_add(dj).wrapping_add(1));
            (0x13_i16.wrapping_sub(t), 0x13_i16.wrapping_sub(t.wrapping_sub(1)), r, r)
        }
        1 => {
            let a = di.wrapping_add(dj);
            let t = di.wrapping_sub(dj).wrapping_add(p).wrapping_sub(1).wrapping_neg();
            (a.wrapping_sub(1), a, t.wrapping_add(0x13), t.wrapping_add(0x12))
        }
        3 => {
            let t = di.wrapping_add(dj);
            let e = di.wrapping_sub(dj).wrapping_add(p);
            (
                0x19_i16.wrapping_sub(t),
                0x19_i16.wrapping_sub(t.wrapping_sub(1)),
                e,
                e.wrapping_sub(1),
            )
        }
        // BUG(original): a rotation past 3 leaves the locals unset; the port says false.
        _ => return false,
    };
    let h = i16::from(island.highest_corner((i, j)));
    // 1050:21b7
    column_low < columns && column >= 0 && h * 9 / 32 < row_low && row_high <= h * 9 / 32 + half_rows + 15
}

/// The squares [`World::spawn_people`] looks at, relative to the view's
/// origin: 11 rows of 61, `[e][l] = ((l + 1) div 2 + e, l div 2 - e)`.
/// The original fills the table at `1110:5536` this way when the game's
/// data is loaded (1030:2bdc), and saves it with the map, where it's the
/// same.
#[must_use]
pub fn view_square(e: i16, l: i16) -> Pos {
    ((l + 1) / 2 + e, l / 2 - e)
}

/// Where a building's door is from its first square, by the view's
/// rotation, for buildings 1, 2 and 3 squares wide. // 1110:34b0
const DOORS: [[Pos; 3]; 4] = [
    [(1, 1), (2, 2), (3, 3)],
    [(0, 1), (0, 2), (0, 3)],
    [(0, 0), (0, 0), (0, 0)],
    [(1, 0), (2, 0), (3, 0)],
];

/// A ship's and a boat's pictures, less 0x18. // 1110:3bb2
const SHIP_SPRITES: [i16; 50] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 24, 25, 26, 27, 28, 29, 30, 31, 32,
    33, 34, 35, 29, 29, 29, 29, 29, 28, 27, 26, 25, 24, 23, 23, 23, 23, 23, 23,
];

/// Where the ships (kinds 0x22–0x24) a harbour (building types 0x58–0x6b)
/// sends out head, from the harbour's square. // 1110:3a02 + 4 · building
const HARBOUR_TARGETS: [Pos; 20] = [
    (2, 0),
    (0, -1),
    (-1, 0),
    (0, 2),
    (2, 0),
    (0, -1),
    (-1, 0),
    (0, 2),
    (2, 0),
    (0, -1),
    (-1, 0),
    (0, 2),
    (2, 0),
    (0, -1),
    (-1, 0),
    (0, 2),
    (2, 0),
    (0, -1),
    (-1, 0),
    (0, 2),
];
/// The first building type of [`HARBOUR_TARGETS`].
const FIRST_HARBOUR: u8 = 0x58;

/// Where a jetty's boat (kind 0x33) is from the jetty's first square, for
/// building types 0x68–0x6b, by rotation: `[(building - 0x68) + 4 ·
/// rotation]`. // 1110:31e0 + 0x10 · rotation + 4 · building
const JETTY_OFFSETS: [Pos; 16] = [
    (0, 2),
    (2, 1),
    (1, 2),
    (2, 0),
    (0, 2),
    (0, 1),
    (1, 2),
    (0, 0),
    (0, 0),
    (0, 1),
    (1, 0),
    (0, 0),
    (0, 0),
    (2, 1),
    (1, 0),
    (2, 0),
];
/// The first building type of [`JETTY_OFFSETS`].
const FIRST_JETTY: u8 = 0x68;

/// Where the stage spectators (kind 0x41) stand from the stage's first
/// square, for building types 0x7b and 0x7c: `[(building - 0x7b) + 2 ·
/// rotation]`. // 1110:3a58 + 8 · rotation + 4 · building
const STAGE_OFFSETS: [Pos; 8] = [(0, 1), (1, 0), (0, 1), (0, 0), (0, 0), (0, 0), (0, 0), (1, 0)];
/// The first building type of [`STAGE_OFFSETS`].
const FIRST_STAGE: u8 = 0x7b;

/// The taxiing planes' pictures (kinds 0x35–0x3a) by rotation.
/// // 1110:3356 + 0xc · rotation + 2 · kind
const TAXIING_SPRITES: [[i16; 6]; 4] = [
    [478, 482, 486, 490, 494, 498],
    [482, 478, 490, 486, 498, 494],
    [478, 482, 486, 490, 494, 498],
    [482, 478, 490, 486, 498, 494],
];

/// An airport's (building types 0x28–0x33) two approaches: two sets of an
/// `i`, `j` offset and two values not known yet, around the heading of
/// its planes (0x19 or 0x1f). // 1110:36a0 + 0x12 · (building - 0x28)
const APPROACHES: [[i16; 9]; 12] = [
    [2, 1, 3, 5, 25, 2, 1, 3, 5],
    [1, 2, 1, 5, 31, 1, 2, 1, 5],
    [3, 2, 8, 11, 25, 3, 0, 0, 3],
    [2, 3, 2, 11, 31, 0, 3, 0, 9],
    [3, 1, 4, 7, 25, 3, 1, 4, 7],
    [1, 3, 1, 7, 31, 1, 3, 1, 7],
    [5, 2, 12, 17, 25, 5, 0, 0, 5],
    [2, 5, 2, 17, 31, 0, 5, 0, 15],
    [4, 2, 10, 14, 25, 4, 2, 10, 14],
    [2, 4, 2, 14, 31, 2, 4, 2, 14],
    [6, 4, 28, 34, 25, 6, 0, 0, 6],
    [4, 6, 4, 34, 31, 0, 6, 0, 30],
];
/// The first building type of [`APPROACHES`].
const FIRST_AIRPORT: u8 = 0x28;

/// A plane's or walker's looks by rotation, by the way it faces
/// (`ANIMDAT4.DAT`'s third column, `ANIMDAT2.DAT`'s third byte).
/// // 1110:33f0
const LOOKS: [[i16; 8]; 4] = [
    [0, 1, 2, 3, 4, 5, 6, 7],
    [6, 7, 0, 1, 2, 3, 4, 5],
    [4, 5, 6, 7, 0, 1, 2, 3],
    [2, 3, 4, 5, 6, 7, 0, 1],
];

/// Which `ANIMDAT3.DAT` track a plane's route uses, by rotation. // 1110:3430
const PLANE_TRACKS: [[i16; 16]; 4] = [
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
    [6, 7, 0, 1, 2, 3, 4, 5, 6, 14, 2, 0, 9, 4, 15, 12],
    [4, 5, 6, 7, 0, 1, 2, 3, 4, 15, 8, 6, 14, 2, 12, 9],
    [2, 3, 4, 5, 6, 7, 0, 1, 11, 12, 6, 4, 15, 0, 9, 14],
];

/// Which `ANIMDAT2.DAT` path a walker's path is, by rotation. // 1110:34e0
const PATH_TURNS: [[i16; 40]; 4] = [
    [
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29,
        30, 31, 32, 33, 34, 35, 36, 37, 38, 39,
    ],
    [
        2, 3, 1, 0, 6, 7, 8, 9, 10, 11, 4, 5, 14, 15, 17, 16, 19, 18, 12, 13, 22, 23, 21, 20, 26, 27, 25, 24, 30, 31,
        33, 32, 35, 34, 28, 29, 38, 39, 37, 36,
    ],
    [
        1, 0, 3, 2, 8, 9, 10, 11, 4, 5, 6, 7, 17, 16, 18, 19, 13, 12, 14, 15, 21, 20, 23, 22, 25, 24, 27, 26, 33, 32,
        34, 35, 29, 28, 30, 31, 37, 36, 39, 38,
    ],
    [
        3, 2, 0, 1, 10, 11, 4, 5, 6, 7, 8, 9, 18, 19, 12, 13, 15, 14, 17, 16, 23, 22, 20, 21, 27, 26, 24, 25, 34, 35,
        28, 29, 31, 30, 33, 32, 39, 38, 36, 37,
    ],
];

/// The map's corners, where vehicles come onto the island. // 1110:3ce4
const CORNERS: [Pos; 4] = [(0, 0), (0, 250), (250, 0), (250, 250)];

/// A flat square's form.
const FLAT: u8 = 16;
/// Water.
const WATER: u8 = 2;
/// The marker layers' value for nothing there.
pub(crate) const FREE: u8 = 0xff;

/// The door `n` of the building on `pos`, by the rotation `r`.
fn door_at(w: &World, r: usize, pos: Pos, n: usize) -> Pos {
    let (i, j) = w.building_origin(pos); // 1030:154c
    let d = DOORS[r][n];
    (d.0.wrapping_add(i), d.1.wrapping_add(j))
}

/// Where someone of `at` comes out of the building on `pos`, by the
/// rotation `r`.
fn place_at(w: &World, r: usize, at: At, pos: Pos) -> Pos {
    let offset = |table: &[Pos], first: u8, stride: usize| {
        let b = w.building_or_none(pos);
        b.checked_sub(first)
            .and_then(|n| table.get(usize::from(n) + stride * r))
            .copied()
            .unwrap_or((0, 0))
    };
    match at {
        At::Square => pos,
        At::Door(n) => door_at(w, r, pos, n),
        At::Jetty | At::Stage => {
            // BUG(original): other types read past the tables; the port says 0.
            let (di, dj) = if let At::Jetty = at {
                offset(&JETTY_OFFSETS, FIRST_JETTY, 4)
            } else {
                offset(&STAGE_OFFSETS, FIRST_STAGE, 2)
            };
            let (i, j) = w.building_origin(pos); // 1030:154c
            (di.wrapping_add(i), dj.wrapping_add(j))
        }
    }
}

/// The corner of the map nearest to `(i, j)`, the first of equals.
/// // 1098:00ef
fn nearest_corner((i, j): Pos) -> Pos {
    let mut best = 5000;
    let mut corner = CORNERS[0];
    for c in CORNERS {
        let d = i
            .wrapping_sub(c.0)
            .wrapping_abs()
            .wrapping_add(j.wrapping_sub(c.1).wrapping_abs());
        if d < best {
            best = d;
            corner = c;
        }
    }
    corner
}

/// Where someone comes out of a building, from its first square.
#[derive(Debug, Clone, Copy)]
enum At {
    /// The square the caller gives.
    Square,
    /// The building's first square plus its [`DOORS`] entry `n`.
    Door(usize),
    /// The first square plus the jetty's [`JETTY_OFFSETS`].
    Jetty,
    /// The first square plus the stage's [`STAGE_OFFSETS`].
    Stage,
}

/// The kinds 1088:0324 fills with one call of 1088:0025: where, the
/// countdown (`+0x1a`), the picture, its offset and the sound it makes
/// where the player sees it. The rest is 0, the flags 1, 0, 1.
#[derive(Debug, Clone, Copy)]
struct Simple {
    kind: u8,
    at: At,
    step: i32,
    sprite: i16,
    offset: (i16, i16),
    sound: Option<u8>,
}

const fn simple(kind: u8, at: At, step: i32, sprite: i16, offset: (i16, i16), sound: Option<u8>) -> Simple {
    Simple {
        kind,
        at,
        step,
        sprite,
        offset,
        sound,
    }
}

/// 1088:0324's cases that are a plain 1088:0025.
const SIMPLE: [Simple; 30] = [
    simple(0x27, At::Square, 10, 0x167, (0xe, 3), None),
    simple(0x28, At::Door(0), 0x20, 0x16c, (0x21, 5), None),
    simple(0x29, At::Square, 8, 0x174, (0x20, 3), None),
    simple(0x30, At::Square, 6, 0x1b0, (0x19, 0xf), None),
    simple(0x31, At::Door(0), 9, 0x1b6, (0x13, -2), None),
    simple(0x32, At::Door(1), 7, 0x1bf, (0x18, -0x16), None),
    simple(0x33, At::Jetty, 7, 0x1c6, (0x1a, -7), None),
    simple(0x34, At::Door(1), 0x11, 0x1cd, (-0x1d, -0x22), None),
    simple(0x3b, At::Door(0), 0x14, 0x1f6, (-0x10, -0x1d), Some(0x2a)),
    simple(0x3c, At::Door(1), 0x14, 0x1f8, (8, -0x59), None),
    simple(0x3d, At::Door(1), 0x14, 0x202, (7, -0xf), None),
    simple(0x3e, At::Door(2), 0x12, 0x20c, (0x36, -0x21), None),
    simple(0x3f, At::Door(0), 0x28, 0x215, (0, 2), None),
    simple(0x40, At::Door(1), 0x14, 0x21f, (10, 2), None),
    simple(0x41, At::Stage, 0xff, 0x229, (0x12, 0xc), None),
    simple(0x42, At::Square, 0xff, 0x233, (0x13, 0xc), None),
    simple(0x43, At::Square, 100, 0x23d, (0x19, 0xe), None),
    simple(0x44, At::Door(0), 100, 0x247, (2, -0x23), None),
    simple(0x45, At::Door(0), 10, 0x251, (0x10, -7), None),
    simple(0x47, At::Door(0), 10, 0x25b, (0x3b, 2), Some(0x28)),
    simple(0x48, At::Door(0), 10, 0x265, (-8, -4), Some(0x28)),
    simple(0x49, At::Door(0), 10, 0x26f, (0x31, -9), Some(0x28)),
    simple(0x4a, At::Door(0), 10, 0x279, (0x16, 0), Some(0x28)),
    simple(0x4b, At::Door(0), 10, 0x283, (0x1b, 0xb), Some(0x28)),
    simple(0x4c, At::Door(0), 10, 0x28d, (10, -0xc), Some(0x28)),
    simple(0x4d, At::Door(1), 0x3c, 0x297, (0, -0x37), None),
    simple(0x4e, At::Door(1), 0x3c, 0x2a1, (0x40, -0x17), None),
    simple(0x4f, At::Door(1), 0x3c, 0x297, (0, 9), None),
    simple(0x50, At::Door(1), 0x3c, 0x297, (0, -0x17), None),
    // The builders, with a sound of five.
    simple(BUILDER, At::Square, 0x14, 0x153, (6, -4), None),
];

/// Every sound number as a list of one, for [`World::sound_at`]'s
/// [`crate::simulation::PlacedSound`].
const ONE_SOUND: [[u8; 1]; 0x40] = {
    let mut all = [[0; 1]; 0x40];
    let mut n = 0;
    while n < 0x40 {
        #[allow(clippy::cast_possible_truncation)] // Below 0x40.
        {
            all[n] = [n as u8];
        }
        n += 1;
    }
    all
};

/// `sound` alone, as a list.
fn sound_list(sound: u8) -> &'static [u8] {
    ONE_SOUND.get(usize::from(sound)).map_or(&[], |s| &s[..])
}

/// What 1088:0324 fills in place for kinds 0x1a–0x1f (from the power
/// station, the sewage works and the zoo), from a door, with the
/// countdown, picture, offset and sound.
const DOOR_EFFECTS: [Simple; 6] = [
    simple(0x1a, At::Door(0), 0xc, 0xe3, (0x22, -13), Some(6)),
    simple(0x1b, At::Door(0), 0x28, 0xef, (6, -11), Some(0x1e)),
    simple(0x1c, At::Door(1), 8, 0x103, (0x32, -7), Some(0x19)),
    simple(0x1d, At::Door(1), 8, 0x10b, (0x37, -0x27), Some(7)),
    simple(0x1e, At::Door(1), 8, 0x113, (-0x24, -0x21), None),
    simple(0x1f, At::Door(1), 8, 0x11b, (9, -8), None),
];

/// The builders' sounds, by `Random(5)`. // 1088:0324
const BUILDER_SOUNDS: [u8; 5] = [0x13, 0x21, 0x26, 0x27, 0x20];

/// A walker's path by the road piece (ground) it starts on, 2 for those
/// not named. // 1088:0324
fn walker_path(ground: u8) -> Option<i16> {
    Some(match ground {
        10 | 0xc | 0xe | 0x10 | 0x19 | 0x1b | 0x1d | 0x1f => 2,
        0xb | 0xd | 0xf | 0x1a | 0x1c | 0x1e => 0,
        0x11 | 0x20 => 4,
        0x12 | 0x21 => 6,
        0x13 | 0x22 => 8,
        0x14 | 0x23 => 10,
        0x15 | 0x24 | 0x33 => 0xc,
        0x16 | 0x25 | 0x34 => 0xe,
        0x17 | 0x26 | 0x35 => 0x11,
        0x18 | 0x27 | 0x36 => 0x12,
        _ => return None,
    })
}

impl World {
    /// Hands the world the human's view, which the people code reads
    /// ([`Viewpoint`]). Marked: player 1's view; with several human
    /// clients `fix.view_people` and `fix.view_news` are on, and nothing
    /// reads it ([`World::reads_viewpoint`]).
    pub fn set_viewpoint(&mut self, viewpoint: Viewpoint) {
        self.viewpoint = viewpoint;
    }

    /// Whether anything in the world reads the viewpoint it was last
    /// handed ([`World::set_viewpoint`]): not with `fix.view_people` and
    /// `fix.view_news`, which several human clients have on
    /// (`crate::seats`), so the view doesn't hand it over then.
    #[must_use]
    pub fn reads_viewpoint(&self) -> bool {
        !(self.people_split() && self.fixes.has(Fix::ViewNews))
    }

    /// Whether the people are split between the world and the views
    /// (`fix.view_people`, [`view`]).
    #[must_use]
    pub fn people_split(&self) -> bool {
        self.fixes.has(Fix::ViewPeople)
    }

    /// The view the world's people go by: the human's
    /// ([`World::viewpoint`]) in the original; with `fix.view_people`
    /// nobody's, the first rotation from the map's corner, which each view
    /// turns its copy of the people from ([`World::people_for_view`]).
    #[must_use]
    pub fn people_view(&self) -> Viewpoint {
        // BUG(original): the people's squares, doors, pictures and a ship's drift go by the human's view (`fix.view_people`: by the first rotation).
        if self.people_split() {
            Viewpoint::default()
        } else {
            self.viewpoint
        }
    }

    /// Lends the people out to a [`Placer`] turned like
    /// [`World::people_view`]. Meanwhile the world's own are empty.
    fn placing<R>(&mut self, f: impl FnOnce(&mut Placer<'_>) -> R) -> R {
        let mut people = std::mem::replace(&mut self.people, People::empty());
        let r = self.people_view().rotation();
        let out = f(&mut Placer {
            w: self,
            people: &mut people,
            r,
        });
        self.people = people;
        out
    }

    /// The building on `pos`, [`NO_BUILDING`] unless both coordinates are
    /// 1..=249. // 1000:3dc9
    pub(crate) fn building_or_none(&self, (i, j): Pos) -> u8 {
        if (1..=249).contains(&i) && (1..=249).contains(&j) {
            self.island.buildings.at((i, j))
        } else {
            NO_BUILDING
        }
    }

    /// The part of `pos`, 0 unless both coordinates are 1..=249.
    /// // 1000:3ed1
    pub(crate) fn part_or_zero(&self, (i, j): Pos) -> u8 {
        if (1..=249).contains(&i) && (1..=249).contains(&j) {
            self.island.parts.at((i, j))
        } else {
            0
        }
    }

    /// `layer_551c` on `pos`, 0 unless both coordinates are 1..=249.
    /// // 1000:3d54 with 0
    fn marker_or_zero(&self, (i, j): Pos) -> u8 {
        if (1..=249).contains(&i) && (1..=249).contains(&j) {
            self.island.layer_551c.at((i, j))
        } else {
            0
        }
    }

    /// Sets `layer_551c` on `pos`. The original writes outside the layer
    /// for a square off the map; the port doesn't.
    pub(crate) fn set_marker(&mut self, pos: Pos, value: u8) {
        // BUG(original): a square off the map is written outside the layer; the port writes nothing.
        if self.island.layer_551c.get(pos).is_some() {
            self.island.layer_551c.set(pos, value);
        }
    }

    /// Sets `layer_5520` on `pos`, likewise.
    fn set_marker_2(&mut self, pos: Pos, value: u8) {
        if self.island.layer_5520.get(pos).is_some() {
            self.island.layer_5520.set(pos, value);
        }
    }

    /// Whether both marker layers are free on `pos`; off the map counts
    /// as free (the original reads outside the layers).
    fn markers_free(&self, pos: Pos) -> bool {
        // BUG(original): off the map is read outside the layers; the port counts it free.
        self.island.layer_551c.get(pos).unwrap_or(FREE) == FREE
            && self.island.layer_5520.get(pos).unwrap_or(FREE) == FREE
    }

    /// Whether the player sees `(i, j)` on the main screen ([`sees`]).
    /// Marked: player 1's view. // 1050:245b
    fn visible(&self, pos: Pos) -> bool {
        sees(&self.viewpoint, &self.island, pos)
    }

    /// A sound from `pos`, if the player sees it. The original plays it
    /// (`play_sound`, 10a0:30b6); the port hands it to the view.
    fn sound_at(&self, pos: Pos, sound: u8, tick: &mut Tick) {
        // BUG(original): the world decides by the human's view whether a sound plays (`fix.view_people`: each view does).
        if self.people_split() {
            self.placed_sound(pos, sound_list(sound), tick);
        } else if self.visible(pos) {
            tick.sounds.push(sound);
        }
    }

    /// With `fix.view_people`: one of `sounds` from `pos`, which each view
    /// that sees it plays, picked with its own random numbers.
    fn placed_sound(&self, at: Pos, sounds: &'static [u8], tick: &mut Tick) {
        debug_assert!(self.people_split());
        tick.placed_sounds.push(crate::simulation::PlacedSound {
            at,
            sounds,
            after: tick.sounds.len(),
        });
    }

    /// Whether someone of `kind` is on `pos`. For the walkers: whether
    /// either marker layer is taken there. // 1088:4ae2
    fn person_at(&self, kind: u8, pos: Pos) -> bool {
        if WALKERS.contains(&kind) {
            return !self.markers_free(pos);
        }
        let mut found = false;
        if self.people.last > LAST_MONEY {
            for n in FIRST..=self.people.last {
                if let Some(p) = self.people.get(n)
                    && p.kind() == kind
                    && p.pos() == pos
                {
                    found = true;
                }
            }
        }
        found
    }

    /// Whether anyone is on `pos`, in the general range. // 1088:4b93
    pub(crate) fn anyone_at(&self, pos: Pos) -> bool {
        let mut found = false;
        if self.people.last > LAST_MONEY {
            for n in FIRST..=self.people.last {
                if let Some(p) = self.people.get(n)
                    && p.kind() != NOBODY
                    && p.pos() == pos
                {
                    found = true;
                }
            }
        }
        found
    }

    /// Where someone of `at` comes out of the building on `pos`, by the
    /// rotation the people go by ([`World::people_view`]).
    fn place(&self, at: At, pos: Pos) -> Pos {
        place_at(self, self.people_view().rotation(), at, pos)
    }

    /// Someone of `kind` comes onto `pos`: into the first free record of
    /// the planes' range for the planes and the helicopter, else of the general range
    /// ([`People::last`]). Most kinds only come while people move
    /// (`1110:0055`); where the player sees them, some make a sound, a few
    /// of them picked with a `Random`. // 1088:0324, 10464 bytes
    pub(crate) fn add_person(&mut self, kind: u8, pos: Pos, tick: &mut Tick) {
        if [PLANE, PLANE + 1, PLANE + 2, HELICOPTER].contains(&kind) {
            if self.options.people_move {
                self.add_plane(kind, pos);
            }
            return;
        }
        let last = self.people.last;
        let mut grew = false;
        if last <= LAST_MONEY {
            return;
        }
        let mut n = FIRST;
        loop {
            if self.people.kind(n) == NOBODY && n < GENERAL_END && self.people.get(n).is_some() {
                if self.options.people_move {
                    self.fill_person(n, kind, pos, tick);
                }
                if kind != 6 {
                    self.fill_late(n, kind, pos, tick);
                }
                if n == self.people.last {
                    grew = true;
                }
                n = self.people.last;
            }
            if n >= last {
                break;
            }
            n += 1;
        }
        if grew && self.people.last < GENERAL_END {
            self.people.last += 1;
        }
    }

    /// A plane (kinds 7, 8, 9) or the helicopter ([`HELICOPTER`]) in the
    /// first free plane record, with its second half 100 records on. // 1088:0324
    fn add_plane(&mut self, kind: u8, pos: Pos) {
        for n in FIRST_PLANE..=LAST_PLANE {
            if self.people.kind(n) != NOBODY {
                continue;
            }
            if kind == HELICOPTER {
                self.add_helicopter(n, pos);
            } else {
                self.add_plane_at(n, kind, pos);
            }
            return;
        }
    }

    /// The helicopter on a random route (`Random(8)`). // 1088:0324
    fn add_helicopter(&mut self, n: i16, pos: Pos) {
        let route = self.random.random(8).cast_signed();
        let Some(p) = self.people.get_mut(n) else {
            return;
        };
        p.set_target((3, p.target().1));
        p.set_kind(HELICOPTER);
        p.set_pos(pos);
        p.set_flags([1, 1, 1]);
        p.set_heading(route);
        p.set_path(0x14);
        // Its first step: 1110:3622 + 4 · route.
        let step = usize::try_from(route)
            .ok()
            .and_then(|r| movement::TRACK_ENDS.get(r))
            .map_or(0, |e| e.1);
        p.set_step(i32::from(step));
        p.set_sprite(0x198);
        p.set_word_26(0);
        if let Some(q) = self.people.get_mut(n + PLANE_HALF) {
            q.set_kind(HELICOPTER + 1);
            q.set_pos(pos);
            q.set_heading(route);
            q.set_offset((0, 0));
            q.set_path(0);
            q.set_step(0);
            q.set_flags([1, 1, 1]);
        }
        self.place_plane(n);
        self.set_marker(pos, 1);
    }

    /// A plane of `kind` for the airport on `pos`, coming from the map's
    /// corner nearest to where it's going, with its second half (kind + 3),
    /// from a height of 220. Two of the headings pick one of the airport's
    /// approaches with a `Random(2)`. // 1088:0324
    fn add_plane_at(&mut self, n: i16, kind: u8, (i, j): Pos) {
        let b = self.building_or_none((i, j));
        let park = b
            .checked_sub(FIRST_AIRPORT)
            .and_then(|k| APPROACHES.get(usize::from(k)))
            .copied();
        // BUG(original): other types read past the table; the port says 0.
        let park = park.unwrap_or([0; 9]);
        let heading = park[4];
        let height = i16::from(self.island.highest_corner((i, j)));
        let lift = (height.wrapping_sub(1).wrapping_mul(-9).wrapping_add(0xdc)) / 0x18;
        let mut target = None;
        if heading == 0x19 || heading == 0x1f {
            let way = if self.random.random(2) == 1 { 0 } else { 5 };
            let (di, dj) = (park[way], park[way + 1]);
            let (ti, tj) = if heading == 0x19 {
                (lift.wrapping_add(i).wrapping_add(di), dj.wrapping_add(j))
            } else {
                (di.wrapping_add(i), lift.wrapping_add(j).wrapping_add(dj))
            };
            target = Some(((ti, tj), (di.wrapping_add(i), dj.wrapping_add(j))));
        }
        let Some(p) = self.people.get_mut(n) else {
            return;
        };
        p.set_heading(heading);
        if let Some((t, h)) = target {
            p.set_target(t);
            p.set_home(h);
        }
        p.set_kind(kind);
        let start = nearest_corner(p.target()); // 1098:00ef
        p.set_pos(start);
        p.set_offset((0, -5));
        p.set_flags([1, 1, 1]);
        p.set_path(0);
        p.set_step(0);
        let sprite = (i16::from(kind) - 7) * 8 + 0x6b + p.path();
        p.set_sprite(sprite);
        p.set_word_26(0xdc);
        let (kind, pos, path) = (p.kind(), p.pos(), p.path());
        if let Some(q) = self.people.get_mut(n + PLANE_HALF) {
            q.set_kind(kind + 3);
            q.set_pos(pos);
            q.set_heading(heading);
            q.set_offset((0, 0));
            q.set_path(0);
            q.set_sprite((i16::from(kind) - 7) * 8 + 0x83 + path);
            q.set_step(0);
            q.set_flags([1, 1, 1]);
        }
    }
}

/// Works out where the people's pictures go for a rotation: the parts of
/// the people code that only place them, on the world's people
/// ([`World::placing`]), a view's copy of them
/// ([`World::people_for_view`]) or a view's own ([`view::ViewPeople`]).
pub(crate) struct Placer<'a> {
    /// The world, for the island and the tables; its people aren't read.
    pub w: &'a World,
    pub people: &'a mut People,
    /// The rotation, 0–3.
    pub r: usize,
}

impl Placer<'_> {
    pub(crate) fn person(&self, n: i16) -> Person {
        self.people.get(n).cloned().unwrap_or_default()
    }

    pub(crate) fn edit(&mut self, n: i16, f: impl FnOnce(&mut Person)) {
        if let Some(p) = self.people.get_mut(n) {
            f(p);
        }
    }

    /// Where someone of `at` comes out of the building on `pos`.
    fn place(&self, at: At, pos: Pos) -> Pos {
        place_at(self.w, self.r, at, pos)
    }

    /// Works out a plane's picture and where it goes from its track, for
    /// both of its halves. // 1088:75de
    pub(crate) fn place_plane(&mut self, n: i16) {
        let r = self.r;
        let t = Arc::clone(&self.w.tables);
        let Some(p) = self.people.get(n).cloned() else {
            return;
        };
        let (step, heading, word_26) = (p.step(), p.heading(), p.word_26());
        // The step's low word picks the row.
        #[allow(clippy::cast_possible_truncation)]
        let row = step as i16;
        let look = t.animdat4(heading, row, 2);
        let look = usize::try_from(look)
            .ok()
            .and_then(|l| LOOKS[r].get(l))
            .copied()
            .unwrap_or(0);
        let odd = step.wrapping_add(i32::from(word_26)) & 1;
        #[allow(clippy::cast_possible_truncation)]
        let sprite = i32::from(look.wrapping_add(0x198)).wrapping_add(odd * 8) as i16;
        let (w, h) = (t.animdat1(sprite, 1) / 2, t.animdat1(sprite, 0) / 2);
        let track = usize::try_from(heading)
            .ok()
            .and_then(|h| PLANE_TRACKS[r].get(h))
            .copied()
            .unwrap_or(0);
        let x = self.people.track_word(track, i32::from(row), 0);
        let y = self.people.track_word(track, i32::from(row), 2);
        let ground = i16::from(self.w.island.highest_corner(p.pos()));
        if let Some(p) = self.people.get_mut(n) {
            p.set_sprite(sprite);
            p.set_offset((x.wrapping_sub(w), p.offset().1));
            let dy = y
                .wrapping_sub(word_26)
                .wrapping_sub(5)
                .wrapping_add(ground)
                .wrapping_sub(h);
            p.set_offset((p.offset().0, dy));
        }
        let half = look.wrapping_add(0x1a8);
        if let Some(q) = self.people.get_mut(n + PLANE_HALF) {
            q.set_sprite(half);
            q.set_offset((x.wrapping_sub(w), y.wrapping_sub(h).wrapping_add(5)));
        }
    }

    /// Works out a walker's picture and where it goes from its path.
    /// // 1088:4c55
    pub(crate) fn place_walker(&mut self, n: i16) {
        let r = self.r;
        let t = Arc::clone(&self.w.tables);
        let Some(p) = self.people.get(n).cloned() else {
            return;
        };
        // The step's low word picks the row.
        #[allow(clippy::cast_possible_truncation)]
        let row = i32::from(p.step() as i16);
        let facing = i16::from(self.people.path_byte(p.path(), row, 2));
        let look = usize::try_from(facing)
            .ok()
            .and_then(|f| LOOKS[r].get(f))
            .copied()
            .unwrap_or(0);
        let sprite = look
            .wrapping_add(0x9b)
            .wrapping_add((i16::from(p.kind()) - 0x11).wrapping_mul(8));
        let (w, h) = (t.animdat1(sprite, 1) / 2, t.animdat1(sprite, 0) / 2);
        let path = usize::try_from(p.path())
            .ok()
            .and_then(|k| PATH_TURNS[r].get(k))
            .copied()
            .unwrap_or(0);
        let x = i16::from(self.people.path_byte(path, row, 0)).wrapping_sub(w);
        let mut y = i16::from(self.people.path_byte(path, row, 1)).wrapping_sub(h);
        // On a bridge over the sea it's a step up.
        if [0x60, 0x61, 99, 100].contains(&self.w.ground_or_water(p.pos())) {
            y = y.wrapping_sub(9);
        }
        if let Some(p) = self.people.get_mut(n) {
            p.set_sprite(sprite);
            p.set_offset((x, y));
        }
    }

    /// Where a taxiing plane's picture goes, by the rotation and the
    /// runway it took (`+0x20`). // 1098:0745, 1098:0a14, 1098:0ce3, 1098:0e6d
    pub(crate) fn turn_taxiing(&mut self, n: i16, table: &TaxiingOffsets) {
        let r = self.r;
        if let Some(p) = self.people.get_mut(n)
            && let Some(&offset) = usize::try_from(p.target().1)
                .ok()
                .and_then(|lane| table[r].get(lane))
                .and_then(Option::as_ref)
        {
            p.set_offset(offset);
        }
    }
}

impl World {
    /// [`Placer::place_plane`] on the world's people.
    fn place_plane(&mut self, n: i16) {
        self.placing(|p| p.place_plane(n));
    }

    /// [`Placer::place_walker`] on the world's people.
    fn place_walker(&mut self, n: i16) {
        self.placing(|p| p.place_walker(n));
    }

    /// [`Placer::turn_taxiing`] on the world's people.
    fn turn_taxiing(&mut self, n: i16, table: &TaxiingOffsets) {
        self.placing(|p| p.turn_taxiing(n, table));
    }

    /// Record `n`, which [`World::add_person`] found free.
    fn free_record(&mut self, n: i16) -> &mut Person {
        self.people.get_mut(n).expect("a free record")
    }

    /// 1088:0324's cases, which only run while people move. Kinds without
    /// a case leave the record as it is.
    #[allow(clippy::too_many_lines)] // One case of the original's for each kind.
    fn fill_person(&mut self, n: i16, kind: u8, (i, j): Pos, tick: &mut Tick) {
        let r = self.people_view().rotation();
        let ship_sprite = |k: i16| {
            usize::try_from(k)
                .ok()
                .and_then(|k| SHIP_SPRITES.get(k))
                .copied()
                .unwrap_or(0)
                .wrapping_add(0x18)
        };
        match kind {
            SHIP | BOAT | 4 => {
                let p = self.free_record(n);
                p.set_offset((0, 0));
                p.set_kind(kind);
                p.set_pos((i, j));
                p.set_flags([1, 0, 1]);
                let sum = i.wrapping_add(j);
                match kind {
                    SHIP => {
                        let path = 0x1c - sum % 0x1c;
                        p.set_path(path);
                        p.set_step(i32::from(path) + 700);
                        p.set_sprite(ship_sprite((path.wrapping_add(sum)) % 0x1c + 0x16));
                    }
                    BOAT => {
                        p.set_path(0);
                        p.set_step(0xff);
                        p.set_sprite(ship_sprite(sum % 0xc + 10));
                    }
                    _ => {
                        p.set_path(0);
                        p.set_step(0xff);
                        p.set_sprite(ship_sprite(sum % 10));
                    }
                }
                if kind == SHIP {
                    self.sound_at((i, j), 1, tick);
                }
                if kind != 4 {
                    self.set_marker((i, j), kind);
                }
            }
            0x11..=0x16 => {
                if self.person_at(kind, (i, j)) {
                    return;
                }
                let p = self.free_record(n);
                p.set_kind(kind);
                p.set_pos((i, j));
                p.set_sprite((i16::from(kind) - 0x11) * 8 + 0x9d);
                p.set_heading(2);
                p.set_path(2);
                p.set_step(0);
                let x = i16::from(self.people.path_byte(2, 0, 0));
                let y = i16::from(self.people.path_byte(2, 0, 1));
                let ground = self.ground_or_water((i, j));
                let p = self.free_record(n);
                p.set_offset((x, y));
                if let Some(path) = walker_path(ground) {
                    p.set_path(path);
                }
                p.set_flags([1, 0, 1]);
                let marker = p.path().to_le_bytes()[0];
                self.set_marker((i, j), marker);
                self.place_walker(n);
                let home = self.people.beach;
                self.free_record(n).set_home(home);
                self.people.beach.0 = -1;
            }
            0x1a..=0x1f => {
                let v = DOOR_EFFECTS[usize::from(kind - 0x1a)];
                let pos = self.place(v.at, (i, j));
                let p = self.free_record(n);
                p.set_kind(kind);
                p.set_pos(pos);
                p.set_offset(v.offset);
                p.set_flags([1, 0, 1]);
                p.set_path(0);
                p.set_step(v.step);
                p.set_sprite(v.sprite);
                if self.people_split() {
                    // The building's square, which each view turns the
                    // door from ([`Placer::place_again_from_home`]).
                    self.free_record(n).set_home((i, j));
                }
                if let Some(s) = v.sound {
                    self.sound_at(pos, s, tick);
                }
            }
            0x20 | 0x21 => {
                let p = self.free_record(n);
                p.set_offset((0, 0));
                p.set_kind(kind);
                p.set_pos((i, j));
                p.set_flags([1, 0, 1]);
                p.set_path(0);
                p.set_step(0);
                p.set_sprite(if kind == 0x20 { 0x123 } else { 299 });
                p.set_home((i, j));
                let di = self.random.random(5).cast_signed();
                let p = self.free_record(n);
                p.set_target((p.pos().0.wrapping_add(di).wrapping_sub(2), p.target().1));
                let dj = self.random.random(5).cast_signed();
                let p = self.free_record(n);
                p.set_target((p.target().0, p.pos().1.wrapping_add(dj).wrapping_sub(2)));
                self.set_marker((i, j), kind);
                let home = self.people.beach;
                self.free_record(n).set_home(home);
                self.people.beach.0 = -1;
            }
            0x22..=0x24 => {
                let corner = nearest_corner((i, j)); // 1098:00ef
                let b = self.building_or_none((i, j));
                // BUG(original): other types read past the table; the port says 0.
                let (di, dj) = b
                    .checked_sub(FIRST_HARBOUR)
                    .and_then(|k| HARBOUR_TARGETS.get(usize::from(k)))
                    .copied()
                    .unwrap_or((0, 0));
                let p = self.free_record(n);
                p.set_pos(corner);
                p.set_target((i.wrapping_add(di), j.wrapping_add(dj)));
                p.set_home(corner);
                p.set_kind(kind);
                p.set_flags([1, 0, 1]);
                p.set_path(0);
                p.set_step(0);
                p.set_word_26(0);
                p.set_sprite(match kind {
                    0x22 => 0x133,
                    0x23 => 0x13b,
                    _ => 0x143,
                });
                p.set_offset((0, 0));
                self.set_marker(corner, kind);
            }
            0x35..=0x3a => {
                let k = usize::from(kind - 0x35);
                let (target_j, offset) = match kind {
                    0x35 => (4, (-1, 0xc)),
                    0x36 => (4, (0x1b, 0xc)),
                    0x37 => (2, (7, 0xb)),
                    0x38 => (2, (0x19, 0xb)),
                    _ => (0, (0x10, 7)),
                };
                let lane = self.people.lane;
                self.free_record(n).fill(&Fill {
                    pos: (i, j),
                    offset,
                    heading: 0,
                    sprite: TAXIING_SPRITES[r][k],
                    flags: [1, 0, 1],
                    kind,
                    path: 0,
                    step: 4,
                    target: (lane, target_j),
                    home: (0, 0),
                    word_26: 0,
                });
                match kind {
                    0x35 => self.turn_taxiing(n, &TAXIING_OFFSETS_35),
                    0x36 => self.turn_taxiing(n, &TAXIING_OFFSETS_36),
                    0x37 => self.turn_taxiing(n, &TAXIING_OFFSETS_37),
                    0x38 => self.turn_taxiing(n, &TAXIING_OFFSETS_38),
                    _ => {}
                }
            }
            _ => {
                let Some(s) = SIMPLE.iter().find(|s| s.kind == kind).copied() else {
                    return;
                };
                if kind == STAGE_SPECTATORS {
                    self.people.flag_3c42 = true;
                }
                let pos = self.place(s.at, (i, j));
                let split = self.people_split();
                self.free_record(n).fill(&Fill {
                    pos,
                    offset: s.offset,
                    sprite: s.sprite,
                    flags: [1, 0, 1],
                    kind,
                    step: s.step,
                    // The building's square, for the views to turn the
                    // door from ([`Placer::place_again_from_home`]).
                    home: if split { (i, j) } else { (0, 0) },
                    ..Fill::default()
                });
                if kind == BUILDER && split {
                    self.placed_sound(pos, &BUILDER_SOUNDS, tick);
                } else if kind == BUILDER {
                    if self.visible(pos) {
                        let sound = BUILDER_SOUNDS[usize::from(self.random.random(5))];
                        tick.sounds.push(sound);
                    }
                } else if let Some(sound) = s.sound {
                    self.sound_at(pos, sound, tick);
                }
            }
        }
    }

    /// The kinds 1088:0324 fills whether or not people move, after its
    /// cases: someone leaving (0x19, [`LEAVING`]) with one of two sounds by
    /// `Random(2)` where the player sees them, kinds 0x2a and 0x46 with a
    /// picture by the square's form, and the oil (0x2b–0x2d).
    fn fill_late(&mut self, n: i16, kind: u8, (i, j): Pos, tick: &mut Tick) {
        let t = Arc::clone(&self.tables);
        match kind {
            0x19 | LEAVING => {
                let (sprite, sounds) = if kind == 0x19 {
                    (0xdb, [0x16, 0x17])
                } else {
                    (0x14b, [0x15, 5])
                };
                let Some(p) = self.people.get_mut(n) else {
                    return;
                };
                p.set_kind(kind);
                p.set_pos((i, j));
                p.set_flags([1, 0, 1]);
                p.set_path(0);
                p.set_step(8);
                p.set_sprite(sprite);
                p.set_offset((0x1f - t.animdat1(sprite, 1) / 2, p.offset().1));
                p.set_offset((p.offset().0, 0x10 - t.animdat1(sprite, 0) / 2));
                // BUG(original): the sound is picked with the world's random numbers, only where the human sees it (`fix.view_people`: by each view).
                if self.people_split() {
                    // Picked by each view that sees it, not with the
                    // world's random numbers.
                    self.placed_sound((i, j), if kind == 0x19 { &[0x16, 0x17] } else { &[0x15, 5] }, tick);
                } else if self.visible((i, j)) {
                    let s = self.random.random(2);
                    tick.sounds.push(sounds[usize::from(s & 1)]);
                }
            }
            0x2a | 0x46 => {
                let form = usize::from(self.form_or_flat((i, j)));
                let correction = t
                    .form_korrektur
                    .get(self.people_view().rotation())
                    .and_then(|row| row.get(form))
                    .copied()
                    .unwrap_or(0); // 1088:4bfa
                let Some(p) = self.people.get_mut(n) else {
                    return;
                };
                p.fill(&Fill {
                    pos: (i, j),
                    sprite: 0x17c + i16::from(correction),
                    flags: [1, 1, 1],
                    kind,
                    step: if kind == 0x2a { 10 } else { 100 },
                    ..Fill::default()
                });
            }
            0x2b..=0x2d => {
                let Some(p) = self.people.get_mut(n) else {
                    return;
                };
                p.fill(&Fill {
                    pos: (i, j),
                    sprite: (i16::from(kind) - 0x2b) * 3 + 399,
                    flags: [1, 0, 1],
                    kind,
                    step: 0x1e,
                    ..Fill::default()
                });
                self.set_marker((i, j), kind);
            }
            _ => {}
        }
    }
}

/// A taxiing plane's picture offset by rotation and runway (0–3), where the
/// original sets one.
type TaxiingOffsets = [[Option<(i16, i16)>; 4]; 4];

/// Kind 0x35. // 1098:0745
const TAXIING_OFFSETS_35: TaxiingOffsets = [
    [Some((0, 0xe)), Some((0x18, 1)), Some((0x10, 5)), Some((8, 9))],
    [Some((0x1c, 0xe)), Some((4, 1)), Some((0xc, 4)), Some((0x14, 9))],
    [Some((0x18, 1)), Some((0, 0xe)), Some((8, 9)), Some((0x10, 5))],
    [Some((4, 1)), Some((0x1c, 0xe)), Some((0x14, 9)), Some((0xc, 4))],
];
/// Kind 0x36. // 1098:0a14
const TAXIING_OFFSETS_36: TaxiingOffsets = [
    [Some((0x1c, 0xe)), Some((4, 1)), Some((0xc, 4)), Some((0x14, 9))],
    [Some((0x18, 1)), Some((0, 0xe)), Some((8, 9)), Some((0x10, 5))],
    [Some((4, 1)), Some((0x1c, 0xe)), Some((0x14, 9)), Some((0xc, 4))],
    [Some((0, 0xe)), Some((0x18, 1)), Some((0x10, 5)), Some((8, 9))],
];
/// Kind 0x37. // 1098:0ce3
const TAXIING_OFFSETS_37: TaxiingOffsets = [
    [Some((7, 0xb)), Some((0x16, 2)), None, None],
    [Some((0x19, 0xb)), Some((10, 2)), None, None],
    [Some((0x16, 2)), Some((7, 0xb)), None, None],
    [Some((10, 2)), Some((0x19, 0xb)), None, None],
];
/// Kind 0x38. // 1098:0e6d
const TAXIING_OFFSETS_38: TaxiingOffsets = [
    [Some((0x1a, 0xc)), Some((0xb, 3)), None, None],
    [Some((0x17, 3)), Some((8, 0xc)), None, None],
    [Some((0xb, 3)), Some((0x1a, 0xc)), None, None],
    [Some((8, 0xc)), Some((0x17, 3)), None, None],
];

/// The taxiing planes (kinds 0x35–0x3a) by airport type 0x28–0x33, as
/// [`World::spawn_people`] sends them: whether a `Random(2)` of 1 picks
/// the first or the second part, the parts their square must show, the
/// runway and the kind. // 1090:2373
#[derive(Debug, Clone, Copy)]
struct AirportPlane {
    parts: [u8; 2],
    random: bool,
    lane: i16,
    kind: u8,
}

const fn airport_plane(random: bool, parts: [u8; 2], lane: i16, kind: u8) -> AirportPlane {
    AirportPlane {
        parts,
        random,
        lane,
        kind,
    }
}

/// [`AirportPlane`] for each of the building types 0x28–0x33.
const AIRPORT_PLANES: [AirportPlane; 12] = [
    airport_plane(false, [5, 5], 3, 0x3a),
    airport_plane(false, [5, 5], 3, 0x39),
    airport_plane(true, [3, 0xb], 4, 0x3a),
    airport_plane(true, [9, 0xb], 4, 0x39),
    airport_plane(false, [7, 7], 4, 0x38),
    airport_plane(false, [7, 7], 4, 0x37),
    airport_plane(true, [5, 0x11], 6, 0x38),
    airport_plane(true, [0xf, 0x11], 6, 0x37),
    airport_plane(false, [0xe, 0xe], 5, 0x36),
    airport_plane(false, [0xe, 0xe], 5, 0x35),
    airport_plane(true, [6, 0x22], 7, 0x36),
    airport_plane(true, [0x1e, 0x22], 7, 0x35),
];

/// Who a building sends out ([`World::send_person`]): the building type,
/// where they're looked for before someone new comes, and the kinds, one
/// picked with `Random(len)` if there are several. // 1090:1a25
#[derive(Debug, Clone, Copy)]
struct Sender {
    building: u8,
    /// Only from the building's first square (part 0).
    first_square: bool,
    /// Where to look for one of the same kind: the square, or a door.
    look: At,
    kinds: &'static [u8],
}

const fn sender(building: u8, first_square: bool, look: At, kinds: &'static [u8]) -> Sender {
    Sender {
        building,
        first_square,
        look,
        kinds,
    }
}

/// The buildings that send someone particular out, in 1090:1a25's order.
/// The kind is picked with `Random(n)`; for 0x49 the original's
/// `Random(2) = 1` picks the first.
const SENDERS: [Sender; 21] = [
    sender(7, false, At::Square, &[4]),
    sender(0x3a, false, At::Square, &[0x42]),
    sender(6, false, At::Square, &[0x43]),
    sender(0x49, true, At::Door(0), &[0x44, 0x45]),
    sender(0x57, false, At::Square, &[0x30]),
    sender(0x4e, true, At::Door(0), &[0x31]),
    sender(0x50, true, At::Door(1), &[0x32]),
    sender(0x46, true, At::Door(1), &[0x34]),
    sender(0x41, true, At::Door(0), &[0x47, 0x48, 0x49, 0x4a, 0x4b, 0x4c]),
    sender(0x4a, true, At::Door(0), &[0x3b]),
    sender(0x37, true, At::Door(1), &[0x3c]),
    sender(0x26, true, At::Door(1), &[0x3d]),
    sender(0x38, true, At::Door(2), &[0x3e]),
    sender(0x27, true, At::Door(1), &[0x40]),
    sender(0x40, true, At::Door(0), &[0x3f]),
    sender(0x20, true, At::Door(0), &[0x1a]),
    sender(0x21, true, At::Door(0), &[0x1b]),
    sender(0x14, false, At::Square, &[0x27]),
    sender(0x48, true, At::Door(0), &[0x28]),
    sender(0x39, false, At::Square, &[0x29]),
    sender(0x45, true, At::Door(1), &[0x1c, 0x1d, 0x1e, 0x1f]),
];

/// The helicopter flights (`H-Rundflüge`).
const HELIPORT: u8 = 0x17;

impl World {
    /// New people come: a boat and a ship at random spots of open sea
    /// (`Random(250)` for `i`, then `j`, each) while there are fewer than
    /// ten, then for each square the player sees (11 rows of 61 from the
    /// view's origin, [`view_square`]): spectators at a full tennis court
    /// and Miss Holiday Island's stage, builders on a building site
    /// (`Random(50)`), planes taxiing at the airports (`Random(100)` and
    /// some a `Random(2)`), and at the harbours a sound now and then
    /// (`Random(500)`, `Random(2)`) and boats from the `Bigport`'s jetties
    /// (`Random(10)`). Only while people move. Marked: what the player
    /// sees decides where people come. // 1090:2373
    pub(crate) fn spawn_people(&mut self, tick: &mut Tick) {
        if !self.options.people_move {
            return;
        }
        for kind in [BOAT, SHIP] {
            let i = self.random.random(250).cast_signed();
            let j = self.random.random(250).cast_signed();
            let full = if kind == BOAT {
                self.people.boats >= 10
            } else {
                self.people.ships >= 10
            };
            if self.form_or_flat((i, j)) == FLAT
                && self.ground_or_water((i, j)) == WATER
                && !full
                && !self.person_at(kind, (i, j))
            {
                self.add_person(kind, (i, j), tick);
                if kind == BOAT {
                    self.people.boats = self.people.boats.wrapping_add(1);
                } else {
                    self.people.ships = self.people.ships.wrapping_add(1);
                }
                // The original then reads the ground two squares away on
                // each side, and does nothing with it.
            }
        }
        // BUG(original): new people come only to the squares the human's view looks at, from the world's random numbers (`fix.view_people`: the views' own).
        if self.people_split() {
            // The rest are the views' own (`view::ViewPeople::spawn`).
            return;
        }
        let origin = self.people_view().origin;
        for l in 0..=60 {
            for e in 0..=10 {
                let (di, dj) = view_square(e, l);
                let pos = (origin.0.wrapping_add(di), origin.1.wrapping_add(dj));
                if (1..250).contains(&pos.0) && (1..250).contains(&pos.1) {
                    self.spawn_at(pos, tick);
                }
            }
        }
    }

    /// [`World::spawn_people`] for one square the player sees.
    fn spawn_at(&mut self, pos: Pos, tick: &mut Tick) {
        let b = self.building_or_none(pos);
        match b {
            0x70 => {
                if self.part_or_zero(pos) == 0 {
                    let d = DOORS[self.people_view().rotation()][1];
                    let look = (pos.0.wrapping_add(d.0), pos.1.wrapping_add(d.1));
                    for kind in TENNIS_SPECTATORS {
                        if !self.person_at(kind, look) {
                            self.add_person(kind, pos, tick);
                        }
                    }
                }
            }
            0x7b | 0x7c => {
                // BUG(original): works out the stage's offset (1110:3a58), then looks for spectators on the square itself.
                let look = if self.fixes.has(Fix::StageSpectators) {
                    self.place(At::Stage, pos)
                } else {
                    pos
                };
                if self.part_or_zero(pos) == 0 && !self.person_at(STAGE_SPECTATORS, look) {
                    self.add_person(STAGE_SPECTATORS, pos, tick);
                }
            }
            0x1f => {
                if self.part_or_zero(pos) == 0 && self.random.random(50) == 1 && !self.person_at(BUILDER, pos) {
                    self.add_person(BUILDER, pos, tick);
                }
            }
            0x28..=0x33 => {
                if self.random.random(100) != 1 {
                    return;
                }
                let h = AIRPORT_PLANES[usize::from(b - 0x28)];
                let part = if h.random && self.random.random(2) != 1 {
                    h.parts[1]
                } else {
                    h.parts[0]
                };
                if self.part_or_zero(pos) == part {
                    self.people.lane = h.lane;
                    if !self.person_at(h.kind, pos) {
                        self.add_person(h.kind, pos, tick);
                    }
                }
            }
            0x58..=0x6b => {
                if self.random.random(500) == 1 && self.visible(pos) {
                    match self.random.random(2) {
                        0 => tick.sounds.push(0x22),
                        1 => tick.sounds.push(0x10),
                        _ => {}
                    }
                }
                if (0x68..=0x6b).contains(&b) && self.part_or_zero(pos) == 0 {
                    let (di, dj) = JETTY_OFFSETS
                        .get(usize::from(b - FIRST_JETTY) + 4 * self.people_view().rotation())
                        .copied()
                        .unwrap_or((0, 0));
                    let look = (di.wrapping_add(pos.0), dj.wrapping_add(pos.1));
                    if self.random.random(10) == 1 && !self.person_at(0x33, look) {
                        self.add_person(0x33, pos, tick);
                    }
                }
            }
            _ => {}
        }
    }

    /// Someone leaves the building on `pos`, by its type: a guest, a
    /// plane, a ship... if there isn't one of that kind there already. Most need
    /// the building's first square; some kinds are picked with a `Random`.
    /// Airports get a plane (`Random(35)`, then `Random(3)`), harbours a
    /// ship (`Random(30)`, then `Random(3)`). // 1090:1a25
    pub(crate) fn send_person(&mut self, pos: Pos, tick: &mut Tick) {
        let b = self.building_or_none(pos);
        if let Some(s) = SENDERS.iter().find(|s| s.building == b).copied() {
            if s.first_square && self.part_or_zero(pos) != 0 {
                return;
            }
            // From the square itself, which is the building's first.
            let look = match s.look {
                At::Door(n) => {
                    let d = DOORS[self.people_view().rotation()][n];
                    (pos.0.wrapping_add(d.0), pos.1.wrapping_add(d.1))
                }
                _ => pos,
            };
            let kind = match s.kinds {
                [kind] => *kind,
                [first, second] => {
                    if self.random.random(2) == 1 {
                        *first
                    } else {
                        *second
                    }
                }
                kinds => {
                    let n = self.random.random(u16::try_from(kinds.len()).unwrap_or(1));
                    kinds[usize::from(n)]
                }
            };
            if !self.person_at(kind, look) {
                self.add_person(kind, pos, tick);
            }
            return;
        }
        match b {
            HELIPORT => {
                if self.marker_or_zero(pos) == FREE {
                    self.add_person(HELICOPTER, pos, tick);
                }
            }
            0x28..=0x33 if self.random.random(0x23) == 0xc && self.part_or_zero(pos) == 0 => {
                let kind = PLANE + u8::try_from(self.random.random(3)).unwrap_or(0);
                if !self.person_at(kind, pos) {
                    self.add_person(kind, pos, tick);
                }
            }
            0x58..=0x6b if self.random.random(0x1e) == 0 && self.part_or_zero(pos) == 0 => {
                let kind = 0x22 + u8::try_from(self.random.random(3)).unwrap_or(0);
                if !self.anyone_at(pos) {
                    self.add_person(kind, pos, tick);
                }
            }
            _ => {}
        }
    }

    /// Record `n` leaves: a plane with its second half; a walker or surfer
    /// takes one off its home's count in `layer_551c` (255 at 0) and a
    /// walker frees its path's square; anyone else on the map clears both
    /// marker layers there. // 1088:2df2
    ///
    /// The original reads and writes outside the layer for a home off the
    /// map (a walker's or surfer's whose beach was taken, -1); the port
    /// leaves that alone.
    pub(crate) fn remove_person(&mut self, n: i16) {
        // BUG(original): a home of -1 is read and written outside the layer; the port leaves it alone.
        let Some(p) = self.people.get(n).cloned() else {
            return;
        };
        let kind = p.kind();
        if [PLANE, PLANE + 1, PLANE + 2, HELICOPTER].contains(&kind) {
            if let Some(q) = self.people.get_mut(n) {
                q.set_kind(NOBODY);
            }
            if let Some(q) = self.people.get_mut(n + PLANE_HALF) {
                q.set_kind(NOBODY);
            }
        } else if WALKERS.contains(&kind) || SURFERS.contains(&kind) {
            let home = p.home();
            if let Some(count) = self.island.layer_551c.get(home) {
                let count = if count == 0 { 0 } else { count - 1 };
                self.island.layer_551c.set(home, if count == 0 { FREE } else { count });
            }
            if WALKERS.contains(&kind) {
                self.free_path_square(n);
            }
        } else if kind != NOBODY {
            let (i, j) = p.pos();
            if (1..250).contains(&i) && (1..250).contains(&j) {
                self.island.layer_551c.set((i, j), FREE);
                self.island.layer_5520.set((i, j), FREE);
            }
        }
        if let Some(q) = self.people.get_mut(n) {
            q.set_kind(NOBODY);
        }
    }

    /// A walker frees the square its path's step leads to (by the step's
    /// fourth byte: 10 `i + 1`, 11 `i - 1`, 12 `j - 1`, 13 `j + 1`, else
    /// its own) in both marker layers; with `fix.walker_markers` in those
    /// that hold its path. // 1088:5841
    fn free_path_square(&mut self, n: i16) {
        let Some(p) = self.people.get(n) else {
            return;
        };
        // The step's low word picks the row.
        #[allow(clippy::cast_possible_truncation)]
        let row = i32::from(p.step() as i16);
        let (i, j) = p.pos();
        let square = match self.people.path_byte(p.path(), row, 3) {
            10 => (i.wrapping_add(1), j),
            11 => (i.wrapping_sub(1), j),
            12 => (i, j.wrapping_sub(1)),
            13 => (i, j.wrapping_add(1)),
            _ => (i, j),
        };
        // The original first clears each layer where it holds the walker's
        // path, then clears both anyway.
        // BUG(original): both layers are cleared, another walker's claim on the square too (fix.walker_markers: only those that hold its path).
        if self.fixes.has(Fix::WalkerMarkers) {
            let path = p.path();
            if self.island.layer_551c.get(square).is_some_and(|m| i16::from(m) == path) {
                self.set_marker(square, FREE);
            }
            if self.island.layer_5520.get(square).is_some_and(|m| i16::from(m) == path) {
                self.set_marker_2(square, FREE);
            }
            return;
        }
        self.set_marker(square, FREE);
        self.set_marker_2(square, FREE);
    }

    /// Takes the people on `pos` off the island, and if a building is
    /// there (not a palm), everyone on its squares: each square gets
    /// someone leaving ([`LEAVING`]) and free marker layers, and the people
    /// on it go, but for those leaving, the money and kind 0x19, and planes
    /// whose `+0x15` flag is set (their records stay, the layers under
    /// the others are cleared). // 1088:6975
    pub(crate) fn remove_people_from(&mut self, pos: Pos, tick: &mut Tick) {
        for n in FIRST_MONEY..=LAST {
            if self.people.get(n).is_some_and(|p| p.pos() == pos) {
                self.remove_person(n); // 1088:2df2
            }
        }
        let b = self.building_or_none(pos);
        if b == NO_BUILDING || b == crate::world::PALM {
            return;
        }
        let origin = self.building_origin(pos); // 1030:154c
        let building = self.building_or_none(origin);
        self.people.last_building = i16::from(building);
        let t = Arc::clone(&self.tables);
        let width = t.hausnr(building, 3).wrapping_sub(1);
        let depth = t.hausnr(building, 4).wrapping_sub(1);
        for e in 0..=width {
            for f in 0..=depth {
                let square = (origin.0.wrapping_add(e), origin.1.wrapping_add(f));
                self.add_person(LEAVING, square, tick);
                self.set_marker(square, FREE);
                self.set_marker_2(square, FREE);
                for n in FIRST_MONEY..=LAST {
                    let Some(p) = self.people.get(n) else {
                        continue;
                    };
                    if p.pos() != square {
                        continue;
                    }
                    let (kind, flags, at) = (p.kind(), p.flags(), p.pos());
                    if (PLANE..=PLANE + 5).contains(&kind) {
                        if flags[1] == 0 {
                            if let Some(p) = self.people.get_mut(n) {
                                p.set_kind(NOBODY);
                            }
                            self.set_marker(at, FREE);
                            self.set_marker_2(at, FREE);
                        }
                    } else if ![0x19, LEAVING, MONEY].contains(&kind)
                        && let Some(p) = self.people.get_mut(n)
                    {
                        p.set_kind(NOBODY);
                    }
                }
            }
        }
    }

    /// The builders ([`BUILDER`]) on the area of a `building` whose first
    /// square is `pos` leave. // 1088:6bac
    pub(crate) fn remove_workers(&mut self, building: u8, (i, j): Pos) {
        let t = Arc::clone(&self.tables);
        let (i1, j1) = (
            t.hausnr(building, 3).wrapping_add(i),
            t.hausnr(building, 4).wrapping_add(j),
        );
        for n in FIRST..=GENERAL_END {
            let Some(p) = self.people.get(n) else {
                continue;
            };
            let (pi, pj) = p.pos();
            if (i..i1).contains(&pi) && (j..j1).contains(&pj) && p.kind() == BUILDER {
                self.remove_person(n); // 1088:2df2
            }
        }
    }

    /// The walkers and surfers whose home is `pos` leave (the walkers'
    /// path squares freed twice, the surfers' squares cleared in
    /// `layer_551c`); says whether there were any. // 1088:2d2a
    pub(crate) fn remove_people_at(&mut self, pos: Pos) -> bool {
        let last = self.people.last;
        let mut found = false;
        if last <= LAST_MONEY {
            return false;
        }
        for n in FIRST..=last {
            let Some(p) = self.people.get(n) else {
                continue;
            };
            let (kind, home, at) = (p.kind(), p.home(), p.pos());
            if home != pos {
                continue;
            }
            if WALKERS.contains(&kind) {
                self.free_path_square(n); // 1088:5841
                self.remove_person(n); // 1088:2df2
                found = true;
            } else if SURFERS.contains(&kind) {
                self.set_marker(at, FREE);
                self.remove_person(n); // 1088:2df2
                found = true;
            }
        }
        found
    }

    /// Like [`World::remove_people_at`], but only the first walker or
    /// surfer whose home is `pos` leaves, and its square's `layer_551c` is
    /// cleared after it (for a walker too); says whether there was one.
    /// // 1088:2c3b
    pub(crate) fn remove_first_at_home(&mut self, pos: Pos) -> bool {
        let last = self.people.last;
        if last <= LAST_MONEY {
            return false;
        }
        for n in FIRST..=last {
            let Some(p) = self.people.get(n) else {
                continue;
            };
            let (kind, home, at) = (p.kind(), p.home(), p.pos());
            if home != pos || !(WALKERS.contains(&kind) || SURFERS.contains(&kind)) {
                continue;
            }
            if WALKERS.contains(&kind) {
                self.free_path_square(n); // 1088:5841
            }
            self.remove_person(n); // 1088:2df2
            self.set_marker(at, FREE);
            // The original goes on from the new last record to the old
            // one, which are all free by then.
            return true;
        }
        false
    }

    /// The nearest road square (ground 10–58, 1030:16a2) on the plot of
    /// `low` with free marker layers (1010:1efa) around the area `low` to
    /// `high`, ring by ring up to `rings`, in [`World::find_sea`]'s order.
    /// // 1010:1f8f
    pub(crate) fn find_road(&self, low: Pos, high: Pos, rings: i16) -> Option<Pos> {
        let plot = self.island.plot_at(low); // 1030:1e62
        let road = |w: &World, pos: Pos| {
            crate::world::on_map(pos)
                && w.island.plot_at(pos) == plot
                && (10..0x3b).contains(&w.island.ground.at(pos))
                && w.markers_free(pos)
        };
        for ring in 1..=rings {
            for i in low.0 - ring..=high.0 + ring {
                for j in [low.1 - ring, high.1 + ring] {
                    if road(self, (i, j)) {
                        return Some((i, j));
                    }
                }
            }
            for j in (low.1 - ring + 1)..(high.1 + ring) {
                for i in [low.0 - ring, high.0 + ring] {
                    if road(self, (i, j)) {
                        return Some((i, j));
                    }
                }
            }
        }
        None
    }

    /// With `fix.view_people`: the people of the views' kinds
    /// ([`is_view_kind`]) leave the world's records, which are free again
    /// (the marker layers as they are: those kinds mark nothing), and go
    /// to the views ([`Tick::view_people`]); the rest are placed again by
    /// the first rotation ([`World::people_view`]). A loaded game and the
    /// fix switched on do it.
    pub(crate) fn hand_over_view_people(&mut self, tick: &mut Tick) {
        for p in &mut self.people.records[..] {
            if p.kind() != NOBODY && is_view_kind(p.kind()) {
                tick.view_people.push(p.clone());
                p.set_kind(NOBODY);
            }
        }
        self.place_people_again(); // 1088:7a14
    }

    /// [`World::hand_over_view_people`] outside a command, for a game
    /// that starts from a file with `fix.view_people`: the people that go
    /// to the view, none without the fix.
    pub fn split_off_view_people(&mut self) -> Vec<Person> {
        let mut tick = Tick::default();
        if self.people_split() {
            self.hand_over_view_people(&mut tick);
        }
        tick.view_people
    }

    /// The people as `viewpoint` sees them, for a view to draw with
    /// `fix.view_people`: a copy of the world's, which go by the first
    /// rotation ([`World::people_view`]), placed again for the view's
    /// rotation as 1088:7a14 does, the doors from the buildings' squares
    /// ([`Placer::place_again_from_home`]), and the planes' pictures as
    /// 1090:2ce5 lifts them.
    #[must_use]
    pub fn people_for_view(&self, viewpoint: Viewpoint) -> People {
        let mut people = self.people.clone();
        Placer {
            w: self,
            people: &mut people,
            r: viewpoint.rotation(),
        }
        .place_again_from_home();
        people
    }

    /// The map changed for the people: the kinds of the records 0–0x23
    /// are cleared, and `1110:6fce` and `1110:6ff2`. // 1088:6c5a
    pub(crate) fn people_map_changed(&mut self) {
        for p in &mut self.people.records[..=0x23] {
            p.set_kind(NOBODY);
        }
        self.people.word_6fce = 0;
        self.people.last_building = 0;
    }

    /// Shows `amount` rising from `pos`: the first free record of
    /// [`FIRST_MONEY`]..=[`LAST_MONEY`] becomes [`MONEY`]. The original
    /// does it only for the human's costs (marked). // 1088:0153
    pub(crate) fn float_money(&mut self, amount: i32, pos: Pos) {
        for n in FIRST_MONEY..=LAST_MONEY {
            let Some(p) = self.people.get_mut(n) else {
                continue;
            };
            if p.kind() != NOBODY {
                continue;
            }
            p.set_kind(MONEY);
            p.set_pos(pos);
            p.set_offset((p.offset().0, 0xf));
            p.set_flags([1, 1, 1]);
            p.set_path(0);
            p.set_step(amount);
            p.set_sprite(0);
            return;
        }
    }

    /// When a championship ends: for the empty tennis court (0x6f) the
    /// spectators of kinds 0x4d–0x50, for Miss Holiday Island's stage
    /// (0x79, 0x7a) those of kind 0x41, leave. // 10a0:0026's first part
    pub(crate) fn send_spectators_away(&mut self, building: u8) {
        let last = self.people.last;
        let leaves = |kind: u8| match building {
            0x6f => TENNIS_SPECTATORS.contains(&kind),
            0x79 | 0x7a => kind == STAGE_SPECTATORS,
            _ => false,
        };
        if !matches!(building, 0x6f | 0x79 | 0x7a) || last <= LAST_MONEY {
            return;
        }
        for n in FIRST..=last {
            if leaves(self.people.kind(n)) {
                self.remove_person(n); // 1088:2df2
            }
        }
    }

    /// The people on a flooded square leave if either marker layer is
    /// taken there, and both become free. // 10a0:0b49's second part
    pub(crate) fn flood_people(&mut self, pos: Pos) {
        let last = self.people.last;
        if self.markers_free(pos) || last <= LAST_MONEY {
            return;
        }
        for n in FIRST..=last {
            if self.people.get(n).is_some_and(|p| p.pos() == pos) {
                self.remove_person(n); // 1088:2df2
                self.set_marker(pos, FREE);
                self.set_marker_2(pos, FREE);
            }
        }
    }

    /// Five tries to send a surfer (kind 0x20 or 0x21 by `Random(2)`) from
    /// the beach at `pos` into the nearest open sea within 15 rings
    /// (1010:20e0); each that comes counts in `layer_551c` at `pos`.
    /// // 10a0:10c6
    pub(crate) fn send_surfers(&mut self, pos: Pos, tick: &mut Tick) {
        for _ in 0..5 {
            let Some(sea) = self.find_sea(pos, (pos.0 + 1, pos.1 + 1), 15) else {
                continue;
            };
            let kind = SURFERS[usize::from(self.random.random(2) & 1)];
            // 1088:2c04
            self.people.beach = pos;
            self.add_person(kind, sea, tick);
            if self.people.beach.0 == -1 {
                let count = self.island.layer_551c.get(pos).unwrap_or(0);
                self.set_marker(pos, count.wrapping_add(1));
            }
        }
    }

    /// The nearest square of open sea (flat water with free marker
    /// layers, 1010:1f2d and 1010:1ea5) around the area `low` to `high`,
    /// ring by ring up to `rings`: in each ring first the columns at
    /// `j - ring` and `j + ring` down `i`, then the rows at `i - ring` and
    /// `i + ring` along `j`. // 1010:20e0
    pub(crate) fn find_sea(&self, low: Pos, high: Pos, rings: i16) -> Option<Pos> {
        let open = |w: &World, pos: Pos| {
            crate::world::on_map(pos)
                && w.island.ground.at(pos) == WATER
                && w.island.forms.at(pos) == FLAT
                && w.markers_free(pos)
        };
        for ring in 1..=rings {
            for i in low.0 - ring..=high.0 + ring {
                for j in [low.1 - ring, high.1 + ring] {
                    if open(self, (i, j)) {
                        return Some((i, j));
                    }
                }
            }
            for j in (low.1 - ring + 1)..(high.1 + ring) {
                for i in [low.0 - ring, high.0 + ring] {
                    if open(self, (i, j)) {
                        return Some((i, j));
                    }
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use vcl::random::Random;

    use super::*;
    use crate::world::{Island, Tables};

    /// A flat island of grass with an open sea around (0..10 and 60..),
    /// room for people, and `building` with `width × depth` squares at
    /// (20, 20).
    fn world(building: u8, width: i16, depth: i16) -> World {
        let mut hausnr = vec![[0; 8]; 0x80];
        hausnr[usize::from(building)] = [1, 0, 0, width, depth, width * depth, width * depth, 0];
        let mut animdat1 = vec![[0; 11]; 0x300];
        animdat1[0x14b][0] = 10;
        animdat1[0x14b][1] = 20;
        let mut island = Island::empty();
        for i in 0..=250 {
            for j in 0..=250 {
                island.forms.set((i, j), FLAT);
                let land = (10..60).contains(&i) && (10..60).contains(&j);
                island.ground.set((i, j), if land { 1 } else { WATER });
                if land {
                    island.heights.set((i, j), 2);
                }
            }
        }
        let mut w = World {
            island,
            tables: Arc::new(Tables {
                hausnr,
                animdat1,
                ..Tables::default()
            }),
            random: Random::new(7),
            ..World::default()
        };
        w.people.last = FIRST;
        let mut part = 0;
        for j in 20..20 + depth {
            for i in 20..20 + width {
                w.island.buildings.set((i, j), building);
                w.island.parts.set((i, j), part);
                part += 1;
            }
        }
        w
    }

    fn kinds(w: &World) -> Vec<(i16, u8)> {
        (0..=LAST)
            .filter_map(|n| w.people.get(n).filter(|p| p.kind() != NOBODY).map(|p| (n, p.kind())))
            .collect()
    }

    #[test]
    fn the_general_range_grows() {
        let mut w = world(0x14, 1, 1);
        let mut tick = Tick::default();
        w.people.last = LAST_MONEY;
        w.add_person(0x27, (20, 20), &mut tick);
        assert_eq!(kinds(&w), []);
        w.people.last = FIRST;
        w.add_person(0x27, (20, 20), &mut tick);
        w.add_person(0x29, (21, 20), &mut tick);
        assert_eq!(kinds(&w), [(FIRST, 0x27), (FIRST + 1, 0x29)]);
        assert_eq!(w.people.last, FIRST + 2);
        let p = w.people.get(FIRST).unwrap();
        assert_eq!(
            (p.pos(), p.offset(), p.sprite(), p.step()),
            ((20, 20), (0xe, 3), 0x167, 10)
        );
        assert_eq!(p.flags(), [1, 0, 1]);
        // A free record in between is taken first, and the end stays.
        w.people.records[usize::try_from(FIRST).unwrap()].set_kind(NOBODY);
        w.add_person(0x29, (22, 20), &mut tick);
        assert_eq!(w.people.kind(FIRST), 0x29);
        assert_eq!(w.people.last, FIRST + 2);
    }

    #[test]
    fn only_while_people_move() {
        let mut w = world(0x14, 1, 1);
        w.options.people_move = false;
        let mut tick = Tick::default();
        w.add_person(0x27, (20, 20), &mut tick);
        // The record is taken but stays empty.
        assert_eq!(kinds(&w), []);
        assert_eq!(w.people.last, FIRST + 1);
        let seed = w.random;
        w.spawn_people(&mut tick);
        assert_eq!(w.random, seed);
    }

    #[test]
    fn a_building_sends_someone_once() {
        let mut w = world(0x14, 1, 1);
        let mut tick = Tick::default();
        w.send_person((20, 20), &mut tick);
        w.send_person((20, 20), &mut tick);
        assert_eq!(kinds(&w), [(FIRST, 0x27)]);
    }

    #[test]
    fn fix_stage_spectators() {
        for fixed in [false, true] {
            let mut w = world(0x7b, 2, 2);
            w.fixes.set(Fix::StageSpectators, fixed);
            let mut tick = Tick::default();
            w.spawn_at((20, 20), &mut tick);
            w.spawn_at((20, 20), &mut tick);
            // They stand at (20, 21): the original looks on (20, 20) and
            // sends more every time.
            let spectators = kinds(&w).iter().filter(|k| k.1 == STAGE_SPECTATORS).count();
            assert_eq!(spectators, if fixed { 1 } else { 2 });
            assert_eq!(w.people.get(FIRST).unwrap().pos(), (20, 21));
        }
    }

    #[test]
    fn doors_follow_the_view() {
        // A casino (0x49) sends someone out of its door, picked by
        // Random(2), only from its first square.
        let mut w = world(0x49, 3, 3);
        let mut tick = Tick::default();
        w.send_person((21, 20), &mut tick);
        assert_eq!(kinds(&w), []);
        let mut r = w.random;
        let kind = if r.random(2) == 1 { 0x44 } else { 0x45 };
        w.send_person((20, 20), &mut tick);
        assert_eq!(kinds(&w), [(FIRST, kind)]);
        assert_eq!(w.people.get(FIRST).unwrap().pos(), (21, 21));
        w.viewpoint.rotation = 3;
        w.send_person((20, 20), &mut tick);
        let p = w.people.get(FIRST + 1).unwrap();
        assert_eq!(p.pos(), (21, 20));
    }

    #[test]
    fn planes_come_from_the_nearest_corner() {
        // A Palm-Airport (0x28): Random(35) must be 12, then Random(3)
        // picks the plane, and Random(2) the approach.
        let mut w = world(0x28, 4, 2);
        let mut tick = Tick::default();
        let mut tries = 0;
        while kinds(&w).is_empty() {
            w.send_person((20, 20), &mut tick);
            tries += 1;
            assert!(tries < 10_000);
        }
        let k = kinds(&w);
        assert_eq!(k.len(), 2);
        let (n, kind) = k[0];
        assert!((FIRST_PLANE..=LAST_PLANE).contains(&n));
        assert!((PLANE..PLANE + 3).contains(&kind));
        assert_eq!(k[1], (n + PLANE_HALF, kind + 3));
        let p = w.people.get(n).unwrap();
        assert_eq!(p.pos(), (0, 0));
        assert_eq!(p.word_26(), 0xdc);
        assert_eq!(p.heading(), 25);
        // Both of the Palm-Airport's ways in are at (2, 1) from it.
        assert_eq!(p.home(), (22, 21));
    }

    #[test]
    fn a_demolished_building_empties() {
        let mut w = world(0x14, 2, 2);
        let mut tick = Tick::default();
        for (n, kind) in [(FIRST, 0x27), (FIRST + 1, 0x19), (FIRST + 2, 0x30)] {
            let p = w.people.get_mut(n).unwrap();
            p.set_kind(kind);
            p.set_pos((21, 21));
        }
        w.people.last = FIRST + 3;
        w.island.layer_551c.set((21, 21), 5);
        w.remove_people_from((20, 20), &mut tick);
        // Everyone on the building's squares went but kind 0x19, and each
        // square got someone leaving.
        let k = kinds(&w);
        assert!(k.contains(&(FIRST + 1, 0x19)));
        assert!(!k.iter().any(|&(_, kind)| kind == 0x27 || kind == 0x30));
        assert_eq!(k.iter().filter(|&&(_, kind)| kind == LEAVING).count(), 4);
        assert_eq!(w.island.layer_551c.at((21, 21)), FREE);
        assert_eq!(w.people.last_building, 0x14);
        let p = w.people.records.iter().find(|p| p.kind() == LEAVING).unwrap();
        assert_eq!(p.offset(), (0x1f - 10, 0x10 - 5));
        w.people_map_changed();
        assert_eq!(w.people.last_building, 0);
    }

    #[test]
    fn builders_leave_their_site() {
        let mut w = world(0x1f, 2, 2);
        for (n, pos) in [(FIRST, (21, 21)), (FIRST + 1, (22, 21)), (FIRST + 2, (20, 20))] {
            let p = w.people.get_mut(n).unwrap();
            p.set_kind(BUILDER);
            p.set_pos(pos);
        }
        w.remove_workers(0x1f, (20, 20));
        assert_eq!(kinds(&w), [(FIRST + 1, BUILDER)]);
    }

    #[test]
    fn surfers_go_out_and_come_back() {
        let mut w = world(0x71, 2, 2);
        let mut tick = Tick::default();
        // The beach at (58, 58), the sea from 60 on.
        w.send_surfers((58, 58), &mut tick);
        let k = kinds(&w);
        assert_eq!(k.len(), 5);
        assert!(k.iter().all(|&(_, kind)| SURFERS.contains(&kind)));
        assert_eq!(w.island.layer_551c.at((58, 58)), FREE.wrapping_add(5));
        let p = w.people.get(FIRST).unwrap();
        assert_eq!(p.home(), (58, 58));
        assert_eq!(w.island.layer_551c.at(p.pos()), p.kind());
        assert!(w.remove_people_at((58, 58)));
        assert_eq!(kinds(&w), []);
        assert!(!w.remove_people_at((58, 58)));
    }

    #[test]
    fn money_rises() {
        let mut w = world(0x14, 1, 1);
        for n in 0..12 {
            w.float_money(-100 * n, (5, 6));
        }
        let k = kinds(&w);
        assert_eq!(k.len(), 11);
        assert!(
            k.iter()
                .all(|&(n, kind)| (FIRST_MONEY..=LAST_MONEY).contains(&n) && kind == MONEY)
        );
        assert_eq!(w.people.get(FIRST_MONEY + 3).unwrap().step(), -300);
    }

    #[test]
    fn spectators_leave() {
        let mut w = world(0x6f, 1, 1);
        for (n, kind) in [(FIRST, 0x4d), (FIRST + 1, 0x50), (FIRST + 2, 0x41), (FIRST + 3, 0x51)] {
            w.people.get_mut(n).unwrap().set_kind(kind);
        }
        w.people.last = FIRST + 3;
        w.send_spectators_away(0x6f);
        assert_eq!(kinds(&w), [(FIRST + 2, 0x41), (FIRST + 3, 0x51)]);
        w.send_spectators_away(0x79);
        assert_eq!(kinds(&w), [(FIRST + 3, 0x51)]);
    }

    #[test]
    fn what_the_player_sees() {
        let mut w = world(0x14, 1, 1);
        w.viewpoint.origin = (20, 20);
        assert!(w.visible((25, 22)));
        assert!(!w.visible((80, 20)));
        assert!(!w.visible((10, 30)));
        let seen = |w: &World, (e, l)| {
            let (di, dj) = view_square(e, l);
            w.visible((20 + di, 20 + dj))
        };
        for square in [(0, 0), (10, 0), (5, 30), (9, 40)] {
            assert!(seen(&w, square), "{square:?}");
        }
        // The squares spawn_people looks at reach below the screen.
        assert!(!seen(&w, (0, 60)));
        // Turned round, the same squares are off the screen.
        w.viewpoint.rotation = 2;
        assert!(!seen(&w, (5, 30)));
    }

    #[test]
    fn corners() {
        assert_eq!(nearest_corner((10, 10)), (0, 0));
        assert_eq!(nearest_corner((10, 200)), (0, 250));
        assert_eq!(nearest_corner((200, 10)), (250, 0));
        assert_eq!(nearest_corner((125, 125)), (0, 0));
        assert_eq!(nearest_corner((240, 240)), (250, 250));
    }

    #[test]
    fn ships_and_boats_come_to_open_sea() {
        let mut w = world(0x14, 1, 1);
        let mut tick = Tick::default();
        for _ in 0..200 {
            w.spawn_people(&mut tick);
        }
        assert_eq!((w.people.boats, w.people.ships), (10, 10));
        for (_, kind) in kinds(&w) {
            assert!([SHIP, BOAT].contains(&kind));
        }
        let p = w.people.records.iter().find(|p| p.kind() == SHIP).unwrap();
        assert_eq!(w.island.ground.at(p.pos()), WATER);
        assert_eq!(w.island.layer_551c.at(p.pos()), SHIP);
    }

    /// A record of `kind` at `pos`, as `n`.
    fn put(w: &mut World, n: i16, kind: u8, pos: Pos) {
        let p = w.people.get_mut(n).unwrap();
        p.set_kind(kind);
        p.set_pos(pos);
    }

    #[test]
    fn people_refresh_turns_them() {
        let mut w = world(0x14, 3, 3);
        // Someone at the first door of the building at (20, 20), a
        // taxiing plane, and a parked plane.
        put(&mut w, FIRST, 0x28, (21, 21));
        put(&mut w, FIRST + 1, 0x39, (40, 40));
        w.people.get_mut(FIRST + 1).unwrap().set_path(2);
        put(&mut w, FIRST_PLANE, PLANE, (30, 30));
        w.people.get_mut(FIRST_PLANE).unwrap().set_heading(0x1b);
        w.people.last = FIRST + 2;
        w.options.animations = false;
        let mut tick = Tick::default();
        w.set_viewpoint(Viewpoint {
            rotation: 1,
            ..Viewpoint::default()
        });
        w.people_refresh(&mut tick);
        assert_eq!(w.people.get(FIRST).unwrap().pos(), (20, 21));
        assert_eq!(w.people.get(FIRST + 1).unwrap().sprite(), TAXIING_SPRITES[1][4] + 2);
        let look = LOOKS[1][0];
        assert_eq!(w.people.get(FIRST_PLANE).unwrap().sprite(), look + 0x6b);
        assert_eq!(w.people.get(FIRST_PLANE + PLANE_HALF).unwrap().sprite(), look + 0x83);
        // No new people without the animations: no random numbers.
        assert_eq!(w.random, Random::new(7));
        assert_eq!(tick, Tick::default());
        // Back to the first rotation: the door's again.
        w.set_viewpoint(Viewpoint::default());
        w.people_refresh(&mut tick);
        assert_eq!(w.people.get(FIRST).unwrap().pos(), (21, 21));
    }

    #[test]
    fn view_moved_command() {
        let mut w = world(0x14, 1, 1);
        w.options = crate::simulation::Options {
            animations: true,
            people_move: true,
            people_spawn: true,
            ..w.options
        };
        put(&mut w, 3, 6, (5, 5));
        let viewpoint = Viewpoint {
            rotation: 0,
            origin: (15, 15),
            parity: 0,
        };
        let moved = |w: &mut World| {
            w.apply(crate::command::Command {
                player: 1,
                action: crate::command::Action::ViewMoved(viewpoint),
            })
        };
        // The tools' records go, and new people come for the view, which
        // draws the boat's and the ship's random numbers.
        moved(&mut w);
        assert_eq!(w.viewpoint, viewpoint);
        assert_eq!(w.people.kind(3), NOBODY);
        assert_ne!(w.random, Random::new(7));
        // The same view and random numbers give the same people.
        let mut a = world(0x14, 1, 1);
        a.options = w.options;
        let mut b = a.clone();
        moved(&mut a);
        moved(&mut b);
        assert_eq!(a, b);
    }

    fn split(mut w: World) -> World {
        w.fixes.set(Fix::ViewPeople, true);
        w.options = crate::simulation::Options {
            animations: true,
            people_move: true,
            people_spawn: true,
            ..w.options
        };
        w
    }

    /// With `fix.view_people` the world sends none of the views' kinds,
    /// and draws only the boat's and the ship's random numbers.
    #[test]
    fn split_spawns_no_view_people() {
        let mut w = split(world(0x1f, 1, 1));
        w.set_viewpoint(Viewpoint {
            origin: (20, 20),
            ..Viewpoint::default()
        });
        let mut only_sea = w.clone();
        only_sea.island.buildings.set((20, 20), NO_BUILDING);
        let mut tick = Tick::default();
        for _ in 0..500 {
            w.spawn_people(&mut tick);
            only_sea.spawn_people(&mut tick);
        }
        assert!(!w.people.records.iter().any(|p| p.kind() == BUILDER));
        assert_eq!(w.random, only_sea.random);
        // The original's builders come, with more random numbers.
        let mut o = world(0x1f, 1, 1);
        o.options = w.options;
        o.set_viewpoint(w.viewpoint);
        for _ in 0..500 {
            o.spawn_people(&mut tick);
        }
        assert!(o.people.records.iter().any(|p| p.kind() == BUILDER));
    }

    /// With `fix.view_people` the scroll the view used to send does
    /// nothing, and the sounds go to the views with their square, picked
    /// there.
    #[test]
    fn split_view_moved_and_sounds() {
        let mut w = split(world(0x14, 1, 1));
        let before = w.clone();
        w.apply(crate::command::Command {
            player: 1,
            action: crate::command::Action::ViewMoved(Viewpoint {
                rotation: 2,
                origin: (15, 15),
                parity: 1,
            }),
        });
        assert_eq!(w, before);
        let mut tick = Tick::default();
        w.add_person(LEAVING, (20, 20), &mut tick);
        assert_eq!(w.random, before.random);
        assert_eq!(tick.sounds, [] as [u8; 0]);
        assert_eq!(
            tick.placed_sounds,
            [crate::simulation::PlacedSound {
                at: (20, 20),
                sounds: &[0x15, 5],
                after: 0,
            }]
        );
        // The original only where the player sees it, from the world's
        // random numbers.
        let mut o = world(0x14, 1, 1);
        o.set_viewpoint(Viewpoint {
            origin: (20, 20),
            ..Viewpoint::default()
        });
        o.add_person(LEAVING, (20, 20), &mut tick);
        assert_ne!(o.random, before.random);
        assert_eq!(tick.sounds.len(), 1);
    }

    /// With `fix.view_people` the door people keep their building's
    /// square, and a view turned the other way finds their door from it.
    #[test]
    fn split_doors_turn_from_home() {
        let mut w = split(world(0x14, 1, 1));
        let mut tick = Tick::default();
        w.add_person(0x28, (20, 20), &mut tick);
        let p = w.people.get(FIRST).unwrap();
        // The first rotation's door, whatever the world was told.
        assert_eq!((p.pos(), p.home()), ((21, 21), (20, 20)));
        for r in 0..4u8 {
            let turned = w.people_for_view(Viewpoint {
                rotation: r,
                ..Viewpoint::default()
            });
            let d = DOORS[usize::from(r)][0];
            assert_eq!(turned.get(FIRST).unwrap().pos(), (20 + d.0, 20 + d.1));
        }
    }

    /// A view's own builders come to the building site it looks at, from
    /// its random numbers, and go when the site does.
    #[test]
    fn view_people_come_and_go() {
        let mut w = split(world(0x1f, 1, 1));
        let random = w.random;
        let mut v = view::ViewPeople::new(5);
        let look = view::Looking::original(Viewpoint {
            origin: (20, 20),
            ..Viewpoint::default()
        });
        let mut sounds = Vec::new();
        for _ in 0..500 {
            sounds.extend(v.tick(&w, &look));
        }
        assert!(v.records().iter().any(|p| p.kind() == BUILDER && p.pos() == (20, 20)));
        assert_eq!(w.random, random);
        w.island.buildings.set((20, 20), 0x14);
        v.tick(&w, &look);
        assert_eq!(v.count(), 0);
        // With the fix off they all go.
        w.island.buildings.set((20, 20), 0x1f);
        let mut most = 0;
        while most == 0 {
            v.tick(&w, &look);
            most = most.max(v.count());
        }
        w.fixes.set(Fix::ViewPeople, false);
        v.tick(&w, &look);
        assert_eq!(v.count(), 0);
    }
}
