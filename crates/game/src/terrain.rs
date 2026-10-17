//! Raising and lowering the land (segment `10b0`), and the plots that
//! grow, merge and vanish as corners rise from the sea or sink into it
//! (1078:09f6, 1078:0d00, segment `1068`).
//!
//! A corner moves one step at a time. Its neighbours follow wherever the
//! land would get steeper than one step between corners, so raising a
//! corner raises a cone around it and lowering one digs a funnel. The
//! original works on a 41 × 41 copy of the heights and buildings around
//! the corner (`*1110:721e`, `*1110:7222`), checks every square it
//! touches (buildings, roads, rivers), and writes the copy back only when
//! nothing stood in the way (and player 1 can pay). Then the squares'
//! forms and grounds are worked out again (10b0:33bc).
//!
//! The computer players level their sites and road squares this way
//! without paying; the human's tools raise and lower one corner a step
//! at a time and pay for it (`crate::command`, 1028:3b7c, 1028:3a68).

// The original's names: squares `(i, j)`, offsets `di`, `dj`.
#![allow(clippy::many_single_char_names)]

use crate::construction::NOT_ENOUGH_MONEY;
use crate::enhance::Fix;
use crate::simulation::Tick;
use crate::world::{NO_BUILDING, PALM, Plot, Pos, World, on_map};

/// What raising and lowering the land costs (`MODATYP.DAT`, saved with
/// the map). The original adds them up for each corner a step moves, and
/// only player 1's tool checks the money; the computers don't pay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct LandCosts {
    /// `LandausWasserhebekosten`: a corner rising from the sea. // 1110:64da
    pub from_water: i32,
    /// `Landhebekosten`: any other corner raised. // 1110:64de
    pub raise: i32,
    /// `LandinWassersenkkosten`: a corner sinking into the sea. // 1110:64e2
    pub into_water: i32,
    /// `Landsenkkosten`: any other corner lowered. // 1110:64e6
    pub lower: i32,
    /// `Palmenabrisskosten`: a palm in the way. // 1110:64f2
    pub palm: i32,
}

/// The person who shows where the land couldn't be moved: kind 0x2a, a
/// picture by the square's form. // 10b0:069f, 10b0:1907
pub const MARKER: u8 = 0x2a;

/// The errors (`FEHLER.TXT`) raising or lowering a corner gives: off the
/// map (0xd for raising, 3 for lowering; 0x16 for [`World::level_corner`]),
/// a building in the way (1), a road (2), a river (0x10), something on
/// `layer_551c` (0x1c), at sea level already (3), as high as it goes (4),
/// too close to the map's edge for its height (5–8).
pub const OFF_THE_MAP: u16 = 0x16;

/// How far the working copy reaches from the corner, each way.
const REACH: i16 = 20;
/// The working copy's side. // 0x29
const SIDE: usize = 41;

/// The forms (0–18) by how much higher than the corner `(i, j)` the
/// corners `(i, j - 1)`, `(i - 1, j - 1)` and `(i - 1, j)` are; a square
/// that fits none gets form 19. The original's rows are four `Integer`s,
/// the first (for `(i, j)` itself) always 0. // 1110:1224
pub(crate) const FORMS: [[i16; 3]; 19] = [
    [0, 1, 1],
    [-1, -1, 0],
    [0, -1, -1],
    [1, 1, 0],
    [0, 1, 0],
    [0, 0, 1],
    [-1, -1, -1],
    [1, 0, 0],
    [1, 2, 1],
    [-1, 0, 1],
    [-1, -2, -1],
    [1, 0, -1],
    [1, 1, 1],
    [-1, 0, 0],
    [0, -1, 0],
    [0, 0, -1],
    [0, 0, 0],
    [-1, 0, -1],
    [1, 0, 1],
];

/// The four squares around a corner, as offsets: the corner `(i, j)` is
/// one of the squares `(i, j)`, `(i + 1, j)`, `(i, j + 1)` and `(i + 1,
/// j + 1)`, in the order the original checks them.
const SQUARES: [Pos; 4] = [(0, 0), (1, 0), (0, 1), (1, 1)];

/// The neighbouring corners a step spreads to, in the original's order.
const NEIGHBOURS: [Pos; 4] = [(1, 0), (0, 1), (-1, 0), (0, -1)];

/// The working copy of the land around a corner, and what moving it has
/// found so far: the locals of 10b0:069f and 10b0:1907, which their nested
/// routines share.
struct Work {
    /// The corner being moved.
    centre: Pos,
    /// The heights and buildings within [`REACH`]; off the map 0 and none.
    /// // *1110:721e, *1110:7222
    heights: Vec<u8>,
    buildings: Vec<u8>,
    /// The error, 0 while there's none, and where it was found.
    error: u16,
    error_at: Pos,
    /// What it costs so far.
    cost: i32,
    /// The corners moved, the lowest and highest `i` and `j` (bytes, as
    /// the original keeps them).
    low: (u8, u8),
    high: (u8, u8),
    /// The corners that rose from the sea (bytes; the original keeps up to
    /// 80 of them).
    risen: Vec<(u8, u8)>,
}

