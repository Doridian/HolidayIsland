//! The accounting's yearly parts (the `ABRECHNE` unit, segment `1008`):
//! the new year's renovations and debt limit, and on the 2nd of January
//! whether the scenario is won or lost. Also what those read: a player's
//! net worth, the score and the money menu's books.

use crate::simulation::{GameOver, Tick};
use crate::world::{FIRST_BOOK, World};

/// The books' kinds of this year (`GELDMENU.TXT`'s lines): the takings
/// of the houses, hotels, restaurants (`Gaststaetten`) and other
/// buildings, then their costs, new buildings, advertising, interest and everything else (`Sonstiges`). Last
/// year's are 13 lower. // 1070:1419
pub const BOOK_HOUSE_RENT: u8 = 0x5f;
pub const BOOK_HOTEL_RENT: u8 = 0x60;
pub const BOOK_RESTAURANT_TAKINGS: u8 = 0x61;
pub const BOOK_FACILITY_TAKINGS: u8 = 0x62;
pub const BOOK_HOUSE_COSTS: u8 = 0x63;
pub const BOOK_HOTEL_COSTS: u8 = 0x64;
pub const BOOK_RESTAURANT_COSTS: u8 = 0x65;
pub const BOOK_FACILITY_COSTS: u8 = 0x66;
/// New buildings: their prices. // 1078:1d21
pub const BOOK_NEW_BUILDINGS: u8 = 0x67;
pub const BOOK_ADVERTISING: u8 = 0x68;
pub const BOOK_INTEREST: u8 = 0x69;
/// `Sonstiges`: the original books demolitions, renovations, insurance
/// and the cost of living here.
pub const BOOK_OTHER: u8 = 0x6a;
/// The books' last year's total. // 1070:269d
const BOOK_LAST_YEAR: u8 = 0x5e;
/// The books' first kind of this year. // 1070:269d
const BOOK_THIS_YEAR: u8 = 0x5f;

/// The books' monthly history (`*[1110:6f04]+0x24b`) is four rings of
/// this size, one after the other: 120 months of a `Longint` for each of
/// 8 players (`0x20` bytes a month), then the highest and lowest value
/// (`Longint`s at `+0xf00`, `+0xf04`, the scale of the money menu's
/// charts), the number of months (`+0xf08`) and where they start
/// (`+0xf09`). // 1070:210a
const RING: usize = 0xf0a;
/// The rings: each player's money less their loan, net worth, guests and
/// reputation. // 1070:210a
const RING_CASH: usize = 0;
const RING_WORTH: usize = RING;
const RING_GUESTS: usize = 2 * RING;
const RING_REPUTATION: usize = 3 * RING;
/// A ring's highest and lowest value, number of months and first month.
const RING_MAX: usize = 0xf00;
const RING_MIN: usize = 0xf04;
const RING_MONTHS: usize = 0xf08;
const RING_START: usize = 0xf09;
/// The months a ring holds.
const RING_LEN: usize = 120;

/// Where the guests' ring keeps each player's `Longint`s. // 1070:1345
const HISTORY: usize = RING_GUESTS;
/// The guests' ring's number of months and first month. // 1070:1345
const HISTORY_MONTHS: usize = RING_GUESTS + RING_MONTHS;
const HISTORY_START: usize = RING_GUESTS + RING_START;

/// What a player whose money can't be read has: when the money is frozen,
/// or for a player not in the game. // 1078:0191
const NO_MONEY: i32 = 99_999_999;

/// The building type scenario 5 counts. // 1008:00a2
const SCENARIO_5_BUILDING: u8 = 0x6d;

