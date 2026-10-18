//! Generates islands from the start menu's settings with the CD's data
//! (10b0:52d4) and plays them for a while. Skipped when `orig/` isn't
//! there. With `ISLAND_PPM=path` it writes the main screen of a few of
//! them next to `path` (`path` with `-atoll`, `-group`, ... before the
//! extension).

use std::path::{Path, PathBuf};

use formats::bmp::Bitmap;
use formats::data::{darstell, flusspos, fontdat, gfxpos, groundnr, grounpos, hausnr, hauspos, sonstige, straspos};
use formats::dfm::Object;
use formats::ne::{RT_RCDATA, ResId, resources};
use game::auction;
use game::command::{self, Command};
use game::darstellung::{BITMAP_FILES, Camera, Tiles};
use game::enhance::Fixes;
use game::hauptschirm::{self, Hauptschirm, LAYOUT, start_viewpoint};
use game::hintergrund::{GameData, Island, new_island, new_world};
use game::startform::Settings;
use game::text::Font;
use game::world::{PALM, World};
use vcl::form::{Form, Kind};
use vcl::surface::Surface;
use vcl::{Palette, Rect};

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
    Some(GameData::load(read, game::lang::Language::De).unwrap())
}

fn settings(island: i16, relief: i16, land: i16, rivers: i16) -> Settings {
    Settings {
        island,
        relief,
        land,
        rivers,
        ..Settings::default()
    }
}

/// How many corners are above the sea, and the highest.
fn land(w: &World) -> (usize, u8) {
    let mut n = 0;
    let mut top = 0;
    for i in 0..251 {
        for j in 0..251 {
            let h = w.island.heights.at((i, j));
            n += usize::from(h > 0);
            top = top.max(h);
        }
    }
    (n, top)
}

/// The island is whole: corners at most a step from their neighbours
/// (but where a river levelled it), none near the edge, forms and grounds
/// that fit, plots on all land.
fn check(w: &World, what: &str) {
    for i in 0..251i16 {
        for j in 0..251i16 {
            let h = w.island.heights.at((i, j));
            assert!(h <= 20, "{what}: {h} at {i},{j}");
            if !(2..=248).contains(&i) || !(2..=248).contains(&j) {
                assert_eq!(h, 0, "{what}: land at the edge {i},{j}");
            }
            let ground = w.island.ground.at((i, j));
            assert!(ground < 10 || (0x37..0x5f).contains(&ground), "{what}: ground {ground}");
            if h > 0 {
                assert_ne!(w.island.plot_at((i, j)), 0, "{what}: no plot at {i},{j}");
            }
            if w.island.buildings.at((i, j)) == PALM {
                assert!(matches!(ground, 0 | 1 | 4), "{what}: palm on {ground}");
            }
        }
    }
    let plots = w.plots.len();
    assert!(
        w.plots
            .iter()
            .all(|p| p.id() >= 1 && usize::from(p.id().cast_unsigned()) <= plots)
    );
    let area: u32 = w.plots.iter().map(|p| u32::from(p.area())).sum();
    assert_eq!(usize::try_from(area).unwrap(), land(w).0, "{what}");
}

/// The fixes are in place before the island is made: those of the
/// generator change it.
#[test]
fn made_with_the_fixes() {
    let Some(data) = game_data() else { return };
    // `ATVariation1` and 2 are 30 and 34 here: `fix.atoll_hills` changes
    // the hills.
    let atoll = settings(0, 4, 4, 5);
    let original = new_world(&atoll, &data, 0x1234_5678, Fixes::ORIGINAL);
    let fixed = new_world(&atoll, &data, 0x1234_5678, Fixes::all_on());
    assert_eq!(original.fixes, Fixes::ORIGINAL);
    assert_eq!(fixed.fixes, Fixes::all_on());
    assert_ne!(original.island, fixed.island);
    check(&fixed, "fixed");
}

