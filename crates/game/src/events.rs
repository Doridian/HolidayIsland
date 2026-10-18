//! The scenario's events (`EREIGNIS.DAT`, or a scenario's `EREIGNn.DAT`):
//! auctions, news, disasters and new building types that come on certain
//! days with a chance (segment `1010`, and the disasters' routines in
//! `10a0`).

use crate::news::Message;
use crate::simulation::Tick;
use crate::world::{Pos, World};

/// The number of events. // 1010:3084
pub const EVENTS: usize = 41;
/// The size of an event's record: `EREIGNIS.DAT`'s 12 values and a
/// 13th. // 1010:3ab4
pub const EVENT_SIZE: usize = 0x34;

/// One event, as the map file stores it (`*1110:6536`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Event {
    /// Out of 65000, the chance each day in its window that it starts;
    /// 0 once it has started, so it never comes twice. // +0
    pub chance: i32,
    /// The window of days ([`World::days`]) in which it may start. // +4, +8
    pub first_day: i32,
    pub last_day: i32,
    /// Days since it started, 0 while it isn't running. // +0xc
    pub running: i32,
    /// When it ends, in days of `running`. // +0x10
    pub duration: i32,
    /// A price or amount for its message and auction. // +0x14
    pub value: i32,
    /// The message when it starts, and the auction's item or the disaster
    /// (5 an oil slick, 6 an earthquake, 0x2b a flood). // +0x18
    pub start: i32,
    /// The message when it ends, and that auction's item. // +0x1c
    pub end: i32,
    /// Not used by `events_tick`. // +0x20
    pub column_8: i32,
    /// The building type a new building type event makes available. // +0x24
    pub building: i32,
    /// What it does, 0–6: [`World::events_tick`]. // +0x28
    pub kind: i32,
    /// A value for the start message. // +0x2c
    pub arg: i32,
    /// Where its message points for kinds 3 and 5 (not from the file).
    /// // +0x30, +0x32
    pub pos: Pos,
}

impl Event {
    fn parse(r: &[u8]) -> Self {
        let long = |n: usize| i32::from_le_bytes([r[4 * n], r[4 * n + 1], r[4 * n + 2], r[4 * n + 3]]);
        let word = |at: usize| i16::from_le_bytes([r[at], r[at + 1]]);
        Event {
            chance: long(0),
            first_day: long(1),
            last_day: long(2),
            running: long(3),
            duration: long(4),
            value: long(5),
            start: long(6),
            end: long(7),
            column_8: long(8),
            building: long(9),
            kind: long(10),
            arg: long(11),
            pos: (word(0x30), word(0x32)),
        }
    }

    /// The events from `EREIGNIS.DAT`'s rows (`formats::data::ereignis`),
    /// with the 13th value 0. // 1010:3ab4
    #[must_use]
    pub fn from_row(row: &[i32; 12]) -> Self {
        let mut r = [0; EVENT_SIZE];
        for (n, v) in row.iter().enumerate() {
            r[4 * n..4 * n + 4].copy_from_slice(&v.to_le_bytes());
        }
        Event::parse(&r)
    }

    /// The event with its 12 values from a row of an events file, and
    /// the 13th, [`Event::pos`], as it was. // 1010:36f8
    #[must_use]
    pub fn with_row(self, row: &[i32; 12]) -> Self {
        Event {
            pos: self.pos,
            ..Event::from_row(row)
        }
    }
}

/// The low word of a `Longint`, where the original reads a `Longint`
/// field as an `Integer`.
pub(crate) fn low_word(v: i32) -> i16 {
    let b = v.to_le_bytes();
    i16::from_le_bytes([b[0], b[1]])
}

/// The events from the map file's `*1110:6536` (unit 1010:3e8a).
#[must_use]
pub fn parse_events(data: &[u8]) -> Option<Vec<Event>> {
    (data.len() == EVENTS * EVENT_SIZE).then(|| data.chunks(EVENT_SIZE).map(Event::parse).collect())
}

