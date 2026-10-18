//! Runs the simulation's clock on the tutorial island and each scenario.
//! Skipped when `orig/` isn't there.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::Arc;

use formats::bmp::Bitmap;
use formats::data::{
    animdat1, animdat4, darstell, fontdat, groundnr, hausdata, hausnr, hausprei, modatyp, monate, sonstige,
};
use formats::savegame::SaveGame;
use game::auction;
use game::command::{self, Command};
use game::darstellung::Camera;
use game::events::Event;
use game::hauptschirm::{BITMAP_FILES, Hauptschirm, InfoTexts};
use game::hintergrund::{SCENARIO_FILES, load_world, scenario_events, start_scenario};
use game::monate::Date;
use game::people::{BOAT, SHIP, view_square};
use game::simulation::Tick;
use game::startform::Settings;
use game::text::Font;
use game::world::{Facility, House, Tables, World};
use vcl::random::Random;
use vcl::surface::Surface;

/// A tick with a player who leaves every auction as soon as its form
/// opens: the form stops the clock until the player does something.
fn step(world: &mut World) -> Tick {
    let mut tick = world.tick();
    if world.auction.open {
        let leave = Command {
            player: 1,
            action: command::Action::Auction(auction::Action::Leave),
        };
        let left = world.apply(leave);
        tick.messages.extend(left.messages);
        tick.redraw.extend(left.redraw);
        tick.redraw_all |= left.redraw_all;
        tick.auction_changes.extend(left.auction_changes);
    }
    tick
}

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

/// How long each world runs: a bit over a month at the scenarios' speed.
const TICKS: usize = 5000;

fn hash(world: &World) -> u64 {
    let mut h = DefaultHasher::new();
    world.hash(&mut h);
    h.finish()
}

/// Ticks `world` `n` times and returns the hash after each 100.
fn run(mut world: World, n: usize) -> (World, Vec<u64>) {
    let mut hashes = Vec::new();
    for i in 1..=n {
        step(&mut world);
        if i % 100 == 0 {
            hashes.push(hash(&world));
        }
    }
    (world, hashes)
}

/// The number of days `n` ticks move a date on from the start of a day.
fn days_after(world: &World, n: usize) -> usize {
    let speed = usize::try_from(world.clock.speed).unwrap();
    let left = 6 * speed - usize::from(world.clock.day_ticks);
    if n < left { 0 } else { 1 + (n - left) / (6 * speed) }
}

#[test]
fn deterministic() {
    if !orig().join("MAPS").exists() {
        eprintln!("skipping: {} not found", orig().display());
        return;
    }
    let mut worlds = vec![(
        "TUTORIAL.MAP".to_owned(),
        World::from_map(&SaveGame::parse(&read("MAPS/TUTORIAL.MAP")).unwrap()).unwrap(),
    )];
    for (n, file) in (1..=6).zip(SCENARIO_FILES) {
        let settings = Settings {
            scenario: n,
            difficulty: 1,
            ..Settings::default()
        };
        let map = SaveGame::parse(&read(&format!("SZENARIO/{file}"))).unwrap();
        worlds.push((file.to_owned(), load_world(&map, &settings, true).unwrap()));
    }
    let tables = tables();
    for (name, mut world) in worlds {
        world.random = Random::new(0x1234_5678);
        world.tables = Arc::clone(&tables);
        // The rest of a scenario's start (state 1500).
        if world.scenario != 0 {
            let events = scenario_events(world.scenario, |name| Some(read(name))).unwrap();
            start_scenario(&mut world, events.as_deref());
        }
        let start = world.date;
        assert!(world.clock.speed > 0, "{name}: speed {}", world.clock.speed);
        assert!((1..=12).contains(&start.month) && start.day >= 1, "{name}: {start:?}");
        let (a, hashes_a) = run(world.clone(), TICKS);
        let (b, hashes_b) = run(world.clone(), TICKS);
        assert_eq!(hashes_a, hashes_b, "{name}");
        assert_eq!(a, b, "{name}");
        assert_ne!(hash(&a), hash(&world), "{name}: nothing changed");

        let mut expected = start;
        if world.clock.paused {
            assert_eq!(a.date, start, "{name}");
            continue;
        }
        for _ in 0..days_after(&world, TICKS) {
            expected.advance();
        }
        assert_eq!(a.date, expected, "{name}");
        assert_ne!(a.date, Date { ..start }, "{name}");
    }
}

