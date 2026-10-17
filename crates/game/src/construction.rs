//! The buildings (segment `1078`): the building sites (`*1110:6f08`),
//! which count down a day at a time and then become the building (with
//! the accounting's 1008:5212), the finished buildings' records and sums,
//! demolishing, and the tick that lets the buildings send out people.

use std::sync::Arc;

use crate::accounts::{BOOK_NEW_BUILDINGS, BOOK_OTHER};
use crate::darstellung::GROUND_ROWS;
use crate::roads::{BRIDGES, CROSSINGS, ROADS};
use crate::simulation::Tick;
use crate::world::{BuildingSite, Facility, House, MAX_BEACH_LIFE, NO_BUILDING, Pos, World, on_map};

/// The building type of a building site. // 1078:1995
pub const BUILDING_SITE: u8 = 31;

/// The building types that are built with a random look: their part is
/// `Random(looks) * HAUSNR.DAT column 5`. // 1078:1de9
const RANDOM_LOOK: [u8; 3] = [0, 12, 0x15];

/// The building type whose part depends on the ground: 1 on sand.
/// // 1078:1de9
const ON_SAND: u8 = 0x3b;

/// The first and last of the championships' buildings, which start a
/// championship when they're finished (10a0:02ad). // 1078:203d
const CHAMPIONSHIPS: std::ops::RangeInclusive<u8> = 0x6f..=0x7c;

/// The building types that raise their owner's reputation by 30 when
/// they're finished. // 1110:1458 (`set of Byte`)
const REPUTATION_BUILDINGS: [u8; 9] = [29, 30, 35, 37, 60, 108, 110, 125, 126];

/// The building types [`World::demolish`] doesn't take for buildings:
/// the bridges (11 over a river, 52 over the sea), palms, beach life and
/// empty squares. Their squares are demolished as roads or bridges, or
/// not at all. // 1110:1378 (`set of Byte`)
const NOT_BUILDINGS: [u8; 5] = [11, 12, 36, 52, NO_BUILDING];

/// The championships' buildings while a championship is on: they can't be
/// demolished, and replacing an empty one by one of these fills it.
/// // 1110:1418 (`set of Byte`)
pub(crate) const FULL_CHAMPIONSHIPS: [u8; 7] = [0x70, 0x75, 0x76, 0x77, 0x78, 0x7b, 0x7c];
/// The empty championship buildings. // 1110:1438 (`set of Byte`)
pub(crate) const EMPTY_CHAMPIONSHIPS: [u8; 7] = [0x6f, 0x71, 0x72, 0x73, 0x74, 0x79, 0x7a];

/// Which list a building's record goes in, by its `HAUSDATA.DAT` kind
/// (column 5) from -1 to 11: 0 the houses, 1 and 2 the facilities (their
/// capacities summed apart), 3 none. The original's table is
/// `array[0..10]` at 1110:1341; kind -1 reads the byte before it, 11 the
/// one after. // 1110:1340
const RECORD_KINDS: [u8; 13] = [3, 0, 0, 1, 1, 2, 2, 2, 3, 1, 2, 2, 2];

/// The capacity class of the facilities of kinds 2 to 10.
/// // 1110:134e (`1110:134c[kind]`)
const CLASSES: [u8; 9] = [0, 1, 3, 4, 5, 10, 2, 6, 6];

/// The `HAUSDATA.DAT` kind of parks, whose squares count with the palms.
/// // 1078:0eda
const PARKS: i16 = 6;
/// The kinds of airports and harbours, counted on their plot. // 1078:0eda
const PORTS: [i16; 2] = [9, 10];

/// The rentals, whose vehicles go when they're demolished: the bicycles,
/// the jeeps, the car showroom and the surfboards. // 1078:273a
const RENTALS: [u8; 4] = [0x1a, 0x0a, 0x1b, 0x6d];

/// What demolishing is for the original's error messages (`FEHLER.TXT`,
/// shown to player 1 by 1030:232d): there isn't enough money.
pub const NOT_ENOUGH_MONEY: u8 = 0xe;
/// It isn't the player's.
pub const NOT_YOURS: u8 = 0x17;
/// Nothing there can be demolished.
pub const NOT_POSSIBLE: u8 = 0x18;
/// A championship is on in it ("the spectators protest").
pub const SPECTATORS_PROTEST: u8 = 0x1d;

/// A finished building's record: in [`World::houses`] or
/// [`World::facilities`]. // 1078:169a
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Record {
    House(usize),
    Facility(usize),
}

impl World {
    /// A new day for the building sites: each counts down a day, and the
    /// ones at 0 or less are finished, in the list's order. // 1008:5212
    ///
    /// The original also closes the info box (`infobox2`, 1038:1ee0) if it
    /// shows a square of a finished site.
    pub(crate) fn construction_tick(&mut self, tick: &mut Tick) {
        let mut n = 0;
        while n < self.building_sites.len() {
            let site = &mut self.building_sites[n];
            site.days = site.days.wrapping_sub(1);
            if site.days < 1 {
                self.finish_building(n, tick);
            } else {
                n += 1;
            }
        }
    }

    /// The building site number `n` becomes its building. // 1078:203d
    fn finish_building(&mut self, n: usize, tick: &mut Tick) {
        let site = self.building_sites[n];
        self.place_building(&site, tick);
        let owner = self.island.owners.get(site.pos).unwrap_or(0);
        if (2..=self.player_count).contains(&owner) {
            // The building's cost comes out of the budget it was planned
            // with. The original doesn't check the budget's number.
            let index = i32::from(owner - 2) * 13 + i32::from(site.budget);
            let cost = i32::from(self.tables.hausdata(site.building, 3));
            if let Some(b) = usize::try_from(index)
                .ok()
                .and_then(|i| self.computer_budgets.get_mut(i))
            {
                *b = b.wrapping_sub(cost);
            }
        }
        self.remove_workers(site.building, site.pos); // 1088:6bac
        if CHAMPIONSHIPS.contains(&site.building) {
            self.start_championship(site.building, site.pos); // 10a0:02ad
        } else if REPUTATION_BUILDINGS.contains(&site.building) {
            // Whoever owns the square, 0 and the human included.
            let r = self.reputation(owner);
            self.set_reputation(owner, r.saturating_add(30).min(100));
        }
        self.building_sites.remove(n); // 1068:2153
    }

    /// Puts the site's building on its squares in place of the building
    /// site, whose effect comes off the location map square by square,
    /// and registers the building. // 1078:1de9
    ///
    /// The original then redraws the area and the overview map, and the
    /// menu of the square under the mouse (1080:0123).
    fn place_building(&mut self, site: &crate::world::BuildingSite, tick: &mut Tick) {
        let b = site.building;
        let t = std::sync::Arc::clone(&self.tables);
        let looks = t.hausnr(b, 0);
        // The part of its first square; the others count on from it.
        let first: u8 = if looks < 2 {
            0
        } else if RANDOM_LOOK.contains(&b) {
            let look = i32::from(self.random.random(looks.cast_unsigned()));
            look.wrapping_mul(i32::from(t.hausnr(b, 5))).to_le_bytes()[0]
        } else if b == ON_SAND {
            u8::from(self.island.ground.get(site.pos) == Some(0))
        } else {
            0
        };
        let width = t.hausnr(b, 3);
        // The loops count in bytes, as the original's.
        let (i0, j0) = (site.pos.0.to_le_bytes()[0], site.pos.1.to_le_bytes()[0]);
        let i1 = width.wrapping_add(site.pos.0).wrapping_sub(1).to_le_bytes()[0];
        let j1 = t.hausnr(b, 4).wrapping_add(site.pos.1).wrapping_sub(1).to_le_bytes()[0];
        for j in j0..=j1 {
            for i in i0..=i1 {
                let pos = (i16::from(i), i16::from(j));
                let (centre, value, radius) = self.location_effect(pos, BUILDING_SITE);
                self.change_location(centre, value, radius, true); // 1030:1d10 with remove
                self.island.buildings.set(pos, b);
                let part = first
                    .wrapping_add(i)
                    .wrapping_sub(i0)
                    .wrapping_add(width.to_le_bytes()[0].wrapping_mul(j.wrapping_sub(j0)));
                self.island.parts.set(pos, part);
                tick.redraw.push(pos);
            }
        }
        self.register_building(b, site.pos);
    }

