//! Starts a game on the tutorial island and each scenario, and draws the
//! main screen. Skipped when `orig/` isn't there. With `ISLAND_PPM=path` it
//! writes the tutorial's main screen to `path`, and each scenario's next
//! to it (`path` with `-geld` and so on before the extension), the
//! first scenario after two years of palm growth (`-palms`), and the
//! people and vehicles on the fourth after some ticks (`-people`).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use formats::bmp::Bitmap;
use formats::data::{
    animdat1, animdat4, darstell, flusspos, fontdat, gfxpos, groundnr, grounpos, hausdata, hausnr, hauspos, hausprei,
    modatyp, monate, sonstige, straspos,
};
use formats::dfm::Object;
use formats::ne::{RT_RCDATA, ResId, resources};
use formats::savegame::SaveGame;
use game::darstellung::{BITMAP_FILES, Camera, Tiles};
use game::hauptschirm::{self, Hauptschirm, InfoTexts, KeyResult, LAYOUT};
use game::hintergrund::{Island, SCENARIO_FILES, load_world, new_island};
use game::startform::Settings;
use game::text::Font;
use game::widgets::ShiftState;
use game::world::{PALM, Tables, World};
use vcl::form::{Form, Kind};
use vcl::random::Random;
use vcl::surface::Surface;
use vcl::{Palette, Rect};

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

fn bitmap(name: &str) -> Bitmap {
    Bitmap::parse(&read(&format!("GRAFIK/{name}"))).unwrap()
}

fn tiles() -> Tiles {
    let data = |n: &str| read(&format!("DATA/{n}"));
    Tiles::new(
        hauspos(&data("HAUSPOS.DAT")).unwrap(),
        hausnr(&data("HAUSNR.DAT")).unwrap(),
        grounpos(&data("GROUNPOS.DAT")).unwrap(),
        groundnr(&data("GROUNDNR.DAT")).unwrap(),
        gfxpos(&data("GFXPOS.DAT")).unwrap(),
        straspos(&data("STRASPOS.DAT")).unwrap(),
        flusspos(&data("FLUSSPOS.DAT")).unwrap(),
        darstell(&data("DARSTELL.DAT")).unwrap(),
        BITMAP_FILES
            .iter()
            .map(|(_, f)| Surface::from_bitmap(&bitmap(f)))
            .collect(),
    )
}

fn main_screen(map: &SaveGame) -> Hauptschirm {
    let d = map.field("1110:5530").unwrap();
    let word = |at: usize| i16::from_le_bytes([d[at], d[at + 1]]);
    main_screen_at(map, (word(0), word(2)))
}

/// The main screen looking at the map from `origin`.
fn main_screen_at(map: &SaveGame, origin: (i16, i16)) -> Hauptschirm {
    let byte = |dest| map.field(dest).unwrap()[0];
    let camera = Camera {
        view: byte("1110:007c"),
        origin: (0, 0),
        parity: 0,
        owner_pointers: byte("1110:0058") != 0,
        occupancy_pointers: byte("1110:0059") != 0,
    };
    let font = Font::new(
        &bitmap("FONT.BMP"),
        &fontdat(&read("DATA/FONTDAT1.DAT")).unwrap(),
        &fontdat(&read("DATA/FONTDAT2.DAT")).unwrap(),
    );
    let title = &sonstige(&read("TEXTE/SONSTIGE.TXT")).unwrap().texts[0];
    let bitmaps = hauptschirm::BITMAP_FILES
        .iter()
        .map(|f| Surface::from_bitmap(&bitmap(f)))
        .collect();
    Hauptschirm::new(camera, origin, bitmaps, &font, title)
}

