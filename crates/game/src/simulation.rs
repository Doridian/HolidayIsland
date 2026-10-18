//! The simulation's clock: the world's part of `THauptschirm`'s
//! `AnimationsTimer` (1020:1e9b), which runs every 100 ms while the main
//! screen is up. `docs/simulation.md` has the whole routine, the view's
//! parts included, and what each subsystem does.
//!
//! Each subsystem is named after what it does, with its address, and
//! draws its random numbers from [`World::random`] in the original's
//! order, the screen's shaking in an earthquake too, whose moves the
//! world draws and each view makes ([`Tick::shake`]).

use crate::auction::Change;
use crate::enhance::Fix;
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
    /// The host's pause: the pause form stopped the clock for everyone
    /// (`AnimationsTimer` off, 1020:24a0), and nothing of the world
    /// runs. A port extension for several human clients
    /// ([`crate::command::Action::Pause`], the host's): with one, the
    /// view just doesn't tick while its pause form is up, as the
    /// original, and this stays off. Not in the map file.
    pub stopped: bool,
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
            stopped: false,
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
    /// With several human seats and "Auktion wartet", how many ticks an
    /// auction waits for the seats to start it or pass; then the seats
    /// that haven't count as passed (`crate::auction`). The host's
    /// ([`crate::command::Action::AuctionTimeout`]), in world ticks so
    /// every peer counts the same. A port extension: the original's one
    /// human starts it whenever they like. Saved in the port's chunk
    /// (`options.auction_timeout`).
    pub auction_timeout: u16,
}

/// [`Options::auction_timeout`]'s default: 30 s of [`TICK_MS`].
pub const AUCTION_TIMEOUT: u16 = 300;

