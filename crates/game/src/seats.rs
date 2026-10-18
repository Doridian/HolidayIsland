//! The seats: which of the players 1 to 8 humans play, and who those
//! humans are. A port extension for lockstep multiplayer
//! (`docs/port-architecture.md`, "Stretch goal: network multiplayer"):
//! the original has one human, player 1, and the computers play players
//! 2 to the player count (`1110:1479`, 10a8:302e).
//!
//! A human is a **client**, with an id ([`Client`]) and a view of their
//! own. A seat (a player number, a colour) is a computer's or one or more
//! clients' (co-op: they play the same colour together). The **host** is
//! a client, in seat 1, and owns the world's options: the speed, the
//! options form's switches, the fixes, loading a game and the seats.
//! Everything else a client sends acts for their seat
//! ([`ClientCommand`], [`World::apply_client`]): the command's `player`
//! is the seat, as for the one human of the original.
//!
//! The clients change by commands ([`Action::Seat`]): one joins a seat
//! (a computer's, or one that has clients already), leaves it, or
//! disconnects and rejoins. A seat no client is at is the computers':
//! they play it until one of its clients rejoins
//! ([`World::is_computer`]). Seat 1 always has the host.
//!
//! With more than one client, the fixes of
//! [`crate::enhance::SINGLE_PLAYER_ONLY`] are on
//! ([`crate::enhance::Fixes::for_humans`]): the clients share no view or
//! UI the world could read. [`World::follow_seats`] switches them on as
//! the second client joins, and the ones it switched back off as the last
//! but one leaves. The seats are the world's, in its hash, and a saved
//! game keeps them in the port's chunk (`seats.` lines,
//! [`Seats::chunk_lines`]); an original file loads as the host alone in
//! seat 1.
//!
//! Where the original means "the human" by player 1 (the money checks,
//! the books, the costs, the computers' handicaps, the build menu, the
//! insurance), the port asks [`World::is_human`], so a human in seats 2
//! to 8 plays as one; what the original keeps once, for player 1, each
//! of those seats has again ([`SeatSlots`]). A seat that went out
//! (bankrupt) with several humans is a spectator's: its clients keep
//! their views and send no more commands for it ([`World::spectates`]).
//!
//! [`Action::Seat`]: crate::command::Action::Seat

use crate::build_menu::BuildMenu;
use crate::command::{Action, Command};
use crate::enhance::{Fix, Fixes, SINGLE_PLAYER_ONLY};
use crate::roads::RoadDrag;
use crate::simulation::Tick;
use crate::world::{FIRST_BOOK, World};

/// A human client's id. The transport hands them out; the world only
/// tells them apart.
pub type Client = u16;

/// The host's id in a game the host started: the one human of a
/// single-player game.
pub const HOST: Client = 0;

/// The host's seat, the original's human. // 1110:0074, player 1
pub const HOST_SEAT: u8 = 1;

/// A client at a seat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Member {
    pub client: Client,
    /// The player they play, 1 to 8.
    pub seat: u8,
    /// Connected. A client that disconnected keeps their seat; while
    /// none of its clients is there, a computer plays it.
    pub present: bool,
}

/// The seats: the human clients, each at a seat, and the host.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Seats {
    /// The clients, by id.
    members: Vec<Member>,
    host: Client,
    /// The fixes of [`SINGLE_PLAYER_ONLY`] that more than one client
    /// switched on ([`World::follow_seats`]), which go off again when one
    /// is left.
    forced: Fixes,
}

impl Default for Seats {
    /// The original's: the host alone, in seat 1.
    fn default() -> Self {
        Seats {
            members: vec![Member {
                client: HOST,
                seat: HOST_SEAT,
                present: true,
            }],
            host: HOST,
            forced: Fixes::ORIGINAL,
        }
    }
}

/// A change of the seats ([`Action::Seat`]).
///
/// [`Action::Seat`]: crate::command::Action::Seat
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SeatChange {
    /// A new client takes `seat`, 1 to the player count: a computer's,
    /// which they then play, or one with clients already (co-op).
    Join { client: Client, seat: u8 },
    /// A client leaves for good. Their seat goes back to the computers
    /// when nobody else is at it. Not the host.
    Leave(Client),
    /// A client's connection dropped: they keep their seat, which a
    /// computer plays while none of its clients is there. Not the host.
    Disconnect(Client),
    /// A client that disconnected is back at their seat.
    Rejoin(Client),
}

