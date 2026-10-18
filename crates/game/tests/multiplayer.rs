//! Several human clients in one process (`game::seats`): one world per
//! peer, each with the same commands, and a view per client. The peers
//! stay the same world, the fixes several humans need are on, only the
//! host changes the world's options, and a computer plays a seat while
//! its clients are away. Runs on a scenario; skipped when `orig/` isn't
//! there.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::Arc;

use formats::data::{animdat1, animdat4, darstell, groundnr, hausdata, hausnr, hausprei, modatyp};
use formats::savegame::SaveGame;
use game::auction;
use game::command::{Action, Command};
use game::enhance::{Fix, Fixes, SINGLE_PLAYER_ONLY};
use game::geldmenu;
use game::hintergrund::{SCENARIO_FILES, load_world, scenario_events, start_scenario};
use game::people::Viewpoint;
use game::people::view::{Looking, ViewPeople};
use game::savegame::MapView;
use game::seats::{Client, ClientCommand, HOST, SeatChange};
use game::startform::Settings;
use game::world::{Tables, World};
use vcl::random::Random;

fn orig() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../orig")
}

/// Reads `name` from `orig/`, whatever the case of its letters.
fn read(name: &str) -> Vec<u8> {
    let mut path = orig();
    for part in name.split('/') {
        let found = std::fs::read_dir(&path)
            .unwrap()
            .filter_map(Result::ok)
            .find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(part))
            .unwrap_or_else(|| panic!("{name} not found"));
        path = found.path();
    }
    std::fs::read(path).unwrap()
}

fn tables() -> Arc<Tables> {
    Arc::new(Tables {
        hausnr: hausnr(&read("DATA/HAUSNR.DAT")).unwrap(),
        hausdata: hausdata(&read("DATA/HAUSDATA.DAT")).unwrap(),
        hausprei: hausprei(&read("DATA/HAUSPREI.DAT")).unwrap(),
        groundnr: groundnr(&read("DATA/GROUNDNR.DAT")).unwrap(),
        animdat1: animdat1(&read("DATA/ANIMDAT1.DAT")).unwrap(),
        animdat4: animdat4(&read("DATA/ANIMDAT4.DAT")).unwrap(),
        form_korrektur: darstell(&read("DATA/DARSTELL.DAT")).unwrap().form_korrektur,
        road_cost: modatyp(&read("DATA/MODATYP.DAT")).unwrap().strassenbaukosten,
        sabotage: Vec::new(),
    })
}

fn hash(world: &World) -> u64 {
    let mut h = DefaultHasher::new();
    world.hash(&mut h);
    h.finish()
}

/// The first scenario with at least three players, started with
/// `fixes`, the people animated, moving and coming.
fn scenario(fixes: Fixes) -> World {
    let tables = tables();
    for (n, file) in (1..=6).zip(SCENARIO_FILES) {
        let settings = Settings {
            scenario: n,
            difficulty: 1,
            ..Settings::default()
        };
        let map = SaveGame::parse(&read(&format!("SZENARIO/{file}"))).unwrap();
        let mut world = load_world(&map, &settings, true).unwrap();
        if world.player_count < 3 {
            continue;
        }
        world.tables = Arc::clone(&tables);
        world.random = Random::new(0x1234_5678);
        let events = scenario_events(world.scenario, |name| Some(read(name))).unwrap();
        start_scenario(&mut world, events.as_deref());
        world.set_loaded_fixes(fixes);
        world.clock.paused = false;
        world.options.animations = true;
        world.options.people_move = true;
        world.options.people_spawn = true;
        return world;
    }
    panic!("no scenario with three players");
}

/// A peer: its world, and the view of each of its clients.
struct Peer {
    world: World,
    views: Vec<(ViewPeople, Looking)>,
}

fn looking(origin: (i16, i16), rotation: u8) -> Looking {
    Looking::original(Viewpoint {
        rotation,
        origin,
        parity: 0,
    })
}

