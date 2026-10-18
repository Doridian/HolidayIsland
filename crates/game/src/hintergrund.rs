//! `THintergrund`'s timer states 1000–1500 (1000:2466), which start a game
//! once the start menu's Start or the load form has set them going
//! (`docs/startup.md`). What's ported is the choice of the island, the
//! part of loading a map file that the world needs, a new game's world
//! (the island made from the start menu's settings and everything the
//! game's data sets up around it), a scenario's ([`load_world`] and
//! [`start_scenario`]) and a saved game's ([`load_saved_game`],
//! `docs/savegame.md`). It follows the later build (en 1010:0591,
//! `docs/versions.md`), whose scenarios 8 to 13 come from `UPGRADE\`.

use std::sync::Arc;

use formats::data::{self, Abrechnung, Modatyp};
use formats::savegame::SaveGame;
use vcl::random::Random;

use crate::enhance::{Enhancement, Enhancements, Fixes};
use crate::events::Event;
use crate::generator::Maperste;
use crate::lang::Language;
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

/// The later build's scenarios 8 to 13, whose files are in `UPGRADE\`
/// (`docs/versions.md`): their maps, events and computer players.
/// Scenario 7 is the tutorial's number, which no file of the start menu
/// has. // en 1010:0591, state 1500
pub const UPGRADE_SCENARIO_FILES: [&str; 6] = [
    "Weltwund.his", // en 1010:0467
    "Camper.his",   // en 1010:0474
    "BBonly.his",   // en 1010:047f
    "PRWunder.his", // en 1010:048a
    "Hurryup.his",  // en 1010:0497
    "Challeng.his", // en 1010:04a3
];
pub const UPGRADE_SCENARIO_EVENTS: [&str; 6] = [
    "ereign7.dat",  // en 1010:0512
    "ereign8.dat",  // en 1010:051e
    "ereign9.dat",  // en 1010:052a
    "ereign10.dat", // en 1010:0536
    "ereign11.dat", // en 1010:0543
    "ereign12.dat", // en 1010:0550
];
pub const UPGRADE_SCENARIO_OPPONENTS: [u8; 6] = [4, 3, 7, 4, 0, 5];

/// The first and last scenario `UPGRADE\` adds.
pub const UPGRADE_SCENARIOS: std::ops::RangeInclusive<u8> = 8..=13;

/// Where scenario `n`'s map is, by its path in the game's folder: 1 to 6
/// in `SZENARIO\`, 8 to 13 in `UPGRADE\`; none for another number.
#[must_use]
pub fn scenario_path(n: u8) -> Option<String> {
    match n {
        1..=6 => Some(format!("SZENARIO/{}", SCENARIO_FILES[usize::from(n - 1)])),
        8..=13 => Some(format!("UPGRADE/{}", UPGRADE_SCENARIO_FILES[usize::from(n - 8)])),
        _ => None,
    }
}

/// How many computer players scenario `n` has; none for a number state
/// 1500 doesn't set them for (0, the tutorial's 7). // en 1010:0591
#[must_use]
pub fn scenario_opponents(n: u8) -> Option<u8> {
    match n {
        1..=6 => Some(SCENARIO_OPPONENTS[usize::from(n - 1)]),
        8..=13 => Some(UPGRADE_SCENARIO_OPPONENTS[usize::from(n - 8)]),
        _ => None,
    }
}

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
    /// A scenario's map: its path in the game's folder
    /// ([`scenario_path`]).
    Scenario(String),
    /// A new island from the start menu's settings, made by 10b0:52d4 in
    /// state 1210: [`new_world`].
    Generated,
}