#[test]
fn same_seed_same_island() {
    let Some(data) = game_data() else { return };
    let menu = Settings::default();
    assert_eq!(new_island(&menu, false), Island::Generated);
    let first = new_world(&menu, &data, 0x1234_5678, Fixes::ORIGINAL);
    let again = new_world(&menu, &data, 0x1234_5678, Fixes::ORIGINAL);
    assert_eq!(first, again);
    let other = new_world(&menu, &data, 0x1234_5679, Fixes::ORIGINAL);
    assert_ne!(first.island, other.island);
    check(&first, "default");
    let (n, top) = land(&first);
    assert!(n > 5000, "{n} corners of land");
    assert!(top >= 4, "highest {top}");
    assert_eq!(first.viewpoint, start_viewpoint());
    assert_eq!(first.player_names[1], menu.name);
    assert!(first.player_names[2..].iter().all(|n| !n.is_empty()));
    assert_eq!(first.finances.money[0], first.finances.start_money);
}

/// Every shape, relief and amount of land, with no, some and many
/// rivers, and every number of rivers once.
#[test]
fn every_setting_generates() {
    let Some(data) = game_data() else { return };
    let mut seed = 1u32;
    for island in 0..=2 {
        for relief in 0..=4 {
            for land_amount in 0..=4 {
                for rivers in [0, 3, 10] {
                    seed = seed.wrapping_mul(0x0808_8405).wrapping_add(1);
                    let what = format!("shape {island} relief {relief} land {land_amount} rivers {rivers}");
                    let w = new_world(
                        &settings(island, relief, land_amount, rivers),
                        &data,
                        seed,
                        Fixes::ORIGINAL,
                    );
                    check(&w, &what);
                    assert!(land(&w).0 > 0, "{what}: no land");
                }
            }
        }
    }
    for rivers in 0..=10 {
        let w = new_world(&settings(2, 4, 4, rivers), &data, 77, Fixes::ORIGINAL);
        check(&w, &format!("rivers {rivers}"));
    }
}

/// All 825 combinations of the settings, each with two seeds. Slow
/// without optimisations: `cargo test --release -- --ignored`.
#[test]
#[ignore = "slow; run with --release"]
fn every_setting_combination_generates() {
    let Some(data) = game_data() else { return };
    let mut seed = 7u32;
    for island in 0..=2 {
        for relief in 0..=4 {
            for land_amount in 0..=4 {
                for rivers in 0..=10 {
                    for _ in 0..2 {
                        seed = seed.wrapping_mul(0x0808_8405).wrapping_add(1);
                        let what =
                            format!("shape {island} relief {relief} land {land_amount} rivers {rivers} seed {seed}");
                        let w = new_world(
                            &settings(island, relief, land_amount, rivers),
                            &data,
                            seed,
                            Fixes::ORIGINAL,
                        );
                        check(&w, &what);
                    }
                }
            }
        }
    }
}