impl World {
    /// The new year: the books' year ends (1070:269d), every player's
    /// buildings are renovated if the year is a multiple of their interval
    /// ([`World::renovation_years`]; the human pays for it), and the debt
    /// limit goes up by 100 000, 50 000 or 10 000 by the difficulty.
    /// // 1008:4f64
    ///
    /// The original then shows the money menu's new year if the player's
    /// option `1110:0064` says so (1070:2b11), the view's part
    /// (`crate::optionen::Settings::year_balance`, `Tick::new_year`).
    pub(crate) fn year_end(&mut self) {
        self.close_books_year();
        let year = self.date.year.cast_signed();
        for n in 0..self.houses.len() {
            let h = &self.houses[n];
            let (pos, building, condition) = (h.pos(), h.building(), h.condition());
            if self.renovation_due(pos, year) {
                self.renovate(pos, building, condition);
                self.houses[n].set_condition(0x7f);
            }
        }
        for n in 0..self.facilities.len() {
            let f = &self.facilities[n];
            let (pos, building, condition) = (f.pos(), f.building(), f.condition());
            if self.renovation_due(pos, year) {
                self.renovate(pos, building, condition);
                self.facilities[n].set_condition(0x7f);
            }
        }
        let f = &mut self.finances;
        match self.difficulty {
            0 => f.max_debt = f.max_debt.wrapping_add(100_000),
            1 => f.max_debt = f.max_debt.wrapping_add(50_000),
            2 => f.max_debt = f.max_debt.wrapping_add(10_000),
            _ => {}
        }
    }

    /// Whether the building at `pos` is renovated this `year`: its owner
    /// (the square's) has an interval, and the year is a multiple of it.
    /// // 1008:4f64
    fn renovation_due(&self, pos: crate::world::Pos, year: i16) -> bool {
        let owner = self.island.owners.get(pos).unwrap_or(0);
        // `1110:1333 + owner`: 1110:1333 is 0, and past player 8 the
        // original reads the bytes after the table.
        let years = usize::from(owner)
            .checked_sub(1)
            .and_then(|i| self.renovation_years.get(i))
            .copied()
            .unwrap_or(0);
        years != 0 && year % i16::from(years) == 0
    }

    /// The human pays for renovating their building at `pos` (marked:
    /// the original only charges player 1; the computer players' buildings
    /// are renovated for free). Single human: player 1; here every human
    /// seat pays for theirs ([`World::is_human`]). // 1008:4f64
    fn renovate(&mut self, pos: crate::world::Pos, building: u8, condition: u8) {
        // BUG(original): only the human pays for renovations.
        let owner = self.island.owners.get(pos).unwrap_or(0);
        if owner != 0 && self.is_human(owner) {
            let cost = self.renovation_price(building, condition);
            self.add_money(owner, cost.wrapping_neg());
            self.book_for(owner, BOOK_OTHER, cost.wrapping_neg());
        }
    }

    /// What renovating a `building` in `condition` costs: `(127 -
    /// condition) * Renovierungskosten * price div 1000`. // 1008:0660
    #[must_use]
    pub fn renovation_price(&self, building: u8, condition: u8) -> i32 {
        (0x7f - i32::from(condition))
            .wrapping_mul(self.renovation_cost)
            .wrapping_mul(self.tables.hausprei(building, 0))
            / 1000
    }

    /// The scenario's check on the 2nd of January, until it's won: won, the
    /// game is over with a score if the player didn't cheat and the money
    /// isn't frozen; lost, it's over without one. The view shows the end
    /// (`schlussform`, 1068:1016) from [`Tick::game_over`]. // 1008:0512
    ///
    /// Single human: the original checks player 1. With several human
    /// seats a normal game (scenario 0) checks each of them as it does
    /// player 1 ([`World::january_second_seats`]); scenarios 1 to 13
    /// still check player 1 only.
    pub(crate) fn january_second(&mut self, tick: &mut Tick) {
        if self.won {
            return;
        }
        // BUG(original): only player 1 wins, loses and goes bankrupt (design issue for multiplayer: each human seat in the normal game, decision 9; scenarios 1 to 13 are phase 3).
        if self.scenario == 0 && self.seats.multiplayer() {
            self.january_second_seats(tick);
            return;
        }
        if self.scenario_won() {
            self.won = true;
            let score = (!self.cheated && !self.finances.frozen).then(|| self.score()); // 1010:0faa
            tick.game_over = Some(GameOver {
                seat: 1,
                won: true,
                score,
                net_worth: self.net_worth(1),
            });
        } else if self.scenario_lost() {
            self.won = false;
            tick.game_over = Some(GameOver {
                seat: 1,
                won: false,
                score: None,
                net_worth: self.net_worth(1),
            });
        }
    }