impl Work {
    /// The copy around `centre`. // 10b0:069f, 10b0:1907
    fn new(w: &World, centre: Pos) -> Work {
        let mut heights = vec![0; SIDE * SIDE];
        let mut buildings = vec![NO_BUILDING; SIDE * SIDE];
        let (i, j) = centre;
        let byte = |v: i16| v.to_le_bytes()[0];
        for di in -REACH..=REACH {
            for dj in -REACH..=REACH {
                let pos = (i + di, j + dj);
                if let Some(n) = Work::index((di, dj))
                    && on_map(pos)
                {
                    heights[n] = w.island.heights.at(pos);
                    buildings[n] = w.island.buildings.at(pos);
                }
            }
        }
        Work {
            centre,
            heights,
            buildings,
            error: 0,
            error_at: centre,
            cost: 0,
            low: (byte(i), byte(j)),
            high: (byte(i), byte(j)),
            risen: Vec::new(),
        }
    }

    /// The copy's index for the offset `(di, dj)`, if it's within reach.
    /// The original reads and writes past the copy where the land would
    /// spread further; the port stops there.
    // BUG(original): the copy is read and written past its edge where the land would spread further; the port stops at the edge.
    fn index((di, dj): Pos) -> Option<usize> {
        let a = usize::try_from(di + REACH).ok()?;
        let b = usize::try_from(dj + REACH).ok()?;
        (a < SIDE && b < SIDE).then_some(a * SIDE + b)
    }

    fn height(&self, d: Pos) -> Option<i16> {
        Work::index(d).map(|n| i16::from(self.heights[n]))
    }

    fn building(&self, d: Pos) -> u8 {
        Work::index(d).map_or(NO_BUILDING, |n| self.buildings[n])
    }

    /// The square or corner at the offset `d`.
    fn at(&self, (di, dj): Pos) -> Pos {
        (self.centre.0 + di, self.centre.1 + dj)
    }

    /// The corner at `d` moved: the bounds take it in (`Longint` min and
    /// max, kept as bytes).
    fn spread(&mut self, d: Pos) {
        let (a, b) = self.at(d);
        let byte = |v: i32| v.to_le_bytes()[0];
        self.low = (
            byte(i32::from(a).min(i32::from(self.low.0))),
            byte(i32::from(b).min(i32::from(self.low.1))),
        );
        self.high = (
            byte(i32::from(a).max(i32::from(self.high.0))),
            byte(i32::from(b).max(i32::from(self.high.1))),
        );
    }

    /// A palm in the copy goes, for `Palmenabrisskosten`: whether the
    /// square at `d` had one.
    // BUG(original): a palm cleared this way only loses its building: part, owner, plot count and location effect stay.
    fn clear_palm(&mut self, d: Pos, costs: &LandCosts) -> bool {
        match Work::index(d) {
            Some(n) if self.buildings[n] == PALM => {
                self.buildings[n] = NO_BUILDING;
                self.cost = self.cost.wrapping_add(costs.palm);
                true
            }
            _ => false,
        }
    }
}

impl World {
    /// Raises or lowers the corner `(i, j)` one step towards `height` for
    /// `player` (nothing if it's there): the error (0 when done) and what
    /// it cost, which the caller pays if it does. 0x16 for a corner not
    /// within 1..=249. // 10b0:2c83
    pub(crate) fn level_corner(&mut self, player: u8, height: i16, pos: Pos, tick: &mut Tick) -> (u16, i32) {
        if !(1..=249).contains(&pos.0) || !(1..=249).contains(&pos.1) {
            return (OFF_THE_MAP, 0);
        }
        match i16::from(self.island.heights.at(pos)).cmp(&height) {
            std::cmp::Ordering::Less => self.raise_corner(player, pos, tick).0, // 10b0:069f
            std::cmp::Ordering::Greater => self.lower_corner(player, pos, tick).0, // 10b0:1907
            std::cmp::Ordering::Equal => (0, 0),
        }
    }

    /// Raises or lowers the corner `(i, j)` step by step until it's at
    /// `height` or a step fails: the last step's error (0 when done) and
    /// the whole cost. The original returns an unset error for a corner
    /// not within 1..=249, the port 0. // 10b0:2b9b
    pub(crate) fn set_corner_height(&mut self, player: u8, height: i16, pos: Pos, tick: &mut Tick) -> (u16, i32) {
        let mut cost = 0i32;
        if !(1..=249).contains(&pos.0) || !(1..=249).contains(&pos.1) {
            // BUG(original): an unset error for a corner off 1..=249; the port returns 0.
            return (0, cost);
        }
        let mut error = 0;
        while i16::from(self.island.heights.at(pos)) < height && error == 0 {
            let ((e, c), _) = self.raise_corner(player, pos, tick); // 10b0:069f
            error = e;
            cost = cost.wrapping_add(c);
        }
        while height < i16::from(self.island.heights.at(pos)) && error == 0 {
            let ((e, c), _) = self.lower_corner(player, pos, tick); // 10b0:1907
            error = e;
            cost = cost.wrapping_add(c);
        }
        (error, cost)
    }

