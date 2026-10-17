//! The location quality map (`Lage`, `*1110:5524`, segment `1030`): how
//! good a place is to build on, which buildings and palms raise or lower
//! around themselves, and sabotage and disasters for a while.

use crate::world::{Pos, World};

/// The last cell of the location map on each axis, `LOCATION_SIZE - 1`.
/// // 1030:185d
const LAST: i32 = 0x7d;

impl World {
    /// Puts `building`'s effect on the location map around the building
    /// at `pos`. // 1030:1d10
    pub fn add_location_effect(&mut self, pos: Pos, building: u8) {
        let (centre, value, radius) = self.location_effect(pos, building);
        self.change_location(centre, value, radius, false); // 1030:1ae9
    }

    /// Takes `building`'s effect around `pos` off the location map again.
    /// // 1030:1dc6
    pub fn remove_location_effect(&mut self, pos: Pos, building: u8) {
        let (centre, value, radius) = self.location_effect(pos, building);
        self.change_location(centre, value, radius, true); // 1030:185d
    }

    /// Where a building's effect is centred, its strength (`HAUSDATA.DAT`'s
    /// last column, 100 for none) and its radius: a cell for every 6 the
    /// strength is away from 100, at least 1 and at most half of
    /// `MaxLageeinfluss`. // 1030:1d10, 1030:1dc6
    pub(crate) fn location_effect(&self, (i, j): Pos, building: u8) -> (Pos, i32, i32) {
        let t = &self.tables;
        let centre = (
            i.wrapping_add(t.hausnr(building, 3) / 2),
            j.wrapping_add(t.hausnr(building, 4) / 2),
        );
        let value = t.hausdata(building, 9);
        let radius = i32::from(value.wrapping_sub(100).wrapping_abs() / 6)
            .min(i32::from(self.max_location_effect >> 1))
            .max(1);
        (centre, i32::from(value), radius)
    }

    /// Multiplies the location map around the square `centre` by
    /// `value` percent, fading out linearly (in steps of the Manhattan
    /// distance) to nothing at `radius + 1` cells away; with `remove`, divides
    /// by the same factor instead, which undoes it up to rounding. The cells
    /// stay within 2..=255. A `value` of 100 does nothing. // 1030:1ae9
    ///
    /// 1030:185d is the same and then redraws the area of the overview map
    /// (10b8:15c4), which the port's view doesn't have yet.
    pub fn change_location(&mut self, (i, j): Pos, value: i32, radius: i32, remove: bool) {
        if value == 100 {
            return;
        }
        let (ci, cj) = (i32::from(i / 2), i32::from(j / 2));
        for a in (ci - radius).max(0)..=(ci + radius).min(LAST) {
            for b in (cj - radius).max(0)..=(cj + radius).min(LAST) {
                let distance = (cj - b).abs() + (ci - a).abs();
                let weight = (radius + 1 - distance).max(0);
                let mut factor = (value - 100) * weight / (radius + 1) + 100;
                if remove {
                    // The original divides by zero for a factor of 0.
                    let Some(inverse) = (factor / 2 + 10000).checked_div(factor) else {
                        continue;
                    };
                    factor = inverse;
                }
                let cell = (a.try_into().unwrap_or(0), b.try_into().unwrap_or(0));
                let old = i32::from(self.island.location.get(cell).unwrap_or(0));
                let new = ((old * factor + 50) / 100).clamp(2, 255);
                self.island.location.set(cell, u8::try_from(new).unwrap_or(u8::MAX));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::Tables;

    #[test]
    fn palm_effect() {
        let mut w = World::default();
        let mut tables = Tables {
            hausnr: vec![[0; 8]; 13],
            hausdata: vec![[0; 10]; 13],
            hausprei: Vec::new(),
            groundnr: Vec::new(),
        };
        tables.hausnr[12][3] = 1;
        tables.hausnr[12][4] = 1;
        tables.hausdata[12][9] = 106;
        w.tables = tables.into();
        w.max_location_effect = 30;
        w.add_location_effect((21, 40), 12);
        let l = &w.island.location;
        // Radius 1: 106 % in the middle, 103 % next to it.
        assert_eq!(l.get((10, 20)), Some(106));
        assert_eq!(l.get((9, 20)), Some(103));
        assert_eq!(l.get((10, 21)), Some(103));
        assert_eq!(l.get((9, 21)), Some(100));
        assert_eq!(l.get((10, 22)), Some(100));
        w.remove_location_effect((21, 40), 12);
        assert_eq!(w.island.location, crate::world::LocationMap::filled(100));
    }

    #[test]
    fn limits() {
        let mut w = World::default();
        // At the map's edge.
        w.change_location((0, 250), 120, 3, false);
        let l = &w.island.location;
        assert_eq!(l.get((0, 125)), Some(120));
        assert_eq!(l.get((3, 125)), Some(105));
        assert_eq!(l.get((2, 124)), Some(105));
        assert_eq!(l.get((4, 125)), Some(100));
        w.change_location((0, 250), 1000, 3, false);
        assert_eq!(w.island.location.get((0, 125)), Some(255));
        w.change_location((0, 250), 0, 3, false);
        assert_eq!(w.island.location.get((0, 125)), Some(2));
    }
}