    /// The building site whose area holds `pos`, if the square shows one.
    /// // 1078:1995
    pub(crate) fn find_building_site(&self, (i, j): Pos) -> Option<usize> {
        if self.island.buildings.get((i, j)) != Some(BUILDING_SITE) {
            return None;
        }
        let t = &self.tables;
        self.building_sites.iter().position(|s| {
            s.pos.0 <= i
                && s.pos.1 <= j
                && i < t.hausnr(s.building, 3).wrapping_add(s.pos.0)
                && j < t.hausnr(s.building, 4).wrapping_add(s.pos.1)
        })
    }
}

impl World {
    /// Which list a building's record goes in: an index into
    /// [`RECORD_KINDS`] by its `HAUSDATA.DAT` kind, 3 (none) past it.
    pub(crate) fn record_kind(&self, building: u8) -> u8 {
        let kind = self.tables.hausdata(building, 5);
        usize::try_from(kind.wrapping_add(1))
            .ok()
            .and_then(|k| RECORD_KINDS.get(k))
            .copied()
            .unwrap_or(3)
    }

    /// A facility's capacity class, 0–10: by its kind for 2 to 10, by
    /// the type for the rubbish dump (9), power station (7) and sewage
    /// works (8) of kind 11, else 10, also for houses. // 1010:2493
    #[must_use]
    pub fn facility_class(&self, building: u8) -> u8 {
        let kind = self.tables.hausdata(building, 5);
        if !(1..=2).contains(&self.record_kind(building)) {
            return 10;
        }
        match kind {
            2..=10 => CLASSES[usize::try_from(kind - 2).unwrap_or(0)],
            11 => match building {
                0x1c => 9,
                0x20 => 7,
                0x21 => 8,
                _ => 10,
            },
            _ => 10,
        }
    }

    /// Where `building`'s capacity is summed: the offset in its plot and
    /// in [`World::totals`], by its record's list. // 1078:0eda
    fn capacity_sums(&self, building: u8) -> Option<(usize, usize)> {
        let class = usize::from(self.facility_class(building));
        match self.record_kind(building) {
            0 => Some((0x22, 2)),
            1 => Some((0x4e + 0x2c * class, 0x2e + 0x2c * class)),
            2 => Some((0xc6 + 4 * class, 0xa6 + 4 * class)),
            _ => None,
        }
    }

    /// Adds `by` to the totals' `Longint` at `at`.
    fn add_to_total(&mut self, at: usize, by: i32) {
        if let Some(b) = self.totals.get_mut(at..at + 4) {
            let v = i32::from_le_bytes([b[0], b[1], b[2], b[3]]).wrapping_add(by);
            b.copy_from_slice(&v.to_le_bytes());
        }
    }

    /// Adds `by` to the totals' count of plots with an airport or harbour.
    fn add_port_plots(&mut self, by: i16) {
        let n = i16::from_le_bytes([self.totals[0], self.totals[1]]).wrapping_add(by);
        self.totals[..2].copy_from_slice(&n.to_le_bytes());
    }

    /// Registers a finished building at `pos`, if the square is on a
    /// plot: its squares (`HAUSNR.DAT` column 5) count on the plot, as
    /// green for a park, it gets its record in the houses or facilities,
    /// its capacity (`HAUSDATA.DAT` column 3) goes into the plot's and the
    /// island's sums by its class, airports and harbours count on the
    /// plot, and its effect goes onto the location map. // 1078:0eda
    ///
    /// The original writes through a nil pointer for a plot number with
    /// no record; the port leaves the plot out then.
    pub(crate) fn register_building(&mut self, building: u8, pos: Pos) {
        self.count_building(building, pos, 1);
    }

    /// Undoes [`World::register_building`] for the building at `pos`, its
    /// first square: its record goes and its effect comes off the location
    /// map. // 1078:124e (through 1078:1684)
    fn unregister_building(&mut self, building: u8, pos: Pos) {
        self.count_building(building, pos, -1);
    }

    /// [`World::register_building`] for `sign` 1, the unregistering for
    /// -1. // 1078:0eda, 1078:124e
    fn count_building(&mut self, building: u8, pos: Pos, sign: i16) {
        let plot = self.island.plot_at(pos); // 1030:1e62
        if plot == 0 {
            return;
        }
        let t = Arc::clone(&self.tables);
        let kind = t.hausdata(building, 5);
        let squares = t.hausnr(building, 5).wrapping_mul(sign);
        if let Some(p) = self.find_plot(plot) {
            if kind == PARKS {
                p.set_palms(p.palms().wrapping_add(squares));
            } else {
                p.set_built(p.built().wrapping_add(squares));
            }
        }
        match (self.record_kind(building), sign > 0) {
            (0, true) => self.new_house(building, pos),        // 1068:1a08
            (1 | 2, true) => self.new_facility(building, pos), // 1068:1b3e
            (0, false) => self.remove_house(pos),              // 1068:1e6d
            (1 | 2, false) => self.remove_facility(pos),       // 1068:1fe0
            _ => {}
        }
        if let Some((at_plot, at_total)) = self.capacity_sums(building) {
            let capacity = i32::from(t.hausdata(building, 3)).wrapping_mul(i32::from(sign));
            if let Some(p) = self.find_plot(plot) {
                p.set_longint(at_plot, p.longint(at_plot).wrapping_add(capacity));
            }
            self.add_to_total(at_total, capacity);
        }
        if PORTS.contains(&kind)
            && let Some(p) = self.find_plot(plot)
        {
            let ports = p.ports().wrapping_add_signed(i8::try_from(sign).unwrap_or(0));
            p.set_ports(ports);
            // The first one on the plot, or the last one gone.
            if ports == u8::from(sign > 0) {
                self.add_port_plots(sign);
            }
        }
        if sign > 0 {
            self.add_location_effect(pos, building); // 1030:1d10
        } else {
            self.remove_location_effect(pos, building); // 1030:1d10 with remove
        }
    }

    /// A new house's record in front of the others and of its plot's: in
    /// the condition of a new one, with its price (`HAUSPREI.DAT` column
    /// 2), `HAUSDATA.DAT` column 4 and how well it's connected.
    /// // 1068:1a08
    fn new_house(&mut self, building: u8, pos: Pos) {
        let t = Arc::clone(&self.tables);
        let mut house = House::new(pos);
        house.set_condition(0x7f);
        house.set_building(building);
        house.set_price(t.hausprei(building, 2));
        house.set_rent_rating(0x50);
        house.set_stars(t.hausdata(building, 4).to_le_bytes()[0]);
        house.set_connection(self.connection(building, pos));
        self.houses.insert(0, house);
        if let Some(p) = self.plot_of(pos) {
            p.houses.insert(0, pos); // 1068:1d05
        }
    }

    /// The same for a facility. // 1068:1b3e
    fn new_facility(&mut self, building: u8, pos: Pos) {
        let t = Arc::clone(&self.tables);
        let mut f = Facility::new(building, pos);
        f.set_connection(self.connection(building, pos));
        f.set_stars(t.hausdata(building, 4).to_le_bytes()[0]);
        f.set_price(t.hausprei(building, 2));
        self.facilities.insert(0, f);
        if let Some(p) = self.plot_of(pos) {
            p.facilities.insert(0, pos); // 1068:1d55
        }
    }

