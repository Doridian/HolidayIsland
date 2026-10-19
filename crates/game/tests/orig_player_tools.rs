//! The human's tools on a scenario's island: the build menu, the
//! placement preview, the road drag, building, levelling and
//! demolishing as commands, their costs and errors. Skipped when `orig/`
//! isn't there. With `PLAYER_TOOLS_PPM=path` it writes the main screen
//! with the build menu open to `path`, and next to it (`path` with a
//! suffix before the extension) the placement preview (`-preview`), a
//! road being dragged (`-road`) and the island after the commands and
//! some ticks (`-after`).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use formats::bmp::Bitmap;
use formats::data::{
    animdat1, animdat4, auswahl, baumenu_dat, baumenu_txt, darstell, fehler, flusspos, fontdat, gfxpos, groundnr,
    grounpos, hausdata, hausname, hausnr, hauspos, hausprei, modatyp, monate, sonstige, straspos,
};
use formats::dfm::Object;
use formats::ne::{RT_RCDATA, ResId, resources};
use formats::savegame::SaveGame;
use game::auction;
use game::baumenue::{self, Baumenue};
use game::command::{Action, Command};
use game::darstellung::{BITMAP_FILES, Camera, Tiles, square_at};
use game::enhance::Enhancement;
use game::hauptschirm::{self, Hauptschirm, InfoTexts, LAYOUT};
use game::hintergrund::{SCENARIO_FILES, load_world};
use game::savegame::Fields;
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

fn load_form(class: &str) -> Form {
    let exe = read("HOLIDAY.EXE");
    let res = resources(&exe)
        .unwrap()
        .into_iter()
        .find(|r| r.kind == ResId::Id(RT_RCDATA) && r.name == ResId::Name(class.as_bytes().to_vec()))
        .unwrap();
    Form::load(&Object::parse(res.data).unwrap()).unwrap()
}

/// An 800 × 600 screen with the main screen in the middle and the build
/// menu, if it's shown, where it is.
fn compose(h: &Hauptschirm, menu: &Baumenue, palette: &Palette) -> Surface {
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
    let mut m = load_form("TBAUMENUE");
    m.components[0].bounds = menu.bounds();
    m.components[0].visible = menu.height > 0;
    let c = m.component_mut("Baumenuebild").unwrap();
    c.bounds = Rect::new(0, 0, menu.width, menu.full_height());
    c.visible = true;
    if let Kind::Image { picture } = &mut c.kind {
        *picture = Some(menu.image.clone());
    }
    m.paint(&mut s, palette);
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

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_stem().unwrap().to_owned();
    name.push(format!("-{suffix}.ppm"));
    path.with_file_name(name)
}

/// The human's command.
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

/// The squares the main screen shows, each with a point of `Hauptbild`
/// the tools pick it at, scanning the picture every 8 pixels.
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

struct Scene {
    world: World,
    tiles: Tiles,
    screen: Hauptschirm,
    tools: Tools,
    font: Font,
    info: InfoTexts,
    palette: Palette,
}

impl Scene {
    fn new() -> Scene {
        let file = SCENARIO_FILES[0];
        let map = SaveGame::parse(&read(&format!("SZENARIO/{file}"))).unwrap();
        let settings = Settings {
            scenario: 1,
            difficulty: 1,
            ..Settings::default()
        };
        let mut world = load_world(&map, &settings, true).unwrap();
        world.tables = world_tables();
        world.random = Random::new(0x7001_5eed);
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
        let info = info();
        let title = &sonstige(&read("TEXTE/SONSTIGE.TXT")).unwrap().texts[0];
        let mut screen = Hauptschirm::new(camera, origin, surfaces(&hauptschirm::BITMAP_FILES), &font, title);
        let tiles = tiles();
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
        // The map's tool and buttons: the pointer's.
        let fields = game::savegame::MapView::from_map(&map).unported;
        tools.start(&fields, &world, &font, &tiles, &info, screen.camera().view);
        screen.draw_buttons(&tools.down);
        Scene {
            world,
            tiles,
            screen,
            tools,
            font,
            info,
            palette: bitmap("GFXGROUN.BMP").palette,
        }
    }

    fn click(&mut self, b: u8) -> Vec<Action> {
        let view = self.screen.camera().view;
        let actions = self
            .tools
            .click(b, &self.world, &self.font, &self.tiles, &self.info, view, (800, 600));
        self.screen.draw_buttons(&self.tools.down);
        actions
    }

