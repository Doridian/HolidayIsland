//! `fix.view_people`: the views' own people never change the world, and
//! with the fix off the people are the original's. Runs on the tutorial
//! island and the scenarios; skipped when `orig/` isn't there.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use formats::data::{animdat1, animdat4, darstell, groundnr, hausdata, hausnr, hausprei, modatyp};
use formats::savegame::SaveGame;
use game::auction;
use game::command::{self, Command};
use game::enhance::{Fix, Fixes};
use game::hintergrund::{SCENARIO_FILES, load_world, scenario_events, start_scenario};
use game::people::view::{Looking, ViewPeople};
use game::people::{Viewpoint, is_view_kind};
use game::savegame::MapView;
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
    })
}

fn hash(world: &World) -> u64 {
    let mut h = DefaultHasher::new();
    world.hash(&mut h);
    h.finish()
}

/// The scenarios' worlds, started, with `fixes` and the people animated,
/// moving and coming.
fn worlds(fixes: Fixes) -> Vec<(String, World)> {
    let tables = tables();
    let mut out = Vec::new();
    for (n, file) in (1..=6).zip(SCENARIO_FILES) {
        let settings = Settings {
            scenario: n,
            difficulty: 1,
            ..Settings::default()
        };
        let map = SaveGame::parse(&read(&format!("SZENARIO/{file}"))).unwrap();
        let mut world = load_world(&map, &settings, true).unwrap();
        world.tables = Arc::clone(&tables);
        world.random = Random::new(0x1234_5678);
        let events = scenario_events(world.scenario, |name| Some(read(name))).unwrap();
        start_scenario(&mut world, events.as_deref());
        world.set_loaded_fixes(fixes);
        world.clock.paused = false;
        world.options.animations = true;
        world.options.people_move = true;
        world.options.people_spawn = true;
        out.push((file.to_owned(), world));
    }
    let last = orig().join("../orig-en/MAPS/LAST.MAP");
    let maps = [("MAPS/TUTORIAL.MAP".to_owned(), read("MAPS/TUTORIAL.MAP"))]
        .into_iter()
        .chain(std::fs::read(&last).ok().map(|d| ("LAST.MAP (en)".to_owned(), d)));
    for (file, data) in maps {
        let mut world = World::from_map(&SaveGame::parse(&data).unwrap()).unwrap();
        world.tables = Arc::clone(&tables);
        world.random = Random::new(0x1234_5678);
        world.set_loaded_fixes(fixes);
        world.clock.paused = false;
        world.options.animations = true;
        world.options.people_move = true;
        world.options.people_spawn = true;
        out.push((file, world));
    }
    out
}

/// A square where the views' kinds come (a building site, a tennis
/// court, the stage, an airport, a jetty), making one a building site if
/// there's none: the original's view looks there.
fn look_at_their_buildings(world: &mut World) -> (i16, i16) {
    let theirs = |b: u8| matches!(b, 0x1f | 0x70 | 0x7b | 0x7c | 0x28..=0x33 | 0x68..=0x6b);
    let found = (1..250)
        .flat_map(|i| (1..250).map(move |j| (i, j)))
        .find(|&p| theirs(world.island.buildings.at(p)) && world.island.parts.at(p) == 0);
    let pos = found.unwrap_or_else(|| {
        let p = (1..250)
            .flat_map(|i| (1..250).map(move |j| (i, j)))
            .find(|&p| world.island.ground.at(p) == 1 && world.island.buildings.at(p) == 255)
            .unwrap();
        world.island.buildings.set(p, 0x1f);
        world.island.parts.set(p, 0);
        p
    });
    world.set_viewpoint(Viewpoint {
        rotation: 0,
        origin: pos,
        parity: 0,
    });
    pos
}

/// A tick, leaving every auction at once.
fn step(world: &mut World) -> game::simulation::Tick {
    let tick = world.tick();
    if world.auction.open {
        world.apply(Command {
            player: 1,
            action: command::Action::Auction(auction::Action::Leave),
        });
    }
    tick
}

fn looking(origin: (i16, i16), rotation: u8, everywhere: bool) -> Looking {
    Looking {
        everywhere,
        ..Looking::original(Viewpoint {
            rotation,
            origin,
            parity: 0,
        })
    }
}

