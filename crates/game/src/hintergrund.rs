//! `THintergrund`'s timer states 1000–1500 (1000:2466), which start a game
//! once the start menu's Start or the load form has set them going
//! (`docs/startup.md`). What's ported is the choice of the island, the
//! part of loading a map file that the world needs, a new game's world
//! (the island made from the start menu's settings and everything the
//! game's data sets up around it), a scenario's ([`load_world`] and
//! [`start_scenario`]) and a saved game's ([`load_saved_game`],
//! `docs/savegame.md`).

use std::sync::Arc;

use formats::data::{self, Abrechnung, Modatyp};
use formats::savegame::SaveGame;
use vcl::random::Random;

use crate::enhance::{Enhancement, Enhancements, Fixes};
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

/// The five screen sizes the game has pictures for, each with the form
/// sizes state 110 snaps to it (`(min, max)` width and height, ±20
/// pixels) and its title and background pictures in `GRAFIK\`, in the
/// order state 110 tries them. // 1000:2466 (state 110), 1000:233b..23b0
pub const LAYOUTS: [Layout; 5] = [
    Layout {
        size: (800, 600),
        widths: (780, 820),
        heights: (580, 620),
        title: "tit80x60.BMP",      // 1000:233b
        background: "bck80x60.BMP", // 1000:237c
    },
    Layout {
        size: (640, 480),
        widths: (620, 660),
        heights: (460, 500),
        title: "tit64x48.BMP",      // 1000:2348
        background: "bck64x48.BMP", // 1000:2389
    },
    Layout {
        size: (1024, 768),
        widths: (1004, 1044),
        heights: (748, 788),
        title: "tit10x76.BMP",      // 1000:2355
        background: "bck10x76.BMP", // 1000:2396
    },
    Layout {
        size: (1152, 870),
        widths: (1132, 1172),
        heights: (850, 890),
        title: "tit11x87.BMP",      // 1000:2362
        background: "bck11x87.BMP", // 1000:23a3
    },
    Layout {
        size: (1280, 1024),
        widths: (1260, 1300),
        heights: (1004, 1044),
        title: "tit12x10.BMP",      // 1000:236f
        background: "bck12x10.BMP", // 1000:23b0
    },
];

/// One of [`LAYOUTS`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout {
    /// `screen_width`, `screen_height`.
    pub size: (i32, i32),
    /// The form widths and heights that snap to it, both ends included.
    pub widths: (i32, i32),
    pub heights: (i32, i32),
    pub title: &'static str,
    pub background: &'static str,
}

/// The screen size the game lays out for (`screen_width`,
/// `screen_height`, `1110:54fc`, `1110:54fe`) on a desktop where the
/// maximized `THintergrund` is `width` × `height`: the first of
/// [`LAYOUTS`] whose ranges hold it, else the form's size as it is.
/// Every other form is placed from this size, never from the desktop's.
/// // 1000:2466 (state 110)
///
/// BUG(original): a form up to 20 pixels smaller than a layout snaps up to it, so what the game puts at the right or bottom edge (the money menu, the compass, the load form) and the right and bottom of the centred forms are cut off by up to 20 pixels.
///
/// With `desktop_background` (`enhance`) it's the form's size as it is.
#[must_use]
pub fn screen_size(width: i32, height: i32, enhance: Enhancements) -> (i32, i32) {
    if enhance.has(Enhancement::DesktopBackground) {
        return (width, height);
    }
    snapped(width, height).map_or((width, height), |l| l.size)
}

/// The layout a form `width` × `height` snaps to. // 1000:2466 (state 110)
fn snapped(width: i32, height: i32) -> Option<&'static Layout> {
    LAYOUTS
        .iter()
        .find(|l| (l.widths.0..=l.widths.1).contains(&width) && (l.heights.0..=l.heights.1).contains(&height))
}