impl Peer {
    /// What every peer does on a tick: the clients' commands, then the
    /// world's tick, an auction left at once by the host, then each
    /// view's own people.
    fn step(&mut self, commands: &[ClientCommand]) {
        for c in commands {
            self.world.apply_client(c.clone());
        }
        let t = self.world.tick();
        if self.world.auction.open {
            self.world.apply_client(ClientCommand {
                client: HOST,
                action: Action::Auction(auction::Action::Leave),
            });
        }
        for (view, look) in &mut self.views {
            view.tick(&self.world, look);
            let _ = view.hear(&self.world, &t, look);
        }
    }
}

fn from(client: Client, action: Action) -> ClientCommand {
    ClientCommand { client, action }
}

/// Computer player 2's state: what its turns change.
fn seat_2(w: &World) -> String {
    let c = &w.computers;
    format!(
        "{:?}",
        (
            c.site[0],
            c.centre[0],
            c.road[0],
            c.build_state[0],
            c.state[0],
            c.waited[1],
            c.counters[0],
        )
    )
}

/// Ticks `peers` `n` times, with `commands` on the first, and checks
/// that they stay the same world.
fn run(peers: &mut [Peer], n: usize, commands: &[ClientCommand]) {
    for i in 0..n {
        let now = if i == 0 { commands } else { &[] };
        for p in peers.iter_mut() {
            p.step(now);
        }
        let h = hash(&peers[0].world);
        assert!(peers.iter().all(|p| hash(&p.world) == h), "tick {i}");
    }
}

/// The host's partner in seat 1, and the client in seat 2.
const PARTNER: Client = 5;
const SEAT_2: Client = 9;

/// Two peers: the host's, with the host and a co-op partner in seat 1,
/// and another client's, who takes computer seat 2. Their worlds stay the
/// same while each view looks somewhere else.
#[test]
fn clients_share_one_world() {
    if !orig().join("SZENARIO").exists() {
        eprintln!("skipping: {} not found", orig().display());
        return;
    }
    let world = scenario(Fixes::ORIGINAL);
    let speed = world.clock.speed;
    let mut peers = [
        Peer {
            world: world.clone(),
            views: vec![
                (ViewPeople::new(1), looking((60, 60), 0)),
                (
                    ViewPeople::new(2),
                    Looking {
                        everywhere: true,
                        ..looking((120, 40), 2)
                    },
                ),
            ],
        },
        Peer {
            world,
            views: vec![(ViewPeople::new(3), looking((30, 150), 1))],
        },
    ];
    // The host seats the others: the fixes several humans need go on.
    run(
        &mut peers,
        1,
        &[
            from(
                HOST,
                Action::Seat(SeatChange::Join {
                    client: PARTNER,
                    seat: 1,
                }),
            ),
            from(
                HOST,
                Action::Seat(SeatChange::Join {
                    client: SEAT_2,
                    seat: 2,
                }),
            ),
        ],
    );
    let w = &peers[0].world;
    assert_eq!(w.seats.humans(), 3);
    assert!(SINGLE_PLAYER_ONLY.iter().all(|&f| w.fixes.has(f)));
    assert!(!w.fixes.has(Fix::SiteEffect));
    assert!(!w.is_computer(2) && w.is_computer(3));
    // Only the host sets the speed: the partner in seat 1 isn't the host.
    run(
        &mut peers,
        1,
        &[
            from(PARTNER, Action::Speed(speed + 7)),
            from(SEAT_2, Action::Speed(speed + 8)),
            from(SEAT_2, Action::SetFix(Fix::ViewNews, false)),
        ],
    );
    assert_eq!(peers[0].world.clock.speed, speed);
    assert!(peers[0].world.fixes.has(Fix::ViewNews));
    run(&mut peers, 1, &[from(HOST, Action::Speed(speed + 1))]);
    assert_eq!(peers[1].world.clock.speed, speed + 1);
    // Co-op: the host and the partner both act for seat 1.
    let level = peers[0].world.advertising_level(1);
    let more = Action::MoneyMenu(geldmenu::Action::Advertising { more: true });
    run(&mut peers, 1, &[from(HOST, more.clone()), from(PARTNER, more.clone())]);
    assert_eq!(peers[0].world.advertising_level(1), (level + 2).min(7));
    // Seat 2's client acts for seat 2.
    let level2 = peers[0].world.advertising_level(2);
    run(&mut peers, 1, &[from(SEAT_2, more)]);
    assert_eq!(peers[1].world.advertising_level(2), (level2 + 1).min(7));
    // While a human plays seat 2 its computer stands still.
    let held = seat_2(&peers[0].world);
    run(&mut peers, 3000, &[]);
    assert_eq!(seat_2(&peers[0].world), held);
    // Disconnected, a computer plays it.
    run(
        &mut peers,
        3000,
        &[from(HOST, Action::Seat(SeatChange::Disconnect(SEAT_2)))],
    );
    assert!(peers[0].world.is_computer(2));
    let played = seat_2(&peers[0].world);
    assert_ne!(played, held);
    // Back, it's the human's again, from where the computer left it.
    run(
        &mut peers,
        3000,
        &[from(HOST, Action::Seat(SeatChange::Rejoin(SEAT_2)))],
    );
    assert_eq!(seat_2(&peers[0].world), played);
    // The views had people of their own.
    assert!(peers.iter().flat_map(|p| &p.views).any(|(v, _)| v.count() > 0));
}