    /// Makes the square `(i, j)` fit a road going on in `direction` (0
    /// `j + 1`, 1 `i + 1`, 2 `j - 1`, 3 `i - 1`) for `player`: its two
    /// corners on that side go to the height of a corner on the other
    /// side; if the square isn't level then, both go a step below that
    /// height (if its lowest corner is lower) or above it. Whether it
    /// worked. The cost is dropped: the computers don't pay. // 10b0:53fe
    pub(crate) fn level_road_square(&mut self, player: u8, direction: u8, (i, j): Pos, tick: &mut Tick) -> bool {
        if !(1..=249).contains(&i) || !(1..=249).contains(&j) {
            return false;
        }
        let heights = &self.island.heights;
        // The reference height and the two corners to move. The original
        // leaves them unset for any other direction; nothing passes one.
        let (reference, a, b) = match direction {
            0 => (heights.at((i, j - 1)), (i - 1, j), (i, j)),
            1 => (heights.at((i - 1, j)), (i, j), (i, j - 1)),
            2 => (heights.at((i, j)), (i - 1, j - 1), (i, j - 1)),
            3 => (heights.at((i, j)), (i - 1, j), (i - 1, j - 1)),
            // BUG(original): the reference height and corners are unset for a direction past 3.
            _ => return false,
        };
        let reference = i16::from(reference);
        let mut set_both = |w: &mut World, height: i16| {
            let _ = w.set_corner_height(player, height, a, tick); // 10b0:2b9b
            let _ = w.set_corner_height(player, height, b, tick);
            let h = |pos| i16::from(w.island.heights.at(pos));
            h(a) == height && h(b) == height
        };
        set_both(self, reference);
        if self.island.highest_corner((i, j)) == self.island.lowest_corner((i, j)) {
            true
        } else if i16::from(self.island.lowest_corner((i, j))) < reference {
            set_both(self, reference - 1)
        } else {
            set_both(self, reference + 1)
        }
    }

    /// Raises the corner `(i, j)` a step for `player`: the error (0 when
    /// done) and the cost so far, and the square that caused the error
    /// (`1110:64ba`, for the message). Single human: only player 1 needs
    /// the money. // 10b0:069f
    pub(crate) fn raise_corner(&mut self, player: u8, pos: Pos, tick: &mut Tick) -> ((u16, i32), Pos) {
        let (i, j) = pos;
        let mut result = (0xd, 0, pos);
        if on_map(pos) {
            let h = i16::from(self.island.heights.at(pos));
            result.0 = if h >= 20 {
                4
            } else if h > 248 - i {
                5
            } else if h > j - 2 {
                6
            } else if h > i - 2 {
                7
            } else if h > 248 - j {
                8
            } else {
                let mut work = Work::new(self, pos);
                self.raise_step(&mut work, (0, 0)); // 10b0:03fa
                result = (work.error, work.cost, work.error_at);
                // BUG(original): for player 1 the money check replaces any earlier error.
                let first = result.0 != 0 && self.fixes.has(Fix::LevellingErrors);
                if player == 1 && self.money_of(1) < work.cost && !first {
                    result.0 = NOT_ENOUGH_MONEY.into();
                }
                if result.0 == 0 {
                    self.write_back(&work, tick);
                }
                result.0
            };
        }
        self.show_marker(result.0, result.2, tick);
        ((result.0, result.1), result.2)
    }

    /// Lowers the corner `(i, j)` a step for `player`: the error (0 when
    /// done), the cost so far and the square that caused the error, as
    /// [`World::raise_corner`]. // 10b0:1907
    pub(crate) fn lower_corner(&mut self, player: u8, pos: Pos, tick: &mut Tick) -> ((u16, i32), Pos) {
        let mut result = (0, 0, pos);
        if !on_map(pos) || self.island.heights.at(pos) == 0 {
            result.0 = 3;
        } else {
            let mut work = Work::new(self, pos);
            self.lower_step(&mut work, (0, 0)); // 10b0:12c7
            result = (work.error, work.cost, work.error_at);
            // BUG(original): for player 1 the money check replaces any earlier error.
            let first = result.0 != 0 && self.fixes.has(Fix::LevellingErrors);
            if player == 1 && self.money_of(1) < work.cost && !first {
                result.0 = NOT_ENOUGH_MONEY.into();
            }
            if result.0 == 0 {
                self.write_back(&work, tick);
                if self.island.heights.at(pos) == 0 {
                    self.corner_sunk(pos); // 1078:0d00
                }
            }
        }
        self.show_marker(result.0, result.2, tick);
        ((result.0, result.1), result.2)
    }

    /// After an error, the marker at the square that caused it.
    /// // 10b0:069f, 10b0:1907
    fn show_marker(&mut self, error: u16, at: Pos, tick: &mut Tick) {
        if error != 0 {
            self.add_person(MARKER, at, tick); // 1088:0324
            // The original also keeps the square at `1110:64ba` for the
            // error message, the view's.
        }
    }

