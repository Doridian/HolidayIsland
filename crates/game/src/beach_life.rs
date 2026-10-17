//! Beach life: the people on the beach around the finished buildings
//! (building type [`BEACH_LIFE`]), which come and go with what the
//! building has to offer (segment `1008`, `ABRECHNE`'s unit, and `1010`).

use std::cmp::Ordering;

use crate::simulation::Tick;
use crate::world::{MAX_BEACH_LIFE, NO_BUILDING, Pos, World};

/// The building type of beach life, `Strandleben` in `HAUSNR.DAT`.
pub const BEACH_LIFE: u8 = 36;

/// The class a house's [`House::value_10`](crate::world::House::value_10)
/// falls in, which is how much beach life it should have: 0–2, 3–10,
/// 11–30, 31–60, 61–110, more. // 1008:1b4b
#[must_use]
pub fn beach_life_class(value: i16) -> u8 {
    match value {
        0..=2 => 0,
        3..=10 => 1,
        11..=30 => 2,
        31..=60 => 3,
        61..=110 => 4,
        _ => 5,
    }
}

impl World {
    /// Each house in turn, with one chance in 30 (`Random(30)`), takes a
    /// step towards the beach life its class asks for. // 1008:1b4b
    pub(crate) fn beach_life_tick(&mut self, tick: &mut Tick) {
        for house in 0..self.houses.len() {
            if self.random.random(30) == 0 {
                let class = beach_life_class(self.houses[house].value_10());
                self.beach_life_step(house, class, tick);
            }
        }
    }

    /// One step of house number `house`'s beach life towards `class`: one
    /// more below it, one less above it. At it, `Random(5)`: 0 adds one
    /// (below the most), 1 takes one away (above none), else one of them
    /// (`Random(n)`) changes its look. // 1008:1aea
    fn beach_life_step(&mut self, house: usize, class: u8, tick: &mut Tick) {
        let n = self.houses[house].beach_life();
        match n.cmp(&class) {
            Ordering::Less => self.add_beach_life(house, tick),
            Ordering::Greater => self.remove_beach_life(house, tick),
            Ordering::Equal => match self.random.random(5) {
                0 => {
                    if class < MAX_BEACH_LIFE {
                        self.add_beach_life(house, tick);
                    }
                }
                1 => {
                    if class > 0 {
                        self.remove_beach_life(house, tick);
                    }
                }
                _ => {
                    if class > 0 {
                        self.change_beach_life(house, tick);
                    }
                }
            },
        }
    }

    /// Puts beach life on a free beach square near the house, if there is
    /// one, and remembers it. // 1008:19ea
    fn add_beach_life(&mut self, house: usize, tick: &mut Tick) {
        let h = &self.houses[house];
        let Some(pos) = self.find_beach_square(h.pos()) else {
            return;
        };
        self.place_beach_life(pos, tick); // 1010:11cd
        let h = &mut self.houses[house];
        let n = h.beach_life().wrapping_add(1);
        h.set_beach_life(n);
        h.set_beach_life_square(n, pos);
    }

    /// Clears the house's last beach life square, whatever is on it now.
    /// // 1008:1a52
    fn remove_beach_life(&mut self, house: usize, tick: &mut Tick) {
        let h = &self.houses[house];
        let n = h.beach_life();
        self.clear_square(h.beach_life_square(n), tick); // 1010:122f
        self.houses[house].set_beach_life(n.wrapping_sub(1));
    }

    /// Gives one of the house's beach life squares (`Random(n)`) another
    /// look (`Random` of the type's part count), whatever is on it now.
    /// // 1008:1a81
    fn change_beach_life(&mut self, house: usize, tick: &mut Tick) {
        let h = &self.houses[house];
        let n = self.random.random(u16::from(h.beach_life()));
        // `Random(n) < n <= 5`.
        let pos = h.beach_life_square(u8::try_from(n + 1).unwrap_or(MAX_BEACH_LIFE));
        let part = self.random_beach_life_look();
        self.island.parts.set(pos, part);
        tick.redraw.push(pos); // 1050:21f9
    }

