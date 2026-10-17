//! `THintergrund`'s timer states 1000–1500 (1000:2466), which start a game
//! once the start menu's Start or the load form has set them going
//! (`docs/startup.md`). What's ported is the choice of the island, the
//! part of loading a map file that the world needs, a new game's world
//! (the island made from the start menu's settings and everything the
//! game's data sets up around it) and a saved game's
//! ([`load_saved_game`], `docs/savegame.md`).

use std::sync::Arc;

use formats::data::{self, Abrechnung, Modatyp};
use formats::savegame::SaveGame;
use vcl::random::Random;

use crate::events::Event;
use crate::generator::Maperste;
use crate::people::People;
use crate::settlement::Settlement;
use crate::startform::Settings;
use crate::world::{self, Books, Tables, World};

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
    /// state 1210: [`new_world`].
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

/// What a new game is set up from: the CD's `DATA\` and `TEXTE\` files
/// the program loads at the start (state 100, 1030:38a3) and when a game
/// starts (state 1220, 1030:3932), parsed.
#[derive(Debug, Clone)]
pub struct GameData {
    /// The tables the simulation reads (`HAUSNR.DAT`, ...).
    pub tables: Arc<Tables>,
    /// `MAPERSTE.DAT`. // 10b0:56ce
    pub maperste: Maperste,
    /// `MODATYP.DAT`. // 1010:000e
    pub modatyp: Modatyp,
    /// `ABRECHNE.DAT`. // 1008:55ee
    pub abrechnung: Abrechnung,
    /// `EREIGNIS.DAT`'s rows. // 1010:3ab4
    pub events: Vec<[i32; 12]>,
    /// `ANIMDAT2.DAT`, `NEINTRAG.DAT`, `ANIMDAT3.DAT`. // 1088:80a7
    pub paths: Vec<Vec<[u8; 4]>>,
    pub entries: Vec<[u8; 4]>,
    pub tracks: Vec<Vec<[i16; 3]>>,
    /// `COMPBAU.DAT`. // 10a8:37da
    pub compbau: Vec<[u8; 7]>,
    /// `NAMEN.TXT`: the name of nobody and the computers' names to pick
    /// from. // 1010:0414
    pub nobody: Vec<u8>,
    pub names: data::Strings,
    /// `BAUMENU.DAT`: the build menu's lists and layouts. // 1018:080f
    pub baumenu: data::BaumenuData,
}

/// A file [`GameData::load`] couldn't read or parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadError(pub String);

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for LoadError {}

