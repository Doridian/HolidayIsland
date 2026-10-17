//! The monthly settlement (the `ABRECHNE` unit, segment `1008`):
//! `settlement_step` (1008:1bf7) does one part of the month's books for
//! everyone each time the accounting scheduler (1008:5307) calls it, by
//! the scheduler's step, five steps a day. `docs/settlement.md` has the
//! steps in order.
//!
//! The ratings come from `ABRECHNE.DAT`'s 38 factors (saved with the map):
//! [`Settlement::scale`] maps a value onto 0–100, [`Settlement::weigh`]
//! lets a rating move a running value by the factor's weight, and
//! [`Settlement::approach`] moves a value part of the way towards another.
//! All of it is `Longint` arithmetic (`sys_long_mul`, `sys_long_div`:
//! 32-bit products, division towards zero), no floating point. Where the
//! original divides by zero it stops with runtime error 200; the port's
//! divisions give 0 instead ([`div`], [`rem`]).

use crate::accounts::{
    BOOK_ADVERTISING, BOOK_FACILITY_COSTS, BOOK_FACILITY_TAKINGS, BOOK_HOTEL_COSTS, BOOK_HOTEL_RENT, BOOK_HOUSE_COSTS,
    BOOK_HOUSE_RENT, BOOK_INTEREST, BOOK_OTHER, BOOK_RESTAURANT_COSTS, BOOK_RESTAURANT_TAKINGS,
};
use crate::construction::{EMPTY_CHAMPIONSHIPS, FULL_CHAMPIONSHIPS};
use crate::simulation::Tick;
use crate::world::{Plot, Pos, World, on_map};

/// The number of `ABRECHNE.DAT`'s factors. // 1008:55ee
pub const FACTORS: usize = 38;

/// The number of `Longint`s from `1110:4aea` on: twelve sums of eight
/// players, and the one before them ([`Settlement::sums`]).
pub const SUMS: usize = 97;

/// The month's sums, `array[1..8] of Longint` each, at `1110:4aea + 0x20
/// · sum` ([`Settlement::sums`]): what the houses (`HAUSDATA.DAT` kind 0)
/// and hotels (kind 1) take in rent, what restaurants (kind 2) and the
/// other buildings take, the four's running costs (`HAUSPREI.DAT` column
/// 1), advertising, the rest (insurance and the cost of living, as a
/// negative amount), interest, and the month's total.
pub const HOUSE_RENT: usize = 0;
pub const HOTEL_RENT: usize = 1;
pub const RESTAURANT_TAKINGS: usize = 2;
pub const FACILITY_TAKINGS: usize = 3;
pub const HOUSE_COSTS: usize = 4;
pub const HOTEL_COSTS: usize = 5;
pub const RESTAURANT_COSTS: usize = 6;
pub const FACILITY_COSTS: usize = 7;
pub const ADVERTISING: usize = 8;
pub const OTHER: usize = 9;
pub const INTEREST: usize = 10;
pub const MONTH: usize = 11;

/// `ABRECHNE.DAT`'s factors by their headings, the index of each row.
mod factor {
    /// The stars of houses and hotels: condition (`Verfall`), then
    /// connection and location in the next two rows.
    pub const HOUSE_CONDITION: usize = 0;
    /// Their surroundings (`Umgebungsqualitaet`): the plot's quality, its
    /// quality for the house's class of rent, reputation, advertising,
    /// the economy (`Wirtschaft`).
    pub const HOUSE_PLOT: usize = 3;
    pub const HOUSE_PLOT_CLASS: usize = 4;
    pub const HOUSE_REPUTATION: usize = 5;
    pub const HOUSE_ADVERTISING: usize = 6;
    pub const HOUSE_ECONOMY: usize = 7;
    /// How full they get (`Auslastung`): the rent, the surroundings.
    pub const HOUSE_RENT_LEVEL: usize = 8;
    pub const HOUSE_SURROUNDINGS: usize = 9;
    /// The other buildings' stars, like the houses'.
    pub const CONDITION: usize = 10;
    /// The shops' (record kind 1, by class) surroundings: the guests per
    /// place (`Gaestedruck`), the guests of the price class per place
    /// (`KlassenGaestedruck`), reputation, advertising, economy.
    pub const SHOP_PRESSURE: usize = 13;
    pub const SHOP_CLASS_PRESSURE: usize = 14;
    /// The rest's: guests per place, reputation, advertising, economy.
    pub const FACILITY_PRESSURE: usize = 18;
    /// How full they get: the price, the surroundings.
    pub const PRICE_LEVEL: usize = 22;
    pub const SURROUNDINGS: usize = 23;
    /// A plot's quality (`Gesamtqualitaet einer Insel`): greenery, the
    /// shops' prices, then the places for leisure, sport, nature, energy,
    /// water and rubbish per bed.
    pub const PLOT_GREENERY: usize = 24;
    pub const PLOT_PRICES: usize = 25;
    pub const PLOT_FACILITIES: usize = 26;
    /// Its quality by class (`Klassenweise Qualitaet`): the shops' prices,
    /// the restaurants', shops' and rentals' places per bed of the class.
    pub const CLASS_PRICES: usize = 32;
    pub const CLASS_SHOPS: usize = 33;
    /// A player's reputation (`Ruf eines Spielers`): the prices, the
    /// shops' places (`Kommerzplaetze`).
    pub const REPUTATION_PRICES: usize = 36;
    pub const REPUTATION_PLACES: usize = 37;
}

/// The facility classes (1010:2493) whose places count for a plot's
/// quality, with their factor from [`factor::PLOT_FACILITIES`] on:
/// leisure, sport, nature, energy, water, rubbish. // 1008:0f37
const PLOT_FACILITY_CLASSES: [usize; 6] = [4, 3, 5, 7, 8, 9];

/// The facility classes everyone uses, whose visitors are at most the
/// plot's guests: transport, energy, water and rubbish. // 1008:1bf7 step 0x42
const SHARED_CLASSES: [usize; 4] = [6, 7, 8, 9];

/// The rentals: which people they send out (two kinds each, by
/// `Random(2)`) and whether to the sea (the surf school) or the road.
/// // 1008:1bf7 step 0x78
const RENTALS: [(u8, [u8; 2], bool); 4] = [
    (0x1a, [0x11, 0x12], false),
    (0x0a, [0x15, 0x16], false),
    (0x1b, [0x13, 0x14], false),
    (0x6d, [0x20, 0x21], true),
];

/// Where a plot record (`*1110:6f0c`) keeps the settlement's values.
mod plot {
    /// Its greenery, 0–100. // 1008:0f37
    pub const GREENERY: usize = 3;
    /// Its quality, 0–100 ([`crate::world::Plot::quality`]).
    pub const QUALITY: usize = 4;
    /// Its quality for each of the ten classes of rent, bytes.
    pub const CLASS_QUALITY: usize = 5;
    /// How fair its shops' prices are, 0–100.
    pub const PRICES: usize = 0xf;
    /// The same by class of rent: read, but nothing writes it (the
    /// original's loop writes [`PRICES`] ten times instead).
    pub const CLASS_PRICES: usize = 0x10;
    /// The guests its houses would have. // `Longint`s from here on
    pub const DEMAND: usize = 0x1a;
    /// The guests who come: the demand, at most what its airports and
    /// harbours take.
    pub const ADMITTED: usize = 0x1e;
    /// Its beds, then by class of rent.
    pub const BEDS: usize = 0x22;
    pub const CLASS_BEDS: usize = 0x26;
    /// The places of its shops (record kind 1) of classes 0–2, `0x2c`
    /// bytes each: all of them, then by price class.
    pub const SHOPS: usize = 0x4e;
    /// The places of its other buildings (record kind 2), by class.
    pub const FACILITIES: usize = 0xc6;
    /// The places of its airports and harbours (class 6).
    pub const PORTS: usize = FACILITIES + 4 * 6;
    /// Its guests, then by class of rent.
    pub const GUESTS: usize = 0xee;
    /// The guests' wishes by class, after the class mixing.
    pub const WISHES: usize = 0x1ba;
    /// What its shops and other buildings would have, laid out like
    /// [`SHOPS`] and [`FACILITIES`] from here.
    pub const DEMANDS: usize = 0x1ba;
    /// What they get, laid out the same way.
    pub const SERVED: usize = 0x286;
    /// From a block's start ([`DEMANDS`], [`SERVED`], [`BEDS`]): the
    /// shops' classes, and the other buildings'.
    pub const BLOCK_SHOPS: usize = 0x2c;
    pub const BLOCK_FACILITIES: usize = 0xa4;
    /// The size of a shop class's sums: all, then ten price classes.
    pub const SHOP_CLASS: usize = 0x2c;
}

/// `a div b` as the original's `Longint` division (towards zero), 0 for
/// a division by zero (the original stops there).
#[must_use]
pub fn div(a: i32, b: i32) -> i32 {
    if b == 0 { 0 } else { a.wrapping_div(b) }
}

/// `a mod b`, the remainder [`div`] leaves, 0 for a division by zero.
#[must_use]
pub fn rem(a: i32, b: i32) -> i32 {
    if b == 0 { 0 } else { a.wrapping_rem(b) }
}