#[test]
fn rivers_reach_the_sea() {
    let Some(data) = game_data() else { return };
    let mut squares = 0;
    for seed in 0..8 {
        let w = new_world(&settings(2, 4, 3, 10), &data, seed, Fixes::ORIGINAL);
        let river = |pos| w.island.ground.get(pos).is_some_and(|g| g >= 0x37);
        for i in 1..250 {
            for j in 1..250 {
                if river((i, j)) {
                    squares += 1;
                    assert_eq!(w.island.buildings.at((i, j)), game::world::NO_BUILDING);
                }
            }
        }
    }
    assert!(squares > 50, "{squares} river squares");
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

#[test]
fn plays_a_generated_island() {
    let Some(data) = game_data() else { return };
    for (island, seed) in [(0, 11), (1, 12), (2, 13)] {
        let mut w = new_world(&settings(island, 2, 2, 5), &data, seed, Fixes::ORIGINAL);
        let start = w.clone();
        let year = w.date.year;
        while w.date.year < year + 2 {
            step(&mut w);
        }
        assert!(w.days > 700);
        assert_ne!(w.island, start.island, "palms grow, the computers build");
        // The computers spent money and built something.
        let built = (0..251)
            .flat_map(|i| (0..251).map(move |j| (i, j)))
            .filter(|&p| w.island.owners.at(p) > 1)
            .count();
        assert!(built > 0, "shape {island}: the computers built nothing");
    }
}

// Drawing, as in orig_island.rs.

fn bitmap(name: &str) -> Bitmap {
    Bitmap::parse(&read(&format!("GRAFIK/{name}")).unwrap()).unwrap()
}

fn tiles() -> Tiles {
    let data = |n: &str| read(&format!("DATA/{n}")).unwrap();
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

fn main_screen(view: u8) -> Hauptschirm {
    let camera = Camera {
        view,
        origin: (0, 0),
        parity: 0,
        owner_pointers: false,
        occupancy_pointers: true,
    };
    let font = Font::new(
        &bitmap("FONT.BMP"),
        &fontdat(&read("DATA/FONTDAT1.DAT").unwrap()).unwrap(),
        &fontdat(&read("DATA/FONTDAT2.DAT").unwrap()).unwrap(),
    );
    let title = &sonstige(&read("TEXTE/SONSTIGE.TXT").unwrap()).unwrap().texts[0];
    let bitmaps = hauptschirm::BITMAP_FILES
        .iter()
        .map(|f| Surface::from_bitmap(&bitmap(f)))
        .collect();
    Hauptschirm::new_game(camera, bitmaps, &font, title)
}

fn compose(h: &Hauptschirm, palette: &Palette) -> Surface {
    let exe = read("HOLIDAY.EXE").unwrap();
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

/// A map of the whole island, a pixel per corner: the sea blue, land
/// greener the higher, rivers light blue, palms dark green.
fn write_overview(path: &Path, w: &World) {
    let mut ppm = b"P6\n251 251\n255\n".to_vec();
    for i in 0..251 {
        for j in 0..251 {
            let h = w.island.heights.at((i, j));
            let ground = w.island.ground.at((i, j));
            let c = if ground >= 0x37 {
                [90, 170, 255]
            } else if w.island.buildings.at((i, j)) == PALM {
                [0, 90, 0]
            } else if h == 0 {
                [0, 40, 140]
            } else {
                let g = 120 + h * 6;
                [g / 2, g, 40]
            };
            ppm.extend_from_slice(&c);
        }
    }
    std::fs::write(path, ppm).unwrap();
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_stem().unwrap().to_owned();
    name.push(format!("-{suffix}.ppm"));
    path.with_file_name(name)
}

#[test]
fn draws_generated_islands() {
    let Some(data) = game_data() else { return };
    let tiles = tiles();
    let palette = bitmap("GFXGROUN.BMP").palette;
    let out = std::env::var_os("ISLAND_PPM").map(PathBuf::from);
    for (name, s, seed) in [
        ("atoll", settings(0, 2, 2, 5), 101),
        ("group", settings(1, 2, 2, 5), 102),
        ("one", settings(2, 2, 2, 5), 103),
        ("one-flat", settings(2, 0, 4, 2), 104),
        ("one-hilly", settings(2, 4, 4, 10), 105),
        ("group-hilly", settings(1, 4, 0, 10), 106),
    ] {
        let w = new_world(&s, &data, seed, Fixes::ORIGINAL);
        let mut h = main_screen(0);
        assert_eq!(h.viewpoint(), w.viewpoint);
        h.paint_map(&w, &tiles);
        let full = h.map.width * h.map.height;
        let drawn = h.map.pixels.iter().filter(|&&p| p != 0).count();
        assert!(drawn > full * 9 / 10, "{name}: {drawn} of {full} drawn");
        if let Some(out) = &out {
            write_ppm(&with_suffix(out, name), &compose(&h, &palette), &palette);
            write_overview(&with_suffix(out, &format!("{name}-map")), &w);
        }
    }
}