    /// Raises the copy's corner at `d` a step, if nothing on its four
    /// squares stands in the way, and the neighbours that end up two
    /// steps lower with it. // 10b0:03fa
    fn raise_step(&self, work: &mut Work, (di, dj): Pos) {
        for (si, sj) in SQUARES {
            if work.error == 0 {
                work.error = self.check_raise(work, (di + si, dj + sj)); // 10b0:0291
            }
        }
        if work.error != 0 {
            return;
        }
        let Some(n) = Work::index((di, dj)) else {
            return;
        };
        work.heights[n] = work.heights[n].wrapping_add(1);
        work.spread((di, dj));
        if work.heights[n] == 1 {
            work.cost = work.cost.wrapping_add(self.land_costs.from_water);
            let (a, b) = work.at((di, dj));
            work.risen.push((a.to_le_bytes()[0], b.to_le_bytes()[0])); // 10b0:025b
        } else {
            work.cost = work.cost.wrapping_add(self.land_costs.raise);
        }
        // BUG(original): only a neighbour exactly two steps off follows, so cliffs get steeper.
        let cliffs = self.fixes.has(Fix::LevellingCliffs);
        for (ni, nj) in NEIGHBOURS {
            let next = (di + ni, dj + nj);
            if let (Some(h), Some(g)) = (work.height((di, dj)), work.height(next))
                && (h - g == 2 || (cliffs && h - g > 2))
            {
                self.raise_step(work, next);
            }
        }
    }

    /// Whether the square at the copy's offset `d` lets the land rise: 0,
    /// or 1 for a building (a palm goes, for a price), 2 a road, 0x10 a
    /// river or bridge, 0x1c something on `layer_551c`. // 10b0:0291
    fn check_raise(&self, work: &mut Work, d: Pos) -> u16 {
        let mut error = 0;
        if work.building(d) != NO_BUILDING && !work.clear_palm(d, &self.land_costs) {
            error = 1;
        }
        let pos = work.at(d);
        // The original reads past the map for squares off it.
        let ground = self.island.ground.get(pos).unwrap_or(0);
        if error == 0 && (10..0x37).contains(&ground) {
            error = 2;
        }
        if error == 0 && ground > 0x36 {
            error = 0x10;
        }
        if error == 0 && self.island.layer_551c.get(pos).unwrap_or(0xff) != 0xff {
            error = 0x1c;
        }
        if error != 0 {
            work.error_at = pos;
        }
        error
    }

    /// Lowers the copy's corner at `d` a step, unless a building (but a
    /// palm, which goes) or a road or river on its four squares stands in
    /// the way, and the neighbours that end up two steps higher with it.
    /// Unlike raising, the buildings are checked (and palms cleared) even
    /// after an error. // 10b0:12c7
    fn lower_step(&self, work: &mut Work, (di, dj): Pos) {
        let first = self.fixes.has(Fix::LevellingErrors);
        for (si, sj) in SQUARES {
            let s = (di + si, dj + sj);
            // BUG(original): the buildings are checked (palms cleared, their cost added) even after an error.
            if first && work.error != 0 {
                break;
            }
            if work.building(s) != NO_BUILDING && !work.clear_palm(s, &self.land_costs) {
                work.error = 1;
                work.error_at = work.at(s);
            }
        }
        if work.error == 0
            && let Some((si, sj)) = SQUARES
                .into_iter()
                .find(|&(si, sj)| self.island.ground.get(work.at((di + si, dj + sj))).unwrap_or(0) >= 10)
        {
            let pos = work.at((di + si, dj + sj));
            let ground = self.island.ground.get(pos).unwrap_or(0);
            work.error = if ground < 0x37 { 2 } else { 0x10 };
            work.error_at = pos;
        }
        if work.error != 0 {
            return;
        }
        let Some(n) = Work::index((di, dj)) else {
            return;
        };
        work.heights[n] = work.heights[n].wrapping_sub(1);
        work.spread((di, dj));
        let cost = if work.heights[n] == 0 {
            self.land_costs.into_water
        } else {
            self.land_costs.lower
        };
        work.cost = work.cost.wrapping_add(cost);
        // BUG(original): only a neighbour exactly two steps off follows, so cliffs get steeper.
        let cliffs = self.fixes.has(Fix::LevellingCliffs);
        for (ni, nj) in NEIGHBOURS {
            let next = (di + ni, dj + nj);
            if let (Some(h), Some(g)) = (work.height((di, dj)), work.height(next))
                && (g - h == 2 || (cliffs && g - h > 2))
            {
                self.lower_step(work, next);
            }
        }
    }

    /// Puts the copy's heights and buildings back on the map, gives the
    /// corners that rose from the sea a plot, and works out the forms and
    /// grounds of the squares the moved corners touch.
    /// // 10b0:069f, 10b0:1907
    fn write_back(&mut self, work: &Work, tick: &mut Tick) {
        let (i, j) = work.centre;
        for a in (i - REACH).max(0)..=(i + REACH).min(250) {
            for b in (j - REACH).max(0)..=(j + REACH).min(250) {
                let Some(n) = Work::index((a - i, b - j)) else {
                    continue;
                };
                self.island.heights.set((a, b), work.heights[n]);
                // BUG(original): a palm cleared this way only loses its building: part, owner, plot count and location effect stay.
                if work.buildings[n] == NO_BUILDING
                    && self.island.buildings.get((a, b)) == Some(PALM)
                    && self.fixes.has(Fix::PalmRemoval)
                {
                    self.clear_palm_square((a, b));
                }
                self.island.buildings.set((a, b), work.buildings[n]);
                let corner = (a.to_le_bytes()[0], b.to_le_bytes()[0]);
                if work.heights[n] == 1 && work.risen.contains(&corner) {
                    // 10b0:01f0
                    self.corner_risen((a, b)); // 1078:09f6
                }
            }
        }
        self.reshape(work.low, work.high); // 10b0:33bc
        // The original redraws the area on the main screen (1050:264d) and
        // the overview map (10b8:1504).
        for a in u16::from(work.low.0)..=u16::from(work.high.0) + 1 {
            for b in u16::from(work.low.1)..=u16::from(work.high.1) + 1 {
                let pos = (a.cast_signed(), b.cast_signed());
                if on_map(pos) {
                    tick.redraw.push(pos);
                }
            }
        }
    }