/// The settlement's tables (`ABRECHNE.DAT` and `MODATYP.DAT`, saved with
/// the map) and the month's sums.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Settlement {
    /// `ABRECHNE.DAT`'s 38 factors, each its weight, divisor, low and
    /// high value. // *1110:4c6e, *1110:4c72, *1110:4c76, *1110:4c7a
    pub factors: Vec<[i32; 4]>,
    /// `MinMiete`, `MaxMiete`: the rents per guest and month the classes
    /// of rent go between, and the difference. // 1110:4a8a, 1110:4a8e,
    /// 1110:4a92
    pub min_rent: i32,
    pub max_rent: i32,
    pub rent_range: i32,
    /// `MinPreis`, `MaxPreis` by facility class (`TKommerzsachen`): the
    /// prices per visitor and day the price classes go between. The map
    /// has 11 of each; the last `MaxPreis` is [`Settlement::sums`]'s
    /// first. // 1110:4a96, 1110:4ac2
    pub min_price: [i32; 11],
    pub max_price: [i32; 10],
    /// The month's sums ([`HOUSE_RENT`] ...) from `1110:4aea`: index `0x8
    /// · sum + player`. Index 0 is the 11th `MaxPreis`, and player 0's
    /// sum is player 8's of the sum before, as in the original, which
    /// adds the buildings of nobody there. // 1110:4aea
    pub sums: [i32; SUMS],
    /// Not known: the base of how full the buildings get, in percent
    /// (with factors 9 and 22). Only ever read from the map. // 1110:004c
    pub base_demand: u8,
    /// `Werbungskosten`: what advertising costs per bed and level.
    /// // 1110:64c6
    pub advertising_cost: i32,
    /// `Lebenshaltungskosten im Monat`: the human's cost of living.
    /// // 1110:64fa
    pub living_costs: i32,
    /// `Klassenmischung` and its divisor: how many of the guests of each
    /// class of rent want what each facility class offers. // *1110:6502,
    /// 1110:64fe
    pub class_mix: [[i16; 10]; 10],
    pub mix_divisor: i32,
    /// `Verfallwahrscheinlichkeit`: the chance in percent that a house's
    /// (0), shop's (1) or other building's (2) condition drops each
    /// month. // 1110:650e
    pub decay: [u8; 3],
}

impl Default for Settlement {
    /// Neutral factors (`1 2 0 100`), no rents or prices.
    fn default() -> Self {
        Settlement {
            factors: vec![[1, 2, 0, 100]; FACTORS],
            min_rent: 0,
            max_rent: 0,
            rent_range: 0,
            min_price: [0; 11],
            max_price: [0; 10],
            sums: [0; SUMS],
            base_demand: 0,
            advertising_cost: 0,
            living_costs: 0,
            class_mix: [[0; 10]; 10],
            mix_divisor: 0,
            decay: [0; 3],
        }
    }
}

impl Settlement {
    /// A factor's weight (0), divisor (1), low (2) or high (3) value.
    fn factor(&self, n: usize, column: usize) -> i32 {
        self.factors.get(n).map_or(0, |f| f[column])
    }

    /// `player`'s month's `sum` ([`HOUSE_RENT`] ...).
    #[must_use]
    pub fn sum(&self, sum: usize, player: u8) -> i32 {
        self.sums.get(sum * 8 + usize::from(player)).copied().unwrap_or(0)
    }

    fn set_sum(&mut self, sum: usize, player: u8, value: i32) {
        if let Some(s) = self.sums.get_mut(sum * 8 + usize::from(player)) {
            *s = value;
        }
    }

    fn add_sum(&mut self, sum: usize, player: u8, by: i32) {
        let v = self.sum(sum, player).wrapping_add(by);
        self.set_sum(sum, player, v);
    }

    /// `MinPreis` of a facility class, 0 past the table.
    fn min_price(&self, class: u8) -> i32 {
        self.min_price.get(usize::from(class)).copied().unwrap_or(0)
    }

    /// `MaxPreis` of a facility class: class 10's is the first sum.
    fn max_price(&self, class: u8) -> i32 {
        match class {
            0..=9 => self.max_price[usize::from(class)],
            10 => self.sums[0],
            _ => 0,
        }
    }

    /// `value` on factor `n`'s scale from its low to its high value, as
    /// 0–100. // 1008:0d10
    #[must_use]
    pub fn scale(&self, value: i32, n: usize) -> i32 {
        let (low, high) = (self.factor(n, 2), self.factor(n, 3));
        let v = if low == 0 && high == 100 {
            value
        } else {
            div(value.wrapping_sub(low).wrapping_mul(100), high.wrapping_sub(low))
        };
        v.clamp(0, 100)
    }

    /// `acc` changed by the rating `value` (0–100, 50 for no change) with
    /// factor `n`'s weight `w` over its divisor `d`: `(acc · (d - w) +
    /// acc · w · value div 50) div d`, as 0–100. // 1008:0de9
    #[must_use]
    pub fn weigh(&self, acc: i32, value: i32, n: usize) -> i32 {
        let (w, d) = (self.factor(n, 0), self.factor(n, 1));
        let moved = div(w.wrapping_mul(acc).wrapping_mul(value), 50);
        div(d.wrapping_sub(w).wrapping_mul(acc).wrapping_add(moved), d).clamp(0, 100)
    }

    /// `acc` moved towards `value` by factor `n`'s weight over its
    /// divisor, rounded, as 0–100. // 1008:0ea1
    #[must_use]
    pub fn approach(&self, acc: i32, value: i32, n: usize) -> i32 {
        let (w, d) = (self.factor(n, 0), self.factor(n, 1));
        let step = div(value.wrapping_sub(acc).wrapping_mul(w).wrapping_add(div(d, 2)), d);
        step.wrapping_add(acc).clamp(0, 100)
    }

    /// Which tenth of `MinMiete` to `MaxMiete` a `rent` falls in, 0–9.
    /// // 1008:097f
    #[must_use]
    pub fn rent_class(&self, rent: i32) -> usize {
        let class = div(rent.wrapping_sub(self.min_rent).wrapping_mul(10), self.rent_range);
        usize::try_from(class.clamp(0, 9)).unwrap_or(0)
    }

    /// The fair rent for a house's stars. // 1008:0829
    fn fair_rent(&self, stars: u8) -> i32 {
        div(i32::from(stars).wrapping_mul(self.rent_range), 100).wrapping_add(self.min_rent)
    }

    /// The fair price per visitor for a facility's stars and class.
    /// // 1008:0709
    fn fair_price(&self, stars: u8, class: u8) -> i32 {
        let low = self.min_price(class);
        div(
            self.max_price(class).wrapping_sub(low).wrapping_mul(i32::from(stars)),
            100,
        )
        .wrapping_add(low)
    }

    /// How fair a house's `rent` is for its stars, 0–100: 100 for no
    /// rent. // 1008:0863
    #[must_use]
    pub fn rent_rating(&self, stars: u8, rent: i32) -> u8 {
        if rent == 0 {
            return 100;
        }
        fairness(div(self.fair_rent(stars).wrapping_mul(50), rent))
    }

    /// How fair a facility's `price` per visitor is for its stars and
    /// class, 0–100: 100 when it's free. // 1008:0776
    #[must_use]
    pub fn price_rating(&self, price: i32, stars: u8, class: u8) -> u8 {
        let r = if price > 0 {
            div(self.fair_price(stars, class).wrapping_mul(50), price)
        } else {
            100
        };
        fairness(r)
    }

    /// A facility's price class, 0–9. The original divides by `MaxPreis`
    /// rather than by the range, then takes `MinPreis` off again.
    /// // 1008:0907
    #[must_use]
    pub fn price_class(&self, price: i32, class: u8) -> u8 {
        let low = self.min_price(class);
        let c = div(price.wrapping_sub(low).wrapping_mul(10), self.max_price(class)).wrapping_sub(low);
        u8::try_from(c.clamp(0, 9)).unwrap_or(0)
    }

    /// Spreads the guests of each class of rent (`guests[1..]`, after
    /// their total) over what they want (`Klassenmischung`), carrying
    /// the remainders on so nobody gets lost: the result is laid out the
    /// same way, its total first. // 1078:0780
    #[must_use]
    pub fn mix(&self, guests: &[i32; 11]) -> [i32; 11] {
        let d = self.mix_divisor;
        let mut out = [0i32; 11];
        let mut carry = 0i32;
        for k in 0..10 {
            let mut v = 0i32;
            for c in 0..10 {
                let m = i32::from(self.class_mix[k][c]).wrapping_mul(guests[1 + c]);
                v = v.wrapping_add(div(m, d));
                carry = carry.wrapping_add(rem(m, d));
                if carry >= d {
                    v = v.wrapping_add(1);
                    carry = carry.wrapping_sub(d);
                }
            }
            out[1 + k] = v;
            out[0] = out[0].wrapping_add(v);
        }
        out
    }
}

/// A rating of a price from its ratio to the fair one (`fair · 50 div
/// price`): as it is from 50 on, below that twice its distance from 25,
/// as 0–100. // 1008:0776, 1008:0863
fn fairness(r: i32) -> u8 {
    let v = if r >= 50 { r } else { r.wrapping_sub(25).wrapping_mul(2) };
    u8::try_from(v.clamp(0, 100)).unwrap_or(0)
}

/// `a · 50 div b` as 0–100 if `b` is positive, else 0. // 1008:06b6
fn per_place(a: i32, b: i32) -> i32 {
    if b > 0 {
        div(a.wrapping_mul(50), b).clamp(0, 100)
    } else {
        0
    }
}

/// The low byte of a rating.
fn byte(v: i32) -> u8 {
    v.to_le_bytes()[0]
}