    /// Takes the first house at `pos` off the list and its plot's.
    /// // 1068:1e6d
    fn remove_house(&mut self, pos: Pos) {
        if let Some(n) = self.houses.iter().position(|h| h.pos() == pos) {
            self.houses.remove(n);
            if let Some(p) = self.plot_of(pos)
                && let Some(k) = p.houses.iter().position(|&h| h == pos)
            {
                p.houses.remove(k); // 1068:2243
            }
        }
    }

    /// The same for a facility. // 1068:1fe0
    fn remove_facility(&mut self, pos: Pos) {
        if let Some(n) = self.facilities.iter().position(|f| f.pos() == pos) {
            self.facilities.remove(n);
            if let Some(p) = self.plot_of(pos)
                && let Some(k) = p.facilities.iter().position(|&f| f == pos)
            {
                p.facilities.remove(k); // 1068:233c
            }
        }
    }

    /// The record of the building on `pos`, by its first square. // 1078:169a
    pub(crate) fn find_record(&self, pos: Pos) -> Option<Record> {
        let origin = self.building_origin(pos);
        let building = self.island.buildings.get(origin)?;
        match self.record_kind(building) {
            0 => self.houses.iter().position(|h| h.pos() == origin).map(Record::House),
            1 | 2 => self
                .facilities
                .iter()
                .position(|f| f.pos() == origin)
                .map(Record::Facility),
            _ => None,
        }
    }

    /// The first square of the building on `pos`, from the square's part:
    /// the part within its look (`HAUSNR.DAT` column 5 squares each) counts
    /// along `i` first, `width` (column 3) at a time. // 1030:154c
    ///
    /// The original stops with a division by zero for a type with no
    /// squares or width; the port takes the square itself then.
    #[must_use]
    pub fn building_origin(&self, (i, j): Pos) -> Pos {
        let building = self.island.buildings.get((i, j)).unwrap_or(NO_BUILDING);
        let part = i16::from(self.island.parts.get((i, j)).unwrap_or(0));
        let t = &self.tables;
        let part = part.checked_rem(t.hausnr(building, 5)).unwrap_or(0);
        let width = t.hausnr(building, 3);
        (
            i.wrapping_sub(part.checked_rem(width).unwrap_or(0)),
            j.wrapping_sub(part.checked_div(width).unwrap_or(0)),
        )
    }

    /// How well a new building at `pos` is connected:
    /// [`World::connection_values`] for the distance to the nearest road.
    /// // 1068:1a08
    pub(crate) fn connection(&self, building: u8, (i, j): Pos) -> u8 {
        let t = &self.tables;
        let end = (
            t.hausnr(building, 3).wrapping_add(i).wrapping_sub(1),
            t.hausnr(building, 4).wrapping_add(j).wrapping_sub(1),
        );
        let distance = self.road_distance((i, j), end);
        usize::try_from(distance + 1)
            .ok()
            .and_then(|d| self.connection_values.get(d))
            .copied()
            .unwrap_or(0)
    }

    /// How many squares the nearest road on the same plot is from the area
    /// `start..=end`, looking in rings 1 to 4 around it; -1 if there's
    /// none that close. Airports and harbours count as roads. // 1030:1706
    fn road_distance(&self, (i, j): Pos, (i_end, j_end): Pos) -> i16 {
        let plot = self.island.plot_at((i, j));
        for d in 1..5 {
            let mut found = false;
            for a in i - d..=i_end + d {
                found |= self.is_road(plot, (a, j - d)) | self.is_road(plot, (a, j_end + d));
            }
            for b in (j - d + 1)..(j_end + d) {
                found |= self.is_road(plot, (i - d, b)) | self.is_road(plot, (i_end + d, b));
            }
            if found {
                return d;
            }
        }
        -1
    }

    /// Whether `pos` is on `plot` and a road, an airport or a harbour.
    /// // 1030:15d2
    fn is_road(&self, plot: u16, pos: Pos) -> bool {
        if !on_map(pos) || self.island.plot_at(pos) != plot {
            return false;
        }
        let ground = self.island.ground.at(pos);
        let building = self.island.buildings.at(pos);
        ROADS.contains(&ground) || (building < 0x80 && PORTS.contains(&self.tables.hausdata(building, 5)))
    }

    /// Gives the empty square `pos` its ground back by its corners: water
    /// with a corner at sea level, sand one step up (or sand to grass,
    /// part 1 or 0, where another corner is 3 high), grass above. A variant
    /// (part) the new tile doesn't have becomes 0. Nothing for a square
    /// with a building on it. The coordinates are bytes. // 10b0:3199
    pub(crate) fn reset_ground(&mut self, (i, j): Pos) {
        let pos = (i16::from(i.to_le_bytes()[0]), i16::from(j.to_le_bytes()[0]));
        if !on_map(pos) || self.island.buildings.at(pos) != NO_BUILDING {
            return;
        }
        let low = self.island.lowest_corner(pos); // 1030:208d
        let ground = match low {
            0 => 2,
            1 | 2 if self.island.highest_corner(pos) < 3 => 0,
            1 | 2 => {
                self.island.parts.set(pos, u8::from(low == 1));
                4
            }
            _ => 1,
        };
        self.island.ground.set(pos, ground);
        if ground != 4 {
            let row = usize::from(GROUND_ROWS[usize::from(ground)]) + usize::from(self.island.forms.at(pos));
            if self.tables.groundnr(row, 0) <= i16::from(self.island.parts.at(pos)) {
                self.island.parts.set(pos, 0);
            }
        }
    }

    /// Empties a square of a demolished building or building site: no
    /// building, part or owner, nobody in it, its ground back. Nothing off
    /// the map.
    fn empty_square(&mut self, pos: Pos, tick: &mut Tick) {
        if !on_map(pos) {
            return;
        }
        self.island.buildings.set(pos, NO_BUILDING);
        self.island.parts.set(pos, 0);
        self.reset_ground(pos); // 10b0:3199
        self.island.owners.set(pos, 0);
        self.island.occupancy.set(pos, 0);
        tick.redraw.push(pos);
    }

    /// Demolishes what's on `pos` for `player`, or for no one (0, as
    /// disasters do), which skips the check that it's theirs: a building
    /// site (its price back for the days left), a building (for a 20th of
    /// its price), a road (`Strassenabrisskosten`) or a bridge over the
    /// sea (for nothing). `Err` holds the error message the original
    /// shows player 1 (marked; [`NOT_POSSIBLE`] and the others); other
    /// players get none. // 1078:280b
    ///
    /// The original also redraws the area on the overview map and
    /// refreshes the money menu (1070:2617).
    ///
    /// # Errors
    /// When nothing there can be demolished, or not by `player`.
    pub fn demolish(&mut self, player: u8, pos: Pos, tick: &mut Tick) -> Result<(), u8> {
        if !on_map(pos) {
            return Err(NOT_POSSIBLE);
        }
        let building = self.island.buildings.at(pos);
        let theirs = player == 0 || self.island.owners.at(pos) == player;
        if !NOT_BUILDINGS.contains(&building) {
            if !theirs {
                return Err(NOT_YOURS);
            }
            if building == BUILDING_SITE {
                self.demolish_site(player, pos, tick);
                return Ok(());
            }
            if FULL_CHAMPIONSHIPS.contains(&building) {
                return Err(SPECTATORS_PROTEST);
            }
            return self.demolish_building(player, building, pos, tick);
        }
        let ground = self.island.ground.at(pos);
        if ROADS.contains(&ground) {
            if !theirs {
                return Err(NOT_YOURS);
            }
            self.demolish_road(player, pos, tick)
        } else if BRIDGES.contains(&ground) {
            if !theirs {
                return Err(NOT_YOURS);
            }
            self.demolish_bridge(pos, tick);
            Ok(())
        } else {
            Err(NOT_POSSIBLE)
        }
    }