/// A save of a game with several clients keeps them, and its news point
/// nowhere in the world but at the saving player's view in the original's
/// fields: loaded back, it's the same world.
#[test]
fn saved_with_clients() {
    if !orig().join("SZENARIO").exists() {
        eprintln!("skipping: {} not found", orig().display());
        return;
    }
    let mut world = scenario(Fixes::ORIGINAL);
    world.apply_client(from(HOST, Action::Seat(SeatChange::Join { client: 4, seat: 1 })));
    world.apply_client(from(HOST, Action::Seat(SeatChange::Join { client: 6, seat: 3 })));
    // A message without a square points nowhere.
    let mut tick = game::simulation::Tick::default();
    world.post_message(
        game::news::Message {
            number: 0,
            ..game::news::Message::default()
        },
        &mut tick,
    );
    let entry = world.news.entry(world.news.last).unwrap().clone();
    assert_eq!((entry.pos, entry.origin), (game::news::NOWHERE, game::news::NOWHERE));
    let view = MapView {
        origin: Some((44, 55)),
        ..MapView::default()
    };
    let map = SaveGame::parse(&world.to_map(&view).to_bytes()).unwrap();
    // The original reads the saving player's view there.
    let mut original = map.clone();
    original.trailing.clear();
    let read = World::from_map(&original).unwrap();
    let saved = read.news.entry(world.news.last).unwrap();
    assert_eq!((saved.pos, saved.origin), ((44, 55), (44, 55)));
    assert_eq!(read.seats, game::seats::Seats::default());
    // The port: the same world, the clients away until they rejoin.
    let mut back = world.load_map(&map).unwrap();
    assert_eq!(back.news, world.news);
    assert_eq!(back.seats, world.seats);
    back.keep_unsaved(&world);
    let alone = World {
        tables: Arc::clone(&world.tables),
        ..World::default()
    };
    let fresh = alone.load_map(&map).unwrap();
    assert_eq!(fresh.seats.humans(), 3);
    assert!(fresh.is_computer(3));
    assert_eq!(fresh.fixes, world.fixes);
    assert_eq!(fresh.news, world.news);
}