    /// The normal game's check with several human seats (decision 9):
    /// each seat a human plays is checked as the original checks player 1
    /// (1008:0297, 1008:042b). In 2056 every seat still in has won: the
    /// first of them ends the game for everyone ([`Tick::game_over`],
    /// with its results), and every seat gets its own
    /// ([`Tick::seat_results`]). Else a seat that's bankrupt
    /// ([`World::bankrupt`], with player 1's rule of last year's books)
    /// is out, its results lost, and its clients spectate
    /// ([`World::spectates`]); the game goes on while any human seat is
    /// in, and ends lost for everyone when none is. A cheat anywhere
    /// takes everyone's score ([`World::cheated`], decision 10). The
    /// clock goes on after the end (decision 8). // 1008:0512
    fn january_second_seats(&mut self, tick: &mut Tick) {
        let seats: Vec<u8> = self.human_seats().collect();
        let fair = !self.cheated && !self.finances.frozen;
        let result = |w: &World, seat: u8, won: bool| GameOver {
            seat,
            won,
            score: (won && fair).then(|| w.score_of(seat)), // 1010:0faa
            net_worth: w.net_worth(seat),
        };
        // A normal game is won in 2056.
        if self.date.year == 0x808 {
            let winners: Vec<u8> = seats.iter().copied().filter(|&p| self.player_out(p) == 0).collect();
            if let Some(&first) = winners.first() {
                self.won = true;
                for &p in &winners {
                    tick.seat_results.push(result(self, p, true));
                }
                tick.game_over = Some(result(self, first, true));
                return;
            }
        }
        let mut went_out = false;
        for &p in &seats {
            if self.bankrupt(p) {
                self.set_player_out(p, 1);
                went_out = true;
                tick.seat_results.push(result(self, p, false));
            }
        }
        if went_out && seats.iter().all(|&p| self.player_out(p) != 0) {
            tick.game_over = Some(GameOver {
                seat: 0,
                won: false,
                score: None,
                net_worth: 0,
            });
        }
    }

    /// Whether the human (marked: player 1) has reached the scenario's
    /// goal. // 1008:0297, en 1008:0555
    fn scenario_won(&self) -> bool {
        match self.scenario {
            // A normal game is won in 2056.
            0 => self.date.year == 0x808,
            1 => self.net_worth(1) >= 50_000_000,
            2 => self.guests_last_months(1) >= 120_000,
            // Monopolus: everyone else is out.
            3 => self.player_out(1) == 0 && (2..=8).all(|p| self.player_out(p) != 0),
            4 => self.luxury_guests(1) >= 4000,
            5 => self.facility_sum(1) >= 4000,
            6 => self.guests_last_months(1) >= 12_000 && self.plot_average() > 0x46,
            // The tutorial's number, not a scenario on the start menu.
            7 => self.date.year >= 2001 || self.net_worth(1) >= 1_000_000,
            // The later build's scenarios (`UPGRADE\`), en 1008:0555:
            // the islands joined into one,
            8 => self.islands_joined(),
            // 5000 campers,
            9 => self.camp_site_guests(1) >= 5000,
            // B&B only: the year 2021,
            10 => self.date.year == 2021,
            // the wonders 80 % full,
            11 => self.wonders_full(1),
            // hurry up: the year 2001,
            12 => self.date.year == 2001,
            // and the most guests on every island.
            13 => self.most_guests_everywhere(),
            // BUG(original): an unset result for another scenario; the port says false.
            _ => false,
        }
    }

    /// Whether exactly one plot is bigger than 41 squares: the islands are
    /// joined (scenario 8). // en 1008:0317
    fn islands_joined(&self) -> bool {
        self.plots.iter().filter(|p| p.area() > 0x29).count() == 1
    }

    /// The guests of `player`'s small camp sites (type 0), scenario 9's
    /// campers. // en 1008:0134
    fn camp_site_guests(&self, player: u8) -> i32 {
        self.houses
            .iter()
            .filter(|h| h.building() == 0 && self.island.owners.get(h.pos()) == Some(player))
            .fold(0i32, |sum, h| sum.wrapping_add(i32::from(h.guests())))
    }

