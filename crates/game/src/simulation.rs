//! The simulation's clock: the world's part of `THauptschirm`'s
//! `AnimationsTimer` (1020:1e9b), which runs every 100 ms while the main
//! screen is up. `docs/simulation.md` has the whole routine, the view's
//! parts included, and what each subsystem does.
//!
//! Each subsystem is named after what it does, with its address, and
//! draws its random numbers from [`World::random`] in the original's
//! order; the one exception is the screen's shaking in an earthquake,
//! which is the view's (see [`World::tick`]).

use crate::auction::Change;
use crate::news::Message;
use crate::world::{Pos, World};

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

/// The animation options, and whether auctions wait for the player. They
/// change the world (people only move with them on, an auction stands
/// until the player starts it), although they're the player's settings in
/// the original.
#[allow(clippy::struct_excessive_bools)] // One flag for each of the original's options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Options {
    /// People and cars are animated at all. // 1110:0054
    pub animations: bool,
    /// They walk along their paths. // 1110:0055
    pub people_move: bool,
    /// New ones come. // 1110:0056
    pub people_spawn: bool,
    /// "Auktion wartet": an auction stands until the player starts it.
    /// // 1110:0063
    pub auction_waits: bool,
}

impl Default for Options {
    /// // 1000:1ea0
    fn default() -> Self {
        Options {
            animations: true,
            people_move: true,
            people_spawn: true,
            auction_waits: true,
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
#[allow(clippy::struct_excessive_bools)] // One flag for each of the original's calls.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Tick {
    /// The date moved on: the original redraws it (1030:228d) and starts
    /// the next music track if the last has ended (1018:367e).
    pub new_day: bool,
    /// Squares that changed, in order, which the original redraws one by
    /// one (1050:21f9 with a 1 × 1 area).
    pub redraw: Vec<Pos>,
    /// The news, in order (1048:0a54).
    pub messages: Vec<Message>,
    /// The whole island needs redrawing: a flood began or ended
    /// (1020:0d48).
    pub redraw_all: bool,
    /// An auction of [`World::auction`]'s item begins: the original starts
    /// `TAuktionsform`'s timer (1028:08ac).
    pub auction: bool,
    /// What happened on the auction form, in order (`crate::auction`).
    pub auction_changes: Vec<Change>,
    /// Sounds to play, in order (`play_sound`, 10a0:30b6): the people's,
    /// where the player sees them, and the auction's.
    pub sounds: Vec<u8>,
    /// The scenario was won or lost ([`World::january_second`]): the
    /// original shows the end of the game (`schlussform`, 1068:1016).
    pub game_over: Option<GameOver>,
    /// The books changed: the original refreshes the money menu
    /// (1070:2617), working out this year's total (1070:1452) if it's
    /// shown.
    pub books: bool,
    /// A command loaded a saved game ([`crate::command::Action::Load`]):
    /// the original redraws everything (`game_show_state`, 1010:26fa).
    pub loaded: bool,
    /// The saved game couldn't be loaded; the world is as it was.
    pub load_failed: bool,
    /// The original refreshes `TUmenuform` if it's shown (1080:0123).
    pub umenu: bool,
    /// Buildings whose occupancy changed by at least a tenth, as their
    /// first and last squares: the original redraws the area on the
    /// overview map (10b8:1534) and, with the occupancy pointers shown
    /// (`1110:0059`), on the main screen (1050:264d).
    pub occupancy: Vec<(Pos, Pos)>,
    /// What the original shows a player on the status bar after their
    /// command, with the player: an error from `FEHLER.TXT` (1030:232d) or
    /// the building in the way (1030:235a). Single human: the original
    /// only has messages for player 1.
    pub status: Vec<(u8, Status)>,
    /// The build menu's submenus the player's building took a type off
    /// (1018:26b9): the original selects their first row again.
    pub build_menu_removed: Vec<u8>,
    /// The human built a type there's only one of: the original presses
    /// the pointer's button (1078:1d21, 1010:2c8b).
    pub pointer_tool: bool,
    /// The year ended ([`World::year_end`]): with the player's option
    /// `1110:0064` (the view's `crate::optionen::Settings::year_balance`)
    /// the original then shows the money menu's balance sheet (1070:2b11
    /// with page 1), before the renovations.
    pub new_year: bool,
    /// The building sites finished, as they were: the original closes the
    /// info box if it shows a square of one (1008:525b, 1038:1ee0).
    pub finished_sites: Vec<crate::world::BuildingSite>,
    /// The news as it went into the archive, one for each of
    /// [`Tick::messages`] (`crate::news`): the TV's part.
    pub news: Vec<crate::news::Posted>,
    /// The people moved (`people_tick`, 1090:5293), which first counts
    /// down how long each sound still blocks itself: the view's
    /// (`crate::sound::Effects::count_down`), after the first this many
    /// of [`Tick::sounds`].
    pub sound_countdown: Option<usize>,
    /// The TV's lights while its speaker reads ([`World::tv_lights`]): the
    /// lower one's frame, and the upper one's if it changes.
    pub tv_lights: Option<(u16, Option<u16>)>,
}

/// A message on the status bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// `FEHLER.TXT`'s text `n` (1-based), with sound 0xc. // 1030:232d
    Error(u8),
    /// The building type in the way: its name and `FEHLER.TXT` 0x1d, error
    /// 1 for a type past 0x7e, nothing for a palm. // 1030:235a
    InTheWay(u8),
}

/// How a game ended. // 1008:0512
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameOver {
    /// Won, or lost. // 1110:0071
    pub won: bool,
    /// For a won game without cheating, the score (0–100) the original
    /// enters in the high scores (`add_highscore`, 1040:1bdf, with the
    /// player's net worth as money text). // 1110:0068, 1110:006c
    pub score: Option<i32>,
    /// The player's net worth for the high scores (1008:0c6e).
    pub net_worth: i32,
}

impl World {
    /// One tick of `AnimationsTimer`, the world's steps in the original's
    /// order. // 1020:1e9b
    ///
    /// Left to the view: re-enabling the forms (1010:2a0c), the status bar
    /// message's timeout and the tooltips (before the clock), scrolling
    /// while a scroll arrow is held, shaking the screen in an earthquake,
    /// the TV (1048:1b3e) and drawing the people.
    ///
    /// The auction form's timers run first, every tick
    /// ([`World::auction_tick`]); while the form is open the timer stands
    /// (1020:24a0) and nothing else runs.
    pub fn tick(&mut self) -> Tick {
        let mut tick = Tick::default();
        self.auction_tick(&mut tick);
        if !self.auction.open {
            self.animations_timer(&mut tick);
        }
        self.geldmenu_refresh(&tick); // 1070:2617
        tick
    }

