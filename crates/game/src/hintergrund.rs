//! `THintergrund`'s timer states 1000–1500 (1000:2466), which start a game
//! once the start menu's Start or the load form has set them going
//! (`docs/startup.md`). What's ported is the choice of the map file and
//! the part of loading it that the island needs.

use formats::savegame::SaveGame;

use crate::startform::Settings;
use crate::world::{self, World};

/// The scenarios' map files in `SZENARIO\`, for `scenario` 1 to 6.
pub const SCENARIO_FILES: [&str; 6] = [
    "Geld.his",     // 1000:23c7
    "Urlauber.his", // 1000:23d0
    "Koenig.his",   // 1000:23dd
    "Luxus.his",    // 1000:23e8
    "Surfer.his",   // 1000:23f2
    "Oeko.his",     // 1000:23fd
];

/// The scenarios' events in `DATA\`, which replace `EREIGNIS.DAT`'s.
pub const SCENARIO_EVENTS: [&str; 6] = [
    "ereign1.dat", // 1000:2412
    "ereign2.dat", // 1000:241e
    "ereign3.dat", // 1000:242a
    "ereign4.dat", // 1000:2436
    "ereign5.dat", // 1000:2442
    "ereign6.dat", // 1000:244e
];

/// How many computer players each scenario has. // 1000:2466, state 1500
pub const SCENARIO_OPPONENTS: [u8; 6] = [1, 3, 7, 2, 3, 0];

/// The saved game state 1500 loads from `MAPS\` when `hintergrund+0x190`
/// is set (by loading a game): `Last.map` unless something put another
/// name there. // 1110:1322
pub const LAST_MAP: &str = "Last.map";

/// Where a new game's island comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Island {
    /// `SZENARIO\` and the file's name.
    Scenario(&'static str),
    /// A new island from the start menu's settings, made by 10b0:52d4 in
    /// state 1210. Not ported yet.
    Generated,
}

/// The island Start leads to: the scenario on the scenario page, or a
/// generated one. // 1000:2466, state 1500
#[must_use]
pub fn new_island(settings: &Settings, scenario_page: bool) -> Island {
    match settings.scenario {
        n @ 1..=6 if scenario_page => Island::Scenario(SCENARIO_FILES[usize::from(n - 1)]),
        _ => Island::Generated,
    }
}

/// The world after state 1500 has loaded `map`. For a scenario
/// (`scenario_page`), state 1000 saved the start menu's name, colours and
/// difficulty, and they replace the scenario's; then the difficulty sets
/// the start money and the scenario the number of computer players.
/// // 1000:2466
///
/// # Errors
/// If the map lacks a field the world needs.
pub fn load_world(map: &SaveGame, settings: &Settings, scenario_page: bool) -> Result<World, world::Error> {
    let mut world = World::from_map(map)?;
    if scenario_page {
        // FUN_1108_148c(8, 0x74, 0x634e): player_colours := the copy.
        world.player_colours = settings.colours;
        world.difficulty = settings.difficulty;
        start_money(&mut world);
        if let Some(&n) = usize::from(settings.scenario)
            .checked_sub(1)
            .and_then(|i| SCENARIO_OPPONENTS.get(i))
        {
            world.player_count = n + 1;
        }
        // TODO: the scenario's events (`EREIGNn.DAT`, 1010:36f8), the
        // players' flags at `*1110:6f24+0x80` and the rest of state 1500.
    }
    Ok(world)
}

/// A scenario's start money by difficulty: the debt limit doubles on easy
/// and halves on hard, and the human starts with half of it, as a loan at
/// 6 percent. Disasters are off on easy. // 1000:2466, state 1500
fn start_money(world: &mut World) {
    let f = &mut world.finances;
    match world.difficulty {
        0 => {
            f.max_debt = f.max_debt.wrapping_mul(2);
            world.disasters = false;
        }
        1 => world.disasters = true,
        2 => {
            f.max_debt /= 2;
            world.disasters = true;
        }
        _ => {}
    }
    if (0..=2).contains(&world.difficulty) {
        f.start_money = f.max_debt / 2;
    }
    // TODO: 1110:133c is set on easy and cleared otherwise; not known yet.
    f.interest_rate = 6;
    f.money[0] = f.start_money;
    world.set_loan(1, world.finances.start_money);
}
