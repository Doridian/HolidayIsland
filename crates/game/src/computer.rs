//! The computer players (segment `10a8`): one of them takes a turn on
//! each of the monthly settlement's free steps (10a8:302e). A turn is a
//! state machine (`*1110:70ca`): think about the next step and manage the
//! loans, build, wait, or sell or demolish its worst building. Building
//! is a second state machine (`*1110:70c6`): find an area, choose a
//! category of building by what the area lacks, a type of it from
//! `COMPBAU.DAT`'s lists, a road near the area and a site near that,
//! build there (levelling the ground first if it has to), then build
//! roads from it. `docs/computer.md` has the whole of it.
//!
//! What the computers do changes only the world, so it all draws from
//! [`World::random`] in the original's order. Their status text
//! (`1110:70f2`) is saved with the map but shown nowhere.
//!
//! They level land with [`game::terrain`](crate::terrain) and lay roads
//! with [`World::build_road`] (`game::roads`), which the human's tools
//! share.

// The original's names: squares `(i, j)`, sizes `w`, `d`, the player `p`.
#![allow(clippy::many_single_char_names)]

use std::sync::Arc;

use formats::savegame::SaveGame;

use crate::roads::{CROSSABLE, NO_DRAG_GROUND};
use crate::simulation::Tick;
use crate::world::{Error, NO_BUILDING, PALM, Pos, World, on_map};

/// The status texts (`1110:70f2`), Pascal strings in the code segment.
/// The turn's.
const STATUS_IDLE: &[u8] = b"Gar nix"; // 10a8:3026
const STATUS_THINKING: &[u8] = b"WLTNaechstenSchrittueberlegen"; // 10a8:2cfa
const STATUS_DEMOLISHING: &[u8] = b"WLTGebaeudeabreissen"; // 10a8:2d18
const STATUS_WAITING: &[u8] = b"WLTAbwarten"; // 10a8:2d2d
const STATUS_SELLING: &[u8] = b"WLTGebaeudeverkaufen"; // 10a8:2d39
/// The building's.
const STATUS_NOTHING: &[u8] = b"WLBNichtstun"; // 10a8:234b
const STATUS_START_AREA: &[u8] = b"WLBStartInselsuchen"; // 10a8:2358
const STATUS_CHANGE_AREA: &[u8] = b"WLBInselwechseln"; // 10a8:236c
const STATUS_CHOOSE_CATEGORY: &[u8] = b"WLBGebaeudeartauswaehlen"; // 10a8:237d
const STATUS_CHOOSE_TYPE: &[u8] = b"WLBGebaeudewaehlen"; // 10a8:2396
const STATUS_FIND_ROAD: &[u8] = b"WLBGebaeudeBauPunktsuchen"; // 10a8:23a9
const STATUS_FIND_SITE: &[u8] = b"WLBGebaeudeBauPunktsuchen2"; // 10a8:23c3
const STATUS_BUILD: &[u8] = b"WLBGebaeudebauen"; // 10a8:23de
const STATUS_LEVEL: &[u8] = b"WLBBauplatzebenmachen"; // 10a8:23ef
const STATUS_EVEN_OUT: &[u8] = b"WLBBauplatzhoeheausgleichen"; // 10a8:2405
const STATUS_ROAD: &[u8] = b"WLBStrassebauen"; // 10a8:2421
const STATUS_ROAD_START: &[u8] = b"WLBStrassenBauPunktsuchen"; // 10a8:2431

/// A turn's states ([`Computers::state`]), by their status texts: build
/// (0, and 1 for a type an event offered, [`World::order_building`]),
/// think about the next step, sell (never set), wait, demolish.
pub const BUILDING: u8 = 0;
pub const ORDERED: u8 = 1;
pub const THINKING: u8 = 2;
pub const SELLING: u8 = 3;
pub const WAITING: u8 = 4;
pub const DEMOLISHING: u8 = 5;

/// The building's states ([`Computers::build_state`]).
pub const CHOOSE_CATEGORY: u8 = 0;
pub const CHOOSE_TYPE: u8 = 1;
pub const FIND_ROAD: u8 = 2;
pub const FIND_SITE: u8 = 3;
pub const BUILD: u8 = 4;
pub const LEVEL: u8 = 5;
pub const EVEN_OUT: u8 = 6;
pub const ROAD: u8 = 7;
pub const CHANGE_AREA: u8 = 8;
pub const START_AREA: u8 = 9;
pub const ROAD_START: u8 = 10;
pub const NOTHING: u8 = 11;

/// The categories (`COMPBAU.DAT`'s rows, [`Computers::category`]) by
/// what [`World::choose_category`] compares: transport (airports and
/// harbours), energy, water, rubbish, nature (greenery first), leisure,
/// shops, sport, restaurants, rentals; houses when nothing is lacking.
pub const TRANSPORT: u8 = 0;
pub const HOUSES: u8 = 10;
/// How many categories have a list of types. // 10a8:388b
const CATEGORIES: u8 = 11;

/// The counters in [`Computers::counters`]: tries to find a road near
/// the area, to build, to change the area, to find a road start.
const TRIES_ROAD: usize = 2;
const TRIES_BUILD: usize = 4;
const TRIES_AREA: usize = 8;
const TRIES_ROAD_START: usize = 10;

/// What [`World::can_build`] says when the ground isn't flat enough or
/// its heights differ: then the computer levels it first.
const NOT_FLAT: u8 = 0x14;
const NOT_LEVEL: u8 = 0xc;

/// The buildings that don't take a square for the computers: palms,
/// beach life and none. // 1110:1358 (`set of Byte`)
const FREE: [u8; 3] = [PALM, 36, NO_BUILDING];
/// The forms a road can be built on: flat, or one side raised.
/// // 1110:13d8 (`set of Byte`)
const ROAD_FORMS: [u8; 5] = [0, 1, 2, 3, 16];

/// The size of a list of `COMPBAU.DAT` types (the count, then 40), and of
/// all 13 of them. // *1110:70ea, 0x215 bytes
const TYPE_ROW: usize = 41;
const TYPES_SIZE: usize = 13 * TYPE_ROW;
/// The building types [`World::build_type_lists`] looks at. // 10a8:388b
const LAST_TYPE: u8 = 0x7e;

/// Three byte lists of up to 254 entries (`1030:0ad2`..`1030:0c62`): the
/// direction, `i` and `j` of the squares a road may start from. Each is
/// 256 bytes, the count first, as the map saves them.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Candidates {
    lists: [Vec<u8>; 3],
}

impl Default for Candidates {
    /// Three empty lists. // 1030:0c82
    fn default() -> Self {
        Candidates {
            lists: [vec![0; 256], vec![0; 256], vec![0; 256]],
        }
    }
}

/// Appends `value` to a byte list: its new count, or 255 when it's full.
/// // 1030:0af2
fn list_push(list: &mut [u8], value: u8) -> u8 {
    if list[0] < 0xfe {
        list[0] += 1;
        list[usize::from(list[0])] = value;
        list[0]
    } else {
        0xff
    }
}

/// The list's entry `n` (from 1), 255 past its end. // 1030:0b3d
fn list_get(list: &[u8], n: u8) -> u8 {
    if list[0] < n || n == 0 {
        0xff
    } else {
        list[usize::from(n)]
    }
}

/// Takes entry `n` out of the list, the ones after it moving up.
/// // 1030:0b76
fn list_remove(list: &mut [u8], n: u8) -> u8 {
    if list[0] < n || n == 0 {
        return 0xff;
    }
    let value = list[usize::from(n)];
    let count = usize::from(list[0]);
    list.copy_within(usize::from(n) + 1..=count, usize::from(n));
    list[0] -= 1;
    value
}

impl Candidates {
    fn from_fields(fields: &[&[u8]]) -> Option<Self> {
        let mut c = Candidates::default();
        for (list, field) in c.lists.iter_mut().zip(fields) {
            if field.len() != 256 {
                return None;
            }
            list.copy_from_slice(field);
        }
        Some(c)
    }

    /// Empties the lists. // 1030:0e15
    pub fn clear(&mut self) {
        for list in &mut self.lists {
            list[0] = 0;
        }
    }

    /// Appends a square and its direction, if it's on the map; 255 if
    /// that or a list is full (the others keep the entry). // 1030:0caf
    pub fn push(&mut self, direction: u8, (i, j): Pos) -> u8 {
        if !on_map((i, j)) {
            return 0xff;
        }
        let [d, li, lj] = &mut self.lists;
        let n = list_push(d, direction);
        if n == 0xff || list_push(li, i.to_le_bytes()[0]) != n || list_push(lj, j.to_le_bytes()[0]) != n {
            0xff
        } else {
            n
        }
    }