#[test]
fn scenario_start_money() {
    if !orig().join("SZENARIO").exists() {
        eprintln!("skipping: {} not found", orig().display());
        return;
    }
    let map = SaveGame::parse(&read("SZENARIO/Geld.his")).unwrap();
    let base = World::from_map(&map).unwrap().finances.max_debt;
    for (difficulty, debt) in [(0, base * 2), (1, base), (2, base / 2)] {
        let settings = Settings {
            scenario: 1,
            difficulty,
            ..Settings::default()
        };
        let world = load_world(&map, &settings, true).unwrap();
        let f = &world.finances;
        assert_eq!(f.max_debt, debt);
        assert_eq!(f.money[0], debt / 2);
        assert_eq!(f.loan[0], debt / 2);
        assert_eq!(f.interest_rate, 6);
        assert_eq!(world.player_count, 2);
        assert_eq!(world.disasters, difficulty != 0);
    }
}

#[test]
fn info_bar() {
    if !orig().join("MAPS").exists() {
        eprintln!("skipping: {} not found", orig().display());
        return;
    }
    let bitmap = |name: &str| Bitmap::parse(&read(&format!("GRAFIK/{name}"))).unwrap();
    let font = Font::new(
        &bitmap("FONT.BMP"),
        &fontdat(&read("DATA/FONTDAT1.DAT")).unwrap(),
        &fontdat(&read("DATA/FONTDAT2.DAT")).unwrap(),
    );
    let texts = sonstige(&read("TEXTE/SONSTIGE.TXT")).unwrap();
    let info = InfoTexts {
        months: monate(&read("TEXTE/MONATE.TXT")).unwrap(),
        language: game::lang::Language::of_land(modatyp(&read("DATA/MODATYP.DAT")).unwrap().land),
        currency_factor: texts.waehrungsfaktor,
        currency: texts.texts[5].clone(),
    };
    assert_eq!(info.currency, b"DM");
    let bitmaps = BITMAP_FILES.iter().map(|f| Surface::from_bitmap(&bitmap(f))).collect();
    let camera = Camera {
        view: 0,
        origin: (100, 100),
        parity: 0,
        owner_pointers: false,
        occupancy_pointers: false,
    };
    let mut h = Hauptschirm::new(camera, (100, 100), bitmaps, &font, &texts.texts[0]);
    let mut world = World::from_map(&SaveGame::parse(&read("MAPS/TUTORIAL.MAP")).unwrap()).unwrap();
    let blank = h.info.clone();
    assert!(h.show_info(&world, &font, &info));
    assert_ne!(h.info, blank);
    assert!(!h.show_info(&world, &font, &info));
    let with_date = h.info.clone();
    world.date.advance();
    assert!(h.show_info(&world, &font, &info));
    assert_ne!(h.info, with_date);
    // The money's half of the bar didn't change.
    let left = |s: &Surface| {
        (0..s.height)
            .flat_map(|y| (0..152).map(move |x| (x, y)))
            .map(|(x, y)| s.pixel(x, y))
            .collect::<Vec<_>>()
    };
    assert_eq!(left(&h.info), left(&with_date));
    world.add_money(1, -2_000_000);
    assert!(h.show_info(&world, &font, &info));
    assert_ne!(left(&h.info), left(&with_date));
}

#[test]
fn events_happen() {
    if !orig().join("SZENARIO").exists() {
        eprintln!("skipping: {} not found", orig().display());
        return;
    }
    let map = SaveGame::parse(&read("SZENARIO/Geld.his")).unwrap();
    let settings = Settings {
        scenario: 1,
        difficulty: 1,
        ..Settings::default()
    };
    let mut world = load_world(&map, &settings, true).unwrap();
    world.tables = tables();
    world.random = Random::new(0x1234_5678);
    assert_eq!(world.events.len(), 41);
    // Three years.
    let end = world.days + 3 * 365;
    let (mut messages, mut auctions) = (Vec::new(), 0);
    while world.days < end {
        let tick = step(&mut world);
        messages.extend(tick.messages.iter().map(|m| m.number));
        auctions += usize::from(tick.auction);
    }
    assert_ne!(messages, []);
    assert!(auctions > 0);
}

#[test]
fn new_year() {
    if !orig().join("SZENARIO").exists() {
        eprintln!("skipping: {} not found", orig().display());
        return;
    }
    let tables = tables();
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
        assert_eq!(world.scenario, n, "{file}");
        assert!(world.building_sites.is_empty(), "{file}");
        assert!(!world.facilities.is_empty(), "{file}");
        assert!((0..=100).contains(&world.score()), "{file}");
        let (year, debt) = (world.date.year, world.finances.max_debt);
        let mut game_over = None;
        while world.date.year == year || world.date.day < 3 {
            game_over = game_over.or(step(&mut world).game_over);
        }
        // A year's end on normal difficulty, and nobody's won or lost yet.
        assert_eq!(world.finances.max_debt, debt + 50_000, "{file}");
        assert_eq!(game_over, None, "{file}");
        // The computer players renovate every year.
        for (pos, condition) in world
            .houses
            .iter()
            .map(|h| (h.pos(), h.condition()))
            .chain(world.facilities.iter().map(|f| (f.pos(), f.condition())))
        {
            if world.island.owners.at(pos) >= 2 {
                assert_eq!(condition, 0x7f, "{file}: {pos:?}");
            }
        }
    }
}

