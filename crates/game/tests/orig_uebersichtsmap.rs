//! The overview map (`TUebersichtsmap2`) and the compass (`TKompassform`)
//! on a scenario's island after a few months: the colourings against the
//! world, the incremental redraws, moving the main screen from the map,
//! the scroll bars, the keys and the saved fields. Skipped when `orig/`
//! isn't there. With `UEBERSICHTSMAP_PPM=dir` it writes the form in each
//! colouring to `dir/uebersichtsmap-<mode>.ppm`, and the compass to
//! `dir/kompass.ppm`.

// The original's `div` by 2, as it's written.
#![allow(clippy::manual_midpoint, clippy::many_single_char_names)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use formats::bmp::Bitmap;
use formats::data::{
    animdat1, animdat4, darstell, flusspos, fontdat, gfxpos, groundnr, grounpos, hausdata, hausnr, hauspos, hausprei,
    modatyp, sonstige, straspos, uebermap_dat, uebermap_txt,
};
use formats::dfm::Object;
use formats::ne::{RT_RCDATA, ResId, resources};
use formats::savegame::SaveGame;
use game::auction;
use game::command::{self, Command};
use game::darstellung::{self, Camera, Tiles};
use game::enhance::Enhancement;
use game::hauptschirm::{self, Hauptschirm};
use game::hintergrund::{SCENARIO_FILES, load_world};
use game::kompassform::{self, Kompassform};
use game::savegame::Fields;
use game::simulation::Tick;
use game::startform::Settings;
use game::text::Font;
use game::uebersichtsmap::{self, Uebersichtsmap, mode};
use game::widgets::ShiftState;
use game::world::{NO_BUILDING, PALM, Pos, Tables, World, on_map};
use vcl::Palette;
use vcl::form::{Form, Kind};
use vcl::random::Random;
use vcl::surface::Surface;

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
        darstellung::BITMAP_FILES
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
    for &i in &s.pixels {
        let c = palette[usize::from(i)];
        ppm.extend_from_slice(&[c.r, c.g, c.b]);
    }
    std::fs::write(path, ppm).unwrap();
}

/// A tick, leaving every auction at once.
fn step(world: &mut World) -> Tick {
    let mut tick = world.tick();
    if world.auction.open {
        let left = world.apply(Command {
            player: 1,
            action: command::Action::Auction(auction::Action::Leave),
        });
        tick.redraw.extend(left.redraw);
    }
    tick
}

/// The first scenario after three months or so, with its view.
fn scenario() -> (World, Camera, Pos) {
    let file = SCENARIO_FILES[0];
    let map = SaveGame::parse(&read(&format!("SZENARIO/{file}"))).unwrap();
    let settings = Settings {
        scenario: 1,
        difficulty: 1,
        ..Settings::default()
    };
    let mut world = load_world(&map, &settings, true).unwrap();
    world.tables = world_tables();
    world.random = Random::new(0x0b5e_55ed);
    let d = map.field("1110:5530").unwrap();
    let origin = (i16::from_le_bytes([d[0], d[1]]), i16::from_le_bytes([d[2], d[3]]));
    let camera = Camera {
        view: 0,
        origin: (0, 0),
        parity: 0,
        owner_pointers: false,
        occupancy_pointers: false,
    };
    // About three months at the scenario's speed.
    for _ in 0..15_000 {
        step(&mut world);
    }
    (world, camera, origin)
}

fn overview(main_view: (i16, i16, u8)) -> Uebersichtsmap {
    let s = sonstige(&read("TEXTE/SONSTIGE.TXT")).unwrap();
    Uebersichtsmap::new(
        font(),
        surfaces(&uebersichtsmap::BITMAP_FILES),
        &uebermap_dat(&read("DATA/UEBERMAP.DAT")).unwrap(),
        &uebermap_txt(&read("TEXTE/UEBERMAP.TXT")).unwrap(),
        &s.texts[13],
        modatyp(&read("DATA/MODATYP.DAT")).unwrap().max_grueneinfluss,
        main_view,
    )
}