impl World {
    /// One step of the monthly settlement, by the accounting scheduler's
    /// step (its low word). Steps the chain doesn't name go to the
    /// computer players (10a8:302e) when they play. // 1008:1bf7
    pub(crate) fn settlement_step(&mut self, step: i16, tick: &mut Tick) {
        match step {
            2 => self.clear_sums(),
            4 => {
                self.pay_costs();
                tick.books = true; // 1070:2617
            }
            6 => self.count_beds(),
            8 => {
                for n in 0..self.plots.len() {
                    self.rate_plot(n); // 1008:0f37
                }
            }
            0xa => {
                for p in &mut self.plots {
                    p.set_longint(plot::DEMAND, 0);
                    p.set_longint(plot::ADMITTED, 0);
                }
            }
            0xc => self.rate_house_stars(),
            0xe => self.rate_rents(),
            0x10 => self.house_demand(),
            0x12 => self.sum_house_demand(),
            0x14 => {
                for p in &mut self.plots {
                    let admitted = p.longint(plot::DEMAND).min(p.longint(plot::PORTS));
                    p.set_longint(plot::ADMITTED, admitted);
                }
            }
            0x16 => self.share_guests(),
            // The original walks the houses and compares their owner with
            // 1, doing nothing either way.
            0x18 => {}
            0x1a => self.collect_rents(),
            0x1c => self.house_costs(),
            0x1d | 0x47 => tick.umenu = true, // 1080:0123
            0x1e => self.house_occupancy(tick),
            0x20 => {
                self.pay_houses();
                tick.books = true; // 1070:2617
            }
            0x22 => self.count_guests(),
            0x24 => {
                for p in &mut self.plots {
                    let total = (0..10).fold(0i32, |s, c| s.wrapping_add(p.longint(plot::GUESTS + 4 + 4 * c)));
                    p.set_longint(plot::GUESTS, total);
                }
            }
            0x26 => self.total_guests(),
            0x28 => self.count_player_guests(),
            0x32 => {
                for p in &mut self.plots {
                    for k in 0..11 {
                        let v = p.longint(plot::GUESTS + 4 * k);
                        p.set_longint(plot::WISHES + 4 * k, v); // 1108:148c
                    }
                }
            }
            0x34 => self.mix_wishes(),
            0x36 => self.rate_facility_stars(),
            0x38 => self.rate_prices(),
            0x39 => self.count_places(),
            0x3a => self.facility_demand(),
            0x3c => {
                for p in &mut self.plots {
                    clear_block(p, plot::DEMANDS); // 1008:09c8
                    clear_block(p, plot::SERVED);
                }
            }
            0x3e => self.sum_facility_demand(),
            0x40 => self.serve_shops(),
            0x42 => self.serve_facilities(),
            0x44 => self.share_visitors(),
            0x46 => self.facility_fill(),
            0x48 => self.facility_occupancy(tick),
            0x4a => self.collect_takings(),
            0x4c => self.facility_costs(),
            0x4e => {
                self.pay_facilities();
                tick.books = true; // 1070:2617
            }
            0x50 => self.decay_houses(),
            0x52 => self.decay_facilities(),
            0x58 => self.count_player_places(),
            0x5a => self.rate_player_prices(),
            0x5c => self.rate_reputations(),
            0x64 => self.check_bankruptcies(),
            0x78 => self.send_rentals(tick),
            0x82 => self.computer_rents(),
            0x84 => self.computer_prices(),
            0x8b => {
                self.close_books_month(); // 1070:210a
                tick.books = true; // 1070:2617
            }
            _ => {
                if self.computers_play {
                    self.computer_players_tick(tick); // 10a8:302e
                }
            }
        }
    }

    /// Step 2: the month's sums start at 0 for every player.
    fn clear_sums(&mut self) {
        for p in 1..=self.player_count {
            for sum in HOUSE_RENT..=MONTH {
                self.settlement.set_sum(sum, p, 0);
            }
        }
    }

    /// Step 4: the human pays for advertising (1008:05d9), insurance (a
    /// 120th of their buildings' value if they're insured, 1008:062c),
    /// their cost of living and their interest; the computer players pay
    /// a sixth of theirs. Everyone's costs come off their money and go
    /// into the month; the human's into the books.
    fn pay_costs(&mut self) {
        let advertising = self
            .block_longint(0x3c + 4)
            .wrapping_mul(i32::from(self.advertising(1)))
            .wrapping_mul(self.settlement.advertising_cost);
        let insurance = if self.insured { div(self.assets(1), 120) } else { 0 };
        let s = &mut self.settlement;
        s.set_sum(ADVERTISING, 1, advertising);
        s.set_sum(OTHER, 1, insurance.wrapping_neg());
        s.set_sum(INTEREST, 1, self.finances.interest[0]); // 1078:05a0
        for p in 2..=self.player_count {
            let interest = self.finances.interest.get(usize::from(p) - 1).copied().unwrap_or(0);
            s.set_sum(INTEREST, p, interest / 6);
        }
        let other = s.sum(OTHER, 1).wrapping_sub(s.living_costs);
        s.set_sum(OTHER, 1, other);
        for p in 1..=self.player_count {
            let s = &mut self.settlement;
            let v = s
                .sum(INTEREST, p)
                .wrapping_neg()
                .wrapping_add(s.sum(OTHER, p))
                .wrapping_sub(s.sum(ADVERTISING, p));
            s.add_sum(MONTH, p, v);
            self.add_money(p, v);
        }
        let s = &self.settlement;
        let (a, i, o) = (s.sum(ADVERTISING, 1), s.sum(INTEREST, 1), s.sum(OTHER, 1));
        self.book(BOOK_ADVERTISING, a.wrapping_neg());
        self.book(BOOK_INTEREST, i.wrapping_neg());
        self.book(BOOK_OTHER, o);
    }

    /// Step 6: each plot's beds, by class of rent (1008:097f of the
    /// house's rent), and in all.
    fn count_beds(&mut self) {
        for n in 0..self.plots.len() {
            let mut beds = [0i32; 10];
            for pos in self.plots[n].houses.clone() {
                let Some(h) = self.house_at(pos) else { continue };
                let h = &self.houses[h];
                let class = self.settlement.rent_class(h.price());
                beds[class] = beds[class].wrapping_add(i32::from(self.tables.hausdata(h.building(), 3)));
            }
            let p = &mut self.plots[n];
            p.set_longint(plot::BEDS, 0);
            for (c, b) in beds.iter().enumerate() {
                p.set_longint(plot::CLASS_BEDS + 4 * c, *b);
            }
            let total = beds.iter().fold(0i32, |s, &b| s.wrapping_add(b));
            p.set_longint(plot::BEDS, total);
        }
    }

    /// A plot's ratings: its greenery (palms and parks less other
    /// buildings, against its area), how fair its shops' prices are, its
    /// quality (from those and its facilities' places per bed) and its
    /// quality for each class of rent. Plots without beds get their
    /// greenery as quality and 50 for every class. // 1008:0f37
    ///
    /// The original also mixes copies of its shops' places by class
    /// (1078:0780) without using them.
    fn rate_plot(&mut self, n: usize) {
        let p = &self.plots[n];
        let area = i32::from(p.area());
        let greenery = if area == 0 {
            50
        } else {
            let d = i32::from(p.palms().cast_unsigned()) - i32::from(p.built().cast_unsigned());
            (div(d.wrapping_mul(50), area).wrapping_add(50)).clamp(0, 100)
        };
        // The shops' places weighted by their price class (so each price
        // class's average is itself), and the other buildings' weighted by
        // how fair their prices are.
        let (mut by_class, mut places_by_class) = ([0i32; 10], [0i32; 10]);
        let (mut rated, mut places) = (0i32, 0i32);
        for pos in p.facilities.clone() {
            let Some(f) = self.facility_at(pos) else { continue };
            let f = &self.facilities[f];
            let capacity = i32::from(self.tables.hausdata(f.building(), 3));
            match self.record_kind(f.building()) {
                1 => {
                    let c = usize::from(f.price_class());
                    if let (Some(r), Some(s)) = (by_class.get_mut(c), places_by_class.get_mut(c)) {
                        *r = r.wrapping_add(capacity.wrapping_mul(i32::from(f.price_class())));
                        *s = s.wrapping_add(capacity);
                    }
                }
                2 => {
                    rated = rated.wrapping_add(capacity.wrapping_mul(i32::from(f.price_rating())));
                    places = places.wrapping_add(capacity);
                }
                _ => {}
            }
        }
        let rules = &self.settlement;
        let mut prices = if places > 0 { byte(div(rated, places)) } else { 50 };
        // The original writes each price class's into the same byte, so
        // the last one's stays.
        for c in 0..10 {
            prices = if places_by_class[c] > 0 {
                byte(div(by_class[c], places_by_class[c]))
            } else {
                50
            };
        }
        let p = &self.plots[n];
        let beds = p.longint(plot::BEDS);
        let quality = if beds > 0 {
            let mut a = rules.weigh(50, greenery, factor::PLOT_GREENERY);
            a = rules.weigh(
                a,
                rules.scale(i32::from(prices), factor::PLOT_PRICES),
                factor::PLOT_PRICES,
            );
            for (k, class) in PLOT_FACILITY_CLASSES.iter().enumerate() {
                let per_bed = div(p.longint(plot::FACILITIES + 4 * class).wrapping_mul(50), beds);
                a = rules.weigh(
                    a,
                    rules.scale(per_bed, factor::PLOT_FACILITIES + k),
                    factor::PLOT_FACILITIES + k,
                );
            }
            byte(a)
        } else {
            byte(greenery)
        };
        let mut classes = [50u8; 10];
        if beds > 0 {
            for (c, q) in classes.iter_mut().enumerate() {
                let class_beds = p.longint(plot::CLASS_BEDS + 4 * c);
                if class_beds <= 0 {
                    continue;
                }
                let prices = i32::from(p.byte(plot::CLASS_PRICES + c));
                let mut a = rules.weigh(50, rules.scale(prices, factor::CLASS_PRICES), factor::CLASS_PRICES);
                for k in 0..3 {
                    let places = p.longint(plot::SHOPS + plot::SHOP_CLASS * k + 4 + 4 * c);
                    let per_bed = div(places.wrapping_mul(50), class_beds);
                    let f = factor::CLASS_SHOPS + k;
                    a = rules.weigh(a, rules.scale(per_bed, f), f);
                }
                *q = byte(a);
            }
        }
        let p = &mut self.plots[n];
        p.set_byte(plot::GREENERY, byte(greenery));
        p.set_byte(plot::PRICES, prices);
        p.set_byte(plot::QUALITY, quality);
        for (c, q) in classes.iter().enumerate() {
            p.set_byte(plot::CLASS_QUALITY + c, *q);
        }
    }