/// Whether an event with `chance` starts today: always from 65536 on,
/// never below 1, else when `Random(65000) + 1 < chance`. The random
/// number is drawn either way. // 1000:3d9e
pub fn event_chance(random: &mut vcl::random::Random, chance: i32) -> bool {
    let r = i32::from(random.random(65000)) + 1;
    chance > 0xffff || (chance >= 0 && r < chance)
}

/// A disaster's damage, which the player gets back at its end if they're
/// insured. // unit 10a0:23c9
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Damage {
    /// The player's losses so far. // 1110:7050
    pub amount: i32,
    /// [`World::insured`] when the disaster began. // 1110:7054
    pub insured: bool,
    /// Where the oil slick is. // 1110:7056
    pub slick: Pos,
}

/// What earthquakes and floods leave standing: palms, beach life and
/// empty squares. // 1110:1358 (`set of Byte`)
pub const UNSHAKEN: [u8; 3] = [12, 36, 255];

/// How many squares an earthquake hits each hour. // 10a0:0d1c
const QUAKE_SQUARES: usize = 100;

impl World {
    /// A scenario's events file (`EREIGNn.DAT`) over the events: each
    /// record's 12 values from its row, in order; the 13th
    /// ([`Event::pos`], from the map) stays. The original reads 41 rows
    /// (and stops the program if the file has fewer); the port reads
    /// those there are. // 1010:36f8
    pub fn load_event_rows(&mut self, rows: &[[i32; 12]]) {
        for (e, row) in self.events.iter_mut().zip(rows) {
            *e = e.with_row(row);
        }
    }

    /// Counts the day ([`World::days`]) and goes through the events. One
    /// that isn't running starts on a day in its window with its chance
    /// (`Random(65000)`, drawn only in the window), which then becomes 0,
    /// and by its kind:
    ///
    /// - 0: its message, and an auction of its `start` item at once.
    /// - 1: its message; at the end the end message and an auction of the
    ///   `end` item.
    /// - 2: a disaster, if they're on ([`World::disasters`]; the event is
    ///   used up either way): flood, earthquake or oil slick, with its
    ///   message; at the end, if any of the three is going on, the end
    ///   message and the disaster stops.
    /// - 3, 5: its message at `pos` (5 also scrolls there); at the end an
    ///   auction of the `start` item, and it could start again, but its
    ///   chance is 0.
    /// - 4: just its message.
    /// - 6: a message with no values, and a new building type.
    ///
    /// A running event counts its days and ends when they reach its
    /// duration. // 1010:3084
    pub(crate) fn events_tick(&mut self, tick: &mut Tick) {
        self.days = self.days.wrapping_add(1);
        for n in 0..self.events.len() {
            let e = self.events[n];
            if e.running == 0 {
                if e.first_day <= self.days && self.days <= e.last_day && event_chance(&mut self.random, e.chance) {
                    self.events[n].chance = 0;
                    self.start_event(n, tick);
                }
            } else {
                let running = e.running.wrapping_add(1);
                self.events[n].running = running;
                if running == e.duration {
                    self.end_event(n, tick);
                }
            }
        }
    }

    /// The start of event `n`. // 1010:3084
    fn start_event(&mut self, n: usize, tick: &mut Tick) {
        let e = self.events[n];
        let message = Message {
            number: low_word(e.start),
            a: low_word(e.arg),
            x: e.value,
            pos: None,
            ..Message::default()
        };
        match e.kind {
            0 => {
                self.post_message(message, tick);
                self.start_auction(e.start, e.value, tick);
            }
            1 => {
                self.post_message(message, tick);
                self.events[n].running = 1;
            }
            2 => {
                if self.disasters {
                    self.events[n].running = 1;
                    let mut pos = None;
                    match e.start {
                        0x2b => {
                            self.flood = true;
                            self.flood_start(tick); // 10a0:0b49
                            // The original redraws the island here
                            // (1020:0d48).
                            tick.redraw_all = true;
                        }
                        6 => {
                            self.earthquake = true;
                            self.quake_start(); // 10a0:0d09
                            // The original remembers the main screen's
                            // position (en 1118:6764) for the end of the
                            // shaking: the view's (`Tick::quake_began`).
                            tick.quake_began = true;
                        }
                        5 => {
                            self.oil_slick = true;
                            pos = Some(self.oil_slick_start(tick)); // 10a0:074d
                        }
                        _ => {}
                    }
                    self.post_message(Message { pos, ..message }, tick);
                }
            }
            3 | 5 => {
                let centre = e.kind == 5;
                self.post_message(
                    Message {
                        pos: Some(e.pos),
                        centre,
                        ..message
                    },
                    tick,
                );
                self.events[n].running = 1;
            }
            4 => self.post_message(message, tick),
            6 => {
                self.post_message(
                    Message {
                        number: message.number,
                        ..Message::default()
                    },
                    tick,
                );
                self.make_available(e.building.to_le_bytes()[0], 0); // 10a0:0691
            }
            _ => {}
        }
    }