/// What a client sends: the action, which acts for their seat. Lockstep
/// sends `(tick, client, action)`; [`World::apply_client`] makes it a
/// [`Command`] for the seat.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClientCommand {
    pub client: Client,
    pub action: Action,
}

impl Seats {
    /// The host's id.
    #[must_use]
    pub fn host(&self) -> Client {
        self.host
    }

    /// The clients, by id.
    #[must_use]
    pub fn members(&self) -> &[Member] {
        &self.members
    }

    /// `client`, if they're one of the game's.
    #[must_use]
    pub fn member(&self, client: Client) -> Option<&Member> {
        self.members.iter().find(|m| m.client == client)
    }

    /// How many human clients the game has, away or not: what
    /// [`Fixes::for_humans`] counts. Two at one seat are two.
    #[must_use]
    pub fn humans(&self) -> usize {
        self.members.len()
    }

    /// More than one human client.
    #[must_use]
    pub fn multiplayer(&self) -> bool {
        self.humans() > 1
    }

    /// Whether a client that's there plays `seat`.
    #[must_use]
    pub fn is_human(&self, seat: u8) -> bool {
        self.members.iter().any(|m| m.seat == seat && m.present)
    }

    /// The fixes more than one client switched on ([`Seats::forced`]).
    #[must_use]
    pub fn forced(&self) -> Fixes {
        self.forced
    }

    /// Carries out `change` if it can be: whether it changed anything.
    /// A new client's seat is 1 to `player_count`; the host never leaves
    /// or disconnects.
    fn change(&mut self, change: SeatChange, player_count: u8) -> bool {
        match change {
            SeatChange::Join { client, seat } => {
                if self.member(client).is_some() || !(1..=player_count.min(8)).contains(&seat) {
                    return false;
                }
                let at = self.members.partition_point(|m| m.client < client);
                self.members.insert(
                    at,
                    Member {
                        client,
                        seat,
                        present: true,
                    },
                );
                true
            }
            SeatChange::Leave(client) => {
                let before = self.members.len();
                if client != self.host {
                    self.members.retain(|m| m.client != client);
                }
                self.members.len() != before
            }
            SeatChange::Disconnect(client) | SeatChange::Rejoin(client) => {
                let present = matches!(change, SeatChange::Rejoin(_));
                match self.members.iter_mut().find(|m| m.client == client) {
                    Some(m) if m.client != self.host && m.present != present => {
                        m.present = present;
                        true
                    }
                    _ => false,
                }
            }
        }
    }

    /// The port's chunk's lines for the seats: `seats.host=<id>`,
    /// `seats.clients=<id>:<seat>` for each (`:away` after one that's
    /// disconnected), and `seats.forced=` the keys of [`Seats::forced`].
    #[must_use]
    pub fn chunk_lines(&self) -> Vec<(String, String)> {
        let clients = self
            .members
            .iter()
            .map(|m| {
                let away = if m.present { "" } else { ":away" };
                format!("{}:{}{away}", m.client, m.seat)
            })
            .collect::<Vec<_>>()
            .join(" ");
        let forced = self.forced.iter().map(|f| f.info().key).collect::<Vec<_>>().join(" ");
        vec![
            (format!("{PREFIX}host"), self.host.to_string()),
            (format!("{PREFIX}clients"), clients),
            (format!("{PREFIX}forced"), forced),
        ]
    }

    /// The seats a saved game's `trailing` bytes keep
    /// ([`Seats::chunk_lines`]); none for a file without them (any
    /// original file) or whose lines don't read, which then loads as the
    /// host alone in seat 1.
    #[must_use]
    pub fn from_chunk(trailing: &[u8]) -> Option<Seats> {
        let lines = formats::savegame::port_chunk(trailing)?;
        let get = |key: &str| {
            lines
                .iter()
                .find(|(k, _)| k.strip_prefix(PREFIX) == Some(key))
                .map(|(_, v)| v.as_str())
        };
        let host: Client = get("host")?.parse().ok()?;
        let mut members = Vec::new();
        for c in get("clients")?.split_whitespace() {
            let mut parts = c.split(':');
            let client: Client = parts.next()?.parse().ok()?;
            let seat: u8 = parts.next()?.parse().ok()?;
            let present = match parts.next() {
                None => true,
                Some("away") => false,
                Some(_) => return None,
            };
            if !(1..=8).contains(&seat) || members.iter().any(|m: &Member| m.client == client) {
                return None;
            }
            members.push(Member { client, seat, present });
        }
        members.sort_by_key(|m| m.client);
        let host_member = members.iter().find(|m| m.client == host)?;
        if host_member.seat != HOST_SEAT || !host_member.present {
            return None;
        }
        let mut forced = Fixes::ORIGINAL;
        for f in get("forced")
            .unwrap_or_default()
            .split_whitespace()
            .filter_map(Fix::from_key)
        {
            forced.set(f, true);
        }
        Some(Seats { members, host, forced })
    }

