//! The pointer's info box (`TInfobox2`) and the buildings' overview
//! (`TUmenuform`) on a scenario's island after a few months: what the box
//! shows for a hotel with guests, a building site, a road and the bare
//! ground, its buttons and the overview's as commands. Skipped when
//! `orig/` isn't there. With `INFOBOX_PPM=dir` it writes the screen with
//! the box over each of them (`infobox-<what>.ppm`) and the overview
//! (`umenu.ppm`) into `dir`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use formats::bmp::Bitmap;
use formats::data::{
    animdat1, animdat4, darstell, flusspos, fontdat, gfxpos, groundnr, grounpos, hausdata, hausname, hausnr, hauspos,
    hausprei, infobox_dat, infobox_txt, modatyp, monate, sonstige, straspos, text_lines,
};
use formats::dfm::Object;
use formats::ne::{RT_RCDATA, ResId, resources};
use formats::savegame::SaveGame;
use game::auction;
use game::command::{Action, Command};
use game::darstellung::{BITMAP_FILES, Camera, Tiles, square_at};
use game::enhance::{Enhancement, Enhancements};
use game::hauptschirm::{self, Hauptschirm, InfoTexts, LAYOUT};
use game::hintergrund::{SCENARIO_FILES, load_world};
use game::infobox::{self, Infobox, Request};
use game::money::format_money;
use game::settlement::raise_rent;
use game::simulation::{Status, Tick};
use game::startform::Settings;
use game::text::Font;
use game::tools::{self, Tools};
use game::umenuform::{self, Redraw, Umenuform};
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

fn write_ppm(path: &Path, s: &Surface, palette: &Palette) {
    let mut ppm = format!("P6\n{} {}\n255\n", s.width, s.height).into_bytes();
    for &p in &s.pixels {
        let c = palette[usize::from(p)];
        ppm.extend_from_slice(&[c.r, c.g, c.b]);
    }
    std::fs::write(path, ppm).unwrap();
}

const SCREEN: (i32, i32) = (800, 600);

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

struct Scene {
    world: World,
    tiles: Tiles,
    screen: Hauptschirm,
    tools: Tools,
    font: Font,
    info: InfoTexts,
    palette: Palette,
    box_: Infobox,
    umenu: Umenuform,
}

impl Scene {
    fn new() -> Scene {
        let file = SCENARIO_FILES[1];
        let map = SaveGame::parse(&read(&format!("SZENARIO/{file}"))).unwrap();
        let settings = Settings {
            scenario: 2,
            difficulty: 1,
            ..Settings::default()
        };
        let mut world = load_world(&map, &settings, true).unwrap();
        world.tables = world_tables();
        world.random = Random::new(0x1f0_b0c5);
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
        let texts = sonstige(&read("TEXTE/SONSTIGE.TXT")).unwrap().texts;
        let mut screen = Hauptschirm::new(camera, origin, surfaces(&hauptschirm::BITMAP_FILES), &font, &texts[0]);
        let tiles = tiles();
        screen.paint_map(&world, &tiles);
        screen.show_info(&world, &font, &info);
        let menu = game::baumenue::Baumenue::new(
            formats::data::baumenu_txt(&read("TEXTE/BAUMENU.TXT")).unwrap(),
            &formats::data::baumenu_dat(&read("DATA/BAUMENU.DAT")).unwrap(),
            hausname(&read("TEXTE/HAUSNAME.TXT")).unwrap(),
            formats::data::auswahl(&read("TEXTE/AUSWAHL.TXT")),
            surfaces(&game::baumenue::BITMAP_FILES),
            SCREEN,
        );
        let tools = Tools::new(menu, formats::data::fehler(&read("TEXTE/FEHLER.TXT")).unwrap());
        screen.draw_buttons(&tools.down);
        let (short_names, long_names) = hausname(&read("TEXTE/HAUSNAME.TXT")).unwrap();
        let ib_texts = infobox_txt(&read("TEXTE/INFOBOX.TXT")).unwrap();
        let box_ = Infobox::new(
            infobox_dat(&read("DATA/INFOBOX.DAT")).unwrap(),
            ib_texts.clone(),
            long_names,
            texts,
            surfaces(&infobox::BITMAP_FILES),
        );
        let umenu = Umenuform::new(
            surfaces(&umenuform::BITMAP_FILES),
            text_lines(&read("TEXTE/UMENU.TXT")),
            ib_texts.short_texts,
            short_names,
        );
        Scene {
            world,
            tiles,
            screen,
            tools,
            font,
            info,
            palette: bitmap("GFXGROUN.BMP").palette,
            box_,
            umenu,
        }
    }