    /// Demolishes the building site at `pos`: its squares are emptied and
    /// their people leave, and its owner gets the price back for the days
    /// left of the building time, `price * days div time`. The site's
    /// effect stays on the location map. // 1078:280b
    ///
    /// The original reads through a nil pointer if no site's area holds
    /// `pos`, and divides by zero for a type with no building time; the
    /// port does nothing, and pays nothing back.
    fn demolish_site(&mut self, player: u8, pos: Pos, tick: &mut Tick) {
        let Some(n) = self.find_building_site(pos) else {
            return;
        };
        let site = self.building_sites[n];
        let t = Arc::clone(&self.tables);
        let (i0, j0) = site.pos;
        let i1 = t.hausnr(site.building, 3).wrapping_add(i0).wrapping_sub(1);
        let j1 = t.hausnr(site.building, 4).wrapping_add(j0).wrapping_sub(1);
        for i in i0..=i1 {
            for j in j0..=j1 {
                self.remove_people_from((i, j), tick); // 1088:6975
                self.empty_square((i, j), tick);
                self.people_map_changed(); // 1088:6c5a
            }
        }
        // Worked out negative, as the original does.
        let refund = t
            .hausprei(site.building, 0)
            .wrapping_neg()
            .wrapping_mul(i32::from(site.days))
            .checked_div(i32::from(t.hausdata(site.building, 0)))
            .unwrap_or(0);
        self.add_money(player, refund.wrapping_neg());
        if player == 1 {
            self.book(BOOK_OTHER, refund.wrapping_neg()); // 1070:1419
            self.float_money(refund, site.pos); // 1088:0153
        }
        self.building_sites.remove(n); // 1068:2153
    }

    /// Demolishes the building on `pos` for a 20th of its price: its beach
    /// life, rented vehicles and people go, its record and effect, and
    /// its squares are emptied. // 1078:280b
    fn demolish_building(&mut self, player: u8, building: u8, pos: Pos, tick: &mut Tick) -> Result<(), u8> {
        let cost = self.tables.hausprei(building, 0) / 20;
        if self.money_of(player) < cost && player == 1 {
            return Err(NOT_ENOUGH_MONEY);
        }
        self.add_money(player, cost.wrapping_neg());
        if player == 1 {
            self.book(BOOK_OTHER, cost.wrapping_neg()); // 1070:1419
        }
        let origin = self.building_origin(pos); // 1030:154c
        if player == 1 {
            self.float_money(cost, origin); // 1088:0153
        }
        self.remove_house_beach_life(origin, tick); // 1078:267e
        if self.island.buildings.get(origin).is_some_and(|b| RENTALS.contains(&b)) {
            let _ = self.remove_people_at(origin); // 1078:273a, 1088:2d2a
        }
        self.remove_people_from(origin, tick); // 1088:6975
        self.unregister_building(building, origin); // 1078:1684
        // Byte loops, as in place_building.
        let t = Arc::clone(&self.tables);
        let (i0, j0) = (origin.0.to_le_bytes()[0], origin.1.to_le_bytes()[0]);
        let i1 = t
            .hausnr(building, 3)
            .wrapping_add(origin.0)
            .wrapping_sub(1)
            .to_le_bytes()[0];
        let j1 = t
            .hausnr(building, 4)
            .wrapping_add(origin.1)
            .wrapping_sub(1)
            .to_le_bytes()[0];
        for i in i0..=i1 {
            for j in j0..=j1 {
                self.empty_square((i16::from(i), i16::from(j)), tick);
            }
        }
        self.people_map_changed(); // 1088:6c5a
        Ok(())
    }

    /// The beach life of the house whose first square is `pos`, if the
    /// building there is a house, is cleared away. // 1078:267e
    fn remove_house_beach_life(&mut self, pos: Pos, tick: &mut Tick) {
        let building = self.island.buildings.get(pos).unwrap_or(NO_BUILDING);
        if !(0..=1).contains(&self.tables.hausdata(building, 5)) {
            return;
        }
        let Some(house) = self.houses.iter().find(|h| h.pos() == pos) else {
            return;
        };
        let squares: Vec<Pos> = (1..=house.beach_life().min(MAX_BEACH_LIFE))
            .map(|n| house.beach_life_square(n))
            .collect();
        for square in squares {
            if on_map(square) {
                self.clear_square(square, tick); // 1010:122f
            }
        }
    }

    /// Demolishes the road on `pos` for `Strassenabrisskosten`: a road
    /// over a river leaves the river, another the ground, and the roads
    /// next to it get their pieces again. // 1078:280b
    fn demolish_road(&mut self, player: u8, pos: Pos, tick: &mut Tick) -> Result<(), u8> {
        let cost = self.road_demolition_cost;
        if self.money_of(player) < cost && player == 1 {
            return Err(NOT_ENOUGH_MONEY);
        }
        self.add_money(player, cost.wrapping_neg());
        self.remove_people_from(pos, tick); // 1088:6975
        if player == 1 {
            self.book(BOOK_OTHER, cost.wrapping_neg()); // 1070:1419
            self.float_money(cost, pos); // 1088:0153
        }
        self.island.owners.set(pos, 0);
        let ground = self.island.ground.at(pos);
        if let Some(&(_, river)) = CROSSINGS.iter().find(|&&(road, _)| road == ground) {
            self.island.buildings.set(pos, NO_BUILDING);
            self.island.ground.set(pos, river);
        } else {
            self.reset_ground(pos); // 10b0:3199
        }
        self.reconnect_roads(pos); // 1080:302a
        redraw_area(tick, (pos.0 - 1, pos.1 - 1), (pos.0 + 1, pos.1 + 1)); // 1050:264d
        self.people_map_changed(); // 1088:6c5a
        Ok(())
    }

    /// Demolishes the bridge over the sea through `pos`, for nothing: from
    /// `pos` along the bridge, by the part (2, 4 towards `i - 1`; 3, 5
    /// towards `j + 1`; then 0, 4 towards `i + 1`; 1, 5 towards `j - 1`),
    /// each square of bridge ground becomes water, then `pos`, and the
    /// roads at both ends get their pieces again. // 1078:280b
    ///
    /// The original adds up `Strassenabrisskosten` for each square
    /// (1078:276f), and never uses the sum.
    fn demolish_bridge(&mut self, pos: Pos, tick: &mut Tick) {
        let bridge = |w: &World, p: Pos| w.island.ground.get(p).is_some_and(|g| BRIDGES.contains(&g));
        let part = self.island.parts.at(pos);
        let mut start = pos;
        let step = match part {
            2 | 4 => Some((-1, 0)),
            3 | 5 => Some((0, 1)),
            _ => None,
        };
        if let Some((di, dj)) = step {
            let mut c = (pos.0 + di, pos.1 + dj);
            while bridge(self, c) {
                self.remove_bridge_square(c, tick);
                start = c;
                c = (c.0 + di, c.1 + dj);
            }
        }
        let mut end = pos;
        let step = match part {
            0 | 4 => Some((1, 0)),
            1 | 5 => Some((0, -1)),
            _ => None,
        };
        if let Some((di, dj)) = step {
            let mut c = (pos.0 + di, pos.1 + dj);
            while bridge(self, c) {
                self.remove_bridge_square(c, tick);
                c = (c.0 + di, c.1 + dj);
            }
            end = (c.0 - di, c.1 - dj);
        }
        self.remove_bridge_square(pos, tick);
        self.reconnect_roads(start); // 1080:302a
        self.reconnect_roads(end);
        let low = (start.0.min(end.0) - 1, start.1.min(end.1) - 1);
        let high = (start.0.max(end.0) + 1, start.1.max(end.1) + 1);
        redraw_area(tick, low, high); // 1050:264d
    }

    /// One square of a bridge over the sea becomes water, and its people
    /// leave. // 1078:276f
    fn remove_bridge_square(&mut self, pos: Pos, tick: &mut Tick) {
        self.remove_people_from(pos, tick); // 1088:6975
        self.island.buildings.set(pos, NO_BUILDING);
        self.island.parts.set(pos, 0);
        self.island.ground.set(pos, 2);
        self.island.owners.set(pos, 0);
        self.island.occupancy.set(pos, 0);
        tick.redraw.push(pos);
        self.people_map_changed(); // 1088:6c5a
    }