    /// Step 0xc: each house's stars, from its type's (`HAUSDATA.DAT`
    /// column 4) by its condition, connection and location.
    fn rate_house_stars(&mut self) {
        for n in 0..self.houses.len() {
            let h = &self.houses[n];
            let stars = self.stars(
                h.building(),
                h.condition(),
                h.connection(),
                h.pos(),
                factor::HOUSE_CONDITION,
            );
            self.houses[n].set_stars(stars);
        }
    }

    /// Step 0x36: the same for the facilities, with their own factors.
    fn rate_facility_stars(&mut self) {
        for n in 0..self.facilities.len() {
            let f = &self.facilities[n];
            let stars = self.stars(f.building(), f.condition(), f.connection(), f.pos(), factor::CONDITION);
            self.facilities[n].set_stars(stars);
        }
    }

    /// A building's stars with the three factors from `first` on.
    /// // 1008:1bf7 steps 0xc, 0x36
    fn stars(&self, building: u8, condition: u8, connection: u8, (i, j): Pos, first: usize) -> u8 {
        let s = &self.settlement;
        let location = if on_map((i, j)) {
            self.island.location.get((i / 2, j / 2)).unwrap_or(0) // 1030:180d
        } else {
            0
        };
        let mut a = i32::from(self.tables.hausdata(building, 4));
        for (k, v) in [condition, connection, location].into_iter().enumerate() {
            a = s.weigh(a, s.scale(i32::from(v), first + k), first + k);
        }
        byte(a)
    }

    /// Step 0xe: how fair each house's rent is (1008:0863).
    fn rate_rents(&mut self) {
        for n in 0..self.houses.len() {
            let h = &self.houses[n];
            let rating = self.settlement.rent_rating(h.stars(), h.price());
            self.houses[n].set_rent_rating(rating);
        }
    }

    /// The owner of the building at `pos`.
    fn owner(&self, pos: Pos) -> u8 {
        self.island.owners.get(pos).unwrap_or(0)
    }

    /// `player`'s advertising level, 0–7 (the money menu's). Player 0
    /// reads the byte before the block in the original; the port says 0.
    /// // *1110:6f24 + player - 1
    fn advertising(&self, player: u8) -> u8 {
        usize::from(player)
            .checked_sub(1)
            .and_then(|i| self.player_block.get(i))
            .copied()
            .unwrap_or(0)
    }

    /// The economy (`Wirtschaft`), 0–255: not known, only read here.
    /// // *1110:6f20 + 0x19a
    fn economy(&self) -> u8 {
        self.totals.get(0x19a).copied().unwrap_or(0)
    }

