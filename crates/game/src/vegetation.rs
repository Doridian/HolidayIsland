//! The palms (building type [`PALM`]), which grow, die and spread on their
//! own (segment `1010`).
//!
//! A palm square's part is `variant * stages + stage`, where `stages` is
//! the palm's `HAUSNR.DAT` part count (10) and `variant` is the ground
//! tile's variant the square had before. Stage 8 is a young palm, 9 a
//! grown one and 0–7 the eight looks of an old one.

use crate::simulation::Tick;
use crate::world::{NO_BUILDING, PALM, Pos, World};

/// The kinds of ground palms grow on: sand, grass and sand to grass.
/// // 1110:13b8 (`set of Byte`)
pub const PALM_GROUND: [u8; 3] = [0, 1, 4];

/// The young palm's stage.
const YOUNG: i16 = 8;
/// The grown palm's stage.
const GROWN: i16 = 9;

impl World {
    /// The number of a palm's stages: its `HAUSNR.DAT` part count.
    /// // hausnr+0xcc
    fn palm_stages(&self) -> i16 {
        self.tables.hausnr(PALM, 6)
    }

    /// Looks at 30 random squares (`Random(250)` for `i`, then `j`): a
    /// young palm grows, a grown one gets one of the old looks
    /// (`Random(8)`), an old one dies with a chance of 60 % (`Random(100)`)
    /// and leaves the ground as it was. An empty square of [`PALM_GROUND`]
    /// gets a new palm with a chance of 30 %. // 1010:129e
    pub(crate) fn vegetation_tick(&mut self, tick: &mut Tick) {
        for _ in 1..=30 {
            let i = self.random.random(250).cast_signed();
            let j = self.random.random(250).cast_signed();
            let pos = (i, j);
            let building = self.island.buildings.at(pos);
            if building == PALM {
                let part = self.island.parts.at(pos);
                let stages = self.palm_stages();
                // Without HAUSNR.DAT the original divides by zero.
                let stage = i16::from(part).checked_rem(stages).unwrap_or(-1);
                if stage == YOUNG {
                    self.island.parts.set(pos, part.wrapping_add(1));
                } else if stage == GROWN {
                    let look = self.random.random(8);
                    let part = u16::from(part).wrapping_sub(9).wrapping_add(look);
                    self.island.parts.set(pos, part.to_le_bytes()[0]);
                } else if self.random.random(100) < 60 {
                    let variant = i16::from(part).checked_div(stages).unwrap_or(0);
                    self.island.parts.set(pos, variant.to_le_bytes()[0]);
                    self.island.buildings.set(pos, NO_BUILDING);
                    self.remove_location_effect(pos, PALM); // 1030:1dc6
                }
                tick.redraw.push(pos); // 1050:21f9
            } else if building == NO_BUILDING
                && PALM_GROUND.contains(&self.island.ground.at(pos))
                && self.random.random(100) < 30
            {
                self.plant_palm(pos, tick);
            }
        }
    }

    /// Plants a young palm on `pos`: counts it on the square's plot, puts
    /// its effect on the location map and keeps the ground's variant in
    /// the part. // 1010:1129
    ///
    /// The original also redraws the square on the overview map
    /// (10b8:1624), which the port's view doesn't have yet.
    pub fn plant_palm(&mut self, pos: Pos, tick: &mut Tick) {
        let plot = self.island.plot_at(pos); // 1030:1e62
        // 1068:2591 finds the plot by its number. The original writes
        // through a nil pointer for a square without a plot, which the CD's
        // maps don't have on land.
        if let Some(p) = self
            .plots
            .iter_mut()
            .find(|p| p.id() >= 0 && p.id().cast_unsigned() == plot)
        {
            p.set_palms(p.palms().wrapping_add(1));
        }
        self.island.buildings.set(pos, PALM);
        self.add_location_effect(pos, PALM); // 1030:1d10
        let part = u16::from(self.island.parts.at(pos))
            .wrapping_mul(self.palm_stages().cast_unsigned())
            .wrapping_add(YOUNG.cast_unsigned());
        self.island.parts.set(pos, part.to_le_bytes()[0]);
        tick.redraw.push(pos); // 1050:21f9
    }
}

#[cfg(test)]
mod tests {
    use vcl::random::Random;

    use super::*;
    use crate::world::{Layer, Plot, Tables};

    /// A world whose every square holds `building` with `part`, with the
    /// palm's rows of the tables as on the CD and one plot everywhere.
    fn world(building: u8, part: u8) -> World {
        let mut tables = Tables {
            hausnr: vec![[0; 8]; 13],
            hausdata: vec![[0; 10]; 13],
            hausprei: Vec::new(),
        };
        tables.hausnr[usize::from(PALM)] = [10, 12, 0, 1, 1, 1, 10, 0];
        tables.hausdata[usize::from(PALM)][9] = 106;
        let mut w = World {
            tables: tables.into(),
            max_location_effect: 30,
            random: Random::new(1234),
            plots: vec![Plot::new(-1), Plot::new(1)],
            ..World::default()
        };
        w.island.buildings = Layer::filled(building);
        w.island.parts = Layer::filled(part);
        for i in 0..251 {
            for j in 0..251 {
                w.island.plot_map.set((i, j), 1);
            }
        }
        w
    }

