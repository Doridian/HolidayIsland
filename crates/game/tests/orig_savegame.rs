//! Saves and loads games with the CD's data: the CD's maps and scenarios
//! saved again as they were read, a played game through a file and back
//! (ticking on as the game in memory does), a saved game loaded from the
//! start menu, and the load form and the file menu drawn. Skipped when
//! `orig/` isn't there. With `LADENFORM_PPM=path` it writes the load form
//! over the start menu, as the screen shows it, to `path`, and the file
//! menu next to it (`path` with `-dateimenue` and `-dateimenue-save`
//! before the extension).

use std::path::{Path, PathBuf};

use formats::bmp::Bitmap;
use formats::data::{fontdat, sonstige, start_txt, szenario_txt};
use formats::savegame::SaveGame;
use game::auction;
use game::command::{self, Command};
use game::dateimenue::{self, Dateimenue};
use game::enhance::Fixes;
use game::hintergrund::{GameData, SCENARIO_FILES, load_saved_game, load_world, new_world};
use game::ladenform::{self, LadenForm, saved_games};
use game::savegame::{MapView, differences};
use game::startform::{self, Settings, StartForm};
use game::text::Font;
use game::widgets::ShiftState;
use game::world::World;
use vcl::Palette;
use vcl::random::Random;
use vcl::surface::{Rop, Surface};

fn orig() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../orig")
}