    /// The end of event `n`, the day its running days reach its duration.
    /// // 1010:3084
    fn end_event(&mut self, n: usize, tick: &mut Tick) {
        let e = self.events[n];
        let message = Message {
            number: low_word(e.end),
            ..Message::default()
        };
        match e.kind {
            1 => {
                self.post_message(message, tick);
                self.start_auction(e.end, e.value, tick);
            }
            2 => {
                if self.flood || self.earthquake || self.oil_slick {
                    self.post_message(message, tick);
                    match e.start {
                        0x2b => {
                            self.flood = false;
                            self.disaster_end(tick); // 10a0:0cce
                            tick.redraw_all = true; // 1020:0d48
                        }
                        6 => {
                            self.earthquake = false;
                            self.disaster_end(tick); // 10a0:0ddc
                            // The original puts the main screen back where
                            // it was when the shaking began, and the TV
                            // where it was last hidden (en 1028:05f7): the
                            // view's (`Tick::quake_ended`).
                            tick.quake_ended = true;
                        }
                        5 => {
                            self.oil_slick = false;
                            self.oil_slick_end(tick); // 10a0:0915
                        }
                        _ => {}
                    }
                }
            }
            3 | 5 => {
                self.start_auction(e.start, e.value, tick);
                self.events[n].running = 0;
            }
            _ => {}
        }
    }

    /// An earthquake begins: no damage yet, and whether the player is
    /// insured now counts for it. Single human: player 1's; each of seats
    /// 2 to 8 has its own in its slot (`crate::seats::SeatSlots`).
    /// // 10a0:0d09
    fn quake_start(&mut self) {
        self.damage.amount = 0;
        self.damage.insured = self.insured;
        for s in &mut self.seat_slots {
            s.damage = 0;
            s.damage_insured = s.insured;
        }
    }

    /// Adds `amount` to the damage of the disaster going on of `owner`,
    /// if a human plays them (single human: player 1's).
    fn add_damage(&mut self, owner: u8, amount: i32) {
        if owner == 1 {
            self.damage.amount = self.damage.amount.wrapping_add(amount);
        } else if self.is_human(owner)
            && let Some(s) = self.slots_mut(owner)
        {
            s.damage = s.damage.wrapping_add(amount);
        }
    }

    /// An hour of an earthquake: 100 random squares (`Random(250)` for `i`,
    /// then `j`) lose their building and people, unless it's a palm or
    /// beach life; the human's count as damage at half their price
    /// (marked: only player 1's; here each human seat's). // 10a0:0d1c
    pub(crate) fn quake_damage(&mut self, tick: &mut Tick) {
        for _ in 0..QUAKE_SQUARES {
            let i = self.random.random(250).cast_signed();
            let j = self.random.random(250).cast_signed();
            let building = self.island.buildings.at((i, j));
            if UNSHAKEN.contains(&building) {
                continue;
            }
            let half = self.tables.hausprei(building, 0) / 2;
            self.add_damage(self.island.owners.at((i, j)), half);
            self.remove_people_from((i, j), tick); // 1088:6975
            // For no one, so it never refuses.
            let _ = self.demolish(0, (i, j), tick);
        }
    }