    /// One of beach life's looks: `Random` of its `HAUSNR.DAT` part count
    /// (`hausnr+0x240`, the first column), as a part.
    fn random_beach_life_look(&mut self) -> u8 {
        let looks = self.tables.hausnr(BEACH_LIFE, 0).cast_unsigned();
        self.random.random(looks).to_le_bytes()[0]
    }

    /// Puts beach life with a random look on `pos`. The original passes the
    /// house's owner too, and doesn't use it. // 1010:11cd
    ///
    /// The original also redraws the square on the overview map
    /// (10b8:1624).
    pub fn place_beach_life(&mut self, pos: Pos, tick: &mut Tick) {
        self.island.buildings.set(pos, BEACH_LIFE);
        let part = self.random_beach_life_look();
        self.island.parts.set(pos, part);
        tick.redraw.push(pos); // 1050:21f9
    }

    /// Empties `pos`: no building, part `Random(1)` (always 0), no owner.
    /// // 1010:122f
    ///
    /// The original also redraws the square on the overview map
    /// (10b8:1564).
    pub fn clear_square(&mut self, pos: Pos, tick: &mut Tick) {
        self.island.buildings.set(pos, NO_BUILDING);
        let part = self.random.random(1).to_le_bytes()[0];
        self.island.parts.set(pos, part);
        self.island.owners.set(pos, 0);
        tick.redraw.push(pos); // 1050:21f9
    }

    /// The first free beach square in the rings around the building at
    /// `(i, j)`, up to 6 squares away: for each ring outwards, first down
    /// the columns left and right of the building (`j - r` then `j_end + r`
    /// for each `i` from `i - r` to `i_end + r`), then along the rows above
    /// and below (`i - r` then `i_end + r` for each `j` in between).
    /// // 1008:1848
    fn find_beach_square(&self, (i, j): Pos) -> Option<Pos> {
        let building = self.island.buildings.get((i, j)).unwrap_or(NO_BUILDING);
        let i_end = self.tables.hausnr(building, 3).wrapping_add(i).wrapping_sub(1);
        let j_end = self.tables.hausnr(building, 4).wrapping_add(j).wrapping_sub(1);
        let plot = self.island.plot_at((i, j)); // 1030:1e62
        let free = |pos| self.is_beach_square(pos, plot).then_some(pos);
        for r in 1..=6 {
            for ii in i - r..=i_end + r {
                if let Some(pos) = free((ii, j - r)).or_else(|| free((ii, j_end + r))) {
                    return Some(pos);
                }
            }
            for jj in (j - r + 1)..(j_end + r) {
                if let Some(pos) = free((i - r, jj)).or_else(|| free((i_end + r, jj))) {
                    return Some(pos);
                }
            }
        }
        None
    }

    /// Whether `pos` can take beach life for a house on `plot`: on the map,
    /// on that plot, flat sand without building or owner, one step above
    /// the sea. // 1008:179f
    fn is_beach_square(&self, pos: Pos, plot: u16) -> bool {
        let island = &self.island;
        island.heights.get(pos).is_some() // 1030:1f93
            && island.plot_at(pos) == plot
            && island.ground.at(pos) == 0
            && island.forms.at(pos) == 16
            && island.buildings.at(pos) == NO_BUILDING
            && island.owners.at(pos) == 0
            && island.heights.at(pos) == 1
    }
}

#[cfg(test)]
mod tests {
    use vcl::random::Random;

    use super::*;
    use crate::world::{House, Layer, Tables};

    /// A beach: flat sand one step above the sea, all on plot 1, with a
    /// 2 × 3 building of type 1 at (100, 100) and its house.
    fn world() -> World {
        let mut tables = Tables {
            hausnr: vec![[0; 8]; 37],
            hausdata: vec![[0; 10]; 37],
            hausprei: Vec::new(),
            groundnr: Vec::new(),
            ..Tables::default()
        };
        tables.hausnr[1][3] = 2;
        tables.hausnr[1][4] = 3;
        tables.hausnr[usize::from(BEACH_LIFE)][0] = 10;
        let mut w = World {
            tables: tables.into(),
            random: Random::new(42),
            houses: vec![House::new((100, 100))],
            ..World::default()
        };
        let island = &mut w.island;
        island.heights = Layer::filled(1);
        island.forms = Layer::filled(16);
        for i in 0..251 {
            for j in 0..251 {
                island.plot_map.set((i, j), 1);
            }
        }
        for (i, j) in [(100, 100), (100, 101), (100, 102), (101, 100), (101, 101), (101, 102)] {
            island.buildings.set((i, j), 1);
        }
        w
    }