    /// The palm on `pos`, cleared by moving the land, goes as a palm
    /// that dies does, with its owner (`fix.palm_removal`): off its
    /// plot's count and the location map, its part back to the ground's
    /// look.
    fn clear_palm_square(&mut self, pos: Pos) {
        self.uncount_palm(pos);
        self.remove_location_effect(pos, PALM); // 1030:1dc6
        let stages = self.tables.hausnr(PALM, 6);
        let part = self.island.parts.at(pos);
        let variant = i16::from(part).checked_div(stages).unwrap_or(0);
        self.island.parts.set(pos, variant.to_le_bytes()[0]);
        self.island.owners.set(pos, 0);
    }

    /// The squares from `low` to one past `high` get the form their
    /// corners make, and those with plain ground (below 10) their ground
    /// back ([`World::reset_ground`]); `j` outside, `i` inside. Off the map
    /// the original reads and writes past the layers; the port reads the
    /// layer's neighbouring bytes and writes nothing. // 10b0:33bc
    pub(crate) fn reshape(&mut self, low: (u8, u8), high: (u8, u8)) {
        for b in u16::from(low.1)..=u16::from(high.1) + 1 {
            for a in u16::from(low.0)..=u16::from(high.0) + 1 {
                let pos = (a.cast_signed(), b.cast_signed());
                let (i, j) = pos;
                let heights = &self.island.heights;
                // BUG(original): reads before row 0 or column 0 and writes forms off the map; the port reads the neighbouring bytes and writes nothing off it.
                let h = |pos| i16::from(heights.flat(pos));
                let here = h(pos);
                let corners = [h((i, j - 1)), h((i - 1, j - 1)), h((i - 1, j))];
                let form = FORMS
                    .iter()
                    .position(|f| (0..3).all(|k| here + f[k] == corners[k]))
                    .unwrap_or(FORMS.len());
                if !on_map(pos) {
                    continue;
                }
                self.island.forms.set(pos, u8::try_from(form).unwrap_or(19));
                if self.island.ground.at(pos) < 10 {
                    self.reset_ground(pos); // 10b0:3199
                }
            }
        }
    }

    /// The corner `(i, j)` rose from the sea: the square gets a plot. With
    /// no land around it a new one; with land of one plot around it that
    /// one, which grows; with several, they merge into one (1068:27a5).
    /// The neighbours' plot numbers are bytes for the first merge, as in
    /// the original (whole with `fix.plot_merge_numbers`). // 1078:09f6
    fn corner_risen(&mut self, (i, j): Pos) {
        let heights = &self.island.heights;
        let above = if i == 0 { 0 } else { heights.at((i - 1, j)) };
        let left = if j == 0 { 0 } else { heights.at((i, j - 1)) };
        let below = if i < 250 { heights.at((i + 1, j)) } else { 0 };
        let right = if j < 250 { heights.at((i, j + 1)) } else { 0 };
        if [above, left, below, right].iter().all(|&h| h == 0) {
            let plot = self.new_plot(); // 1068:2698
            self.island.plot_map.set((i, j), plot);
            return;
        }
        let island = &self.island;
        // BUG(original): the neighbours' plot numbers are cut to a byte for the first merge.
        let whole = self.fixes.has(Fix::PlotMergeNumbers);
        let plot = |h: u8, pos| {
            if h == 0 {
                0
            } else if whole {
                island.plot_at(pos)
            } else {
                u16::from(island.plot_at(pos).to_le_bytes()[0])
            }
        };
        let above = plot(above, (i - 1, j));
        let right = plot(right, (i, j + 1));
        let left = plot(left, (i, j - 1));
        let below = plot(below, (i + 1, j));
        let most = below.max(above).max(right).max(left);
        let one = [above, below, right, left].iter().all(|&p| p == 0 || p == most);
        if one {
            if let Some(p) = self.find_plot(most) {
                p.set_area(p.area().wrapping_add(1));
            }
            self.island.plot_map.set((i, j), most);
        } else {
            let plot = self.merge_plots(below, above); // 1068:27a5
            self.island.plot_map.set((i, j), plot);
            for side in [(i, j - 1), (i, j + 1)] {
                let here = self.island.plot_at((i, j));
                let other = self.island.plot_at(side);
                let plot = self.merge_plots(other, here);
                self.island.plot_map.set((i, j), plot);
            }
            let here = self.island.plot_at((i, j));
            if let Some(p) = self.find_plot(here) {
                p.set_area(p.area().wrapping_add(1));
            }
        }
    }