    /// The seats of a game loaded from a file whose chunk keeps `saved`
    /// (none for a file without them) while `self` are the running
    /// game's: the running game's clients stay as they are, with its
    /// host; the file's others come back disconnected, at their seats,
    /// until they rejoin. The fixes the clients forced are the file's
    /// (the running game's for a file without seats, which keeps its
    /// fixes).
    #[must_use]
    pub fn loaded(&self, saved: Option<Seats>) -> Seats {
        let mut seats = self.clone();
        if let Some(saved) = saved {
            for m in saved.members {
                if seats.member(m.client).is_none() {
                    let at = seats.members.partition_point(|n| n.client < m.client);
                    seats.members.insert(at, Member { present: false, ..m });
                }
            }
            seats.forced = saved.forced;
        }
        seats
    }
}

/// The prefix of the seats' lines in the port's chunk.
const PREFIX: &str = "seats.";

/// What the original keeps once, as player 1's (the human's), for one of
/// seats 2 to 8, which a human may play here ([`World::seat_slots`]): a
/// port extension, so a human in seat 2 has books, insurance, a road
/// drag and a build menu of their own. Seat 1 keeps the original's
/// fields. Each is the smallest extension of the original's one slot:
/// the same data again, for each seat. Saved in the port's chunk
/// (`seat.<p>.` lines, [`World::seat_lines`]) when it isn't as a new
/// game has it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct SeatSlots {
    /// The books' sums by kind, as [`crate::world::Books::year`].
    /// // [1110:6f04]+0x49
    pub books: [i32; 29],
    /// Insured. // 1110:0072
    pub insured: bool,
    /// The seat's money menu is shown, as [`World::money_menu_shown`]
    /// (with several humans `fix.year_total` is on, and nothing reads it).
    pub money_menu_shown: bool,
    /// The seat's road drag. // 1110:634c, 1110:704e, 1110:704f
    pub drag: RoadDrag,
    /// The damage of the disaster going on, and whether the seat was
    /// insured when it began. // 1110:7050, 1110:7054
    pub damage: i32,
    pub damage_insured: bool,
    /// The seat's build menu: none until a client first takes the seat
    /// ([`SeatChange::Join`]), which then gets a copy of seat 1's lists;
    /// from then on it has its own one-of-a-kind buildings and prizes,
    /// and what an event gives everyone. // *[1110:6516]
    pub build_menu: Option<Box<BuildMenu>>,
}

/// The prefix of the seat slots' lines in the port's chunk.
const SLOT_PREFIX: &str = "seat.";

impl SeatSlots {
    /// The slots a saved game's `trailing` bytes keep in the port's chunk
    /// ([`World::seat_lines`]); a new game's for a seat without lines
    /// (any original file).
    #[must_use]
    pub fn from_chunk(trailing: &[u8]) -> [SeatSlots; 7] {
        let mut slots: [SeatSlots; 7] = Default::default();
        let Some(lines) = formats::savegame::port_chunk(trailing) else {
            return slots;
        };
        for (key, value) in lines {
            let Some((seat, field)) = key.strip_prefix(SLOT_PREFIX).and_then(|k| k.split_once('.')) else {
                continue;
            };
            let Some(s) = seat
                .parse::<usize>()
                .ok()
                .and_then(|p| p.checked_sub(2))
                .and_then(|i| slots.get_mut(i))
            else {
                continue;
            };
            let flags = |v: &str| -> Option<Vec<bool>> {
                v.split(',')
                    .map(|f| match f {
                        "1" => Some(true),
                        "0" => Some(false),
                        _ => None,
                    })
                    .collect()
            };
            match field {
                "books" => {
                    let n: Option<Vec<i32>> = value.split(',').map(|v| v.parse().ok()).collect();
                    if let Some(books) = n.and_then(|n| <[i32; 29]>::try_from(n).ok()) {
                        s.books = books;
                    }
                }
                "insured" => {
                    if let Some(&[on]) = flags(&value).as_deref() {
                        s.insured = on;
                    }
                }
                "money_menu" => {
                    if let Some(&[on]) = flags(&value).as_deref() {
                        s.money_menu_shown = on;
                    }
                }
                "drag" => {
                    let n: Option<Vec<u8>> = value.split(',').map(|v| v.parse().ok()).collect();
                    if let Some(&[on, ground, form]) = n.as_deref() {
                        s.drag = RoadDrag {
                            on: on != 0,
                            ground,
                            form,
                        };
                    }
                }
                "damage" => {
                    if let Some((amount, insured)) = value.split_once(',')
                        && let (Ok(amount), Some(&[insured])) = (amount.parse(), flags(insured).as_deref())
                    {
                        s.damage = amount;
                        s.damage_insured = insured;
                    }
                }
                "build_menu" => {
                    let parts: Option<Vec<Vec<u8>>> = value.split(',').map(unhex).collect();
                    if let Some([counts, rows, lengths]) = parts.as_deref() {
                        s.build_menu = BuildMenu::from_bytes(counts, rows, lengths).map(Box::new);
                    }
                }
                _ => {}
            }
        }
        slots
    }
}