    /// One of 30 groups of buildings may each send someone out: the
    /// houses with `(i + j) mod 30 = slice`, with a chance of their guests
    /// (`+0x10`) in percent of their capacity (`HAUSDATA.DAT` column 3),
    /// then the facilities with `(i * j) mod 30 = slice`, with the chance
    /// of their `+0x15` (how full, in percent). Each draws `Random(100)`.
    /// // 1078:0002
    ///
    /// The original stops with a division by zero for a house type with
    /// no capacity; the port skips the house, without a random number.
    pub(crate) fn building_slice(&mut self, slice: i16, tick: &mut Tick) {
        let slice = i32::from(slice);
        for n in 0..self.houses.len() {
            let house = &self.houses[n];
            let (i, j) = house.pos();
            if (i32::from(i) + i32::from(j)) % 30 != slice {
                continue;
            }
            let capacity = i32::from(self.tables.hausdata(house.building(), 3));
            let Some(chance) = (i32::from(house.guests()) * 100).checked_div(capacity) else {
                continue;
            };
            if i32::from(self.random.random(100)) < chance {
                self.send_person((i, j), tick); // 1090:1a25
            }
        }
        for n in 0..self.facilities.len() {
            let f = &self.facilities[n];
            let (i, j) = f.pos();
            if (i32::from(i) * i32::from(j)) % 30 != slice {
                continue;
            }
            let chance = u16::from(f.fill());
            if self.random.random(100) < chance {
                self.send_person((i, j), tick); // 1090:1a25
            }
        }
    }
}

/// Squares from `low` to `high` that are on the map, for the view to
/// redraw. // 1050:264d
fn redraw_area(tick: &mut Tick, low: Pos, high: Pos) {
    for i in low.0..=high.0 {
        for j in low.1..=high.1 {
            if on_map((i, j)) {
                tick.redraw.push((i, j));
            }
        }
    }
}

/// [`World::can_build`]'s answers (`FEHLER.TXT`'s texts, shown to player
/// 1): the area is past the map's edge.
pub const OFF_THE_MAP: u8 = 0x16;

impl World {
    /// Whether `player` can build `building` with its first square at
    /// `(i, j)`, and what it costs: its price (`HAUSPREI.DAT` column 0),
    /// and for a building on any land (`HAUSDATA.DAT` column 7 is 2), not
    /// a palm, `Fundamentbaukosten` for each square that isn't level. 0
    /// when it can, else the error (`FEHLER.TXT`): 0x16 past the map, 0xe
    /// not enough money (only for player 1), 2 a road, 0xf a river's
    /// crossing, 0x10 a river, 1 a building, 0xc a square's highest corner
    /// isn't the first square's, the wrong ground for column 7 (0x11 sand,
    /// 0x12 grass, 0xb not water, 0x13 water; 4 to 7 water along one
    /// side) or the wrong form for column 8 (0x14 flat, 10 not steep,
    /// 0x15 sloping; 4 to 7 sloping along one side, 8 no palm). The
    /// squares go by rows of `j`, and the first error wins. // 1030:04e3
    ///
    /// For error 1 the original keeps the square in the way at
    /// `1110:64ba` (for the message, the view's).
    #[must_use]
    #[allow(clippy::many_single_char_names)] // The original's i, j, w and d.
    pub fn can_build(&self, (i, j): Pos, building: u8, player: u8) -> (u8, i32) {
        let t = &self.tables;
        let (w, d) = (t.hausnr(building, 3), t.hausnr(building, 4));
        if i < 0 || j < 0 || w.wrapping_add(i).wrapping_sub(1) >= 251 || d.wrapping_add(j).wrapping_sub(1) >= 251 {
            return (OFF_THE_MAP, 0);
        }
        let mut cost = t.hausprei(building, 0);
        let poor = |w: &World, cost: i32| player == 1 && w.money_of(player) < cost;
        if poor(self, cost) {
            return (NOT_ENOUGH_MONEY, cost);
        }
        let island = &self.island;
        let top = island.highest_corner((i, j));
        let ground_kind = t.hausdata(building, 7);
        let form_kind = t.hausdata(building, 8);
        let mut error = 0;
        for b in j..=j.wrapping_add(d).wrapping_sub(1) {
            for a in i..=i.wrapping_add(w).wrapping_sub(1) {
                if error != 0 {
                    return (error, cost);
                }
                let pos = (a, b);
                if island.lowest_corner(pos) < island.highest_corner(pos)
                    && ground_kind == 2
                    && building != crate::world::PALM
                {
                    cost = cost.wrapping_add(self.foundation_cost);
                    if poor(self, cost) {
                        error = NOT_ENOUGH_MONEY;
                    }
                }
                if error == 0 {
                    error = self.ground_error(pos, (i, j), top, ground_kind);
                }
                if error == 0 {
                    error = form_error(island.forms.at(pos), island.buildings.at(pos), pos, (i, j), form_kind);
                }
            }
        }
        (error, cost)
    }

    /// [`World::can_build`]'s ground: no road or river, no building but
    /// a palm, the first square's highest corner, the ground column 7
    /// asks for. // 1030:04e3
    fn ground_error(&self, pos: Pos, (i, j): Pos, top: u8, kind: i16) -> u8 {
        let g = self.island.ground.at(pos);
        match g {
            10..=54 => return 2,
            55..=58 => return 0xf,
            59.. => return 0x10,
            _ => {}
        }
        let here = self.island.buildings.at(pos);
        if here != NO_BUILDING && here != crate::world::PALM {
            return 1;
        }
        if self.island.highest_corner(pos) != top {
            return 0xc;
        }
        let (sand, water) = (if g == 0 { 0 } else { 0x11 }, if g == 2 { 0 } else { 0x13 });
        // 4 to 7: water along the first row or column, sand on the rest,
        // or the other way round.
        let (first_i, first_j) = (pos.0 == i, pos.1 == j);
        match kind {
            1 if g != 1 => 0x12,
            2 if g == 2 => 0xb,
            0 => sand,
            3 => water,
            4 => {
                if first_i {
                    sand
                } else {
                    water
                }
            }
            5 => {
                if first_j {
                    water
                } else {
                    sand
                }
            }
            6 => {
                if first_i {
                    water
                } else {
                    sand
                }
            }
            7 => {
                if first_j {
                    sand
                } else {
                    water
                }
            }
            _ => 0,
        }
    }

    /// `player` starts building `building` at `pos` for `cost`: a palm is
    /// planted, anything else becomes a building site ([`World::place_site`]),
    /// and the cost is paid. For player 1 the books' new buildings get the
    /// price and the cost rises from the square. // 1078:1d21
    ///
    /// For player 1 and a type of which there's one (`HAUSDATA.DAT` column
    /// 6 is 1), the original also takes it out of the build menu
    /// (1018:26b9, 1010:2c8b), the view's.
    pub fn start_building(&mut self, cost: i32, budget: i8, player: u8, building: u8, pos: Pos, tick: &mut Tick) {
        if building == crate::world::PALM {
            self.plant_palm(pos, tick); // 1010:1129
        } else {
            self.place_site(budget, player, pos, building, tick); // 1078:1b5e
        }
        self.add_money(player, cost.wrapping_neg());
        if player == 1 {
            let price = self.tables.hausprei(building, 0);
            self.book(BOOK_NEW_BUILDINGS, price.wrapping_neg()); // 1070:1419
            tick.books = true; // 1070:2617
            self.float_money(cost, pos); // 1088:0153
        }
    }