/// With the fix on, two views looking at different places (one at the
/// whole island), each with its own people, ticking, moving and hearing
/// the world's sounds, leave the world as a world without views: its
/// hash and its random numbers. And a view's scroll, which the original
/// makes a command, does nothing to it.
#[test]
fn views_leave_the_world_alone() {
    if !orig().join("SZENARIO").exists() {
        eprintln!("skipping: {} not found", orig().display());
        return;
    }
    let mut own = 0;
    for (name, mut world) in worlds(Fixes::all_on()) {
        let busy = look_at_their_buildings(&mut world);
        let mut alone = world.clone();
        let mut watched = world.clone();
        let mut a = ViewPeople::new(1);
        let mut b = ViewPeople::new(2);
        let look_a = looking(busy, 0, false);
        let look_b = looking((100, 30), 2, true);
        for i in 0..600 {
            step(&mut alone);
            let random = watched.random;
            let before = hash(&watched);
            let t = step(&mut watched);
            assert_eq!(hash(&watched), hash(&alone), "{name}: tick {i}");
            let after = hash(&watched);
            // The views: nothing they do reaches the world.
            a.tick(&watched, &look_a);
            b.tick(&watched, &look_b);
            let _ = a.hear(&watched, &t, &look_a);
            let _ = b.hear(&watched, &t, &look_b);
            if i % 50 == 0 {
                a.refresh(&watched, &looking((i16::try_from(i / 5).unwrap(), 40), 1, false));
                // The scroll the original sends the world.
                watched.apply(Command {
                    player: 1,
                    action: command::Action::ViewMoved(Viewpoint {
                        rotation: 3,
                        origin: (60, 60),
                        parity: 1,
                    }),
                });
            }
            assert_eq!(hash(&watched), after, "{name}: tick {i}");
            assert_ne!((before, random), (0, Random::new(0)));
        }
        assert_eq!(watched, alone, "{name}");
        // The world has none of the views' kinds.
        assert!(!watched.people.records.iter().any(|p| is_view_kind(p.kind())), "{name}");
        own += a.count() + b.count();
    }
    // The views have people of their own somewhere.
    assert!(own > 0);
}

/// With the fix on, the world's people don't follow the viewpoint the
/// world was last handed: two worlds that only differ there tick the
/// same people and random numbers.
#[test]
fn the_world_follows_no_view() {
    if !orig().join("SZENARIO").exists() {
        eprintln!("skipping: {} not found", orig().display());
        return;
    }
    for (name, world) in worlds(Fixes::all_on()) {
        let mut a = world.clone();
        let mut b = world;
        b.set_viewpoint(Viewpoint {
            rotation: 2,
            origin: (120, 10),
            parity: 1,
        });
        for _ in 0..600 {
            step(&mut a);
            step(&mut b);
        }
        assert_eq!(a.people, b.people, "{name}");
        assert_eq!(a.random, b.random, "{name}");
        assert_eq!(a.island, b.island, "{name}");
        // Nor do the news and the events (`fix.view_news`): the
        // viewpoint the world was handed is all that differs.
        let differ: Vec<_> = game::savegame::differences(&a, &b)
            .into_iter()
            .filter(|&f| f != "viewpoint")
            .collect();
        assert!(differ.is_empty(), "{name}: {differ:?}");
    }
}

/// With the fix off the people are the original's: the view's kinds come
/// in the world, from its random numbers, where its viewpoint looks.
#[test]
fn fix_off_is_the_original() {
    if !orig().join("SZENARIO").exists() {
        eprintln!("skipping: {} not found", orig().display());
        return;
    }
    let mut seen = 0;
    for (name, mut world) in worlds(Fixes::ORIGINAL) {
        assert!(!world.people_split());
        look_at_their_buildings(&mut world);
        let mut a = world.clone();
        let mut b = world;
        b.set_viewpoint(Viewpoint {
            rotation: 2,
            origin: (120, 10),
            parity: 1,
        });
        for _ in 0..600 {
            step(&mut a);
            step(&mut b);
            // Every step's: they come and go, and at the end there may
            // be none on the CD's islands alone (without `orig-en/`'s save).
            seen += a.people.records.iter().filter(|p| is_view_kind(p.kind())).count();
        }
        // A view that changes is a world that changes.
        assert_ne!(hash(&a), hash(&b), "{name}");
        // And a view's people are none.
        let mut v = ViewPeople::new(1);
        v.tick(&a, &looking((50, 50), 0, true));
        assert_eq!(v.count(), 0, "{name}");
    }
    assert!(seen > 0);
}