/// Which square each pixel of `MapBild` was last drawn for, and where in
/// its two by two block: 10b8:0eeb's walk, again.
fn owners(form: &Uebersichtsmap, world: &World) -> Vec<Option<(Pos, i32, i32)>> {
    let (w, h) = form.map_size();
    let (ox, oy) = form.origin();
    let mut out = vec![None; usize::try_from(w * h).unwrap()];
    let mut y = -2;
    while y <= h + 2 {
        let mut x = -1;
        while x <= w + 1 {
            let (px, py) = (x + ox, y + oy);
            let half = px / 2;
            let pos = (
                i16::try_from((py + half + 750) / 2 - 500).unwrap(),
                i16::try_from((py - half + 1250) / 2 - 500).unwrap(),
            );
            if on_map(pos) && world.island.ground.at(pos) != 2 {
                let (bx, by) = (x - px % 2, y - (py - half + 250) % 2 - 2);
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let (qx, qy) = (bx + dx, by + dy);
                    if (0..w).contains(&qx) && (0..h).contains(&qy) {
                        out[usize::try_from(qy * w + qx).unwrap()] = Some((pos, dx, dy));
                    }
                }
            }
            x += 2;
        }
        y += 2;
    }
    out
}

/// Checks each pixel of `MapBild` drawn for a square that `expected`
/// gives a colour for (column, row in `Ueberma2.BMP`); how many it
/// checked.
fn check(form: &Uebersichtsmap, world: &World, bare: &Surface, expected: impl Fn(Pos) -> Option<(i32, i32)>) -> usize {
    let colours = Surface::from_bitmap(&bitmap("UEBERMA2.BMP"));
    let (w, _) = form.map_size();
    let mut n = 0;
    for (k, owner) in owners(form, world).into_iter().enumerate() {
        let Some((pos, dx, dy)) = owner else {
            continue;
        };
        let Some((cx, cy)) = expected(pos) else {
            continue;
        };
        let (x, y) = (i32::try_from(k).unwrap() % w, i32::try_from(k).unwrap() / w);
        let want = colours.pixel(usize::try_from(cx + dx).unwrap(), usize::try_from(cy + dy).unwrap());
        assert_eq!(
            bare.pixel(usize::try_from(x).unwrap(), usize::try_from(y).unwrap()),
            want,
            "{pos:?} at {x}, {y}"
        );
        n += 1;
    }
    n
}

fn building_proper(b: u8) -> bool {
    ![11, PALM, 36, 52, NO_BUILDING].contains(&b)
}

fn plain_ground(g: u8) -> bool {
    !((10..=100).contains(&g))
}

/// `MapBild` without the frame: the main screen's view moved off the map.
fn bare(form: &Uebersichtsmap) -> Surface {
    let mut f = form.clone();
    f.set_main_view(-1000, -1000, 0);
    f.map.clone()
}

/// The form through the VCL form from the DFM.
fn paint(form: &Uebersichtsmap, palette: &Palette) -> Surface {
    let mut vcl = load_form("TUEBERSICHTSMAP2");
    vcl.components[0].bounds = vcl::Rect::new(0, 0, form.width, form.height);
    vcl.components[0].visible = true;
    for (image, bounds, picture) in form.images() {
        let c = vcl.component_mut(image.name()).unwrap();
        c.bounds = bounds;
        c.visible = true;
        if let Kind::Image { picture: p } = &mut c.kind {
            *p = Some(picture.clone());
        }
    }
    let mut out = Surface::new(
        usize::try_from(form.width).unwrap(),
        usize::try_from(form.height).unwrap(),
        0,
    );
    vcl.paint(&mut out, palette);
    out
}