    /// Applies the human's action and shows what it did, as the game does.
    fn apply(&mut self, action: Action) -> Tick {
        let tick = self.world.apply(human(action));
        let view = self.screen.camera().view;
        self.tools.world_changed(
            &tick,
            1,
            &self.world,
            &self.font,
            &self.tiles,
            &self.info,
            view,
            (800, 600),
        );
        if let Some(text) = self.tools.status.take() {
            self.screen.show_status(&text, &self.font);
        }
        self.screen.draw_buttons(&self.tools.down);
        self.screen.redraw_squares(&tick.redraw, &self.world, &self.tiles);
        tick
    }

    /// The island with the people and the tool's pictures for the mouse at
    /// `mouse`.
    fn show_preview(&mut self, mouse: (i32, i32)) {
        let cam = self.screen.camera();
        self.tools.preview(&self.world, &self.tiles, &cam, mouse, true);
        self.screen
            .show_people(&self.world, &self.tiles, &self.font, &self.info);
        self.tools
            .draw_preview(&mut self.screen.map, &self.world, &self.tiles, &cam);
    }

    fn picture(&self) -> Surface {
        compose(&self.screen, &self.tools.menu, &self.palette)
    }
}

/// The first visible square where `ok` holds.
fn find(scene: &Scene, ok: impl Fn(&World, Pos) -> bool) -> (Pos, (i32, i32)) {
    let cam = scene.screen.camera();
    visible_squares(&scene.world, &scene.tiles, &cam)
        .into_iter()
        .find(|&(p, _)| ok(&scene.world, p))
        .expect("no such square in view")
}

