//! Auctions: the world's part of `TAuktionsform` (segment `1028`). An
//! event puts something up for auction (1028:08ac), and a second later
//! the form opens (`Timer2`, 1028:1940), which stops the simulation's
//! clock until it closes. The computers have worked out how much each of
//! them would pay (10a8:0373, in [`crate::computer`]); while the form is
//! open, every 100 ms (`Timer3`, 1028:1973) one of them may bid, the
//! auctioneer calls "going once, twice, sold" when nobody has bid for a
//! while, and three seconds after the third call (`Timer1`, 1028:190d)
//! the form closes and the last bidder gets the item (1028:08e7).
//!
//! The form's timers are wall-clock `TTimer`s in the original; here they
//! count the world's 100 ms ticks. Where the original waits on
//! `GetTickCount` (the paddle's animation, how long the auctioneer
//! talks), that's ticks too. The human's clicks and keys are commands
//! ([`Action`], [`World::auction_command`]); the form itself is the view's
//! (`crate::auktionsform`). `docs/auction.md` has the whole flow.
//!
//! Single human: the form is player 1's. With several human clients
//! (decision 7) every human seat has the form; a co-op seat's clients act
//! for it together (any one's bid or pass is the seat's). With "Auktion
//! wartet" the auction stands until every human seat has started it or
//! passed, or [`crate::simulation::Options::auction_timeout`] ticks have
//! gone by, after which the others count as passed. Any seat holding a
//! button keeps the computers from bidding, and the item goes to the
//! computers only when every human seat has passed
//! ([`Auction::started`], [`Auction::passed`], [`Auction::held_by`]).

use crate::accounts::BOOK_NEW_BUILDINGS;
use crate::construction::BUILDING_SITE;
use crate::enhance::Fix;
use crate::events::{UNSHAKEN, low_word};
use crate::news::Message;
use crate::simulation::Tick;
use crate::world::{NO_BUILDING, Pos, World, on_map};

/// `Timer2.Interval`, the DFM's default of 1000 ms: how long after the
/// event the form opens. // 1028:08ac, 1028:1940
pub const OPEN_TICKS: u16 = 10;
/// `Timer1.Interval`, 3000 ms: how long the form stays up after the
/// third call. // 1028:190d
pub const SOLD_TICKS: u16 = 30;
/// How long a bid's paddle animation blocks everything: 35 frames, each
/// at least 25 ms after the last by `GetTickCount` (1028:15ef), so 875 ms
/// or more.
pub const BID_TICKS: u16 = 9;
/// For 800 ms after a call the auctioneer's mouth moves (1028:0221).
pub const CALL_TICKS: u16 = 8;
/// `1110:65a0` before anyone has bid.
pub const NO_BID: u16 = 0xff;
/// The third call: the item is sold.
pub const SOLD: u16 = 3;
/// A building a player sells (event 9, [`World::computer_turn`] and the
/// human's sale), and a bankrupt player's property (event 10). // 1028:08e7
pub const BUILDING_SALE: i16 = 0x15;
pub const PROPERTY_SALE: i16 = 0x16;
/// The events whose argument and square those two items read
/// (`*1110:6536 + 0x200`, `+0x204`, `+0x234`).
const SALE_EVENT: usize = 9;
const PROPERTY_EVENT: usize = 10;
/// The sounds of the three calls (1028:1973) and of the hammer
/// (1028:0221).
const CALL_SOUNDS: [u8; 3] = [0x1a, 0x1b, 0x1c];
const HAMMER_SOUND: u8 = 0x14;

/// What winning each of the other items gives: its message, and the
/// building type made available to the winner (`make_available(type,
/// winner)`, 10a0:0691). Any other item gives nothing, though the price is
/// paid. // 1028:08e7
const PRIZES: [(i16, i16, Option<u8>); 13] = [
    (7, 0x25, Some(0x71)),
    (0x55, 0x56, Some(0x79)),
    (10, 0xb, Some(0x1e)),
    (1, 2, Some(0x1d)),
    (6, 0x27, None),
    (8, 0x28, Some(0x6f)),
    (0xd, 0xe, Some(0x3c)),
    (0x10, 0x11, Some(0x23)),
    (0x3c, 0x3d, Some(0x6e)),
    (0x33, 0x34, Some(0x6c)),
    (0x13, 0x14, Some(0x25)),
    (0x39, 0x3a, Some(0x7e)),
    (0x36, 0x37, Some(0x7d)),
];

/// The auctioneer's animation (1028:0221). It's only for the picture, but
/// the original draws its random numbers from `RandSeed` between the
/// computers' bids, and nothing in it depends on a player's view, so the
/// world draws them in the original's order and keeps the frames for the
/// view to draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Frames {
    /// The mouth, 0–6: `Random(7)` for 800 ms after a call, else 0.
    pub mouth: u8,
    /// The eyes and the brows, 0–4: one time in 20 `Random(4) + 1`.
    pub eyes: u8,
    pub brows: u8,
    /// The hammer's frame, 0–6. // 1110:65b4 drawn
    pub hammer: u8,
    /// The other hand's frame, 0–3. // 1110:65b6 drawn
    pub hand: u8,
    /// The hammer comes down, after a call. // 1110:65b8
    pub hammering: bool,
    /// The hand waves (one time in 15 while nobody bids). // 1110:65b9
    pub waving: bool,
    /// The next frames of each. // 1110:65b4, 1110:65b6
    hammer_step: u8,
    hand_step: u8,
}

impl Frames {
    /// Every value, the steps too, for the port's chunk
    /// (`fix.saved_state`, `crate::savegame`).
    #[must_use]
    pub fn values(&self) -> [u8; 9] {
        [
            self.mouth,
            self.eyes,
            self.brows,
            self.hammer,
            self.hand,
            u8::from(self.hammering),
            u8::from(self.waving),
            self.hammer_step,
            self.hand_step,
        ]
    }

    /// The frames [`Frames::values`] gave.
    #[must_use]
    pub fn from_values(v: [u8; 9]) -> Self {
        Frames {
            mouth: v[0],
            eyes: v[1],
            brows: v[2],
            hammer: v[3],
            hand: v[4],
            hammering: v[5] != 0,
            waving: v[6] != 0,
            hammer_step: v[7],
            hand_step: v[8],
        }
    }
}