/// One human, through the clients' way in or as the original's commands,
/// plays the same game.
#[test]
fn one_human_is_the_original() {
    if !orig().join("SZENARIO").exists() {
        eprintln!("skipping: {} not found", orig().display());
        return;
    }
    let mut a = scenario(Fixes::ORIGINAL);
    let mut b = a.clone();
    let actions = [
        Action::Speed(20),
        Action::MoneyMenu(geldmenu::Action::Advertising { more: true }),
        Action::MoneyMenu(geldmenu::Action::Shown(true)),
        Action::SetOption(game::optionen::WorldOption::PeopleSpawn, false),
        Action::ViewMoved(Viewpoint {
            rotation: 1,
            origin: (70, 70),
            parity: 0,
        }),
    ];
    for i in 0..2000 {
        if let Some(action) = actions.get(i / 300).filter(|_| i % 300 == 0) {
            a.apply_client(from(HOST, action.clone()));
            b.apply(Command {
                player: 1,
                action: action.clone(),
            });
        }
        a.tick();
        b.tick();
        if a.auction.open {
            for w in [&mut a, &mut b] {
                w.apply(Command {
                    player: 1,
                    action: Action::Auction(auction::Action::Leave),
                });
            }
        }
    }
    assert_eq!(a, b);
    assert_eq!(a.fixes, Fixes::ORIGINAL);
}

// Small worlds of no game folder: the multiplayer behaviours of phase 2,
// each on a few peers that must stay the same world.

/// Peers of `world`, each fed the same clients' commands and ticks, whose
/// worlds are checked to stay the same after every step.
struct Peers(Vec<World>);

impl Peers {
    fn new(world: &World, n: usize) -> Self {
        Peers(vec![world.clone(); n])
    }

    /// The clients' `commands`, then `ticks` ticks; the ticks' results of
    /// the first peer.
    fn step(&mut self, commands: &[ClientCommand], ticks: usize) -> Vec<game::simulation::Tick> {
        let mut out = Vec::new();
        for (k, w) in self.0.iter_mut().enumerate() {
            let mut ticks_out = Vec::new();
            for c in commands {
                ticks_out.push(w.apply_client(c.clone()));
            }
            for _ in 0..ticks {
                ticks_out.push(w.tick());
            }
            if k == 0 {
                out = ticks_out;
            }
        }
        self.check();
        out
    }

    fn check(&self) {
        let h = hash(&self.0[0]);
        assert!(self.0.iter().all(|w| hash(w) == h), "the peers differ");
    }

    fn world(&self) -> &World {
        &self.0[0]
    }

    fn each(&mut self, f: impl Fn(&mut World)) {
        for w in &mut self.0 {
            f(w);
        }
        self.check();
    }
}

fn join(client: Client, seat: u8) -> ClientCommand {
    from(HOST, Action::Seat(SeatChange::Join { client, seat }))
}

/// A world of four players without computers' turns, at the default
/// speed, as a game folder isn't needed for: the host in seat 1, a
/// partner of theirs in seat 1 (co-op) and a client in seat 2.
fn small() -> Peers {
    let mut w = World {
        player_count: 4,
        computers_play: false,
        ..World::default()
    };
    w.options.animations = false;
    let mut peers = Peers::new(&w, 3);
    peers.step(&[join(PARTNER, 1), join(SEAT_2, 2)], 0);
    assert!(peers.world().seats.multiplayer());
    peers
}

/// Decision 5: with several humans neither the news' TV nor its button
/// pause the game; with one, the TV pauses it as the original's.
#[test]
fn the_tv_pauses_nothing_with_several_humans() {
    let mut alone = World {
        computers_play: false,
        ..World::default()
    };
    let mut tick = game::simulation::Tick::default();
    alone.post_message(game::news::Message::default(), &mut tick);
    alone.apply_client(from(HOST, Action::ShowTv(0)));
    assert!(alone.clock.paused);
    alone.apply_client(from(HOST, Action::TvPause(false)));
    assert!(!alone.clock.paused);

    let mut peers = small();
    peers.each(|w| {
        let mut tick = game::simulation::Tick::default();
        w.post_message(game::news::Message::default(), &mut tick);
    });
    let date = peers.world().date;
    peers.step(
        &[
            from(HOST, Action::ShowTv(0)),
            from(SEAT_2, Action::ShowTv(0)),
            from(PARTNER, Action::TvPause(true)),
        ],
        30,
    );
    assert!(!peers.world().clock.paused);
    assert_ne!(peers.world().date, date, "the clock went on");
}

