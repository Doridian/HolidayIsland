//! The simulation's clock: the world's part of `THauptschirm`'s
//! `AnimationsTimer` (1020:1e9b), which runs every 100 ms while the main
//! screen is up. `docs/simulation.md` has the whole routine, the view's
//! parts included, and what each subsystem does.
//!
//! The subsystems that aren't ported yet are stubs here, named after what
//! they do, with their address. Each one that draws random numbers in the
//! original must draw them from [`World::random`] in the same order once
//! it's ported.

use crate::world::World;

/// How often the original's timer fires.
pub const TICK_MS: u32 = 100; // THauptschirm.AnimationsTimer.Interval

/// The counters that make up game time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Clock {
    /// Ticks into the current "hour", a sixth of a day. // THauptschirm+0x1d6
    pub hour_ticks: u16,
    /// Ticks into the current day; the first five run one subsystem each.
    /// // THauptschirm+0x1d8
    pub day_ticks: u16,
    /// Ticks per hour: the game speed, `100 -` the options' slider
    /// (1060:0619). // 1110:0096
    pub speed: i32,
    /// Which of 30 groups of buildings [`World::building_slice`] does next.
    /// // 1110:009e
    pub slice: i16,
    /// The game is paused while the TV shows a message (1048:12e9 sets it,
    /// 1048:077a clears it). // 1110:0067
    pub paused: bool,
}

impl Default for Clock {
    /// // 1000:1ea0
    fn default() -> Self {
        Clock {
            hour_ticks: 0,
            day_ticks: 0,
            speed: 4,
            slice: 0,
            paused: false,
        }
    }
}

/// The animation options. They change the world (people only move with
/// them on), although they're the player's settings in the original.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Options {
    /// People and cars are animated at all. // 1110:0054
    pub animations: bool,
    /// They walk along their paths. // 1110:0055
    pub people_move: bool,
    /// New ones come. // 1110:0056
    pub people_spawn: bool,
}

impl Default for Options {
    /// // 1000:1ea0
    fn default() -> Self {
        Options {
            animations: true,
            people_move: true,
            people_spawn: true,
        }
    }
}

/// The schedule of the monthly settlement ([`World::accounting_tick`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Accounting {
    /// The settlement's step: 0 on the 1st of the month, then five a day.
    /// // 1110:4a84
    pub step: i32,
    /// The day of the month the last accounting tick saw. // 1110:4a88
    pub last_day: u16,
}

/// What a tick did that the view shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Tick {
    /// The date moved on: the original redraws it (1030:228d) and starts
    /// the next music track if the last has ended (1018:367e).
    pub new_day: bool,
}

impl World {
    /// One tick of `AnimationsTimer`, the world's steps in the original's
    /// order. // 1020:1e9b
    ///
    /// Left to the view: re-enabling the forms (1010:2a0c), the status bar
    /// message's timeout and the tooltips (before the clock), scrolling
    /// while a scroll arrow is held, shaking the screen in an earthquake,
    /// the TV (1048:1b3e) and drawing the people.
    pub fn tick(&mut self) -> Tick {
        let mut tick = Tick::default();
        if !self.clock.paused {
            let c = &mut self.clock;
            c.hour_ticks = c.hour_ticks.wrapping_add(1);
            c.day_ticks = c.day_ticks.wrapping_add(1);
            match c.day_ticks {
                1 => self.events_tick(),
                2 => self.effects_tick(),
                3 | 5 => self.vegetation_tick(),
                4 => self.ratings_tick(),
                _ => {}
            }
            // Compared as Longints: a speed of 0 or less ends every tick.
            if i32::from(self.clock.day_ticks) >= self.clock.speed.wrapping_mul(6) {
                self.clock.day_ticks = 0;
                self.date.advance(); // 1030:00eb
                tick.new_day = true;
            }
            if i32::from(self.clock.hour_ticks) >= self.clock.speed {
                self.clock.hour_ticks = 0;
                self.accounting_tick();
            }
        }
        self.clock.slice = (self.clock.slice.wrapping_add(1)) % 30;
        self.building_slice(self.clock.slice);
        // The original shakes the screen here during an earthquake, with
        // two `Random(7)`s; the port leaves that to the view, so its random
        // numbers part from the original's while the earth shakes.
        if self.earthquake && self.clock.hour_ticks == 1 {
            self.quake_damage();
        }
        if self.options.animations {
            self.people_tick();
            if self.options.people_spawn {
                self.spawn_people();
            }
        }
        tick
    }

    /// The monthly settlement's scheduler. // 1008:5307
    pub fn accounting_tick(&mut self) {
        let day = u16::from(self.date.day);
        if day != self.accounting.last_day {
            self.construction_tick();
            self.accounting.last_day = day;
            return;
        }
        let a = &mut self.accounting;
        a.step = a.step.wrapping_add(1);
        if a.step > 12 && self.date.day == 1 {
            a.step = 0;
            if self.date.month == 1 {
                self.year_end();
            }
        }
        if self.date.month == 1 && self.accounting.step == 5 {
            self.january_second();
        }
        self.settlement_step(self.accounting.step);
    }
}