    /// How many entries there are, 255 if the lists disagree. // 1030:0dcb
    #[must_use]
    pub fn len(&self) -> u8 {
        let n = self.lists[0][0];
        if self.lists[1][0] != n || self.lists[2][0] != n {
            0xff
        } else {
            n
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Entry `n` (from 1): its direction and square. // 1030:0d20
    #[must_use]
    pub fn get(&self, n: u8) -> (u8, Pos) {
        let i = i16::from(list_get(&self.lists[1], n));
        let j = i16::from(list_get(&self.lists[2], n));
        (list_get(&self.lists[0], n), (i, j))
    }

    /// Takes entry `n` out. // 1030:0d71
    pub fn remove(&mut self, n: u8) {
        let [d, li, lj] = &mut self.lists;
        if list_remove(d, n) != 0xff && list_remove(lj, n) != 0xff {
            list_remove(li, n);
        }
    }
}

/// The computer players' state, saved with the map (unit 10a8:32ec).
/// Arrays of 8 are by player 1 to 8, those of 7 by player 2 to 8.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Computers {
    /// The turns spent waiting. // 1110:7062, `array[1..8] of Word`
    pub waited: [u16; 8],
    /// The turns before demolishing. // 1110:7072
    pub demolish_wait: [u16; 8],
    /// Levelling a site: 0 work out the height, 1 level the corners to it.
    /// // 1110:7082
    pub levelling: [u16; 8],
    /// The height the site is levelled to. // 1110:7092
    pub level: [u8; 8],
    /// Who owns the square that stopped the levelling. // 1110:709a
    pub blocked_by: [u8; 8],
    /// The square the computer works at: the area it builds in, then the
    /// site, then a road's end. // *1110:70a2
    pub site: [Pos; 7],
    /// A road near the area, the site is looked for around it.
    /// // *1110:70a6
    pub centre: [Pos; 7],
    /// The square the next road starts from. // *1110:70aa
    pub road: [Pos; 7],
    /// Its direction: 0 `j + 1`, 1 `i + 1`, 2 `j - 1`, 3 `i - 1`.
    /// // *1110:70ae
    pub road_direction: [u8; 7],
    /// Set when the game's data is loaded, then never read: `Random(7)`,
    /// 1, `Random(2) + 7` and the first again. // *1110:70b2 .. *1110:70be
    pub unknown: [[i16; 7]; 4],
    /// The building's state. // *1110:70c6
    pub build_state: [u8; 7],
    /// The turn's state. // *1110:70ca
    pub state: [u8; 7],
    /// The category it builds. // *1110:70ce
    pub category: [u8; 7],
    /// It builds in a new area: no road is needed near the site.
    /// // *1110:70d2
    pub new_area: [u8; 7],
    /// The area has no road yet: the first ones start around the
    /// transport building ([`Computers::road_ends`]). // *1110:70d6
    pub first_road: [u8; 7],
    /// The building type it builds. // *1110:70da
    pub building: [u8; 7],
    /// Counters, 12 bytes each; 2, 4, 8 and 10 are used. // *1110:70de
    pub counters: [[u8; 12]; 7],
    /// Where an area's first roads may start: around its transport
    /// building. // *1110:70e2
    pub road_ends: Vec<Candidates>,
    /// Where roads may go on from: the ends of those built. // *1110:70e6
    pub roads: Vec<Candidates>,
    /// For each category the types the computers build, `COMPBAU.DAT`'s
    /// matches: 13 rows of a count and 40 types, 11 used. Kept as one
    /// block, as the original reads and writes past a row.
    /// // *1110:70ea, 0x215 bytes
    pub types: Vec<u8>,
    /// `COMPBAU.DAT`. // *1110:70ee
    pub compbau: Vec<[u8; 7]>,
    /// What the last one did, a status text. // 1110:70f2, `string`
    pub status: Vec<u8>,
    /// Not known yet: 20 at first (1000:1ea0), the lowest height a site's
    /// levelling starts from. // 1110:00a2
    pub height_limit: u16,
}

impl Default for Computers {
    fn default() -> Self {
        Computers {
            waited: [0; 8],
            demolish_wait: [0; 8],
            levelling: [0; 8],
            level: [0; 8],
            blocked_by: [0; 8],
            site: [(0, 0); 7],
            centre: [(0, 0); 7],
            road: [(0, 0); 7],
            road_direction: [0; 7],
            unknown: [[0; 7]; 4],
            build_state: [0; 7],
            state: [0; 7],
            category: [0; 7],
            new_area: [0; 7],
            first_road: [0; 7],
            building: [0; 7],
            counters: [[0; 12]; 7],
            road_ends: vec![Candidates::default(); 7],
            roads: vec![Candidates::default(); 7],
            types: vec![0; TYPES_SIZE],
            compbau: vec![[0; 7]; 13],
            status: Vec::new(),
            height_limit: 20,
        }
    }
}

impl Computers {
    /// The state as the map saved it (unit 10a8:32ec, and `1110:00a2`
    /// from 1000:0c6a). The candidate lists are saved for players 2 to
    /// the map's player count; the others start empty.
    ///
    /// # Errors
    /// If a field is missing or has the wrong size.
    pub fn from_map(save: &SaveGame) -> Result<Self, Error> {
        let field = |dest: &'static str| save.fields(dest).last().ok_or(Error::Field(dest));
        let sized = |dest: &'static str, n: usize| {
            let f = field(dest)?;
            if f.len() == n { Ok(f) } else { Err(Error::Field(dest)) }
        };
        let words = |dest| -> Result<[u16; 8], Error> {
            let f = sized(dest, 16)?;
            Ok(std::array::from_fn(|i| u16::from_le_bytes([f[2 * i], f[2 * i + 1]])))
        };
        let bytes8 = |dest| -> Result<[u8; 8], Error> { Ok(sized(dest, 8)?.try_into().unwrap_or([0; 8])) };
        let bytes7 = |dest| -> Result<[u8; 7], Error> { Ok(sized(dest, 7)?.try_into().unwrap_or([0; 7])) };
        let signed = |f: &[u8], i: usize| i16::from_le_bytes([f[2 * i], f[2 * i + 1]]);
        let squares = |dest| -> Result<[Pos; 7], Error> {
            let f = sized(dest, 0x1c)?;
            Ok(std::array::from_fn(|n| (signed(f, 2 * n), signed(f, 2 * n + 1))))
        };
        let integers = |dest| -> Result<[i16; 7], Error> {
            let f = sized(dest, 0xe)?;
            Ok(std::array::from_fn(|n| signed(f, n)))
        };
        let lists = |dest: &'static str| -> Result<Vec<Candidates>, Error> {
            let fields: Vec<&[u8]> = save.fields(dest).collect();
            let mut out = vec![Candidates::default(); 7];
            for (c, chunk) in out.iter_mut().zip(fields.chunks(3)) {
                *c = Candidates::from_fields(chunk).ok_or(Error::Field(dest))?;
            }
            Ok(out)
        };
        let counters = sized("*1110:70de", 0x54)?;
        let compbau = sized("*1110:70ee", 0x5b)?;
        let status = sized("1110:70f2", 0x100)?;
        let limit = sized("1110:00a2", 2)?;
        Ok(Computers {
            waited: words("1110:7062")?,
            demolish_wait: words("1110:7072")?,
            levelling: words("1110:7082")?,
            level: bytes8("1110:7092")?,
            blocked_by: bytes8("1110:709a")?,
            site: squares("*1110:70a2")?,
            centre: squares("*1110:70a6")?,
            road: squares("*1110:70aa")?,
            road_direction: bytes7("*1110:70ae")?,
            unknown: [
                integers("*1110:70b2")?,
                integers("*1110:70b6")?,
                integers("*1110:70ba")?,
                integers("*1110:70be")?,
            ],
            build_state: bytes7("*1110:70c6")?,
            state: bytes7("*1110:70ca")?,
            category: bytes7("*1110:70ce")?,
            new_area: bytes7("*1110:70d2")?,
            first_road: bytes7("*1110:70d6")?,
            building: bytes7("*1110:70da")?,
            counters: std::array::from_fn(|n| std::array::from_fn(|c| counters[12 * n + c])),
            road_ends: lists("*[1110:70e2[i]]")?,
            roads: lists("*[1110:70e6[i]]")?,
            types: sized("*1110:70ea", TYPES_SIZE)?.to_vec(),
            compbau: compbau.as_chunks::<7>().0.to_vec(),
            status: status[1..=usize::from(status[0])].to_vec(),
            height_limit: u16::from_le_bytes([limit[0], limit[1]]),
        })
    }

    /// Whether `building` is in `COMPBAU.DAT`'s row `category`: the row's
    /// `HAUSDATA.DAT` column 4, one of its two kinds (column 5) and one
    /// of its four types, 255 matching anything. // 10a8:03a6
    fn matches(&self, category: u8, building: u8, hausdata: impl Fn(usize) -> i16) -> bool {
        let Some(r) = self.compbau.get(usize::from(category)) else {
            return false;
        };
        let any = |v: u8| v == 0xff;
        (any(r[0]) || i16::from(r[0]) == hausdata(4))
            && (any(r[1]) || i16::from(r[1]) == hausdata(5) || any(r[2]) || i16::from(r[2]) == hausdata(5))
            && r[3..].iter().any(|&t| any(t) || t == building)
    }

    /// The count of `category`'s list of types.
    fn type_count(&self, category: u8) -> u8 {
        self.types.get(usize::from(category) * TYPE_ROW).copied().unwrap_or(0)
    }

    /// Appends `building` to `category`'s list, not checking that it's
    /// full: past 40 it runs into the next row. // 10a8:0523
    fn add_type(&mut self, category: u8, building: u8) {
        let row = usize::from(category) * TYPE_ROW;
        let n = self.type_count(category).wrapping_add(1);
        if let Some(c) = self.types.get_mut(row) {
            *c = n;
        }
        if let Some(t) = self.types.get_mut(row + usize::from(n)) {
            *t = building;
        }
    }
}

/// The index of player `p` (2 to 8) in the arrays of 7.
fn slot(p: u8) -> usize {
    usize::from(p - 2)
}

impl World {
    /// The computer players' lists of types from `COMPBAU.DAT` (rows 0 to
    /// 10): each type up to 0x7e whose row matches and that isn't one of
    /// a kind (`HAUSDATA.DAT` column 6 is 0). The original makes them when
    /// it loads the game's data, before the map replaces them. // 10a8:388b
    pub fn build_type_lists(&mut self) {
        let t = Arc::clone(&self.tables);
        let c = &mut self.computers;
        for category in 0..CATEGORIES {
            let row = usize::from(category) * TYPE_ROW;
            let mut n: u8 = 0;
            for building in 0..=LAST_TYPE {
                if c.matches(category, building, |col| t.hausdata(building, col)) && t.hausdata(building, 6) == 0 {
                    n = n.wrapping_add(1);
                    // BUG(original): more than 40 types write on into the next row.
                    if let Some(cell) = c.types.get_mut(row + usize::from(n)) {
                        *cell = building;
                    }
                }
            }
            c.types[row] = n;
        }
    }