#[test]
#[allow(clippy::too_many_lines)]
fn the_overview_map() {
    if !orig().join("GRAFIK").exists() {
        eprintln!("skipping: {} not found", orig().display());
        return;
    }
    let palette = bitmap("GFXGROUN.BMP").palette;
    let out = std::env::var_os("UEBERSICHTSMAP_PPM").map(PathBuf::from);
    let save = |form: &Uebersichtsmap, name: &str| {
        if let Some(dir) = &out {
            write_ppm(
                &dir.join(format!("uebersichtsmap-{name}.ppm")),
                &paint(form, &palette),
                &palette,
            );
        }
    };
    let (mut world, camera, origin) = scenario();
    let font = font();
    let tiles = tiles();
    let title = &sonstige(&read("TEXTE/SONSTIGE.TXT")).unwrap().texts[0];
    let mut screen = Hauptschirm::new(camera, origin, surfaces(&hauptschirm::BITMAP_FILES), &font, title);
    screen.paint_map(&world, &tiles);
    let (wo, ns) = screen.scroll_position();
    let mut form = overview((wo, ns, 0));
    assert!(!form.visible());
    form.game_start(&world);
    let opened = form.open(&world);
    assert_eq!(opened.sounds, [0xd]);
    assert!(form.visible() && (form.width, form.height) == (421, 348));
    assert_eq!(form.map_size(), (405, 275));
    // The window is around the main screen's view.
    let (ox, oy) = form.origin();
    assert_eq!(ox, ((i32::from(wo) + 10) * 2 - 405 / 2).clamp(0, 1002 - 405));
    assert_eq!(oy, (i32::from(ns) + 13 - 275 / 2).clamp(0, 502 - 275));
    assert_eq!(form.mode(), mode::TERRAIN);

    // The land alone, by heights: water shows the sea's tile, land its
    // height's colour.
    let colours = Surface::from_bitmap(&bitmap("UEBERMA2.BMP"));
    let b = bare(&form);
    let n = check(&form, &world, &b, |pos| {
        let g = world.island.ground.at(pos);
        let h = i32::from(world.island.heights.at(pos));
        let grass = g == 1 || (0x19..0x28).contains(&g);
        if (0x37..=0x5e).contains(&g) {
            Some((0x30, 0x1a))
        } else if grass && h >= 3 {
            Some((0x32, ((h - 3) * 2).min(0x24)))
        } else if !grass && h < 6 {
            Some((0x30, h * 2 + 0x1a))
        } else {
            None
        }
    });
    assert!(n > 10_000, "{n} pixels of land");
    let sea = owners(&form, &world).iter().position(Option::is_none).unwrap();
    let (sx, sy) = (sea % 405, sea / 405);
    assert_eq!(b.pixel(sx, sy), colours.pixel(sx % 48, sy % 38));
    save(&form, "terrain");

    // The toggle: slopes.
    let _ = form.toggle_mouse_down(true);
    let _ = form.toggle_mouse_up(5, 5, true, &world);
    assert!(!form.heights());
    let b = bare(&form);
    let n = check(&form, &world, &b, |pos| {
        let g = world.island.ground.at(pos);
        let grass = g == 1 || (0x19..0x28).contains(&g);
        let row = i32::from(world.island.forms.at(pos)) * 2;
        Some(if (0x37..=0x5e).contains(&g) {
            (0x30, 0x1a)
        } else if grass {
            (0x36, row)
        } else {
            (0x34, row)
        })
    });
    assert!(n > 10_000);
    save(&form, "slopes");
    let _ = form.toggle_mouse_down(true);
    let _ = form.toggle_mouse_up(5, 5, true, &world);
    assert!(form.heights());

    // The owners, with key 1: buildings in their owner's colour.
    let r = form.key_down(0x31, ShiftState::default(), &world);
    assert!(r.refresh && form.mode() == mode::OWNERS);
    let b = bare(&form);
    let n = check(&form, &world, &b, |pos| {
        let i = &world.island;
        let owner = usize::from(i.owners.at(pos));
        (building_proper(i.buildings.at(pos)) && plain_ground(i.ground.at(pos)) && (1..=8).contains(&owner))
            .then(|| (0x30, (i32::from(world.player_colours[owner - 1]) - 1) * 2))
    });
    assert!(n > 50, "{n} pixels of buildings");
    save(&form, "owners");

    // The incremental redraws: a few weeks of ticks shown, and the same
    // as drawing it all again.
    for _ in 0..3000 {
        let t = step(&mut world);
        form.show_tick(&world, &t);
    }
    let mut fresh = form.clone();
    let _ = fresh.key_down(0x31, ShiftState::default(), &world);
    let _ = fresh.key_down(0x31, ShiftState::default(), &world);
    assert_eq!(fresh.mode(), mode::OWNERS);
    assert_eq!(bare(&form), bare(&fresh), "the redraws missed a change");

    // Buildings by their kind, with F2.
    let _ = form.key_down(0x71, ShiftState::default(), &world);
    assert_eq!(form.mode(), mode::BUILDINGS);
    let b = bare(&form);
    let n = check(&form, &world, &b, |pos| {
        let i = &world.island;
        let building = i.buildings.at(pos);
        (building_proper(building) && plain_ground(i.ground.at(pos)))
            .then(|| (0x38, i32::from(world.tables.hausdata(building, 5)) * 2 + 2))
    });
    assert!(n > 50);
    save(&form, "buildings");

    // The location quality: every land square.
    let _ = form.key_down(0x33, ShiftState::default(), &world);
    assert_eq!(form.mode(), mode::LOCATION);
    let b = bare(&form);
    let n = check(&form, &world, &b, |pos| {
        Some((0x3a, 0x24 - i32::from(world.location_at(pos)) * 0x24 / 0xff))
    });
    assert!(n > 10_000);
    // Its changes are drawn as they come.
    for _ in 0..2000 {
        let t = step(&mut world);
        form.show_tick(&world, &t);
    }
    let b = bare(&form);
    check(&form, &world, &b, |pos| {
        Some((0x3a, 0x24 - i32::from(world.location_at(pos)) * 0x24 / 0xff))
    });
    save(&form, "location");

    // Green: palms.
    let _ = form.key_down(0x34, ShiftState::default(), &world);
    assert_eq!(form.mode(), mode::GREEN);
    let b = bare(&form);
    let n = check(&form, &world, &b, |pos| {
        (world.island.buildings.at(pos) == PALM).then_some((0x38, 0x1e))
    });
    assert!(n > 50, "{n} pixels of palms");
    save(&form, "green");

    // The occupancy.
    let _ = form.key_down(0x35, ShiftState::SHIFT, &world);
    assert_eq!(form.mode(), mode::OCCUPANCY);
    let b = bare(&form);
    let n = check(&form, &world, &b, |pos| {
        let i = &world.island;
        building_proper(i.buildings.at(pos)).then(|| (0x3c, 0x20 - i32::from(i.occupancy.at(pos)) * 32 / 100))
    });
    assert!(n > 50);
    save(&form, "occupancy");
    // Its button again: the land alone.
    let _ = form.key_down(0x74, ShiftState::default(), &world);
    assert_eq!(form.mode(), mode::TERRAIN);

    // A left click on the map moves the main screen there, with the
    // frame.
    let before = form.map.clone();
    let (x, y) = (200, 100);
    let r = form.map_mouse_down(x, y, ShiftState::LEFT, &world);
    let (ox, oy) = form.origin();
    let want = (
        i16::try_from((x - 20 + ox) / 2).unwrap(),
        i16::try_from(y - 12 + oy).unwrap(),
    );
    assert_eq!(r.scroll, Some(want));
    assert_ne!(form.map, before);
    screen.set_scroll(want.0, want.1, &world, &tiles);
    assert_eq!(screen.scroll_position(), want);
    // The frame is centred on the click: its picture at the click's
    // offset, the map elsewhere as it was.
    let b = bare(&form);
    let framed = (0..275usize)
        .flat_map(|y| (0..405usize).map(move |x| (x, y)))
        .filter(|&(x, y)| form.map.pixel(x, y) != b.pixel(x, y))
        .collect::<Vec<_>>();
    assert_ne!(framed.len(), 0);
    assert!(framed.iter().all(|&(fx, fy)| {
        let (fx, fy) = (i32::try_from(fx).unwrap(), i32::try_from(fy).unwrap());
        (x - 22..x + 20).contains(&fx) && (y - 12..y + 28).contains(&fy)
    }));
    // Dragging follows; the main screen's scrolling moves the frame too.
    let r = form.map_mouse_move(x + 10, y, ShiftState::LEFT);
    assert_eq!(r.scroll.map(|s| s.0), Some(want.0 + 5));
    assert!(form.map_mouse_move(x + 10, y, ShiftState::RIGHT).scroll.is_none());
    assert!(form.map_mouse_up(true).draw_main);
    screen.scroll(2, 0, &world, &tiles);
    let (wo, ns) = screen.scroll_position();
    assert!(form.set_main_view(wo, ns, screen.camera().view));

    // A right click centres the window on the point.
    let (ox, oy) = form.origin();
    let _ = form.map_mouse_down(300, 200, ShiftState::RIGHT, &world);
    assert_eq!(
        form.origin(),
        ((ox + 300 - 202).clamp(0, 597), (oy + 200 - 137).clamp(0, 227))
    );

    // The keys scroll by 20; the vertical arrow by 5, every 25 ms while
    // held, with no new people meanwhile.
    let (ox, oy) = form.origin();
    let _ = form.key_down(0x26, ShiftState::default(), &world);
    assert_eq!(form.origin(), (ox, (oy - 20).max(0)));
    let (ox, oy) = form.origin();
    let r = form.scroll_mouse_down(270, true, &world);
    assert_eq!(r.sounds, [9]);
    assert_eq!(
        r.action,
        Some(command::Action::SetOption(
            game::optionen::WorldOption::PeopleSpawn,
            false
        ))
    );
    world.apply(Command {
        player: 1,
        action: r.action.unwrap(),
    });
    assert!(!world.options.people_spawn && form.scrolling());
    assert!(form.scroll_timers(&world));
    assert_eq!(form.origin(), (ox, (oy + 10).min(227)));
    let r = form.scroll_mouse_up();
    world.apply(Command {
        player: 1,
        action: r.action.unwrap(),
    });
    assert!(world.options.people_spawn && !form.scrolling());
    // The right button presses the vertical bar too (10b8:222b); with
    // `mouse_buttons` only the left one does.
    let before = (form.origin(), form.enhance);
    form.enhance.set(Enhancement::MouseButtons, true);
    let r = form.scroll_mouse_down(270, false, &world);
    assert!(r.sounds.is_empty() && r.action.is_none() && !form.scrolling());
    assert_eq!(form.origin(), before.0);
    form.enhance = before.1;
    let r = form.scroll_mouse_down(270, false, &world);
    assert_eq!(r.sounds, [9]);
    assert!(r.action.is_some() && form.scrolling());
    assert_eq!(form.origin(), (before.0.0, (before.0.1 + 5).min(227)));
    world.apply(Command {
        player: 1,
        action: r.action.unwrap(),
    });
    let r = form.scroll_mouse_up();
    world.apply(Command {
        player: 1,
        action: r.action.unwrap(),
    });
    assert!(world.options.people_spawn && !form.scrolling());
    // The horizontal one's left arrow.
    let (ox, _) = form.origin();
    let r = form.menu_mouse_down(5, 5, true, &world);
    assert_eq!(
        r.action,
        Some(command::Action::SetOption(
            game::optionen::WorldOption::PeopleSpawn,
            false
        ))
    );
    assert_eq!(form.origin().0, (ox - 5).max(0));
    let _ = form.menu_mouse_up(5, 5, &world);

    // The menu's button picks a colouring when it's let go over it.
    let r = form.menu_mouse_down(90, 30, true, &world);
    assert_eq!((r.sounds.as_slice(), form.mode()), ([9u8].as_slice(), mode::TERRAIN));
    let _ = form.menu_mouse_up(90, 30, &world);
    assert_eq!(form.mode(), mode::OWNERS);

    // The fields go into a map file and back.
    let mut fields = Fields::default();
    form.save(&mut fields);
    let mut back = overview((0, 0, 0));
    back.load(&fields);
    back.game_start(&world);
    let mut again = Fields::default();
    back.save(&mut again);
    assert_eq!(fields, again);
    assert_eq!(back.mode(), mode::OWNERS);
    assert_eq!(back.origin(), form.origin());

    // The size button: the small layout.
    let _ = form.title_mouse_down(395, ShiftState::LEFT, (0, 0));
    let r = form.title_mouse_up(395, 5, &world);
    assert!(r.moved && form.small());
    assert_eq!(form.map_size(), (274, 189));
    save(&form, "small");

    // Dragging the title bar, then the close button.
    let _ = form.title_mouse_down(100, ShiftState::LEFT, (100, 5));
    assert_eq!(form.height, 16);
    let _ = form.title_mouse_move(ShiftState::LEFT, (150, 60), (800, 600));
    assert_eq!((form.left, form.top), (50, 55));
    let _ = form.title_mouse_up(100, 5, &world);
    assert_eq!(form.height, 262);
    let _ = form.title_mouse_down(5, ShiftState::LEFT, (55, 60));
    let r = form.title_mouse_up(5, 5, &world);
    assert!(r.closed && !form.visible());
    assert_eq!(r.sounds, [10, 0xe]);

    // Escape closes it too; other keys go to the main screen.
    let _ = form.open(&world);
    assert_eq!(
        form.key_down(0x41, ShiftState::default(), &world).key,
        Some((0x41, ShiftState::default()))
    );
    assert!(form.key_down(0x1b, ShiftState::default(), &world).closed);
}