/// The auction an event started, and how it stands. // unit 1028:2506
///
/// While it's [`Auction::open`], the simulation's clock stands for
/// everyone (`AnimationsTimer` off, 1020:24a0). That stays so with
/// several humans: the auction ends by its timers, and its wait for the
/// seats by [`Auction::waiting`], in world ticks.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Auction {
    /// The price: the start price, then the last bid. // 1110:654a
    pub price: i32,
    /// The price it started at; each bid sets it to the bid too.
    /// // 1110:659c
    pub start_price: i32,
    /// What's on sale: the message number of the event that started it.
    /// // 1110:656e
    pub item: i16,
    /// Who bid last, 0 for nobody. // 1110:6548
    pub bidder: i16,
    /// What each player would bid at most, players 1 to 8, worked out
    /// when the form opens (10a8:0373). Not saved. // 1110:654e
    pub limits: [i32; 8],
    /// The colour of the last bidder's paddle, 1 when the form opens.
    /// // 1110:6596
    pub colour: u16,
    /// The calls so far: [`NO_BID`] before the first bid, 0 after a bid,
    /// then 1 to [`SOLD`] (and on, uselessly, until the form closes).
    /// // 1110:65a0
    pub going: u16,
    /// The auction's steps since the last call (or the opening).
    /// // 1110:65a2
    pub ticks: i32,
    /// Added to the 10 steps before a call; always 0. // 1110:65a6
    pub wait: i32,
    /// The player has started the auction, or it didn't wait for them
    /// ([`crate::simulation::Options::auction_waits`]); until then it
    /// stands. // 1110:65ae
    pub joined: bool,
    /// The player holds a button down on the form, which keeps the
    /// computers from bidding (`1110:6599` isn't 0: the view's mouse state
    /// in the original, here the player's command [`Action::Hold`]).
    pub held: bool,
    /// The form is shown and its step timer (`Timer3`) runs; the clock
    /// stands. Closing it sets `Timer1.Tag`, which is `!open` here.
    pub open: bool,
    /// `Timer2`: ticks until the form opens. // 1028:08ac
    pub opening: Option<u16>,
    /// `Timer1`: ticks until the item goes to the last bidder. // 1028:1973
    pub sold: Option<u16>,
    /// Ticks the paddle's animation still blocks everything (1028:15ef).
    pub bid_pause: u16,
    /// Ticks since the last call, for the auctioneer's mouth; the most
    /// when the form opens. // 1110:65aa
    pub since_call: u16,
    pub frames: Frames,
    /// With several human seats, those that started the auction, a bit
    /// for each (`1 << (p - 1)`): [`Auction::joined`] for each seat. A
    /// port extension (decision 7), as the three after it.
    pub started: u8,
    /// The human seats that passed (or closed their form): the item goes
    /// to the computers once every one has.
    pub passed: u8,
    /// The human seats holding a button down on their form: any one keeps
    /// the computers from bidding ([`Auction::held`]).
    pub held_by: u8,
    /// With "Auktion wartet": the ticks left until the seats that haven't
    /// started the auction or passed count as passed
    /// ([`crate::simulation::Options::auction_timeout`]).
    pub waiting: Option<u16>,
    /// With `fix.auction_queue`: the auctions started while this one
    /// waited to open or was on, each its item (the event's message
    /// number, as [`World::start_auction`] takes it) and price, the
    /// first next. A port extension, kept in a saved game's port chunk
    /// (`auction.queue`, [`queue_lines`], [`queue_in`]).
    pub queue: Vec<(i32, i32)>,
}

/// The port chunk's line of the auctions waiting their turn
/// ([`Auction::queue`]): `item,price` pairs; none for an empty queue.
#[must_use]
pub fn queue_lines(a: &Auction) -> Vec<(String, String)> {
    if a.queue.is_empty() {
        return Vec::new();
    }
    let pairs: Vec<String> = a.queue.iter().map(|(item, price)| format!("{item},{price}")).collect();
    vec![(QUEUE_KEY.to_owned(), pairs.join(" "))]
}