    /// The area of `building` at `(i, j)` (byte loops) becomes `player`'s
    /// building site: a palm on it comes off its plot's count (the first
    /// square's plot) and the location map, and each square gets the
    /// site's effect and a random look (`Random` of `HAUSNR.DAT`'s count
    /// for type 31). Then the site's record. // 1078:1b5e
    fn place_site(&mut self, budget: i8, player: u8, (i, j): Pos, building: u8, tick: &mut Tick) {
        let plot = self.island.plot_at((i, j)); // 1030:1e62
        let t = Arc::clone(&self.tables);
        let last = |v: i16, column| v.wrapping_add(t.hausnr(building, column)).wrapping_sub(1).to_le_bytes()[0];
        let (i0, i1, j0, j1) = (i.to_le_bytes()[0], last(i, 3), j.to_le_bytes()[0], last(j, 4));
        let looks = t.hausnr(BUILDING_SITE, 0).cast_unsigned();
        for a in i0..=i1 {
            for b in j0..=j1 {
                let pos = (i16::from(a), i16::from(b));
                if !on_map(pos) {
                    continue;
                }
                if self.island.buildings.at(pos) == crate::world::PALM {
                    // The original writes through a nil pointer for a
                    // square without a plot.
                    if let Some(p) = self.find_plot(plot) {
                        p.set_palms(p.palms().wrapping_sub(1));
                    }
                    self.remove_location_effect(pos, crate::world::PALM); // 1030:1d10
                }
                self.island.buildings.set(pos, BUILDING_SITE);
                self.add_location_effect(pos, BUILDING_SITE);
                let part = self.random.random(looks).to_le_bytes()[0];
                self.island.parts.set(pos, part);
                self.island.owners.set(pos, player);
                tick.redraw.push(pos); // 1050:21f9, 10b8:1624
            }
        }
        self.add_site(budget, building, (i, j)); // 1078:15d9
    }

    /// A building site's record goes in front of the list, with
    /// `HAUSDATA.DAT` column 0's days (1 while the money is frozen); for a
    /// computer player's square, the type's capacity (column 3) goes onto
    /// its `budget`. // 1078:15d9, 1068:1c7d
    fn add_site(&mut self, budget: i8, building: u8, (i, j): Pos) {
        let owner = self.island.owners.get((i, j)).unwrap_or(0);
        let days = if self.finances.frozen {
            1
        } else {
            self.tables.hausdata(building, 0)
        };
        self.building_sites.insert(
            0,
            BuildingSite {
                // Not written: whatever the heap had.
                byte_0: 0,
                // `Byte`s.
                pos: (i16::from(i.to_le_bytes()[0]), i16::from(j.to_le_bytes()[0])),
                days,
                building,
                budget,
            },
        );
        if (2..=self.player_count).contains(&owner) {
            let index = i32::from(owner - 2) * 13 + i32::from(budget);
            let capacity = i32::from(self.tables.hausdata(building, 3));
            if let Some(b) = usize::try_from(index)
                .ok()
                .and_then(|i| self.computer_budgets.get_mut(i))
            {
                *b = b.wrapping_add(capacity);
            }
        }
    }
}