impl GameData {
    /// Parses the files `read` gives by their path on the CD
    /// (`DATA/MAPERSTE.DAT`, ...).
    ///
    /// # Errors
    /// The first file that's missing or doesn't parse.
    pub fn load(read: impl Fn(&str) -> Option<Vec<u8>>) -> Result<GameData, LoadError> {
        fn parsed<T, E: std::fmt::Display>(name: &str, r: Result<T, E>) -> Result<T, LoadError> {
            r.map_err(|e| LoadError(format!("{name}: {e}")))
        }
        let file = |name: &str| read(name).ok_or_else(|| LoadError(format!("{name} not found")));
        let dat = |name: &str| file(&format!("DATA/{name}"));
        let modatyp = parsed("MODATYP.DAT", data::modatyp(&dat("MODATYP.DAT")?))?;
        let tables = Tables {
            hausnr: parsed("HAUSNR.DAT", data::hausnr(&dat("HAUSNR.DAT")?))?,
            hausdata: parsed("HAUSDATA.DAT", data::hausdata(&dat("HAUSDATA.DAT")?))?,
            hausprei: parsed("HAUSPREI.DAT", data::hausprei(&dat("HAUSPREI.DAT")?))?,
            groundnr: parsed("GROUNDNR.DAT", data::groundnr(&dat("GROUNDNR.DAT")?))?,
            animdat1: parsed("ANIMDAT1.DAT", data::animdat1(&dat("ANIMDAT1.DAT")?))?,
            animdat4: parsed("ANIMDAT4.DAT", data::animdat4(&dat("ANIMDAT4.DAT")?))?,
            form_korrektur: parsed("DARSTELL.DAT", data::darstell(&dat("DARSTELL.DAT")?))?.form_korrektur,
            road_cost: modatyp.strassenbaukosten,
        };
        let maperste = parsed("MAPERSTE.DAT", data::maperste(&dat("MAPERSTE.DAT")?))?;
        let (nobody, _player, names) = parsed("NAMEN.TXT", data::namen(&file("TEXTE/NAMEN.TXT")?))?;
        Ok(GameData {
            tables: Arc::new(tables),
            maperste: Maperste::new(&maperste).ok_or_else(|| LoadError("MAPERSTE.DAT: not 11 tables".into()))?,
            modatyp,
            abrechnung: parsed("ABRECHNE.DAT", data::abrechne(&dat("ABRECHNE.DAT")?))?,
            events: parsed("EREIGNIS.DAT", data::ereignis(&dat("EREIGNIS.DAT")?))?,
            paths: parsed("ANIMDAT2.DAT", data::animdat2(&dat("ANIMDAT2.DAT")?))?,
            entries: parsed("NEINTRAG.DAT", data::neintrag(&dat("NEINTRAG.DAT")?))?,
            tracks: parsed(
                "ANIMDAT3.DAT",
                data::animdat3(&dat("ANIMDAT3.DAT")?, &crate::people::track_last_rows()),
            )?,
            compbau: parsed("COMPBAU.DAT", data::compbau(&dat("COMPBAU.DAT")?))?,
            nobody,
            names,
            baumenu: parsed("BAUMENU.DAT", data::baumenu_dat(&dat("BAUMENU.DAT")?))?,
        })
    }
}

/// The steps the computer players' names are picked with: `Random(10)`
/// picks one. // 1110:147a
const NAME_STEPS: [i16; 10] = [53, 59, 61, 67, 71, 73, 79, 83, 87, 89];

/// A new game's world, as states 100 to 1500 of the timer leave it for a
/// normal game: `RandSeed` is `seed` as `Randomize` (1108:1e21) left it
/// at the start, the computers' names are picked, the start menu's
/// `settings` go in, the island is generated (state 1210, 10b0:52d4), and
/// the game's data sets up the rest (state 1220, 1030:36e5, in its
/// order). State 1500 only sets up the main screen, whose first view
/// is [`crate::hauptschirm::start_viewpoint`]. // 1000:2466
///
/// Not set up, as the world doesn't have them yet: `1110:651a` to
/// `1110:6530` (1010:2fa9), `1110:6ef2` to `1110:6efe` (1018:37de),
/// `1110:65b4` to `1110:65b9` (1028:2449), `1110:54fa` and `1110:65ba`
/// (1028:361f), and what only the forms keep.
#[must_use]
pub fn new_world(settings: &Settings, data: &GameData, seed: u32) -> World {
    set_up(settings, data, seed, true)
}