#[test]
#[allow(clippy::too_many_lines)] // One scripted game.
fn the_player_builds_levels_and_demolishes() {
    if !orig().join("SZENARIO").exists() {
        eprintln!("skipping: {} not found", orig().display());
        return;
    }
    let mut scene = Scene::new();
    let out = std::env::var_os("PLAYER_TOOLS_PPM").map(PathBuf::from);
    assert_eq!(scene.tools.tool, tools::POINTER);

    // The houses' submenu: the button goes down and the menu opens with
    // the row the map saved as selected.
    assert_eq!(scene.click(9), vec![]);
    assert_eq!(scene.tools.tool, 9);
    assert!(scene.tools.down[9] && !scene.tools.down[usize::from(tools::POINTER)]);
    assert!(scene.tools.menu.open && scene.tools.menu.height > 0);
    let house = u8::try_from(scene.tools.menu.building).unwrap();
    let houses = &scene.world.build_menu.rows[0];
    assert!(houses.iter().any(|row| row.contains(&i16::from(house))), "{house}");
    if let Some(out) = &out {
        write_ppm(out, &scene.picture(), &scene.palette);
    }

    // A site where it fits, and one in the sea where it doesn't.
    let (site, mouse) = find(&scene, |w, p| w.can_build(p, house, 1).0 == 0);
    scene.show_preview(mouse);
    assert_ne!(scene.tools.sprites, vec![]);
    assert!(
        scene.tools.sprites.iter().all(|s| s.sprite < 0x17c),
        "fits: {:?}",
        scene.tools.sprites
    );
    if let Some(out) = &out {
        write_ppm(&with_suffix(out, "preview"), &scene.picture(), &scene.palette);
    }
    let (sea, sea_mouse) = find(&scene, |w, p| {
        w.island.ground.at(p) == 2 && w.island.buildings.at(p) == NO_BUILDING
    });
    scene.show_preview(sea_mouse);
    assert!(scene.tools.sprites.iter().all(|s| s.sprite >= 0x17c), "doesn't fit");

    // The menu closes with "Bauen" (Enter), the tool stays.
    let r = scene.tools.menu.key_down(0xd, 0);
    assert_eq!(r, baumenue::Response::Close);
    let view = scene.screen.camera().view;
    let (actions, key) = scene.tools.menu_response(
        r,
        &scene.world,
        &scene.font,
        &scene.tiles,
        &scene.info,
        view,
        (800, 600),
    );
    assert!(actions.is_empty() && key.is_none());
    assert_eq!((scene.tools.tool, scene.tools.menu.height), (9, 0));

    // Building: a click becomes a command; the price is paid and booked.
    let cam = scene.screen.camera();
    let actions = scene
        .tools
        .press(mouse.0, mouse.1, ShiftState::LEFT, &scene.world, &scene.tiles, &cam);
    assert_eq!(
        actions,
        vec![Action::Build {
            building: house,
            at: site
        }]
    );
    // Only exactly the left button counts.
    let shifted = ShiftState::LEFT | ShiftState::SHIFT;
    let none = scene
        .tools
        .press(mouse.0, mouse.1, shifted, &scene.world, &scene.tiles, &cam);
    assert_eq!(none, vec![]);
    let (_, cost) = scene.world.can_build(site, house, 1);
    assert!(cost > 0);
    let money = scene.world.finances.money[0];
    let start = scene.world.clone();
    let tick = scene.apply(actions[0].clone());
    assert_eq!(scene.world.finances.money[0], money - cost);
    assert_eq!(scene.world.island.buildings.at(site), 31, "a building site");
    assert_eq!(scene.world.island.owners.at(site), 1);
    assert!(tick.books && tick.status.is_empty());
    assert_eq!(scene.world.building_sites[0].building, house);

    // The same again: the site is in the way.
    let before = scene.world.clone();
    let tick = scene.apply(Action::Build {
        building: house,
        at: site,
    });
    assert_eq!(tick.status, vec![(1, Status::InTheWay(31))]);
    assert_eq!(scene.world, before, "nothing changed");
    let text = scene.tools.status_text(Status::InTheWay(31)).unwrap();
    assert!(text.ends_with(b"im Weg!"), "{}", String::from_utf8_lossy(&text));
    // In the sea: the original's error for its ground.
    let error = scene.world.can_build(sea, house, 1).0;
    assert_ne!(error, 0);
    let tick = scene.apply(Action::Build {
        building: house,
        at: sea,
    });
    assert_eq!(tick.status, vec![(1, Status::Error(error))]);
    assert_eq!(scene.world, before);
    // Off the map.
    let tick = scene.apply(Action::Build {
        building: house,
        at: (251, 5),
    });
    assert_eq!(tick.status, vec![(1, Status::Error(0x16))]);
    // Without the money: "Sie haben nicht genug Geld!".
    let mut poor = scene.world.clone();
    poor.finances.money[0] = 0;
    let free = find(&scene, |w, p| w.can_build(p, house, 1).0 == 0).0;
    let tick = poor.apply(human(Action::Build {
        building: house,
        at: free,
    }));
    assert_eq!(tick.status, vec![(1, Status::Error(0xe))]);
    assert_eq!(
        scene.tools.status_text(Status::Error(0xe)).unwrap(),
        b"Sie haben nicht genug Geld!"
    );

    // The road tool: press, drag (the preview follows the mouse each
    // tick), let go.
    scene.click(tools::ROAD);
    assert_eq!(scene.tools.tool, tools::ROAD);
    assert!(!scene.tools.down[9] && scene.tools.down[usize::from(tools::ROAD)]);
    let cam = scene.screen.camera();
    let visible = visible_squares(&scene.world, &scene.tiles, &cam);
    let free = |w: &World, p: Pos| {
        w.island.ground.at(p) <= 1
            && w.island.forms.at(p) == 16
            && matches!(w.island.buildings.at(p), NO_BUILDING | game::world::PALM)
            && w.island.owners.at(p) == 0
    };
    let seen = |p: Pos| visible.iter().find(|(q, _)| *q == p).map(|(_, m)| *m);
    let ((ri, rj), road_mouse, (di, dj)) = [(0, 1), (1, 0), (0, -1), (-1, 0)]
        .into_iter()
        .find_map(|(di, dj)| {
            visible.iter().find_map(|&((i, j), m)| {
                let ok = (0..6).all(|k| free(&scene.world, (i + di * k, j + dj * k)));
                (ok && seen((i + di * 5, j + dj * 5)).is_some()).then_some(((i, j), m, (di, dj)))
            })
        })
        .expect("six free squares in a row");
    let actions = scene.tools.press(
        road_mouse.0,
        road_mouse.1,
        ShiftState::LEFT,
        &scene.world,
        &scene.tiles,
        &cam,
    );
    assert_eq!(actions, vec![Action::RoadDragStart((ri, rj))]);
    scene.apply(actions[0].clone());
    assert!(scene.world.road_dragging);
    let end = (ri + di * 5, rj + dj * 5);
    let end_mouse = seen(end).unwrap();
    scene.show_preview(end_mouse);
    assert_eq!(scene.tools.drag_end, end);
    assert_eq!(scene.tools.sprites.len(), 6, "the road's squares");
    if let Some(out) = &out {
        write_ppm(&with_suffix(out, "road"), &scene.picture(), &scene.palette);
    }
    // `road_drag`: the original ends the road where the last tick's
    // picture put it, the fix where the mouse is let go.
    let mid = (ri + di * 3, rj + dj * 3);
    let mid_mouse = seen(mid).unwrap();
    let to_of = |a: &[Action]| match a {
        [Action::RoadDragEnd { to, .. }] => *to,
        other => panic!("{other:?}"),
    };
    let mut fixed = scene.tools.clone();
    fixed.enhance.set(Enhancement::RoadDrag, true);
    assert_eq!(
        to_of(&fixed.release(true, &scene.world, &scene.tiles, &cam, mid_mouse)),
        mid
    );
    let money = scene.world.finances.money[0];
    let actions = scene.tools.release(true, &scene.world, &scene.tiles, &cam, mid_mouse);
    assert_eq!(to_of(&actions), end, "the original's end");
    assert_eq!(
        actions,
        vec![Action::RoadDragEnd {
            from: (ri, rj),
            to: end
        }]
    );
    scene.apply(actions[0].clone());
    assert!(!scene.world.road_dragging);
    let road_cost = scene.world.tables.road_cost;
    assert!(road_cost > 0);
    assert_eq!(scene.world.finances.money[0], money - 6 * road_cost);
    for k in 0..6 {
        let p = (ri + di * k, rj + dj * k);
        assert!((10..=0x3a).contains(&scene.world.island.ground.at(p)), "{p:?}");
        assert_eq!(scene.world.island.owners.at(p), 1);
    }
    // Letting go again does nothing: the drag is over.
    let before = scene.world.clone();
    scene.apply(actions[0].clone());
    assert_eq!(scene.world, before);

    // Raising a corner of free land, then lowering it back.
    scene.click(tools::RAISE);
    let (corner_square, corner_mouse) = find(&scene, |w, (i, j)| {
        (i - 2..=i + 2).all(|a| {
            (j - 2..=j + 2).all(|b| {
                matches!(w.island.buildings.at((a, b)), NO_BUILDING | game::world::PALM)
                    && w.island.ground.at((a, b)) <= 1
                    && w.island.owners.at((a, b)) == 0
            })
        })
    });
    let cam = scene.screen.camera();
    let actions = scene.tools.press(
        corner_mouse.0,
        corner_mouse.1 + 16,
        ShiftState::LEFT,
        &scene.world,
        &scene.tiles,
        &cam,
    );
    let [
        Action::Level {
            raise: true,
            corner,
            at,
        },
    ] = actions[..]
    else {
        panic!("{actions:?}");
    };
    assert_eq!(
        at,
        square_at(&scene.world, &scene.tiles, &cam, corner_mouse.0, corner_mouse.1)
    );
    let height = scene.world.island.heights.at(corner);
    let money = scene.world.finances.money[0];
    let books = scene.world.books.year;
    let tick = scene.apply(actions[0].clone());
    assert!(tick.status.is_empty(), "{:?} at {corner_square:?}", tick.status);
    assert_eq!(scene.world.island.heights.at(corner), height + 1);
    let paid = money - scene.world.finances.money[0];
    assert!(paid >= scene.world.land_costs.raise, "{paid}");
    assert_ne!(scene.world.books.year, books);
    scene.click(tools::LOWER);
    let tick = scene.apply(Action::Level {
        raise: false,
        corner,
        at,
    });
    assert_eq!(tick.status, vec![]);
    assert_eq!(scene.world.island.heights.at(corner), height);
    // A corner on the map's edge can't go up.
    let tick = scene.apply(Action::Level {
        raise: true,
        corner: (1, 1),
        at: (1, 1),
    });
    assert!(
        matches!(tick.status[..], [(1, Status::Error(5..=8))]),
        "{:?}",
        tick.status
    );

    // Demolishing the building site gives the rest of its price back.
    scene.click(tools::DEMOLISH);
    let money = scene.world.finances.money[0];
    let tick = scene.apply(Action::Demolish(site));
    assert_eq!(tick.status, vec![]);
    assert_ne!(scene.world.island.buildings.at(site), 31);
    assert!(scene.world.finances.money[0] >= money);
    // Nothing to demolish: "Geht nicht!".
    let tick = scene.apply(Action::Demolish(sea));
    assert_eq!(tick.status, vec![(1, Status::Error(0x18))]);

    // Build again, raise the corner for good, and let the game run.
    let tick = scene.apply(Action::Build {
        building: house,
        at: site,
    });
    assert_eq!(tick.status, vec![]);
    for _ in 0..2 {
        let tick = scene.apply(Action::Level {
            raise: true,
            corner,
            at,
        });
        assert_eq!(tick.status, vec![]);
    }
    assert_eq!(scene.world.island.heights.at(corner), height + 2);
    for _ in 0..400 {
        scene.world.set_viewpoint(scene.screen.viewpoint());
        let tick = step(&mut scene.world);
        scene.screen.redraw_squares(&tick.redraw, &scene.world, &scene.tiles);
    }
    scene.screen.paint_map(&scene.world, &scene.tiles);
    scene.screen.show_info(&scene.world, &scene.font, &scene.info);
    scene.click(tools::POINTER);
    scene.show_preview((0, 0));
    if let Some(out) = &out {
        write_ppm(&with_suffix(out, "after"), &scene.picture(), &scene.palette);
    }
    assert_ne!(scene.world, start);
}