/// The overview map's fields in a map file. // 10b8:4e34
const DESTS: [&str; 27] = [
    "[1110:723a]+0x198",
    "[1110:723a]+0x199",
    "[1110:723a]+0x19a",
    "[1110:723a]+0x19b",
    "[1110:723a]+0x19d",
    "[1110:723a]+0x19f",
    "[1110:723a]+0x1a0",
    "[1110:723a]+0x1a2",
    "[1110:723a]+0x213",
    "[1110:723a]+0x1a4",
    "[1110:723a]+0x1a5",
    "[1110:723a]+0x1a6",
    "[1110:723a]+0x1a7",
    "[1110:723a]+0x1b0",
    "[1110:723a]+0x1b4",
    "[1110:723a]+0x1b8",
    "[1110:723a]+0x1bc",
    "[1110:723a]+0x1be",
    "[1110:723a]+0x1c0",
    "[1110:723a]+0x1c2",
    "[1110:723a]+0x1c4",
    "[1110:723a]+0x1c6",
    "[1110:723a]+0x1c8",
    "[1110:723a]+0x1ca",
    "[1110:723a]+0x1cc",
    "[1110:723a]+0x1d0",
    "[1110:723a]+0x212",
];

/// The overview map's fields of the CD's maps: read and written back as
/// they were, and shown.
#[test]
fn the_saved_fields() {
    if !orig().join("MAPS").exists() {
        eprintln!("skipping: {} not found", orig().display());
        return;
    }
    let files = std::iter::once("MAPS/TUTORIAL.MAP".to_owned()).chain(SCENARIO_FILES.map(|f| format!("SZENARIO/{f}")));
    for file in files {
        let map = SaveGame::parse(&read(&file)).unwrap();
        let fields = game::savegame::MapView::from_map(&map).unported;
        let world = World::from_map(&map).unwrap();
        let mut form = overview((0, 0, 0));
        form.load(&fields);
        let mut back = Fields::default();
        form.save(&mut back);
        for dest in DESTS {
            assert_eq!(back.get(dest, 0), fields.get(dest, 0), "{file}: {dest}");
        }
        form.game_start(&world);
        assert!(!form.small() && form.mode() <= mode::TERRAIN, "{file}");
        assert_eq!(form.map_size(), (405, 275), "{file}");
    }
}