/// The layout whose pictures states 430 (the title), 450 and 1130 (the
/// background) load for `screen`, the size [`screen_size`] picked; none
/// for any other size, which leaves `backgroundimage` empty and the
/// form's black showing. // 1000:2466 (states 430, 450, 1130)
///
/// BUG(original): any screen size but the five has no title and no background picture: the intro's title and the desktop around the game stay black.
///
/// With `desktop_background` (`enhance`) every size has one: the
/// layout's it would snap to, else the biggest that fits, else the
/// smallest; the caller centres its pictures on the screen.
#[must_use]
pub fn layout(screen: (i32, i32), enhance: Enhancements) -> Option<&'static Layout> {
    if enhance.has(Enhancement::DesktopBackground) {
        let fits = |l: &&Layout| l.size.0 <= screen.0 && l.size.1 <= screen.1;
        return snapped(screen.0, screen.1)
            .or_else(|| LAYOUTS.iter().filter(fits).max_by_key(|l| l.size.0 * l.size.1))
            .or_else(|| LAYOUTS.iter().min_by_key(|l| l.size.0 * l.size.1));
    }
    LAYOUTS.iter().find(|l| l.size == screen)
}

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
/// the start money and the scenario the number of computer players, who
/// are in the game and the seats after them out
/// (`*1110:6f24 + 0x7f + p`), and the books' years start empty
/// (1070:2653). [`start_scenario`] does the rest of state 1500, which
/// needs the game's tables and random numbers. // 1000:2466
///
/// # Errors
/// If the map lacks a field the world needs.
pub fn load_world(map: &SaveGame, settings: &Settings, scenario_page: bool) -> Result<World, world::Error> {
    let mut world = World::from_map(map)?;
    if scenario_page {
        // sys_pstr_store(0xff, 1110:0a02, 1110:6356): player 1's name.
        if let Some(name) = world.player_names.get_mut(1) {
            name.clone_from(&settings.name);
        }
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
        // The players in the game and the empty seats.
        for p in 1..=8 {
            world.set_player_out(p, u8::from(p > world.player_count));
        }
        clear_book_years(&mut world); // 1070:2653
    }
    Ok(world)
}

/// The building types scenarios 1, 4 and 6 take off the build menu: the
/// player can't build them there. // 1000:21a0
pub const SCENARIO_BANNED: [u8; 12] = [0x6f, 0x71, 0x79, 0x7e, 0x25, 0x3c, 0x1e, 0x1d, 0x7d, 0x23, 0x6c, 0x6e];

/// The scenarios whose build menu goes without [`SCENARIO_BANNED`].
/// // 1000:2466, state 1500
const BANNING_SCENARIOS: [u8; 3] = [1, 4, 6];

/// The rest of state 1500 for a scenario, once the world has its tables
/// and its random numbers: the scenario's events (`events`, the rows of
/// its `EREIGNn.DAT`, [`scenario_events`]) replace the map's, scenarios
/// 1, 4 and 6 take [`SCENARIO_BANNED`] off the build menu (1000:21a0),
/// and the computer players find where they are (10a8:0002). The
/// original then sets the speech and the music on, the view's options.
/// // 1000:2466
pub fn start_scenario(world: &mut World, events: Option<&[[i32; 12]]>) {
    if let Some(rows) = events {
        world.load_event_rows(rows); // 1010:36f8
    }
    if BANNING_SCENARIOS.contains(&world.scenario) {
        for building in SCENARIO_BANNED {
            world.remove_from_build_menu(building); // 1018:26b9
        }
    }
    world.computers_find_themselves(); // 10a8:0002
}

/// The rows of scenario `scenario`'s events, `DATA\EREIGNn.DAT`
/// ([`SCENARIO_EVENTS`]), from `read` by their path on the CD; `None` for
/// a number that isn't a scenario's. // 1000:2466, 1010:36f8
///
/// # Errors
/// If the file is missing or doesn't parse.
pub fn scenario_events(
    scenario: u8,
    read: impl Fn(&str) -> Option<Vec<u8>>,
) -> Result<Option<Vec<[i32; 12]>>, LoadError> {
    let Some(name) = usize::from(scenario)
        .checked_sub(1)
        .and_then(|i| SCENARIO_EVENTS.get(i))
    else {
        return Ok(None);
    };
    let path = format!("DATA/{}", name.to_ascii_uppercase());
    let data = read(&path).ok_or_else(|| LoadError(format!("{path} not found")))?;
    data::ereignis(&data)
        .map(Some)
        .map_err(|e| LoadError(format!("{path}: {e}")))
}

/// The books' years, last year's and this one's with their totals
/// (kinds 0x52 to 0x6b), start at 0. // 1070:2653
fn clear_book_years(world: &mut World) {
    for kind in 0x52..=0x6b_u8 {
        if let Some(v) = world.books.year.get_mut(usize::from(kind - world::FIRST_BOOK)) {
            *v = 0;
        }
    }
}