    /// Whether more than 8 of `player`'s buildings of the reputation types
    /// (the wonders, `1110:1458`) are at least 80 % full (scenario 11).
    /// // en 1008:036e
    fn wonders_full(&self, player: u8) -> bool {
        self.facilities
            .iter()
            .filter(|f| {
                crate::construction::REPUTATION_BUILDINGS.contains(&f.building())
                    && f.fill() > 0x4f
                    && self.island.owners.get(f.pos()) == Some(player)
            })
            .count()
            > 8
    }

    /// Whether on every plot the human's (marked: player 1's) houses have
    /// more guests than half of the plot's (`+0xee` div 2), scenario 13.
    /// The houses' guests are added up by the plot of their square, in a
    /// table of 3001; the plots are looked at in their list's order until
    /// one fails. // en 1008:03fc
    fn most_guests_everywhere(&self) -> bool {
        let mut sums = vec![0i32; 3001];
        for h in &self.houses {
            if self.island.owners.get(h.pos()) == Some(1)
                && let Some(s) = sums.get_mut(usize::from(self.island.plot_at(h.pos())))
            {
                *s = s.wrapping_add(i32::from(h.guests()));
            }
        }
        // BUG(original): a plot number outside 0..3000 reads the stack around the table; the port reads 0.
        self.plots.iter().all(|p| {
            let mine = usize::try_from(p.id())
                .ok()
                .and_then(|i| sums.get(i))
                .copied()
                .unwrap_or(0);
            p.longint(crate::settlement::plot::GUESTS) / 2 < mine
        })
    }

    /// Whether the human (player 1) has lost: bankrupt in a normal game
    /// (which also marks them out), or the scenario's last year has come.
    /// // 1008:042b, en 1008:075f
    fn scenario_lost(&mut self) -> bool {
        match self.scenario {
            // The normal game, the tutorial's 7 and the later build's 10
            // and 12 (en 1008:075f).
            0 | 7 | 10 | 12 => {
                let lost = self.bankrupt(1);
                if lost {
                    self.set_player_out(1, 1);
                }
                lost
            }
            1 | 2 | 3 | 6 => self.date.year == 2017,
            4 => self.date.year == 2027,
            5 => self.date.year == 2007,
            // The later build's scenarios, en 1008:075f.
            8 => self.date.year == 2016,
            9 => self.date.year == 2011,
            11 | 13 => self.date.year == 2026,
            _ => false,
        }
    }

    /// Whether `player` is out of the game. // *1110:6f24 + 0x7f + player
    #[must_use]
    pub fn player_out(&self, player: u8) -> u8 {
        self.player_block.get(0x7f + usize::from(player)).copied().unwrap_or(0)
    }

    pub fn set_player_out(&mut self, player: u8, out: u8) {
        if let Some(b) = self.player_block.get_mut(0x7f + usize::from(player)) {
            *b = out;
        }
    }

    /// Whether `player` (still in the game) has gone bankrupt: their net
    /// worth is below minus the debt limit, and for the human, last year's
    /// books were in the red too. Single human: player 1; here every
    /// human seat, by its own books ([`World::is_human`]). // 1008:0002
    #[must_use]
    pub fn bankrupt(&self, player: u8) -> bool {
        if self.player_out(player) != 0 {
            return false;
        }
        let broke = self.finances.max_debt.wrapping_neg() > self.net_worth(player);
        if player == 1 || self.is_human(player) {
            broke && self.book_value_of(player, BOOK_LAST_YEAR) < 0 // 1070:13f1
        } else {
            broke
        }
    }

    /// `player`'s money, or 99 999 999 while the money is frozen and for a
    /// player not in the game. // 1078:0191
    pub(crate) fn money_of(&self, player: u8) -> i32 {
        if self.finances.frozen || player == 0 || player > self.player_count {
            return NO_MONEY;
        }
        self.finances.money[usize::from(player - 1)]
    }