/// The auctions waiting their turn that a saved game's port chunk in
/// `trailing` keeps ([`queue_lines`]); none in a file without the line.
#[must_use]
pub fn queue_in(trailing: &[u8]) -> Vec<(i32, i32)> {
    formats::savegame::port_chunk(trailing)
        .unwrap_or_default()
        .into_iter()
        .find(|(k, _)| k == QUEUE_KEY)
        .map(|(_, v)| {
            v.split_whitespace()
                .filter_map(|pair| {
                    let (item, price) = pair.split_once(',')?;
                    Some((item.parse().ok()?, price.parse().ok()?))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The port chunk's key of [`Auction::queue`].
const QUEUE_KEY: &str = "auction.queue";

/// Seat `p`'s bit in [`Auction::started`], [`Auction::passed`] and
/// [`Auction::held_by`]; 0 for no seat.
#[must_use]
pub fn seat_bit(p: u8) -> u8 {
    match p {
        1..=8 => 1 << (p - 1),
        _ => 0,
    }
}

/// What happened to the auction in a tick or command, for the view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// The form opens. // 1028:0dde
    Opened,
    /// The player started the auction. // 1028:2208, 1028:26e4
    Joined,
    /// A player bid: the paddle in their colour goes up. // 1028:15ef
    Bid(i16),
    /// The auctioneer calls, 1 to [`SOLD`] (and on). // 1028:1973
    Going(u16),
    /// The form closes. // 1028:0476
    Closed,
}

/// What a player does on the auction form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    /// A button on the form goes down (true) or up (false). // 1110:6599
    Hold(bool),
    /// Bid, or start an auction that waits for the player: the mouse's
    /// button, only while the item isn't sold and with the money for it
    /// (1028:1e23, 1028:2208). v1.0's Enter (1028:26e4) also bid, without
    /// either check; the later build took Enter out (en 1028:3620).
    Bid,
    /// Leave the item to the computers: the pass or close button goes
    /// down. // 1028:1e23, 1028:1b77 (1028:0c82)
    Pass,
    /// The pass or close button comes up over itself. // 1028:2208,
    /// 1028:1ce0 (1028:0476)
    Close,
    /// Escape, Space or Alt+F4: close the form, then leave the item to the
    /// computers. // 1028:26e4, en 1028:3620
    Leave,
}

/// A player number as the byte the original passes on.
fn player_byte(p: i16) -> u8 {
    p.to_le_bytes()[0]
}

/// A timer one tick closer to firing, and whether it's due.
fn count_down(timer: &mut Option<u16>) -> bool {
    match timer {
        Some(n) => {
            *n = n.saturating_sub(1);
            *n == 0
        }
        None => false,
    }
}

impl World {
    /// Puts `item` up for auction at `price`, and starts the form's
    /// opening timer (`Timer2`, unless it's on already), which
    /// [`Tick::auction`] tells the view. // 1028:08ac
    ///
    /// With `fix.auction_queue`, while another auction waits to open, is
    /// on or waits for `Timer1` ([`World::auction_busy`]), this one goes
    /// into [`Auction::queue`] and starts once that one is over
    /// ([`World::next_auction`]).
    pub(crate) fn start_auction(&mut self, item: i32, price: i32, tick: &mut Tick) {
        // BUG(original): a second auction before the form opens (Timer2 already on), or while one is on, takes the first one's item and price: the first is never held (fix.auction_queue: it waits its turn).
        if self.fixes.has(Fix::AuctionQueue) && self.auction_busy() {
            self.auction.queue.push((item, price));
            return;
        }
        let a = &mut self.auction;
        a.price = price;
        a.start_price = price;
        a.item = low_word(item);
        a.opening.get_or_insert(OPEN_TICKS);
        tick.auction = true;
    }

    /// The auction form's three timers for 100 ms, in the DFM's order:
    /// `Timer1` (sold), `Timer2` (open) and `Timer3` (a step), which runs
    /// from the tick after the form opens. While a paddle's animation
    /// blocks the original's message loop they count on but don't fire.
    /// The world's tick runs this before the clock's steps, which stand
    /// while the form is open. // 1028:190d, 1028:1940, 1028:1973
    pub(crate) fn auction_tick(&mut self, tick: &mut Tick) {
        self.auction_timers(tick);
        self.next_auction(tick);
    }

    /// Whether an auction waits to open (`Timer2`), is on, or waits for
    /// `Timer1` to hand its item over.
    #[must_use]
    pub fn auction_busy(&self) -> bool {
        let a = &self.auction;
        a.opening.is_some() || a.open || a.sold.is_some()
    }

    /// With `fix.auction_queue`, once no auction is on
    /// ([`World::auction_busy`]): the next one waiting its turn starts
    /// ([`World::start_auction`]).
    fn next_auction(&mut self, tick: &mut Tick) {
        if !self.fixes.has(Fix::AuctionQueue) || self.auction_busy() || self.auction.queue.is_empty() {
            return;
        }
        let (item, price) = self.auction.queue.remove(0);
        self.start_auction(item, price, tick);
    }

    /// Whether event `n`'s one record (9, a building's sale, or 10, a
    /// bankrupt player's property) is taken: the event is waiting to come
    /// (its chance isn't 0 and its window isn't over) or running, or its
    /// auction ([`BUILDING_SALE`], [`PROPERTY_SALE`]) waits to open, is
    /// on, waits for `Timer1` or waits its turn ([`Auction::queue`]).
    /// Scheduling it again then would overwrite its value, player and
    /// square (`fix.sale_event_busy`).
    #[must_use]
    pub fn sale_event_busy(&self, n: usize) -> bool {
        let item = if n == PROPERTY_EVENT {
            PROPERTY_SALE
        } else {
            BUILDING_SALE
        };
        let event = self
            .events
            .get(n)
            .is_some_and(|e| e.running != 0 || (e.chance != 0 && self.days <= e.last_day));
        let a = &self.auction;
        event || (a.item == item && self.auction_busy()) || a.queue.iter().any(|&(i, _)| low_word(i) == item)
    }

    /// [`World::auction_tick`]'s three timers. // 1028:190d, 1028:1940,
    /// 1028:1973
    fn auction_timers(&mut self, tick: &mut Tick) {
        let a = &mut self.auction;
        a.since_call = a.since_call.saturating_add(1);
        let sold = count_down(&mut a.sold);
        let opening = count_down(&mut a.opening);
        if a.bid_pause > 0 {
            a.bid_pause -= 1;
            return;
        }
        if sold {
            self.auction.sold = None;
            self.auction_sold(tick);
        }
        if opening {
            // `Timer2` (1028:1940) does nothing while the TV's speaker is
            // open (`1110:662d`), and fires again a second later.
            // BUG(original): the auction waits on one player's speaker, whose speech and option are the view's (fix.auction_speaker: it doesn't).
            if self.news.speaker == crate::news::Speaker::Off || self.fixes.has(Fix::AuctionSpeaker) {
                self.auction.opening = None;
                self.open_auction(tick);
                return;
            }
            self.auction.opening = Some(OPEN_TICKS);
        }
        if self.auction.open {
            self.auction_step(tick);
        }
    }

    /// The form opens: every player's limit at the price (10a8:0373), and
    /// unless the human sells their own building, which only the
    /// computers bid for (1028:0c82), the auction begins with nobody's
    /// bid and the clock stands. The rest is the view's: the form's
    /// pictures and texts, closing the TV (1048:077a) and disabling the
    /// other forms (1010:2970). // 1028:0dde
    fn open_auction(&mut self, tick: &mut Tick) {
        self.computer_bids(self.auction.price); // 10a8:0373
        let seller = self.events.get(SALE_EVENT).map_or(0, |e| e.arg);
        // BUG(original): the form is player 1's, who alone starts the auction, holds the computers and passes for everyone (design issue for multiplayer: a form for each human seat, decision 7).
        if self.seats.multiplayer() {
            self.open_auction_seats(seller, tick);
            return;
        }
        if self.auction.item == BUILDING_SALE && seller == 1 {
            self.sell_to_computers(false, tick); // 1028:0c82
            return;
        }
        let a = &mut self.auction;
        a.joined = !self.options.auction_waits;
        a.open = true; // 1020:24a0: the clock stands.
        a.colour = 1;
        a.bidder = 0;
        a.going = NO_BID;
        a.held = false;
        a.ticks = 0;
        a.wait = 0;
        a.since_call = u16::MAX;
        tick.auction_changes.push(Change::Opened);
    }

    /// The human seats that take part in an auction: those a client plays
    /// that are still in, as bits ([`seat_bit`]).
    #[must_use]
    pub fn auction_seats(&self) -> u8 {
        self.human_seats()
            .filter(|&p| self.player_out(p) == 0)
            .fold(0, |bits, p| bits | seat_bit(p))
    }

    /// [`World::open_auction`] with several human seats (decision 7):
    /// every human seat gets the form. A seat that sells its own building
    /// has passed from the start, as the original's human, who gets no
    /// form for it (1028:0c82); with no other human seat the computers
    /// bid among themselves at once. With "Auktion wartet" the auction
    /// waits for the seats, at most [`Options::auction_timeout`] ticks;
    /// without, it starts at once, as the original's.
    ///
    /// [`Options::auction_timeout`]: crate::simulation::Options::auction_timeout
    fn open_auction_seats(&mut self, seller: i32, tick: &mut Tick) {
        let passed = match u8::try_from(seller) {
            Ok(p) if self.auction.item == BUILDING_SALE && self.is_human(p) => seat_bit(p),
            _ => 0,
        };
        if self.auction_seats() & !passed == 0 {
            self.sell_to_computers(false, tick); // 1028:0c82
            return;
        }
        let waits = self.options.auction_waits;
        let timeout = self.options.auction_timeout;
        let a = &mut self.auction;
        a.joined = !waits;
        a.open = true; // 1020:24a0: the clock stands.
        a.colour = 1;
        a.bidder = 0;
        a.going = NO_BID;
        a.held = false;
        a.ticks = 0;
        a.wait = 0;
        a.since_call = u16::MAX;
        a.started = 0;
        a.passed = passed;
        a.held_by = 0;
        a.waiting = waits.then_some(timeout);
        tick.auction_changes.push(Change::Opened);
    }

    /// With several human seats, before the auction has begun: its wait
    /// counts down a tick, and once it has run out every seat that hasn't
    /// started the auction counts as passed; then whether the seats have
    /// decided ([`World::auction_decided`]).
    fn auction_wait(&mut self, tick: &mut Tick) {
        if let Some(n) = &mut self.auction.waiting {
            *n = n.saturating_sub(1);
            if *n == 0 {
                self.auction.waiting = None;
                let seats = self.auction_seats();
                let a = &mut self.auction;
                a.passed |= seats & !a.started;
            }
        }
        self.auction_decided(tick);
    }

    /// With several human seats, before the auction has begun: once every
    /// human seat has started it or passed it begins, or with all passed
    /// the form closes and the computers bid among themselves (1028:0c82).
    fn auction_decided(&mut self, tick: &mut Tick) {
        let seats = self.auction_seats();
        let a = &mut self.auction;
        if seats & !(a.started | a.passed) != 0 {
            return;
        }
        if seats & !a.passed == 0 {
            self.close_auction(tick); // 1028:0476
            self.sell_to_computers(true, tick); // 1028:0c82
        } else {
            a.joined = true;
            a.waiting = None;
            tick.auction_changes.push(Change::Joined);
        }
    }

    /// With several human seats, once a step: a seat whose clients have
    /// all gone holds no button, and with every human seat passed (one
    /// that had not has gone) the item goes to the computers, until it's
    /// sold.
    fn auction_seats_step(&mut self, tick: &mut Tick) {
        let seats = self.auction_seats();
        let a = &mut self.auction;
        a.held_by &= seats;
        a.held = a.held_by != 0;
        if a.sold.is_none() && seats & !a.passed == 0 {
            self.close_auction(tick); // 1028:0476
            self.sell_to_computers(true, tick); // 1028:0c82
        }
    }

    /// A step of the auction (`Timer3`): nothing until the player has
    /// started it. After a bid, once 10 steps have passed, one time in 20
    /// (`Random(20)`) the auctioneer calls, with his sound and hammer, and
    /// the third call starts `Timer1`; then one time in 15 (`Random(15)`)
    /// his hand waves. Unless the item is sold, a random computer
    /// (`Random(7) + 2`) that isn't the last bidder bids one time in 30
    /// (`Random(30)`) if its limit is above the price, while the player
    /// holds no button. Then the auctioneer's animation. // 1028:1973
    fn auction_step(&mut self, tick: &mut Tick) {
        if !self.auction.joined {
            if self.seats.multiplayer() {
                self.auction_wait(tick);
            }
            return;
        }
        if self.seats.multiplayer() {
            self.auction_seats_step(tick);
            if !self.auction.open {
                return;
            }
        }
        let a = &mut self.auction;
        a.ticks = a.ticks.wrapping_add(1);
        // BUG(original): the calls go on after the third ("sold") until `Timer1` closes the form: the hammer comes down again and the calls past 3 draw their `Random(20)` and `Random(15)` (en 1028:28b1); kept, as stopping them would change the random numbers.
        if a.going != NO_BID && a.ticks > a.wait.wrapping_add(10) {
            if self.random.random(20) == 1 {
                let a = &mut self.auction;
                a.going = a.going.wrapping_add(1);
                a.since_call = 0; // GetTickCount
                if let Some(&s) = CALL_SOUNDS.get(usize::from(a.going.wrapping_sub(1))) {
                    tick.sounds.push(s); // 10a0:30b6
                }
                a.frames.hammering = true;
                tick.auction_changes.push(Change::Going(a.going)); // 1028:05a3
                a.ticks = 0;
                a.wait = 0;
                if a.going == SOLD {
                    a.sold.get_or_insert(SOLD_TICKS);
                }
            }
            let wave = self.random.random(15) == 1;
            let f = &mut self.auction.frames;
            if wave && !f.hammering {
                f.waving = true;
            }
        }
        if self.auction.sold.is_none() {
            let b = self.random.random(7).cast_signed() + 2;
            let a = &self.auction;
            // BUG(original): a computer bids when its limit is above the price, though its bid (5 % more) may be above the limit.
            let limit = a.limits[usize::from(player_byte(b) - 1)];
            let within = if self.fixes.has(Fix::AuctionComputerLimit) {
                a.price.wrapping_add(a.price / 20) <= limit
            } else {
                a.price < limit
            };
            if a.bidder != b && !a.held && self.random.random(30) == 1 && within {
                self.auction_bid(b, tick); // 1028:15ef
            }
        }
        self.animate_auctioneer(tick); // AuktionsImage is visible.
    }

    /// Player `p` bids 5 % more (`price div 20`), which becomes the start
    /// price too; the calls start again. The original then shows `p`'s
    /// paddle (in their colour) going up and down for about a second,
    /// waiting on `GetTickCount` with its message loop stopped, which
    /// here blocks the auction's timers for [`BID_TICKS`]. // 1028:15ef
    fn auction_bid(&mut self, p: i16, tick: &mut Tick) {
        // `player_colours` as `1110:0073 + p`; another byte for a player
        // past 8.
        let colour = usize::try_from(p - 1)
            .ok()
            .and_then(|i| self.player_colours.get(i))
            .copied()
            .unwrap_or(0);
        let a = &mut self.auction;
        a.colour = u16::from(colour);
        a.going = 0;
        a.price = a.price.wrapping_add(a.price / 20);
        a.bidder = p;
        a.start_price = a.price;
        a.bid_pause = BID_TICKS;
        tick.auction_changes.push(Change::Bid(p));
    }

    /// The auctioneer's animation, drawn each step: his mouth (`Random(7)`
    /// for 800 ms after a call), his eyes and brows (each one time in 20,
    /// `Random(20)` then `Random(4)`), the hammer's next frame while it
    /// comes down (with its sound at the fifth) and the hand's while it
    /// waves. // 1028:0221
    fn animate_auctioneer(&mut self, tick: &mut Tick) {
        let talking = self.auction.since_call <= CALL_TICKS;
        let r = &mut self.random;
        let mouth = if talking { r.random(7) } else { 0 };
        let mut part = || {
            if r.random(20) == 1 { r.random(4) + 1 } else { 0 }
        };
        let (eyes, brows) = (part(), part());
        let f = &mut self.auction.frames;
        // `Random(7)` and `Random(4) + 1` are below 8.
        let byte = |n: u16| n.to_le_bytes()[0];
        f.mouth = byte(mouth);
        f.eyes = byte(eyes);
        f.brows = byte(brows);
        if f.hammering {
            f.hammer = f.hammer_step;
            f.hammer_step += 1;
            if f.hammer_step == 5 {
                tick.sounds.push(HAMMER_SOUND); // 10a0:30b6
            }
            if f.hammer_step == 7 {
                f.hammer_step = 0;
                f.hammering = false;
            }
        } else {
            f.hammer = 0;
        }
        if f.waving {
            f.hand = f.hand_step;
            f.hand_step += 1;
            if f.hand_step == 4 {
                f.hand_step = 0;
                f.waving = false;
            }
        } else {
            f.hand = 0;
        }
    }

    /// `Timer1`: the form closes if it's still open, and the item goes to
    /// the last bidder. // 1028:190d
    fn auction_sold(&mut self, tick: &mut Tick) {
        if self.auction.open {
            self.close_auction(tick); // 1028:0476
        }
        self.award_auction(tick); // 1028:08e7
    }

    /// The form closes: the step timer stops and the clock runs again
    /// (1020:24b7). `Timer1` keeps running if it was. The view hides the
    /// form and enables the other forms again (1010:29cc). // 1028:0476
    fn close_auction(&mut self, tick: &mut Tick) {
        self.auction.open = false;
        tick.auction_changes.push(Change::Closed);
    }

    /// The computers bid among themselves: if any of their limits is
    /// above the price, up to 101 tries of a raise of `Random(20)` times 5 %
    /// and a computer (`Random(7) + 2`), until one whose limit is above
    /// the raised price, who gets the item at it. If none's is, a bankrupt
    /// player's property is cleared away (10a0:09ea). // 1028:0c82
    ///
    /// With `fix.auction_pass` nothing happens once the item is sold
    /// (`Timer1` gives it to the last bidder), and when no computer takes
    /// it, a bidder who leads gets it at their bid; `passed` says the
    /// player passed on the form (not the human's own sale, which only the
    /// computers bid for).
    fn sell_to_computers(&mut self, passed: bool, tick: &mut Tick) {
        let fixed = passed && self.fixes.has(Fix::AuctionPass);
        // BUG(original): passing hands the item to a computer or to nobody, even when a bidder leads (the player too); after the third call the close button gives it away a second time.
        if fixed && self.auction.sold.is_some() {
            return;
        }
        let a = &self.auction;
        let best = a.limits[1..].iter().copied().max().unwrap_or(0);
        let tried = best > a.price;
        let mut sold = false;
        if tried {
            for _ in 0..=100 {
                let price = self.auction.price;
                let raise = i32::from(self.random.random(20)).wrapping_mul(price / 20);
                let offer = price.wrapping_add(raise);
                let b = self.random.random(7).cast_signed() + 2;
                if self.auction.limits[usize::from(player_byte(b) - 1)] > offer {
                    self.auction.bidder = b;
                    self.auction.price = offer;
                    self.award_auction(tick); // 1028:08e7
                    sold = true;
                    break;
                }
            }
        }
        // BUG(original): the property is cleared only when no computer's limit is above the price; when one is but the 101 tries all fail, it stays the bankrupt player's for good (fix.property_unsold: cleared whenever no computer takes it).
        let unsold = !tried || (!sold && self.fixes.has(Fix::PropertyUnsold));
        if fixed && !sold && self.auction.bidder != 0 {
            self.award_auction(tick); // 1028:08e7
        } else if unsold && self.auction.item == PROPERTY_SALE {
            let owner = self.events.get(PROPERTY_EVENT).map_or(0, |e| e.arg);
            self.clear_property(owner.to_le_bytes()[0], tick); // 10a0:09ea
        }
    }

    /// The last bidder pays the price and gets the item: for player 1 the
    /// money menu's books show it as new buildings. A building sale makes
    /// the building theirs and pays its seller; a bankrupt player's
    /// property becomes theirs; any other item posts its message and makes
    /// its building type available to the winner ([`PRIZES`]).
    /// // 1028:08e7
    fn award_auction(&mut self, tick: &mut Tick) {
        let (winner, price, item) = (self.auction.bidder, self.auction.price, self.auction.item);
        let w = player_byte(winner);
        self.add_money(w, price.wrapping_neg()); // 1078:01e7
        // Single human: player 1's books; a human seat's own here.
        if winner == 1 || self.is_human(w) {
            self.book_for(w, BOOK_NEW_BUILDINGS, price.wrapping_neg()); // 1070:1419
            tick.books = true; // 1070:2617
        }
        let event = |n: usize| self.events.get(n).map_or((0, (0, 0)), |e| (e.arg, e.pos));
        match item {
            BUILDING_SALE => {
                let (seller, pos) = event(SALE_EVENT);
                let message = Message {
                    number: 0x29,
                    a: low_word(seller),
                    b: winner,
                    x: price,
                    pos: Some(pos),
                    centre: true,
                    ..Message::default()
                };
                self.post_message(message, tick); // 1048:0a54
                self.transfer_building(w, pos, tick); // 10a0:03ca
                let s = seller.to_le_bytes()[0];
                self.add_money(s, price);
                // BUG(original): the seller's money goes up without a booking, so the sale isn't in their books (fix.sale_booked: under "other", as the refunds are).
                if self.fixes.has(Fix::SaleBooked) && (s == 1 || self.is_human(s)) {
                    self.book_for(s, crate::accounts::BOOK_OTHER, price); // 1070:1419
                    tick.books = true; // 1070:2617
                }
            }
            PROPERTY_SALE => {
                let (owner, _) = event(PROPERTY_EVENT);
                let message = Message {
                    number: 0x2a,
                    a: low_word(owner),
                    b: winner,
                    x: price,
                    ..Message::default()
                };
                self.post_message(message, tick);
                self.transfer_property(owner.to_le_bytes()[0], w, tick); // 10a0:0346
            }
            _ => {
                if let Some(&(_, number, building)) = PRIZES.iter().find(|p| p.0 == item) {
                    let message = Message {
                        number,
                        a: winner,
                        x: price,
                        ..Message::default()
                    };
                    self.post_message(message, tick);
                    // BUG(original): player 8 gets nothing for the item: make_available has no case for them.
                    if let Some(b) = building {
                        self.make_available(b, w); // 10a0:0691
                    }
                }
            }
        }
    }

    /// The building whose first square is `pos` becomes `owner`'s: the
    /// squares of its type (a site's, the type it becomes) from `pos` on,
    /// in byte loops. // 10a0:03ca
    fn transfer_building(&mut self, owner: u8, pos: Pos, tick: &mut Tick) {
        let Some(b) = self.island.buildings.get(pos) else {
            // BUG(original): a square off the map reads outside the layer; the port does nothing.
            return;
        };
        let building = if b == BUILDING_SITE {
            // BUG(original): a site no record holds is read through a nil pointer; the port does nothing.
            let Some(n) = self.find_building_site(pos) else {
                return;
            };
            self.building_sites[n].building
        } else if b == NO_BUILDING {
            return;
        } else {
            b
        };
        let t = &self.tables;
        let last = |start: i16, size: i16| size.wrapping_add(start).wrapping_sub(1).to_le_bytes()[0];
        let (i0, j0) = (pos.0.to_le_bytes()[0], pos.1.to_le_bytes()[0]);
        let (i1, j1) = (last(pos.0, t.hausnr(building, 3)), last(pos.1, t.hausnr(building, 4)));
        for i in i0..=i1 {
            for j in j0..=j1 {
                let square = (i16::from(i), i16::from(j));
                // BUG(original): a building reaching past the map's edge writes owners outside the layer; the port skips them.
                if on_map(square) {
                    self.island.owners.set(square, owner);
                    tick.redraw.push(square); // 1050:21f9, 10b8:1534
                }
            }
        }
    }

    /// Everything of `from`'s becomes `to`'s. The original redraws the
    /// main screen and the overview map. // 10a0:0346
    fn transfer_property(&mut self, from: u8, to: u8, tick: &mut Tick) {
        for i in 0..=250 {
            for j in 0..=250 {
                if self.island.owners.at((i, j)) == from {
                    self.island.owners.set((i, j), to);
                }
            }
        }
        tick.redraw_all = true;
    }

    /// A bankrupt player's property nobody bought is demolished: each of
    /// their squares but palms, beach life and empty ones off the roads,
    /// and the people on it leave. The original turns the sounds off
    /// meanwhile (`1110:0061`). // 10a0:09ea
    fn clear_property(&mut self, player: u8, tick: &mut Tick) {
        for i in 0..=250 {
            for j in 0..=250 {
                let pos = (i, j);
                let road = (10..=58).contains(&self.island.ground.at(pos));
                if UNSHAKEN.contains(&self.island.buildings.at(pos)) && !road {
                    continue;
                }
                if self.island.owners.at(pos) == player {
                    let _ = self.demolish(0, pos, tick); // 1078:280b
                    self.flood_people(pos);
                }
            }
        }
    }

    /// The player `player`'s `action` on the auction form, while it's
    /// open. The original's form is the human's, player 1; a bid's limit
    /// here is the commanding player's. // 1028:1e23, 1028:2208,
    /// 1028:1b77, 1028:1ce0, en 1028:3620
    ///
    /// Single human: with several human seats each seat's form is its own
    /// ([`World::auction_seat_command`]).
    pub(crate) fn auction_command(&mut self, player: u8, action: Action, tick: &mut Tick) {
        if !self.auction.open {
            return;
        }
        if self.seats.multiplayer() {
            self.auction_seat_command(player, action, tick);
            return;
        }
        match action {
            Action::Hold(held) => self.auction.held = held,
            Action::Bid => {
                let a = &self.auction;
                // The mouse does nothing on the buttons once it's sold.
                if a.sold.is_some() {
                    return;
                }
                if !a.joined {
                    self.auction.joined = true;
                    tick.auction_changes.push(Change::Joined);
                    return;
                }
                let limit = a.limits.get(usize::from(player.wrapping_sub(1))).copied().unwrap_or(0);
                // The view shows error 0xe (1030:232d) for the mouse.
                if a.price.wrapping_add(a.price / 20) > limit {
                    return;
                }
                self.auction_bid(i16::from(player), tick);
            }
            Action::Pass => self.sell_to_computers(true, tick),
            Action::Close => self.close_auction(tick),
            Action::Leave => {
                self.close_auction(tick);
                self.sell_to_computers(true, tick);
            }
        }
    }

    /// [`World::auction_command`] with several human seats (decision 7),
    /// for the seat `player`, whichever of its clients sent it. A button
    /// held keeps the computers from bidding while any seat holds one. A
    /// bid before the auction has begun starts it for the seat; after, it
    /// bids, unless the seat passed. Passing, closing the form and leaving
    /// it are the seat's pass: its form closes (the view's) and it holds
    /// no button; once every human seat has passed the form closes for
    /// everyone and the computers bid among themselves (1028:0c82).
    fn auction_seat_command(&mut self, player: u8, action: Action, tick: &mut Tick) {
        let bit = seat_bit(player);
        if bit & self.auction_seats() == 0 {
            return;
        }
        let a = &mut self.auction;
        match action {
            Action::Hold(held) => {
                if held && a.passed & bit == 0 {
                    a.held_by |= bit;
                } else {
                    a.held_by &= !bit;
                }
                a.held = a.held_by != 0;
            }
            Action::Bid => {
                if a.sold.is_some() || a.passed & bit != 0 {
                    return;
                }
                if !a.joined {
                    a.started |= bit;
                    self.auction_decided(tick);
                    return;
                }
                let limit = a.limits.get(usize::from(player.wrapping_sub(1))).copied().unwrap_or(0);
                // The view shows error 0xe (1030:232d) for the mouse.
                if a.price.wrapping_add(a.price / 20) > limit {
                    return;
                }
                self.auction_bid(i16::from(player), tick); // 1028:15ef
            }
            Action::Pass | Action::Close | Action::Leave => {
                if a.passed & bit != 0 {
                    return;
                }
                a.passed |= bit;
                a.held_by &= !bit;
                a.held = a.held_by != 0;
                if !a.joined {
                    self.auction_decided(tick);
                } else if self.auction_seats() & !self.auction.passed == 0 {
                    self.close_auction(tick); // 1028:0476
                    self.sell_to_computers(true, tick); // 1028:0c82
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{self, Command};
    use crate::events::Event;
    use vcl::random::Random;

    /// A world with three players, an auction of item 7 at 10 000, and
    /// money for everyone.
    fn world(waits: bool) -> World {
        let mut w = World {
            player_count: 3,
            events: vec![Event::default(); 41],
            random: Random::new(1234),
            ..World::default()
        };
        w.options.auction_waits = waits;
        w.finances.money[..3].copy_from_slice(&[50_000, 30_000, 25_000]);
        w.finances.max_debt = 10_000;
        let mut tick = Tick::default();
        w.start_auction(7, 10_000, &mut tick);
        assert!(tick.auction);
        w
    }

    fn run(w: &mut World, ticks: usize) -> Vec<Tick> {
        (0..ticks).map(|_| w.tick()).collect()
    }

    #[test]
    fn waits_for_the_tvs_speaker() {
        let mut w = world(false);
        let speaker = |w: &mut World, s| {
            let _ = w.apply(Command {
                player: 1,
                action: command::Action::Speaker(s),
            });
        };
        speaker(&mut w, crate::news::Speaker::Reading);
        // The lights take random numbers each tick while it reads.
        let ticks = run(&mut w, usize::from(OPEN_TICKS) * 2);
        assert!(!w.auction.open);
        assert!(ticks.iter().all(|t| t.tv_lights.is_some()));
        speaker(&mut w, crate::news::Speaker::Open);
        let ticks = run(&mut w, usize::from(OPEN_TICKS));
        assert!(!w.auction.open && ticks.iter().all(|t| t.tv_lights.is_none()));
        // A second after the speaker closed at the latest.
        speaker(&mut w, crate::news::Speaker::Off);
        let ticks = run(&mut w, usize::from(OPEN_TICKS));
        assert!(w.auction.open);
        assert_eq!(ticks.last().unwrap().auction_changes, [Change::Opened]);
    }

    #[test]
    fn fix_auction_speaker() {
        let mut w = world(false);
        w.fixes.set(Fix::AuctionSpeaker, true);
        let _ = w.apply(Command {
            player: 1,
            action: command::Action::Speaker(crate::news::Speaker::Reading),
        });
        // The auction doesn't wait; the lights still draw without
        // `fix.tv_lights`.
        let ticks = run(&mut w, usize::from(OPEN_TICKS));
        assert!(w.auction.open);
        assert!(ticks[0].tv_lights.is_some());
        // With both, the world doesn't keep the speaker at all.
        w.fixes.set(Fix::TvLights, true);
        let _ = w.apply(Command {
            player: 1,
            action: command::Action::Speaker(crate::news::Speaker::Reading),
        });
        assert_eq!(w.news.speaker, crate::news::Speaker::Off);
    }

    #[test]
    fn computers_bid_until_sold() {
        let mut w = world(false);
        let ticks = run(&mut w, OPEN_TICKS.into());
        let before = w.clock.day_ticks;
        assert_eq!(ticks.last().unwrap().auction_changes, [Change::Opened]);
        assert!(w.auction.open);
        // The human's limit is their money; the computers' are random
        // between the price and three times it, or what they can pay.
        assert_eq!(w.auction.limits[0], 50_000);
        assert_eq!(w.auction.limits[3..], [0; 5]);
        assert!(w.auction.limits[1] >= 10_000 && w.auction.limits[1] <= 30_000);
        let mut changes = Vec::new();
        let mut n = 0;
        while w.auction.open || w.auction.sold.is_some() {
            changes.extend(w.tick().auction_changes);
            n += 1;
            assert!(n < 100_000, "the auction never ends");
        }
        // The clock stood all the while, and ran again in the tick the
        // form closed.
        assert_eq!(w.clock.day_ticks, before + 1);
        assert_eq!(changes.last(), Some(&Change::Closed));
        let bids: Vec<_> = changes
            .iter()
            .filter_map(|c| if let Change::Bid(p) = c { Some(*p) } else { None })
            .collect();
        assert_ne!(bids, []);
        assert!(bids.iter().all(|p| (2..=3).contains(p)));
        assert!(bids.windows(2).all(|b| b[0] != b[1]), "no computer outbids itself");
        let winner = *bids.last().unwrap();
        assert_eq!(w.auction.bidder, winner);
        // Each bid was 5 % more.
        let mut price = 10_000;
        for _ in &bids {
            price += price / 20;
        }
        assert_eq!(w.auction.price, price);
        let paid = [30_000, 25_000][usize::from(player_byte(winner)) - 2] - price;
        assert_eq!(w.finances.money[usize::from(player_byte(winner)) - 1], paid);
        // The calls came in order, the third last.
        let calls: Vec<_> = changes
            .iter()
            .filter_map(|c| if let Change::Going(g) = c { Some(*g) } else { None })
            .collect();
        assert!(calls.contains(&SOLD));
        // The same seed gives the same auction.
        let mut again = world(false);
        run(&mut again, OPEN_TICKS.into());
        for _ in 0..n {
            again.tick();
        }
        assert_eq!(again, w);
    }

    #[test]
    fn waits_for_the_player() {
        let mut w = world(true);
        run(&mut w, OPEN_TICKS.into());
        assert!(w.auction.open && !w.auction.joined);
        let seed = w.random;
        run(&mut w, 50);
        // Nothing moves, not even the random numbers.
        assert_eq!(w.random, seed);
        assert_eq!(w.auction.going, NO_BID);
        let join = Command {
            player: 1,
            action: command::Action::Auction(Action::Bid),
        };
        assert_eq!(w.apply(join).auction_changes, [Change::Joined]);
        assert!(w.auction.joined);
    }

    #[test]
    fn the_human_bids() {
        let mut w = world(false);
        run(&mut w, OPEN_TICKS.into());
        let bid = || Command {
            player: 1,
            action: command::Action::Auction(Action::Bid),
        };
        let tick = w.apply(bid());
        assert_eq!(tick.auction_changes, [Change::Bid(1)]);
        assert_eq!((w.auction.bidder, w.auction.price, w.auction.going), (1, 10_500, 0));
        assert_eq!(w.auction.colour, 1);
        assert_eq!(w.auction.bid_pause, BID_TICKS);
        // The paddle's animation blocks the auction.
        let seed = w.random;
        run(&mut w, BID_TICKS.into());
        assert_eq!(w.random, seed);
        // Over the player's money the player can't bid.
        w.auction.bid_pause = 0;
        w.auction.limits[0] = 11_000;
        assert_eq!(w.apply(bid()).auction_changes, []);
        assert_eq!(w.auction.price, 10_500);
        w.auction.limits[0] = 1_000_000;
        assert_eq!(w.apply(bid()).auction_changes, [Change::Bid(1)]);
        assert_eq!(w.auction.price, 11_025);
        // Nobody outbids the player when the computers can't pay more.
        w.auction.bid_pause = 0;
        w.auction.limits[1..].fill(0);
        while w.auction.sold.is_none() {
            w.tick();
        }
        let money = w.finances.money[0];
        let mut messages = Vec::new();
        while w.auction.sold.is_some() {
            messages.extend(w.tick().messages);
        }
        assert!(!w.auction.open);
        assert_eq!(w.finances.money[0], money - 11_025);
        assert_eq!(w.book_value(BOOK_NEW_BUILDINGS), -11_025);
        assert_eq!((messages[0].number, messages[0].a, messages[0].x), (0x25, 1, 11_025));
    }

    #[test]
    fn passing_leaves_it_to_the_computers() {
        let mut w = world(false);
        run(&mut w, OPEN_TICKS.into());
        w.auction.limits[1..].fill(10_001);
        let pass = Command {
            player: 1,
            action: command::Action::Auction(Action::Leave),
        };
        let tick = w.apply(pass);
        assert_eq!(tick.auction_changes, [Change::Closed]);
        assert!(!w.auction.open);
        // Only a raise of 0 is below the limits.
        assert!((2..=8).contains(&w.auction.bidder));
        assert_eq!(w.auction.price, 10_000);
        assert_eq!(tick.messages.len(), 1);
    }

    fn auction_command(w: &mut World, action: Action) -> Tick {
        w.apply(Command {
            player: 1,
            action: command::Action::Auction(action),
        })
    }

    #[test]
    fn fix_auction_computer_limit() {
        for fixed in [false, true] {
            let mut w = world(false);
            w.fixes.set(Fix::AuctionComputerLimit, fixed);
            run(&mut w, OPEN_TICKS.into());
            // Computer 2 would pay 10 100: above the price, below its bid.
            w.auction.limits[1] = 10_100;
            w.auction.limits[2..].fill(0);
            let bids = run(&mut w, 3000)
                .iter()
                .flat_map(|t| t.auction_changes.clone())
                .filter(|c| matches!(c, Change::Bid(_)))
                .count();
            assert_eq!(bids, usize::from(!fixed));
            assert_eq!(w.auction.price, if fixed { 10_000 } else { 10_500 });
        }
    }

    #[test]
    fn fix_auction_pass() {
        for fixed in [false, true] {
            // The player leads and no computer would pay more.
            let mut w = world(false);
            w.fixes.set(Fix::AuctionPass, fixed);
            run(&mut w, OPEN_TICKS.into());
            assert_eq!(auction_command(&mut w, Action::Bid).auction_changes, [Change::Bid(1)]);
            w.auction.limits[1..].fill(0);
            let tick = auction_command(&mut w, Action::Pass);
            // The original gives the item to nobody.
            assert_eq!(tick.messages.len(), usize::from(fixed));
            assert_eq!(w.finances.money[0], if fixed { 50_000 - 10_500 } else { 50_000 });
            // After the third call the close button sells it again.
            let mut w = world(false);
            w.fixes.set(Fix::AuctionPass, fixed);
            run(&mut w, OPEN_TICKS.into());
            auction_command(&mut w, Action::Bid);
            w.auction.sold = Some(SOLD_TICKS);
            w.auction.limits[1..].fill(1_000_000);
            let tick = auction_command(&mut w, Action::Leave);
            assert_eq!(tick.messages.len(), usize::from(!fixed));
            assert_eq!(w.auction.bidder == 1, fixed);
        }
    }

    #[test]
    fn fix_tv_lights() {
        for fixed in [false, true] {
            let reading = |speaker| {
                let mut w = world(false);
                w.fixes.set(Fix::TvLights, fixed);
                let _ = w.apply(Command {
                    player: 1,
                    action: command::Action::Speaker(speaker),
                });
                let ticks = run(&mut w, usize::from(OPEN_TICKS) * 2);
                (w.random, ticks.iter().any(|t| t.tv_lights.is_some()))
            };
            let (read, lights) = reading(crate::news::Speaker::Reading);
            let (open, _) = reading(crate::news::Speaker::Open);
            // The original's lights take the game's random numbers.
            assert_eq!(read == open, fixed);
            assert_eq!(lights, !fixed);
        }
    }

    #[test]
    fn a_building_changes_hands() {
        let mut hausnr = vec![[0; 8]; 13];
        hausnr[5] = [1, 0, 0, 2, 3, 6, 0, 0];
        let mut w = World {
            tables: std::sync::Arc::new(crate::world::Tables {
                hausnr,
                ..crate::world::Tables::default()
            }),
            player_count: 3,
            events: vec![Event::default(); 41],
            ..World::default()
        };
        w.events[SALE_EVENT].arg = 3;
        w.events[SALE_EVENT].pos = (10, 20);
        for i in 10..12 {
            for j in 20..23 {
                w.island.buildings.set((i, j), 5);
                w.island.owners.set((i, j), 3);
            }
        }
        w.finances.money[..3].copy_from_slice(&[0, 9000, 0]);
        w.auction = Auction {
            item: BUILDING_SALE,
            bidder: 2,
            price: 8000,
            ..Auction::default()
        };
        let mut tick = Tick::default();
        w.award_auction(&mut tick);
        assert_eq!(w.finances.money[..3], [0, 1000, 8000]);
        assert_eq!(tick.redraw.len(), 6);
        assert!(tick.redraw.iter().all(|&p| w.island.owners.at(p) == 2));
        assert_eq!(w.island.owners.at((12, 20)), 0);
        assert_eq!(tick.messages[0].pos, Some((10, 20)));
    }

    #[test]
    fn fix_sale_booked() {
        for fixed in [false, true] {
            let mut w = World {
                player_count: 3,
                events: vec![Event::default(); 41],
                ..World::default()
            };
            w.fixes.set(Fix::SaleBooked, fixed);
            // The human sold; computer 2 bought.
            w.events[SALE_EVENT].arg = 1;
            w.finances.money[..3].copy_from_slice(&[0, 9000, 0]);
            w.auction = Auction {
                item: BUILDING_SALE,
                bidder: 2,
                price: 8000,
                ..Auction::default()
            };
            let mut tick = Tick::default();
            w.award_auction(&mut tick);
            assert_eq!(w.finances.money[..2], [8000, 1000]);
            assert_eq!(w.book_value(crate::accounts::BOOK_OTHER), if fixed { 8000 } else { 0 });
            assert_eq!(tick.books, fixed);
        }
    }

    #[test]
    fn fix_auction_queue() {
        for fixed in [false, true] {
            let mut w = world(false);
            w.fixes.set(Fix::AuctionQueue, fixed);
            // A second auction before the form opens.
            let mut tick = Tick::default();
            w.start_auction(10, 20_000, &mut tick);
            run(&mut w, OPEN_TICKS.into());
            assert!(w.auction.open);
            let first = if fixed { (7, 10_000) } else { (10, 20_000) };
            assert_eq!((w.auction.item, w.auction.price), first);
            assert_eq!(w.auction.queue, if fixed { vec![(10, 20_000)] } else { vec![] });
            // The queue is in the port's chunk.
            let lines = queue_lines(&w.auction);
            let chunk = formats::savegame::make_port_chunk(lines.iter().map(|(k, v)| (k.as_str(), v.as_str())));
            assert_eq!(queue_in(&chunk), w.auction.queue);
            // The player passes, nobody takes it; the next one opens a
            // second later.
            w.auction.limits[1..].fill(0);
            auction_command(&mut w, Action::Leave);
            assert!(!w.auction_busy());
            let ticks = run(&mut w, usize::from(OPEN_TICKS) + 1);
            assert_eq!(w.auction.open, fixed);
            if fixed {
                assert_eq!((w.auction.item, w.auction.price), (10, 20_000));
                assert_eq!(w.auction.queue, []);
                assert_eq!(ticks.last().unwrap().auction_changes, [Change::Opened]);
            }
        }
    }

    /// A world with player 2 bankrupt and their building on (10, 20) up
    /// for auction at 1000, only computer 8 with a limit above it, by 1.
    fn property_world(seed: u32, fixed: bool) -> World {
        let mut w = World {
            tables: std::sync::Arc::new(crate::world::Tables {
                hausnr: vec![[1, 0, 0, 1, 1, 1, 0, 0]; 0x80],
                hausdata: vec![[0; 10]; 0x80],
                ..crate::world::Tables::default()
            }),
            player_count: 8,
            events: vec![Event::default(); 41],
            random: Random::new(seed),
            ..World::default()
        };
        w.fixes.set(Fix::PropertyUnsold, fixed);
        w.events[PROPERTY_EVENT].arg = 2;
        w.island.buildings.set((10, 20), 5);
        w.island.owners.set((10, 20), 2);
        w.auction = Auction {
            item: PROPERTY_SALE,
            price: 1000,
            ..Auction::default()
        };
        w.auction.limits[7] = 1001;
        w
    }

    #[test]
    fn fix_property_unsold() {
        // A seed whose 101 tries all miss computer 8 at no raise.
        let seed = (0..1000)
            .find(|&seed| {
                let mut w = property_world(seed, false);
                w.sell_to_computers(true, &mut Tick::default());
                w.auction.bidder == 0
            })
            .unwrap();
        for fixed in [false, true] {
            let mut w = property_world(seed, fixed);
            w.sell_to_computers(true, &mut Tick::default());
            assert_eq!(w.auction.bidder, 0);
            // The original leaves it standing; cleared, it's demolished.
            assert_eq!(w.island.owners.at((10, 20)) == 2, !fixed);
            assert_eq!(w.island.buildings.at((10, 20)) == 5, !fixed);
        }
    }

    #[test]
    fn sale_event_busy() {
        let mut w = world(false);
        assert!(!w.sale_event_busy(SALE_EVENT) && !w.sale_event_busy(PROPERTY_EVENT));
        // Waiting, in its window.
        w.events[PROPERTY_EVENT].chance = 0x3fffc;
        w.events[PROPERTY_EVENT].last_day = 10;
        assert!(w.sale_event_busy(PROPERTY_EVENT) && !w.sale_event_busy(SALE_EVENT));
        // Past its window it never comes.
        w.days = 11;
        assert!(!w.sale_event_busy(PROPERTY_EVENT));
        // Its auction waits its turn.
        w.auction.queue.push((i32::from(BUILDING_SALE), 100));
        assert!(w.sale_event_busy(SALE_EVENT));
        w.auction.queue.clear();
        // Its auction waits for the form.
        w.auction.item = PROPERTY_SALE;
        assert!(w.sale_event_busy(PROPERTY_EVENT));
        w.auction.opening = None;
        assert!(!w.sale_event_busy(PROPERTY_EVENT));
    }
}