    /// `AnimationsTimer`'s world steps. // 1020:1e9b
    fn animations_timer(&mut self, tick: &mut Tick) {
        if !self.clock.paused {
            let c = &mut self.clock;
            c.hour_ticks = c.hour_ticks.wrapping_add(1);
            c.day_ticks = c.day_ticks.wrapping_add(1);
            match c.day_ticks {
                1 => self.events_tick(tick),
                2 => self.effects_tick(tick),
                3 | 5 => self.vegetation_tick(tick),
                4 => self.beach_life_tick(tick),
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
                self.accounting_tick(tick);
            }
        }
        self.clock.slice = (self.clock.slice.wrapping_add(1)) % 30;
        self.building_slice(self.clock.slice, tick);
        // The original shakes the screen here during an earthquake, with
        // two `Random(7)`s; the port leaves that to the view, so its random
        // numbers part from the original's while the earth shakes.
        if self.earthquake && self.clock.hour_ticks == 1 {
            self.quake_damage(tick);
        }
        self.tv_lights(tick); // 1048:1b3e
        if self.options.animations {
            tick.sound_countdown = Some(tick.sounds.len());
            self.people_tick(tick); // 1090:5293
            if self.options.people_spawn {
                self.spawn_people(tick); // 1090:2373
            }
        }
    }

    /// The monthly settlement's scheduler. // 1008:5307
    pub fn accounting_tick(&mut self, tick: &mut Tick) {
        let day = u16::from(self.date.day);
        if day != self.accounting.last_day {
            self.construction_tick(tick);
            self.accounting.last_day = day;
            return;
        }
        let a = &mut self.accounting;
        a.step = a.step.wrapping_add(1);
        if a.step > 12 && self.date.day == 1 {
            a.step = 0;
            if self.date.month == 1 {
                self.year_end();
                tick.new_year = true;
            }
        }
        if self.date.month == 1 && self.accounting.step == 5 {
            self.january_second(tick);
        }
        // The step's low word.
        #[allow(clippy::cast_possible_truncation)]
        let step = self.accounting.step as i16;
        self.settlement_step(step, tick); // 1008:1bf7
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
            assert!(!w.tick().new_day, "tick {n}");
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