/// Pause and speed are the host's world commands: the host's pause stops
/// the world for every peer, another client's does nothing, and with one
/// human the command does nothing (the view stops its own clock).
#[test]
fn the_hosts_pause_stops_the_world() {
    let mut alone = World::default();
    alone.apply_client(from(HOST, Action::Pause(true)));
    assert!(!alone.clock.stopped);

    let mut peers = small();
    peers.step(
        &[from(SEAT_2, Action::Pause(true)), from(PARTNER, Action::Pause(true))],
        1,
    );
    assert!(!peers.world().clock.stopped);
    peers.step(&[from(HOST, Action::Pause(true))], 0);
    let stopped = peers.world().clone();
    peers.step(&[], 50);
    assert_eq!(peers.world(), &stopped, "nothing runs while paused");
    peers.step(&[from(HOST, Action::Pause(false))], 30);
    assert_ne!(peers.world().clock, stopped.clock);
    // The last other client leaving takes the pause with it.
    peers.step(&[from(HOST, Action::Pause(true))], 0);
    peers.step(
        &[
            from(SEAT_2, Action::Seat(SeatChange::Leave(SEAT_2))),
            from(PARTNER, Action::Seat(SeatChange::Leave(PARTNER))),
        ],
        0,
    );
    assert!(!peers.world().clock.stopped);
}

/// An auction of item 7 at 10 000 opening on the next tick, with money
/// for everyone.
fn auction(peers: &mut Peers, waits: bool, timeout: u16) {
    peers.each(|w| {
        w.options.auction_waits = waits;
        w.options.auction_timeout = timeout;
        w.finances.money = [1_000_000; 8];
        w.auction.price = 10_000;
        w.auction.start_price = 10_000;
        w.auction.item = 7;
        w.auction.opening = Some(1);
    });
}

/// Decision 7: with "Auktion wartet" the auction waits for every human
/// seat, at most the host's timeout in ticks; then the seats that didn't
/// start it count as passed, and it begins.
#[test]
fn the_auction_waits_for_the_seats() {
    let mut peers = small();
    // Only the host sets the timeout.
    peers.step(&[from(SEAT_2, Action::AuctionTimeout(1))], 0);
    assert_eq!(peers.world().options.auction_timeout, game::simulation::AUCTION_TIMEOUT);
    auction(&mut peers, true, 300);
    peers.step(&[from(HOST, Action::AuctionTimeout(5))], 0);
    assert_eq!(peers.world().options.auction_timeout, 5);
    peers.step(&[], 1);
    let a = &peers.world().auction;
    assert!(a.open && !a.joined);
    assert_eq!(a.waiting, Some(5));
    // Seat 2 bids with its money, as the human, not as a computer.
    assert_eq!(a.limits[..2], [1_000_000, 1_000_000]);
    // Seat 1 starts it; seat 2 says nothing.
    peers.step(&[from(HOST, Action::Auction(auction::Action::Bid))], 3);
    let a = &peers.world().auction;
    assert!(!a.joined);
    assert_eq!(a.started, auction::seat_bit(1));
    peers.step(&[], 2);
    let a = &peers.world().auction;
    assert!(a.joined, "the timeout ran out");
    assert_eq!(a.passed, auction::seat_bit(2));
    // Seat 2 can't bid now; seat 1 can.
    peers.step(&[from(SEAT_2, Action::Auction(auction::Action::Bid))], 0);
    assert_eq!(peers.world().auction.bidder, 0);
    peers.step(&[from(HOST, Action::Auction(auction::Action::Bid))], 0);
    assert_eq!(peers.world().auction.bidder, 1);
}