    fn main_rect() -> Rect {
        let (left, top) = Hauptschirm::position(SCREEN.0, SCREEN.1);
        Rect::new(left, top, hauptschirm::WIDTH, hauptschirm::HEIGHT)
    }

    /// The human's command, and the view's answer: the status bar, the
    /// island, the box over a finished site, the overview.
    fn apply(&mut self, action: Action) -> Tick {
        let tick = self.world.apply(human(action));
        self.shown(&tick);
        tick
    }

    fn tick(&mut self) -> Tick {
        let tick = step(&mut self.world);
        self.shown(&tick);
        self.screen.show_info(&self.world, &self.font, &self.info);
        tick
    }

    fn shown(&mut self, tick: &Tick) {
        self.screen.redraw_squares(&tick.redraw, &self.world, &self.tiles);
        self.box_.world_changed(&self.world, tick);
        if tick.umenu {
            self.umenu.refresh(&self.world, &self.font, &self.info);
        }
    }

    /// A press of the pointer at `mouse` in `Hauptbild`, and the box's
    /// answer.
    fn point(&mut self, mouse: (i32, i32), shift: ShiftState) {
        let cam = self.screen.camera();
        let actions = self
            .tools
            .press(mouse.0, mouse.1, shift, &self.world, &self.tiles, &cam);
        assert_eq!(actions, []);
        match self.tools.info_box.take() {
            Some(Request::Show { square, mouse, modal }) => self.box_.show(
                &self.world,
                &self.font,
                &self.info,
                square,
                mouse,
                Scene::main_rect(),
                modal,
            ),
            other => panic!("{other:?}"),
        }
    }

    fn release(&mut self) {
        let cam = self.screen.camera();
        let actions = self.tools.release(true, &self.world, &self.tiles, &cam, (0, 0));
        assert_eq!(actions, []);
        if let Some(Request::Hide) = self.tools.info_box.take() {
            self.box_.hide();
        }
    }

    /// Clicks the box's button `b`; applies its command.
    fn press_button(&mut self, b: usize) -> infobox::Response {
        let (x, y) = self.button_point(b);
        self.box_.mouse_down(x, y, &self.font);
        let r = self
            .box_
            .mouse_up(x, y, &self.world, &self.font, &self.info, Scene::main_rect());
        if let Some(a) = r.action.clone() {
            let tick = self.apply(a);
            assert!(tick.status.is_empty(), "{:?}", tick.status);
            self.box_.redraw(&self.world, &self.font, &self.info);
        }
        r
    }

    /// The middle of the box's button `b` as it's drawn.
    fn button_point(&mut self, b: usize) -> (i32, i32) {
        let r = self.box_.button(b).unwrap_or_else(|| panic!("button {b} not shown"));
        (r.left + r.width() / 2, r.top + r.height() / 2)
    }

