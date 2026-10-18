//! The later build's `UPGRADE\` content from the English release's
//! folder (`orig-en/`, `docs/versions.md`): its scenarios 8 to 13 start
//! and play, the start menu has twelve scenarios, the high scores
//! fourteen tables, the end form the new texts and films, and the
//! prices come from `UPGRADE\HAUSPREI.DAT`. Skipped when `orig-en/` isn't
//! there. With `UPGRADE_PPM=dir` it writes the start menu's scenario page
//! and the high scores there.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use formats::bmp::Bitmap;
use formats::data::{
    SZENARIO_LINES_UPGRADE, animdat1, animdat4, darstell, fontdat, groundnr, hausdata, hausnr, modatyp, start_txt,
    szenario_txt_lines, text_lines,
};
use formats::highscore::HighScores;
use formats::savegame::SaveGame;
use game::highscoreform::{self, Highscoreform, ScoreTable};
use game::hintergrund::{
    GameData, SCENARIO_BANNED, UPGRADE_SCENARIO_OPPONENTS, UPGRADE_SCENARIOS, hausprei_table, load_world,
    scenario_events, scenario_path, start_scenario,
};
use game::lang::Language;
use game::schlussform::{self, Results};
use game::startform::{self, Action, Settings, StartForm};
use game::text::Font;
use game::widgets::ShiftState;
use game::world::{Tables, World};
use vcl::Palette;
use vcl::random::Random;
use vcl::surface::Surface;

fn dir(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").join(name)
}

/// Reads `name` from the folder `root`, whatever the case of its letters.
fn try_read_in(root: &Path, name: &str) -> Option<Vec<u8>> {
    let mut path = root.to_path_buf();
    for part in name.split('/') {
        let found = std::fs::read_dir(&path)
            .ok()?
            .filter_map(Result::ok)
            .find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(part))?;
        path = found.path();
    }
    std::fs::read(path).ok()
}

fn try_read(name: &str) -> Option<Vec<u8>> {
    try_read_in(&dir("orig-en"), name)
}

fn read(name: &str) -> Vec<u8> {
    try_read(name).unwrap_or_else(|| panic!("{name} not found"))
}

fn have_en() -> bool {
    let ok = try_read("UPGRADE/SZENARIO.TXT").is_some();
    if !ok {
        eprintln!("skipping: no {}", dir("orig-en/UPGRADE").display());
    }
    ok
}