/// When every human seat has started it, the auction begins at once;
/// without "Auktion wartet" it begins as it opens.
#[test]
fn the_auction_begins_when_all_have_started() {
    let mut peers = small();
    auction(&mut peers, true, 300);
    peers.step(&[], 1);
    peers.step(
        &[
            from(SEAT_2, Action::Auction(auction::Action::Bid)),
            from(PARTNER, Action::Auction(auction::Action::Bid)),
        ],
        0,
    );
    assert!(peers.world().auction.joined);

    let mut peers = small();
    auction(&mut peers, false, 300);
    peers.step(&[], 1);
    let a = &peers.world().auction;
    assert!(a.open && a.joined && a.waiting.is_none());
}

/// Co-op: either client of seat 1 starts, bids and passes for it; any
/// human holding a button keeps the computers from bidding; the item
/// goes to the computers only when every human seat has passed.
#[test]
fn co_op_bids_and_the_seats_passing() {
    let mut peers = small();
    auction(&mut peers, true, 300);
    peers.step(&[], 1);
    peers.step(
        &[
            from(PARTNER, Action::Auction(auction::Action::Bid)),
            from(SEAT_2, Action::Auction(auction::Action::Bid)),
        ],
        0,
    );
    assert_eq!(
        peers.world().auction.started,
        auction::seat_bit(1) | auction::seat_bit(2)
    );
    // The partner bids for seat 1, in seat 1's colour, at seat 1's limit.
    peers.step(&[from(PARTNER, Action::Auction(auction::Action::Bid))], 0);
    assert_eq!(peers.world().auction.bidder, 1);
    assert_eq!(peers.world().auction.price, 10_500);
    // Seat 2 holds a button: no computer bids meanwhile.
    peers.step(&[from(SEAT_2, Action::Auction(auction::Action::Hold(true)))], 0);
    assert!(peers.world().auction.held);
    // The host passes for seat 1; the auction goes on for seat 2.
    peers.step(&[from(HOST, Action::Auction(auction::Action::Pass))], 0);
    let a = &peers.world().auction;
    assert!(a.open && a.held);
    assert_eq!(a.passed, auction::seat_bit(1));
    // The partner's bid doesn't count any more: seat 1 passed.
    peers.step(&[from(PARTNER, Action::Auction(auction::Action::Bid))], 0);
    assert_eq!(peers.world().auction.price, 10_500);
    // Seat 2 passes too: no button is held, the form closes for everyone
    // and the computers bid among themselves.
    let t = peers.step(&[from(SEAT_2, Action::Auction(auction::Action::Leave))], 0);
    let a = &peers.world().auction;
    assert!(!a.open && !a.held);
    assert!(t[0].auction_changes.contains(&auction::Change::Closed));
}

/// Everyone passing before it begins hands it to the computers at once.
#[test]
fn the_auction_with_everyone_passed() {
    let mut peers = small();
    auction(&mut peers, true, 300);
    peers.step(&[], 1);
    peers.step(
        &[
            from(HOST, Action::Auction(auction::Action::Pass)),
            from(SEAT_2, Action::Auction(auction::Action::Close)),
        ],
        0,
    );
    let a = &peers.world().auction;
    assert!(!a.open && !a.joined);
}