#[test]
fn disasters_demolish() {
    if !orig().join("SZENARIO").exists() {
        eprintln!("skipping: {} not found", orig().display());
        return;
    }
    let tables = tables();
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
        world.clock.paused = false;
        // A flood on the next day, and an earthquake going on.
        world.disasters = true;
        world.events = vec![Event {
            chance: 0x10000,
            first_day: 0,
            last_day: i32::MAX,
            duration: 1000,
            start: 0x2b,
            kind: 2,
            ..Event::default()
        }];
        world.earthquake = true;
        let before = world.houses.len() + world.facilities.len();
        for _ in 0..2000 {
            step(&mut world);
        }
        assert!(world.flood, "{file}");
        let after = world.houses.len() + world.facilities.len();
        assert!(after < before, "{file}: {before} buildings, then {after}");
        // The demolished buildings' records went with them.
        for pos in world
            .houses
            .iter()
            .map(House::pos)
            .chain(world.facilities.iter().map(Facility::pos))
        {
            assert_ne!(world.island.buildings.at(pos), 255, "{file}: {pos:?}");
        }
    }
}

#[test]
fn people_from_the_map() {
    if !orig().join("SZENARIO").exists() {
        eprintln!("skipping: {} not found", orig().display());
        return;
    }
    let tables = tables();
    let mut changed = 0;
    for file in ["MAPS/TUTORIAL.MAP".to_owned()]
        .into_iter()
        .chain(SCENARIO_FILES.iter().map(|f| format!("SZENARIO/{f}")))
    {
        let map = SaveGame::parse(&read(&file)).unwrap();
        let mut world = World::from_map(&map).unwrap();
        world.tables = Arc::clone(&tables);
        world.random = Random::new(0x1234_5678);
        world.clock.paused = false;
        // The ships' and boats' counts are theirs.
        let count =
            |w: &World, kind| i16::try_from(w.people.records.iter().filter(|p| p.kind() == kind).count()).unwrap();
        assert_eq!(count(&world, SHIP), world.people.ships, "{file}");
        assert_eq!(count(&world, BOAT), world.people.boats, "{file}");
        // The map's copy of the squares spawn_people looks at is the table
        // 1030:2bdc fills.
        let table = map.field("1110:5536").unwrap();
        for e in 0..=10 {
            for l in 0..=60 {
                let at = usize::try_from(e * 0xf4 + l * 4).unwrap();
                let saved = (
                    i16::from_le_bytes([table[at], table[at + 1]]),
                    i16::from_le_bytes([table[at + 2], table[at + 3]]),
                );
                assert_eq!(saved, view_square(e, l), "{file}");
            }
        }
        let before = world.people.records.clone();
        for _ in 0..TICKS {
            step(&mut world);
        }
        // People came, moved and left (people_tick, 1090:5293).
        changed += before
            .iter()
            .zip(&world.people.records)
            .filter(|(a, b)| a.kind() != b.kind() || a.pos() != b.pos())
            .count();
        assert!(world.people.ships <= 10 && world.people.boats <= 10, "{file}");
        assert_eq!(count(&world, SHIP), world.people.ships, "{file}");
        assert_eq!(count(&world, BOAT), world.people.boats, "{file}");
    }
    assert!(changed > 0);
}