    /// The flood begins: no damage yet, insurance as for an earthquake,
    /// and every square at sea level (lowest corner below 2) whose building
    /// isn't one of [`UNSHAKEN`] loses it ([`World::demolish`] for no one),
    /// the player's counted as damage at half their price (marked: only
    /// player 1's), and then the people on it. // 10a0:0b49
    fn flood_start(&mut self, tick: &mut Tick) {
        self.quake_start();
        for i in 0..=250 {
            for j in 0..=250 {
                let building = self.island.buildings.at((i, j));
                if UNSHAKEN.contains(&building) {
                    continue;
                }
                if self.island.lowest_corner((i, j)) < 2 {
                    let half = self.tables.hausprei(building, 0) / 2;
                    self.add_damage(self.island.owners.at((i, j)), half);
                    let _ = self.demolish(0, (i, j), tick);
                }
                if self.island.lowest_corner((i, j)) < 2 {
                    self.flood_people((i, j));
                }
            }
        }
    }

    /// A flood or an earthquake ends: an insured player gets the damage
    /// back, in the books' "other". Single human: the insurance is player
    /// 1's; here each human seat's, of their own damage. // 10a0:0cce,
    /// 10a0:0ddc
    fn disaster_end(&mut self, tick: &mut Tick) {
        if self.damage.insured {
            self.add_money(1, self.damage.amount);
            self.book(crate::accounts::BOOK_OTHER, self.damage.amount); // 1070:1419
            tick.books = true; // 1070:2617
        }
        for p in 2..=8 {
            let Some((damage, true)) = self
                .slots(p)
                .filter(|_| self.is_human(p))
                .map(|s| (s.damage, s.damage_insured))
            else {
                continue;
            };
            self.add_money(p, damage);
            self.book_for(p, crate::accounts::BOOK_OTHER, damage); // 1070:1419
            tick.books = true; // 1070:2617
        }
    }
}

/// How many spots an oil slick tries, less one. // 10a0:074d
const OIL_TRIES: i16 = 1000;
/// The steps of the oil's random walk, less one. // 10a0:074d
const OIL_STEPS: i16 = 0x32;
/// The oil's three kinds of people, by `Random(3)`. // 10a0:074d
pub const OIL: [u8; 3] = [0x2b, 0x2c, 0x2d];
/// The location map around the slick: × 50 % within 30 cells.
/// // 10a0:074d, 10a0:0915
const OIL_LOCATION: (i32, i32) = (50, 30);

impl World {
    /// An oil slick begins. Up to 1001 tries at a random square
    /// (`Random(250)` for `i`, then `j`) of flat sea with sand (ground 0)
    /// two squares off on a side, a beach; from the first that is, 51
    /// steps of a random walk (`Random(3) - 1` on `i`, then `j`) over flat
    /// sea, each to a square nobody is on leaving oil of a kind by
    /// `Random(3)` (1088:0324).
    /// Where the walk ends (or the last try, if none was right) is the
    /// slick's square and the message's. The damage is a hundredth of the
    /// human's buildings' value (1008:0b0b), insured if they are now, and
    /// the location map goes down to 50 % within 30 cells around the
    /// square (1030:185d). Single human: the damage is player 1's; here
    /// each human seat's, of their own buildings. // 10a0:074d, 456 bytes
    fn oil_slick_start(&mut self, tick: &mut Tick) -> Pos {
        let mut pos: Pos;
        let mut tries: i16 = 0;
        loop {
            let i = self.random.random(250).cast_signed();
            let j = self.random.random(250).cast_signed();
            pos = (i, j);
            if self.oil_start_square(pos) {
                tries = OIL_TRIES;
                for _ in 0..=OIL_STEPS {
                    let ni = pos.0.wrapping_add(self.random.random(3).cast_signed()).wrapping_sub(1);
                    let nj = pos.1.wrapping_add(self.random.random(3).cast_signed()).wrapping_sub(1);
                    let next = (ni, nj);
                    if self.ground_or_water(next) == 2 && self.form_or_flat(next) == crate::roads::FLAT {
                        pos = next;
                        if !self.anyone_at(next) {
                            let kind = self.random.random(3);
                            // BUG(original): the walk can leave the map by its edge rows, and the oil there is written outside the marker layer; the port writes no marker (`set_marker`).
                            self.add_person(OIL[usize::from(kind)], next, tick);
                        }
                    }
                }
            }
            if tries == OIL_TRIES {
                break;
            }
            tries += 1;
        }
        self.damage.slick = pos;
        self.damage.insured = self.insured;
        self.damage.amount = self.assets(1) / 100; // 1008:0b0b
        // Single human: player 1's damage; each human seat's here.
        for p in 2..=8 {
            let amount = if self.is_human(p) { self.assets(p) / 100 } else { 0 };
            if let Some(s) = self.slots_mut(p) {
                s.damage = amount;
                s.damage_insured = s.insured;
            }
        }
        let (value, radius) = OIL_LOCATION;
        self.change_location(pos, value, radius, false); // 1030:185d
        pos
    }