/// [`World::can_build`]'s form: what column 8 asks for of the square's
/// form `f` (16 flat, 0–3 one side raised). // 1030:04e3
fn form_error(f: u8, here: u8, pos: Pos, (i, j): Pos, kind: i16) -> u8 {
    let flat = if f == 16 { 0 } else { 0x14 };
    let sloping = |side: u8| if f == side { 0 } else { 0x15 };
    match kind {
        0 => flat,
        1 if (8..=11).contains(&f) => 10,
        2 if f > 3 => 0x15,
        4 => {
            if pos.0 == i {
                flat
            } else {
                sloping(0)
            }
        }
        5 => {
            if pos.1 == j {
                sloping(1)
            } else {
                flat
            }
        }
        6 => {
            if pos.0 == i {
                sloping(2)
            } else {
                flat
            }
        }
        7 => {
            if pos.1 == j {
                flat
            } else {
                sloping(3)
            }
        }
        8 if here == crate::world::PALM => 0x1b,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use vcl::random::Random;

    use super::*;
    use crate::world::{BuildingSite, PALM, Plot, Tables};

    fn world() -> World {
        let mut hausnr = vec![[0; 8]; 127];
        hausnr[40] = [1, 0, 0, 2, 3, 0, 0, 0];
        hausnr[0] = [4, 0, 0, 1, 1, 10, 0, 0];
        hausnr[0x6f] = [1, 0, 0, 1, 1, 0, 0, 0];
        hausnr[29] = [1, 0, 0, 1, 1, 0, 0, 0];
        let mut hausdata = vec![[0; 10]; 127];
        hausdata[40][3] = 500;
        for row in &mut hausdata {
            row[9] = 100;
        }
        World {
            tables: Arc::new(Tables {
                hausnr,
                hausdata,
                hausprei: Vec::new(),
                groundnr: Vec::new(),
                ..Tables::default()
            }),
            random: Random::new(3),
            ..World::default()
        }
    }

    fn site(building: u8, pos: Pos, days: i16) -> BuildingSite {
        BuildingSite {
            pos,
            days,
            building,
            ..BuildingSite::default()
        }
    }

    fn put_site(w: &mut World, s: BuildingSite) {
        let t = Arc::clone(&w.tables);
        for i in 0..t.hausnr(s.building, 3) {
            for j in 0..t.hausnr(s.building, 4) {
                w.island.buildings.set((s.pos.0 + i, s.pos.1 + j), BUILDING_SITE);
            }
        }
        w.building_sites.push(s);
    }

    #[test]
    fn counts_down_and_finishes() {
        let mut w = world();
        put_site(&mut w, site(40, (10, 20), 2));
        put_site(&mut w, site(40, (30, 20), 1));
        let mut tick = Tick::default();
        w.construction_tick(&mut tick);
        assert_eq!(w.building_sites, [site(40, (10, 20), 1)]);
        // Two wide, three deep: the parts count along `i` first.
        assert_eq!(tick.redraw[..3], [(30, 20), (31, 20), (30, 21)]);
        let parts: Vec<_> = tick.redraw.iter().map(|&p| w.island.parts.at(p)).collect();
        assert_eq!(parts, [0, 1, 2, 3, 4, 5]);
        assert!(tick.redraw.iter().all(|&p| w.island.buildings.at(p) == 40));
        assert_eq!(w.island.buildings.at((10, 20)), BUILDING_SITE);
        w.construction_tick(&mut Tick::default());
        assert_eq!(w.building_sites, []);
        assert_eq!(w.island.buildings.at((11, 22)), 40);
        assert_eq!(w.random, Random::new(3));
    }

    #[test]
    fn computer_budget_and_reputation() {
        let mut w = world();
        w.island.owners.set((10, 20), 3);
        w.island.owners.set((50, 50), 3);
        w.set_reputation(3, 90);
        put_site(
            &mut w,
            BuildingSite {
                budget: 2,
                ..site(40, (10, 20), 1)
            },
        );
        put_site(&mut w, site(29, (50, 50), 1));
        w.construction_tick(&mut Tick::default());
        assert_eq!(w.computer_budgets[13 + 2], -500);
        assert_eq!(w.reputation(3), 100);
    }

    #[test]
    fn random_look_and_championship() {
        let mut w = world();
        put_site(&mut w, site(0, (5, 5), 1));
        put_site(&mut w, site(0x6f, (7, 7), 1));
        w.championships[0].active = true;
        w.construction_tick(&mut Tick::default());
        let mut r = Random::new(3);
        let look = r.random(4).to_le_bytes()[0];
        assert_eq!(w.island.parts.at((5, 5)), look * 10);
        assert_eq!(w.random, r);
        let c = w.championships[1];
        assert!(c.active);
        assert_eq!((c.kind, c.pos, c.countdown), (0, (7, 7), 0x3c));
    }

    /// A world with plot 1 under the squares up to 99, a house type 40
    /// (2 × 3, capacity 500), a shop type 41 (kind 3), a harbour type 42
    /// (kind 10) and a road at (10, 23).
    fn plot_world() -> World {
        let mut w = world();
        let mut t = (*w.tables).clone();
        t.hausnr[40] = [1, 0, 0, 2, 3, 6, 0, 0];
        t.hausnr[41] = [1, 0, 0, 1, 1, 1, 0, 0];
        t.hausnr[42] = [1, 0, 0, 1, 1, 1, 0, 0];
        t.hausdata[40] = [10, 0, 0, 500, 7, 0, 0, 0, 0, 110];
        t.hausdata[41] = [5, 0, 0, 50, 0, 3, 0, 0, 0, 100];
        t.hausdata[42] = [5, 0, 0, 20, 0, 10, 0, 0, 0, 100];
        t.hausprei = vec![[0; 4]; 127];
        t.hausprei[40] = [2000, 0, 80, 0];
        w.tables = Arc::new(t);
        for i in 0..100 {
            for j in 0..100 {
                w.island.plot_map.set((i, j), 1);
            }
        }
        w.plots = vec![Plot::new(1)];
        w.connection_values = [5, 0, 40, 30, 20, 10];
        w.max_location_effect = 30;
        w.island.ground.set((10, 23), 10);
        w
    }

    #[test]
    fn registers_a_house() {
        let mut w = plot_world();
        let pos = (10, 20);
        w.register_building(40, pos);
        let h = &w.houses[0];
        assert_eq!((h.building(), h.pos(), h.condition()), (40, pos, 0x7f));
        assert_eq!((h.price(), h.rent_rating(), h.stars()), (80, 0x50, 7));
        // The road is a square past the area's last row.
        assert_eq!(h.connection(), 40);
        let p = &w.plots[0];
        assert_eq!((p.built(), p.palms(), p.longint(0x22)), (6, 0, 500));
        assert_eq!(p.houses, [pos]);
        assert_eq!(w.totals[2..6], 500i32.to_le_bytes());
        assert_eq!(w.island.location.get((5, 10)), Some(110));
        assert_eq!(w.find_record((11, 22)), None);
        w.unregister_building(40, pos);
        assert_eq!(w.houses, []);
        let p = &w.plots[0];
        assert_eq!((p.built(), p.longint(0x22)), (0, 0));
        assert_eq!(p.houses, []);
        assert_eq!(w.totals, vec![0; crate::world::TOTALS_SIZE]);
        assert_eq!(w.island.location.get((5, 10)), Some(100));
    }

    #[test]
    fn no_plot_no_record() {
        let mut w = plot_world();
        w.island.plot_map.set((10, 20), 0);
        // A form that doesn't point at a neighbour's plot.
        w.island.forms.set((10, 20), 16);
        w.register_building(40, (10, 20));
        assert_eq!(w.houses, []);
        assert_eq!(w.island.location.get((5, 10)), Some(100));
    }

    #[test]
    fn facilities_and_ports() {
        let mut w = plot_world();
        assert_eq!(w.facility_class(41), 1);
        assert_eq!(w.facility_class(42), 6);
        assert_eq!(w.facility_class(40), 10);
        for (b, pos) in [(41, (50, 50)), (42, (60, 60)), (42, (60, 70))] {
            w.island.buildings.set(pos, b);
            w.register_building(b, pos);
        }
        assert_eq!(w.facilities.len(), 3);
        assert_eq!(w.facilities[2].pos(), (50, 50));
        // Far from any road; harbours count as roads for the next one.
        assert_eq!(w.facilities[2].connection(), 5);
        let p = &w.plots[0];
        assert_eq!(p.longint(0x4e + 0x2c), 50);
        assert_eq!(p.longint(0xc6 + 4 * 6), 40);
        assert_eq!(p.ports(), 2);
        assert_eq!(p.facilities, [(60, 70), (60, 60), (50, 50)]);
        assert_eq!(w.totals[..2], [1, 0]);
        assert_eq!(w.find_record((60, 60)), Some(Record::Facility(1)));
        w.unregister_building(42, (60, 60));
        assert_eq!(w.totals[..2], [1, 0]);
        w.unregister_building(42, (60, 70));
        assert_eq!((w.plots[0].ports(), w.totals[0]), (0, 0));
        assert_eq!(w.facilities.len(), 1);
    }

    #[test]
    fn demolish_a_building() {
        let mut w = plot_world();
        w.finances.money[0] = 10_000;
        for i in 9..=12 {
            for j in 19..=23 {
                w.island.heights.set((i, j), 1);
            }
        }
        w.island.owners.set((10, 20), 1);
        put_site(&mut w, site(40, (10, 20), 1));
        w.construction_tick(&mut Tick::default());
        for i in 10..=11 {
            for j in 20..=22 {
                w.island.owners.set((i, j), 1);
            }
        }
        w.houses[0].set_beach_life(1);
        w.houses[0].set_beach_life_square(1, (30, 30));
        w.island.buildings.set((30, 30), 36);
        assert_eq!(w.demolish(2, (11, 22), &mut Tick::default()), Err(NOT_YOURS));
        w.finances.money[0] = 50;
        assert_eq!(w.demolish(1, (11, 22), &mut Tick::default()), Err(NOT_ENOUGH_MONEY));
        assert_eq!(w.houses.len(), 1);
        w.finances.money[0] = 10_000;
        let random = w.random;
        let mut tick = Tick::default();
        assert_eq!(w.demolish(1, (11, 22), &mut tick), Ok(()));
        assert_eq!(w.finances.money[0], 9900);
        assert_eq!(w.book_value(BOOK_OTHER), -100);
        assert_eq!(w.houses, []);
        assert_eq!(w.island.buildings.at((10, 20)), NO_BUILDING);
        assert_eq!(w.island.owners.at((11, 22)), 0);
        // Sand: the corners are one step above the sea.
        assert_eq!(w.island.ground.at((11, 21)), 0);
        // The beach life went too, with its `Random(1)`.
        assert_eq!(w.island.buildings.at((30, 30)), NO_BUILDING);
        let mut r = random;
        r.random(1);
        assert_eq!(w.random, r);
        assert!(tick.redraw.contains(&(11, 22)) && tick.redraw.contains(&(30, 30)));
        assert_eq!(w.demolish(1, (11, 22), &mut tick), Err(NOT_POSSIBLE));
    }

    #[test]
    fn demolish_a_site() {
        let mut w = plot_world();
        w.finances.money[0] = 100;
        put_site(&mut w, site(40, (10, 20), 5));
        assert_eq!(w.demolish(1, (11, 21), &mut Tick::default()), Err(NOT_YOURS));
        w.island.owners.set((11, 21), 1);
        assert_eq!(w.demolish(1, (11, 21), &mut Tick::default()), Ok(()));
        // 2000 × 5 days of 10 back.
        assert_eq!(w.finances.money[0], 1100);
        assert_eq!(w.book_value(BOOK_OTHER), 1000);
        assert_eq!(w.building_sites, []);
        assert_eq!(w.island.buildings.at((11, 22)), NO_BUILDING);
        // A computer player's championship can't be demolished.
        w.island.buildings.set((5, 5), 0x70);
        assert_eq!(w.demolish(0, (5, 5), &mut Tick::default()), Err(SPECTATORS_PROTEST));
    }

    #[test]
    fn demolish_roads() {
        let mut w = plot_world();
        w.finances.money[0] = 100;
        w.road_demolition_cost = 30;
        for i in 40..=62 {
            for j in 40..=62 {
                w.island.heights.set((i, j), 4);
                w.island.forms.set((i, j), 16);
            }
        }
        w.island.ground.set((50, 50), 10);
        w.island.ground.set((50, 51), 10);
        w.island.ground.set((50, 52), 10);
        w.island.owners.set((50, 50), 1);
        let mut tick = Tick::default();
        assert_eq!(w.demolish(1, (50, 51), &mut tick), Err(NOT_YOURS));
        w.island.owners.set((50, 51), 1);
        assert_eq!(w.demolish(1, (50, 51), &mut tick), Ok(()));
        assert_eq!(w.finances.money[0], 70);
        assert_eq!(w.island.ground.at((50, 51)), 1);
        // The ends lost their neighbour: grass pieces with no road next to
        // them.
        assert_eq!(w.island.ground.at((50, 50)), 10 + 0xf);
        assert_eq!(w.island.ground.at((50, 52)), 10 + 0xf);
        assert_eq!(tick.redraw.len(), 9);
        // A road over a river leaves the river.
        w.island.ground.set((60, 60), 0x38);
        w.island.buildings.set((60, 60), 11);
        assert_eq!(w.demolish(0, (60, 60), &mut tick), Ok(()));
        assert_eq!(w.island.ground.at((60, 60)), 0x3d);
        assert_eq!(w.island.buildings.at((60, 60)), NO_BUILDING);
    }

    #[test]
    fn demolish_a_bridge() {
        let mut w = plot_world();
        w.finances.money[0] = 100;
        // A bridge along `i` from 70 to 72, ends 0 and 2, 4 in between.
        for (i, part) in [(70, 0), (71, 4), (72, 2)] {
            w.island.ground.set((i, 70), 0x5f);
            w.island.buildings.set((i, 70), 52);
            w.island.parts.set((i, 70), part);
        }
        w.island.ground.set((73, 70), 0x5f);
        w.island.parts.set((73, 70), 0);
        w.island.owners.set((71, 70), 1);
        let mut tick = Tick::default();
        assert_eq!(w.demolish(1, (71, 70), &mut tick), Ok(()));
        for i in 70..=73 {
            assert_eq!(w.island.ground.at((i, 70)), 2, "{i}");
            assert_eq!(w.island.buildings.at((i, 70)), NO_BUILDING, "{i}");
        }
        // For nothing.
        assert_eq!(w.finances.money[0], 100);
        assert!(tick.redraw.contains(&(74, 71)));
    }

    #[test]
    fn slices() {
        let mut w = plot_world();
        let mut h = House::new((10, 20));
        h.set_building(40);
        h.set_guests(250);
        w.houses.push(h);
        let mut h = House::new((11, 20));
        h.set_building(40);
        w.houses.push(h);
        // Type 0 has no capacity: no random number.
        w.houses.push(House::new((30, 0)));
        let mut f = Facility::new(41, (5, 6));
        f.set_fill(100);
        w.facilities.push(f);
        w.building_slice(0, &mut Tick::default());
        let mut r = Random::new(3);
        r.random(100);
        r.random(100);
        assert_eq!(w.random, r);
        w.building_slice(1, &mut Tick::default());
        r.random(100);
        assert_eq!(w.random, r);
    }

    #[test]
    fn origin() {
        let mut w = plot_world();
        w.island.buildings.set((12, 22), 40);
        w.island.parts.set((12, 22), 6 + 5);
        assert_eq!(w.building_origin((12, 22)), (11, 20));
    }

    #[test]
    fn find() {
        let mut w = world();
        put_site(&mut w, site(40, (10, 20), 5));
        assert_eq!(w.find_building_site((11, 22)), Some(0));
        assert_eq!(w.find_building_site((12, 22)), None);
        w.island.buildings.set((12, 22), BUILDING_SITE);
        assert_eq!(w.find_building_site((12, 22)), None);
    }

    /// A 2 × 2 type 50 on any land, flat, for 1000, on a plot of flat
    /// sand at height 1.
    fn build_world() -> World {
        let mut w = plot_world();
        let mut t = (*w.tables).clone();
        t.hausnr[50] = [1, 0, 0, 2, 2, 4, 0, 0];
        t.hausnr[usize::from(BUILDING_SITE)] = [3, 0, 0, 1, 1, 1, 0, 0];
        t.hausdata[50] = [6, 0, 0, 80, 0, 9, 0, 2, 0, 100];
        t.hausdata[usize::from(BUILDING_SITE)][9] = 100;
        t.hausprei[50] = [1000, 10, 0, 0];
        w.tables = Arc::new(t);
        w.player_count = 3;
        w.foundation_cost = 7;
        for i in 0..100 {
            for j in 0..100 {
                w.island.heights.set((i, j), 1);
                w.island.forms.set((i, j), 16);
            }
        }
        w
    }

    #[test]
    fn can_build() {
        let mut w = build_world();
        assert_eq!(w.can_build((30, 30), 50, 2), (0, 1000));
        assert_eq!(w.can_build((-1, 30), 50, 2).0, OFF_THE_MAP);
        assert_eq!(w.can_build((250, 30), 50, 2).0, OFF_THE_MAP);
        // Only the human needs the money.
        assert_eq!(w.can_build((30, 30), 50, 1), (NOT_ENOUGH_MONEY, 1000));
        w.finances.money[0] = 5000;
        assert_eq!(w.can_build((30, 30), 50, 1), (0, 1000));
        w.island.ground.set((31, 31), 12);
        assert_eq!(w.can_build((30, 30), 50, 2).0, 2);
        w.island.ground.set((31, 31), 56);
        assert_eq!(w.can_build((30, 30), 50, 2).0, 0xf);
        w.island.ground.set((31, 31), 2);
        assert_eq!(w.can_build((30, 30), 50, 2).0, 0xb);
        w.island.ground.set((31, 31), 0);
        w.island.buildings.set((30, 31), 41);
        assert_eq!(w.can_build((30, 30), 50, 2).0, 1);
        // A palm is in nobody's way.
        w.island.buildings.set((30, 31), PALM);
        assert_eq!(w.can_build((30, 30), 50, 2).0, 0);
        w.island.forms.set((31, 30), 3);
        assert_eq!(w.can_build((30, 30), 50, 2).0, 0x14);
        w.island.forms.set((31, 30), 16);
        // A raised corner: the square isn't level, which costs the
        // foundation, and its highest corner isn't the first square's.
        w.island.heights.set((31, 31), 2);
        assert_eq!(w.can_build((30, 30), 50, 2), (0xc, 1007));
        // From (31, 31) all four squares have the raised corner: their
        // highest corners agree, and each pays the foundation.
        assert_eq!(w.can_build((31, 31), 50, 2), (0, 1028));
    }

    #[test]
    fn start_a_building_site() {
        let mut w = build_world();
        w.finances.money = [0, 5000, 0, 0, 0, 0, 0, 0];
        w.island.buildings.set((31, 30), PALM);
        w.plots[0].set_palms(3);
        let mut tick = Tick::default();
        w.start_building(1000, 4, 2, 50, (30, 30), &mut tick);
        for pos in [(30, 30), (30, 31), (31, 30), (31, 31)] {
            assert_eq!(w.island.buildings.at(pos), BUILDING_SITE);
            assert_eq!(w.island.owners.at(pos), 2);
            assert!(w.island.parts.at(pos) < 3);
        }
        assert_eq!(w.plots[0].palms(), 2);
        assert_eq!(tick.redraw.len(), 4);
        assert_eq!(
            w.building_sites[0],
            BuildingSite {
                byte_0: 0,
                pos: (30, 30),
                days: 6,
                building: 50,
                budget: 4,
            }
        );
        // The computer's budget gets the capacity; difficulty 0 gives
        // nothing back.
        assert_eq!(w.computer_budgets[4], 80);
        assert_eq!(w.finances.money[1], 4000);
        // Finished, the budget is back.
        for _ in 0..6 {
            w.construction_tick(&mut tick);
        }
        assert_eq!(w.building_sites.len(), 0);
        assert_eq!(w.computer_budgets[4], 0);
        assert_eq!(w.island.buildings.at((30, 30)), 50);
        // The human's books and the money rising.
        w.finances.money[0] = 5000;
        w.start_building(1200, 0, 1, 50, (40, 40), &mut tick);
        assert_eq!(w.book_value(BOOK_NEW_BUILDINGS), -1000);
        assert!(tick.books);
        assert_eq!(w.finances.money[0], 3800);
    }
}