/// `bytes` as hex digits.
fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// The bytes of [`hex`]'s digits, none if they aren't.
fn unhex(digits: &str) -> Option<Vec<u8>> {
    if !digits.len().is_multiple_of(2) {
        return None;
    }
    (0..digits.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(digits.get(i..i + 2)?, 16).ok())
        .collect()
}

/// Whether only the host may send `action`: the world's options (the
/// speed, the pause, the options form's switches, the overview map's new
/// people, the auction's timeout, the fixes), loading a game, and the
/// seats but a client's own leaving.
fn hosts_only(action: &Action, client: Client) -> bool {
    match action {
        Action::Seat(SeatChange::Leave(c)) => *c != client,
        Action::Speed(_)
        | Action::Pause(_)
        | Action::SetOption(..)
        | Action::AuctionTimeout(_)
        | Action::SetFix(..)
        | Action::Load(_)
        | Action::Seat(_) => true,
        _ => false,
    }
}

impl World {
    /// Whether a computer plays player `p`: one of 2 to the player count
    /// (the original's computers, 10a8:302e) that no client plays now.
    #[must_use]
    pub fn is_computer(&self, p: u8) -> bool {
        (2..=self.player_count.min(8)).contains(&p) && !self.seats.is_human(p)
    }

    /// Carries out what `command`'s client sent, for their seat
    /// ([`World::apply`] of a [`Command`] whose `player` is the seat).
    /// Nothing happens for a client the game doesn't have, or that's
    /// disconnected, or for one that isn't the host sending what only the
    /// host may ([`Seats`]): every peer ignores it the same way.
    ///
    /// A seat that went out with several humans in the game
    /// ([`World::spectates`]) is a spectator's: its clients' commands do
    /// nothing, but a client's own leaving and the host's own commands.
    pub fn apply_client(&mut self, command: ClientCommand) -> Tick {
        let Some(member) = self.seats.member(command.client).copied() else {
            return Tick::default();
        };
        let host_only = hosts_only(&command.action, command.client);
        if !member.present || (command.client != self.seats.host && host_only) {
            return Tick::default();
        }
        let leaving = matches!(command.action, Action::Seat(SeatChange::Leave(c)) if c == command.client);
        if self.spectates(member.seat) && !host_only && !leaving {
            return Tick::default();
        }
        self.apply(Command {
            player: member.seat,
            action: command.action,
        })
    }

    /// The seats change ([`Action::Seat`]), and the forced fixes follow
    /// the number of clients ([`World::follow_seats`]).
    ///
    /// [`Action::Seat`]: crate::command::Action::Seat
    ///
    /// A client taking seat 2 to 8 for the first time gives it a build
    /// menu, a copy of seat 1's ([`SeatSlots::build_menu`]).
    pub(crate) fn seat_change(&mut self, change: SeatChange, tick: &mut Tick) {
        if self.seats.change(change, self.player_count) {
            if let SeatChange::Join { seat, .. } = change
                && self.slots(seat).is_some_and(|s| s.build_menu.is_none())
            {
                let _ = self.build_menu_mut(seat);
            }
            self.follow_seats(tick);
        }
    }

    /// The fixes follow the number of human clients
    /// ([`Fixes::for_humans`]): with more than one, those of
    /// [`SINGLE_PLAYER_ONLY`] that are off go on, and the seats remember
    /// them ([`Seats::forced`]); with one, those go off again, as the host
    /// had them.
    pub fn follow_seats(&mut self, tick: &mut Tick) {
        self.follow_seats_with(|w, f, on| w.switch_fix(f, on, tick));
    }

