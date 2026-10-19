//! Sabotage on a scenario's island after a few months: the sabotage
//! tool's command against a computer's building, what it costs, how it
//! takes a slot and has its effects, the refusals, and the tool and
//! `TSabowin` in the view. Skipped when `orig/` isn't there. With
//! `SABOWIN_PPM=path` it writes the main screen with the sabotage window
//! open and the tool's pictures over the island to `path`, and the window
//! alone to `path` with `-form` before the extension.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use formats::bmp::Bitmap;
use formats::data::{
    animdat1, animdat4, auswahl, baumenu_dat, baumenu_txt, darstell, fehler, flusspos, fontdat, gfxpos, groundnr,
    grounpos, hausdata, hausname, hausnr, hauspos, hausprei, modatyp, monate, sabotage, sabotext, sonstige, straspos,
};
use formats::dfm::Object;
use formats::ne::{RT_RCDATA, ResId, resources};
use formats::savegame::SaveGame;
use game::auction;
use game::baumenue::{self, Baumenue};
use game::command::{Action, Command};
use game::darstellung::{BITMAP_FILES, Camera, Tiles, square_at};
use game::events::UNSHAKEN;
use game::hauptschirm::{self, Hauptschirm, InfoTexts, LAYOUT};
use game::hintergrund::load_world;
use game::sabotage::{BOMB, GO_SLOW, PIRANHAS, SHARK_DIVER, STINK_BOMB};
use game::sabowin::{self, Sabowin};
use game::simulation::{Status, Tick};
use game::startform::Settings;
use game::text::Font;
use game::tools::{self, Tools};
use game::widgets::ShiftState;
use game::world::{NO_BUILDING, Pos, Tables, World};
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
    // The world's sabotage table is its language's, from `lang/`: the
    // German CD's file, as `orig/` has it.
    let (_, kinds) = sabotage(&read("TEXTE/SABOTAGE.TXT")).unwrap();
    assert_eq!(
        game::sabotage::table(game::lang::Language::De),
        game::sabotage::kinds(&kinds)
    );
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
        language: game::lang::Language::of_land(modatyp(&read("DATA/MODATYP.DAT")).unwrap().land),
        currency_factor: sonstige.waehrungsfaktor,
        currency: sonstige.texts[5].clone(),
    }
}

