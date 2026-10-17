//! Road pieces (segment `1080`): which piece a road square shows follows
//! the roads and bridges next to it. Building roads isn't ported yet;
//! demolishing one ([`World::demolish`]) puts its neighbours' pieces
//! right.

use crate::world::{Pos, World};

/// The road pieces' grounds (`STRASPOS.DAT`): 10–54, and 55–58 where a
/// road crosses a river (with building 11, a bridge, on it).
/// // 1078:280b
pub(crate) const ROADS: std::ops::RangeInclusive<u8> = 10..=0x3a;
/// The road crossing a river: grounds 55–58, which become the river
/// pieces 59, 61, 60 and 62 without the road. // 1078:280b, 1080:232a
pub(crate) const CROSSINGS: [(u8, u8); 4] = [(0x37, 0x3b), (0x38, 0x3d), (0x39, 0x3c), (0x3a, 0x3e)];
/// The grounds of bridges over the sea (building type 52): 95–100.
/// // 1078:280b
pub(crate) const BRIDGES: std::ops::RangeInclusive<u8> = 0x5f..=0x64;

/// The building type of a bridge over a river. // 1080:232a
const RIVER_BRIDGE: u8 = 11;

/// A flat road's piece by which neighbours have a road or bridge: 1 for
/// `(i + 1, j)`, 2 for `(i, j + 1)`, 4 for `(i - 1, j)`, 8 for `(i, j - 1)`.
/// // 1080:232a (its jump table at 1080:230a)
const PIECES: [u8; 16] = [
    10, 11, 10, 0x13, 11, 11, 0x12, 0xd, 10, 0x14, 10, 0xe, 0x11, 0xf, 0xc, 0x10,
];

/// A flat square's form. // 1080:302a
const FLAT: u8 = 16;

impl World {
    /// The ground of `pos`, or 2 (water) for a square on the map's edge or
    /// off it: only 1..=249 count. // 1000:3e0d
    pub(crate) fn ground_or_water(&self, (i, j): Pos) -> u8 {
        if (1..=249).contains(&i) && (1..=249).contains(&j) {
            self.island.ground.at((i, j))
        } else {
            2
        }
    }

    /// The form of `pos`, or [`FLAT`] on the edge or off the map.
    /// // 1000:3e8d
    pub(crate) fn form_or_flat(&self, (i, j): Pos) -> u8 {
        if (1..=249).contains(&i) && (1..=249).contains(&j) {
            self.island.forms.at((i, j))
        } else {
            FLAT
        }
    }

    /// The flat road squares next to `(i, j)` get the piece their
    /// neighbours call for, in the order `(i + 1, j)`, `(i - 1, j)`,
    /// `(i, j + 1)`, `(i, j - 1)`. // 1080:302a
    pub(crate) fn reconnect_roads(&mut self, (i, j): Pos) {
        for pos in [(i + 1, j), (i - 1, j), (i, j + 1), (i, j - 1)] {
            if self.form_or_flat(pos) == FLAT && ROADS.contains(&self.ground_or_water(pos)) {
                let piece = self.road_piece(pos);
                self.island.ground.set(pos, piece);
            }
        }
    }

    /// The piece the road on `pos` (on the map, not on its edge) shows: a
    /// plain road gets its ground back first ([`World::reset_ground`]),
    /// and then the piece for its neighbours from [`PIECES`] in that
    /// ground's set of pieces; a crossing keeps its piece, and a river
    /// piece becomes the crossing with a bridge on it. // 1080:232a
    fn road_piece(&mut self, (i, j): Pos) -> u8 {
        let ground = self.ground_or_water((i, j));
        if ground < 0x37 {
            self.reset_ground((i, j)); // 10b0:3199
            let road = |w: &World, pos| {
                let g = w.ground_or_water(pos);
                ROADS.contains(&g) || BRIDGES.contains(&g)
            };
            let neighbours = [(i + 1, j), (i, j + 1), (i - 1, j), (i, j - 1)];
            let mask = neighbours
                .iter()
                .enumerate()
                .filter(|&(_, &pos)| road(self, pos))
                .fold(0, |mask, (bit, _)| mask | (1 << bit));
            PIECES[mask].wrapping_add(self.road_set((i, j)))
        } else if let Some(n) = CROSSINGS.iter().position(|&(_, river)| river == ground) {
            // 59 → 55 with part 0, 61 → 56 with 1, 60 → 57 with 2, 62 →
            // 58 with 3.
            self.island.buildings.set((i, j), RIVER_BRIDGE);
            self.island.parts.set((i, j), u8::try_from(n).unwrap_or(0));
            CROSSINGS[n].0
        } else {
            ground
        }
    }

    /// Where the road pieces for the ground of `pos` start: 0 on sand, 15
    /// on grass, 0x5f on water and 0x1e on sand to grass. // 1080:22be
    fn road_set(&self, pos: Pos) -> u8 {
        match self.ground_or_water(pos) {
            1 => 0xf,
            2 | 3 => 0x5f,
            4 | 5 => 0x1e,
            _ => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::NO_BUILDING;

    /// Raises the corners of the squares in `0..=n` on both axes to
    /// `height`, so they're dry and flat.
    fn land(w: &mut World, n: i16, height: u8) {
        for i in 0..=n {
            for j in 0..=n {
                w.island.heights.set((i, j), height);
                w.island.forms.set((i, j), FLAT);
            }
        }
    }

    #[test]
    fn pieces_follow_the_neighbours() {
        let mut w = World::default();
        land(&mut w, 20, 4);
        // Grass (the corners are high) with a road from (10, 10) to
        // (10, 12) and a road ending at (11, 10).
        for pos in [(10, 10), (10, 11), (10, 12), (11, 10)] {
            w.island.ground.set(pos, 10);
        }
        w.reconnect_roads((10, 11));
        // (11, 11) isn't a road; (10, 10) has (11, 10) and (10, 11) next
        // to it: 1 + 2.
        assert_eq!(w.island.ground.at((10, 10)), 0x13 + 0xf);
        // (10, 12) has only (10, 11): 8.
        assert_eq!(w.island.ground.at((10, 12)), 10 + 0xf);
        assert_eq!(w.island.ground.at((11, 11)), 0);
        assert_eq!(w.island.buildings.at((10, 10)), NO_BUILDING);
    }

    #[test]
    fn crossings_keep_their_piece() {
        let mut w = World::default();
        land(&mut w, 20, 4);
        w.island.ground.set((5, 6), 0x38);
        w.reconnect_roads((5, 5));
        assert_eq!(w.island.ground.at((5, 6)), 0x38);
        // A river piece next door isn't a road.
        w.island.ground.set((5, 4), 0x3c);
        w.reconnect_roads((5, 5));
        assert_eq!(w.island.ground.at((5, 4)), 0x3c);
        assert_eq!(w.road_piece((5, 4)), 0x39);
        assert_eq!(
            (w.island.buildings.at((5, 4)), w.island.parts.at((5, 4))),
            (RIVER_BRIDGE, 2)
        );
    }
}