    #[test]
    fn classes() {
        let classes: Vec<u8> = [-1, 0, 2, 3, 10, 11, 30, 31, 60, 61, 110, 111]
            .into_iter()
            .map(beach_life_class)
            .collect();
        assert_eq!(classes, [5, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5]);
    }

    #[test]
    fn rings() {
        let mut w = world();
        // The first ring's first square: left of the building's top row.
        assert_eq!(w.find_beach_square((100, 100)), Some((99, 99)));
        w.island.ground.set((99, 99), 1);
        assert_eq!(w.find_beach_square((100, 100)), Some((99, 103)));
        // Down the columns before the rows.
        for i in 99..=102 {
            w.island.owners.set((i, 99), 2);
            w.island.heights.set((i, 103), 2);
        }
        assert_eq!(w.find_beach_square((100, 100)), Some((99, 100)));
        // Another plot, a slope or a building don't do.
        for j in 100..=102 {
            w.island.plot_map.set((99, j), 2);
            w.island.forms.set((102, j), 3);
        }
        w.island.buildings.set((98, 98), 7);
        assert_eq!(w.find_beach_square((100, 100)), Some((98, 104)));
    }

    #[test]
    fn grows_to_its_class_and_back() {
        let mut w = world();
        w.houses[0].set_value_10(200);
        let mut tick = Tick::default();
        for _ in 0..5 {
            w.beach_life_step(0, 5, &mut tick);
        }
        let h = &w.houses[0];
        assert_eq!(h.beach_life(), 5);
        let squares: Vec<Pos> = (1..=5).map(|n| h.beach_life_square(n)).collect();
        assert_eq!(tick.redraw, squares);
        for &pos in &squares {
            assert_eq!(w.island.buildings.at(pos), BEACH_LIFE);
            assert!(w.island.parts.at(pos) < 10);
        }
        // Each one is in a different place.
        let mut sorted = squares.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 5);

        let mut tick = Tick::default();
        w.beach_life_step(0, 3, &mut tick);
        w.beach_life_step(0, 3, &mut tick);
        assert_eq!(w.houses[0].beach_life(), 3);
        // The last ones go first.
        assert_eq!(tick.redraw, [squares[4], squares[3]]);
        for &pos in &squares[3..] {
            assert_eq!(w.island.buildings.at(pos), NO_BUILDING);
            assert_eq!(w.island.parts.at(pos), 0);
        }
    }

    #[test]
    fn at_its_class() {
        let mut w = world();
        w.houses[0].set_value_10(20);
        let mut tick = Tick::default();
        w.beach_life_step(0, 2, &mut tick);
        w.beach_life_step(0, 2, &mut tick);
        // Then it stays about there.
        for _ in 0..200 {
            let before = w.houses[0].beach_life();
            let mut r = w.random;
            let choice = r.random(5);
            w.beach_life_step(0, 2, &mut tick);
            let after = w.houses[0].beach_life();
            match choice {
                0 => assert_eq!(after, before + 1),
                1 => assert_eq!(after, before - 1),
                _ => assert_eq!(after, before),
            }
            // Away from the class, it heads back.
            while w.houses[0].beach_life() != 2 {
                w.beach_life_step(0, 2, &mut tick);
            }
        }
    }

    #[test]
    fn tick_odds() {
        let mut w = world();
        w.houses = (0..300).map(|n| House::new((n % 250, 10))).collect();
        for h in &mut w.houses {
            h.set_value_10(1);
        }
        // Class 0 with no beach life: a chosen house draws `Random(5)`
        // after its `Random(30)`, and on a 0 adds beach life with a random
        // look.
        let mut r = Random::new(42);
        for _ in 0..300 {
            if r.random(30) == 0 && r.random(5) == 0 {
                r.random(10);
            }
        }
        w.beach_life_tick(&mut Tick::default());
        assert_eq!(w.random, r);
    }
}
