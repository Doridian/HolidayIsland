//! A scenario's start, state 1500 of hintergrund's timer (1000:2466): its
//! events (`EREIGNn.DAT`), the players in the game, the books, the build
//! menu and the computers; then an oil slick on its island. Skipped when
//! `orig/` isn't there.

use std::path::PathBuf;
use std::sync::Arc;

use formats::data::{animdat1, animdat4, darstell, groundnr, hausdata, hausnr, hausprei, modatyp};
use formats::savegame::SaveGame;
use game::events::{Event, OIL};
use game::hintergrund::{SCENARIO_BANNED, SCENARIO_FILES, load_world, scenario_events, start_scenario};
use game::startform::Settings;
use game::world::{Tables, World};
use vcl::random::Random;

fn orig() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../orig")
}

/// Reads `name` from `orig/`, whatever the case of its letters.
fn try_read(name: &str) -> Option<Vec<u8>> {
    let mut path = orig();
    for part in name.split('/') {
        let found = std::fs::read_dir(&path)
            .ok()?
            .filter_map(Result::ok)
            .find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(part))?;
        path = found.path();
    }
    std::fs::read(path).ok()
}

fn read(name: &str) -> Vec<u8> {
    try_read(name).unwrap_or_else(|| panic!("{name} not found"))
}

/// The tables the simulation reads from `DATA\`.
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

/// Scenario `n`'s world after state 1500, on `difficulty`.
fn scenario(n: u8, difficulty: i16) -> (World, World) {
    let file = SCENARIO_FILES[usize::from(n - 1)];
    let settings = Settings {
        scenario: n,
        difficulty,
        name: b"Tester".to_vec(),
        ..Settings::default()
    };
    let map = SaveGame::parse(&read(&format!("SZENARIO/{file}"))).unwrap();
    let mut before = World::from_map(&map).unwrap();
    before.tables = tables();
    let mut world = load_world(&map, &settings, true).unwrap();
    world.tables = tables();
    world.random = Random::new(0x5eed);
    let rows = scenario_events(world.scenario, try_read).unwrap().unwrap();
    start_scenario(&mut world, Some(&rows));
    (before, world)
}

fn in_menu(world: &World, building: u8) -> bool {
    world
        .build_menu
        .rows
        .iter()
        .flatten()
        .any(|row| row.contains(&i16::from(building)))
}

#[test]
fn scenario_start() {
    if !orig().join("SZENARIO").exists() {
        eprintln!("skipping: {} not found", orig().display());
        return;
    }
    for n in 1..=6 {
        let (before, world) = scenario(n, 0);
        let file = SCENARIO_FILES[usize::from(n - 1)];
        // The events from `EREIGNn.DAT`, their squares from the map.
        let rows = game::hintergrund::scenario_events(n, try_read).unwrap().unwrap();
        assert_eq!(world.events.len(), 41, "{file}");
        for (k, (e, old)) in world.events.iter().zip(&before.events).enumerate() {
            assert_eq!(*e, old.with_row(&rows[k]), "{file}: event {k}");
        }
        assert_eq!(world.player_names[1], b"Tester", "{file}");
        // Easy: played without disasters.
        assert!(world.played_without_disasters && !world.disasters, "{file}");
        for p in 1..=8 {
            assert_eq!(world.player_out(p), u8::from(p > world.player_count), "{file}: {p}");
        }
        assert!(world.books.year[..26].iter().all(|&v| v == 0), "{file}");
        // Scenarios 1, 4 and 6 take their types off the build menu.
        for b in SCENARIO_BANNED {
            if [1, 4, 6].contains(&n) {
                assert!(!in_menu(&world, b), "{file}: {b:#x}");
            } else {
                assert_eq!(in_menu(&world, b), in_menu(&before, b), "{file}: {b:#x}");
            }
        }
        // Each computer works at its first building, and its roads are
        // where it goes on from.
        for p in 2..=world.player_count {
            let k = usize::from(p - 2);
            let c = &world.computers;
            assert_eq!(c.build_state[k], game::computer::CHOOSE_CATEGORY, "{file}: {p}");
            let owns = (1..=250).any(|i| (1..=250).any(|j| world.island.owners.at((i, j)) == p));
            if owns {
                assert_eq!(world.island.owners.at(c.site[k]), p, "{file}: {p}");
            }
            for e in 1..=c.roads[k].len() {
                let (direction, pos) = c.roads[k].get(e);
                assert!(direction < 4);
                assert_eq!(world.island.owners.at(pos), p, "{file}: {p}");
                assert!((10..=0x3a).contains(&world.island.ground.at(pos)), "{file}: {p}");
            }
        }
        // Normal and hard don't count as without disasters.
        assert!(!scenario(n, 1).1.played_without_disasters);
        assert!(!scenario(n, 2).1.played_without_disasters);
    }
    // Some computers find roads to go on from.
    let (_, koenig) = scenario(3, 1);
    assert!((0..7).any(|k| !koenig.computers.roads[k].is_empty()));
}

#[test]
fn oil_slick_on_a_scenario() {
    if !orig().join("SZENARIO").exists() {
        eprintln!("skipping: {} not found", orig().display());
        return;
    }
    for n in 1..=6 {
        let (_, mut world) = scenario(n, 1);
        let file = SCENARIO_FILES[usize::from(n - 1)];
        // The scenario's oil slick ("Oelpest"), now.
        let k = world
            .events
            .iter()
            .position(|e| e.kind == 2 && e.start == 5)
            .unwrap_or_else(|| panic!("{file}: no oil slick"));
        let days = world.days;
        world.events[k] = Event {
            chance: 0x10000,
            first_day: days + 1,
            last_day: days + 1,
            ..world.events[k]
        };
        let duration = world.events[k].duration;
        let location = world.island.location.clone();
        let mut messages = Vec::new();
        while !world.oil_slick {
            let day = world.days;
            while world.days == day {
                let tick = world.tick();
                if world.auction.open {
                    world.auction.open = false;
                }
                messages.extend(tick.messages.clone());
            }
        }
        let start = messages.iter().find(|m| m.number == 5).expect("the message");
        let slick = world.damage.slick;
        assert_eq!(start.pos, Some(slick), "{file}");
        let oil: Vec<_> = world
            .people
            .records
            .iter()
            .filter(|p| OIL.contains(&p.kind()))
            .map(game::people::Person::pos)
            .collect();
        assert!(!oil.is_empty(), "{file}");
        for &pos in &oil {
            assert_eq!(world.island.ground.at(pos), 2, "{file}: {pos:?}");
        }
        assert!(
            world.location_at(slick) < location.get((slick.0 / 2, slick.1 / 2)).unwrap(),
            "{file}"
        );
        // It ends after its days.
        for _ in 0..=duration {
            let day = world.days;
            while world.days == day {
                world.tick();
                world.auction.open = false;
            }
        }
        assert!(!world.oil_slick, "{file}");
        assert!(!world.people.records.iter().any(|p| OIL.contains(&p.kind())), "{file}");
    }
}