    /// Whether an oil slick may start on `pos`: flat sea with sand (ground
    /// 0) two squares off in one of the four directions. // 10a0:074d
    fn oil_start_square(&self, (i, j): Pos) -> bool {
        self.ground_or_water((i, j)) == 2
            && self.form_or_flat((i, j)) == crate::roads::FLAT
            && [(2, 0), (-2, 0), (0, 2), (0, -2)]
                .iter()
                .any(|&(di, dj)| self.ground_or_water((i.wrapping_add(di), j.wrapping_add(dj))) == 0)
    }

    /// The oil slick ends: the location map around its square back
    /// (1030:185d), an insured player gets the damage back, in the books'
    /// "other", and the oil goes (1088:2df2), its squares' markers free.
    /// Single human: the insurance is player 1's; here each human
    /// seat's ([`World::disaster_end`]). // 10a0:0915, 213 bytes
    fn oil_slick_end(&mut self, tick: &mut Tick) {
        let (value, radius) = OIL_LOCATION;
        self.change_location(self.damage.slick, value, radius, true); // 1030:185d
        self.disaster_end(tick);
        let last = self.people.last;
        if last <= crate::people::LAST_MONEY {
            return;
        }
        for n in crate::people::FIRST..=last {
            if OIL.contains(&self.people.kind(n)) {
                self.remove_person(n); // 1088:2df2
                let pos = self.people.get(n).map(crate::people::Person::pos);
                if let Some(pos) = pos {
                    self.set_marker(pos, crate::people::FREE);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use vcl::random::Random;

    use super::*;

    fn world(events: Vec<Event>) -> World {
        World {
            events,
            random: Random::new(99),
            ..World::default()
        }
    }

    fn event(kind: i32) -> Event {
        Event {
            chance: 0x10000,
            first_day: 3,
            last_day: 5,
            duration: 4,
            value: 5000,
            start: 9,
            end: 10,
            kind,
            arg: 2,
            pos: (30, 40),
            ..Event::default()
        }
    }

    fn day(w: &mut World) -> Tick {
        let mut tick = Tick::default();
        w.events_tick(&mut tick);
        tick
    }

    #[test]
    fn chance() {
        let mut r = Random::new(5);
        assert!(event_chance(&mut r, 0x10000));
        assert!(!event_chance(&mut r, 0));
        assert!(!event_chance(&mut r, -1));
        assert!(event_chance(&mut r, 65001));
        let mut r2 = r;
        let n = i32::from(r2.random(65000));
        assert!(event_chance(&mut r, n + 2));
        let mut r2 = r;
        let n = i32::from(r2.random(65000));
        assert!(!event_chance(&mut r, n + 1));
        // One random number each time.
        let mut r2 = Random::new(5);
        for _ in 0..6 {
            r2.random(1);
        }
        assert_eq!(r, r2);
    }

    #[test]
    fn window_and_random_numbers() {
        // Outside its window an event draws nothing; in it, it draws its
        // `Random(65000)` every day until it starts (with a chance of 1,
        // never).
        let mut w = world(vec![Event { chance: 1, ..event(4) }; 3]);
        w.days = 1;
        day(&mut w);
        assert_eq!(w.days, 2);
        assert_eq!(w.random, Random::new(99));
        let tick = day(&mut w);
        assert_eq!(tick.messages, []);
        let mut r = Random::new(99);
        for _ in 0..3 {
            r.random(65000);
        }
        assert_eq!(w.random, r);
    }

    #[test]
    fn auction_at_the_end() {
        let mut w = world(vec![event(1)]);
        w.days = 2;
        let tick = day(&mut w);
        let mut r = Random::new(99);
        r.random(65000);
        let text = 9 * 3 + r.random(3).cast_signed();
        assert_eq!(
            tick.messages,
            [Message {
                number: 9,
                text,
                a: 2,
                x: 5000,
                ..Message::default()
            }]
        );
        assert_eq!(w.events[0].chance, 0);
        assert_eq!(w.events[0].running, 1);
        assert!(!tick.auction);
        for _ in 0..2 {
            assert_eq!(day(&mut w).messages, []);
        }
        let tick = day(&mut w);
        assert_eq!(w.events[0].running, 4);
        assert_eq!(tick.messages.len(), 1);
        assert_eq!((tick.messages[0].number, tick.messages[0].x), (10, 0));
        assert!(tick.auction);
        let a = &w.auction;
        assert_eq!((a.price, a.start_price, a.item), (5000, 5000, 10));
        assert_eq!(a.opening, Some(crate::auction::OPEN_TICKS));
        // It keeps counting, and never ends or starts again.
        day(&mut w);
        assert_eq!(w.events[0].running, 5);
    }

    #[test]
    fn auction_at_once() {
        let mut w = world(vec![event(0)]);
        w.days = 2;
        let tick = day(&mut w);
        assert!(tick.auction);
        assert_eq!(w.auction.item, 9);
        assert_eq!(w.events[0].running, 0);
        assert_eq!(day(&mut w).messages, []);
    }

    #[test]
    fn at_a_place() {
        let mut w = world(vec![event(5), event(3)]);
        w.days = 2;
        let tick = day(&mut w);
        let pos: Vec<_> = tick.messages.iter().map(|m| (m.pos, m.centre)).collect();
        assert_eq!(pos, [(Some((30, 40)), true), (Some((30, 40)), false)]);
        for _ in 0..3 {
            day(&mut w);
        }
        assert_eq!(w.events[0].running, 0);
        assert_eq!(w.auction.item, 9);
    }

    #[test]
    fn earthquake() {
        let quake = Event {
            start: 6,
            end: 39,
            ..event(2)
        };
        let mut w = world(vec![quake]);
        w.disasters = false;
        w.days = 2;
        // Off: the event is used up without happening.
        assert_eq!(day(&mut w).messages, []);
        assert_eq!((w.events[0].chance, w.events[0].running), (0, 0));
        assert!(!w.earthquake);

        let mut w = world(vec![quake]);
        w.days = 2;
        w.insured = true;
        w.player_count = 1;
        let tick = day(&mut w);
        assert!(w.earthquake);
        assert!(tick.quake_began && !tick.quake_ended);
        assert!(w.damage.insured);
        assert_eq!(tick.messages[0].number, 6);
        w.damage.amount = 1234;
        for _ in 0..2 {
            day(&mut w);
        }
        let tick = day(&mut w);
        assert!(!w.earthquake);
        assert!(tick.quake_ended && !tick.quake_began);
        assert_eq!(tick.messages[0].number, 39);
        assert_eq!(w.finances.money[0], 1234);
    }

    #[test]
    fn quake_damage() {
        let mut hausprei = vec![[0; 4]; 13];
        hausprei[5][0] = 1001;
        let mut hausnr = vec![[0; 8]; 13];
        hausnr[5] = [1, 0, 0, 1, 1, 1, 0, 0];
        let mut w = World {
            tables: std::sync::Arc::new(crate::world::Tables {
                hausnr,
                hausdata: Vec::new(),
                hausprei,
                groundnr: Vec::new(),
                ..crate::world::Tables::default()
            }),
            random: Random::new(4),
            ..World::default()
        };
        let mut r = Random::new(4);
        let squares: Vec<Pos> = (0..QUAKE_SQUARES)
            .map(|_| (r.random(250).cast_signed(), r.random(250).cast_signed()))
            .collect();
        // The human's building on the first square, a palm on the second.
        w.island.buildings.set(squares[0], 5);
        w.island.owners.set(squares[0], 1);
        w.island.buildings.set(squares[1], 12);
        w.island.owners.set(squares[1], 1);
        w.quake_damage(&mut Tick::default());
        assert_eq!(w.damage.amount, 500);
        assert_eq!(w.random, r);
        // Demolished for no one: nobody pays.
        assert_eq!(w.island.buildings.at(squares[0]), 255);
        assert_eq!(w.island.owners.at(squares[0]), 0);
        assert_eq!(w.island.buildings.at(squares[1]), 12);
        assert_eq!(w.finances.money, [0; 8]);
    }

    #[test]
    fn flood_demolishes_at_sea_level() {
        let mut hausprei = vec![[0; 4]; 13];
        hausprei[5][0] = 1001;
        let mut hausnr = vec![[0; 8]; 13];
        hausnr[5] = [1, 0, 0, 1, 1, 1, 0, 0];
        let mut w = World {
            tables: std::sync::Arc::new(crate::world::Tables {
                hausnr,
                hausdata: vec![[0; 10]; 13],
                hausprei,
                groundnr: Vec::new(),
                ..crate::world::Tables::default()
            }),
            ..World::default()
        };
        // The human's and a computer's building at sea level, one on a
        // hill and a palm.
        for (pos, owner) in [((10, 10), 1), ((20, 20), 2), ((30, 30), 1), ((40, 40), 1)] {
            w.island.buildings.set(pos, 5);
            w.island.owners.set(pos, owner);
        }
        w.island.buildings.set((40, 40), 12);
        for i in 29..=30 {
            for j in 29..=30 {
                w.island.heights.set((i, j), 2);
            }
        }
        let mut tick = Tick::default();
        w.flood_start(&mut tick);
        assert_eq!(w.damage.amount, 500);
        assert_eq!(w.island.buildings.at((10, 10)), 255);
        assert_eq!(w.island.buildings.at((20, 20)), 255);
        assert_eq!(w.island.owners.at((20, 20)), 0);
        assert_eq!(w.island.buildings.at((30, 30)), 5);
        assert_eq!(w.island.buildings.at((40, 40)), 12);
        // The empty squares get the sea's ground.
        assert_eq!(w.island.ground.at((10, 10)), 2);
        assert!(tick.redraw.contains(&(20, 20)));
    }

    #[test]
    fn flood() {
        let flood = Event {
            start: 0x2b,
            ..event(2)
        };
        let mut w = world(vec![flood]);
        w.days = 2;
        assert!(day(&mut w).redraw_all);
        assert!(w.flood);
        for _ in 0..2 {
            day(&mut w);
        }
        // Something else ended it already: no message.
        w.flood = false;
        let tick = day(&mut w);
        assert_eq!(tick.messages, []);
        assert!(!tick.redraw_all);
    }

    /// A sea of flat water with an island of sand at 60..190.
    fn sea() -> World {
        let mut island = crate::world::Island::empty();
        for i in 0..=250 {
            for j in 0..=250 {
                island.forms.set((i, j), crate::roads::FLAT);
                let land = (60..190).contains(&i) && (60..190).contains(&j);
                island.ground.set((i, j), if land { 0 } else { 2 });
            }
        }
        island.location = crate::world::LocationMap::filled(100);
        let mut w = World {
            island,
            random: Random::new(31),
            ..World::default()
        };
        w.people.last = crate::people::FIRST;
        w
    }

    fn oil(w: &World) -> Vec<Pos> {
        (crate::people::FIRST..=w.people.last)
            .filter_map(|n| w.people.get(n))
            .filter(|p| OIL.contains(&p.kind()))
            .map(crate::people::Person::pos)
            .collect()
    }

    #[test]
    fn oil_slick() {
        let mut w = sea();
        w.insured = true;
        let mut tick = Tick::default();
        let pos = w.oil_slick_start(&mut tick);
        // The first try next to the island is the start: count the random
        // numbers up to it.
        let mut r = Random::new(31);
        let start = loop {
            let at = (r.random(250).cast_signed(), r.random(250).cast_signed());
            if w.oil_start_square(at) {
                break at;
            }
        };
        let near = |v: i16| (58..60).contains(&v) || (190..192).contains(&v);
        assert!(near(start.0) || near(start.1), "{start:?}");
        let spots = oil(&w);
        assert_ne!(spots, Vec::<Pos>::new());
        assert!(spots.len() <= 51);
        for &at in &spots {
            assert_eq!(w.island.ground.at(at), 2);
            assert!(OIL.contains(&w.island.layer_551c.at(at)));
            assert!((at.0 - start.0).abs() <= 51 && (at.1 - start.1).abs() <= 51);
        }
        // Nobody twice on a square.
        let mut sorted = spots.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), spots.len());
        assert_eq!(w.damage.slick, pos);
        assert!(w.damage.insured);
        assert_eq!(w.damage.amount, 0);
        assert_eq!(w.location_at(pos), 50);
        assert!(w.location_at((pos.0 + 40, pos.1)) < 100 || w.location_at((pos.0 - 40, pos.1)) < 100);
        assert_eq!(
            w.location_at((pos.0 + 80, pos.1 + 80))
                .max(w.location_at((pos.0 - 80, pos.1 - 80))),
            100
        );

        w.damage.amount = 300;
        let mut tick = Tick::default();
        w.oil_slick_end(&mut tick);
        assert_eq!(oil(&w), []);
        for at in spots {
            assert_eq!(w.island.layer_551c.at(at), 0xff);
        }
        assert_eq!(w.location_at(pos), 100);
        assert_eq!(w.finances.money[0], 300);
        assert_eq!(w.book_value(crate::accounts::BOOK_OTHER), 300);
        assert!(tick.books);
    }

    #[test]
    fn oil_slick_event() {
        let slick = Event {
            start: 5,
            end: 40,
            ..event(2)
        };
        let mut w = sea();
        w.events = vec![slick];
        w.days = 2;
        let tick = day(&mut w);
        assert!(w.oil_slick);
        assert_eq!(tick.messages[0].number, 5);
        assert_eq!(tick.messages[0].pos, Some(w.damage.slick));
        assert_ne!(oil(&w), Vec::<Pos>::new());
        for _ in 0..2 {
            day(&mut w);
        }
        let tick = day(&mut w);
        assert!(!w.oil_slick);
        assert_eq!(tick.messages[0].number, 40);
        assert_eq!(oil(&w), []);
    }

    #[test]
    fn oil_slick_nowhere() {
        // No sea next to land: 1001 tries, then the last one's square,
        // with no oil.
        let mut w = sea();
        for i in 60..190 {
            for j in 60..190 {
                w.island.ground.set((i, j), 2);
            }
        }
        let pos = w.oil_slick_start(&mut Tick::default());
        let mut r = Random::new(31);
        let mut last = (0, 0);
        for _ in 0..=OIL_TRIES {
            last = (r.random(250).cast_signed(), r.random(250).cast_signed());
        }
        assert_eq!(pos, last);
        assert_eq!(w.random, r);
        assert_eq!(oil(&w), []);
    }

    #[test]
    fn scenario_rows() {
        let mut w = world(vec![event(1); 2]);
        w.load_event_rows(&[[7; 12]]);
        assert_eq!(
            w.events[0],
            Event {
                pos: (30, 40),
                ..Event::from_row(&[7; 12])
            }
        );
        assert_eq!(w.events[1], event(1));
    }

    #[test]
    fn from_the_file() {
        let row = [1000, 435, 505, 0, 30, 80000, 9, 10, 11, 0, 1, 0];
        let e = Event::from_row(&row);
        assert_eq!(
            e,
            Event {
                chance: 1000,
                first_day: 435,
                last_day: 505,
                running: 0,
                duration: 30,
                value: 80000,
                start: 9,
                end: 10,
                column_8: 11,
                building: 0,
                kind: 1,
                arg: 0,
                pos: (0, 0),
            }
        );
    }
}