    /// The computer players start out: each looks for an area from a
    /// random square (`Random(200) + 25` twice), thinking first, with no
    /// budgets and empty lists; then `Random(7)` and `Random(2)` for
    /// values nothing reads. The turns count from 0. The original does it
    /// when it loads the game's data, before the map replaces it.
    /// // 10a8:3558
    pub fn init_computers(&mut self) {
        for p in 2..=self.player_count.min(8) {
            let (d, k) = (usize::from(p - 1), slot(p));
            let c = &mut self.computers;
            c.waited[d] = 0;
            c.demolish_wait[d] = 0;
            c.levelling[d] = 0;
            c.level[d] = 0;
            c.blocked_by[d] = 0;
            let i = self.random.random(200).cast_signed().wrapping_add(25);
            let j = self.random.random(200).cast_signed().wrapping_add(25);
            c.site[k] = (i, j);
            c.centre[k] = (i, j);
            c.unknown[0][k] = self.random.random(7).cast_signed();
            c.unknown[1][k] = 1;
            c.unknown[2][k] = self.random.random(2).cast_signed().wrapping_add(7);
            c.unknown[3][k] = c.unknown[0][k];
            c.build_state[k] = START_AREA;
            c.state[k] = THINKING;
            c.category[k] = 0;
            c.building[k] = NO_BUILDING;
            for b in &mut self.computer_budgets[k * 13..k * 13 + 13] {
                *b = 0;
            }
            c.new_area[k] = 0;
            c.first_road[k] = 0;
            c.roads[k].clear();
            c.road_ends[k].clear();
            c.road[k] = c.site[k];
            c.road_direction[k] = 0;
            for n in &mut c.counters[k][4..=11] {
                *n = 0;
            }
        }
        self.computer_turns = 0;
    }

    /// What each player would pay at most in the auction of
    /// [`World::auction`]'s item at `price`, into
    /// [`Auction::limits`](crate::auction::Auction::limits), players 1 to 8
    /// in turn. // 10a8:0373
    pub(crate) fn computer_bids(&mut self, price: i32) {
        for p in 1..=8 {
            let limit = self.bid_limit(p, price); // 10a8:0221
            self.auction.limits[usize::from(p - 1)] = limit;
        }
    }

    /// Player `p`'s limit at `price`: the human's money (the original's
    /// player 1); a computer that's in the game and not the building's
    /// owner bids for a building or a bankrupt player's property, and for
    /// anything else unless its area still waits for its first roads;
    /// nobody else bids. No one bids for their own building (the square
    /// of event 9). // 10a8:0221
    fn bid_limit(&mut self, p: u8, price: i32) -> i32 {
        let item = self.auction.item;
        let square = self.events.get(9).map(|e| e.pos);
        let owns = |w: &World| square.is_some_and(|pos| on_map(pos) && w.island.owners.at(pos) == p);
        if p < 2 || p > self.player_count {
            if p != 1 || (item == crate::auction::BUILDING_SALE && owns(self)) {
                0
            } else {
                self.money_of(1) // 1078:0191
            }
        } else if self.player_out(p) != 0 {
            0
        } else if item == crate::auction::BUILDING_SALE {
            if owns(self) { 0 } else { self.computer_bid(p, price) }
        } else if item == crate::auction::PROPERTY_SALE || self.computers.first_road[slot(p)] == 0 {
            self.computer_bid(p, price)
        } else {
            0
        }
    }

    /// What computer `p` bids at most at `price`: nothing if `price` is
    /// above what it can pay (its money less its loan plus `MaxSchulden`),
    /// else `price` and a random part (`Random(1000)` of 999) of the way to
    /// what it can pay or three times `price`, whichever is less.
    /// // 10a8:0167
    fn computer_bid(&mut self, p: u8, price: i32) -> i32 {
        let can = self
            .money_of(p)
            .wrapping_sub(self.loan_of(p))
            .wrapping_add(self.finances.max_debt);
        if price > can {
            return 0;
        }
        let top = can.min(price.wrapping_mul(3)); // 1030:0f53
        let r = i32::from(self.random.random(1000));
        // BUG(original): the product overflows for a range past about 2 150 000, and the bid wraps.
        price.wrapping_add(top.wrapping_sub(price).wrapping_mul(r) / 999)
    }

    /// An event makes `building` available: the build menu gets it
    /// (`kind` 0 and 1, the view's, 1018:23f8) and with `kind` 0 the
    /// computers' lists too, the hotels 0x60, 0x64 and 0x68 with the
    /// three types after them and 0x2c, 0x2e, 0x30 and 0x32 with the one
    /// after; `kind` 2 to 7 is a player who builds it next. // 10a0:0691
    pub(crate) fn make_available(&mut self, building: u8, kind: u8) {
        match kind {
            0 => {
                let more = match building {
                    0x60 | 0x64 | 0x68 => 3,
                    0x2c | 0x2e | 0x30 | 0x32 => 1,
                    _ => 0,
                };
                for n in 0..=more {
                    self.offer_type(building.wrapping_add(n)); // 10a8:0523
                }
            }
            2..=7 => self.order_building(kind, building), // 10a8:2cc1
            _ => {}
        }
    }

    /// The computers' lists of each category whose `COMPBAU.DAT` row
    /// `building` matches get it, one of a kind or not. // 10a8:0523
    fn offer_type(&mut self, building: u8) {
        let t = Arc::clone(&self.tables);
        for category in 0..CATEGORIES {
            if self
                .computers
                .matches(category, building, |col| t.hausdata(building, col))
            {
                self.computers.add_type(category, building);
            }
        }
    }

    /// Computer player `p` builds `building` next: it looks for a road for
    /// it right away. // 10a8:2cc1
    pub fn order_building(&mut self, p: u8, building: u8) {
        if !(2..=8).contains(&p) {
            return;
        }
        let k = slot(p);
        let c = &mut self.computers;
        c.state[k] = ORDERED;
        c.building[k] = building;
        c.build_state[k] = FIND_ROAD;
    }

    /// The status text: what the computer player last did. // 1110:70f2
    fn set_status(&mut self, text: &[u8]) {
        self.computers.status.clear();
        self.computers.status.extend_from_slice(text);
    }

    /// The computers' turns: the status text is `Gar nix`, the turns count
    /// up and player `turns mod opponents + 2` takes one. // 10a8:302e
    pub(crate) fn computer_players_tick(&mut self, tick: &mut Tick) {
        self.set_status(STATUS_IDLE);
        self.computer_turns = self.computer_turns.wrapping_add(1);
        // `computer_opponents` (1110:1478), which the start screen keeps
        // one below the player count.
        let opponents = i32::from(self.player_count.saturating_sub(1));
        if opponents > 0 {
            // 1108:14e1, a signed `Longint` division's remainder.
            let player = crate::settlement::rem(self.computer_turns, opponents).wrapping_add(2);
            self.computer_turn(player.to_le_bytes()[0], tick);
        }
    }

    /// Computer player `p`'s turn, unless it's out: by its state, build;
    /// think (manage the loans, then build, or wait while the bank says no
    /// and it's in the red; demolish one time in 1000 or 100); wait 11
    /// turns; or demolish: every other turn, its building that earns least
    /// below its costs is sold through event 9 (one time in five) or
    /// demolished. // 10a8:2d5a
    pub(crate) fn computer_turn(&mut self, p: u8, tick: &mut Tick) {
        // The original's arrays have players 2 to 8.
        if !(2..=8).contains(&p) || self.player_out(p) != 0 {
            return;
        }
        let (d, k) = (usize::from(p - 1), slot(p));
        match self.computers.state[k] {
            BUILDING | ORDERED => self.build_step(p, tick),
            THINKING => {
                self.set_status(STATUS_THINKING);
                self.manage_loans(p);
                self.computers.state[k] = if !self.finances.credit_refused[d] || self.money_of(p) >= 0 {
                    if self.random.random(1000) == 0 {
                        DEMOLISHING
                    } else {
                        BUILDING
                    }
                } else if self.random.random(100) == 0 {
                    DEMOLISHING
                } else {
                    WAITING
                };
            }
            SELLING => self.set_status(STATUS_SELLING),
            WAITING => {
                self.set_status(STATUS_WAITING);
                let c = &mut self.computers;
                c.waited[d] = c.waited[d].wrapping_add(1);
                if c.waited[d] > 10 {
                    c.waited[d] = 0;
                    c.state[k] = THINKING;
                }
            }
            DEMOLISHING => {
                self.set_status(STATUS_DEMOLISHING);
                let c = &mut self.computers;
                c.demolish_wait[d] = c.demolish_wait[d].wrapping_add(1);
                if c.demolish_wait[d] > 1 {
                    c.demolish_wait[d] = 0;
                    self.computers.state[k] = match self.worst_building(p) {
                        None => WAITING,
                        Some(pos) => {
                            self.get_rid_of(p, pos, tick);
                            THINKING
                        }
                    };
                }
            }
            _ => {}
        }
    }