    /// The screen with the main screen, the box and the overview where
    /// they are.
    fn picture(&self) -> Surface {
        let mut s = Surface::new(SCREEN.0.unsigned_abs() as usize, SCREEN.1.unsigned_abs() as usize, 0);
        let mut form = load_form("THAUPTSCHIRM");
        form.components[0].bounds = Scene::main_rect();
        form.components[0].visible = true;
        for (name, bounds) in LAYOUT {
            let c = form.component_mut(name).unwrap();
            c.bounds = bounds;
            c.visible = true;
        }
        for (name, image) in self.screen.images() {
            if let Kind::Image { picture } = &mut form.component_mut(name).unwrap().kind {
                *picture = Some(image.clone());
            }
        }
        form.paint(&mut s, &self.palette);
        if self.box_.visible {
            let mut f = load_form("TINFOBOX2");
            f.components[0].bounds = self.box_.bounds();
            f.components[0].visible = true;
            for (name, bounds, image) in self.box_.images() {
                let c = f.component_mut(name).unwrap();
                c.bounds = bounds;
                c.visible = true;
                if let Kind::Image { picture } = &mut c.kind {
                    *picture = Some(image.clone());
                }
            }
            f.paint(&mut s, &self.palette);
        }
        if self.umenu.visible {
            let mut f = load_form("TUMENUFORM");
            f.components[0].bounds = self.umenu.bounds();
            f.components[0].visible = true;
            for (name, bounds, image) in self.umenu.images() {
                let c = f.component_mut(name).unwrap();
                c.bounds = bounds;
                c.visible = true;
                if let Kind::Image { picture } = &mut c.kind {
                    *picture = Some(image.clone());
                }
            }
            f.paint(&mut s, &self.palette);
        }
        s
    }

    fn save(&self, what: &str) {
        if let Some(dir) = std::env::var_os("INFOBOX_PPM") {
            let name = if what == "umenu" {
                "umenu.ppm".to_owned()
            } else {
                format!("infobox-{what}.ppm")
            };
            write_ppm(&PathBuf::from(dir).join(name), &self.picture(), &self.palette);
        }
    }

    /// The squares the main screen shows, each with a point of `Hauptbild`
    /// the pointer picks it at, scanning the picture every 4 pixels.
    fn visible(&self) -> Vec<(Pos, (i32, i32))> {
        let cam = self.screen.camera();
        let mut out: Vec<(Pos, (i32, i32))> = Vec::new();
        for y in (60..340).step_by(4) {
            for x in (60..560).step_by(4) {
                let pos = square_at(&self.world, &self.tiles, &cam, x, y);
                if !out.iter().any(|(p, _)| *p == pos) {
                    out.push((pos, (x, y)));
                }
            }
        }
        out
    }

    /// The first visible square where `ok` holds.
    fn find(&self, ok: impl Fn(&World, Pos) -> bool) -> Option<(Pos, (i32, i32))> {
        self.visible().into_iter().find(|&(p, _)| ok(&self.world, p))
    }

    /// The view jumps to show `pos` near the middle.
    fn look_at(&mut self, pos: Pos) {
        self.screen.jump_to(pos, &self.world, &self.tiles);
    }

    fn line(&self, n: usize) -> String {
        let l = &self.box_.lines[n];
        // `TEXTE` is Windows-1252, which is Latin-1 for these letters.
        let s = |t: &[u8]| t.iter().map(|&b| char::from(b)).collect::<String>();
        format!("{}|{}|{}", s(&l.label), s(&l.value), s(&l.unit))
    }
}

/// The house's first square with guests: a hotel (kind 1) if there's one.
fn hotel_with_guests(w: &World) -> Option<Pos> {
    let guests = |h: &&game::world::House| h.guests() > 0 && w.tables.hausdata(h.building(), 5) == 1;
    w.houses.iter().find(guests).map(game::world::House::pos)
}

