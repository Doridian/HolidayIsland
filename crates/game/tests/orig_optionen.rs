//! The options form and the pause on a scenario's island: the options'
//! world commands (the speed, the animations, "Auktion wartet",
//! disasters) applied to the world between months of play, and the forms
//! painted from the original's files. Skipped when `orig/` isn't there.
//! With `OPTIONEN_PPM=path` it writes the main screen with the options
//! form open to `path`, with `PAUSEFORM_PPM=path` the one with the pause.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use formats::bmp::Bitmap;
use formats::data::{
    animdat1, animdat4, darstell, flusspos, fontdat, gfxpos, groundnr, grounpos, hausdata, hausnr, hauspos, hausprei,
    modatyp, monate, sonstige, straspos, text_lines,
};
use formats::dfm::Object;
use formats::ne::{RT_RCDATA, ResId, resources};
use formats::savegame::SaveGame;
use game::auction;
use game::command::{Action, Command};
use game::darstellung::{BITMAP_FILES, Camera, Tiles};
use game::hauptschirm::{self, Hauptschirm, InfoTexts, LAYOUT};
use game::hintergrund::{SCENARIO_FILES, load_world};
use game::optionen::{self, Optionenform, Settings, WorldOption};
use game::pauseform::{self, Pauseform};
use game::people::FIRST;
use game::startform;
use game::text::Font;
use game::widgets::ShiftState;
use game::world::{Tables, World};
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

fn surfaces(files: &[&str]) -> Vec<Surface> {
    files.iter().map(|f| Surface::from_bitmap(&bitmap(f))).collect()
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
        sabotage: Vec::new(),
    })
}

fn font() -> Font {
    Font::new(
        &bitmap("FONT.BMP"),
        &fontdat(&read("DATA/FONTDAT1.DAT")).unwrap(),
        &fontdat(&read("DATA/FONTDAT2.DAT")).unwrap(),
    )
}

fn info() -> InfoTexts {
    let sonstige = sonstige(&read("TEXTE/SONSTIGE.TXT")).unwrap();
    InfoTexts {
        months: monate(&read("TEXTE/MONATE.TXT")).unwrap(),
        land: modatyp(&read("DATA/MODATYP.DAT")).unwrap().land,
        currency_factor: sonstige.waehrungsfaktor,
        currency: sonstige.texts[5].clone(),
    }
}

fn load_form(class: &str) -> Form {
    let exe = read("HOLIDAY.EXE");
    let res = resources(&exe)
        .unwrap()
        .into_iter()
        .find(|r| r.kind == ResId::Id(RT_RCDATA) && r.name == ResId::Name(class.as_bytes().to_vec()))
        .unwrap();
    Form::load(&Object::parse(res.data).unwrap()).unwrap()
}

fn write_ppm(path: &Path, s: &Surface, palette: &Palette) {
    let mut ppm = format!("P6\n{} {}\n255\n", s.width, s.height).into_bytes();
    for &p in &s.pixels {
        let c = palette[usize::from(p)];
        ppm.extend_from_slice(&[c.r, c.g, c.b]);
    }
    std::fs::write(path, ppm).unwrap();
}

/// The first scenario's world and the map it came from.
fn scenario() -> (World, SaveGame) {
    let map = SaveGame::parse(&read(&format!("SZENARIO/{}", SCENARIO_FILES[0]))).unwrap();
    let settings = startform::Settings {
        scenario: 1,
        difficulty: 1,
        ..startform::Settings::default()
    };
    let mut world = load_world(&map, &settings, true).unwrap();
    world.tables = world_tables();
    world.random = Random::new(0x0b7e_1996);
    (world, map)
}

/// The human's command.
fn human(action: Action) -> Command {
    Command { player: 1, action }
}

/// `n` ticks, in which every auction is left as soon as it opens.
fn run(world: &mut World, n: usize) {
    for _ in 0..n {
        world.tick();
        if world.auction.open {
            world.apply(human(Action::Auction(auction::Action::Leave)));
        }
    }
}

/// About two months at the default speed.
const TWO_MONTHS: usize = 61 * 6 * 4;

fn options_form() -> Optionenform {
    Optionenform::new(
        font(),
        surfaces(&optionen::BITMAP_FILES),
        text_lines(&read("TEXTE/OPTIONEN.TXT")),
    )
}

