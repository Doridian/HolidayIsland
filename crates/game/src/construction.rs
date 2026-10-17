//! The building sites (`*1110:6f08`): buildings under construction, which
//! count down a day at a time and then become the building (segment
//! `1078`, and the accounting's 1008:5212).

use crate::simulation::Tick;
use crate::world::{Pos, World};

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
        self.remove_workers(site.building, site.pos);
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

/// The parts of finishing a building that aren't ported yet.
#[allow(clippy::unused_self)]
impl World {
    /// Registers a finished building: adds to its plot's counts (`+0x354`
    /// palms for kind 6, else `+0x356`, by `HAUSNR.DAT` column 5), makes
    /// its record in the houses (1068:1a08) or the facilities (1068:1b3e)
    /// by `1110:1341[kind]`, adds its cost to the plot's and the player's
    /// sums (`*1110:6f20`, by 1010:2493's class) and puts its effect on the
    /// location map (1030:1d10). // 1078:0eda, 884 bytes
    fn register_building(&mut self, building: u8, pos: Pos) {
        // TODO: port 1078:0eda once `*1110:6f20` and the plots' lists of
        // buildings (`+0x358`, `+0x35c`, 1068:1d05, 1068:1d55) are world
        // state. Until then a finished building has no record and no
        // effect on the location map.
        let _ = (building, pos);
    }

    /// Sends the people of kind 0x26 (the builders?) in the building's
    /// area away (1088:2df2). // 1088:6bac, 174 bytes
    fn remove_workers(&mut self, building: u8, pos: Pos) {
        // TODO: port 1088:6bac with the people (`*1110:6fd6`).
        let _ = (building, pos);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use vcl::random::Random;

    use super::*;
    use crate::world::{BuildingSite, Tables};

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

    #[test]
    fn find() {
        let mut w = world();
        put_site(&mut w, site(40, (10, 20), 5));
        assert_eq!(w.find_building_site((11, 22)), Some(0));
        assert_eq!(w.find_building_site((12, 22)), None);
        w.island.buildings.set((12, 22), BUILDING_SITE);
        assert_eq!(w.find_building_site((12, 22)), None);
    }
}