    /// [`World::follow_seats`] for a world just loaded
    /// ([`World::load_map`]), whose loader hands the views their people
    /// afterwards.
    pub(crate) fn follow_loaded_seats(&mut self) {
        self.follow_seats_with(|w, f, on| w.fixes.set(f, on));
    }

    fn follow_seats_with(&mut self, mut switch: impl FnMut(&mut World, Fix, bool)) {
        if self.seats.multiplayer() {
            // The TV's pause was one human's dialog; with several the
            // news and the TV pause nothing (`World::tv_shown`).
            self.clock.paused = false;
            for &f in SINGLE_PLAYER_ONLY {
                if !self.fixes.has(f) {
                    self.seats.forced.set(f, true);
                    switch(self, f, true);
                }
            }
        } else {
            for f in self.seats.forced.iter() {
                switch(self, f, false);
            }
            self.seats.forced = Fixes::ORIGINAL;
            // The host's pause is for several humans: one human's view
            // stops its own clock (`Clock::stopped`).
            self.clock.stopped = false;
        }
        debug_assert_eq!(self.fixes, self.fixes.for_humans(self.seats.humans()));
    }

    /// A fix switched by command ([`Action::SetFix`]): with more than one
    /// client, one of [`SINGLE_PLAYER_ONLY`] stays on. A fix the host
    /// switches is no longer one the seats forced.
    ///
    /// [`Action::SetFix`]: crate::command::Action::SetFix
    pub(crate) fn set_fix(&mut self, fix: Fix, on: bool, tick: &mut Tick) {
        if !on && self.seats.multiplayer() && SINGLE_PLAYER_ONLY.contains(&fix) {
            return;
        }
        self.seats.forced.set(fix, false);
        self.switch_fix(fix, on, tick);
    }

    /// Switches `fix`: switched on, `fix.view_people` hands the views
    /// their own people.
    fn switch_fix(&mut self, fix: Fix, on: bool, tick: &mut Tick) {
        let split = self.people_split();
        self.fixes.set(fix, on);
        if !split && self.people_split() {
            self.hand_over_view_people(tick);
        }
    }
}

impl World {
    /// Whether a human plays seat `p`: one of its clients is there. The
    /// original's human is player 1, the host's seat; where it means "the
    /// human" by player 1 (the money checks, the books, the costs, the
    /// computers' handicaps, the build menu), the port asks this, so a
    /// human in seats 2 to 8 plays as one, and a computer that plays a
    /// seat while its clients are away as a computer. With one human
    /// client it's `p == 1`.
    #[must_use]
    pub fn is_human(&self, p: u8) -> bool {
        // BUG(original): the human is player 1 and 2 and up the computers, with their handicaps (design issue for multiplayer: every seat a client plays is a human's).
        self.seats.is_human(p)
    }