#[test]
fn the_compass() {
    if !orig().join("GRAFIK").exists() {
        eprintln!("skipping: {} not found", orig().display());
        return;
    }
    let palette = bitmap("GFXGROUN.BMP").palette;
    let mut k = Kompassform::new(font(), surfaces(&kompassform::BITMAP_FILES), 800);
    assert!(!k.visible());
    assert_eq!((k.left, k.top), (740, 0));
    // The knob opens it with the view's button on.
    let r = k.knob_mouse_down(8, 8, 1);
    assert!(r.knob && k.visible() && k.knob_on());
    assert_eq!(r.sounds, [9]);
    let mut corner = Surface::new(16, 16, 0);
    k.draw_knob(&mut corner);
    assert_eq!(corner, Surface::from_bitmap(&bitmap("KOMPKNPR.BMP")));
    // North turns the view to 2.
    let r = k.mouse_down(30, 25, ShiftState::LEFT, (770, 25));
    assert_eq!(r.view, Some(2));
    assert_eq!(k.mouse_down(30, 25, ShiftState::LEFT, (770, 25)).view, None);
    if let Some(dir) = std::env::var_os("UEBERSICHTSMAP_PPM") {
        let mut vcl = load_form("TKOMPASSFORM");
        vcl.components[0].bounds = vcl::Rect::new(0, 0, kompassform::WIDTH, k.height);
        vcl.components[0].visible = true;
        let c = vcl.component_mut("Bild").unwrap();
        c.bounds = vcl::Rect::new(0, 0, kompassform::WIDTH, kompassform::HEIGHT);
        if let Kind::Image { picture } = &mut c.kind {
            *picture = Some(k.image.clone());
        }
        let mut out = Surface::new(60, 76, 0);
        vcl.paint(&mut out, &palette);
        write_ppm(&PathBuf::from(dir).join("kompass.ppm"), &out, &palette);
    }
    // The title row drags it.
    let _ = k.mouse_down(40, 5, ShiftState::LEFT, (780, 5));
    assert_eq!(k.height, 16);
    let _ = k.mouse_move(ShiftState::LEFT, (700, 100), (800, 600));
    assert_eq!((k.left, k.top), (660, 95));
    let _ = k.mouse_up(40, 5);
    assert_eq!(k.height, 76);
    // The close button hides it and lets the knob up.
    let down = k.mouse_down(5, 5, ShiftState::LEFT, (665, 100));
    assert_eq!(down.sounds, [8]);
    let r = k.mouse_up(5, 5);
    assert!(r.knob && !k.visible() && !k.knob_on());
    // The keys turn the view: the buttons follow.
    k.update(3);
    let r = k.knob_mouse_down(8, 8, 3);
    assert!(k.visible() && r.refresh);
    assert_eq!(
        k.mouse_down(45, 45, ShiftState::LEFT, (0, 0)).view,
        None,
        "east is already on"
    );
}