/// The island Start leads to: the scenario on the scenario page, or a
/// generated one. // 1000:2466, en 1010:0591, state 1500
#[must_use]
pub fn new_island(settings: &Settings, scenario_page: bool) -> Island {
    match scenario_path(settings.scenario) {
        Some(path) if scenario_page => Island::Scenario(path),
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
        if let Some(n) = scenario_opponents(settings.scenario) {
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

/// The scenarios whose build menu goes without [`SCENARIO_BANNED`]: 1, 4
/// and 6, and the later build's 8, 12 and 13. // 1000:2466, en 1010:0591,
/// state 1500
const BANNING_SCENARIOS: [u8; 6] = [1, 4, 6, 8, 12, 13];

/// What scenario 11 (the wonders) takes off the build menu: the first
/// three of [`SCENARIO_BANNED`]. // en 1010:01d5
const SCENARIO_11_BANNED: [u8; 3] = [0x6f, 0x71, 0x79];

/// Scenario 10 (B&B only) cuts the computers' list of types of category
/// 10 to its first seven (`computer_types[0x19a] := 7`): no hotels for
/// them either. // en 10b0:0002
const BB_ONLY_CATEGORY: usize = 10;
const BB_ONLY_TYPES: u8 = 7;

/// The scenarios whose computer players find where they are at the
/// start: all but the later build's 9, 10 and 12. // en 1010:0591
const FINDING_SCENARIOS: [u8; 10] = [1, 2, 3, 4, 5, 6, 7, 8, 11, 13];

/// The rest of state 1500 for a scenario, once the world has its tables
/// and its random numbers: the scenario's events (`events`, the rows of
/// its `EREIGNn.DAT`, [`scenario_events`]) replace the map's, scenarios
/// 1, 4, 6, 8, 12 and 13 take [`SCENARIO_BANNED`] off the build menu
/// (1000:21a0) and 11 the three of [`SCENARIO_11_BANNED`], 10 cuts the
/// computers' list of category 10, and the computer players find where
/// they are (10a8:0002), but not in 9, 10 and 12. Scenario 12 also puts
/// the build menu's submenu 5 back to its first row, which is the view's
/// (`crate::baumenue::Baumenue::start_scenario`). The original then sets
/// the speech and the music on, the view's options. // 1000:2466,
/// en 1010:0591
pub fn start_scenario(world: &mut World, events: Option<&[[i32; 12]]>) {
    if let Some(rows) = events {
        world.load_event_rows(rows); // 1010:36f8
    }
    if BANNING_SCENARIOS.contains(&world.scenario) {
        for building in SCENARIO_BANNED {
            world.remove_from_build_menu(1, building); // 1018:26b9
        }
    }
    match world.scenario {
        10 => world.computers.cut_type_list(BB_ONLY_CATEGORY, BB_ONLY_TYPES), // en 10b0:0002
        11 => {
            for building in SCENARIO_11_BANNED {
                world.remove_from_build_menu(1, building); // en 1010:01d5
            }
        }
        _ => {}
    }
    if FINDING_SCENARIOS.contains(&world.scenario) {
        world.computers_find_themselves(); // 10a8:0002
    }
}

/// The rows of scenario `scenario`'s events, `DATA\EREIGNn.DAT`
/// ([`SCENARIO_EVENTS`]) or for the later build's 8 to 13
/// `UPGRADE\EREIGN7.DAT` to `EREIGN12.DAT`, from `read` by their path in
/// the game's folder; `None` for a number that isn't a scenario's.
/// // 1000:2466, en 1010:0591, 1010:36f8
///
/// # Errors
/// If the file is missing or doesn't parse.
pub fn scenario_events(
    scenario: u8,
    read: impl Fn(&str) -> Option<Vec<u8>>,
) -> Result<Option<Vec<[i32; 12]>>, LoadError> {
    let path = match scenario {
        1..=6 => format!(
            "DATA/{}",
            SCENARIO_EVENTS[usize::from(scenario - 1)].to_ascii_uppercase()
        ),
        8..=13 => format!(
            "UPGRADE/{}",
            UPGRADE_SCENARIO_EVENTS[usize::from(scenario - 8)].to_ascii_uppercase()
        ),
        _ => return Ok(None),
    };
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

/// The pyramid's building type, whose `HAUSPREI.DAT` second column the
/// later build's `UPGRADE\HAUSPREI.DAT` lowers from 40 000 to 30 000; the
/// only number of the game's tables it changes (`docs/versions.md`).
const PYRAMID: usize = 125;
const PYRAMID_V10: i32 = 40_000;
const PYRAMID_LATER: i32 = 30_000;

/// `HAUSPREI.DAT` as the later build reads it: `UPGRADE\HAUSPREI.DAT`
/// (en 1048:341c), else the CD's `DATA\HAUSPREI.DAT` (1048:341c) with
/// the later build's one change, the pyramid's price, so that a game
/// without `UPGRADE\` plays with the later build's numbers too.
///
/// # Errors
/// If the file is missing or doesn't parse.
pub fn hausprei_table(read: impl Fn(&str) -> Option<Vec<u8>>) -> Result<Vec<[i32; 4]>, LoadError> {
    if let Some(upgrade) = read("UPGRADE/HAUSPREI.DAT") {
        return data::hausprei(&upgrade).map_err(|e| LoadError(format!("UPGRADE/HAUSPREI.DAT: {e}")));
    }
    let data = read("DATA/HAUSPREI.DAT").ok_or_else(|| LoadError("DATA/HAUSPREI.DAT not found".into()))?;
    let mut table = data::hausprei(&data).map_err(|e| LoadError(format!("HAUSPREI.DAT: {e}")))?;
    if let Some(row) = table.get_mut(PYRAMID)
        && row[1] == PYRAMID_V10
    {
        row[1] = PYRAMID_LATER;
    }
    Ok(table)
}

/// What a new game is set up from: the CD's `DATA\` and `TEXTE\` files
/// the program loads at the start (state 100, 1030:38a3) and when a game
/// starts (state 1220, 1030:3932), parsed. The texts are the world's
/// language's, from the port's language data (`crate::lang`).
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
    /// from, in [`GameData::language`]. // 1010:0414
    pub nobody: Vec<u8>,
    pub names: data::Strings,
    /// `BAUMENU.DAT`: the build menu's lists and layouts. // 1018:080f
    pub baumenu: data::BaumenuData,
    /// The world's language ([`World::language`]), whose `NAMEN.TXT` this
    /// is and whose `MONATE.TXT` the news archive's dates are written
    /// with. (Its `SABOTAGE.TXT` the world reads itself,
    /// `crate::sabotage::table`.)
    pub language: Language,
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
    /// (`DATA/MAPERSTE.DAT`, ...), and the texts of `language`.
    ///
    /// # Errors
    /// The first file that's missing or doesn't parse.
    pub fn load(read: impl Fn(&str) -> Option<Vec<u8>>, language: Language) -> Result<GameData, LoadError> {
        fn parsed<T, E: std::fmt::Display>(name: &str, r: Result<T, E>) -> Result<T, LoadError> {
            r.map_err(|e| LoadError(format!("{name}: {e}")))
        }
        let file = |name: &str| {
            language
                .file(name)
                .ok_or_else(|| LoadError(format!("{name} not in lang/{}", language.code())))
        };
        let dat = |name: &str| {
            let path = format!("DATA/{name}");
            read(&path).ok_or_else(|| LoadError(format!("{path} not found")))
        };
        let modatyp = parsed("MODATYP.DAT", data::modatyp(&dat("MODATYP.DAT")?))?;
        let tables = Tables {
            hausnr: parsed("HAUSNR.DAT", data::hausnr(&dat("HAUSNR.DAT")?))?,
            hausdata: parsed("HAUSDATA.DAT", data::hausdata(&dat("HAUSDATA.DAT")?))?,
            hausprei: hausprei_table(&read)?,
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
            language,
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
/// Not the world's, which the view sets up: the button bar's buttons
/// (`1110:651a` to `1110:6530`, 1010:2fa9), the playlist (`1110:6ef2` to
/// `1110:6efe`, 1018:37de), the sabotage window's `1110:54fa` and
/// `1110:65ba` (1028:361f), and what only the forms keep. The
/// auctioneer's frames (`1110:65b4` to `1110:65b9`, 1028:2449) start at 0
/// in the world's auction.
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
    let mut w = set_up_begin(settings, data, seed, fixes);
    // State 1210: for a map file, `clear_map` (10b0:3030) leaves the
    // empty island `World::default` has.
    let mut plots = 0;
    if generate {
        for stage in 0..crate::generator::STAGES.len() {
            w.generate_stage(stage, settings, &data.maperste, &mut plots); // 10b0:52d4
        }
    }
    set_up_finish(&mut w, data, plots);
    w
}

/// State 100 of [`new_world`] with `fixes`: the game's data and the
/// start menu's settings, before state 1210 makes the island
/// ([`World::generate_stage`], which the progress window shows a stage
/// at a time) and [`set_up_finish`] the rest. // 1000:2466
#[must_use]
pub fn set_up_begin(settings: &Settings, data: &GameData, seed: u32, fixes: Fixes) -> World {
    let mut w = World {
        random: Random::new(seed),
        tables: Arc::clone(&data.tables),
        fixes,
        ..World::default()
    };
    // State 100: 1010:064a.
    apply_modatyp(&mut w, &data.modatyp); // 1010:000e
    // The world's text is in the language the game is created in.
    w.language = data.language;
    w.folder_language = Language::of_land(data.modatyp.land);
    w.news = crate::news::News::new(w.language); // 1048:1ffd
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
    w
}

/// States 1220 and 1500 of [`new_world`], after [`set_up_begin`] and the
/// island's `plots`: the game's set-up and the first view. // 1000:2466
pub fn set_up_finish(w: &mut World, data: &GameData, plots: u16) {
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
    let mut w = fresh.load_map(map)?;
    // `game_start_setup` (1010:2820): with the money menu shown by its
    // saved window, 1070:2630 works this year's total out.
    w.geldmenu_refresh(&crate::simulation::Tick {
        loaded: true,
        ..crate::simulation::Tick::default()
    });
    Ok(w)
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