    /// Computer player `p` sells the building at `pos` (`Random(100)`
    /// below 20): event 9 is made certain with the price, twice
    /// `HAUSPREI.DAT` column 2 for a type of price 10 or less, else a
    /// quarter of the price. Otherwise it's demolished. // 10a8:2d5a
    fn get_rid_of(&mut self, p: u8, pos: Pos, tick: &mut Tick) {
        if self.random.random(100) < 20 {
            let origin = self.building_origin(pos); // 1030:154c
            let b = self.island.buildings.get(origin).unwrap_or(NO_BUILDING);
            if b != NO_BUILDING {
                let price = self.tables.hausprei(b, 0);
                let value = if price <= 10 {
                    self.tables.hausprei(b, 2).wrapping_mul(2)
                } else {
                    price / 4
                };
                self.schedule_event(9, value, i32::from(p), origin); // 1010:3006
            }
        } else {
            self.remove_people_from(pos, tick); // 1088:6975
            // The errors are only shown to player 1.
            let _ = self.demolish(p, pos, tick); // 1078:280b
            self.people_map_changed(); // 1088:6c5a
        }
    }

    /// While `p` is in the red it borrows `Kreditgroesse` at a time until
    /// the bank says no; while it has more than that and owes as much, it
    /// pays that much back. // 10a8:2a60
    ///
    /// The original loops for ever with a `Kreditgroesse` of 0 or less;
    /// the port does nothing then.
    pub fn manage_loans(&mut self, p: u8) {
        let size = self.finances.credit_size;
        // BUG(original): a `Kreditgroesse` of 0 or less loops for ever; the port skips the loans.
        if size <= 0 || p == 0 {
            return;
        }
        let refused = |w: &World| {
            w.finances
                .credit_refused
                .get(usize::from(p - 1))
                .copied()
                .unwrap_or(true)
        };
        while self.money_of(p) < 0 && !refused(self) {
            self.take_loan(p, size); // 1078:05c6
        }
        while self.money_of(p) > size && self.loan_of(p) >= size {
            self.repay_loan(p, size); // 1078:061b
        }
    }

    /// `p`'s building that loses money (its costs, `HAUSPREI.DAT` column
    /// 1, above its takings) and is least full: a house by its guests in
    /// percent of its beds, then a facility (not one of a kind) by its
    /// fill, below 100. // 10a8:2ade
    ///
    /// A house type without beds stops the original with a division by
    /// zero; the port skips the house.
    #[must_use]
    pub fn worst_building(&self, p: u8) -> Option<Pos> {
        let t = &self.tables;
        let owner = |pos| self.island.owners.get(pos).unwrap_or(0);
        let mut best: i16 = 100;
        let mut found = None;
        for h in &self.houses {
            let b = h.building();
            if owner(h.pos()) != p {
                continue;
            }
            let takings = i32::from(h.guests()).wrapping_mul(h.price());
            let beds = t.hausdata(b, 3);
            // BUG(original): a house type without beds divides by zero; skipped.
            if t.hausprei(b, 1) > takings && beds != 0 {
                let full = h.guests().wrapping_mul(100).wrapping_div(beds);
                if full < best {
                    found = Some(h.pos());
                    best = full;
                }
            }
        }
        for f in &self.facilities {
            let b = f.building();
            if owner(f.pos()) != p {
                continue;
            }
            let takings = i32::from(f.fill()).wrapping_mul(f.price()) / 100;
            if t.hausprei(b, 1) > takings && i16::from(f.fill()) < best && t.hausdata(b, 6) != 1 {
                found = Some(f.pos());
                best = i16::from(f.fill());
            }
        }
        if best < 100 { found } else { None }
    }

    /// One step of `p`'s building, by its state. // 10a8:2463
    #[allow(clippy::too_many_lines)] // One state after the other.
    fn build_step(&mut self, p: u8, tick: &mut Tick) {
        let k = slot(p);
        match self.computers.build_state[k] {
            CHOOSE_CATEGORY => {
                self.set_status(STATUS_CHOOSE_CATEGORY);
                self.choose_category(p);
                self.computers.build_state[k] = CHOOSE_TYPE;
            }
            CHOOSE_TYPE => {
                self.set_status(STATUS_CHOOSE_TYPE);
                let chosen = self.choose_type(p);
                let c = &mut self.computers;
                if chosen {
                    c.build_state[k] = FIND_ROAD;
                    c.counters[k][TRIES_ROAD] = 0;
                } else {
                    c.build_state[k] = CHOOSE_CATEGORY;
                    c.state[k] = WAITING;
                }
            }
            FIND_ROAD => {
                self.set_status(STATUS_FIND_ROAD);
                let c = &mut self.computers;
                c.counters[k][TRIES_ROAD] = c.counters[k][TRIES_ROAD].wrapping_add(1);
                let squares = i32::from(self.tables.hausnr(c.building[k], 5));
                if i32::from(c.counters[k][TRIES_ROAD]) > squares.min(8).wrapping_mul(4) {
                    c.build_state[k] = ROAD_START;
                    if c.state[k] != ORDERED {
                        c.state[k] = THINKING;
                    }
                } else if self.find_road_near(p) {
                    self.computers.build_state[k] = FIND_SITE;
                }
            }
            FIND_SITE => {
                self.set_status(STATUS_FIND_SITE);
                let found = self.find_site(p);
                let c = &mut self.computers;
                if found {
                    c.build_state[k] = BUILD;
                    c.counters[k][TRIES_BUILD] = 0;
                } else {
                    c.build_state[k] = FIND_ROAD;
                }
            }
            BUILD => {
                self.set_status(STATUS_BUILD);
                let c = &mut self.computers;
                c.counters[k][TRIES_BUILD] = c.counters[k][TRIES_BUILD].wrapping_add(1);
                let error = self.build_on_site(p, tick);
                let c = &mut self.computers;
                if error == 0 {
                    c.state[k] = THINKING;
                    if c.new_area[k] == 0 {
                        c.build_state[k] = CHOOSE_CATEGORY;
                    } else {
                        c.build_state[k] = ROAD_START;
                        c.new_area[k] = 0;
                    }
                } else if c.counters[k][TRIES_BUILD] < 2 {
                    c.build_state[k] = match error {
                        NOT_FLAT => LEVEL,
                        NOT_LEVEL => EVEN_OUT,
                        _ => FIND_ROAD,
                    };
                } else {
                    c.build_state[k] = FIND_ROAD;
                }
            }
            LEVEL | EVEN_OUT => {
                let text = if self.computers.build_state[k] == LEVEL {
                    STATUS_LEVEL
                } else {
                    STATUS_EVEN_OUT
                };
                self.set_status(text);
                if self.level_site(p, tick) {
                    self.computers.build_state[k] = BUILD;
                }
            }
            ROAD => {
                self.set_status(STATUS_ROAD);
                // Not while the human drags a road (marked).
                if !self.road_dragging {
                    let built = self.build_road_on(p, tick);
                    let c = &mut self.computers;
                    if built {
                        if c.state[k] == ORDERED {
                            c.build_state[k] = FIND_ROAD;
                            c.counters[k][TRIES_ROAD] = 0;
                        } else {
                            c.state[k] = THINKING;
                            c.build_state[k] = CHOOSE_CATEGORY;
                        }
                        c.counters[k][TRIES_ROAD_START] = 0;
                        c.first_road[k] = 0;
                    } else {
                        c.build_state[k] = ROAD_START;
                    }
                }
            }
            CHANGE_AREA => {
                self.set_status(STATUS_CHANGE_AREA);
                let changed = self.change_area(p);
                let c = &mut self.computers;
                if !changed {
                    c.counters[k][TRIES_AREA] = c.counters[k][TRIES_AREA].wrapping_add(1);
                }
                if changed || c.counters[k][TRIES_AREA] > 99 {
                    c.counters[k][TRIES_AREA] = 0;
                    self.new_area(p);
                }
            }
            START_AREA => {
                self.set_status(STATUS_START_AREA);
                if self.start_area(p) {
                    self.new_area(p);
                }
            }
            ROAD_START => {
                self.set_status(STATUS_ROAD_START);
                let c = &mut self.computers;
                c.counters[k][TRIES_ROAD_START] = c.counters[k][TRIES_ROAD_START].wrapping_add(1);
                if c.counters[k][TRIES_ROAD_START] < 21 {
                    if self.find_road_start(p, tick) {
                        self.computers.build_state[k] = ROAD; // 10a8:2060
                    }
                } else {
                    c.counters[k][TRIES_ROAD_START] = 0;
                    c.build_state[k] = CHANGE_AREA;
                }
            }
            NOTHING => self.set_status(STATUS_NOTHING),
            _ => {}
        }
    }

    /// `p` builds in a new area next: transport first, and its first roads
    /// around that. // 10a8:2463 states 8, 9
    fn new_area(&mut self, p: u8) {
        let k = slot(p);
        let c = &mut self.computers;
        c.build_state[k] = CHOOSE_TYPE;
        c.category[k] = TRANSPORT;
        c.new_area[k] = 1;
        c.first_road[k] = 1;
    }

    /// `p`'s budget `n`: the capacity of its building sites of category
    /// `n`, which [`World::finish_building`] takes off again.
    fn budget(&self, p: u8, n: u8) -> i32 {
        self.computer_budgets
            .get(slot(p) * 13 + usize::from(n))
            .copied()
            .unwrap_or(0)
    }