    /// What `player` owns: their buildings and building sites' value, at
    /// least 0, and their money less their loan, saturating at the top.
    /// // 1008:0c6e
    #[must_use]
    pub fn net_worth(&self, player: u8) -> i32 {
        let assets = self.assets(player).max(0); // 1030:0ee7
        // 1078:057a reads the loan; player 0 would read the money of 8.
        let loan = usize::from(player)
            .checked_sub(1)
            .and_then(|i| self.finances.loan.get(i))
            .copied()
            .unwrap_or(0);
        let cash = self.money_of(player).wrapping_sub(loan);
        if cash > 0 && i32::MAX.wrapping_sub(cash).wrapping_sub(assets) < 0 {
            i32::MAX
        } else {
            assets.wrapping_add(cash)
        }
    }

    /// The value of `player`'s houses, other buildings and building sites
    /// (their price). // 1008:0b0b
    pub(crate) fn assets(&self, player: u8) -> i32 {
        let owned = |pos| self.island.owners.get(pos) == Some(player);
        let mut sum = 0i32;
        for h in self.houses.iter().filter(|h| owned(h.pos())) {
            sum = sum.wrapping_add(self.building_value(h.building(), h.stars(), h.condition()));
        }
        for f in self.facilities.iter().filter(|f| owned(f.pos())) {
            sum = sum.wrapping_add(self.building_value(f.building(), f.stars(), f.condition()));
        }
        for s in self.building_sites.iter().filter(|s| owned(s.pos)) {
            sum = sum.wrapping_add(self.tables.hausprei(s.building, 0));
        }
        sum
    }

    /// A building's value: its price times `stars` over `HAUSDATA.DAT`'s
    /// column 4 if that's positive, else times its condition over 127.
    /// // 1008:0a56
    fn building_value(&self, building: u8, stars: u8, condition: u8) -> i32 {
        let price = self.tables.hausprei(building, 0);
        let column_4 = i32::from(self.tables.hausdata(building, 4));
        if column_4 > 0 {
            price.wrapping_mul(i32::from(stars)) / column_4
        } else {
            price.wrapping_mul(i32::from(condition)) / 0x7f
        }
    }

    /// The `+0x10` values (guests?) of `player`'s houses whose price is
    /// over 320. // 1008:0134
    fn luxury_guests(&self, player: u8) -> i32 {
        self.houses
            .iter()
            .filter(|h| self.island.owners.get(h.pos()) == Some(player) && h.price() > 320)
            .fold(0i32, |sum, h| sum.wrapping_add(i32::from(h.guests())))
    }

    /// The `+0xc` values of `player`'s buildings of type 0x6d.
    /// // 1008:00a2
    fn facility_sum(&self, player: u8) -> i32 {
        self.facilities
            .iter()
            .filter(|f| f.building() == SCENARIO_5_BUILDING && self.island.owners.get(f.pos()) == Some(player))
            .fold(0i32, |sum, f| sum.wrapping_add(f.visitors()))
    }

    /// The plots' `+4` values averaged by their weights, rounded. The
    /// original divides by zero without plots; the port says 0.
    /// // 1008:01d2
    fn plot_average(&self) -> i32 {
        // BUG(original): without plots this divides by zero; the port says 0.
        let (mut sum, mut weights) = (0i32, 0i32);
        for p in &self.plots {
            let w = i32::from(p.area());
            sum = sum.wrapping_add(w.wrapping_mul(i32::from(p.quality())));
            weights = weights.wrapping_add(w);
        }
        if weights == 0 {
            return 0;
        }
        (weights / 2).wrapping_add(sum) / weights
    }

    /// The human's score, 0–100: the mean of their net worth in 90 000s,
    /// their last twelve months' guests in 1800s and their reputation, each
    /// at most 100; lowered by 10 for a game played without disasters
    /// ([`World::played_without_disasters`], `1110:133c`) and on the
    /// easier levels (by 10, 20 on easy), but never below half. // 1010:0faa
    #[must_use]
    pub fn score(&self) -> i32 {
        self.score_of(1)
    }