/// The sabotage window with the CD's texts and pictures.
fn sabowin() -> Sabowin {
    let (labels, kinds) = sabotage(&read("TEXTE/SABOTAGE.TXT")).unwrap();
    let pictures = sabowin::picture_files(&kinds)
        .iter()
        .map(|f| Surface::from_bitmap(&bitmap(f)))
        .collect();
    let texts = sabowin::Texts {
        sabotext: sabotext(&read("TEXTE/SABOTEXT.TXT")),
        labels,
        kinds,
        info: info(),
    };
    Sabowin::new(font(), surfaces(&sabowin::BITMAP_FILES), pictures, texts)
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

/// Paints the sabotage window, if it's shown, into `s` through the VCL
/// form from the DFM: the form at its bounds, `Saboimage` filling it
/// (1028:34a2 sizes both).
fn paint_sabowin(s: &mut Surface, f: &Sabowin, palette: &Palette) {
    let mut form = load_form("TSABOWIN");
    form.components[0].bounds = f.bounds();
    form.components[0].visible = f.visible;
    let c = form.component_mut("Saboimage").unwrap();
    c.bounds = Rect::new(0, 0, sabowin::WIDTH, sabowin::HEIGHT);
    c.visible = true;
    if let Kind::Image { picture } = &mut c.kind {
        *picture = Some(f.image().clone());
    }
    form.paint(s, palette);
}

/// An 800 × 600 screen with the main screen in the middle and the
/// sabotage window over it.
fn compose(h: &Hauptschirm, tools: &Tools, palette: &Palette) -> Surface {
    let (sw, sh) = (800, 600);
    let mut s = Surface::new(800, 600, 0);
    let mut form = load_form("THAUPTSCHIRM");
    let (left, top) = Hauptschirm::position(sw, sh);
    form.components[0].bounds = Rect::new(left, top, hauptschirm::WIDTH, hauptschirm::HEIGHT);
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
    form.paint(&mut s, palette);
    if let Some(f) = &tools.sabowin {
        paint_sabowin(&mut s, f, palette);
    }
    s
}

fn write_ppm(path: &Path, s: &Surface, palette: &Palette) {
    let mut ppm = format!("P6\n{} {}\n255\n", s.width, s.height).into_bytes();
    for &p in &s.pixels {
        let c = palette[usize::from(p)];
        ppm.extend_from_slice(&[c.r, c.g, c.b]);
    }
    std::fs::write(path, ppm).unwrap();
}

fn human(action: Action) -> Command {
    Command { player: 1, action }
}

/// A tick in which every auction is left as soon as it opens.
fn step(world: &mut World) -> Tick {
    let tick = world.tick();
    if world.auction.open {
        world.apply(human(Action::Auction(auction::Action::Leave)));
    }
    tick
}

/// "König" (seven computer players) after about two months.
fn world() -> (World, SaveGame) {
    let settings = Settings {
        scenario: 3,
        difficulty: 1,
        ..Settings::default()
    };
    let map = SaveGame::parse(&read("SZENARIO/KOENIG.HIS")).unwrap();
    let mut world = load_world(&map, &settings, true).unwrap();
    world.tables = world_tables();
    world.random = Random::new(0x5ab0_7a9e);
    world.clock.paused = false;
    for _ in 0..10_000 {
        step(&mut world);
    }
    (world, map)
}

/// The first square of a computer's building (not a palm or beach life)
/// whose type `ok` accepts.
fn computer_building(w: &World, ok: impl Fn(u8) -> bool) -> Pos {
    let squares = (0..=250).flat_map(|i| (0..=250).map(move |j| (i, j)));
    squares
        .filter(|&p| (2..=8).contains(&w.island.owners.at(p)))
        .find(|&p| {
            let b = w.island.buildings.at(p);
            !UNSHAKEN.contains(&b) && ok(b) && w.building_origin(p) == p
        })
        .expect("no computer building")
}

/// The command with `seed` as the RNG, on a copy of `w`.
fn sabotage_with(w: &World, seed: u32, kind: u8, at: Pos) -> (World, Tick) {
    let mut w = w.clone();
    w.random = Random::new(seed);
    let tick = w.apply(human(Action::Sabotage { kind, at }));
    (w, tick)
}

/// The two `Random(100)`s of 10a0:1d5e for `seed`, after the message's
/// `Random(3)` would come.
fn draws(seed: u32) -> (u16, u16) {
    let mut r = Random::new(seed);
    (r.random(100), r.random(100))
}

fn errors(tick: &Tick) -> Vec<Status> {
    tick.status.iter().filter(|s| s.0 == 1).map(|s| s.1).collect()
}

#[test]
#[allow(clippy::too_many_lines)] // One scripted game.
fn sabotage_on_a_computers_building() {
    if !orig().join("SZENARIO").exists() {
        eprintln!("skipping: {} not found", orig().display());
        return;
    }
    let (mut w, _) = world();
    w.finances.money[0] = 1_000_000;
    let cost = w.sabotage_kind(STINK_BOMB).cost;
    assert_eq!(cost, 10_000);
    let target = computer_building(&w, |_| true);
    let victim = w.island.owners.at(target);
    let free_slot = w.sabotage.iter().position(|s| !s.active).unwrap();

    // It works 70 % of the time: a seed whose first draw is 30 or more.
    let seed = (1..10_000).find(|&s| draws(s).0 >= 30).unwrap();
    let (mut worked, tick) = sabotage_with(&w, seed, STINK_BOMB, target);
    assert_eq!(errors(&tick), []);
    assert_eq!(worked.finances.money[0], 1_000_000 - cost);
    assert!(tick.books);
    let slot = worked.sabotage[free_slot];
    assert!(slot.active);
    assert_eq!(
        (slot.attacker, slot.victim, slot.kind, slot.pos, slot.countdown),
        (1, victim, STINK_BOMB, target, 90)
    );
    // The news of it (message 0x2f) with the attacker and the victim.
    let msg = tick.messages.iter().find(|m| m.number == 0x2f).expect("no message");
    assert_eq!((msg.a, msg.b, msg.pos), (1, i16::from(victim), Some(target)));
    // The next day effects_tick makes the place worse: 60 % in the middle.
    let cell = (target.0 / 2, target.1 / 2);
    let before = i32::from(worked.island.location.get(cell).unwrap());
    // The day's second tick (1020:1e9b).
    let mut ticks = 0;
    while worked.sabotage[free_slot].countdown == 90 {
        step(&mut worked);
        ticks += 1;
        assert!(ticks <= 6 * usize::try_from(worked.clock.speed).unwrap() + 2);
    }
    assert_eq!(worked.sabotage[free_slot].countdown, 89);
    let after = i32::from(worked.island.location.get(cell).unwrap());
    assert!(after <= before * 60 / 100 + 1, "{before} -> {after}");

    // Found out: no slot, the reputation 50 down, message 0x3e.
    let seed = (1..10_000).find(|&s| draws(s).0 < 30 && draws(s).1 >= 50).unwrap();
    let (caught, tick) = sabotage_with(&w, seed, STINK_BOMB, target);
    assert_eq!(caught.finances.money[0], 1_000_000 - cost);
    assert_eq!(caught.sabotage, w.sabotage);
    assert_eq!(caught.reputation(1), w.reputation(1).saturating_sub(50));
    assert!(tick.messages.iter().any(|m| m.number == 0x3e));
    // Failed quietly: paid for, message 0x1c.
    let seed = (1..10_000).find(|&s| draws(s).0 < 30 && draws(s).1 < 50).unwrap();
    let (quiet, tick) = sabotage_with(&w, seed, STINK_BOMB, target);
    assert_eq!(
        (quiet.finances.money[0], quiet.reputation(1)),
        (1_000_000 - cost, w.reputation(1))
    );
    assert_eq!(quiet.sabotage, w.sabotage);
    assert_eq!(tick.messages.iter().map(|m| m.number).collect::<Vec<_>>(), [0x1c]);

    // A bomb that works blows the building up at once.
    let seed = (1..10_000).find(|&s| draws(s).0 >= 30).unwrap();
    let (bombed, _) = sabotage_with(&w, seed, BOMB, target);
    assert_eq!(bombed.island.buildings.at(target), NO_BUILDING);
    assert_eq!(bombed.finances.money[0], 1_000_000 - w.sabotage_kind(BOMB).cost);

    // The refusals: nothing paid, nothing drawn.
    let refused = |w: &World, kind, at, error: u8| {
        let (after, tick) = sabotage_with(w, 7, kind, at);
        assert_eq!(errors(&tick), [Status::Error(error)], "{kind} at {at:?}");
        assert_eq!(after.finances.money, w.finances.money);
        assert_eq!(after.random, Random::new(7));
    };
    refused(&w, STINK_BOMB, (251, 3), 0x16);
    let mut own = w.clone();
    own.island.owners.set(target, 1);
    refused(&own, STINK_BOMB, target, 0x19);
    let mut poor = w.clone();
    poor.finances.money[0] = cost - 1;
    refused(&poor, STINK_BOMB, target, 0xe);
    // A go-slow needs a building site, a shark diver the sea.
    refused(&w, GO_SLOW, target, 0x1a);
    refused(&w, SHARK_DIVER, target, 0x1a);
    let land = (0..=250)
        .flat_map(|i| (0..=250).map(move |j| (i, j)))
        .find(|&p| w.island.buildings.at(p) == NO_BUILDING && w.island.ground.at(p) == 1)
        .unwrap();
    refused(&w, STINK_BOMB, land, 0x1a);
    // Piranhas in what isn't a pool: no error, nothing at all (the bug).
    let pools = w.sabotage_kind(PIRANHAS).targets;
    let not_pool = computer_building(&w, |b| !pools.contains(&i16::from(b)));
    let (after, tick) = sabotage_with(&w, 7, PIRANHAS, not_pool);
    assert_eq!((errors(&tick), after.finances.money), (vec![], w.finances.money));
    // The second square of a building: the slot keeps the first.
    if w.tables.hausnr(w.island.buildings.at(target), 3) > 1 {
        let seed = (1..10_000).find(|&s| draws(s).0 >= 30).unwrap();
        let (second, _) = sabotage_with(&w, seed, STINK_BOMB, (target.0 + 1, target.1));
        assert_eq!(second.sabotage[free_slot].pos, target);
    }
}

/// The squares the main screen shows, each with a point of `Hauptbild`
/// the tools pick it at.
fn visible_squares(world: &World, tiles: &Tiles, cam: &Camera) -> Vec<(Pos, (i32, i32))> {
    let mut out: Vec<(Pos, (i32, i32))> = Vec::new();
    for y in (40..380).step_by(8) {
        for x in (40..580).step_by(8) {
            let pos = square_at(world, tiles, cam, x, y);
            if !out.iter().any(|(p, _)| *p == pos) {
                out.push((pos, (x, y)));
            }
        }
    }
    out
}

#[test]
#[allow(clippy::too_many_lines)] // One scripted game.
fn the_sabotage_tool_and_window() {
    if !orig().join("SZENARIO").exists() {
        eprintln!("skipping: {} not found", orig().display());
        return;
    }
    let (mut world, map) = world();
    world.finances.money[0] = 1_000_000;
    let font = font();
    let info = info();
    let byte = |dest| map.field(dest).unwrap()[0];
    let camera = Camera {
        view: byte("1110:007c"),
        origin: (0, 0),
        parity: 0,
        owner_pointers: byte("1110:0058") != 0,
        occupancy_pointers: byte("1110:0059") != 0,
    };
    let tiles = tiles();
    let title = &sonstige(&read("TEXTE/SONSTIGE.TXT")).unwrap().texts[0];
    // The view on a computer's building.
    let target = computer_building(&world, |b| world.tables.hausnr(b, 3) >= 2);
    let origin = (target.0 - 10, target.1 - 10);
    let mut screen = Hauptschirm::new(camera, origin, surfaces(&hauptschirm::BITMAP_FILES), &font, title);
    screen.paint_map(&world, &tiles);
    screen.show_info(&world, &font, &info);
    let menu = Baumenue::new(
        baumenu_txt(&read("TEXTE/BAUMENU.TXT")).unwrap(),
        &baumenu_dat(&read("DATA/BAUMENU.DAT")).unwrap(),
        hausname(&read("TEXTE/HAUSNAME.TXT")).unwrap(),
        auswahl(&read("TEXTE/AUSWAHL.TXT")),
        surfaces(&baumenue::BITMAP_FILES),
        (800, 600),
    );
    let mut tools = Tools::new(menu, fehler(&read("TEXTE/FEHLER.TXT")).unwrap());
    tools.sabowin = Some(sabowin());
    let view = screen.camera().view;
    let palette = bitmap("GFXGROUN.BMP").palette;

    // Button 21: the tool, and the window shown (sound 0xd) with the keys.
    assert_eq!(
        tools.click(tools::SABOTAGE, &world, &font, &tiles, &info, view, (800, 600)),
        []
    );
    assert_eq!(tools.tool, tools::SABOTAGE);
    assert!(tools.down[usize::from(tools::SABOTAGE)]);
    assert!(tools.sabowin.as_ref().unwrap().visible && tools.sabowin_focused);
    assert!(tools.sounds.contains(&sabowin::SOUND_SHOW));
    // The bomb, two rows down.
    let _ = tools.sabowin_key_down(0x28, ShiftState::default());
    assert_eq!(tools.sabotage_kind(), BOMB);
    // The tool's pictures: the building in the colour that fits, and the
    // sabotage pointer.
    let cam = screen.camera();
    let (_, mouse) = visible_squares(&world, &tiles, &cam)
        .into_iter()
        .find(|&(p, _)| world.building_origin(p) == target)
        .expect("the building isn't in view");
    tools.preview(&world, &tiles, &cam, mouse, true);
    let pointer = tools.sprites.last().unwrap();
    assert!(pointer.pointer && (0x44..0x4c).contains(&pointer.sprite));
    let squares = &tools.sprites[..tools.sprites.len() - 1];
    assert!(
        !squares.is_empty() && squares.iter().all(|s| s.sprite < 0x17c),
        "{squares:?}"
    );
    screen.show_people(&world, &tiles, &font, &info);
    tools.draw_preview(&mut screen.map, &world, &tiles, &cam);
    screen.draw_buttons(&tools.down);
    if let Some(out) = std::env::var_os("SABOWIN_PPM").map(PathBuf::from) {
        write_ppm(&out, &compose(&screen, &tools, &palette), &palette);
        let mut form = Surface::new(sabowin::WIDTH as usize, sabowin::HEIGHT as usize, 0);
        let mut f = tools.sabowin.clone().unwrap();
        (f.left, f.top) = (0, 0);
        paint_sabowin(&mut form, &f, &palette);
        let mut name = out.file_stem().unwrap().to_owned();
        name.push("-form.ppm");
        write_ppm(&out.with_file_name(name), &form, &palette);
    }

    // "Sabotieren": the window goes, the tool stays.
    let _ = tools.sabowin_mouse_down(330, 195, true);
    let r = tools.sabowin_mouse_up(330, 195, true);
    assert_eq!(r.close, Some(sabowin::Close::Hide));
    assert!(!tools.sabowin.as_ref().unwrap().visible && !tools.sabowin_focused);
    assert_eq!(tools.tool, tools::SABOTAGE);
    // The click on the island is the command.
    let actions = tools.press(mouse.0, mouse.1, ShiftState::LEFT, &world, &tiles, &cam);
    let at = square_at(&world, &tiles, &cam, mouse.0, mouse.1);
    assert_eq!(actions, [Action::Sabotage { kind: BOMB, at }]);
    let money = world.finances.money[0];
    let _ = world.apply(human(actions[0].clone()));
    assert_eq!(world.finances.money[0], money - world.sabotage_kind(BOMB).cost);

    // The button again: the pointer.
    let _ = tools.click(tools::SABOTAGE, &world, &font, &tiles, &info, view, (800, 600));
    assert_eq!(tools.tool, tools::POINTER);
    // Shown again, "Schliessen" gives the pointer too, and another tool
    // hides it.
    let _ = tools.click(tools::SABOTAGE, &world, &font, &tiles, &info, view, (800, 600));
    assert!(tools.sabowin.as_ref().unwrap().visible);
    assert_eq!(tools.sabotage_kind(), BOMB, "the kind stays");
    let _ = tools.sabowin_mouse_down(250, 200, true);
    let _ = tools.sabowin_mouse_up(250, 200, true);
    assert_eq!(tools.tool, tools::POINTER);
    assert!(tools.down[usize::from(tools::POINTER)] && !tools.down[usize::from(tools::SABOTAGE)]);
    let _ = tools.click(tools::SABOTAGE, &world, &font, &tiles, &info, view, (800, 600));
    let _ = tools.click(tools::DEMOLISH, &world, &font, &tiles, &info, view, (800, 600));
    assert!(!tools.sabowin.as_ref().unwrap().visible);
    assert!(tools.sounds.ends_with(&[9, sabowin::SOUND_HIDE]) || tools.sounds.contains(&sabowin::SOUND_HIDE));
}