/// Switching the fix on in the middle of a game hands the views' kinds
/// over; a save then has them back among the world's, as the original
/// keeps them, the original loads it with them, and the port with the
/// fix hands them over again.
#[test]
fn saved_and_loaded() {
    if !orig().join("SZENARIO").exists() {
        eprintln!("skipping: {} not found", orig().display());
        return;
    }
    let (name, mut world) = worlds(Fixes::ORIGINAL).remove(0);
    look_at_their_buildings(&mut world);
    for _ in 0..1500 {
        step(&mut world);
    }
    let theirs = world.people.records.iter().filter(|p| is_view_kind(p.kind())).count();
    assert!(theirs > 0, "{name}");
    let t = world.apply(Command {
        player: 1,
        action: command::Action::SetFix(Fix::ViewPeople, true),
    });
    assert_eq!(t.view_people.len(), theirs, "{name}");
    assert!(!world.people.records.iter().any(|p| is_view_kind(p.kind())));
    let mut view = ViewPeople::new(3);
    view.adopt(&t.view_people);
    assert_eq!(view.count(), theirs);
    // The save: the view's own back in the world's records.
    let viewpoint = world.viewpoint;
    let map = world.to_map(&MapView {
        people: Some((viewpoint, view.saved_with(&world, viewpoint))),
        ..MapView::default()
    });
    let map = SaveGame::parse(&map.to_bytes()).unwrap();
    let mut original = World::from_map(&map).unwrap();
    let count = |w: &World| w.people.records.iter().filter(|p| is_view_kind(p.kind())).count();
    assert_eq!(count(&original), theirs, "{name}");
    // The world's own are all there too.
    let others = |w: &World| {
        w.people
            .records
            .iter()
            .filter(|p| p.kind() != 0 && !is_view_kind(p.kind()))
            .count()
    };
    assert_eq!(others(&original), others(&world), "{name}");
    // The port with the fix: handed over again.
    original.set_loaded_fixes(Fixes::all_on());
    let handed = original.split_off_view_people();
    assert_eq!(handed.len(), theirs);
    assert_eq!(count(&original), 0);
}

/// How long a view's own people take a tick, by the view and the whole
/// island (`people_everywhere`), on the busiest scenario after a while.
#[test]
fn whole_island_cost() {
    if !orig().join("SZENARIO").exists() {
        eprintln!("skipping: {} not found", orig().display());
        return;
    }
    for (name, mut world) in worlds(Fixes::all_on()) {
        let mut view = ViewPeople::new(1);
        let mut whole = ViewPeople::new(1);
        let look = looking(world.viewpoint.origin, 0, false);
        let everywhere = looking(world.viewpoint.origin, 0, true);
        let (mut t_view, mut t_whole, mut t_world) = (0u128, 0u128, 0u128);
        let ticks = 1000;
        for _ in 0..ticks {
            let start = Instant::now();
            step(&mut world);
            t_world += start.elapsed().as_nanos();
            let start = Instant::now();
            view.tick(&world, &look);
            t_view += start.elapsed().as_nanos();
            let start = Instant::now();
            whole.tick(&world, &everywhere);
            t_whole += start.elapsed().as_nanos();
        }
        let start = Instant::now();
        let turned = world.people_for_view(Viewpoint {
            rotation: 1,
            ..world.viewpoint
        });
        let t_turn = start.elapsed().as_nanos();
        assert_eq!(turned.records.len(), world.people.records.len());
        if name == "Geld.his" {
            // The worst: every square of land a full tennis court, a
            // building site, an airport or a jetty of its own.
            let mut busy = world.clone();
            for i in 1..250 {
                for j in 1..250 {
                    if busy.island.ground.at((i, j)) != 2 {
                        let b = [0x70, 0x1f, 0x28, 0x68][usize::try_from((i + j) % 4).unwrap()];
                        busy.island.buildings.set((i, j), b);
                        busy.island.parts.set((i, j), 0);
                    }
                }
            }
            let mut v = ViewPeople::new(1);
            let start = Instant::now();
            for _ in 0..100 {
                v.tick(&busy, &everywhere);
            }
            eprintln!(
                "{name}, all built over: the whole island's people {} µs a tick ({} of them)",
                start.elapsed().as_nanos() / 100 / 1000,
                v.count()
            );
        }
        eprintln!(
            "{name} ({} buildings): a world tick {} µs, the view's people {} µs ({} of them), the whole island's {} µs ({} of them); turning the world's for a view {} µs",
            world.houses.len() + world.facilities.len(),
            t_world / ticks / 1000,
            t_view / ticks / 1000,
            view.count(),
            t_whole / ticks / 1000,
            whole.count(),
            t_turn / 1000,
        );
    }
}