/// A scenario's start money by difficulty: the debt limit doubles on easy
/// and halves on hard, and the human starts with half of it, as a loan at
/// 6 percent. Disasters are off on easy, which counts as playing without
/// them ([`World::played_without_disasters`]). // 1000:2466, state 1500
fn start_money(world: &mut World) {
    let f = &mut world.finances;
    match world.difficulty {
        0 => {
            f.max_debt = f.max_debt.wrapping_mul(2);
            world.disasters = false;
            world.played_without_disasters = true;
        }
        1 => {
            world.disasters = true;
            world.played_without_disasters = false;
        }
        2 => {
            f.max_debt /= 2;
            world.disasters = true;
            world.played_without_disasters = false;
        }
        _ => {}
    }
    if (0..=2).contains(&world.difficulty) {
        f.start_money = f.max_debt / 2;
    }
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
    /// `MONATE.TXT`, which the news archive's dates are written with
    /// (`crate::news::DateFormat`). // 1030:000d
    pub months: data::Strings,
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
            sabotage: crate::sabotage::kinds(&parsed("SABOTAGE.TXT", data::sabotage(&file("TEXTE/SABOTAGE.TXT")?))?.1),
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
            months: parsed("MONATE.TXT", data::monate(&file("TEXTE/MONATE.TXT")?))?,
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
///
/// The game plays with `fixes` (`crate::enhance`), which are in place
/// before anything is set up, as some change how the island is made.
pub fn new_world(settings: &Settings, data: &GameData, seed: u32, fixes: Fixes) -> World {
    set_up(settings, data, seed, true, fixes)
}

/// States 100 to 1500 for a new game with `fixes`, with the island
/// generated in state 1210 (`generate`) or, for a map file, only
/// cleared. // 1000:2466
fn set_up(settings: &Settings, data: &GameData, seed: u32, generate: bool, fixes: Fixes) -> World {
    let mut w = World {
        random: Random::new(seed),
        tables: Arc::clone(&data.tables),
        fixes,
        ..World::default()
    };
    // State 100: 1010:064a.
    apply_modatyp(&mut w, &data.modatyp); // 1010:000e
    pick_names(&mut w, &data.nobody, &data.names); // 1010:0414
    w.news.date_format = crate::news::DateFormat {
        months: data.months.clone(),
        land: data.modatyp.land as u8,
    };
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
/// stay. The game plays with the file's fixes, and `fixes` for those the
/// file doesn't name (all of them in an original file). // 1000:2466
///
/// # Errors
/// If the map lacks a field the world needs.
pub fn load_saved_game(
    map: &SaveGame,
    settings: &Settings,
    data: &GameData,
    seed: u32,
    fixes: Fixes,
) -> Result<World, world::Error> {
    let fresh = set_up(settings, data, seed, false, fixes);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screen_sizes() {
        assert_eq!(screen_size(800, 600, Enhancements::ORIGINAL), (800, 600));
        assert_eq!(screen_size(780, 620, Enhancements::ORIGINAL), (800, 600));
        assert_eq!(screen_size(779, 600, Enhancements::ORIGINAL), (779, 600));
        assert_eq!(screen_size(660, 460, Enhancements::ORIGINAL), (640, 480));
        assert_eq!(screen_size(1024, 768, Enhancements::ORIGINAL), (1024, 768));
        assert_eq!(screen_size(1152, 864, Enhancements::ORIGINAL), (1152, 870));
        assert_eq!(screen_size(1280, 1024, Enhancements::ORIGINAL), (1280, 1024));
        assert_eq!(screen_size(1366, 768, Enhancements::ORIGINAL), (1366, 768));
        assert_eq!(screen_size(1920, 1080, Enhancements::ORIGINAL), (1920, 1080));
        let o = Enhancements::ORIGINAL;
        assert_eq!(layout((800, 600), o).map(|l| l.background), Some("bck80x60.BMP"));
        assert_eq!(layout((1366, 768), o), None);
        // `desktop_background`: no snapping, and a picture for any size.
        let mut e = o;
        e.set(Enhancement::DesktopBackground, true);
        assert_eq!(screen_size(780, 620, e), (780, 620));
        let background = |size| layout(size, e).map(|l| l.background);
        assert_eq!(background((780, 620)), Some("bck80x60.BMP"));
        assert_eq!(background((1366, 768)), Some("bck10x76.BMP"));
        assert_eq!(background((1920, 1080)), Some("bck12x10.BMP"));
        assert_eq!(background((900, 700)), Some("bck80x60.BMP"));
        assert_eq!(background((700, 1000)), Some("bck64x48.BMP"));
    }
}