    /// The category `p` builds next, by what the plot of its square lacks
    /// for its beds (`+0x22`), counting what's being built ([budgets]):
    /// airports and harbours' places (`+0xde`) below the beds; then energy
    /// (`+0xe2`), water (`+0xe6`) and rubbish (`+0xea`), each with
    /// `Random(50)`, below three fifths of the beds; greenery (`+3`, with
    /// `Random(20)`) up to 51; then nature (`+0xda`), leisure (`+0xd6`),
    /// shops (`+0x7a`), sport (`+0xd2`), rentals (`+0xa6`) and restaurants
    /// (`+0x4e`), each with `Random(40)`, below three fifths of the beds;
    /// else houses. In a new area it's transport; without a plot it stays.
    /// // 10a8:0987
    ///
    /// [budgets]: World::computer_budgets
    fn choose_category(&mut self, p: u8) -> bool {
        let k = slot(p);
        let plot_no = self.island.plot_at(self.computers.site[k]); // 1030:1e62
        if plot_no == 0 {
            return false;
        }
        if self.computers.new_area[k] != 0 {
            self.computers.category[k] = TRANSPORT;
            return true;
        }
        // 1068:2591.
        // BUG(original): a plot number without a record is read through a nil pointer; the port reads 0.
        let plot = self
            .plots
            .iter()
            .find(|q| q.id() >= 0 && q.id().cast_unsigned() == plot_no);
        let value = |at: usize| plot.map_or(0, |q| q.longint(at));
        let beds = value(0x22);
        let ports = value(0xde);
        let utilities = [(1, value(0xe2)), (2, value(0xe6)), (3, value(0xea))];
        let greenery = plot.map_or(0, |q| q.byte(3));
        let facilities = [
            (4, value(0xda)),
            (5, value(0xd6)),
            (6, value(0x7a)),
            (7, value(0xd2)),
            (9, value(0xa6)),
            (8, value(0x4e)),
        ];
        let wanted = beds.wrapping_mul(3) / 5;
        // 10a8:0927
        let fits = |w: &mut World, n: u8, wanted: i32, have: i32| {
            let fits = w.budget(p, n).wrapping_add(have) <= wanted;
            if fits {
                w.computers.category[k] = n;
            }
            fits
        };
        if fits(self, 0, beds, ports) {
            return true;
        }
        for (n, have) in utilities {
            let have = i32::from(self.random.random(50)).wrapping_add(have);
            if fits(self, n, wanted, have) {
                return true;
            }
        }
        let have = i32::from(greenery) + i32::from(self.random.random(20));
        if fits(self, 4, 51, have) {
            return true;
        }
        for (n, have) in facilities {
            let have = i32::from(self.random.random(40)).wrapping_add(have);
            if fits(self, n, wanted, have) {
                return true;
            }
        }
        self.computers.category[k] = HOUSES;
        true
    }

    /// A type from `p`'s category's list, `Random(count)`; none for a
    /// category past 10. // 10a8:078a
    fn choose_type(&mut self, p: u8) -> bool {
        let k = slot(p);
        let category = self.computers.category[k];
        if category > 10 {
            return false;
        }
        let row = usize::from(category) * TYPE_ROW;
        let r = self.random.random(u16::from(self.computers.type_count(category)));
        self.computers.building[k] = self.computers.types.get(row + 1 + usize::from(r)).copied().unwrap_or(0);
        true
    }

    /// A road on the plot of `p`'s square (any square of it in a new
    /// area) found by a random walk from it, which becomes the centre the
    /// site is looked for around. // 10a8:13f6
    fn find_road_near(&mut self, p: u8) -> bool {
        let k = slot(p);
        match self.walk_to_road(p, self.computers.site[k]) {
            Some(pos) => {
                self.computers.centre[k] = pos;
                true
            }
            None => false,
        }
    }

    /// 100 steps of a random walk from `start`, `Random(5) - 2` on `i` then
    /// `j`, kept on the map, until a square on the same plot (a road
    /// unless `p` is in a new area). // 10a8:0e30
    fn walk_to_road(&mut self, p: u8, start: Pos) -> Option<Pos> {
        let plot = self.island.plot_at(start);
        let (mut i, mut j) = start;
        for _ in 0..100 {
            let mut step = |v: i16| {
                let r = i32::from(self.random.random(5));
                i16::try_from((i32::from(v) - 2 + r).clamp(0, 250)).unwrap_or(0)
            };
            i = step(i);
            j = step(j);
            let q = self.island.plot_at((i, j));
            if q.cast_signed() >= 0
                && q == plot
                && (self.computers.new_area[slot(p)] != 0 || (10..=58).contains(&self.island.ground.at((i, j))))
            {
                return Some((i, j));
            }
        }
        None
    }

    /// A site for `p`'s building around its centre, which becomes its
    /// square. // 10a8:1454
    fn find_site(&mut self, p: u8) -> bool {
        let k = slot(p);
        match self.try_sites(p, self.computers.centre[k]) {
            Some(pos) => {
                self.computers.site[k] = pos;
                true
            }
            None => false,
        }
    }

    /// Up to 51 random sites within 8 squares of `centre` (`Random(17)`
    /// for `i`, then `j`, from `centre - size - 7`) on its plot, until
    /// one fits ([`World::site_fits`]). // 10a8:1309
    fn try_sites(&mut self, p: u8, (i, j): Pos) -> Option<Pos> {
        let building = self.computers.building[slot(p)];
        let plot = self.island.plot_at((i, j));
        if plot == 0 {
            return None;
        }
        let (w, d) = (self.tables.hausnr(building, 3), self.tables.hausnr(building, 4));
        for _ in 0..=50 {
            let a = i
                .wrapping_sub(7)
                .wrapping_sub(w)
                .wrapping_add(self.random.random(17).cast_signed());
            let b = j
                .wrapping_sub(7)
                .wrapping_sub(d)
                .wrapping_add(self.random.random(17).cast_signed());
            if self.site_fits(p, building, plot, (a, b)) {
                return Some((a, b));
            }
        }
        None
    }

    /// Whether `building` fits at `pos` for `p`: on `plot`, on free land
    /// ([`World::area_free`]), and unless it's a new area, with `p`'s
    /// road along it and allowed there. // 10a8:1269
    fn site_fits(&self, p: u8, building: u8, plot: u16, pos: Pos) -> bool {
        let q = self.island.plot_at(pos);
        if q.cast_signed() < 0 || q != plot || !self.area_free(building, pos) {
            return false;
        }
        if self.computers.new_area[slot(p)] != 0 {
            return true;
        }
        // The size as `Byte`s.
        let size = |c| i16::from(self.tables.hausnr(building, c).to_le_bytes()[0]);
        self.road_along(size(3), size(4), pos, p) && self.site_allowed(p, building, pos)
    }

    /// Whether `building`'s area from `(i, j)` is on the map with no
    /// building but palms and beach life, on plain ground (no road or
    /// river), a harbour's (0x58–0x6b) with its highest corner 1.
    /// // 10a8:0f67
    fn area_free(&self, building: u8, (i, j): Pos) -> bool {
        let (w, d) = (self.tables.hausnr(building, 3), self.tables.hausnr(building, 4));
        let island = &self.island;
        for a in i..=i.wrapping_add(w).wrapping_sub(1) {
            for b in j..=j.wrapping_add(d).wrapping_sub(1) {
                if !on_map((a, b))
                    || !FREE.contains(&island.buildings.at((a, b)))
                    || island.ground.at((a, b)) >= 10
                    // The original asks for the highest corner twice.
                    || ((0x58..=0x6b).contains(&building) && island.highest_corner((a, b)) != 1)
                {
                    return false;
                }
            }
        }
        true
    }

    /// Whether `p` has a road square (ground 10–58) within 3 squares of
    /// every column of the area from `i - 1` to `i + w`, or else of every
    /// row from `j - 1` to `j + d`. // 10a8:0cb6
    fn road_along(&self, w: i16, d: i16, (i, j): Pos, p: u8) -> bool {
        let island = &self.island;
        let road = |pos| on_map(pos) && (10..=58).contains(&island.ground.at(pos)) && island.owners.at(pos) == p;
        let along = |outer: std::ops::RangeInclusive<i16>, inner: &dyn Fn(i16) -> bool| {
            // Each line must have one; the first without ends the search.
            outer.into_iter().all(inner)
        };
        let span = |v: i16, before: i16, size: i16, after: i16| {
            v.wrapping_sub(before)..=v.wrapping_add(size).wrapping_add(after)
        };
        along(span(i, 1, w, 0), &|a| span(j, 3, d, 2).any(|b| road((a, b))))
            || along(span(j, 1, d, 0), &|b| span(i, 3, w, 2).any(|a| road((a, b))))
    }

    /// Whether `p` may build `building` at `pos`: houses only where the
    /// location is 180 or more, utilities and airports (kinds 11, 9) not
    /// with one of `p`'s houses within 2 squares. // 10a8:11f7
    fn site_allowed(&self, p: u8, building: u8, pos: Pos) -> bool {
        let mut allowed = true;
        if self.computers.category[slot(p)] == HOUSES && self.location_at(pos) < 180 {
            allowed = false;
        }
        let kind = self.tables.hausdata(building, 5);
        if (kind == 11 || kind == 9) && self.own_house_near(p, building, pos) {
            allowed = false;
        }
        allowed
    }

    /// Whether one of `p`'s houses (kinds 0, 1) is within 2 squares of
    /// `building`'s area at `(i, j)`. // 10a8:1098
    fn own_house_near(&self, p: u8, building: u8, (i, j): Pos) -> bool {
        let (w, d) = (self.tables.hausnr(building, 3), self.tables.hausnr(building, 4));
        let island = &self.island;
        (i.wrapping_sub(2)..=i.wrapping_add(w).wrapping_add(1)).any(|a| {
            (j.wrapping_sub(2)..=j.wrapping_add(d).wrapping_add(1)).any(|b| {
                let b0 = island.buildings.get((a, b)).unwrap_or(NO_BUILDING);
                on_map((a, b))
                    && !FREE.contains(&b0)
                    && island.owners.at((a, b)) == p
                    && matches!(self.tables.hausdata(b0, 5), 0 | 1)
            })
        })
    }