/// Reads `name` from `orig/`, whatever the case of its letters.
fn read(name: &str) -> Option<Vec<u8>> {
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

fn game_data() -> Option<GameData> {
    if !orig().join("DATA").exists() {
        eprintln!("skipping: {} not found", orig().display());
        return None;
    }
    Some(GameData::load(read).unwrap())
}

/// The CD's map files: the tutorial and the scenarios.
fn cd_maps() -> Vec<(String, SaveGame)> {
    std::iter::once("MAPS/TUTORIAL.MAP".to_owned())
        .chain(SCENARIO_FILES.iter().map(|f| format!("SZENARIO/{f}")))
        .map(|name| {
            let map = SaveGame::parse(&read(&name).unwrap()).unwrap();
            (name, map)
        })
        .collect()
}

/// A tick with a player who leaves every auction as soon as its form
/// opens.
fn step(world: &mut World) {
    world.tick();
    if world.auction.open {
        world.apply(Command {
            player: 1,
            action: command::Action::Auction(auction::Action::Leave),
        });
    }
}

/// What a reload turns round, put back in the order of `before`, the
/// game that was saved: the building sites come back the other way
/// round, and each plot's houses and facilities in the reverse of the
/// houses' list, whatever order they had.
fn turn_round(world: &mut World, before: &World) {
    let mut sites = world.building_sites.clone();
    sites.reverse();
    assert_eq!(sites, before.building_sites, "the sites, the other way round");
    world.building_sites = sites;
    assert_eq!(world.plots.len(), before.plots.len());
    for (p, q) in world.plots.iter_mut().zip(&before.plots) {
        for (mine, theirs) in [(&mut p.houses, &q.houses), (&mut p.facilities, &q.facilities)] {
            let (mut a, mut b) = (mine.clone(), theirs.clone());
            a.sort_unstable();
            b.sort_unstable();
            assert_eq!(a, b, "a plot's buildings");
            mine.clone_from(theirs);
        }
    }
}

/// `world` saved, as bytes, parsed again.
fn through_a_file(world: &World, view: &MapView) -> SaveGame {
    let bytes = world.to_map(view).to_bytes();
    SaveGame::parse(&bytes).unwrap()
}

/// Whether the `string`s in a block of them are the same, up to each
/// one's length: what follows is whatever memory held in the original.
fn same_strings(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len()
        && a.chunks(256)
            .zip(b.chunks(256))
            .all(|(x, y)| x[..=usize::from(x[0])] == y[..=usize::from(x[0])])
}

#[test]
fn cd_maps_save_as_they_were_read() {
    if game_data().is_none() {
        return;
    }
    for (name, file) in cd_maps() {
        let world = World::from_map(&file).unwrap();
        let view = MapView::from_map(&file);
        let saved = world.to_map(&view);
        assert_eq!(saved.version, file.version);
        let (a, b): (Vec<_>, Vec<_>) = (file.numbered().collect(), saved.numbered().collect());
        assert_eq!(a.len(), b.len(), "{name}");
        for ((n, x), (_, y)) in a.iter().zip(&b) {
            assert_eq!(x.dest, y.dest, "{name}");
            if x.data == y.data {
                continue;
            }
            match x.dest {
                // The players' names and the computers' status text.
                "1110:0902" | "1110:70f2" => assert!(same_strings(&x.data, &y.data), "{name}: {}", x.dest),
                // The news archive's dates, the same way.
                "*1110:663a" => assert!(
                    x.data.len() == y.data.len()
                        && x.data
                            .chunks(0x11c)
                            .zip(y.data.chunks(0x11c))
                            .all(|(a, b)| a[..0x14] == b[..0x14]
                                && same_strings(&a[0x14..0x114], &b[0x14..0x114])
                                && a[0x114..] == b[0x114..]),
                    "{name}: the news"
                ),
                // `save_map` writes zeros there; the CD's files have
                // something else (an older version's data?).
                "(a local in 10a0:23c9)" => assert!(y.data.iter().all(|&b| b == 0)),
                _ => panic!("{name}: {} #{n} changed", x.dest),
            }
        }
        // The rest is kept: about 20 of the world's globals and 150 of the
        // view's fields aren't used yet.
        assert!(!world.unported.is_empty() && !view.unported.is_empty(), "{name}");
    }
}

#[test]
fn cd_maps_load_again() {
    let Some(data) = game_data() else { return };
    for (name, file) in cd_maps() {
        let mut world = World::from_map(&file).unwrap();
        world.tables = std::sync::Arc::clone(&data.tables);
        world.random = Random::new(0x1996);
        let view = MapView::from_map(&file);
        let parsed = through_a_file(&world, &view);
        assert_eq!(MapView::from_map(&parsed), view, "{name}");
        let mut back = world.load_map(&parsed).unwrap();
        turn_round(&mut back, &world);
        assert_eq!(differences(&back, &world), Vec::<&str>::new(), "{name}");
    }
}

/// A new game played for a few months, saved, loaded in the middle of the
/// game (the file menu's "Laden") and played on: it plays as the game in
/// memory does, once the building sites, which a reload turns round, are
/// in their order again.
#[test]
fn a_played_game_loads_and_plays_on() {
    let Some(data) = game_data() else { return };
    let settings = Settings::default();
    let mut world = new_world(&settings, &data, 0x0bad_cafe, Fixes::ORIGINAL);
    while world.days < 120 || world.building_sites.len() < 2 {
        step(&mut world);
        assert!(world.days < 2000, "no two building sites at once");
    }
    let view = MapView {
        scroll: (240, 237),
        occupancy_pointers: true,
        ..MapView::default()
    };
    let parsed = through_a_file(&world, &view);
    // Through the file menu's command, which reads it over this game.
    let mut loaded = world.clone();
    let tick = loaded.apply(Command {
        player: 1,
        action: command::Action::Load(std::sync::Arc::new(parsed.clone())),
    });
    assert!(tick.loaded && tick.redraw_all);
    // The original turns the building sites and the plots' lists round.
    assert_ne!(loaded.building_sites, world.building_sites);
    assert_eq!(differences(&loaded, &world), ["plots", "building_sites"]);
    turn_round(&mut loaded, &world);
    assert_eq!(differences(&loaded, &world), Vec::<&str>::new());
    // Played on, the two stay the same.
    for _ in 0..9000 {
        step(&mut world);
        step(&mut loaded);
    }
    assert!(world.days > 480);
    assert_eq!(differences(&loaded, &world), Vec::<&str>::new());
    // Saved again, it's the same file.
    assert_eq!(through_a_file(&loaded, &view), through_a_file(&world, &view));

    // A broken file loads nothing.
    let mut bytes = parsed.to_bytes();
    bytes.truncate(1000);
    assert!(SaveGame::parse(&bytes).is_err());
}

/// "Altes Spiel fortsetzen": the saved game over a game set up as for
/// Start, without an island. It has the file's world; what the file
/// doesn't hold is the new set-up's, and it plays.
#[test]
fn a_saved_game_loads_from_the_start_menu() {
    let Some(data) = game_data() else { return };
    let mut world = new_world(&Settings::default(), &data, 7, Fixes::ORIGINAL);
    for _ in 0..2000 {
        step(&mut world);
    }
    let parsed = through_a_file(&world, &MapView::default());
    // A different start menu: the file's settings win.
    let menu = Settings {
        difficulty: 2,
        name: b"Jemand".to_vec(),
        ..Settings::default()
    };
    let mut loaded = load_saved_game(&parsed, &menu, &data, 99, Fixes::ORIGINAL).unwrap();
    assert_eq!(loaded.player_names, world.player_names);
    assert_eq!(loaded.difficulty, world.difficulty);
    assert_eq!(loaded.island, world.island);
    // The set-up drew random numbers: RandSeed isn't the file's (it has
    // none) and not the seed.
    assert_ne!(loaded.random, Random::new(99));
    let mut differ = differences(&loaded, &world);
    differ.retain(|f| !["random", "plots", "building_sites", "people", "auction", "computers"].contains(f));
    assert_eq!(differ, Vec::<&str>::new());
    // The start menu shows the game's settings when the player goes back.
    assert_eq!(Settings::from_world(&loaded).name, world.player_names[1]);
    for _ in 0..2000 {
        step(&mut loaded);
    }
    assert!(loaded.days > world.days);

    // The CD's tutorial loads the same way.
    let tutorial = SaveGame::parse(&read("MAPS/TUTORIAL.MAP").unwrap()).unwrap();
    let mut w = load_saved_game(&tutorial, &menu, &data, 3, Fixes::ORIGINAL).unwrap();
    assert_eq!(w.island, World::from_map(&tutorial).unwrap().island);
    let day = w.days;
    for _ in 0..1500 {
        step(&mut w);
    }
    assert!(w.days > day);
}

/// Each scenario, played a little, then saved and loaded, plays on as
/// before.
#[test]
fn scenarios_play_on_after_a_reload() {
    let Some(data) = game_data() else { return };
    for (n, file) in SCENARIO_FILES.iter().enumerate() {
        let map = SaveGame::parse(&read(&format!("SZENARIO/{file}")).unwrap()).unwrap();
        let settings = Settings {
            scenario: u8::try_from(n + 1).unwrap(),
            ..Settings::default()
        };
        let mut world = load_world(&map, &settings, true).unwrap();
        world.tables = std::sync::Arc::clone(&data.tables);
        world.random = Random::new(u32::try_from(n).unwrap());
        for _ in 0..600 {
            step(&mut world);
        }
        let view = MapView::from_map(&map);
        let mut loaded = world.load_map(&through_a_file(&world, &view)).unwrap();
        turn_round(&mut loaded, &world);
        assert_eq!(differences(&loaded, &world), Vec::<&str>::new(), "{file}");
        for _ in 0..3000 {
            step(&mut world);
            step(&mut loaded);
        }
        assert_eq!(differences(&loaded, &world), Vec::<&str>::new(), "{file}");
    }
}

// The forms.

fn bitmap(name: &str) -> Bitmap {
    Bitmap::parse(&read(&format!("GRAFIK/{name}")).unwrap()).unwrap()
}

fn font() -> Font {
    Font::new(
        &bitmap("FONT.BMP"),
        &fontdat(&read("DATA/FONTDAT1.DAT").unwrap()).unwrap(),
        &fontdat(&read("DATA/FONTDAT2.DAT").unwrap()).unwrap(),
    )
}

fn surfaces(files: &[&str]) -> Vec<Surface> {
    files.iter().map(|f| Surface::from_bitmap(&bitmap(f))).collect()
}

fn write_ppm(path: &Path, s: &Surface, palette: &Palette) {
    let mut ppm = format!("P6\n{} {}\n255\n", s.width, s.height).into_bytes();
    for &i in &s.pixels {
        let c = palette[usize::from(i)];
        ppm.extend_from_slice(&[c.r, c.g, c.b]);
    }
    std::fs::write(path, ppm).unwrap();
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_stem().unwrap().to_owned();
    name.push(format!("-{suffix}.ppm"));
    path.with_file_name(name)
}

#[test]
fn draws_the_load_form_and_the_file_menu() {
    if game_data().is_none() {
        return;
    }
    let texts = sonstige(&read("TEXTE/SONSTIGE.TXT").unwrap()).unwrap().texts;
    // What the port lists: the CD's tutorial and a few of the player's.
    let files = [
        &b"Insel_1.map"[..],
        b"Last.map",
        b"TUTORIAL.MAP",
        b"Wahl96.map",
        b"zweite.map",
    ]
    .map(<[u8]>::to_vec);
    let games = saved_games(&files, game::enhance::Enhancements::ORIGINAL);
    assert_eq!(games[0], b"zweite");
    let mut form = LadenForm::new(font(), surfaces(&ladenform::BITMAP_FILES), &texts[7], games.clone());
    let background = bitmap("BCK80X60.BMP");
    let palette = background.palette;

    // The second game, then "Laden".
    let r = form.mouse_down(0x30, 0x2e + 0xf + 4, ShiftState::LEFT);
    assert!(r.refresh);
    let picked = form.bild().clone();
    let (x, y) = (0x22 + 0x20, 0xba + 8);
    let _ = form.mouse_down(x, y, ShiftState::LEFT);
    let r = form.mouse_up(x, y);
    assert_eq!(r.action, Some(ladenform::Action::Load(b"Wahl96.map".to_vec())));

    let mut menu = Dateimenue::new(font(), surfaces(&dateimenue::BITMAP_FILES), texts, 305, 139);
    menu.show(games);
    let shown = menu.image().clone();
    let _ = menu.mouse_down(0x61 + 0x30, 0x2e + 4, true, ShiftState::LEFT);
    let _ = menu.mouse_down(20, 75, true, ShiftState::LEFT);
    let saving = menu.image().clone();
    assert_eq!(menu.mouse_up(20, 75, true).action, None);
    assert_eq!(
        menu.key_down(0x0d, ShiftState::default()).action,
        Some(dateimenue::Action::Save(b"zweite.map".to_vec()))
    );

    if let Some(out) = std::env::var_os("LADENFORM_PPM") {
        let out = PathBuf::from(out);
        // The screen: the background, the start menu, the form.
        let mut screen = Surface::from_bitmap(&background);
        let clip = screen.bounds();
        let mut start = StartForm::new(
            font(),
            surfaces(&startform::BITMAP_FILES),
            &start_txt(&read("TEXTE/START.TXT").unwrap()),
            &szenario_txt(&read("TEXTE/SZENARIO.TXT").unwrap()),
            Settings::default(),
        );
        start.show();
        let (sx, sy) = StartForm::position(800, 600);
        screen.blit(sx, sy, start.bild(), start.bild().bounds(), Rop::Copy, clip);
        let (lx, ly) = LadenForm::position(800, 600);
        screen.blit(lx, ly, &picked, picked.bounds(), Rop::Copy, clip);
        write_ppm(&out, &screen, &palette);
        write_ppm(&with_suffix(&out, "dateimenue"), &shown, &palette);
        write_ppm(&with_suffix(&out, "dateimenue-save"), &saving, &palette);
    }
}