    /// [`World::score`] of the human in seat `player`. Single human:
    /// player 1's.
    #[must_use]
    pub fn score_of(&self, player: u8) -> i32 {
        let worth = (self.net_worth(player) / 90_000).clamp(0, 100);
        let guests = (self.guests_last_months(player) / 1800).clamp(0, 100);
        let reputation = i32::from(self.reputation(player)).min(100);
        let mut score = (worth.wrapping_add(guests).wrapping_add(reputation) / 3).clamp(0, 100);
        let lower = |s: i32, by: i32| (s - by).max(0).max(s / 2);
        if self.played_without_disasters {
            score = lower(score, 10);
        }
        match self.difficulty {
            0 => lower(score, 20),
            1 => lower(score, 10),
            _ => score,
        }
    }

    /// Adds `amount` to the books' `kind` (marked: the books are the
    /// human's, player 1's; [`World::book_for`] a human seat's). The
    /// original writes any field of the money menu for other kinds; the
    /// port only has 0x52–0x6e. // 1070:1419
    pub fn book(&mut self, kind: u8, amount: i32) {
        // BUG(original): another kind writes some other field of the money menu; the port writes nothing.
        self.book_for(1, kind, amount);
    }

    /// The books' `kind`, 0 outside 0x52–0x6e: player 1's
    /// ([`World::book_value_of`]). // 1070:13f1
    #[must_use]
    pub fn book_value(&self, kind: u8) -> i32 {
        self.book_value_of(1, kind)
    }

    /// The books' year ends: this year's total and the balance (1070:1452)
    /// and sums become last year's, and this year's start at 0. // 1070:269d
    ///
    /// The original also redraws the money menu (1070:1eeb), and works
    /// the total out again if it's shown (1070:2617), which gives 0.
    /// Single human: player 1's books; here each human seat's.
    fn close_books_year(&mut self) {
        let seats: Vec<u8> = self.human_seats().collect();
        for p in seats {
            self.geldmenu_year_total(p); // 1070:1452
            let Some(y) = self.books_of_mut(p) else { continue };
            let this = usize::from(BOOK_THIS_YEAR - FIRST_BOOK);
            for k in 0..13 {
                y[k] = y[this + k];
                y[this + k] = 0;
            }
        }
    }

    /// `player`'s guests in the last twelve months (or fewer, as many as
    /// there are) of the books' history. // 1070:1345
    #[must_use]
    pub fn guests_last_months(&self, player: u8) -> i32 {
        let h = &self.books.history;
        let months = h.get(HISTORY_MONTHS).copied().unwrap_or(0);
        let start = h.get(HISTORY_START).copied().unwrap_or(0);
        let mut sum = 0i32;
        for k in 1..=months.min(12) {
            let month = (usize::from(months) + usize::from(start) - usize::from(k)) % 120;
            let at = (HISTORY + month * 0x20 + usize::from(player) * 4).wrapping_sub(4);
            if let Some(b) = h.get(at..at + 4) {
                sum = sum.wrapping_add(i32::from_le_bytes([b[0], b[1], b[2], b[3]]));
            }
        }
        sum
    }

    /// The month's end in the books' history: each player's money less
    /// their loan, net worth, guests ([`Finances::guests`]) and reputation
    /// go into the next month of their rings, the first two widening
    /// their rings' scale. A full ring drops its first month. Run by the
    /// settlement's step 0x8b. // 1070:210a
    ///
    /// [`Finances::guests`]: crate::money::Finances::guests
    pub(crate) fn close_books_month(&mut self) {
        for p in 1..=self.player_count {
            let loan = self.finances.loan.get(usize::from(p) - 1).copied().unwrap_or(0); // 1078:057a
            let cash = self.money_of(p).wrapping_sub(loan);
            self.ring_put(RING_CASH, p, cash);
            self.widen_scale(RING_CASH, cash);
        }
        self.ring_advance(RING_CASH);
        for p in 1..=self.player_count {
            let worth = self.net_worth(p);
            self.ring_put(RING_WORTH, p, worth);
            self.widen_scale(RING_WORTH, worth);
        }
        self.ring_advance(RING_WORTH);
        for p in 1..=self.player_count {
            let guests = self.finances.guests(p); // 1078:014c
            self.ring_put(RING_GUESTS, p, guests);
            // The scale only goes up, to an even number.
            let top = self.ring_longint(RING_GUESTS + RING_MAX).max(guests);
            self.ring_set_longint(RING_GUESTS + RING_MAX, top.wrapping_add(1) / 2 * 2);
        }
        self.ring_advance(RING_GUESTS);
        for p in 1..=self.player_count {
            let reputation = i32::from(self.reputation(p));
            self.ring_put(RING_REPUTATION, p, reputation);
        }
        self.ring_advance(RING_REPUTATION);
    }

