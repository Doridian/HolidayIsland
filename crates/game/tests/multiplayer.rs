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