/// A click at `x`, `y` in the form, with what the press and the release
/// asked for.
fn click(form: &mut Optionenform, x: i32, y: i32) -> (Vec<Action>, Option<optionen::Action>) {
    let down = form.mouse_down(x, y, ShiftState::LEFT);
    let up = form.mouse_up(x, y);
    let mut commands = down.commands;
    commands.extend(up.commands);
    (commands, down.action.or(up.action))
}

#[test]
fn options_change_the_world() {
    if !orig().join("SZENARIO").exists() {
        eprintln!("skipping: {} not found", orig().display());
        return;
    }
    let (mut world, map) = scenario();
    run(&mut world, TWO_MONTHS);
    let mut form = options_form();
    let settings = Settings::default().with_map(&map);
    form.show(&settings, &world, true, (800, 600));
    assert_eq!(world.clock.speed, 4);

    let mut commands = Vec::new();
    // `+` sixteen times: the slider at its end, one tick an hour.
    for _ in 0..16 {
        commands.extend(form.key_down(0xbb, ShiftState::default()).commands);
    }
    assert_eq!(commands.len(), 16);
    assert_eq!(commands.last(), Some(&Action::Speed(1)));
    // "Animationen", "Katastrophen" and "Auktion wartet" with the mouse.
    let (c, _) = click(&mut form, 0xb3 + 4, 0x10f + 4);
    assert_eq!(c, [Action::SetOption(WorldOption::PeopleMove, false)]);
    commands.extend(c);
    let (c, _) = click(&mut form, 0xb3 + 4, 0x127 + 4);
    assert_eq!(c, [Action::SetOption(WorldOption::Disasters, false)]);
    commands.extend(c);
    let waits = world.options.auction_waits;
    let c = form.key_down(0x79, ShiftState::default()).commands; // F10
    assert_eq!(c, [Action::SetOption(WorldOption::AuctionWaits, !waits)]);
    commands.extend(c);
    // The player's own options give no commands.
    for key in [0x4d, 0x47, 0x70, 0x78, 0x74] {
        assert_eq!(form.key_down(key, ShiftState::default()).commands, []);
    }
    let (_, action) = click(&mut form, 0xb3 + 10, 0x158 + 5);
    assert_eq!(action, Some(optionen::Action::Close));

    let before = world.clone();
    let mut redraw = false;
    for action in &commands {
        redraw |= world.apply(human(action.clone())).redraw_all;
    }
    assert!(redraw);
    assert_eq!(world.clock.speed, 1);
    assert!(!world.options.people_move && !world.disasters && world.played_without_disasters);
    assert_eq!(world.options.auction_waits, !waits);
    let last = world.people.last;
    assert!(last >= FIRST);
    let walkers = (FIRST..=last)
        .filter_map(|n| world.people.get(n))
        .filter(|p| game::people::WALKERS.contains(&p.kind()));
    assert!(walkers.clone().count() > 0, "no walkers on the island");
    assert!(walkers.clone().all(|p| p.flags()[2] == 0));

    // At one tick an hour, a day is six ticks: a month in 186.
    let days = world.days;
    run(&mut world, 31 * 6);
    assert_eq!(world.days - days, 31);
    // The same commands, the same world.
    let mut again = before.clone();
    for action in &commands {
        again.apply(human(action.clone()));
    }
    run(&mut again, 31 * 6);
    assert_eq!(again, world);
    // And the other way: the default speed and the animations back.
    let mut slower = before;
    slower.apply(human(Action::Speed(4)));
    slower.apply(human(Action::SetOption(WorldOption::PeopleMove, true)));
    let days = slower.days;
    run(&mut slower, 31 * 6);
    assert!(slower.options.people_move);
    assert!(slower.days - days < 9);
    run(&mut world, TWO_MONTHS);
}

/// The main screen on an 800 × 600 screen with `buttons` down, and the
/// form `class` over it at `bounds` with its images.
fn compose(screen: &Hauptschirm, class: &str, bounds: Rect, images: &[(&str, Rect, &Surface)]) -> Surface {
    let palette = bitmap("GFXGROUN.BMP").palette;
    let mut s = Surface::new(800, 600, 0);
    let mut form = load_form("THAUPTSCHIRM");
    let (left, top) = Hauptschirm::position(800, 600);
    form.components[0].bounds = Rect::new(left, top, hauptschirm::WIDTH, hauptschirm::HEIGHT);
    form.components[0].visible = true;
    for (name, b) in LAYOUT {
        let c = form.component_mut(name).unwrap();
        c.bounds = b;
        c.visible = true;
    }
    for (name, image) in screen.images() {
        if let Kind::Image { picture } = &mut form.component_mut(name).unwrap().kind {
            *picture = Some(image.clone());
        }
    }
    form.paint(&mut s, &palette);
    let mut f = load_form(class);
    f.components[0].bounds = bounds;
    f.components[0].visible = true;
    for &(name, b, image) in images {
        let c = f.component_mut(name).unwrap();
        c.bounds = b;
        c.visible = true;
        if let Kind::Image { picture } = &mut c.kind {
            *picture = Some(image.clone());
        }
    }
    f.paint(&mut s, &palette);
    s
}

