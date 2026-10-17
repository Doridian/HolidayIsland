//! Starts a game on the tutorial island and each scenario, and draws the
//! main screen. Skipped when `orig/` isn't there. With `ISLAND_PPM=path` it
//! writes the tutorial's main screen to `path`, and each scenario's next
//! to it (`path` with `-geld` and so on before the extension), and the
//! first scenario after two years of palm growth (`-palms`).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use formats::bmp::Bitmap;
use formats::data::{
    darstell, flusspos, fontdat, gfxpos, groundnr, grounpos, hausdata, hausnr, hauspos, hausprei, sonstige, straspos,
};
use formats::dfm::Object;
use formats::ne::{RT_RCDATA, ResId, resources};
use formats::savegame::SaveGame;
use game::darstellung::{BITMAP_FILES, Camera, Tiles};
use game::hauptschirm::{self, Hauptschirm, KeyResult, LAYOUT};
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
    let byte = |dest| map.field(dest).unwrap()[0];
    let word = |dest, at: usize| {
        let d = map.field(dest).unwrap();
        i16::from_le_bytes([d[at], d[at + 1]])
    };
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
    Hauptschirm::new(
        camera,
        (word("1110:5530", 0), word("1110:5530", 2)),
        bitmaps,
        &font,
        title,
    )
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