/// The same commands between the same ticks give the same world.
#[test]
fn commands_are_deterministic() {
    if !orig().join("SZENARIO").exists() {
        eprintln!("skipping: {} not found", orig().display());
        return;
    }
    let scene = Scene::new();
    let house = u8::try_from(scene.world.build_menu.rows[0][1][0]).unwrap();
    let site = find(&scene, |w, p| w.can_build(p, house, 1).0 == 0).0;
    let (ri, rj) = find(&scene, |w, (i, j)| {
        (0..4).all(|k| w.island.ground.at((i + k, j)) <= 1 && w.island.buildings.at((i + k, j)) == NO_BUILDING)
    })
    .0;
    let script = [
        (
            3,
            Action::Build {
                building: house,
                at: site,
            },
        ),
        (5, Action::RoadDragStart((ri, rj))),
        (
            5,
            Action::RoadDragEnd {
                from: (ri, rj),
                to: (ri + 3, rj),
            },
        ),
        (
            9,
            Action::Level {
                raise: true,
                corner: (ri + 1, rj + 3),
                at: (ri + 1, rj + 3),
            },
        ),
        (20, Action::Demolish((ri + 1, rj))),
    ];
    let run = || {
        let mut w = scene.world.clone();
        let mut ticks = Vec::new();
        for t in 0..60 {
            for (_, a) in script.iter().filter(|(at, _)| *at == t) {
                ticks.push(w.apply(human(a.clone())));
            }
            ticks.push(step(&mut w));
        }
        (w, ticks)
    };
    let (a, ta) = run();
    let (b, tb) = run();
    assert_eq!(a, b);
    assert_eq!(ta, tb);
    assert_ne!(a, scene.world);
}

/// The world's part of the build menu goes through a map file and back,
/// and the tools' fields through the view's.
#[test]
fn the_build_menu_is_saved() {
    if !orig().join("SZENARIO").exists() {
        eprintln!("skipping: {} not found", orig().display());
        return;
    }
    let mut scene = Scene::new();
    assert!(scene.world.build_menu.counts.iter().any(|&c| c > 0));
    scene.click(0x12);
    let mut fields = Fields::default();
    scene.tools.save(&mut fields);
    let map = scene.world.to_map(&game::savegame::MapView {
        unported: fields.clone(),
        ..Default::default()
    });
    let again = World::from_map(&map).unwrap();
    assert_eq!(again.build_menu, scene.world.build_menu);
    let view = game::savegame::MapView::from_map(&map);
    assert_eq!(view.unported.get("1110:6056", 0), Some(&[0x12, 0][..]));
    assert_eq!(view.unported.get("[1110:6516]+0x1ac", 0), Some(&[0x12][..]));
}