    /// The `Longint` at `at` in [`World::player_block`], 0 past it.
    fn block_longint(&self, at: usize) -> i32 {
        self.player_block
            .get(at..at + 4)
            .map_or(0, |b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn set_block_longint(&mut self, at: usize, value: i32) {
        if let Some(b) = self.player_block.get_mut(at..at + 4) {
            b.copy_from_slice(&value.to_le_bytes());
        }
    }

    /// Step 0x10: how many guests each house would have, plot by plot:
    /// its surroundings (the plot's quality and its quality for the
    /// house's class of rent, the owner's reputation and advertising,
    /// the economy) on the base demand, then how fair its rent is, in
    /// percent of its beds.
    fn house_demand(&mut self) {
        for n in 0..self.plots.len() {
            for pos in self.plots[n].houses.clone() {
                let Some(h) = self.house_at(pos) else { continue };
                let rules = &self.settlement;
                let p = &self.plots[n];
                let house = &self.houses[h];
                let owner = self.owner(house.pos());
                let class = rules.rent_class(house.price());
                let mut a = rules.weigh(50, i32::from(p.byte(plot::QUALITY)), factor::HOUSE_PLOT);
                a = rules.weigh(
                    a,
                    i32::from(p.byte(plot::CLASS_QUALITY + class)),
                    factor::HOUSE_PLOT_CLASS,
                );
                for (f, v) in [
                    (factor::HOUSE_REPUTATION, self.reputation(owner)),
                    (factor::HOUSE_ADVERTISING, self.advertising(owner)),
                    (factor::HOUSE_ECONOMY, self.economy()),
                ] {
                    a = rules.weigh(a, rules.scale(i32::from(v), f), f);
                }
                let mut b = rules.weigh(i32::from(rules.base_demand), a, factor::HOUSE_SURROUNDINGS);
                b = rules.weigh(b, i32::from(house.rent_rating()), factor::HOUSE_RENT_LEVEL);
                let beds = i32::from(self.tables.hausdata(house.building(), 3));
                let demand = beds.min(div(beds.wrapping_mul(b).wrapping_add(50), 100));
                self.houses[h].set_demand(demand);
            }
        }
    }

    /// Step 0x12: each plot's houses' demand.
    fn sum_house_demand(&mut self) {
        for n in 0..self.plots.len() {
            let mut sum = self.plots[n].longint(plot::DEMAND);
            for pos in &self.plots[n].houses {
                if let Some(h) = self.house_at(*pos) {
                    sum = sum.wrapping_add(self.houses[h].demand());
                }
            }
            self.plots[n].set_longint(plot::DEMAND, sum);
        }
    }

    /// Step 0x16: each house gets its demand in guests if everyone who
    /// wants to come can, else its share of those admitted, the
    /// remainders carried on through the plot's houses, at most its
    /// beds.
    fn share_guests(&mut self) {
        for n in 0..self.plots.len() {
            let (demand, admitted) = (
                self.plots[n].longint(plot::DEMAND),
                self.plots[n].longint(plot::ADMITTED),
            );
            let mut carry = 0i32;
            for pos in self.plots[n].houses.clone() {
                let Some(h) = self.house_at(pos) else { continue };
                let house = &self.houses[h];
                // The original keeps the low word of a `Longint`.
                #[allow(clippy::cast_possible_truncation)]
                let guests = if admitted >= demand {
                    house.demand() as i16
                } else if demand > 0 {
                    let t = house.demand().wrapping_mul(admitted);
                    carry = carry.wrapping_add(rem(t, demand));
                    let mut g = div(t, demand) as i16;
                    if carry >= demand {
                        g = g.wrapping_add(1);
                        carry = carry.wrapping_sub(demand);
                    }
                    let beds = i32::from(self.tables.hausdata(house.building(), 3));
                    i32::from(g).min(beds) as i16
                } else {
                    0
                };
                self.houses[h].set_guests(guests);
            }
        }
    }

    /// Step 0x1a: the houses' and hotels' rent, per guest.
    fn collect_rents(&mut self) {
        for n in 0..self.houses.len() {
            let h = &self.houses[n];
            let rent = i32::from(h.guests()).wrapping_mul(h.price());
            let sum = if self.tables.hausdata(h.building(), 5) == 0 {
                HOUSE_RENT
            } else {
                HOTEL_RENT
            };
            let owner = self.owner(h.pos());
            self.settlement.add_sum(sum, owner, rent);
        }
    }

    /// Step 0x1c: their running costs (`HAUSPREI.DAT` column 1).
    fn house_costs(&mut self) {
        for n in 0..self.houses.len() {
            let h = &self.houses[n];
            let cost = self.tables.hausprei(h.building(), 1);
            let sum = if self.tables.hausdata(h.building(), 5) == 0 {
                HOUSE_COSTS
            } else {
                HOTEL_COSTS
            };
            let owner = self.owner(h.pos());
            self.settlement.add_sum(sum, owner, cost);
        }
    }

    /// Step 0x1e: each house's squares show how full it is, its guests
    /// in percent of its beds (`Integer` arithmetic). A house type
    /// without beds would divide by zero in the original; the port
    /// skips it.
    fn house_occupancy(&mut self, tick: &mut Tick) {
        for n in 0..self.houses.len() {
            let h = &self.houses[n];
            let beds = self.tables.hausdata(h.building(), 3);
            if beds == 0 {
                continue;
            }
            let occupancy = h.guests().wrapping_mul(100).wrapping_div(beds);
            let (building, pos) = (h.building(), h.pos());
            self.show_occupancy(building, pos, occupancy, tick);
        }
    }

    /// Step 0x48: the same for the facilities, by how full they are.
    fn facility_occupancy(&mut self, tick: &mut Tick) {
        for n in 0..self.facilities.len() {
            let f = &self.facilities[n];
            let (building, pos, fill) = (f.building(), f.pos(), i16::from(f.fill()));
            self.show_occupancy(building, pos, fill, tick);
        }
    }

    /// Puts `occupancy` on the squares of the building at `pos`; if its
    /// tens changed, the view redraws the area ([`Tick::occupancy`]).
    /// // 1008:1bf7 steps 0x1e, 0x48
    fn show_occupancy(&mut self, building: u8, (i, j): Pos, occupancy: i16, tick: &mut Tick) {
        let shown = i16::from(self.island.occupancy.get((i, j)).unwrap_or(0));
        let changed = occupancy / 10 != shown / 10;
        let last = (
            self.tables.hausnr(building, 3).wrapping_add(i).wrapping_sub(1),
            self.tables.hausnr(building, 4).wrapping_add(j).wrapping_sub(1),
        );
        for a in i..=last.0 {
            for b in j..=last.1 {
                if on_map((a, b)) {
                    self.island.occupancy.set((a, b), occupancy.to_le_bytes()[0]);
                }
            }
        }
        if changed {
            tick.occupancy.push(((i, j), last)); // 10b8:1534, 1050:264d
        }
    }

    /// Step 0x20: the houses' and hotels' rent less their costs.
    fn pay_houses(&mut self) {
        self.pay_month([HOUSE_RENT, HOTEL_RENT], [HOUSE_COSTS, HOTEL_COSTS]);
        self.book_month(
            [(BOOK_HOUSE_RENT, HOUSE_RENT), (BOOK_HOTEL_RENT, HOTEL_RENT)],
            [(BOOK_HOUSE_COSTS, HOUSE_COSTS), (BOOK_HOTEL_COSTS, HOTEL_COSTS)],
        );
    }

    /// Step 0x4e: the restaurants' and other buildings' takings less
    /// their costs.
    fn pay_facilities(&mut self) {
        self.pay_month(
            [RESTAURANT_TAKINGS, FACILITY_TAKINGS],
            [RESTAURANT_COSTS, FACILITY_COSTS],
        );
        self.book_month(
            [
                (BOOK_RESTAURANT_TAKINGS, RESTAURANT_TAKINGS),
                (BOOK_FACILITY_TAKINGS, FACILITY_TAKINGS),
            ],
            [
                (BOOK_RESTAURANT_COSTS, RESTAURANT_COSTS),
                (BOOK_FACILITY_COSTS, FACILITY_COSTS),
            ],
        );
    }

    /// Every player gets `takings - costs` of the two kinds of buildings
    /// (first, less its costs, then the second) into the month and their
    /// money. // 1008:1bf7 steps 0x20, 0x4e
    fn pay_month(&mut self, takings: [usize; 2], costs: [usize; 2]) {
        for p in 1..=self.player_count {
            let s = &mut self.settlement;
            let v = s
                .sum(takings[0], p)
                .wrapping_sub(s.sum(costs[0], p))
                .wrapping_add(s.sum(takings[1], p))
                .wrapping_sub(s.sum(costs[1], p));
            s.add_sum(MONTH, p, v);
            self.add_money(p, v);
        }
    }

    /// The human's takings and costs into the books (marked: player 1's),
    /// in the original's order. // 1070:1419
    fn book_month(&mut self, takings: [(u8, usize); 2], costs: [(u8, usize); 2]) {
        for k in 0..2 {
            let (t, c) = (self.settlement.sum(takings[k].1, 1), self.settlement.sum(costs[k].1, 1));
            self.book(takings[k].0, t);
            self.book(costs[k].0, c.wrapping_neg());
        }
    }

    /// Step 0x22: each plot's guests by class of rent.
    fn count_guests(&mut self) {
        for n in 0..self.plots.len() {
            let mut guests = [0i32; 10];
            for pos in &self.plots[n].houses {
                if let Some(h) = self.house_at(*pos) {
                    let h = &self.houses[h];
                    let class = self.settlement.rent_class(h.price());
                    guests[class] = guests[class].wrapping_add(i32::from(h.guests()));
                }
            }
            for (c, g) in guests.iter().enumerate() {
                self.plots[n].set_longint(plot::GUESTS + 4 + 4 * c, *g);
            }
        }
    }

    /// Step 0x26: the island's guests, in all and by class.
    /// // *1110:6f20 + 0xce, + 0xd2
    fn total_guests(&mut self) {
        let mut sums = [0i32; 11];
        for p in &self.plots {
            for (k, s) in sums.iter_mut().enumerate() {
                *s = s.wrapping_add(p.longint(plot::GUESTS + 4 * k));
            }
        }
        for (k, s) in sums.iter().enumerate() {
            if let Some(b) = self.totals.get_mut(0xce + 4 * k..0xd2 + 4 * k) {
                b.copy_from_slice(&s.to_le_bytes());
            }
        }
    }

    /// Step 0x28: each player's guests ([`crate::money::Finances::guests`]),
    /// for all eight players; the houses of nobody count for player 8's
    /// interest, as in the original.
    fn count_player_guests(&mut self) {
        for p in 1..=8 {
            self.finances.set_guests(p, 0); // 1078:0172
        }
        for n in 0..self.houses.len() {
            let h = &self.houses[n];
            let owner = self.owner(h.pos());
            let guests = self.finances.guests(owner).wrapping_add(i32::from(h.guests()));
            self.finances.set_guests(owner, guests);
        }
    }

    /// Step 0x34: the guests' wishes by class (1078:0780).
    fn mix_wishes(&mut self) {
        for n in 0..self.plots.len() {
            let wishes: [i32; 11] = std::array::from_fn(|k| self.plots[n].longint(plot::WISHES + 4 * k));
            let mixed = self.settlement.mix(&wishes);
            for (k, v) in mixed.iter().enumerate() {
                self.plots[n].set_longint(plot::WISHES + 4 * k, *v);
            }
        }
    }

    /// Step 0x38: each facility's price per visitor and day (`price ·
    /// 100 div (places · 30)`) rated against the fair one for its stars
    /// (1008:0776), and its price class (1008:0907).
    fn rate_prices(&mut self) {
        for n in 0..self.facilities.len() {
            let f = &self.facilities[n];
            let places = i32::from(self.tables.hausdata(f.building(), 3));
            let price = div(f.price().wrapping_mul(100), places.wrapping_mul(30));
            let class = self.facility_class(f.building());
            let rating = self.settlement.price_rating(price, f.stars(), class);
            let price_class = self.settlement.price_class(price, class);
            let f = &mut self.facilities[n];
            f.set_price_rating(rating);
            f.set_price_class(price_class);
        }
    }

    /// Step 0x39: each plot's places, the shops' by class and price
    /// class, the other buildings' by class.
    fn count_places(&mut self) {
        for n in 0..self.plots.len() {
            clear_block(&mut self.plots[n], plot::BEDS); // 1008:09c8
            for pos in self.plots[n].facilities.clone() {
                let Some(f) = self.facility_at(pos) else { continue };
                let f = &self.facilities[f];
                let (building, price_class) = (f.building(), usize::from(f.price_class()));
                let places = i32::from(self.tables.hausdata(building, 3));
                let class = usize::from(self.facility_class(building));
                let at = match self.record_kind(building) {
                    1 => plot::SHOPS + plot::SHOP_CLASS * class + 4 + 4 * price_class,
                    2 => plot::FACILITIES + 4 * class,
                    _ => continue,
                };
                let p = &mut self.plots[n];
                p.set_longint(at, p.longint(at).wrapping_add(places));
            }
            let p = &mut self.plots[n];
            for k in 0..3 {
                let at = plot::SHOPS + plot::SHOP_CLASS * k;
                let total = (0..10).fold(p.longint(at), |s, c| s.wrapping_add(p.longint(at + 4 + 4 * c)));
                p.set_longint(at, total);
            }
        }
    }

    /// Step 0x3a: how many visitors each facility would have, plot by
    /// plot: the guests per place (for a shop also those of its price
    /// class per place of it), the owner's reputation and advertising and
    /// the economy, then how fair its price is on the base demand, in
    /// percent of its places.
    fn facility_demand(&mut self) {
        for n in 0..self.plots.len() {
            for pos in self.plots[n].facilities.clone() {
                let Some(fi) = self.facility_at(pos) else { continue };
                let rules = &self.settlement;
                let p = &self.plots[n];
                let f = &self.facilities[fi];
                let building = f.building();
                let owner = self.owner(f.pos());
                let class = usize::from(self.facility_class(building));
                let wishes = p.longint(plot::WISHES);
                let rated = [self.reputation(owner), self.advertising(owner), self.economy()];
                let mut a;
                let first;
                if self.record_kind(building) == 1 {
                    let shops = plot::SHOPS + plot::SHOP_CLASS * class;
                    let f0 = factor::SHOP_PRESSURE;
                    a = rules.weigh(50, rules.scale(per_place(wishes, p.longint(shops)), f0), f0);
                    let c = 4 * usize::from(f.price_class());
                    let class_wishes = p.longint(plot::WISHES + 4 + c);
                    let class_places = p.longint(shops + 4 + c);
                    let f1 = factor::SHOP_CLASS_PRESSURE;
                    a = rules.weigh(a, rules.scale(per_place(class_wishes, class_places), f1), f1);
                    first = factor::SHOP_CLASS_PRESSURE + 1;
                } else {
                    let f0 = factor::FACILITY_PRESSURE;
                    let places = p.longint(plot::FACILITIES + 4 * class);
                    a = rules.weigh(50, rules.scale(per_place(wishes, places), f0), f0);
                    first = factor::FACILITY_PRESSURE + 1;
                }
                for (k, v) in rated.into_iter().enumerate() {
                    a = rules.weigh(a, rules.scale(i32::from(v), first + k), first + k);
                }
                let mut b = rules.weigh(
                    i32::from(rules.base_demand),
                    i32::from(f.price_rating()),
                    factor::PRICE_LEVEL,
                );
                b = rules.weigh(b, a, factor::SURROUNDINGS);
                let places = i32::from(self.tables.hausdata(building, 3));
                let demand = places.min(div(places.wrapping_mul(b).wrapping_add(50), 100));
                self.facilities[fi].set_demand(demand);
            }
        }
    }

    /// Step 0x3e: each plot's facilities' demand, the shops' by class and
    /// price class, the others' by class.
    fn sum_facility_demand(&mut self) {
        for n in 0..self.plots.len() {
            for pos in self.plots[n].facilities.clone() {
                let Some(f) = self.facility_at(pos) else { continue };
                let f = &self.facilities[f];
                let (building, demand) = (f.building(), f.demand());
                let class = usize::from(self.facility_class(building));
                let ats = match self.record_kind(building) {
                    1 => {
                        let shops = plot::DEMANDS + plot::BLOCK_SHOPS + plot::SHOP_CLASS * class;
                        vec![shops, shops + 4 + 4 * usize::from(f.price_class())]
                    }
                    2 => vec![plot::DEMANDS + plot::BLOCK_FACILITIES + 4 * class],
                    _ => continue,
                };
                let p = &mut self.plots[n];
                for at in ats {
                    p.set_longint(at, p.longint(at).wrapping_add(demand));
                }
            }
        }
    }

    /// Step 0x40: for each shop class, the guests' wishes of each class
    /// go to its shops of that price class, from the dearest down, at
    /// most what they'd take; those left over try the next cheaper ones.
    fn serve_shops(&mut self) {
        for p in &mut self.plots {
            for k in 0..3 {
                let mut carry = 0i32;
                for c in (0..10).rev() {
                    let wishes = p.longint(plot::WISHES + 4 + 4 * c).wrapping_add(carry);
                    let off = plot::BLOCK_SHOPS + plot::SHOP_CLASS * k + 4 + 4 * c;
                    let served = wishes.min(p.longint(plot::DEMANDS + off));
                    p.set_longint(plot::SERVED + off, served);
                    carry = wishes.wrapping_sub(served);
                }
            }
        }
    }

    /// Step 0x42: the other buildings of each class 3–9 serve the plot's
    /// guests, at most their places for transport, energy, water and
    /// rubbish, else at most their demand.
    fn serve_facilities(&mut self) {
        for p in &mut self.plots {
            let guests = p.longint(plot::WISHES);
            for k in 3..=9 {
                let limit = if SHARED_CLASSES.contains(&k) {
                    p.longint(plot::FACILITIES + 4 * k)
                } else {
                    p.longint(plot::DEMANDS + plot::BLOCK_FACILITIES + 4 * k)
                };
                p.set_longint(plot::SERVED + plot::BLOCK_FACILITIES + 4 * k, guests.min(limit));
            }
        }
    }

    /// Step 0x44: each facility (but the championships') gets its demand
    /// in visitors if its class (and price class) is served in full,
    /// else its share of those served, the remainders carried on
    /// through the plot's facilities, at most its places. With no demand
    /// at all it keeps its visitors.
    fn share_visitors(&mut self) {
        for n in 0..self.plots.len() {
            let mut shop_carry = [0i32; 30]; // 1110:4cb0
            let mut carry = [0i32; 11]; // 1110:4d1c
            for pos in self.plots[n].facilities.clone() {
                let Some(fi) = self.facility_at(pos) else { continue };
                let f = &self.facilities[fi];
                let building = f.building();
                if EMPTY_CHAMPIONSHIPS.contains(&building) || FULL_CHAMPIONSHIPS.contains(&building) {
                    continue;
                }
                let class = usize::from(self.facility_class(building));
                let price_class = usize::from(f.price_class());
                let (off, acc) = match self.record_kind(building) {
                    1 => (
                        plot::BLOCK_SHOPS + plot::SHOP_CLASS * class + 4 + 4 * price_class,
                        shop_carry.get_mut(class * 10 + price_class),
                    ),
                    2 => (plot::BLOCK_FACILITIES + 4 * class, carry.get_mut(class)),
                    _ => continue,
                };
                let p = &self.plots[n];
                let (demand, served) = (p.longint(plot::DEMANDS + off), p.longint(plot::SERVED + off));
                let f = &self.facilities[fi];
                let visitors = if demand == served {
                    f.demand()
                } else if demand > 0 {
                    let t = f.demand().wrapping_mul(served);
                    let mut v = div(t, demand);
                    if let Some(acc) = acc {
                        *acc = acc.wrapping_add(rem(t, demand));
                        if *acc >= demand {
                            v = v.wrapping_add(1);
                            *acc = acc.wrapping_sub(demand);
                        }
                    }
                    i32::from(self.tables.hausdata(building, 3)).min(v)
                } else {
                    continue;
                };
                self.facilities[fi].set_visitors(visitors);
            }
        }
    }

    /// Step 0x46: how full each facility is, its visitors in percent of
    /// its places (the low byte).
    fn facility_fill(&mut self) {
        for n in 0..self.facilities.len() {
            let f = &self.facilities[n];
            let places = i32::from(self.tables.hausdata(f.building(), 3));
            let fill = byte(div(f.visitors().wrapping_mul(100), places));
            self.facilities[n].set_fill(fill);
        }
    }

    /// Step 0x4a: the facilities' takings, their price times how full
    /// they are, rounded; restaurants (kind 2) apart.
    fn collect_takings(&mut self) {
        for n in 0..self.facilities.len() {
            let f = &self.facilities[n];
            let takings = div(i32::from(f.fill()).wrapping_mul(f.price()).wrapping_add(50), 100);
            let sum = if self.tables.hausdata(f.building(), 5) == 2 {
                RESTAURANT_TAKINGS
            } else {
                FACILITY_TAKINGS
            };
            let owner = self.owner(f.pos());
            self.settlement.add_sum(sum, owner, takings);
        }
    }

    /// Step 0x4c: their running costs.
    fn facility_costs(&mut self) {
        for n in 0..self.facilities.len() {
            let f = &self.facilities[n];
            let cost = self.tables.hausprei(f.building(), 1);
            let sum = if self.tables.hausdata(f.building(), 5) == 2 {
                RESTAURANT_COSTS
            } else {
                FACILITY_COSTS
            };
            let owner = self.owner(f.pos());
            self.settlement.add_sum(sum, owner, cost);
        }
    }

    /// Step 0x50: each house in some condition loses a step of it with
    /// the houses' decay chance (`Random(100)`).
    fn decay_houses(&mut self) {
        let chance = u16::from(self.settlement.decay[0]);
        for n in 0..self.houses.len() {
            if self.houses[n].condition() > 0 && chance > self.random.random(100) {
                let c = self.houses[n].condition();
                self.houses[n].set_condition(c - 1);
            }
        }
    }

    /// Step 0x52: the same for the facilities, by their record's kind.
    fn decay_facilities(&mut self) {
        for n in 0..self.facilities.len() {
            if self.facilities[n].condition() == 0 {
                continue;
            }
            let r = self.random.random(100);
            // The original reads the byte after the table for kind 3.
            let kind = usize::from(self.record_kind(self.facilities[n].building()));
            let chance = u16::from(self.settlement.decay.get(kind).copied().unwrap_or(0));
            if chance > r {
                let c = self.facilities[n].condition();
                self.facilities[n].set_condition(c - 1);
            }
        }
    }

    /// Step 0x58: each player's beds and places. // *1110:6f24 + 0x3c,
    /// + 0x5c
    fn count_player_places(&mut self) {
        for p in 1..=usize::from(self.player_count) {
            self.set_block_longint(0x3c + 4 * p, 0);
            self.set_block_longint(0x5c + 4 * p, 0);
        }
        for n in 0..self.houses.len() {
            let h = &self.houses[n];
            let at = 0x3c + 4 * usize::from(self.owner(h.pos()));
            let beds = i32::from(self.tables.hausdata(h.building(), 3));
            self.set_block_longint(at, self.block_longint(at).wrapping_add(beds));
        }
        for n in 0..self.facilities.len() {
            let f = &self.facilities[n];
            let at = 0x5c + 4 * usize::from(self.owner(f.pos()));
            let places = i32::from(self.tables.hausdata(f.building(), 3));
            self.set_block_longint(at, self.block_longint(at).wrapping_add(places));
        }
    }

    /// Step 0x5a: how fair each player's prices are, their facilities'
    /// ratings weighted by their places, 50 without any. // *1110:6f24 + 4
    fn rate_player_prices(&mut self) {
        for p in 1..=usize::from(self.player_count) {
            self.set_block_longint(4 + 4 * p, 0);
        }
        for n in 0..self.facilities.len() {
            let f = &self.facilities[n];
            let at = 4 + 4 * usize::from(self.owner(f.pos()));
            let places = i32::from(self.tables.hausdata(f.building(), 3));
            let rated = places.wrapping_mul(i32::from(f.price_rating()));
            self.set_block_longint(at, self.block_longint(at).wrapping_add(rated));
        }
        for p in 1..=usize::from(self.player_count) {
            let places = self.block_longint(0x5c + 4 * p);
            let rating = if places > 0 {
                div(self.block_longint(4 + 4 * p), places)
            } else {
                50
            };
            self.set_block_longint(4 + 4 * p, rating);
        }
    }

    /// Step 0x5c: each player's places per bed (five times the ratio, at
    /// most 100, 50 without beds), and their reputation moves towards
    /// their prices' rating and that. // *1110:6f24 + 0x27 + player
    fn rate_reputations(&mut self) {
        for p in 1..=self.player_count {
            let i = usize::from(p);
            let beds = self.block_longint(0x3c + 4 * i);
            let places = if beds > 0 {
                byte(div(self.block_longint(0x5c + 4 * i).wrapping_mul(5), beds).clamp(0, 100))
            } else {
                50
            };
            if let Some(b) = self.player_block.get_mut(0x27 + i) {
                *b = places;
            }
            let s = &self.settlement;
            let prices = s.scale(self.block_longint(4 + 4 * i), factor::REPUTATION_PRICES);
            let mut r = s.approach(i32::from(self.reputation(p)), prices, factor::REPUTATION_PRICES);
            r = s.approach(r, i32::from(places), factor::REPUTATION_PLACES);
            self.set_reputation(p, byte(r));
        }
    }

    /// Step 0x64: a computer player who has gone bankrupt is out, and
    /// event 10 (an auction of their property, it seems) comes for 3/5
    /// of their buildings' value plus 1000. Unless the human cheated or
    /// the money is frozen, their net worth passing 100 000, 1 000 000 or
    /// 10 000 000 brings events 12, 16 or 18 (one a month, the first that
    /// hasn't come yet). The events point at the human's view (marked).
    fn check_bankruptcies(&mut self) {
        let here = self.viewpoint.origin;
        for p in 2..=self.player_count {
            if self.bankrupt(p) && self.player_out(p) == 0 {
                let amount = div(self.assets(p), 5).wrapping_mul(3).wrapping_add(1000);
                self.schedule_event(10, amount, i32::from(p), here);
                self.set_player_out(p, 1);
            }
        }
        if self.cheated || self.finances.frozen {
            return;
        }
        let worth = self.net_worth(1);
        let ready = |w: &World, n: usize| w.events.get(n).is_some_and(|e| e.chance == 1);
        if worth > 100_000 {
            if ready(self, 12) {
                self.schedule_event(12, 100_000, 1, here);
            } else if worth > 1_000_000 {
                if ready(self, 16) {
                    self.schedule_event(16, 1_000_000, 1, here);
                } else if worth > 10_000_000 && ready(self, 18) {
                    self.schedule_event(18, 10_000_000, 1, here);
                }
            }
        }
    }

    /// Makes event `n` certain to come in its window (chance `0x3fffc`)
    /// with `value`, `arg` and its message at `pos`. // 1010:3006
    pub(crate) fn schedule_event(&mut self, n: usize, value: i32, arg: i32, pos: Pos) {
        if let Some(e) = self.events.get_mut(n) {
            e.chance = 0x3fffc;
            e.value = value;
            e.arg = arg;
            e.pos = pos;
        }
    }

    /// Step 0x78: the rentals (the bicycles, jeeps, car showroom and surf
    /// school) send out one of their two kinds of people (`Random(2)`).
    fn send_rentals(&mut self, tick: &mut Tick) {
        for n in 0..self.facilities.len() {
            let b = self.facilities[n].building();
            let Some(&(_, kinds, by_sea)) = RENTALS.iter().find(|r| r.0 == b) else {
                continue;
            };
            let kind = kinds[usize::from(self.random.random(2) & 1)];
            self.send_rental(n, kind, by_sea, tick);
        }
    }

    /// The rental number `n` (in [`World::facilities`]) has as many people
    /// out (counted in `layer_551c` at its square) as a twentieth of how
    /// full it is: one more goes out, to the nearest road on its plot or
    /// the sea within 10 rings, or one comes back if two are too many.
    /// // 1008:15dd, 1008:16be
    fn send_rental(&mut self, n: usize, kind: u8, by_sea: bool, tick: &mut Tick) {
        let f = &self.facilities[n];
        let (pos, fill) = (f.pos(), i16::from(f.fill()));
        let out = match self.island.layer_551c.get(pos).unwrap_or(0) {
            0xff => 0,
            c => i16::from(c),
        };
        let wanted = (fill - 1) / 20;
        if wanted < out {
            if wanted < out - 1 && out != 0 {
                self.remove_first_at_home(pos); // 1088:2c3b
            }
            return;
        }
        let near = (pos.0 + 1, pos.1 + 1);
        let found = if by_sea {
            self.find_sea(pos, near, 10) // 1010:20e0
        } else {
            self.find_road(pos, near, 10) // 1010:1f8f
        };
        if let Some(at) = found {
            // 1088:2c04
            self.people.beach = pos;
            self.add_person(kind, at, tick);
            if self.people.beach.0 == -1 {
                self.set_marker(pos, out.wrapping_add(1).to_le_bytes()[0]);
            }
        }
    }

    /// Step 0x82: the computer players' (and nobody's) rents go up a step
    /// when a house is full (1010:1d62) and down one when the rent is
    /// rated unfair (1010:1e05).
    fn computer_rents(&mut self) {
        for n in 0..self.houses.len() {
            let h = &self.houses[n];
            if self.owner(h.pos()) == 1 {
                continue;
            }
            let rent = h.price();
            let rent = if self.tables.hausdata(h.building(), 3) == h.guests() {
                raise_rent(rent)
            } else if h.rent_rating() < 100 {
                lower_rent(rent)
            } else {
                continue;
            };
            self.houses[n].set_price(rent);
        }
    }

    /// Step 0x84: the same for their facilities' prices, by whether
    /// they're full (1010:1ae2) and how fair their price is (1010:1c86).
    fn computer_prices(&mut self) {
        for n in 0..self.facilities.len() {
            let f = &self.facilities[n];
            if self.owner(f.pos()) == 1 {
                continue;
            }
            let price = if f.fill() == 100 {
                self.raise_price(f.building(), f.price())
            } else if f.price_rating() < 100 {
                lower_price(f.price())
            } else {
                continue;
            };
            self.facilities[n].set_price(price);
        }
    }

    /// A facility's price a step up, at most `HAUSPREI.DAT`'s column 2
    /// for transport (kinds 9–11) and full championships, else three
    /// times it. // 1010:1ae2
    fn raise_price(&self, building: u8, price: i32) -> i32 {
        let p = match price {
            i32::MIN..0x280 => price.wrapping_add(40),
            0x280..0xa00 => price.wrapping_add(160),
            0xa00..0x2800 => price.wrapping_add(640),
            0x2800..0xa000 => price.wrapping_add(2560),
            0xa000..0x28000 => price.wrapping_add(10240),
            _ => price.wrapping_add(40960).min(1_000_000),
        };
        let top = self.tables.hausprei(building, 2);
        if (9..=11).contains(&self.tables.hausdata(building, 5)) || FULL_CHAMPIONSHIPS.contains(&building) {
            p.min(top)
        } else {
            p.min(top.wrapping_mul(3))
        }
    }
}

/// A rent a step up: by 4, 8, 20 or 40 below 80, 200, 400 or 1561, else
/// 1600. // 1010:1d62
#[must_use]
pub fn raise_rent(rent: i32) -> i32 {
    match rent {
        i32::MIN..80 => rent.wrapping_add(4),
        80..200 => rent.wrapping_add(8),
        200..400 => rent.wrapping_add(20),
        400..0x619 => rent.wrapping_add(40),
        _ => 1600,
    }
}

/// A rent a step down: by 40 above 400, 20 above 200, 8 above 80, 4 from
/// 4 on, else 0. // 1010:1e05
#[must_use]
pub fn lower_rent(rent: i32) -> i32 {
    match rent {
        0x191.. => rent.wrapping_sub(40),
        0xc9.. => rent.wrapping_sub(20),
        0x51.. => rent.wrapping_sub(8),
        4.. => rent.wrapping_sub(4),
        _ => 0,
    }
}

/// A facility's price a step down: by 40960, 10240, 2560, 640 or 160
/// above 163840, 40960, 10240, 2560 or 640, else by 40 to at least 0.
/// // 1010:1c86
#[must_use]
pub fn lower_price(price: i32) -> i32 {
    match price {
        0x28001.. => price.wrapping_sub(0xa000),
        0xa001.. => price.wrapping_sub(0x2800),
        0x2801.. => price.wrapping_sub(0xa00),
        0xa01.. => price.wrapping_sub(0x280),
        0x281.. => price.wrapping_sub(0xa0),
        _ => price.wrapping_sub(40).max(0),
    }
}

/// Clears a block of a plot's sums from `base`: the shops' three classes
/// (all and by price class) and the other buildings' classes 3–9.
/// // 1008:09c8
fn clear_block(p: &mut Plot, base: usize) {
    for k in 0..3 {
        let at = base + plot::BLOCK_SHOPS + plot::SHOP_CLASS * k;
        p.set_longint(at, 0);
        for c in 0..10 {
            p.set_longint(at + 4 + 4 * c, 0);
        }
    }
    for k in 3..=9 {
        p.set_longint(base + plot::BLOCK_FACILITIES + 4 * k, 0);
    }
}

impl World {
    /// The house whose first square is `pos`: what a plot's list points
    /// to.
    fn house_at(&self, pos: Pos) -> Option<usize> {
        self.houses.iter().position(|h| h.pos() == pos)
    }

    /// The facility whose first square is `pos`.
    fn facility_at(&self, pos: Pos) -> Option<usize> {
        self.facilities.iter().position(|f| f.pos() == pos)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::world::{Facility, House, Tables};

    /// Rents from 0 to 800 and prices from 0 to 1000 for every class.
    fn settlement() -> Settlement {
        let mut s = Settlement {
            min_rent: 0,
            max_rent: 800,
            rent_range: 800,
            ..Settlement::default()
        };
        s.max_price = [1000; 10];
        s
    }

    /// A house type (40, 100 beds, running costs 50, kind 0), a hotel
    /// (41, kind 1), and a shop (42, 10 places, record kind 1).
    fn world() -> World {
        let mut hausdata = vec![[0; 10]; 127];
        hausdata[40][3] = 100;
        hausdata[41][3] = 100;
        hausdata[41][5] = 1;
        hausdata[42][3] = 10;
        hausdata[42][5] = 2;
        let mut hausprei = vec![[0; 4]; 127];
        hausprei[40][1] = 50;
        hausprei[41][1] = 70;
        World {
            tables: Arc::new(Tables {
                hausnr: vec![[1, 0, 0, 1, 1, 1, 1, 0]; 127],
                hausdata,
                hausprei,
                ..Tables::default()
            }),
            player_count: 2,
            settlement: settlement(),
            ..World::default()
        }
    }

    #[test]
    fn ratings() {
        let mut s = settlement();
        // Neutral factors leave a value alone at 50 and scale 0..100.
        assert_eq!(s.weigh(70, 50, 0), 70);
        assert_eq!(s.weigh(70, 100, 0), 100);
        assert_eq!(s.weigh(70, 0, 0), 35);
        assert_eq!(s.scale(150, 0), 100);
        assert_eq!(s.scale(-3, 0), 0);
        // Advertising goes from -7 to 7.
        s.factors[6] = [1, 1, -7, 7];
        assert_eq!(s.scale(0, 6), 50);
        assert_eq!(s.scale(7, 6), 100);
        // Half way, rounded.
        assert_eq!(s.approach(50, 100, 0), 75);
        assert_eq!(s.approach(51, 0, 0), 26);
        assert_eq!(s.approach(0, 1, 0), 1);
        // A divisor of 0 would stop the original.
        s.factors[1] = [1, 0, 5, 5];
        assert_eq!((s.weigh(70, 50, 1), s.scale(5, 1)), (0, 0));
    }

    #[test]
    fn prices() {
        let s = settlement();
        assert_eq!(s.rent_class(-5), 0);
        assert_eq!(s.rent_class(399), 4);
        assert_eq!(s.rent_class(5000), 9);
        // 50 stars make 400 fair: that rent rates 50, half of it 100,
        // twice it (25 - 25) * 2.
        assert_eq!(s.rent_rating(50, 400), 50);
        assert_eq!(s.rent_rating(50, 200), 100);
        assert_eq!(s.rent_rating(50, 800), 0);
        assert_eq!(s.rent_rating(50, 0), 100);
        assert_eq!(s.price_rating(0, 50, 3), 100);
        assert_eq!(s.price_rating(500, 50, 3), 50);
        // The original divides by MaxPreis, not the range, then takes
        // MinPreis off again.
        assert_eq!(s.price_class(450, 2), 4);
        let mut s = s;
        s.min_price[2] = 100;
        assert_eq!(s.price_class(450, 2), 0);
        // Class 10's MaxPreis is the first sum.
        s.sums[0] = 1000;
        assert_eq!(s.price_class(500, 10), 5);
    }

    #[test]
    fn class_mixing_keeps_everyone() {
        let mut s = settlement();
        s.mix_divisor = 3;
        // Each class of rent wants class 0 and its own class once each.
        for k in 0..10 {
            s.class_mix[k][k] = 1;
            s.class_mix[0][k] = 1;
        }
        let guests = [0, 10, 2, 0, 0, 0, 0, 0, 0, 0, 1];
        let mixed = s.mix(&guests);
        // Class 0: (1·10 + 1·2 + 1·1) / 3 with the remainder carried on.
        assert_eq!(mixed[1], 3 + 1);
        assert_eq!(mixed[0], mixed[1..].iter().sum::<i32>());
    }

    #[test]
    fn monthly_costs() {
        let mut w = world();
        w.finances.money = [10_000, 10_000, 0, 0, 0, 0, 0, 0];
        w.finances.interest = [600, 600, 0, 0, 0, 0, 0, 0];
        w.settlement.living_costs = 100;
        w.settlement.advertising_cost = 2;
        // 300 beds at advertising level 3.
        w.player_block[0] = 3;
        w.player_block[0x40..0x44].copy_from_slice(&300i32.to_le_bytes());
        let mut tick = Tick::default();
        w.settlement_step(4, &mut tick);
        assert!(tick.books);
        // 1800 for advertising, 600 interest, 100 cost of living.
        assert_eq!(w.finances.money[0], 10_000 - 1800 - 600 - 100);
        // The computer pays a sixth of its interest.
        assert_eq!(w.finances.money[1], 10_000 - 100);
        assert_eq!(w.settlement.sum(MONTH, 1), -2500);
        assert_eq!(w.book_value(BOOK_ADVERTISING), -1800);
        assert_eq!(w.book_value(BOOK_INTEREST), -600);
        assert_eq!(w.book_value(BOOK_OTHER), -100);
        // Step 2 starts the month again.
        w.settlement_step(2, &mut tick);
        assert_eq!(w.settlement.sum(MONTH, 1), 0);
    }

    #[test]
    fn houses_take_rent() {
        let mut w = world();
        let mut house = House::new((10, 10));
        house.set_building(40);
        house.set_price(300);
        house.set_guests(20);
        let mut hotel = House::new((12, 10));
        hotel.set_building(41);
        hotel.set_price(500);
        hotel.set_guests(10);
        w.houses = vec![house, hotel];
        w.island.owners.set((10, 10), 1);
        w.island.owners.set((12, 10), 2);
        let mut tick = Tick::default();
        for step in [0x1a, 0x1c, 0x1e, 0x20] {
            w.settlement_step(step, &mut tick);
        }
        assert_eq!(w.finances.money[0], 20 * 300 - 50);
        assert_eq!(w.finances.money[1], 10 * 500 - 70);
        assert_eq!(w.book_value(BOOK_HOUSE_RENT), 6000);
        assert_eq!(w.book_value(BOOK_HOUSE_COSTS), -50);
        assert_eq!(w.book_value(BOOK_HOTEL_RENT), 0);
        // Their squares show how full they are; both changed tens.
        assert_eq!(w.island.occupancy.at((10, 10)), 20);
        assert_eq!(tick.occupancy, [((10, 10), (10, 10)), ((12, 10), (12, 10))]);
        // Player 8's interest gets the guests of nobody's houses.
        w.island.owners.set((12, 10), 0);
        w.settlement_step(0x28, &mut tick);
        assert_eq!(w.finances.guests(1), 20);
        assert_eq!(w.finances.interest[7], 10);
    }

    #[test]
    fn guests_are_shared_out() {
        let mut w = world();
        let mut plot = Plot::new(1);
        for n in 0..3 {
            let mut h = House::new((n, 0));
            h.set_building(40);
            h.set_demand(10);
            w.houses.push(h);
            plot.houses.push((n, 0));
        }
        plot.set_longint(plot::DEMAND, 30);
        plot.set_longint(plot::ADMITTED, 20);
        w.plots = vec![plot];
        w.settlement_step(0x16, &mut Tick::default());
        // 6.67 each, the remainders carried on: nobody gets lost.
        let guests: Vec<i16> = w.houses.iter().map(House::guests).collect();
        assert_eq!(guests, [6, 7, 7]);
        // With room for everyone, each gets its demand.
        w.plots[0].set_longint(plot::ADMITTED, 30);
        w.settlement_step(0x16, &mut Tick::default());
        assert_eq!(w.houses[2].guests(), 10);
    }

    #[test]
    fn facilities_take_their_share() {
        let mut w = world();
        let mut plot = Plot::new(1);
        let mut f = Facility::new(42, (5, 5));
        f.set_demand(8);
        f.set_price(1000);
        w.facilities.push(f);
        plot.facilities.push((5, 5));
        // Its class and price class are served 6 of a demand of 8.
        let class = usize::from(w.facility_class(42));
        let off = plot::BLOCK_SHOPS + plot::SHOP_CLASS * class + 4;
        plot.set_longint(plot::DEMANDS + off, 8);
        plot.set_longint(plot::SERVED + off, 6);
        w.plots = vec![plot];
        w.island.owners.set((5, 5), 1);
        let mut tick = Tick::default();
        for step in [0x44, 0x46, 0x4a, 0x4c, 0x4e] {
            w.settlement_step(step, &mut tick);
        }
        let f = &w.facilities[0];
        assert_eq!((f.visitors(), f.fill()), (6, 60));
        // 60 percent of 1000, a restaurant's.
        assert_eq!(w.finances.money[0], 600);
        assert_eq!(w.book_value(BOOK_RESTAURANT_TAKINGS), 600);
    }

    #[test]
    fn computer_prices() {
        assert_eq!(raise_rent(79), 83);
        assert_eq!(raise_rent(1560), 1600);
        assert_eq!(raise_rent(2000), 1600);
        assert_eq!(lower_rent(401), 361);
        assert_eq!(lower_rent(81), 73);
        assert_eq!(lower_rent(3), 0);
        assert_eq!(lower_price(0x28001), 0x1e001);
        assert_eq!(lower_price(0x280), 0x258);
        assert_eq!(lower_price(10), 0);
        let mut w = world();
        let house = |pos, guests, rating| {
            let mut h = House::new(pos);
            h.set_building(40);
            h.set_price(100);
            h.set_guests(guests);
            h.set_rent_rating(rating);
            h
        };
        w.houses = vec![
            house((1, 1), 100, 100),
            house((2, 2), 99, 99),
            house((3, 3), 99, 100),
            house((4, 4), 100, 0),
        ];
        for (pos, owner) in [((1, 1), 2), ((2, 2), 2), ((3, 3), 0), ((4, 4), 1)] {
            w.island.owners.set(pos, owner);
        }
        w.settlement_step(0x82, &mut Tick::default());
        let rents: Vec<i32> = w.houses.iter().map(House::price).collect();
        // Full: up; rated unfair: down; rated fair: kept; the human's
        // alone.
        assert_eq!(rents, [108, 92, 100, 100]);
    }

    #[test]
    fn computer_turns_on_free_steps() {
        let mut w = world();
        w.player_count = 3;
        let mut tick = Tick::default();
        w.settlement_step(3, &mut tick);
        w.settlement_step(0x18, &mut tick);
        assert_eq!(w.computer_turns, 1);
        w.computers_play = false;
        w.settlement_step(5, &mut tick);
        assert_eq!(w.computer_turns, 1);
    }

    #[test]
    fn reputation_follows_prices_and_places() {
        let mut w = world();
        w.player_count = 1;
        w.set_reputation(1, 50);
        let mut f = Facility::new(42, (5, 5));
        f.set_price_rating(100);
        w.facilities.push(f);
        let mut h = House::new((6, 6));
        h.set_building(40);
        w.houses.push(h);
        w.island.owners.set((5, 5), 1);
        w.island.owners.set((6, 6), 1);
        let mut tick = Tick::default();
        for step in [0x58, 0x5a, 0x5c] {
            w.settlement_step(step, &mut tick);
        }
        // 10 places for 100 beds: 5 · 10 / 100 = 0; prices 100.
        assert_eq!(w.player_block[0x28], 0);
        // Half way to 100, then half way to 0.
        assert_eq!(w.reputation(1), 38);
    }

    #[test]
    fn month_in_the_history() {
        let mut w = world();
        w.player_count = 2;
        w.finances.money = [500, 2000, 0, 0, 0, 0, 0, 0];
        w.finances.loan = [1500, 0, 0, 0, 0, 0, 0, 0];
        w.finances.guest_count = [7, 9, 0, 0, 0, 0, 0, 0];
        for _ in 0..13 {
            w.close_books_month();
        }
        assert_eq!(w.guests_last_months(1), 7 * 12);
        assert_eq!(w.guests_last_months(2), 9 * 12);
        let long = |at: usize| i32::from_le_bytes(w.books.history[at..at + 4].try_into().unwrap());
        // Money less loan, with a scale as far below 0 as above.
        assert_eq!((long(0), long(4)), (-1000, 2000));
        assert_eq!((long(0xf00), long(0xf04)), (2000, -2000));
        // The guests' scale goes up to even numbers.
        assert_eq!(long(2 * 0xf0a + 0xf00), 10);
        assert_eq!(w.books.history[0xf08], 13);
    }
}