    /// The corner `(i, j)` sank into the sea: its plot shrinks, and goes
    /// when it was this one square with no land around it. The square's
    /// entry in the plot map (shared with the row next to it) goes when
    /// the corner beside it in that row is at sea level too. // 1078:0d00
    fn corner_sunk(&mut self, (i, j): Pos) {
        let heights = &self.island.heights;
        let above = if i == 0 { 0 } else { heights.at((i - 1, j)) };
        let left = if j == 0 { 0 } else { heights.at((i, j - 1)) };
        let below = if i < 250 { heights.at((i + 1, j)) } else { 0 };
        let right = if j < 250 { heights.at((i, j + 1)) } else { 0 };
        let alone = [above, left, below, right].iter().all(|&h| h == 0);
        let plot = self.island.plot_at((i, j));
        if plot == 0 {
            return;
        }
        // The original reads through a nil pointer for a plot without a
        // record; the port leaves the map alone then.
        // BUG(original): a plot without a record is used through a nil pointer; skipped.
        let Some(area) = self.find_plot(plot).map(|p| p.area()) else {
            return;
        };
        if alone && area < 2 {
            self.remove_plot(plot); // 1068:2a2b
            self.island.plot_map.set((i, j), 0);
        } else {
            if let Some(p) = self.find_plot(plot) {
                p.set_area(area.wrapping_sub(1));
            }
            let pair = if i % 2 == 0 {
                (i < 250).then(|| self.island.heights.at((i + 1, j)))
            } else {
                Some(self.island.heights.at((i - 1, j)))
            };
            if pair == Some(0) {
                self.island.plot_map.set((i, j), 0);
            }
        }
    }

    /// A new plot in front of the list, numbered one past the last: one
    /// square, nothing on it, its ratings at 50. Its number. The original
    /// leaves nothing of the record unset that the port knows of.
    /// // 1068:2698
    fn new_plot(&mut self) -> u16 {
        let id = self.plot_count().wrapping_add(1);
        let mut plot = Plot::new(id.cast_signed());
        plot.set_area(1);
        for at in 3..=0x19 {
            plot.set_byte(at, 50);
        }
        self.plots.insert(0, plot);
        id
    }

    /// The plot `gone` merges into `keep`: its counts and sums are added
    /// (`+2`, the `Longint`s at `+0x1a` and `+0x1e`, the four tables of 51
    /// `Longint`s from `+0x22` (1010:2225), `+0x352` to `+0x356`), its
    /// buildings go after `keep`'s (1068:1da5, 1068:1e09), the ratings go
    /// back to 50, its squares become `keep`'s, and it's removed
    /// (1068:2a2b). The merged plot's number, which the removal may have
    /// changed; `gone` if `keep` is 0, `keep` if `gone` is 0 or the same.
    /// // 1068:27a5
    fn merge_plots(&mut self, gone: u16, keep: u16) -> u16 {
        if keep == 0 {
            return gone;
        }
        if gone == 0 || gone == keep {
            return keep;
        }
        let find = |w: &World, id: u16| w.plots.iter().position(|p| p.id().cast_unsigned() == id);
        // The original reads through nil pointers for numbers without a
        // record; the port only renumbers the map then.
        // BUG(original): plots without a record are used through nil pointers; skipped.
        if let (Some(g), Some(k)) = (find(self, gone), find(self, keep)) {
            let from = self.plots[g].clone();
            let into = &mut self.plots[k];
            into.set_ports(into.ports().wrapping_add(from.ports()));
            let mut add = |at: usize| into.set_longint(at, into.longint(at).wrapping_add(from.longint(at)));
            add(0x1a);
            add(0x1e);
            for at in (0x22..0x352).step_by(4) {
                add(at);
            }
            into.set_built(into.built().wrapping_add(from.built()));
            into.set_palms(into.palms().wrapping_add(from.palms()));
            into.set_area(into.area().wrapping_add(from.area()));
            into.houses.extend_from_slice(&from.houses);
            into.facilities.extend_from_slice(&from.facilities);
            for at in 3..=0x19 {
                into.set_byte(at, 50);
            }
        }
        self.island.plot_map.replace(gone, keep);
        let last = self.plot_count();
        self.remove_plot(gone);
        // The last plot takes the removed one's number, `keep` too.
        if keep == last { gone } else { keep }
    }

    /// Removes the plot `id`. The last one (the list's head) takes its
    /// number and its place in the list, and its squares in the plot map.
    /// // 1068:2a2b
    fn remove_plot(&mut self, id: u16) {
        let count = self.plot_count();
        if self.plots.is_empty() {
            return;
        }
        if id == count {
            self.plots.remove(0);
            return;
        }
        let mut head = self.plots.remove(0);
        if let Some(n) = self.plots.iter().position(|p| p.id().cast_unsigned() == id) {
            head.set_id(id.cast_signed());
            self.plots[n] = head;
        } else {
            // The original's list always has it.
            self.plots.insert(0, head);
            return;
        }
        self.island.plot_map.replace(count, id);
    }