/// The main screen as `THauptschirm`'s form shows it, laid out by
/// [`LAYOUT`].
fn compose(h: &Hauptschirm, palette: &Palette) -> Surface {
    let exe = read("HOLIDAY.EXE");
    let res = resources(&exe)
        .unwrap()
        .into_iter()
        .find(|r| r.kind == ResId::Id(RT_RCDATA) && r.name == ResId::Name(b"THAUPTSCHIRM".to_vec()))
        .unwrap();
    let mut form = Form::load(&Object::parse(res.data).unwrap()).unwrap();
    form.components[0].bounds = Rect::new(0, 0, hauptschirm::WIDTH, hauptschirm::HEIGHT);
    form.components[0].visible = true;
    for (name, bounds) in LAYOUT {
        let c = form.component_mut(name).unwrap();
        c.bounds = bounds;
        c.visible = true;
    }
    for (name, image) in h.images() {
        if let Kind::Image { picture } = &mut form.component_mut(name).unwrap().kind {
            *picture = Some(image.clone());
        }
    }
    let size = |n: i32| usize::try_from(n).unwrap();
    let mut s = Surface::new(size(hauptschirm::WIDTH), size(hauptschirm::HEIGHT), 0);
    form.paint(&mut s, palette);
    s
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

/// How many of the map's pixels aren't black.
fn coverage(s: &Surface) -> usize {
    s.pixels.iter().filter(|&&p| p != 0).count()
}

#[test]
fn draws_the_islands() {
    if !orig().join("MAPS").exists() {
        eprintln!("skipping: {} not found", orig().display());
        return;
    }
    let tiles = tiles();
    assert!(tiles.complete());
    let palette = bitmap("GFXGROUN.BMP").palette;
    let out = std::env::var_os("ISLAND_PPM").map(PathBuf::from);

    let tutorial = SaveGame::parse(&read("MAPS/TUTORIAL.MAP")).unwrap();
    let world = World::from_map(&tutorial).unwrap();
    let mut h = main_screen(&tutorial);
    h.paint_map(&world, &tiles);
    let full = h.map.width * h.map.height;
    assert!(coverage(&h.map) > full * 9 / 10, "{} of {full} drawn", coverage(&h.map));
    if let Some(out) = &out {
        write_ppm(out, &compose(&h, &palette), &palette);
    }
    // Scrolling and turning redraw it.
    let before = h.map.clone();
    assert_eq!(
        h.key_down(0x27, ShiftState::default(), &world, &tiles),
        KeyResult::Redrawn
    );
    assert_ne!(h.map, before);
    for (view, name) in [(1, "view1"), (2, "view2"), (3, "view3")] {
        assert_eq!(
            h.key_down(0x2d, ShiftState::default(), &world, &tiles),
            KeyResult::Redrawn
        );
        assert_eq!(h.camera.view, view);
        if let Some(out) = &out {
            write_ppm(&with_suffix(out, name), &compose(&h, &palette), &palette);
        }
    }

    for (n, file) in (1..=6).zip(SCENARIO_FILES) {
        let settings = Settings {
            scenario: n,
            ..Settings::default()
        };
        assert_eq!(new_island(&settings, true), Island::Scenario(file));
        let map = SaveGame::parse(&read(&format!("SZENARIO/{file}"))).unwrap();
        let world = load_world(&map, &settings, true).unwrap();
        let mut h = main_screen(&map);
        h.paint_map(&world, &tiles);
        assert!(
            coverage(&h.map) > full * 9 / 10,
            "{file}: {} of {full} drawn",
            coverage(&h.map)
        );
        if let Some(out) = &out {
            let stem = file.split('.').next().unwrap().to_ascii_lowercase();
            write_ppm(&with_suffix(out, &stem), &compose(&h, &palette), &palette);
        }
    }
}

/// The palm squares' parts, by stage (`part mod 10`).
fn palm_stages(world: &World) -> [usize; 10] {
    let mut stages = [0; 10];
    for i in 0..251 {
        for j in 0..251 {
            if world.island.buildings.at((i, j)) == PALM {
                stages[usize::from(world.island.parts.at((i, j)) % 10)] += 1;
            }
        }
    }
    stages
}

#[test]
fn palms_grow() {
    if !orig().join("SZENARIO").exists() {
        eprintln!("skipping: {} not found", orig().display());
        return;
    }
    let tiles = tiles();
    let palette = bitmap("GFXGROUN.BMP").palette;
    let file = SCENARIO_FILES[0];
    let map = SaveGame::parse(&read(&format!("SZENARIO/{file}"))).unwrap();
    let settings = Settings {
        scenario: 1,
        ..Settings::default()
    };
    let mut world = load_world(&map, &settings, true).unwrap();
    world.tables = Arc::new(Tables {
        hausnr: hausnr(&read("DATA/HAUSNR.DAT")).unwrap(),
        hausdata: hausdata(&read("DATA/HAUSDATA.DAT")).unwrap(),
        hausprei: hausprei(&read("DATA/HAUSPREI.DAT")).unwrap(),
        groundnr: groundnr(&read("DATA/GROUNDNR.DAT")).unwrap(),
        animdat1: animdat1(&read("DATA/ANIMDAT1.DAT")).unwrap(),
        animdat4: animdat4(&read("DATA/ANIMDAT4.DAT")).unwrap(),
        form_korrektur: darstell(&read("DATA/DARSTELL.DAT")).unwrap().form_korrektur,
        road_cost: modatyp(&read("DATA/MODATYP.DAT")).unwrap().strassenbaukosten,
    });
    world.random = Random::new(0xdead_beef);
    let before = palm_stages(&world);
    let planted_before: i32 = world.plots.iter().map(|p| i32::from(p.palms())).sum();
    let location_before = world.island.location.clone();
    let mut h = main_screen(&map);
    h.paint_map(&world, &tiles);
    let picture_before = h.map.clone();

    // Two years.
    let days = world.days + 730;
    let mut redrawn = 0;
    while world.days < days {
        let tick = world.tick();
        redrawn += tick.redraw.len();
        if h.redraw_squares(&tick.redraw, &world, &tiles) {
            assert_ne!(tick.redraw, []);
        }
    }
    let after = palm_stages(&world);
    assert_ne!(before, after);
    assert!(redrawn > 0);
    // About 60 squares a day, a quarter of them land, 30 % of the empty
    // ones get a palm: thousands in two years.
    let planted: i32 = world.plots.iter().map(|p| i32::from(p.palms())).sum();
    assert!(planted - planted_before > 1000, "{planted_before} -> {planted}");
    assert!(after.iter().sum::<usize>() > before.iter().sum::<usize>());
    assert_ne!(world.island.location, location_before);
    assert_ne!(h.map, picture_before);
    if let Some(out) = std::env::var_os("ISLAND_PPM").map(PathBuf::from) {
        write_ppm(
            &with_suffix(&out, "palms-before"),
            &compose_map(&picture_before, &h, &palette),
            &palette,
        );
        write_ppm(&with_suffix(&out, "palms"), &compose(&h, &palette), &palette);
    }
}

/// The main screen with `map` in place of the island's picture.
fn compose_map(map: &Surface, h: &Hauptschirm, palette: &Palette) -> Surface {
    let mut h = h.clone();
    h.map = map.clone();
    compose(&h, palette)
}

/// The tables the simulation reads from `DATA\`.
fn world_tables() -> Arc<Tables> {
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

#[test]
fn people_on_the_island() {
    if !orig().join("SZENARIO").exists() {
        eprintln!("skipping: {} not found", orig().display());
        return;
    }
    let tiles = tiles();
    let palette = bitmap("GFXGROUN.BMP").palette;
    // Luxus, with its harbours, airports and guests.
    let file = SCENARIO_FILES[3];
    let map = SaveGame::parse(&read(&format!("SZENARIO/{file}"))).unwrap();
    let settings = Settings {
        scenario: 4,
        ..Settings::default()
    };
    let mut world = load_world(&map, &settings, true).unwrap();
    world.tables = world_tables();
    world.random = Random::new(0x5eed);
    let font = Font::new(
        &bitmap("FONT.BMP"),
        &fontdat(&read("DATA/FONTDAT1.DAT")).unwrap(),
        &fontdat(&read("DATA/FONTDAT2.DAT")).unwrap(),
    );
    let sonstige = sonstige(&read("TEXTE/SONSTIGE.TXT")).unwrap();
    let texts = InfoTexts {
        months: monate(&read("TEXTE/MONATE.TXT")).unwrap(),
        land: modatyp(&read("DATA/MODATYP.DAT")).unwrap().land,
        currency_factor: sonstige.waehrungsfaktor,
        currency: sonstige.texts[5].clone(),
    };
    // Over the town in the south west, with its harbour and its guests.
    let mut h = main_screen_at(&map, (90, 185));
    h.paint_map(&world, &tiles);
    let mut sounds = 0;
    for _ in 0..1500 {
        world.set_viewpoint(h.viewpoint());
        let tick = world.tick();
        sounds += tick.sounds.len();
        h.redraw_squares(&tick.redraw, &world, &tiles);
    }
    h.show_people(&world, &tiles, &font, &texts);
    // Someone is in view, drawn over the island.
    let seen: Vec<_> = (0x24..=game::people::LAST)
        .filter_map(|n| world.people.get(n))
        .filter(|p| p.kind() != 0 && game::people::sees(&h.viewpoint(), &world.island, p.pos()))
        .map(game::people::Person::kind)
        .collect();
    assert_ne!(seen, []);
    let mut bare = h.clone();
    bare.paint_map(&world, &tiles);
    assert_ne!(h.map, bare.map, "{seen:?}");
    assert!(sounds > 0);
    if let Some(out) = std::env::var_os("ISLAND_PPM").map(PathBuf::from) {
        write_ppm(&with_suffix(&out, "people"), &compose(&h, &palette), &palette);
        write_ppm(&with_suffix(&out, "people-bare"), &compose(&bare, &palette), &palette);
        // Turned once: the people turn with the island.
        h.key_down(0x2d, ShiftState::default(), &world, &tiles);
        world.set_viewpoint(h.viewpoint());
        world.tick();
        h.paint_map(&world, &tiles);
        h.show_people(&world, &tiles, &font, &texts);
        write_ppm(&with_suffix(&out, "people-view1"), &compose(&h, &palette), &palette);
    }
}

#[test]
fn computers_shape_the_island() {
    if !orig().join("SZENARIO").exists() {
        eprintln!("skipping: {} not found", orig().display());
        return;
    }
    let tiles = tiles();
    let palette = bitmap("GFXGROUN.BMP").palette;
    // "König", seven computer players, for three years.
    let file = SCENARIO_FILES[2];
    let map = SaveGame::parse(&read(&format!("SZENARIO/{file}"))).unwrap();
    let settings = Settings {
        scenario: 3,
        difficulty: 1,
        ..Settings::default()
    };
    let mut world = load_world(&map, &settings, true).unwrap();
    world.tables = world_tables();
    world.random = Random::new(0x1234_5678);
    world.clock.paused = false;
    let before = world.clone();
    let year = world.date.year;
    while world.date.year < year + 3 {
        world.tick();
    }
    // Where the computers built the most roads.
    let squares = || (0..251).flat_map(|i| (0..251).map(move |j| (i, j)));
    let road = |w: &World, pos| (10..=0x3a).contains(&w.island.ground.at(pos)) && w.island.owners.at(pos) > 1;
    let new_roads: Vec<_> = squares().filter(|&p| road(&world, p) && !road(&before, p)).collect();
    let moved = squares()
        .filter(|&p| world.island.heights.at(p) != before.island.heights.at(p))
        .count();
    assert!(new_roads.len() >= 20, "{}", new_roads.len());
    assert!(moved >= 5, "{moved}");
    // Every new road square has the piece its neighbours call for, or a
    // ramp's, a crossing's or a bridge's.
    for &pos in &new_roads {
        let form = world.island.forms.at(pos);
        assert!(form == 16 || form <= 3, "{pos:?} has form {form}");
    }
    let busiest = *new_roads
        .iter()
        .max_by_key(|&&(i, j)| {
            new_roads
                .iter()
                .filter(|&&(a, b)| (a - i).abs() < 12 && (b - j).abs() < 12)
                .count()
        })
        .unwrap();
    eprintln!(
        "{} new road squares, {moved} corners moved; most around {busiest:?}",
        new_roads.len()
    );
    if let Some(out) = std::env::var_os("ISLAND_PPM").map(PathBuf::from) {
        let origin: (i16, i16) = std::env::var("ISLAND_ORIGIN")
            .ok()
            .and_then(|s| {
                let (a, b) = s.split_once(',')?;
                Some((a.parse().ok()?, b.parse().ok()?))
            })
            // Puts the busiest area in the middle of the screen (view 0).
            .unwrap_or((busiest.0 - 18, busiest.1 + 4));
        for (w, name) in [(&before, "computers-before"), (&world, "computers")] {
            let mut h = main_screen_at(&map, origin);
            h.paint_map(w, &tiles);
            write_ppm(&with_suffix(&out, name), &compose(&h, &palette), &palette);
        }
    }
}