    /// `p` builds its building on its site if it can, whatever its money
    /// says; for the first roads of an area, the squares around a new
    /// transport building become their starts. The error from
    /// [`World::can_build`], 0 when it's built. // 10a8:163b
    fn build_on_site(&mut self, p: u8, tick: &mut Tick) -> u8 {
        let k = slot(p);
        let (pos, building) = (self.computers.site[k], self.computers.building[k]);
        let (mut error, cost) = self.can_build(pos, building, p); // 1030:04e3
        if error == crate::construction::NOT_ENOUGH_MONEY {
            error = 0;
        }
        if error == 0 {
            let category = self.computers.category[k];
            self.start_building(cost, category.cast_signed(), p, building, pos, tick); // 1078:1d21
            if self.computers.first_road[k] != 0 && category == TRANSPORT {
                self.collect_road_ends(p, pos, building);
            }
        }
        error
    }

    /// The squares within 2 of `building`'s area at `(i, j)` that are free
    /// (palms, beach life, none), flat enough for a road and above the
    /// sea become `p`'s road ends, in each of the four directions.
    /// // 10a8:14b2
    fn collect_road_ends(&mut self, p: u8, (i, j): Pos, building: u8) {
        let k = slot(p);
        self.computers.road_ends[k].clear();
        let (w, d) = (self.tables.hausnr(building, 3), self.tables.hausnr(building, 4));
        for a in i.wrapping_sub(2)..=i.wrapping_add(w).wrapping_add(1) {
            for b in j.wrapping_sub(2)..=j.wrapping_add(d).wrapping_add(1) {
                // The original reads off the map too, but those squares
                // have no lowest corner.
                let (Some(here), Some(form)) = (self.island.buildings.get((a, b)), self.island.forms.get((a, b)))
                else {
                    continue;
                };
                if FREE.contains(&here) && ROAD_FORMS.contains(&form) && self.island.lowest_corner((a, b)) != 0 {
                    for direction in 0..4 {
                        self.computers.road_ends[k].push(direction, (a, b)); // 1030:0caf
                    }
                }
            }
        }
    }

    /// Levels `p`'s site: first the height, the mean of the area's lowest
    /// and highest corner, if it's 1 to 9 and suits the building's ground
    /// (`HAUSDATA.DAT` column 7: sand up to 2, grass from 3, any land);
    /// then a corner at a time, the first one not at that height from a
    /// random start (`Random` of the width and depth plus 1, stepping by
    /// 17), or the owner of one that's someone else's. Whether it's done.
    /// // 10a8:173b
    fn level_site(&mut self, p: u8, tick: &mut Tick) -> bool {
        let (d, k) = (usize::from(p - 1), slot(p));
        let (i0, j0) = self.computers.site[k];
        let building = self.computers.building[k];
        let (w, depth) = (self.tables.hausnr(building, 3), self.tables.hausnr(building, 4));
        match self.computers.levelling[d] {
            0 => {
                let mut low = self.computers.height_limit.to_le_bytes()[0];
                let mut high = 0u8;
                for a in i0.wrapping_sub(1)..=i0.wrapping_add(w).wrapping_sub(1) {
                    for b in j0.wrapping_sub(1)..=j0.wrapping_add(depth).wrapping_sub(1) {
                        let h = self.island.heights.get((a, b)).unwrap_or(0);
                        low = low.min(h);
                        high = high.max(h);
                    }
                }
                let mean = u16::midpoint(u16::from(low), u16::from(high)).to_le_bytes()[0];
                let ground = self.tables.hausdata(building, 7);
                let suits = (ground == 0 && mean <= 2) || (ground == 1 && mean >= 3) || ground == 2;
                if (1..=9).contains(&mean) && suits {
                    self.computers.level[d] = mean;
                    self.computers.levelling[d] = 1;
                    false
                } else {
                    self.computers.levelling[d] = 0;
                    true
                }
            }
            1 => {
                let width = w.wrapping_add(1);
                let mut a = self.random.random(width.cast_unsigned()).cast_signed();
                let mut done = None;
                for _ in 1..=width {
                    let deep = depth.wrapping_add(1);
                    let mut b = self.random.random(deep.cast_unsigned()).cast_signed();
                    for _ in 1..=deep {
                        let pos = (a.wrapping_add(i0).wrapping_sub(1), b.wrapping_add(j0).wrapping_sub(1));
                        let owner = self.island.owners.get(pos).unwrap_or(0);
                        if owner != 0 && owner != p {
                            self.computers.blocked_by[d] = owner;
                            done = Some(1);
                        } else {
                            let level = self.computers.level[d].min(10);
                            if self.island.heights.get(pos).unwrap_or(0) != level {
                                done = Some(self.level_corner(p, level.into(), pos, tick).0); // 10b0:2c83
                            }
                        }
                        if done.is_some() {
                            break;
                        }
                        b = b.wrapping_add(17) % deep;
                    }
                    if done.is_some() {
                        break;
                    }
                    a = a.wrapping_add(17) % width;
                }
                if done == Some(0) {
                    false
                } else {
                    self.computers.levelling[d] = 0;
                    true
                }
            }
            // BUG(original): an unset result for another levelling state; the port says false.
            _ => false,
        }
    }

    /// `p` builds a road five squares on from its road start, in its
    /// direction; the end becomes its square, and the new road's ends
    /// (and the start's sides, for an area's first road) where roads may
    /// go on from. Whether it was built. // 10a8:1b0a
    fn build_road_on(&mut self, p: u8, tick: &mut Tick) -> bool {
        let k = slot(p);
        let start = self.computers.road[k];
        let dir = self.computers.road_direction[k];
        let Some(to) = step(dir, 5, start) else {
            return false;
        };
        // 1078:2396, with the start's ground as the module `roads`
        // explains.
        // BUG(original): the computers use the ground the human's last drag started on (1110:704e); the port passes sand.
        let Ok(end) = self.build_road(p, start, to, NO_DRAG_GROUND, tick) else {
            return false;
        };
        self.computers.site[k] = end;
        let flat = |w: &World, pos| w.island.lowest_corner(pos) == w.island.highest_corner(pos);
        let sides = |w: &mut World, pos| {
            if flat(w, pos) {
                w.computers.roads[k].push((dir + 1) % 4, pos);
                w.computers.roads[k].push((dir + 3) % 4, pos);
            }
        };
        if self.computers.first_road[k] == 0 {
            self.computers.roads[k].push(dir, end);
            // 1030:0fb2, on `Longint`s.
            let far = |a: i16, b: i16| (i32::from(a) - i32::from(b)).abs() > 1;
            if far(start.0, end.0) || far(start.1, end.1) {
                sides(self, end);
            }
        } else {
            self.computers.roads[k].push((dir + 2) % 4, start);
            self.computers.roads[k].push(dir, end);
            sides(self, end);
            sides(self, start);
        }
        true
    }

    /// The next of `p`'s road starts, `Random(count)` of the area's first
    /// ones or of its roads' ends, taken off the list. Whether there was
    /// one. // 10a8:1ded
    fn next_road_start(&mut self, p: u8) -> bool {
        let k = slot(p);
        let first = self.computers.first_road[k] != 0;
        let list = if first {
            &self.computers.road_ends[k]
        } else {
            &self.computers.roads[k]
        };
        let n = list.len();
        if n == 0 {
            return false;
        }
        let r = self.random.random(u16::from(n)).to_le_bytes()[0].wrapping_add(1);
        let c = &mut self.computers;
        let list = if first { &mut c.road_ends[k] } else { &mut c.roads[k] };
        let (dir, pos) = list.get(r);
        list.remove(r);
        c.road_direction[k] = dir;
        c.road[k] = pos;
        true
    }

    /// Up to 10 road starts until the square a step on from one is free
    /// for a road ([`World::road_square`] 0), or one that has to be
    /// levelled can be. In an area's first roads each start's square is
    /// levelled towards its direction first. // 10a8:1f44 (through 10a8:2060)
    fn find_road_start(&mut self, p: u8, tick: &mut Tick) -> bool {
        let k = slot(p);
        let mut tries: i32 = 1;
        while tries < 11 {
            if !self.next_road_start(p) {
                return false;
            }
            let (pos, dir) = (self.computers.road[k], self.computers.road_direction[k]);
            if self.computers.first_road[k] != 0 {
                self.level_road_square(p, dir, pos, tick); // 10b0:53fe
            }
            match step(dir, 1, pos).map(|next| (next, self.road_square(next))) {
                Some((_, 0)) => return true,
                Some((next, 5)) if self.level_road_square(p, dir, next, tick) => return true,
                _ => tries += 1,
            }
        }
        false
    }

    /// What a road can do with the square `pos`: 0 go on it, 1 it's water,
    /// 2 a river it can't cross, 3 a building's in the way, 4 a road's
    /// there, 5 too steep, 6 off the map. // 10a8:064b
    #[must_use]
    pub fn road_square(&self, pos: Pos) -> u8 {
        let island = &self.island;
        let (Some(building), Some(ground)) = (island.buildings.get(pos), island.ground.get(pos)) else {
            return 6;
        };
        if !FREE.contains(&building) {
            3
        } else if ground == 2 {
            1
        } else if (10..=58).contains(&ground) {
            4
        } else if (59..=94).contains(&ground) && !CROSSABLE.contains(&ground) {
            2
        } else if ROAD_FORMS.contains(&island.forms.at(pos)) {
            0
        } else {
            5
        }
    }

