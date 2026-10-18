//! What the tests share: a scenario's world from `orig/`.

#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::Arc;

use formats::data::{animdat1, animdat4, darstell, groundnr, hausdata, hausnr, hausprei, modatyp};
use formats::savegame::SaveGame;
use game::enhance::Fixes;
use game::hintergrund::{SCENARIO_FILES, load_world, scenario_events, start_scenario};
use game::startform::Settings;
use game::world::{Tables, World};
use vcl::random::Random;

pub fn orig() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../orig")
}

/// Reads `name` from `orig/`, whatever the case of its letters.
pub fn read(name: &str) -> Vec<u8> {
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

pub fn tables() -> Arc<Tables> {
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

/// The first scenario with at least four players, started with `fixes`,
/// the people animated, moving and coming.
pub fn scenario(fixes: Fixes) -> World {
    let tables = tables();
    for (n, file) in (1..=6).zip(SCENARIO_FILES) {
        let settings = Settings {
            scenario: n,
            difficulty: 1,
            ..Settings::default()
        };
        let map = SaveGame::parse(&read(&format!("SZENARIO/{file}"))).unwrap();
        let mut world = load_world(&map, &settings, true).unwrap();
        if world.player_count < 4 {
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
    panic!("no scenario with four players");
}