    /// How many plots there are (`1110:552c`): the list's length, as the
    /// plots are numbered 1 to that.
    fn plot_count(&self) -> u16 {
        u16::try_from(self.plots.len()).unwrap_or(u16::MAX)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::Layer;

    /// Flat land at height `h` for `(1..=60, 1..=60)`, sea around it,
    /// with plot 1 on it.
    fn world(h: u8) -> World {
        let mut w = World {
            land_costs: LandCosts {
                from_water: 100,
                raise: 10,
                into_water: 50,
                lower: 5,
                palm: 1,
            },
            ..World::default()
        };
        w.island.layer_551c = Layer::filled(0xff);
        for i in 0..=60 {
            for j in 0..=60 {
                w.island.heights.set((i, j), h);
                w.island.forms.set((i, j), 16);
                w.island.ground.set((i, j), 1);
                w.island.plot_map.set((i, j), 1);
            }
        }
        let mut plot = Plot::new(1);
        plot.set_area(61 * 61);
        w.plots = vec![plot];
        w
    }

    #[test]
    fn raising_spreads_a_cone() {
        let mut w = world(3);
        let mut tick = Tick::default();
        let (error, cost) = w.level_corner(2, 5, (30, 30), &mut tick);
        assert_eq!((error, cost), (0, 10));
        assert_eq!(w.island.heights.at((30, 30)), 4);
        // Again: the neighbours follow so no step is steeper than one.
        let (error, cost) = w.level_corner(2, 5, (30, 30), &mut tick);
        assert_eq!(error, 0);
        assert_eq!(w.island.heights.at((30, 30)), 5);
        for n in [(31, 30), (30, 31), (29, 30), (30, 29)] {
            assert_eq!(w.island.heights.at(n), 4, "{n:?}");
        }
        assert_eq!(w.island.heights.at((31, 31)), 3);
        assert_eq!(cost, 50);
        // The squares around the top have the forms of its slopes: the
        // square (30, 30) has the top as its corner (i, j).
        assert_eq!(w.island.forms.at((30, 30)), 10);
        assert_eq!(w.island.forms.at((31, 31)), 8);
        assert_eq!(w.island.forms.at((40, 40)), 16);
        assert_ne!(tick.redraw.len(), 0);
    }

    #[test]
    fn lowering_digs_a_funnel() {
        let mut w = world(3);
        let mut tick = Tick::default();
        assert_eq!(w.set_corner_height(2, 1, (30, 30), &mut tick), (0, 5 + 5 * 5));
        assert_eq!(w.island.heights.at((30, 30)), 1);
        assert_eq!(w.island.heights.at((31, 30)), 2);
        assert_eq!(w.island.heights.at((32, 30)), 3);
        // A road on one of the squares stops it.
        w.island.ground.set((40, 40), 12);
        assert_eq!(w.level_corner(2, 1, (40, 40), &mut tick).0, 2);
        assert_eq!(w.island.heights.at((40, 40)), 3);
        // So does a building but a palm, which goes for its price.
        w.island.buildings.set((20, 21), PALM);
        assert_eq!(w.level_corner(2, 2, (20, 20), &mut tick), (0, 6));
        assert_eq!(w.island.buildings.at((20, 21)), NO_BUILDING);
        w.island.buildings.set((10, 10), 5);
        assert_eq!(w.level_corner(2, 2, (10, 10), &mut tick).0, 1);
    }

    #[test]
    fn the_edge_and_the_top() {
        let mut w = world(3);
        let mut tick = Tick::default();
        assert_eq!(w.level_corner(2, 9, (0, 30), &mut tick).0, OFF_THE_MAP);
        // Too close to the edge for a cone of that height.
        assert_eq!(w.level_corner(2, 9, (4, 30), &mut tick).0, 7);
        assert_eq!(w.level_corner(2, 9, (30, 4), &mut tick).0, 6);
        w.island.heights.set((30, 30), 20);
        assert_eq!(w.level_corner(2, 30, (30, 30), &mut tick).0, 4);
        // Player 1 has to pay for it.
        w.island.heights.set((30, 30), 3);
        assert_eq!(w.level_corner(1, 9, (30, 30), &mut tick).0, 0xe);
        assert_eq!(w.island.heights.at((30, 30)), 3);
    }

    #[test]
    fn land_from_the_sea() {
        let mut w = world(0);
        let mut tick = Tick::default();
        // An island in the middle of the sea becomes a new plot.
        w.island.plot_map = crate::world::PlotMap::empty();
        assert_eq!(w.level_corner(2, 1, (30, 30), &mut tick), (0, 100));
        assert_eq!(w.plots.len(), 2);
        assert_eq!(w.plots[0].id(), 2);
        assert_eq!(w.island.plot_map.get((30, 30)), Some(2));
        assert_eq!(w.plots[0].area(), 1);
        // Next to it, it grows.
        assert_eq!(w.level_corner(2, 1, (30, 31), &mut tick).0, 0);
        assert_eq!(w.island.plot_map.get((30, 31)), Some(2));
        assert_eq!(w.plots[0].area(), 2);
        // Another island, then land joining the two: they merge.
        assert_eq!(w.level_corner(2, 1, (30, 33), &mut tick).0, 0);
        assert_eq!(w.plots.len(), 3);
        assert_eq!(w.island.plot_map.get((30, 33)), Some(3));
        w.plots[0].set_longint(0x22, 7);
        w.plots[1].set_longint(0x22, 5);
        assert_eq!(w.level_corner(2, 1, (30, 32), &mut tick).0, 0);
        assert_eq!(w.plots.len(), 2);
        let merged = w.island.plot_map.get((30, 32)).unwrap();
        assert_eq!(w.island.plot_map.get((30, 31)), Some(merged));
        assert_eq!(w.island.plot_map.get((30, 33)), Some(merged));
        let p = w.plots.iter().find(|p| p.id().cast_unsigned() == merged).unwrap();
        assert_eq!(p.area(), 4);
        assert_eq!(p.longint(0x22), 12);
        // Sinking a corner shrinks it.
        assert_eq!(w.level_corner(2, 0, (30, 33), &mut tick).0, 0);
        let p = w.plots.iter().find(|p| p.id().cast_unsigned() == merged).unwrap();
        assert_eq!(p.area(), 3);
    }

    /// Tables with the palm's rows as on the CD.
    fn palm_tables(w: &mut World) {
        let mut t = crate::world::Tables {
            hausnr: vec![[0; 8]; 13],
            hausdata: vec![[0; 10]; 13],
            ..crate::world::Tables::default()
        };
        t.hausnr[usize::from(PALM)] = [10, 12, 0, 1, 1, 1, 10, 0];
        t.hausdata[usize::from(PALM)][9] = 106;
        w.tables = t.into();
        w.max_location_effect = 30;
    }

    #[test]
    fn fix_palm_removal_by_moving_the_land() {
        for fixed in [false, true] {
            let mut w = world(3);
            palm_tables(&mut w);
            w.fixes.set(Fix::PalmRemoval, fixed);
            w.plots[0].set_palms(5);
            w.island.buildings.set((20, 21), PALM);
            w.island.parts.set((20, 21), 3 * 10 + 2);
            w.island.owners.set((20, 21), 4);
            w.add_location_effect((20, 21), PALM);
            let with_palm = w.island.location.clone();
            assert_eq!(w.level_corner(2, 2, (20, 20), &mut Tick::default()), (0, 6));
            assert_eq!(w.island.buildings.at((20, 21)), NO_BUILDING);
            if fixed {
                assert_eq!(w.plots[0].palms(), 4);
                assert_eq!(w.island.owners.at((20, 21)), 0);
                assert_eq!(w.island.location, crate::world::LocationMap::filled(100));
            } else {
                // Only the building goes.
                assert_eq!(w.plots[0].palms(), 5);
                assert_eq!(w.island.owners.at((20, 21)), 4);
                assert_eq!(w.island.location, with_palm);
            }
        }
    }

    #[test]
    fn fix_levelling_cliffs() {
        for fixed in [false, true] {
            let mut w = world(3);
            w.fixes.set(Fix::LevellingCliffs, fixed);
            // A cliff three steps down from (30, 30) to (31, 30), and four
            // up from (40, 40) to (41, 40).
            w.island.heights.set((31, 30), 0);
            w.island.heights.set((41, 40), 7);
            assert_eq!(w.level_corner(2, 4, (30, 30), &mut Tick::default()).0, 0);
            assert_eq!(w.level_corner(2, 2, (40, 40), &mut Tick::default()).0, 0);
            // The original leaves them, so they get steeper.
            let (low, high) = if fixed { (1, 6) } else { (0, 7) };
            assert_eq!(w.island.heights.at((31, 30)), low);
            assert_eq!(w.island.heights.at((41, 40)), high);
        }
    }

    #[test]
    fn fix_levelling_errors() {
        for fixed in [false, true] {
            let mut w = world(3);
            w.fixes.set(Fix::LevellingErrors, fixed);
            w.finances.money[0] = -100;
            // Player 1 without money, a building in the way: the original
            // says the money.
            w.island.buildings.set((30, 30), 5);
            let in_the_way = if fixed { 1 } else { 0xe };
            assert_eq!(w.level_corner(1, 9, (30, 30), &mut Tick::default()).0, in_the_way);
            assert_eq!(w.level_corner(1, 1, (30, 30), &mut Tick::default()).0, in_the_way);
            // Lowering looks at the other squares after the first
            // building: the last is the one in the way.
            w.island.buildings.set((11, 11), 5);
            w.island.buildings.set((10, 10), 5);
            let (error, at) = w.lower_corner(2, (10, 10), &mut Tick::default());
            assert_eq!(error.0, 1);
            assert_eq!(at, if fixed { (10, 10) } else { (11, 11) });
        }
    }

    #[test]
    fn fix_plot_merge_numbers() {
        for fixed in [false, true] {
            let mut w = world(0);
            w.fixes.set(Fix::PlotMergeNumbers, fixed);
            w.island.plot_map = crate::world::PlotMap::empty();
            w.plots = vec![Plot::new(300)];
            w.island.heights.set((29, 30), 1);
            w.island.plot_map.set((29, 30), 300);
            assert_eq!(w.level_corner(2, 1, (30, 30), &mut Tick::default()).0, 0);
            // The original takes plot 300 for 44, which has no record.
            let plot = if fixed { 300 } else { 300 % 256 };
            assert_eq!(w.island.plot_map.get((30, 30)), Some(plot));
            assert_eq!(w.plots[0].area(), u16::from(fixed));
        }
    }
}