/// `overview_colours`: low grass, high sand and nobody's buildings, which
/// the original draws in no colour (leaving what was there), get the
/// bitmap's nearest. The scenario has none, so some squares in view are
/// made so: every eighth grass square lowered to 2, sand square raised to
/// 7, and building given to nobody. With `UI_PPM=dir` the terrain and the
/// owners go there as `ui-overview_colours-original.ppm` and `-fixed.ppm`
/// (side by side).
#[test]
fn the_overview_colours_with_the_fix() {
    if !orig().join("GRAFIK").exists() {
        eprintln!("skipping: {} not found", orig().display());
        return;
    }
    let palette = bitmap("GFXGROUN.BMP").palette;
    let (mut world, camera, origin) = scenario();
    let font = font();
    let tiles = tiles();
    let title = &sonstige(&read("TEXTE/SONSTIGE.TXT")).unwrap().texts[0];
    let mut screen = Hauptschirm::new(camera, origin, surfaces(&hauptschirm::BITMAP_FILES), &font, title);
    screen.paint_map(&world, &tiles);
    let (wo, ns) = screen.scroll_position();
    let mut seen: Vec<Pos> = owners(&overview_open(&world, (wo, ns)), &world)
        .into_iter()
        .flatten()
        .map(|(p, _, _)| p)
        .collect();
    seen.sort_unstable();
    seen.dedup();
    let grassy = |g: u8| g == 1 || (0x19..0x28).contains(&g);
    for (n, &p) in seen.iter().enumerate() {
        let isl = &mut world.island;
        if n % 8 != 0 {
            continue;
        }
        let g = isl.ground.at(p);
        if building_proper(isl.buildings.at(p)) {
            isl.owners.set(p, 0);
        } else if grassy(g) {
            isl.heights.set(p, 2);
        } else if g == 0 {
            isl.heights.set(p, 7);
        }
    }
    let mut pictures = Vec::new();
    let mut differ = Vec::new();
    for owner_mode in [false, true] {
        let mut forms: Vec<Uebersichtsmap> = [false, true]
            .into_iter()
            .map(|fix| {
                let mut form = overview((wo, ns, 0));
                form.enhance.set(Enhancement::OverviewColours, fix);
                form.game_start(&world);
                let _ = form.open(&world);
                if owner_mode {
                    let _ = form.key_down(0x31, ShiftState::default(), &world);
                }
                form
            })
            .collect();
        let (a, b) = (bare(&forms[0]), bare(&forms[1]));
        differ.push(a.pixels.iter().zip(&b.pixels).filter(|(p, q)| p != q).count());
        pictures.push(forms.iter_mut().map(|f| paint(f, &palette)).collect::<Vec<_>>());
    }
    assert!(differ[0] > 100 && differ[1] > 10, "{differ:?} pixels differ");
    if let Some(dir) = std::env::var_os("UI_PPM") {
        let dir = PathBuf::from(dir);
        for (k, name) in ["original", "fixed"].iter().enumerate() {
            let (a, b) = (&pictures[0][k], &pictures[1][k]);
            let mut both = Surface::new(a.width + b.width, a.height, 0);
            let all = both.bounds();
            both.blit(0, 0, a, a.bounds(), vcl::surface::Rop::Copy, all);
            both.blit(
                i32::try_from(a.width).unwrap(),
                0,
                b,
                b.bounds(),
                vcl::surface::Rop::Copy,
                all,
            );
            write_ppm(&dir.join(format!("ui-overview_colours-{name}.ppm")), &both, &palette);
        }
    }
}

/// The overview map, opened around the main view at `wo`, `ns`.
fn overview_open(world: &World, (wo, ns): (i16, i16)) -> Uebersichtsmap {
    let mut form = overview((wo, ns, 0));
    form.game_start(world);
    let _ = form.open(world);
    form
}