#[test]
fn settlement_moves_money() {
    if !orig().join("SZENARIO").exists() {
        eprintln!("skipping: {} not found", orig().display());
        return;
    }
    let tables = tables();
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
        world.clock.paused = false;
        let (year, start) = (world.date.year, world.finances.money);
        let interest = world.finances.interest[0];
        let (mut months, mut books) = (0, 0);
        // Three years and a bit.
        while world.date.year < year + 3 || world.date.month < 2 {
            let tick = step(&mut world);
            books += usize::from(tick.books);
            if tick.new_day && world.date.day == 1 {
                months += 1;
            }
        }
        let money = world.finances.money;
        assert_ne!(money[0], start[0], "{file}");
        assert!(money.iter().all(|m| m.abs() < 50_000_000), "{file}: {money:?}");
        // The computer players' houses and shops earned or cost them
        // money (in Surfer they all lose, their plots' ports being too
        // small for their beds).
        let players = usize::from(world.player_count);
        assert!((1..players).all(|p| money[p] != start[p]), "{file}: {money:?}");
        // Four of the settlement's steps refresh the money menu each month.
        assert!(books >= 4 * (months - 1), "{file}: {books} in {months} months");
        // A whole year of interest in last year's books, and the cost of
        // living in its other costs.
        assert_eq!(world.book_value(0x5c), -12 * interest, "{file}");
        assert!(world.book_value(0x5d) <= -12 * world.settlement.living_costs, "{file}");
        // The month's guests stay within the beds.
        for h in &world.houses {
            assert!((0..=tables.hausdata(h.building(), 3)).contains(&h.guests()), "{file}");
            assert!((0..=100).contains(&h.stars()) && h.rent_rating() <= 100, "{file}");
        }
        for p in &world.plots {
            assert!(p.longint(0xee) <= p.longint(0x22).max(0), "{file}");
            assert!(p.quality() <= 100, "{file}");
        }
        let guests: i32 = world.houses.iter().map(|h| i32::from(h.guests())).sum();
        assert!(guests > 0, "{file}");
        if world.houses.iter().any(|h| world.island.owners.at(h.pos()) == 1) {
            // The human's guests are in the books' history, and their rent
            // in last year's books.
            assert!(world.guests_last_months(1) > 0, "{file}");
            assert!(world.book_value(0x52) + world.book_value(0x53) > 0, "{file}");
        }
    }
}

#[test]
fn computers_build() {
    if !orig().join("SZENARIO").exists() {
        eprintln!("skipping: {} not found", orig().display());
        return;
    }
    let tables = tables();
    // "König": seven computer players.
    let settings = Settings {
        scenario: 3,
        difficulty: 1,
        ..Settings::default()
    };
    let map = SaveGame::parse(&read("SZENARIO/KOENIG.HIS")).unwrap();
    let mut world = load_world(&map, &settings, true).unwrap();
    world.tables = Arc::clone(&tables);
    world.random = Random::new(0x1234_5678);
    world.clock.paused = false;
    assert_eq!(world.player_count, 8);
    assert!(world.computers_play);
    let computer = |w: &World, pos| (2..=w.player_count).contains(&w.island.owners.get(pos).unwrap_or(0));
    let facilities = |w: &World| w.facilities.iter().filter(|f| computer(w, f.pos())).count();
    let start_facilities = facilities(&world);
    let start_money = world.finances.money;
    let roads = |w: &World| {
        let squares = (0..=250).flat_map(|i| (0..=250).map(move |j| (i, j)));
        squares
            .filter(|&pos| (10..=0x3a).contains(&w.island.ground.at(pos)) && computer(w, pos))
            .count()
    };
    let start_roads = roads(&world);
    let start_heights = world.island.heights.clone();
    let year = world.date.year;
    let mut started = Vec::new();
    while world.date.year < year + 3 {
        let sites = world.building_sites.len();
        step(&mut world);
        if world.building_sites.len() > sites {
            let s = world.building_sites[0];
            started.push((world.island.owners.at(s.pos), s.building, s.budget));
        }
    }
    // Several of them started building sites, of types from their
    // COMPBAU.DAT lists, each on the budget of its category.
    assert!(started.len() >= 5, "{started:?}");
    let builders: std::collections::BTreeSet<u8> = started.iter().map(|s| s.0).collect();
    assert!(builders.len() >= 3, "{started:?}");
    for &(owner, building, budget) in &started {
        assert!((2..=8).contains(&owner), "{started:?}");
        let k = usize::try_from(budget).unwrap();
        let row = &world.computers.types[k * 41..k * 41 + 41];
        assert!(row[1..=usize::from(row[0])].contains(&building), "{started:?}");
    }
    // The sites were finished.
    assert!(facilities(&world) > start_facilities);
    let money = world.finances.money;
    assert!(
        builders
            .iter()
            .all(|&p| money[usize::from(p - 1)] != start_money[usize::from(p - 1)])
    );
    assert_ne!(world.computers.status.len(), 0);
    // They laid roads, and levelled land for their sites and roads.
    let new_roads = roads(&world) - start_roads;
    let moved = (0..=250)
        .flat_map(|i| (0..=250).map(move |j| (i, j)))
        .filter(|&pos| world.island.heights.at(pos) != start_heights.at(pos))
        .count();
    eprintln!(
        "{new_roads} road squares, {moved} corners moved, {} sites",
        started.len()
    );
    assert!(new_roads >= 20, "{new_roads}");
    assert!(moved >= 5, "{moved}");
}