fn main_screen(world: &World, map: &SaveGame, button: usize) -> Hauptschirm {
    let byte = |dest| map.field(dest).unwrap()[0];
    let d = map.field("1110:5530").unwrap();
    let origin = (i16::from_le_bytes([d[0], d[1]]), i16::from_le_bytes([d[2], d[3]]));
    let camera = Camera {
        view: byte("1110:007c"),
        origin: (0, 0),
        parity: 0,
        owner_pointers: byte("1110:0058") != 0,
        occupancy_pointers: byte("1110:0059") != 0,
    };
    let font = font();
    let title = &sonstige(&read("TEXTE/SONSTIGE.TXT")).unwrap().texts[0];
    let mut screen = Hauptschirm::new(camera, origin, surfaces(&hauptschirm::BITMAP_FILES), &font, title);
    let tiles = tiles();
    screen.paint_map(world, &tiles);
    screen.show_info(world, &font, &info());
    screen.show_people(world, &tiles, &font, &info());
    let mut down = [false; 24];
    down[0x16] = true;
    down[button] = true;
    screen.draw_buttons(&down);
    screen
}

#[test]
fn paints_the_forms() {
    if !orig().join("SZENARIO").exists() {
        eprintln!("skipping: {} not found", orig().display());
        return;
    }
    let (mut world, map) = scenario();
    run(&mut world, TWO_MONTHS);
    let palette = bitmap("GFXGROUN.BMP").palette;

    // The options, with the speed one faster and the owners' pointers.
    let mut form = options_form();
    form.show(&Settings::default().with_map(&map), &world, true, (800, 600));
    assert_eq!((form.left, form.top), ((800 - optionen::WIDTH) / 2, 0));
    let bare = Surface::from_bitmap(&bitmap("optionen.bmp"));
    let drawn = (0..form.image().height)
        .flat_map(|y| (0..form.image().width).map(move |x| (x, y)))
        .filter(|&(x, y)| form.image().pixel(x, y) != bare.pixel(x, y))
        .count();
    assert!(drawn > 3000, "{drawn} pixels of widgets");
    let r = form.key_down(0xbb, ShiftState::default());
    for action in r.commands {
        world.apply(human(action));
    }
    assert_eq!(world.clock.speed, 3);
    let r = form.key_down(0x74, ShiftState::default()); // F5
    assert_eq!(r.changes, [optionen::Change::Pointers]);
    let mut screen = main_screen(&world, &map, 2);
    screen.camera.owner_pointers = form.settings().owner_pointers;
    screen.camera.occupancy_pointers = form.settings().occupancy_pointers;
    let tiles = tiles();
    screen.paint_map(&world, &tiles);
    screen.show_people(&world, &tiles, &font(), &info());
    let images: Vec<_> = optionen::LAYOUT
        .iter()
        .zip(form.images())
        .map(|((name, b), (_, image))| (*name, *b, image))
        .collect();
    let out = compose(&screen, "TOPTIONENFORM", form.bounds(), &images);
    if let Some(path) = std::env::var_os("OPTIONEN_PPM") {
        write_ppm(Path::new(&path), &out, &palette);
    }

    // The pause.
    let mut pause = Pauseform::new(surfaces(&pauseform::BITMAP_FILES));
    pause.show((800, 600));
    assert_eq!(pause.bounds(), Rect::new(225, 195, pauseform::WIDTH, pauseform::HEIGHT));
    assert_eq!(pause.key_down(0x20), pauseform::Action::Close);
    let screen = main_screen(&world, &map, 1);
    let images: Vec<_> = pauseform::LAYOUT
        .iter()
        .zip(pause.images())
        .map(|((name, b), (_, image))| (*name, *b, image))
        .collect();
    let out = compose(&screen, "TPAUSEFORM", pause.bounds(), &images);
    if let Some(path) = std::env::var_os("PAUSEFORM_PPM") {
        write_ppm(Path::new(&path), &out, &palette);
    }
}