/// States 100 to 1500 for a new game, with the island generated in state
/// 1210 (`generate`) or, for a map file, only cleared. // 1000:2466
fn set_up(settings: &Settings, data: &GameData, seed: u32, generate: bool) -> World {
    let mut w = World {
        random: Random::new(seed),
        tables: Arc::clone(&data.tables),
        ..World::default()
    };
    // State 100: 1010:064a.
    apply_modatyp(&mut w, &data.modatyp); // 1010:000e
    pick_names(&mut w, &data.nobody, &data.names); // 1010:0414
    // The start menu's globals; Start puts the name in for player 1
    // (1058:021a).
    w.player_colours = settings.colours;
    w.player_count = settings.players;
    w.difficulty = settings.difficulty;
    w.island_settings = [settings.island, settings.relief, settings.land, settings.rivers];
    w.scenario = 0;
    if let Some(name) = w.player_names.get_mut(1) {
        name.clone_from(&settings.name);
    }
    // State 1210: for a map file, `clear_map` (10b0:3030) leaves the
    // empty island `World::default` has.
    let plots = if generate {
        w.generate_island(settings, &data.maperste) // 10b0:52d4
    } else {
        0
    };
    // State 1220, 1030:36e5.
    w.settlement = Settlement::new_game(&data.abrechnung, &data.modatyp); // 1008:57dc
    w.accounting = crate::simulation::Accounting::default();
    w.people = People::new_game(&data.paths, &data.entries, &data.tracks); // 1088:868c
    w.computers.compbau.clone_from(&data.compbau); // 10a8:388b
    w.build_type_lists();
    w.init_computers(); // 10a8:3558
    w.init_players(plots); // 1078:3271
    let money = w.money_of(1);
    let loan = w.loan_of(1);
    w.books = Books::new_game(money, loan); // 1070:2e6c
    w.build_menu = crate::build_menu::BuildMenu::new(&data.baumenu); // 1018:2fa2
    w.events = data.events.iter().map(Event::from_row).collect(); // 1010:3ab4
    w.sabotage = Default::default(); // 10a0:0f6c
    w.championships = Default::default();
    // State 1500: hauptschirm_init (1020:2501).
    w.viewpoint = crate::hauptschirm::start_viewpoint();
    w
}

/// The world after states 1000 to 1500 have loaded the saved game `map`
/// (`hintergrund+0x190`, set by the load form, 1038:3a95): a new game is
/// set up as for Start, with `settings` the start menu's, but without
/// generating an island, and `load_map` reads the file over it. What the
/// file doesn't hold stays as the set-up left it
/// ([`World::keep_unsaved`]), `RandSeed` included, which the set-up has
/// drawn from. Unlike a scenario, the file's name, colours and difficulty
/// stay. // 1000:2466
///
/// # Errors
/// If the map lacks a field the world needs.
pub fn load_saved_game(map: &SaveGame, settings: &Settings, data: &GameData, seed: u32) -> Result<World, world::Error> {
    let fresh = set_up(settings, data, seed, false);
    fresh.load_map(map)
}

/// `MODATYP.DAT`'s values the world keeps. // 1010:000e
fn apply_modatyp(w: &mut World, m: &Modatyp) {
    w.max_location_effect = m.max_lageeinfluss;
    w.renovation_cost = m.renovierungskosten;
    let f = &mut w.finances;
    f.credit_size = m.kreditgroesse;
    f.max_debt = m.max_schulden;
    f.interest_rate = m.zinssatz;
    f.start_money = m.startgeld;
    w.land_costs = crate::terrain::LandCosts {
        from_water: m.land_aus_wasser_hebekosten,
        raise: m.landhebekosten,
        into_water: m.land_in_wasser_senkkosten,
        lower: m.landsenkkosten,
        palm: m.palmenabrisskosten,
    };
    w.road_demolition_cost = m.strassenabrisskosten;
    w.foundation_cost = m.fundamentbaukosten;
    w.connection_values = m.angeschlossenumrechnung;
}

/// The players' names: `NAMEN.TXT`'s first line for nobody (player 0),
/// then from a random name of the list (`Random(count)`) every
/// `NAME_STEPS[Random(10)]`th one round the list for players 2 to 8. The
/// second line, the human's, is the start menu's to show. The original
/// divides by zero with no names; the port leaves them empty. // 1010:0414
fn pick_names(w: &mut World, nobody: &[u8], names: &[Vec<u8>]) {
    let mut player_names = vec![Vec::new(); 9];
    player_names[0] = nobody.to_vec();
    let step = NAME_STEPS[usize::from(w.random.random(10))];
    let count = i16::try_from(names.len()).unwrap_or(i16::MAX);
    let mut k = w.random.random(count.cast_unsigned()).cast_signed();
    // BUG(original): with no names the modulo divides by zero; the port leaves them empty.
    if count > 0 {
        for name in &mut player_names[2..] {
            *name = names.get(usize::try_from(k).unwrap_or(0)).cloned().unwrap_or_default();
            k = (k + step) % count;
        }
    }
    w.player_names = player_names;
}