    /// The squares the next tick looks at, and the `Random(100)` after
    /// each when `chance` is set.
    fn squares(seed: u32, chance: bool) -> Vec<(Pos, u16)> {
        let mut r = Random::new(seed);
        (0..30)
            .map(|_| {
                let pos = (r.random(250).cast_signed(), r.random(250).cast_signed());
                (pos, if chance { r.random(100) } else { 0 })
            })
            .collect()
    }

    #[test]
    fn young_palms_grow() {
        let mut w = world(PALM, 3 * 10 + 8);
        let mut tick = Tick::default();
        w.vegetation_tick(&mut tick);
        let seen = squares(1234, false);
        // Two `Random(250)` per square and nothing else, unless a square
        // came up twice.
        let mut r = Random::new(1234);
        for _ in 0..60 {
            r.random(1);
        }
        assert_eq!(w.random, r);
        for (pos, _) in &seen {
            assert_eq!(w.island.parts.at(*pos), 3 * 10 + 9);
        }
        assert_eq!(tick.redraw, seen.iter().map(|s| s.0).collect::<Vec<_>>());
        assert_eq!(w.island.parts.at((250, 250)), 3 * 10 + 8);
    }

    #[test]
    fn old_palms_die() {
        let mut w = world(PALM, 3 * 10 + 2);
        let mut tick = Tick::default();
        w.vegetation_tick(&mut tick);
        let mut died = 0;
        for (pos, chance) in squares(1234, true) {
            if chance < 60 {
                assert_eq!(w.island.buildings.at(pos), NO_BUILDING);
                assert_eq!(w.island.parts.at(pos), 3);
                died += 1;
            } else {
                assert_eq!(w.island.buildings.at(pos), PALM);
                assert_eq!(w.island.parts.at(pos), 3 * 10 + 2);
            }
        }
        assert!(died > 0);
        assert_eq!(tick.redraw.len(), 30);
        // Each took its effect off the location map.
        let mut below = 0;
        for a in 0..126 {
            for b in 0..126 {
                below += usize::from(w.island.location.get((a, b)) < Some(100));
            }
        }
        assert!(below >= died, "{below} cells for {died} palms");
    }

    #[test]
    fn grown_palms_get_old() {
        let mut w = world(PALM, 3 * 10 + 9);
        w.vegetation_tick(&mut Tick::default());
        let mut r = Random::new(1234);
        for _ in 0..30 {
            let pos = (r.random(250).cast_signed(), r.random(250).cast_signed());
            let look = r.random(8);
            assert_eq!(u16::from(w.island.parts.at(pos)), 3 * 10 + look);
        }
    }

    #[test]
    fn palms_spread() {
        let mut w = world(NO_BUILDING, 2);
        // Water doesn't grow palms.
        w.island.ground = Layer::filled(2);
        let mut tick = Tick::default();
        w.vegetation_tick(&mut tick);
        assert_eq!(tick.redraw, []);
        let mut r = Random::new(1234);
        for _ in 0..60 {
            r.random(1);
        }
        assert_eq!(w.random, r);

        let mut w = world(NO_BUILDING, 2);
        w.island.ground = Layer::filled(4);
        let mut tick = Tick::default();
        w.vegetation_tick(&mut tick);
        let planted: Vec<Pos> = squares(1234, true)
            .into_iter()
            .filter(|&(_, chance)| chance < 30)
            .map(|(pos, _)| pos)
            .collect();
        assert_ne!(planted, []);
        assert_eq!(tick.redraw, planted);
        for &pos in &planted {
            assert_eq!(w.island.buildings.at(pos), PALM);
            assert_eq!(w.island.parts.at(pos), 2 * 10 + 8);
            assert!(w.island.location.get((pos.0 / 2, pos.1 / 2)) > Some(100));
        }
        // Counted on the plot with the number, not the one without.
        assert_eq!(w.plots[0].palms(), 0);
        assert_eq!(usize::try_from(w.plots[1].palms()).unwrap(), planted.len());
    }

    #[test]
    fn plot_lookup() {
        let mut w = world(NO_BUILDING, 0);
        w.island.plot_map = crate::world::PlotMap::empty();
        w.island.plot_map.set((10, 20), 5);
        w.island.forms = Layer::filled(16);
        assert_eq!(w.island.plot_at((10, 20)), 5);
        assert_eq!(w.island.plot_at((11, 20)), 5);
        assert_eq!(w.island.plot_at((12, 20)), 0);
        // A square without a plot looks at its neighbour by its form.
        w.island.forms.set((12, 20), 0);
        assert_eq!(w.island.plot_at((12, 20)), 5);
        w.island.forms.set((10, 19), 1);
        assert_eq!(w.island.plot_at((10, 19)), 5);
        w.island.forms.set((10, 21), 3);
        assert_eq!(w.island.plot_at((10, 21)), 5);
        assert_eq!(w.island.plot_at((-1, 20)), 0);
        assert_eq!(w.island.plot_at((10, 251)), 0);
    }
}