#[test]
#[allow(clippy::too_many_lines, clippy::many_single_char_names)] // One scripted game.
fn the_info_box_and_the_overview() {
    if !orig().join("SZENARIO").exists() {
        eprintln!("skipping: {} not found", orig().display());
        return;
    }
    let mut scene = Scene::new();
    assert_eq!(scene.tools.tool, tools::POINTER);

    // The human builds a small house (2 days) where it fits, and the
    // months go by.
    let house = 0;
    let (site, _) = scene
        .find(|w, p| w.can_build(p, house, 1).0 == 0)
        .expect("room for a house");
    let tick = scene.apply(Action::Build {
        building: house,
        at: site,
    });
    assert_eq!(tick.status, []);
    assert_eq!(scene.world.island.buildings.at(site), 31);
    // Two more for the overview.
    for _ in 0..2 {
        if let Some((more, _)) = scene.find(|w, p| w.can_build(p, house, 1).0 == 0) {
            scene.apply(Action::Build {
                building: house,
                at: more,
            });
        }
    }

    // The box over the building site, then the site finishes and closes it.
    let mouse = scene.find(|_, p| p == site).unwrap().1;
    scene.point(mouse, ShiftState::LEFT | ShiftState::DOUBLE);
    assert!(scene.box_.visible && scene.box_.modal);
    assert_eq!(scene.box_.kind, infobox::IB_BAUSTELLE);
    let lines: Vec<String> = (0..scene.box_.lines.len()).map(|n| scene.line(n)).collect();
    eprintln!("site: {lines:?}");
    assert!(lines[0].starts_with("|kleiner Zeltplatz|"), "{lines:?}");
    assert!(lines[3].starts_with("Fertig|in "), "{lines:?}");
    scene.save("site");
    // Off the map the original names the site's building (the box
    // doesn't look the square up), `infobox_text` the sea.
    for fix in [false, true] {
        scene.box_.enhance.set(Enhancement::InfoboxText, fix);
        let (world, font, info) = (&scene.world, &scene.font, &scene.info);
        scene
            .box_
            .show(world, font, info, (-5, -5), (100, 100), Scene::main_rect(), true);
        let lines: Vec<String> = (0..scene.box_.lines.len()).map(|n| scene.line(n)).collect();
        assert_eq!(lines.iter().any(|l| l.contains("kleiner Zeltplatz")), !fix, "{lines:?}");
        // Back to the site.
        scene.point(mouse, ShiftState::LEFT | ShiftState::DOUBLE);
    }
    scene.box_.enhance = Enhancements::ORIGINAL;
    // `infobox_size`: drawn again, the box keeps its size in the
    // original and takes the new one with the fix.
    let width = scene.box_.width;
    for fix in [false, true] {
        scene.box_.enhance.set(Enhancement::InfoboxSize, fix);
        scene.box_.width = 1;
        scene.box_.redraw(&scene.world, &scene.font, &scene.info);
        assert_eq!(scene.box_.width, if fix { width } else { 1 });
    }
    scene.box_.width = width;
    scene.box_.enhance = Enhancements::ORIGINAL;
    // Bribing the builders: 5000, and the days go down.
    let money = scene.world.finances.money[0];
    let days = scene.world.building_sites[0].days;
    let r = scene.press_button(infobox::BRIBE);
    assert_eq!(r.action, Some(Action::Bribe(site)));
    assert_eq!(scene.world.finances.money[0], money - 5000);
    assert!(scene.world.building_sites[0].days <= days);
    let mut n = 0;
    while scene.box_.visible {
        let tick = scene.tick();
        if !tick.finished_sites.is_empty() {
            assert!(!scene.box_.visible, "a finished site closes the box");
        }
        n += 1;
        assert!(n < 20_000, "the site never finished");
    }
    for _ in 0..4_000 {
        scene.tick();
    }

    // A hotel with guests, the human's own house, a road and the ground.
    let hotel = hotel_with_guests(&scene.world).expect("a hotel with guests");
    scene.look_at(hotel);
    let mouse = scene.find(|_, p| p == hotel).expect("the hotel in view").1;
    scene.point(mouse, ShiftState::LEFT);
    assert!(scene.box_.visible && !scene.box_.modal);
    let lines: Vec<String> = (0..scene.box_.lines.len()).map(|n| scene.line(n)).collect();
    eprintln!("hotel: {lines:?}");
    let h = scene.world.houses.iter().find(|h| h.pos() == hotel).unwrap().clone();
    let capacity = scene.world.tables.hausdata(h.building(), 3);
    assert_eq!(scene.box_.kind, 0, "IBHotel");
    assert!(lines[0].contains(&format!("   {}_*", h.stars())), "{lines:?}");
    assert!(
        lines
            .iter()
            .any(|l| l == &format!("Belegung:|{}/{capacity}|Url.", h.guests())),
        "{lines:?}"
    );
    let owner = scene.world.island.owners.at(hotel);
    let name: String = scene.world.player_names[usize::from(owner)]
        .iter()
        .map(|&b| char::from(b))
        .collect();
    assert!(lines.contains(&format!("Besitzer:|{name}|")), "{lines:?}");
    scene.save("hotel");
    // The button comes up: the box goes.
    scene.release();
    assert!(!scene.box_.visible);

    // A facility with visitors: how full it is and its price per visit.
    let facility = scene
        .world
        .facilities
        .iter()
        .find(|f| f.fill() > 0 && scene.world.tables.hausdata(f.building(), 5) == 2)
        .cloned()
        .expect("a restaurant with guests");
    scene.look_at(facility.pos());
    let mouse = scene.find(|_, p| p == facility.pos()).expect("in view").1;
    scene.point(mouse, ShiftState::LEFT);
    let lines: Vec<String> = (0..scene.box_.lines.len()).map(|n| scene.line(n)).collect();
    eprintln!("restaurant: {lines:?}");
    assert_eq!(scene.box_.kind, 2, "IBLokal");
    let capacity = i32::from(scene.world.tables.hausdata(facility.building(), 3));
    let visitors = i32::from(facility.fill()) * capacity / 100;
    assert!(
        lines.contains(&format!("Auslastung:|{visitors}/{capacity}|")),
        "{lines:?}"
    );
    assert!(
        lines.iter().any(|l| l.starts_with("Menüpreis:|") && l.ends_with("|DM")),
        "{lines:?}"
    );
    scene.save("restaurant");
    scene.release();

    // The human's own house, with buttons.
    scene.look_at(site);
    let mouse = scene.find(|_, p| p == site).unwrap().1;
    scene.point(mouse, ShiftState::LEFT | ShiftState::DOUBLE);
    scene.release();
    assert!(scene.box_.visible, "a double click keeps it open");
    assert_eq!(scene.box_.kind, 1, "IBHaus");
    let rent = scene.world.houses.iter().find(|h| h.pos() == site).unwrap().price();
    let r = scene.press_button(infobox::RAISE);
    assert_eq!(r.action, Some(Action::Price { at: site, raise: true }));
    let after = scene.world.houses.iter().find(|h| h.pos() == site).unwrap().price();
    assert_eq!(after, raise_rent(rent));
    let lines: Vec<String> = (0..scene.box_.lines.len()).map(|n| scene.line(n)).collect();
    eprintln!("house: {lines:?}");
    let shown = format_money(after / 4, scene.info.currency_factor, scene.info.language);
    let shown: String = shown.iter().map(|&b| char::from(b)).collect();
    assert!(lines.contains(&format!("Miete:|{shown}|DM/W")), "{lines:?}");
    let money = scene.world.finances.money[0];
    let condition = scene.world.houses.iter().find(|h| h.pos() == site).unwrap().condition();
    let price = scene.world.renovation_price(house, condition);
    scene.press_button(infobox::RENOVATE);
    assert_eq!(scene.world.finances.money[0], money - price);
    assert_eq!(
        scene.world.houses.iter().find(|h| h.pos() == site).unwrap().condition(),
        0x7f
    );
    scene.save("house");
    // Selling makes event 9 certain.
    let r = scene.press_button(infobox::SELL);
    assert_eq!(r.action, Some(Action::Sell(site)));
    assert_eq!(scene.world.events[9].chance, 0x3fffc);
    assert_eq!(scene.world.events[9].pos, site);
    // "Übersicht": the box closes and the overview opens.
    let r = scene.press_button(infobox::OVERVIEW);
    assert!(r.closed && r.open_overview && !scene.box_.visible);
    scene.umenu.open(&scene.world, &scene.font, &scene.info, SCREEN);
    assert!(scene.umenu.visible);
    assert_eq!(scene.umenu.kind, 0);
    assert!(scene.umenu.list.contains(&site), "{:?}", scene.umenu.list);
    let row = scene.umenu.list.iter().position(|&p| p == site).unwrap();
    // Select the house's row, then "+" and "Springe hin".
    let (x, y) = (20, 0x30 + 16 * i32::try_from(row).unwrap() + 4);
    let r = scene
        .umenu
        .mouse_down(x, y, ShiftState::LEFT, &scene.world, &scene.font, &scene.info);
    assert!(r.refresh);
    scene.umenu.mouse_up(x, y, &scene.world, &scene.font, &scene.info);
    assert_eq!(scene.umenu.selected, i32::try_from(row).unwrap());
    let rent = scene.world.houses.iter().find(|h| h.pos() == site).unwrap().price();
    let r = scene
        .umenu
        .key_down(0xbb, ShiftState::default(), &scene.world, &scene.font, &scene.info);
    assert_eq!(r.action, Some(Action::Price { at: site, raise: true }));
    assert_eq!(r.redraw, Some(Redraw::Kind));
    scene.apply(r.action.unwrap());
    scene.umenu.redraw(Redraw::Kind, &scene.world, &scene.font, &scene.info);
    assert_eq!(
        scene.world.houses.iter().find(|h| h.pos() == site).unwrap().price(),
        raise_rent(rent)
    );
    // The minus button with the mouse.
    let (mx, my) = (0xb6 + 8, 0x109 + 8);
    let down = scene
        .umenu
        .mouse_down(mx, my, ShiftState::LEFT, &scene.world, &scene.font, &scene.info);
    assert_eq!(down.sounds, vec![8]);
    let up = scene.umenu.mouse_up(mx, my, &scene.world, &scene.font, &scene.info);
    assert_eq!(up.action, Some(Action::Price { at: site, raise: false }));
    assert_eq!(up.redraw, Some(Redraw::List));
    scene.apply(up.action.unwrap());
    scene.umenu.redraw(Redraw::List, &scene.world, &scene.font, &scene.info);
    // Enter jumps to the house.
    let r = scene
        .umenu
        .key_down(0x0d, ShiftState::default(), &scene.world, &scene.font, &scene.info);
    assert_eq!(r.jump, Some(site));
    scene.save("umenu");
    // With fewer than 12 buildings v1.0's Up put the list's top before
    // its start; the later build keeps it at 0, with `overview_list` or
    // not (en 1088:2040). A click on column 299 selects a row only in the
    // original.
    assert!(scene.umenu.list.len() >= 3, "{:?}", scene.umenu.list);
    for fix in [false, true] {
        let u = &mut scene.umenu;
        u.enhance.set(Enhancement::OverviewList, fix);
        u.selected = 2;
        let _ = u.key_down(0x26, ShiftState::default(), &scene.world, &scene.font, &scene.info);
        assert_eq!(u.selected, 1);
        assert!(u.top_row() >= 0, "{}", u.top_row());
        let _ = u.key_down(0x28, ShiftState::default(), &scene.world, &scene.font, &scene.info);
        assert_eq!(u.selected, 2);
        u.selected = 0;
        let _ = u.mouse_down(
            299,
            0x30 + 16 + 4,
            ShiftState::LEFT,
            &scene.world,
            &scene.font,
            &scene.info,
        );
        let _ = u.mouse_up(299, 0x30 + 16 + 4, &scene.world, &scene.font, &scene.info);
        assert_eq!(u.selected != 0, !fix);
    }
    scene.umenu.enhance = Enhancements::ORIGINAL;
    // The hotels' kind with the key Right, and back past the roads' with
    // Left.
    scene
        .umenu
        .key_down(0x27, ShiftState::default(), &scene.world, &scene.font, &scene.info);
    assert_eq!(scene.umenu.kind, 1);
    for _ in 0..3 {
        scene
            .umenu
            .key_down(0x25, ShiftState::default(), &scene.world, &scene.font, &scene.info);
    }
    assert_eq!(scene.umenu.kind, 10);
    for _ in 0..4 {
        scene
            .umenu
            .key_down(0x25, ShiftState::default(), &scene.world, &scene.font, &scene.info);
    }
    assert_eq!(scene.umenu.kind, 5, "kind 7 skipped");
    let r = scene
        .umenu
        .key_down(0x1b, ShiftState::default(), &scene.world, &scene.font, &scene.info);
    assert!(r.closed && !scene.umenu.visible);

    // A road and the bare ground, without buttons.
    let is_road = |w: &World, p: Pos| (10..=0x36).contains(&w.island.ground.at(p));
    let any = (0..251)
        .flat_map(|i| (0..251).map(move |j| (i, j)))
        .find(|&p| is_road(&scene.world, p) && scene.world.island.owners.at(p) > 1);
    if let Some(any) = any {
        scene.look_at(any);
    }
    let road = scene.find(is_road);
    assert!(road.is_some(), "a road in view");
    if let Some((road, mouse)) = road {
        scene.point(mouse, ShiftState::LEFT);
        assert_eq!(scene.box_.square, road);
        assert_eq!(scene.box_.kind, infobox::IB_STRASSE);
        let lines: Vec<String> = (0..scene.box_.lines.len()).map(|n| scene.line(n)).collect();
        eprintln!("road: {lines:?}");
        assert!(lines[0].contains("Straße"), "{lines:?}");
        scene.save("road");
        scene.release();
    }
    let (ground, mouse) = scene
        .find(|w, p| {
            w.island.buildings.at(p) == NO_BUILDING && w.island.ground.at(p) == 1 && w.island.owners.at(p) == 0
        })
        .expect("grass");
    scene.point(mouse, ShiftState::LEFT | ShiftState::DOUBLE);
    assert_eq!(scene.box_.kind, infobox::IB_GROUND);
    let lines: Vec<String> = (0..scene.box_.lines.len()).map(|n| scene.line(n)).collect();
    eprintln!("ground: {lines:?}");
    let height = i32::from(scene.world.island.highest_corner(ground)) * 3;
    assert_eq!(lines, [String::from("|Grasland|"), format!("Hoehe:|{height}|müNN")]);
    scene.save("ground");
    // The island's info, where the box is.
    let (left, top) = (scene.box_.left, scene.box_.top);
    scene.press_button(infobox::ISLAND_INFO);
    assert_eq!(scene.box_.kind, infobox::IB_INSEL);
    assert_eq!((scene.box_.left, scene.box_.top), (left, top));
    let lines: Vec<String> = (0..scene.box_.lines.len()).map(|n| scene.line(n)).collect();
    eprintln!("island: {lines:?}");
    assert!(lines[0].starts_with("INSEL-INFORMATION"), "{lines:?}");
    scene.save("island");
    // OK closes it; Escape would too.
    let r = scene.press_button(infobox::OK);
    assert!(r.closed && !scene.box_.visible);
    scene.point(mouse, ShiftState::LEFT | ShiftState::DOUBLE);
    assert!(
        scene
            .box_
            .key_down(0x1b, ShiftState::default())
            .is_some_and(|r| r.closed)
    );
    assert!(scene.box_.key_down(0x41, ShiftState::default()).is_none());
}

#[test]
fn renovating_and_bribing_need_the_money() {
    if !orig().join("SZENARIO").exists() {
        eprintln!("skipping: {} not found", orig().display());
        return;
    }
    let mut scene = Scene::new();
    let (site, _) = scene.find(|w, p| w.can_build(p, 0, 1).0 == 0).unwrap();
    scene.apply(Action::Build { building: 0, at: site });
    let mut poor = scene.world.clone();
    poor.finances.money[0] = 100;
    let before = poor.clone();
    let tick = poor.apply(human(Action::Bribe(site)));
    assert_eq!(tick.status, vec![(1, Status::Error(0xe))]);
    assert_eq!(poor, before);
    // A computer's building: nothing.
    if let Some(theirs) = scene
        .world
        .houses
        .iter()
        .map(game::world::House::pos)
        .find(|&p| scene.world.island.owners.at(p) > 1)
    {
        let before = scene.world.clone();
        scene.world.apply(human(Action::Renovate(theirs)));
        scene.world.apply(human(Action::Price {
            at: theirs,
            raise: true,
        }));
        scene.world.apply(human(Action::Sell(theirs)));
        assert_eq!(scene.world, before);
    }
}