/// A human in seat 2 plays as a human, not with the computers'
/// handicaps: their money is checked, they get nothing back on the
/// harder levels, they keep books, pay the human's costs, bid with their
/// money, and are checked for bankruptcy as player 1 (decision 9): out,
/// they spectate.
#[test]
fn a_human_in_seat_2() {
    let mut peers = small();
    peers.each(|w| {
        w.difficulty = 2;
        w.finances.money = [0, -1, -1, 0, 0, 0, 0, 0];
    });
    let w = peers.world();
    assert!(w.is_human(2) && !w.is_human(3));
    // The money checks: building in the red. The computers don't need
    // the money.
    assert_eq!(w.can_build((30, 30), 0, 2).0, game::construction::NOT_ENOUGH_MONEY);
    assert_ne!(w.can_build((30, 30), 0, 3).0, game::construction::NOT_ENOUGH_MONEY);
    peers.each(|w| w.finances.money = [0; 8]);
    // Paying: the computer gets a third back on hard, the human nothing,
    // and the human's books have it.
    peers.each(|w| {
        w.add_money(2, -300);
        w.add_money(3, -300);
        w.book_for(2, game::accounts::BOOK_OTHER, -300);
    });
    let w = peers.world();
    assert_eq!(w.finances.money[1], -300);
    assert_eq!(w.finances.money[2], -200);
    assert_eq!(w.book_value_of(2, game::accounts::BOOK_OTHER), -300);
    assert_eq!(w.book_value(game::accounts::BOOK_OTHER), 0);
    // Seat 2's insurance is its own.
    peers.step(&[from(SEAT_2, Action::MoneyMenu(geldmenu::Action::Insurance(true)))], 0);
    assert!(peers.world().insured_of(2) && !peers.world().insured);
    // Bankrupt on the 2nd of January: in the red past the debt limit and
    // last year's books negative.
    peers.each(|w| {
        w.finances.max_debt = 1000;
        w.finances.money[1] = -5000;
        w.date = game::monate::Date {
            day: 1,
            month: 1,
            year: 2003,
        };
        w.accounting.last_day = 1;
        w.accounting.step = 4;
        w.clock.hour_ticks = u16::try_from(w.clock.speed - 1).unwrap();
        w.clock.day_ticks = 6;
    });
    assert!(!peers.world().bankrupt(2), "last year's books aren't negative");
    peers.each(|w| w.book_for(2, 0x5e, -1));
    assert!(peers.world().bankrupt(2));
    let t = peers.step(&[], 1);
    let w = peers.world();
    assert_eq!(w.player_out(2), 1);
    assert_eq!(w.player_out(1), 0);
    assert!(t[0].game_over.is_none(), "seat 1 is still in");
    let lost = t[0].results_for(2).unwrap();
    assert!(!lost.won && lost.seat == 2);
    assert!(w.spectates(2) && !w.spectates(1));
    // A spectator sends no commands; it may still leave.
    let level = w.advertising_level(2);
    peers.step(
        &[from(
            SEAT_2,
            Action::MoneyMenu(geldmenu::Action::Advertising { more: true }),
        )],
        0,
    );
    assert_eq!(peers.world().advertising_level(2), level);
    peers.step(&[from(SEAT_2, Action::Seat(SeatChange::Leave(SEAT_2)))], 0);
    assert!(peers.world().seats.member(SEAT_2).is_none());
}

/// The months' check of the computers' bankruptcy (settlement step 0x64)
/// leaves a human seat to the 2nd of January.
#[test]
fn a_human_seat_isnt_a_computers_bankruptcy() {
    let mut peers = small();
    peers.each(|w| {
        w.finances.max_debt = 1000;
        w.finances.money = [0, -5000, -5000, 0, 0, 0, 0, 0];
        w.events = vec![game::events::Event::default(); game::events::EVENTS];
        // Past the month's first day, which starts the steps again.
        w.date.day = 2;
        w.accounting.step = 0x63;
        w.accounting.last_day = 2;
        w.clock.hour_ticks = u16::try_from(w.clock.speed - 1).unwrap();
    });
    peers.step(&[], 1);
    let w = peers.world();
    assert_eq!(w.player_out(3), 1, "a computer goes out");
    assert_eq!(w.player_out(2), 0, "a human seat doesn't");
}