    /// The seats humans play, in order: 1, and those of 2 to the player
    /// count a client is at.
    pub fn human_seats(&self) -> impl Iterator<Item = u8> + '_ {
        (1..=self.player_count.clamp(1, 8)).filter(|&p| self.is_human(p))
    }

    /// Whether seat `seat` is a spectator's: with several humans in the
    /// game, it went out (bankrupt, `*1110:6f24 + 0x7f + p`). Its clients
    /// keep their views and send no commands for it
    /// ([`World::apply_client`]). With one human the original's game ends
    /// instead.
    #[must_use]
    pub fn spectates(&self, seat: u8) -> bool {
        self.seats.multiplayer() && self.player_out(seat) != 0
    }

    /// Seat `p`'s slots ([`SeatSlots`]), for 2 to 8.
    #[must_use]
    pub fn slots(&self, p: u8) -> Option<&SeatSlots> {
        usize::from(p).checked_sub(2).and_then(|i| self.seat_slots.get(i))
    }

    pub(crate) fn slots_mut(&mut self, p: u8) -> Option<&mut SeatSlots> {
        usize::from(p).checked_sub(2).and_then(|i| self.seat_slots.get_mut(i))
    }

    /// Seat `p`'s road drag: the original's fields for seat 1 (and any
    /// other `p` outside 2 to 8), the seat's slot for a human in 2 to 8.
    #[must_use]
    pub fn road_drag(&self, p: u8) -> RoadDrag {
        // BUG(original): one road drag in the world, the human's (design issue for multiplayer: each human seat's own).
        match self.slots(p) {
            Some(s) if self.is_human(p) => s.drag,
            _ => RoadDrag {
                on: self.road_dragging,
                ground: self.road_drag_ground,
                form: self.road_drag_form,
            },
        }
    }

    /// Sets seat `p`'s road drag ([`World::road_drag`]).
    pub(crate) fn set_road_drag(&mut self, p: u8, drag: RoadDrag) {
        let human = self.is_human(p);
        match self.slots_mut(p) {
            Some(s) if human => s.drag = drag,
            _ => {
                self.road_dragging = drag.on;
                self.road_drag_ground = drag.ground;
                self.road_drag_form = drag.form;
            }
        }
    }

    /// A road `p` built ends a drag (1078:2396, 1080:3216): the
    /// original's one flag, which the computers' roads end too; a human
    /// in seats 2 to 8 ends their own.
    pub(crate) fn end_road_drag(&mut self, p: u8) {
        let drag = RoadDrag {
            on: false,
            ..self.road_drag(p)
        };
        self.set_road_drag(p, drag);
    }

    /// Player `p`'s books' sums ([`crate::world::Books::year`]): seat 1's
    /// the original's, 2 to 8 their slot's (which only a human's
    /// commands and costs book into).
    #[must_use]
    pub fn books_of(&self, p: u8) -> Option<&[i32; 29]> {
        if p == 1 {
            Some(&self.books.year)
        } else {
            self.slots(p).map(|s| &s.books)
        }
    }

    pub(crate) fn books_of_mut(&mut self, p: u8) -> Option<&mut [i32; 29]> {
        if p == 1 {
            Some(&mut self.books.year)
        } else {
            self.slots_mut(p).map(|s| &mut s.books)
        }
    }

    /// Adds `amount` to human `p`'s books' `kind`, as
    /// [`World::book`] does player 1's. // 1070:1419
    pub fn book_for(&mut self, p: u8, kind: u8, amount: i32) {
        if let Some(v) = kind
            .checked_sub(FIRST_BOOK)
            .and_then(|k| self.books_of_mut(p)?.get_mut(usize::from(k)))
        {
            *v = v.wrapping_add(amount);
        }
    }

    /// `p`'s books' `kind`, 0 outside 0x52–0x6e or for a player without
    /// books. // 1070:13f1
    #[must_use]
    pub fn book_value_of(&self, p: u8, kind: u8) -> i32 {
        kind.checked_sub(FIRST_BOOK)
            .and_then(|k| self.books_of(p)?.get(usize::from(k)))
            .copied()
            .unwrap_or(0)
    }

    /// Whether `p` is insured: player 1 by `1110:0072`, 2 to 8 by their
    /// slot.
    #[must_use]
    pub fn insured_of(&self, p: u8) -> bool {
        if p == 1 {
            self.insured
        } else {
            self.slots(p).is_some_and(|s| s.insured)
        }
    }

    /// Seat `p`'s build menu: seat 1's the original's, a human seat's of
    /// 2 to 8 its own once it has one ([`SeatSlots::build_menu`]).
    #[must_use]
    pub fn build_menu_of(&self, p: u8) -> &BuildMenu {
        match self.slots(p) {
            Some(SeatSlots {
                build_menu: Some(menu), ..
            }) => menu,
            _ => &self.build_menu,
        }
    }

    /// [`World::build_menu_of`] to change: a seat of 2 to 8 without one
    /// yet gets a copy of seat 1's first.
    pub(crate) fn build_menu_mut(&mut self, p: u8) -> &mut BuildMenu {
        if p == 1 || self.slots(p).is_none() {
            return &mut self.build_menu;
        }
        let base = Box::new(self.build_menu.clone());
        let s = self.slots_mut(p).expect("seat 2 to 8");
        s.build_menu.get_or_insert(base)
    }

    /// The port's chunk's lines for the seats' slots: `seat.<p>.books=`
    /// the 29 sums, `.insured=`, `.money_menu=`, `.drag=` on, ground,
    /// form, `.damage=` amount, insured, and `.build_menu=` its three
    /// fields' bytes in hex, each only when it isn't a new game's.
    #[must_use]
    pub fn seat_lines(&self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        let flag = |b: bool| if b { "1" } else { "0" };
        for (i, s) in self.seat_slots.iter().enumerate() {
            let mut put = |key: &str, value: String| out.push((format!("{SLOT_PREFIX}{}.{key}", i + 2), value));
            let new = SeatSlots::default();
            if s.books != new.books {
                let books = s.books.iter().map(i32::to_string).collect::<Vec<_>>().join(",");
                put("books", books);
            }
            if s.insured {
                put("insured", flag(s.insured).to_owned());
            }
            if s.money_menu_shown {
                put("money_menu", flag(s.money_menu_shown).to_owned());
            }
            if s.drag != new.drag {
                let d = s.drag;
                put("drag", format!("{},{},{}", u8::from(d.on), d.ground, d.form));
            }
            if s.damage != 0 || s.damage_insured {
                put("damage", format!("{},{}", s.damage, flag(s.damage_insured)));
            }
            if let Some(menu) = &s.build_menu {
                let [counts, rows, lengths] = menu.bytes();
                put(
                    "build_menu",
                    format!("{},{},{}", hex(&counts), hex(&rows), hex(&lengths)),
                );
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn join(client: Client, seat: u8) -> Action {
        Action::Seat(SeatChange::Join { client, seat })
    }

    fn from(client: Client, action: Action) -> ClientCommand {
        ClientCommand { client, action }
    }

    #[test]
    fn the_original_has_one_human() {
        let w = World {
            player_count: 4,
            ..World::default()
        };
        assert_eq!(w.seats.humans(), 1);
        assert!(!w.seats.multiplayer());
        assert!(w.seats.is_human(1));
        assert!(!w.is_computer(1));
        assert!((2..=4).all(|p| w.is_computer(p)));
        assert!(!w.is_computer(5));
    }

    #[test]
    fn joining_leaving_and_the_forced_fixes() {
        let mut w = World {
            player_count: 4,
            ..World::default()
        };
        let mut tick = Tick::default();
        // Only the host seats people.
        w.apply_client(from(HOST, join(7, 3)));
        assert_eq!(w.seats.humans(), 2);
        assert!(!w.is_computer(3));
        assert_eq!(w.fixes, Fixes::ORIGINAL.for_humans(2));
        assert_eq!(w.seats.forced(), Fixes::ORIGINAL.for_humans(2));
        w.apply_client(from(7, join(8, 2)));
        assert_eq!(w.seats.humans(), 2);
        // A seat past the player count, or a client twice, doesn't join.
        w.seat_change(SeatChange::Join { client: 8, seat: 5 }, &mut tick);
        w.seat_change(SeatChange::Join { client: 7, seat: 2 }, &mut tick);
        assert_eq!(w.seats.humans(), 2);
        // A forced fix stays on.
        w.apply_client(from(HOST, Action::SetFix(Fix::ViewNews, false)));
        assert!(w.fixes.has(Fix::ViewNews));
        // Disconnected: the computer plays the seat, the fixes stay.
        w.apply_client(from(HOST, Action::Seat(SeatChange::Disconnect(7))));
        assert!(w.is_computer(3));
        assert!(w.fixes.has(Fix::ViewPeople));
        // Its commands do nothing meanwhile.
        let speed = w.clock.speed;
        w.apply_client(from(7, Action::Seat(SeatChange::Leave(7))));
        assert_eq!(w.seats.humans(), 2);
        w.apply_client(from(HOST, Action::Seat(SeatChange::Rejoin(7))));
        assert!(!w.is_computer(3));
        // A client may leave by themselves; the fixes go back.
        w.apply_client(from(7, Action::Seat(SeatChange::Leave(7))));
        assert_eq!(w.seats, Seats::default());
        assert_eq!(w.fixes, Fixes::ORIGINAL);
        assert_eq!(w.clock.speed, speed);
        // The host never leaves.
        w.apply_client(from(HOST, Action::Seat(SeatChange::Leave(HOST))));
        assert_eq!(w.seats.humans(), 1);
    }

    #[test]
    fn a_fix_the_host_switched_stays() {
        let mut w = World {
            player_count: 4,
            ..World::default()
        };
        w.apply_client(from(HOST, join(1, 2)));
        // The host switches a forced one on for themselves: it stays on
        // when the game is theirs alone again.
        w.apply_client(from(HOST, Action::SetFix(Fix::YearTotal, true)));
        w.apply_client(from(HOST, Action::Seat(SeatChange::Leave(1))));
        let mut only = Fixes::ORIGINAL;
        only.set(Fix::YearTotal, true);
        assert_eq!(w.fixes, only);
    }

    #[test]
    fn host_only() {
        let mut w = World {
            player_count: 4,
            ..World::default()
        };
        // Co-op: client 5 shares seat 1 with the host, client 6 plays 2.
        w.apply_client(from(HOST, join(5, 1)));
        w.apply_client(from(HOST, join(6, 2)));
        for client in [5, 6] {
            w.apply_client(from(client, Action::Speed(3)));
            w.apply_client(from(client, Action::SetFix(Fix::SiteEffect, true)));
            w.apply_client(from(
                client,
                Action::SetOption(crate::optionen::WorldOption::Disasters, false),
            ));
            w.apply_client(from(client, Action::Seat(SeatChange::Leave(HOST))));
            w.apply_client(from(client, join(9, 3)));
        }
        assert_ne!(w.clock.speed, 3);
        assert!(!w.fixes.has(Fix::SiteEffect));
        assert!(w.disasters);
        assert_eq!(w.seats.humans(), 3);
        w.apply_client(from(HOST, Action::Speed(3)));
        assert_eq!(w.clock.speed, 3);
        // A client the game doesn't have does nothing.
        w.apply_client(from(42, Action::Speed(4)));
        assert_eq!(w.clock.speed, 3);
    }

    #[test]
    fn co_op_hands_the_seat_over_last() {
        let mut w = World {
            player_count: 4,
            ..World::default()
        };
        w.apply_client(from(HOST, join(5, 2)));
        w.apply_client(from(HOST, join(6, 2)));
        assert_eq!(w.seats.humans(), 3);
        // Their commands act for seat 2.
        w.apply_client(from(
            6,
            Action::MoneyMenu(crate::geldmenu::Action::Advertising { more: true }),
        ));
        assert_eq!(w.advertising_level(2), 1);
        w.apply_client(from(HOST, Action::Seat(SeatChange::Disconnect(5))));
        assert!(!w.is_computer(2));
        w.apply_client(from(6, Action::Seat(SeatChange::Leave(6))));
        assert!(w.is_computer(2));
        // Still two clients: the fixes stay.
        assert!(w.seats.multiplayer());
        w.apply_client(from(HOST, Action::Seat(SeatChange::Rejoin(5))));
        assert!(!w.is_computer(2));
    }

    #[test]
    fn saved_slots() {
        let mut w = World {
            player_count: 4,
            ..World::default()
        };
        assert!(w.seat_lines().is_empty(), "a new game's slots write nothing");
        w.apply_client(from(HOST, join(9, 3)));
        w.book_for(3, 0x5f, 1234);
        w.set_road_drag(
            3,
            RoadDrag {
                on: true,
                ground: 2,
                form: 1,
            },
        );
        if let Some(s) = w.slots_mut(3) {
            s.insured = true;
            s.damage = -77;
            s.damage_insured = true;
            s.money_menu_shown = true;
        }
        // Seat 1's are the original's fields.
        assert!(!w.road_dragging && w.road_drag(3).on);
        let chunk = formats::savegame::make_port_chunk(w.seat_lines().iter().map(|(k, v)| (k.as_str(), v.as_str())));
        assert_eq!(SeatSlots::from_chunk(&chunk), w.seat_slots);
        assert_eq!(w.book_value_of(3, 0x5f), 1234);
        assert_eq!(w.book_value(0x5f), 0);
        // Without a client there the seat is a computer's, whose drag is
        // the original's one flag.
        w.apply_client(from(9, Action::Seat(SeatChange::Leave(9))));
        assert!(!w.road_drag(3).on);
    }

    #[test]
    fn saved_seats() {
        let mut w = World {
            player_count: 4,
            ..World::default()
        };
        w.apply_client(from(HOST, join(3, 1)));
        w.apply_client(from(HOST, join(9, 4)));
        w.apply_client(from(HOST, Action::Seat(SeatChange::Disconnect(9))));
        let chunk =
            formats::savegame::make_port_chunk(w.seats.chunk_lines().iter().map(|(k, v)| (k.as_str(), v.as_str())));
        assert_eq!(Seats::from_chunk(&chunk), Some(w.seats.clone()));
        // An original file has none.
        assert_eq!(Seats::from_chunk(&[]), None);
        // Loaded by the host alone: the others come back away.
        let loaded = Seats::default().loaded(Seats::from_chunk(&chunk));
        assert_eq!(loaded.humans(), 3);
        assert!(loaded.members().iter().all(|m| m.present == (m.client == HOST)));
        assert_eq!(loaded.forced(), w.seats.forced());
        assert_eq!(Seats::default().loaded(None), Seats::default());
    }
}