/// The subsystems still to port. The stubs keep the original's order of
/// calls; most don't touch the world yet.
#[allow(clippy::unused_self)]
impl World {
    /// The scenario's events: counts [`World::days`], starts due events
    /// (with a `Random(65000)` chance each, 1000:3d9e), shows their
    /// messages on the TV (1048:0a54), applies their effects (1028:08ac)
    /// and ends them. // 1010:3084, 809 bytes
    fn events_tick(&mut self) {
        // TODO: port 1010:3084 once the events' records (`*1110:6536`)
        // are mapped. Until then only the day counter runs.
        self.days = self.days.wrapping_add(1);
    }

    /// Counts down sabotage and disasters (`*1110:705a`, `*1110:705e`)
    /// and applies their effects. // 10a0:117e, 1495 bytes
    fn effects_tick(&mut self) {
        // TODO: port 10a0:117e.
    }

    /// Palms grow, seed and die on 30 random squares (`Random(250)`).
    /// // 1010:129e, 404 bytes
    fn vegetation_tick(&mut self) {
        // TODO: port 1010:129e with 1010:1129 (plant a palm), 1030:1dc6 and
        // 1030:1d10 (the location map).
    }

    /// Buildings' ratings drift towards their class, one chance in 30
    /// each. // 1008:1b4b, 172 bytes
    fn ratings_tick(&mut self) {
        // TODO: port 1008:1b4b and 1008:1aea once the building list
        // (`1110:6f10`) is loaded.
    }

    /// Building sites count down and finish (1078:203d). // 1008:5212,
    /// 245 bytes
    fn construction_tick(&mut self) {
        // TODO: port 1008:5212 once the building site list (`1110:6f08`) is
        // loaded.
    }

    /// The new year. // 1008:4f64, 686 bytes
    fn year_end(&mut self) {
        // TODO: port 1008:4f64.
    }

    /// Something on the 2nd of January. // 1008:0512, 199 bytes
    fn january_second(&mut self) {
        // TODO: port 1008:0512.
    }

    /// One step of the monthly settlement. // 1008:1bf7, 13165 bytes
    fn settlement_step(&mut self, step: i32) {
        // TODO: port 1008:1bf7, step by step.
        let _ = step;
    }

    /// The buildings whose `(x + y) mod 30` is `slice` may send out a
    /// person (1090:1a25, `Random(100)`). // 1078:0002, 330 bytes
    fn building_slice(&mut self, slice: i16) {
        // TODO: port 1078:0002.
        let _ = slice;
    }

    /// An earthquake's damage, once an hour. // 10a0:0d1c, 192 bytes
    fn quake_damage(&mut self) {
        // TODO: port 10a0:0d1c.
    }

    /// The people and cars move (`*1110:6fd6`). // 1090:5293, about
    /// 3850 bytes, and 1090:2ce5, 9498 bytes
    fn people_tick(&mut self) {
        // TODO: port 1090:5293.
    }

    /// New people and cars come to the island. // 1090:2373, 2220 bytes
    fn spawn_people(&mut self) {
        // TODO: port 1090:2373.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monate::{Date, START};

    #[test]
    fn a_day_is_six_hours() {
        let mut w = World::default();
        let ticks = 6 * 4;
        for n in 1..ticks {
            assert_eq!(w.tick(), Tick::default(), "tick {n}");
        }
        assert_eq!(w.date, START);
        assert!(w.tick().new_day);
        assert_eq!(w.date, Date { day: 2, ..START });
        assert_eq!((w.clock.day_ticks, w.clock.hour_ticks), (0, 0));
        // The events ran once, on the day's first tick.
        assert_eq!(w.days, 1);
        assert_eq!(w.clock.slice, 24);
    }

    #[test]
    fn pause() {
        let mut w = World::default();
        w.clock.paused = true;
        for _ in 0..100 {
            w.tick();
        }
        assert_eq!(w.date, START);
        assert_eq!((w.clock.day_ticks, w.clock.hour_ticks), (0, 0));
        assert_eq!(w.clock.slice, 100 % 30);
    }

    #[test]
    fn accounting_schedule() {
        let mut w = World::default();
        w.accounting.last_day = 1;
        // A month at the default speed: the step counts the accounting
        // ticks that see no new day, and starts again on the 1st.
        let mut steps = Vec::new();
        while w.date.month == 1 {
            w.tick();
            if w.clock.hour_ticks == 0 {
                steps.push((w.date.day, w.accounting.step));
            }
        }
        assert_eq!(&steps[..6], &[(1, 1), (1, 2), (1, 3), (1, 4), (1, 5), (2, 5)]);
        assert_eq!(steps[6], (2, 6));
        assert_eq!(w.accounting.step, 31 * 5);
        // On the 1st of February, the first tick that sees the day again
        // starts the month's steps.
        for _ in 0..4 {
            w.tick();
        }
        assert_eq!(w.date, Date { month: 2, ..START });
        assert_eq!(w.accounting.step, 0);
    }
}