fn bitmap(name: &str) -> Bitmap {
    Bitmap::parse(&read(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn grafik(files: &[&str]) -> Vec<Surface> {
    files
        .iter()
        .map(|f| Surface::from_bitmap(&bitmap(&format!("GRAFIK/{f}"))))
        .collect()
}

fn upgrade(files: &[&str]) -> Vec<Surface> {
    files
        .iter()
        .map(|f| Surface::from_bitmap(&bitmap(&format!("UPGRADE/{f}"))))
        .collect()
}

fn font() -> Font {
    Font::new(
        &bitmap("GRAFIK/FONT.BMP"),
        &fontdat(&read("DATA/FONTDAT1.DAT")).unwrap(),
        &fontdat(&read("DATA/FONTDAT2.DAT")).unwrap(),
    )
}

fn tables() -> Arc<Tables> {
    Arc::new(Tables {
        hausnr: hausnr(&read("DATA/HAUSNR.DAT")).unwrap(),
        hausdata: hausdata(&read("DATA/HAUSDATA.DAT")).unwrap(),
        hausprei: hausprei_table(try_read).unwrap(),
        groundnr: groundnr(&read("DATA/GROUNDNR.DAT")).unwrap(),
        animdat1: animdat1(&read("DATA/ANIMDAT1.DAT")).unwrap(),
        animdat4: animdat4(&read("DATA/ANIMDAT4.DAT")).unwrap(),
        form_korrektur: darstell(&read("DATA/DARSTELL.DAT")).unwrap().form_korrektur,
        road_cost: modatyp(&read("DATA/MODATYP.DAT")).unwrap().strassenbaukosten,
    })
}

fn write_ppm(name: &str, s: &Surface, palette: &Palette) {
    let Some(out) = std::env::var_os("UPGRADE_PPM") else {
        return;
    };
    let mut ppm = format!("P6\n{} {}\n255\n", s.width, s.height).into_bytes();
    for &i in &s.pixels {
        let c = palette[usize::from(i)];
        ppm.extend_from_slice(&[c.r, c.g, c.b]);
    }
    std::fs::write(Path::new(&out).join(format!("{name}.ppm")), ppm).unwrap();
}

fn in_menu(world: &World, building: u8) -> bool {
    world
        .build_menu
        .rows
        .iter()
        .flatten()
        .any(|row| row.contains(&i16::from(building)))
}

/// Scenario `n`'s world after state 1500 (en 1010:0591).
fn scenario(n: u8) -> World {
    let path = scenario_path(n).unwrap();
    let map = SaveGame::parse(&read(&path)).unwrap_or_else(|e| panic!("{path}: {e}"));
    let settings = Settings {
        scenario: n,
        difficulty: 1,
        name: b"Tester".to_vec(),
        ..Settings::default()
    };
    let mut world = load_world(&map, &settings, true).unwrap();
    assert_eq!(world.scenario, n, "{path}");
    world.tables = tables();
    world.random = Random::new(0x5eed);
    let rows = scenario_events(n, try_read).unwrap().unwrap();
    start_scenario(&mut world, Some(&rows));
    world
}

#[test]
fn prices_from_upgrade() {
    if !have_en() {
        return;
    }
    // The pyramid's price, the one number the later build changes.
    let en = hausprei_table(try_read).unwrap();
    assert_eq!(en[125][1], 30_000);
    // Without `UPGRADE\` the CD's table gets the same change.
    if try_read_in(&dir("orig"), "DATA/HAUSPREI.DAT").is_some() {
        let de = hausprei_table(|n| try_read_in(&dir("orig"), n)).unwrap();
        assert_eq!(de[125][1], 30_000);
        assert_eq!(de[124], en[124]);
    }
}

#[test]
fn the_new_scenarios_start_and_play() {
    if !have_en() {
        return;
    }
    for n in UPGRADE_SCENARIOS {
        let mut world = scenario(n);
        let opponents = UPGRADE_SCENARIO_OPPONENTS[usize::from(n - 8)];
        assert_eq!(world.player_count, opponents + 1, "scenario {n}");
        for p in 1..=8 {
            assert_eq!(
                world.player_out(p) != 0,
                p > world.player_count,
                "scenario {n}, player {p}"
            );
        }
        match n {
            8 | 12 | 13 => assert!(SCENARIO_BANNED.iter().all(|&b| !in_menu(&world, b)), "scenario {n}"),
            11 => assert!([0x6f, 0x71, 0x79].iter().all(|&b| !in_menu(&world, b))),
            _ => {}
        }
        if n == 10 {
            // The computers' list of category 10 cut to seven types.
            assert_eq!(world.computers.types[10 * 41], 7);
        }
        // A year of the game.
        for _ in 0..360 * 24 {
            let tick = world.tick();
            assert!(tick.game_over.is_none(), "scenario {n} over after a year");
        }
    }
}

#[test]
fn the_scenario_page_has_twelve() {
    if !have_en() {
        return;
    }
    let mut images = grafik(&startform::BITMAP_FILES);
    images.extend(upgrade(&startform::UPGRADE_BITMAP_FILES));
    let palette = bitmap("UPGRADE/STARTSZ1.BMP").palette;
    let szenario = szenario_txt_lines(&read("UPGRADE/SZENARIO.TXT"), SZENARIO_LINES_UPGRADE);
    let mut form = StartForm::new(
        font(),
        images,
        &start_txt(&read("TEXTE/START.TXT")),
        &szenario,
        Settings::default(),
    );
    form.show();
    // The mode button: the scenario page.
    let _ = form.mouse_down(0x12 + 4, 0x132 + 4, ShiftState::LEFT);
    let _ = form.mouse_up(0x12 + 4, 0x132 + 4, true);
    assert!(form.scenario_mode());
    assert_eq!(form.settings().scenario, 1);
    // The fourth column's second box is scenario 12.
    let _ = form.mouse_down(0x176 + 4, 0xec + 4, ShiftState::LEFT);
    assert_eq!(form.settings().scenario, 12);
    write_ppm("versions-startform-scenarios", form.bild(), &palette);
    // And the first column's first, scenario 1.
    let _ = form.mouse_down(5 + 4, 0xdb + 4, ShiftState::LEFT);
    assert_eq!(form.settings().scenario, 1);
    let _ = form.mouse_down(0x152 + 4, 0x132 + 4, ShiftState::LEFT);
    let up = form.mouse_up(0x152 + 4, 0x132 + 4, true);
    assert_eq!(up.action, Some(Action::Start { scenario: true }));
}

#[test]
fn high_scores_of_fourteen_games() {
    if !have_en() {
        return;
    }
    let first = HighScores::parse(&read("SOUND/HIGHSCOR.WAV")).unwrap();
    let second = HighScores::parse(&read("UPGRADE/HIGHSCO2.WAV")).unwrap();
    let table = ScoreTable::new(&first, Some(&second));
    let mut images = grafik(&highscoreform::BITMAP_FILES);
    images.extend(upgrade(&highscoreform::UPGRADE_BITMAP_FILES));
    let palette = bitmap("UPGRADE/HIGHSCO2.BMP").palette;
    let names = text_lines(&read("UPGRADE/HIGHSCO2.TXT"));
    let mut form = Highscoreform::new(
        font(),
        images,
        highscoreform::highscor_lines(&read("TEXTE/HIGHSCOR.TXT")),
        table,
    )
    .with_upgrade(names);
    // A win of scenario 9 goes into its table and the form opens on it.
    let r = form.add(9, b"Tester", b"17.10.26", b"1.000.000", 77);
    assert!(r.save);
    assert_eq!(form.table.entries[0][9].score, 77);
    let _ = form.show(9, false, (800, 600));
    assert_eq!(form.game, 9);
    write_ppm("versions-highscores", form.image(), &palette);
    // The normal game's box, at the right.
    let _ = form.mouse_down(0x1a5 + 4, 0xe6 + 4, ShiftState::LEFT);
    assert_eq!(form.game, 0);
    // Saved: scenario 9 in the second file's slot 2.
    assert_eq!(form.table.second_file().entries[0][2].score, 77);
}

#[test]
fn the_end_of_a_new_scenario() {
    if !have_en() {
        return;
    }
    let lines = schlussform::schluss_lines_upgrade(&read("UPGRADE/SCHLUSS.TXT"));
    let results = |scenario, won| Results {
        won,
        cheated: false,
        scenario,
        net_worth: b"1.000.000".to_vec(),
        guests: 5000,
        score: Some(50),
    };
    let won = schlussform::schluss_load_texts(&lines, &results(9, true));
    assert!(
        won.labels[3].starts_with(b"The campers"),
        "{:?}",
        String::from_utf8_lossy(&won.labels[3])
    );
    assert_eq!(schlussform::end_media(&results(9, true)).1, Some("ABCamper.avi"));
    let lost = schlussform::schluss_load_texts(&lines, &results(8, false));
    assert!(
        lost.labels[3].starts_with(b"Even today"),
        "{:?}",
        String::from_utf8_lossy(&lost.labels[3])
    );
    // The rest is twelve lines further on than v1.0's.
    assert!(
        won.labels[4].starts_with(b"Your fortune"),
        "{:?}",
        String::from_utf8_lossy(&won.labels[4])
    );
}

/// What the world takes from the game folder is the same in the German
/// CD's files (`orig/`) and the English release's: the tables
/// (`HAUSPREI.DAT` once v1.0's pyramid price is the later build's), the
/// set-up's data, `MODATYP.DAT` but for its `land` and the build menu's
/// lists. The English release's `BAUMENU.DAT` changes only the labels'
/// layouts, the view's (`lang/<code>/DATA/`). So a German and an English
/// player can play one network game (`docs/networking.md`). Skipped
/// without both folders.
#[test]
fn the_releases_give_the_world_the_same_data() {
    if !have_en() || try_read_in(&dir("orig"), "DATA/MODATYP.DAT").is_none() {
        return;
    }
    for language in Language::ALL {
        let de = GameData::load(|n| try_read_in(&dir("orig"), n), language).unwrap();
        let en = GameData::load(|n| try_read_in(&dir("orig-en"), n), language).unwrap();
        assert_eq!(de.tables, en.tables);
        let mut m = en.modatyp.clone();
        assert_eq!(
            (de.modatyp.land, m.land),
            (formats::data::Land::Deutschland, formats::data::Land::England)
        );
        m.land = de.modatyp.land;
        assert_eq!(format!("{:?}", de.modatyp), format!("{m:?}"));
        for (name, a, b) in [
            (
                "MAPERSTE.DAT",
                format!("{:?}", de.maperste),
                format!("{:?}", en.maperste),
            ),
            (
                "ABRECHNE.DAT",
                format!("{:?}", de.abrechnung),
                format!("{:?}", en.abrechnung),
            ),
            ("EREIGNIS.DAT", format!("{:?}", de.events), format!("{:?}", en.events)),
            ("ANIMDAT2.DAT", format!("{:?}", de.paths), format!("{:?}", en.paths)),
            ("NEINTRAG.DAT", format!("{:?}", de.entries), format!("{:?}", en.entries)),
            ("ANIMDAT3.DAT", format!("{:?}", de.tracks), format!("{:?}", en.tracks)),
            ("COMPBAU.DAT", format!("{:?}", de.compbau), format!("{:?}", en.compbau)),
            (
                "BAUMENU.DAT",
                format!("{:?}", de.baumenu.menus),
                format!("{:?}", en.baumenu.menus),
            ),
        ] {
            assert_eq!(a, b, "{name}");
        }
        assert_ne!(de.baumenu.layouts, en.baumenu.layouts, "the labels' layouts differ");
    }
}