/// Decision 9 and 10: in 2056 the human seats still in have won; the
/// first ends the game for everyone, each seat has its results, and a
/// cheat by any human takes everyone's score.
#[test]
fn the_first_winner_ends_the_game_for_all() {
    for cheat in [false, true] {
        let mut peers = small();
        peers.each(|w| {
            w.date = game::monate::Date {
                day: 1,
                month: 1,
                year: 0x808,
            };
            w.accounting.last_day = 1;
            w.accounting.step = 4;
            w.clock.hour_ticks = u16::try_from(w.clock.speed - 1).unwrap();
            w.clock.day_ticks = 6;
        });
        if cheat {
            peers.step(&[from(SEAT_2, Action::Cheat)], 0);
            let w = peers.world();
            assert!(w.cheated);
            assert_eq!(w.finances.money[1], 1_000_000_000);
            assert_eq!(w.finances.money[0], 0);
        }
        let t = peers.step(&[], 1);
        let over = t[0].game_over.expect("over");
        assert!(over.won && over.seat == 1);
        for seat in [1, 2] {
            let r = t[0].results_for(seat).unwrap();
            assert!(r.won && r.seat == seat);
            assert_eq!(r.score.is_some(), !cheat, "seat {seat}");
        }
        assert!(peers.world().won);
        // Nothing stops: the clock goes on (decision 8), and no second end.
        let t = peers.step(&[], 200);
        assert!(t.iter().all(|t| t.game_over.is_none()));
    }
}

/// Each human seat drags its own road: seat 2's drag leaves seat 1's,
/// the original's one flag, alone.
#[test]
fn each_seat_drags_its_own_road() {
    let mut peers = small();
    peers.step(&[from(SEAT_2, Action::RoadDragStart((10, 10)))], 0);
    let w = peers.world();
    assert!(w.road_drag(2).on && !w.road_drag(1).on && !w.road_dragging);
    peers.step(&[from(PARTNER, Action::RoadDragStart((20, 20)))], 0);
    peers.step(&[from(SEAT_2, Action::RoadDragCancel)], 0);
    let w = peers.world();
    assert!(!w.road_drag(2).on && w.road_drag(1).on && w.road_dragging);
}

/// Decision 12: a save keeps every client's view in the port's chunk and
/// the saving client's in the original's fields, and the seats' slots;
/// only the host loads a game in the middle of one.
#[test]
fn every_clients_view_saved_and_only_the_host_loads() {
    let mut peers = small();
    peers.step(
        &[
            from(SEAT_2, Action::MoneyMenu(geldmenu::Action::Insurance(true))),
            from(HOST, Action::AuctionTimeout(77)),
        ],
        0,
    );
    let w = peers.world().clone();
    let view = |origin: (i16, i16), scroll: (i16, i16)| MapView {
        scroll,
        origin: Some(origin),
        news: [true, false, true, false],
        ..MapView::default()
    };
    let (host, partner, two) = (view((1, 2), (10, 20)), view((3, 4), (30, 40)), view((5, 6), (50, 60)));
    // The client in seat 2 saves: their view in the original's fields.
    let map = w.to_map_with_views(&two, &[(HOST, &host), (PARTNER, &partner), (SEAT_2, &two)]);
    let map = Arc::new(SaveGame::parse(&map.to_bytes()).unwrap());
    assert_eq!(MapView::from_map(&map).scroll, (50, 60));
    for (client, v) in [(HOST, &host), (PARTNER, &partner), (SEAT_2, &two)] {
        let (back, _) = game::savegame::client_view(&map, client).unwrap();
        assert_eq!(back.scroll, v.scroll);
        assert_eq!(back.origin, v.origin);
        assert_eq!(back.news, v.news);
    }
    assert!(game::savegame::client_view(&map, 42).is_none());
    // Loaded as it was: the seats, their slots and the timeout.
    let back = w.load_map(&map).unwrap();
    assert_eq!(back.seats, w.seats);
    assert_eq!(back.seat_slots, w.seat_slots);
    assert!(back.insured_of(2));
    assert_eq!(back.options.auction_timeout, 77);
    // Only the host loads it.
    peers.step(&[from(HOST, Action::RoadDragStart((10, 10)))], 0);
    let before = hash(peers.world());
    peers.step(
        &[
            from(SEAT_2, Action::Load(Arc::clone(&map))),
            from(PARTNER, Action::Load(Arc::clone(&map))),
        ],
        0,
    );
    assert_eq!(hash(peers.world()), before);
    let t = peers.step(&[from(HOST, Action::Load(Arc::clone(&map)))], 0);
    assert!(t[0].loaded);
    assert!(!peers.world().road_dragging);
}