    /// `p`'s first area: a random walk from its square (100 steps of
    /// `Random(4)` for the way and `Random(20)` for how far, kept on the
    /// map) until a plot of 200 squares or more. // 10a8:207b
    fn start_area(&mut self, p: u8) -> bool {
        let k = slot(p);
        let mut pos = self.computers.site[k];
        let mut tries = 0;
        loop {
            let plot = self.island.plot_at(pos);
            if (plot != 0 && self.plot_area(plot) > 199) || tries > 99 {
                break;
            }
            tries += 1;
            pos = self.wander(pos, 20);
        }
        if tries < 100 {
            self.computers.site[k] = pos;
        }
        tries < 100
    }

    /// Another area for `p`: a random walk from its square (50 steps of
    /// `Random(4)` and `Random(15)`) until a different plot, as long as the
    /// plot it left (sic) has 200 squares or more. // 10a8:21de
    fn change_area(&mut self, p: u8) -> bool {
        let k = slot(p);
        let mut pos = self.computers.site[k];
        let first = self.island.plot_at(pos);
        // BUG(original): checks the area of the plot it leaves, not the one it moves to.
        let area = self.plot_area(first);
        let mut tries = 0;
        loop {
            let plot = self.island.plot_at(pos);
            // The new number is compared as an `Integer`, the old as a
            // `Word`.
            let other = i32::from(plot.cast_signed()) != i32::from(first);
            if (plot != 0 && other && area > 199) || tries > 49 {
                break;
            }
            tries += 1;
            pos = self.wander(pos, 15);
        }
        if tries < 50 {
            self.computers.site[k] = pos;
        }
        tries < 50
    }

    /// A step of the areas' random walks: `Random(4)` picks `i + 1`,
    /// `i - 1`, `j + 1` or `j - 1`, `Random(far)` how far, at most 250 up
    /// and at least 0 down (`Longint`s). // 10a8:207b, 10a8:21de
    fn wander(&mut self, (i, j): Pos, far: u16) -> Pos {
        let way = self.random.random(4);
        let r = i32::from(self.random.random(far));
        let up = |v: i16| (i32::from(v) + r).min(250).to_le_bytes();
        let down = |v: i16| (i32::from(v) - r).max(0).to_le_bytes();
        let word = |b: [u8; 4]| i16::from_le_bytes([b[0], b[1]]);
        match way {
            0 => (word(up(i)), j),
            1 => (word(down(i)), j),
            2 => (i, word(up(j))),
            _ => (i, word(down(j))),
        }
    }

    /// The plot's area (`+0x352`), 0 for a number without a record (the
    /// original reads through a nil pointer).
    fn plot_area(&self, plot: u16) -> u16 {
        // BUG(original): a plot number without a record is read through a nil pointer; the port says 0.
        self.plots
            .iter()
            .find(|q| q.id() >= 0 && q.id().cast_unsigned() == plot)
            .map_or(0, crate::world::Plot::area)
    }
}