impl Default for Options {
    /// // 1000:1ea0
    fn default() -> Self {
        Options {
            animations: true,
            people_move: true,
            people_spawn: true,
            auction_waits: true,
            auction_timeout: AUCTION_TIMEOUT,
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
    /// With `fix.view_people`, the people's sounds as they come, for each
    /// view to play where it sees them ([`PlacedSound`]); the world
    /// doesn't know what anyone sees.
    pub placed_sounds: Vec<PlacedSound>,
    /// With `fix.view_people`, the people a loaded game or the fix
    /// switched on took out of the world because they're the views' own
    /// (`crate::people::view`): each view takes them over.
    pub view_people: Vec<crate::people::Person>,
    /// The scenario was won or lost ([`World::january_second`]): the
    /// original shows the end of the game (`schlussform`, 1068:1016).
    /// With several human seats the game's end for everyone: the first
    /// winner's results, or a loss for all (seat 0) when every human seat
    /// is out; each view shows its seat's from [`Tick::seat_results`]
    /// ([`Tick::results_for`]).
    pub game_over: Option<GameOver>,
    /// With several human seats, each seat's own results as the game
    /// ends or as the seat goes out (decision 9). Empty with one human.
    pub seat_results: Vec<GameOver>,
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
    /// (1018:26b9), with the seat whose menu it is: the original selects
    /// their first row again. Single human: the menu is player 1's.
    pub build_menu_removed: Vec<(u8, u8)>,
    /// The human built a type there's only one of: the original presses
    /// the pointer's button (1078:1d21, 1010:2c8b). The seat that built
    /// it; single human: player 1.
    pub pointer_tool: Option<u8>,
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
    /// The earth shook (step 7): the original moves the main screen by
    /// `Random(7) - 3` across, then down, from `RandSeed`, every tick of
    /// an earthquake, paused or not (en 1020:2367). The world draws them
    /// in the original's order; each view moves its own main screen
    /// (`crate::hauptschirm::Hauptschirm::shake`).
    pub shake: Option<(i32, i32)>,
    /// An earthquake began: the original remembers where the main screen
    /// is (en 1028:02b1), the view's.
    pub quake_began: bool,
    /// An earthquake ended: the original puts the main screen back where
    /// it was remembered and the TV where it was last hidden
    /// (en 1028:05f7), the view's.
    pub quake_ended: bool,
}

/// A sound of the people's from a square, with `fix.view_people`: a view
/// that sees the square plays one of `sounds`, picked with its own random
/// numbers where there are several (the original picks with the world's,
/// and only where the player sees it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlacedSound {
    /// Where it comes from.
    pub at: Pos,
    /// One of them plays: `sounds[Random(len)]`.
    pub sounds: &'static [u8],
    /// How many of [`Tick::sounds`] came before it, which it plays after.
    pub after: usize,
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
    /// Whose results these are: 1 for the original's human; with several
    /// human seats the seat, or 0 in [`Tick::game_over`] when all lost.
    pub seat: u8,
    /// Won, or lost. // 1110:0071
    pub won: bool,
    /// For a won game without cheating, the score (0–100) the original
    /// enters in the high scores (`add_highscore`, 1040:1bdf, with the
    /// player's net worth as money text). // 1110:0068, 1110:006c
    pub score: Option<i32>,
    /// The player's net worth for the high scores (1008:0c6e).
    pub net_worth: i32,
}

impl Tick {
    /// What the end form of a view of `seat` shows: the seat's own
    /// results ([`Tick::seat_results`]) or, with one human or for a seat
    /// that has none (a spectator's), the game's end ([`Tick::game_over`]).
    #[must_use]
    pub fn results_for(&self, seat: u8) -> Option<GameOver> {
        self.seat_results
            .iter()
            .find(|r| r.seat == seat)
            .copied()
            .or(self.game_over)
    }
}

impl World {
    /// One tick of `AnimationsTimer`, the world's steps in the original's
    /// order. // 1020:1e9b
    ///
    /// Left to the view: re-enabling the forms (1010:2a0c), the status bar
    /// message's timeout and the tooltips (before the clock), scrolling
    /// while a scroll arrow is held, moving the main screen in an
    /// earthquake ([`Tick::shake`]), the TV (1048:1b3e) and drawing the
    /// people.
    ///
    /// The auction form's timers run first, every tick
    /// ([`World::auction_tick`]); while the form is open the timer stands
    /// (1020:24a0) and nothing else runs.
    pub fn tick(&mut self) -> Tick {
        let mut tick = Tick::default();
        // BUG(original): one player's dialog stops the clock for everyone (1020:24a0; design issue for multiplayer: with several humans only the host's pause, a command, decisions 8 and 11).
        // Single human: the pause form, the options, the quit question
        // and the end form stop the view's clock, which then doesn't
        // tick. With several human clients a client's dialog stops
        // nothing, and only the host's pause stops the world for
        // everyone, as a command (`Clock::stopped`).
        if self.clock.stopped {
            return tick;
        }
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
        if self.earthquake {
            // The screen shakes: the main screen's `Left`, then `Top`, by
            // `Random(7) - 3`, which the view makes. // en 1020:2367
            let dx = i32::from(self.random.random(7)) - 3;
            let dy = i32::from(self.random.random(7)) - 3;
            tick.shake = Some((dx, dy));
            // BUG(original): the hour's counter stands while the game is paused (the TV), and every paused tick at its first takes another 100 squares.
            let paused = self.clock.paused && self.fixes.has(Fix::QuakePause);
            if self.clock.hour_ticks == 1 && !paused {
                self.quake_damage(tick); // 10a0:0d1c
            }
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

    /// The earth shakes every tick of an earthquake, paused or not: two
    /// `Random(7) - 3` from the world's numbers, before the hour's damage
    /// (en 1020:2367).
    #[test]
    fn earthquake_shakes() {
        let mut world = World {
            earthquake: true,
            ..World::default()
        };
        world.clock.speed = 1000;
        // No people, whose steps draw numbers too.
        world.options.animations = false;
        let mut numbers = world.random;
        let tick = world.tick();
        let dx = i32::from(numbers.random(7)) - 3;
        let dy = i32::from(numbers.random(7)) - 3;
        assert_eq!(tick.shake, Some((dx, dy)));
        // The first tick of the hour: the damage's 200 numbers after them.
        for _ in 0..200 {
            numbers.random(250);
        }
        assert_eq!(world.random.seed, numbers.seed);
        let tick = world.tick();
        let (dx, dy) = (numbers.random(7), numbers.random(7));
        assert_eq!(tick.shake, Some((i32::from(dx) - 3, i32::from(dy) - 3)));
        assert_eq!(world.random.seed, numbers.seed, "no damage after the hour's first tick");
        world.earthquake = false;
        assert_eq!(world.tick().shake, None);
    }

    /// Paused at an hour's first tick, the original takes 100 squares on
    /// every tick of an earthquake; `fix.quake_pause` only on the first.
    #[test]
    fn fix_quake_pause() {
        for fixed in [false, true] {
            let mut w = World {
                earthquake: true,
                ..World::default()
            };
            w.fixes.set(Fix::QuakePause, fixed);
            w.clock.speed = 1000;
            // No people, whose steps draw numbers too.
            w.options.animations = false;
            w.tick();
            w.clock.paused = true;
            let mut r = w.random;
            for _ in 0..3 {
                assert!(w.tick().shake.is_some(), "it shakes while paused");
                r.random(7);
                r.random(7);
                if !fixed {
                    for _ in 0..200 {
                        r.random(250);
                    }
                }
            }
            assert_eq!(w.clock.hour_ticks, 1);
            assert_eq!(w.random.seed, r.seed, "fixed: {fixed}");
        }
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