    /// The `Longint` at `at` in the history, 0 past it.
    fn ring_longint(&self, at: usize) -> i32 {
        self.books
            .history
            .get(at..at + 4)
            .map_or(0, |b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn ring_set_longint(&mut self, at: usize, value: i32) {
        if let Some(b) = self.books.history.get_mut(at..at + 4) {
            b.copy_from_slice(&value.to_le_bytes());
        }
    }

    /// Writes `player`'s `value` into the month after the last of the
    /// ring at `ring`. // 1070:210a
    fn ring_put(&mut self, ring: usize, player: u8, value: i32) {
        let h = &self.books.history;
        let months = usize::from(h.get(ring + RING_MONTHS).copied().unwrap_or(0));
        let start = usize::from(h.get(ring + RING_START).copied().unwrap_or(0));
        let month = (months + start) % RING_LEN;
        let at = (ring + month * 0x20 + usize::from(player) * 4).wrapping_sub(4);
        self.ring_set_longint(at, value);
    }

    /// The ring at `ring` holds one more month, or drops its first one
    /// once it has 120. // 1070:210a
    fn ring_advance(&mut self, ring: usize) {
        let h = &mut self.books.history;
        if let Some(&months) = h.get(ring + RING_MONTHS) {
            if usize::from(months) < RING_LEN {
                h[ring + RING_MONTHS] = months + 1;
            } else if let Some(start) = h.get_mut(ring + RING_START) {
                *start = u8::try_from((usize::from(*start) + 1) % RING_LEN).unwrap_or(0);
            }
        }
    }

    /// The scale of the ring at `ring` takes in `value`, and stays as far
    /// below 0 as above. // 1070:210a
    fn widen_scale(&mut self, ring: usize, value: i32) {
        let mut top = self.ring_longint(ring + RING_MAX).max(value);
        let mut bottom = self.ring_longint(ring + RING_MIN).min(value);
        top = top.max(bottom.wrapping_neg());
        bottom = top.wrapping_neg().min(bottom);
        self.ring_set_longint(ring + RING_MAX, top);
        self.ring_set_longint(ring + RING_MIN, bottom);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::world::{BuildingSite, Facility, House, Tables};

    fn world() -> World {
        let mut hausprei = vec![[0; 4]; 127];
        hausprei[40] = [10_000, 0, 0, 0];
        hausprei[41] = [12_700, 0, 0, 0];
        let mut hausdata = vec![[0; 10]; 127];
        hausdata[40][4] = 4;
        World {
            tables: Arc::new(Tables {
                hausnr: Vec::new(),
                hausdata,
                hausprei,
                groundnr: Vec::new(),
                ..Tables::default()
            }),
            player_count: 2,
            renovation_cost: 3,
            ..World::default()
        }
    }

    #[test]
    fn renovation() {
        let mut w = world();
        w.renovation_years = [2, 3, 0, 0, 0, 0, 0, 0];
        w.date.year = 2000;
        let mut mine = House::new((10, 10));
        mine.set_building(41);
        mine.set_condition(100);
        w.island.owners.set((10, 10), 1);
        let mut f = Facility::new(41, (20, 20));
        f.set_condition(7);
        w.island.owners.set((20, 20), 2);
        w.houses = vec![mine, House::new((30, 30))];
        w.facilities = vec![f];
        w.year_end();
        // 27 steps of condition at 3 per 1000 of 12 700.
        assert_eq!(w.finances.money[0], -(27 * 3 * 12_700 / 1000));
        assert_eq!(w.book_value(BOOK_LAST_YEAR), 0);
        assert_eq!(w.book_value(BOOK_OTHER), -1028);
        assert_eq!(w.houses[0].condition(), 0x7f);
        // Player 2's every three years: not in 2000.
        assert_eq!(w.facilities[0].condition(), 7);
        assert_eq!(w.finances.max_debt, 100_000);
        w.date.year = 2001;
        w.difficulty = 2;
        w.houses[0].set_condition(50);
        w.year_end();
        assert_eq!(w.houses[0].condition(), 50);
        // The computer player's renovation is free.
        assert_eq!(w.facilities[0].condition(), 0x7f);
        assert_eq!(w.finances.money[1], 0);
        assert_eq!(w.book_value(BOOK_LAST_YEAR), -1028);
        assert_eq!(w.book_value(0x5d), -1028);
        assert_eq!(w.book_value(BOOK_OTHER), 0);
        assert_eq!(w.finances.max_debt, 110_000);
    }

    #[test]
    fn net_worth() {
        let mut w = world();
        w.finances.money = [5000, 0, 0, 0, 0, 0, 0, 0];
        w.finances.loan = [2000, 0, 0, 0, 0, 0, 0, 0];
        assert_eq!(w.net_worth(1), 3000);
        let mut h = House::new((1, 1));
        h.set_building(40);
        h.set_stars(2);
        w.houses.push(h);
        w.island.owners.set((1, 1), 1);
        let mut f = Facility::new(41, (2, 2));
        f.set_condition(10);
        w.facilities.push(f);
        w.island.owners.set((2, 2), 1);
        w.building_sites.push(BuildingSite {
            pos: (3, 3),
            building: 40,
            ..BuildingSite::default()
        });
        w.island.owners.set((3, 3), 1);
        assert_eq!(w.net_worth(1), 3000 + 5000 + 1000 + 10_000);
        w.finances.money[0] = i32::MAX;
        assert_eq!(w.net_worth(1), i32::MAX);
        w.finances.frozen = true;
        assert_eq!(w.net_worth(1), NO_MONEY - 2000 + 16_000);
    }

    #[test]
    fn bankrupt_and_lost() {
        let mut w = world();
        w.finances.max_debt = 1000;
        w.finances.money[0] = -1001;
        assert!(!w.bankrupt(1));
        w.books.year[usize::from(BOOK_LAST_YEAR - FIRST_BOOK)] = -1;
        assert!(w.bankrupt(1));
        w.finances.money[1] = -1001;
        assert!(w.bankrupt(2));
        let mut tick = Tick::default();
        w.january_second(&mut tick);
        assert_eq!(
            tick.game_over,
            Some(GameOver {
                seat: 1,
                won: false,
                score: None,
                net_worth: -1001
            })
        );
        assert_eq!(w.player_out(1), 1);
        assert!(!w.bankrupt(1));
    }

    #[test]
    fn won_with_a_score() {
        let mut w = world();
        w.scenario = 1;
        w.difficulty = 2;
        w.finances.money[0] = 50_000_000;
        w.set_reputation(1, 50);
        let mut tick = Tick::default();
        w.january_second(&mut tick);
        // (100 + 0 + 50) / 3.
        assert_eq!(tick.game_over.map(|g| (g.won, g.score)), Some((true, Some(50))));
        assert!(w.won);
        // Only once.
        let mut tick = Tick::default();
        w.january_second(&mut tick);
        assert_eq!(tick.game_over, None);
        w.difficulty = 0;
        assert_eq!(w.score(), 30);
        w.cheated = true;
        w.won = false;
        w.january_second(&mut tick);
        assert_eq!(tick.game_over.map(|g| g.score), Some(None));
    }

    #[test]
    fn history() {
        let mut w = world();
        let h = &mut w.books.history;
        h[HISTORY_MONTHS] = 14;
        h[HISTORY_START] = 110;
        for month in 0..120 {
            let at = HISTORY + month * 0x20 + 4;
            h[at..at + 4].copy_from_slice(&i32::try_from(month).unwrap().to_le_bytes());
        }
        // Months 123 down to 112, wrapping round the ring.
        let expected: i32 = (112..124).map(|m| m % 120).sum();
        assert_eq!(w.guests_last_months(2), expected);
        assert_eq!(w.scenario, 0);
        assert!(!w.scenario_won());
    }
}