/// `pos` moved `n` squares in `direction` (0 `j + n`, 1 `i + n`, 2 `j - n`,
/// 3 `i - n`), if that stays on the map. // 10a8:0582
fn step(direction: u8, n: i16, (i, j): Pos) -> Option<Pos> {
    match direction {
        0 if j <= 250 - n => Some((i, j + n)),
        1 if i <= 250 - n => Some((i + n, j)),
        2 if n <= j => Some((i, j - n)),
        3 if n <= i => Some((i - n, j)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use vcl::random::Random;

    use super::*;
    use crate::world::{Facility, House, Plot, Tables};

    /// A 2 × 2 airport (type 40, kind 9) and a 1 × 1 hotel (type 2, kind
    /// 1), with `COMPBAU.DAT`'s transport and houses rows, on plot 1 of
    /// flat sand at height 1 (`(10, 10)` to `(69, 69)`).
    fn world() -> World {
        let mut hausnr = vec![[0; 8]; 127];
        hausnr[40] = [1, 0, 0, 2, 2, 4, 0, 0];
        hausnr[2] = [1, 0, 0, 1, 1, 1, 0, 0];
        hausnr[usize::from(crate::construction::BUILDING_SITE)] = [2, 0, 0, 1, 1, 1, 0, 0];
        let mut hausdata = vec![[0; 10]; 127];
        for row in &mut hausdata {
            row[5] = 7;
            row[9] = 100;
        }
        hausdata[40] = [5, 0, 0, 300, 0, 9, 0, 2, 0, 100];
        hausdata[2] = [3, 0, 0, 20, 0, 1, 0, 2, 0, 100];
        // One of a kind: not on the lists.
        hausdata[3] = [3, 0, 0, 20, 0, 1, 1, 2, 0, 100];
        let mut hausprei = vec![[0; 4]; 127];
        hausprei[40] = [5000, 100, 0, 0];
        hausprei[2] = [800, 50, 0, 0];
        let mut w = World {
            tables: Arc::new(Tables {
                hausnr,
                hausdata,
                hausprei,
                ..Tables::default()
            }),
            random: Random::new(77),
            player_count: 2,
            max_location_effect: 30,
            ..World::default()
        };
        w.computers.compbau[0] = [255, 9, 10, 255, 255, 255, 255];
        w.computers.compbau[10] = [255, 0, 1, 255, 255, 255, 255];
        for i in 0..100 {
            for j in 0..100 {
                w.island.heights.set((i, j), 1);
                w.island.forms.set((i, j), 16);
                if (10..70).contains(&i) && (10..70).contains(&j) {
                    w.island.plot_map.set((i, j), 1);
                }
            }
        }
        let mut plot = Plot::new(1);
        plot.set_byte(0x352, 0x84);
        plot.set_byte(0x353, 0x0e);
        w.plots = vec![plot];
        w.build_type_lists();
        w
    }

    #[test]
    fn candidate_lists() {
        let mut c = Candidates::default();
        assert!(c.is_empty());
        assert_eq!(c.push(1, (3, 4)), 1);
        assert_eq!(c.push(2, (5, 6)), 2);
        assert_eq!(c.push(3, (7, 8)), 3);
        assert_eq!(c.push(0, (251, 8)), 0xff);
        assert_eq!(c.len(), 3);
        assert_eq!(c.get(2), (2, (5, 6)));
        assert_eq!(c.get(4), (0xff, (0xff, 0xff)));
        c.remove(2);
        assert_eq!(c.len(), 2);
        assert_eq!(c.get(2), (3, (7, 8)));
        c.clear();
        assert_eq!(c.len(), 0);
        for _ in 0..0xfe {
            c.push(0, (1, 1));
        }
        assert_eq!(c.push(0, (1, 1)), 0xff);
        assert_eq!(c.len(), 0xfe);
    }

    #[test]
    fn type_lists() {
        let mut w = world();
        let row = |w: &World, c: usize| w.computers.types[c * TYPE_ROW..c * TYPE_ROW + 4].to_vec();
        assert_eq!(row(&w, 0), [1, 40, 0, 0]);
        // Kinds 0 and 1, but not the one of a kind.
        assert_eq!(row(&w, 10), [1, 2, 0, 0]);
        // An event offers one: it goes on the list anyway.
        w.make_available(3, 0);
        assert_eq!(row(&w, 10), [2, 2, 3, 0]);
        w.make_available(2, 2);
        assert_eq!(w.computers.state[0], ORDERED);
        assert_eq!(w.computers.build_state[0], FIND_ROAD);
        assert_eq!(w.computers.building[0], 2);
    }

    #[test]
    fn starting_out() {
        let mut w = world();
        w.player_count = 4;
        let mut r = Random::new(5);
        w.random = r;
        w.init_computers();
        for k in 0..3 {
            let i = r.random(200).cast_signed() + 25;
            let j = r.random(200).cast_signed() + 25;
            let a = r.random(7).cast_signed();
            let b = r.random(2).cast_signed() + 7;
            let c = &w.computers;
            assert_eq!(c.site[k], (i, j));
            assert_eq!(c.road[k], (i, j));
            assert_eq!(
                [c.unknown[0][k], c.unknown[1][k], c.unknown[2][k], c.unknown[3][k]],
                [a, 1, b, a]
            );
            assert_eq!(
                (c.state[k], c.build_state[k], c.building[k]),
                (THINKING, START_AREA, NO_BUILDING)
            );
        }
        assert_eq!(w.random, r);
        // Player 5 isn't playing.
        assert_eq!(w.computers.site[3], (0, 0));
    }

    #[test]
    fn road_squares() {
        let mut w = world();
        assert_eq!(w.road_square((20, 20)), 0);
        assert_eq!(w.road_square((-1, 20)), 6);
        w.island.forms.set((20, 20), 5);
        assert_eq!(w.road_square((20, 20)), 5);
        w.island.ground.set((20, 20), 12);
        assert_eq!(w.road_square((20, 20)), 4);
        w.island.ground.set((20, 20), 63);
        assert_eq!(w.road_square((20, 20)), 2);
        w.island.ground.set((20, 20), 60);
        assert_eq!(w.road_square((20, 20)), 5);
        w.island.ground.set((20, 20), 2);
        assert_eq!(w.road_square((20, 20)), 1);
        w.island.buildings.set((20, 20), 40);
        assert_eq!(w.road_square((20, 20)), 3);
        assert_eq!(step(0, 5, (10, 246)), None);
        assert_eq!(step(0, 5, (10, 245)), Some((10, 250)));
        assert_eq!(step(1, 1, (3, 3)), Some((4, 3)));
        assert_eq!(step(2, 4, (3, 3)), None);
        assert_eq!(step(3, 3, (3, 3)), Some((0, 3)));
        assert_eq!(step(4, 1, (3, 3)), None);
    }

    #[test]
    fn loans() {
        let mut w = world();
        w.finances.credit_size = 1000;
        w.finances.max_debt = 2500;
        w.finances.money = [0, -2600, 0, 0, 0, 0, 0, 0];
        w.manage_loans(2);
        // 1000, 2000, then the bank says no.
        assert_eq!(w.finances.money[1], -600);
        assert_eq!(w.finances.loan[1], 2000);
        assert!(w.finances.credit_refused[1]);
        w.finances.money[1] = 2500;
        w.manage_loans(2);
        // Pays back while it has more than 1000.
        assert_eq!(w.finances.money[1], 500);
        assert_eq!(w.finances.loan[1], 0);
        assert!(!w.finances.credit_refused[1]);
        // A computer pays a sixth less at difficulty 1.
        w.difficulty = 1;
        w.finances.money[1] = 1200;
        w.finances.loan[1] = 1000;
        w.manage_loans(2);
        assert_eq!(w.finances.money[1], 1200 - 1000 + 166);
    }

    #[test]
    fn the_worst_building() {
        let mut w = world();
        assert_eq!(w.worst_building(2), None);
        for (n, guests) in [(0, 15), (1, 5), (2, 2)] {
            let pos = (20 + n * 3, 20);
            let mut h = House::new(pos);
            h.set_building(2);
            h.set_guests(guests);
            h.set_price(5);
            w.houses.push(h);
            w.island.owners.set(pos, 2);
        }
        // 2 guests at 5 cover less than the costs of 50: 10 % full.
        w.island.owners.set((26, 20), 3);
        assert_eq!(w.worst_building(2), Some((23, 20)));
        let mut f = Facility::new(40, (40, 40));
        f.set_fill(10);
        f.set_price(100);
        w.facilities.push(f);
        w.island.owners.set((40, 40), 2);
        assert_eq!(w.worst_building(2), Some((40, 40)));
    }

    /// Runs `p`'s turns until a building site appears, at most `turns`.
    fn turns_until_site(w: &mut World, p: u8, turns: usize) -> Option<usize> {
        let mut tick = Tick::default();
        (1..=turns).find(|_| {
            w.computer_turn(p, &mut tick);
            !w.building_sites.is_empty()
        })
    }

    #[test]
    fn builds_an_airport_in_a_new_area() {
        let mut w = world();
        w.init_computers();
        w.computers.site[0] = (40, 40);
        let turns = turns_until_site(&mut w, 2, 30).expect("no building site");
        let site = w.building_sites[0];
        assert_eq!(site.building, 40);
        assert_eq!(site.budget, TRANSPORT.cast_signed());
        assert_eq!(w.island.owners.at(site.pos), 2);
        assert_eq!(w.computers.site[0], site.pos);
        // Paid for, and counted on the transport budget.
        assert_eq!(w.finances.money[1], -5000);
        assert_eq!(w.computer_budgets[0], 300);
        // The squares around it are where its roads start.
        assert_eq!(w.computers.road_ends[0].len(), 4 * (6 * 6 - 4));
        let c = &w.computers;
        assert_eq!(
            (c.state[0], c.build_state[0], c.new_area[0], c.first_road[0]),
            (THINKING, ROAD_START, 0, 1)
        );
        assert!(turns >= 6, "{turns}");
        // Then it lays roads from them, paying for each square.
        Arc::make_mut(&mut w.tables).road_cost = 7;
        let money = w.finances.money[1];
        let mut tick = Tick::default();
        let road = |w: &World| -> Vec<Pos> {
            (0..=250)
                .flat_map(|i| (0..=250).map(move |j| (i, j)))
                .filter(|&pos| crate::roads::ROADS.contains(&w.island.ground.at(pos)) && w.island.owners.at(pos) == 2)
                .collect()
        };
        for _ in 0..200 {
            w.computer_turn(2, &mut tick);
            if !road(&w).is_empty() {
                break;
            }
        }
        let squares = road(&w).len();
        assert!(squares >= 2, "{squares}");
        assert_eq!(w.finances.money[1], money - 7 * i32::try_from(squares).unwrap());
        assert_eq!(w.computers.first_road[0], 0);
        assert_eq!(w.computers.build_state[0], CHOOSE_CATEGORY);
        // The road's end is where it goes on from.
        assert!(!w.computers.roads[0].is_empty());
    }

    #[test]
    fn houses_need_a_road_and_a_good_location() {
        let mut w = world();
        w.init_computers();
        let k = 0;
        w.computers.state[k] = BUILDING;
        w.computers.build_state[k] = FIND_SITE;
        w.computers.category[k] = HOUSES;
        w.computers.building[k] = 2;
        w.computers.centre[k] = (30, 30);
        let mut tick = Tick::default();
        w.computer_turn(2, &mut tick);
        assert_eq!(w.computers.build_state[k], FIND_ROAD, "no road of its own");
        // An own road along the whole of the area's rows.
        for i in 25..36 {
            w.island.ground.set((i, 33), 10);
            w.island.owners.set((i, 33), 2);
        }
        w.computers.build_state[k] = FIND_SITE;
        w.computer_turn(2, &mut tick);
        assert_eq!(w.computers.build_state[k], FIND_ROAD, "the location is 100");
        w.island.location = crate::world::LocationMap::filled(200);
        w.computers.build_state[k] = FIND_SITE;
        w.computer_turn(2, &mut tick);
        assert_eq!(w.computers.build_state[k], BUILD);
        w.computer_turn(2, &mut tick);
        let site = w.building_sites[0];
        assert_eq!(site.building, 2);
        assert!((30..=33).contains(&site.pos.1), "{site:?}");
        assert_eq!(w.computers.build_state[k], CHOOSE_CATEGORY);
        assert_eq!(w.computer_budgets[10], 20);
    }

    #[test]
    fn levelling_a_site() {
        let mut w = world();
        w.init_computers();
        let (d, k) = (1, 0);
        w.computers.site[k] = (30, 30);
        w.computers.building[k] = 40;
        // Heights 1 and 4 around: the mean is 2, any land suits.
        w.island.heights.set((31, 31), 4);
        assert!(!w.level_site(2, &mut Tick::default()));
        assert_eq!((w.computers.levelling[d], w.computers.level[d]), (1, 2));
        // A corner at a time, until all nine are at 2.
        let mut turns = 0;
        while !w.level_site(2, &mut Tick::default()) {
            turns += 1;
            assert!(turns < 20);
        }
        assert!(turns >= 8, "{turns}");
        for i in 29..=31 {
            for j in 29..=31 {
                assert_eq!(w.island.heights.at((i, j)), 2, "{i} {j}");
            }
        }
        assert_eq!(w.computers.levelling[d], 0);
        // Someone else's square stops it.
        w.computers.levelling[d] = 1;
        for i in 29..=31 {
            for j in 29..=31 {
                w.island.owners.set((i, j), 5);
            }
        }
        assert!(w.level_site(2, &mut Tick::default()));
        assert_eq!(w.computers.blocked_by[d], 5);
        // At sea level there's nothing to level to.
        w.island.heights = crate::world::Layer::filled(0);
        assert!(w.level_site(2, &mut Tick::default()));
        assert_eq!(w.computers.levelling[d], 0);
    }

    #[test]
    fn thinking_and_waiting() {
        let mut w = world();
        w.init_computers();
        let mut tick = Tick::default();
        let mut r = w.random;
        w.computer_turn(2, &mut tick);
        let expected = if r.random(1000) == 0 { DEMOLISHING } else { BUILDING };
        assert_eq!(w.computers.state[0], expected);
        assert_eq!(w.computers.status, STATUS_THINKING);
        // In the red with the bank saying no, it mostly waits.
        w.computers.state[0] = THINKING;
        w.finances.money[1] = -10;
        w.finances.credit_refused[1] = true;
        let mut r = w.random;
        w.computer_turn(2, &mut tick);
        let expected = if r.random(100) == 0 { DEMOLISHING } else { WAITING };
        assert_eq!(w.computers.state[0], expected);
        w.computers.state[0] = WAITING;
        for _ in 0..10 {
            w.computer_turn(2, &mut tick);
        }
        assert_eq!(w.computers.state[0], WAITING);
        w.computer_turn(2, &mut tick);
        assert_eq!(w.computers.state[0], THINKING);
        assert_eq!(w.computers.waited[1], 0);
        // Out of the game, nothing.
        w.set_player_out(2, 1);
        w.computer_turn(2, &mut tick);
        assert_eq!(w.computers.state[0], THINKING);
    }

    #[test]
    fn demolishing() {
        let mut w = world();
        w.init_computers();
        w.events = vec![crate::events::Event::default(); 41];
        let pos = (20, 20);
        let mut h = House::new(pos);
        h.set_building(2);
        w.houses.push(h);
        w.island.buildings.set(pos, 2);
        w.island.owners.set(pos, 2);
        w.computers.state[0] = DEMOLISHING;
        let mut tick = Tick::default();
        w.computer_turn(2, &mut tick);
        assert_eq!(w.computers.state[0], DEMOLISHING, "every other turn");
        let mut r = w.random;
        w.computer_turn(2, &mut tick);
        assert_eq!(w.computers.state[0], THINKING);
        if r.random(100) < 20 {
            // Sold: event 9 with a quarter of the price.
            assert_eq!(w.events[9].chance, 0x3fffc);
            assert_eq!((w.events[9].value, w.events[9].arg, w.events[9].pos), (200, 2, pos));
        } else {
            assert_eq!(w.houses.len(), 0);
            assert_eq!(w.island.buildings.at(pos), NO_BUILDING);
        }
        // Nothing to get rid of: it waits.
        w.houses.clear();
        w.computers.state[0] = DEMOLISHING;
        w.computer_turn(2, &mut tick);
        w.computer_turn(2, &mut tick);
        assert_eq!(w.computers.state[0], WAITING);
    }
}
