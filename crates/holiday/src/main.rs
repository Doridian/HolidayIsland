//! Holiday Island. For now this shows the main form with the title picture
//! and then the background, like the end of the intro (`docs/startup.md`,
//! states 430–450), and then the start menu on top (state 500). Start
//! loads the scenario (or, until island generation is ported, the tutorial
//! island) and shows the main screen with the island, which the arrow keys
//! scroll and Insert and Delete turn, and runs the simulation's clock
//! (`World::tick`, every 100 ms), with the date and money on the info bar.
//! When an event starts an auction the auction form opens over it; its
//! buttons and keys become commands to the world. Loading a game is only
//! logged.
//!
//! `holiday [GAME_DIR]`, where `GAME_DIR` holds `HOLIDAY.EXE`, `GRAFIK`,
//! `DATA` and `TEXTE` (default `orig`).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use formats::bmp::Bitmap;
use formats::data::{
    animdat1, animdat4, auktion, darstell, flusspos, fontdat, gfxpos, groundnr, grounpos, hausdata, hausnr, hauspos,
    hausprei, message_media_names, modatyp, monate, sonstige, start_txt, straspos, szenario_txt,
};
use formats::dfm::Object;
use formats::ne::{RT_RCDATA, ResId, resources};
use formats::savegame::SaveGame;
use game::auction::{self, Change};
use game::auktionsform::{self, Auktionsform};
use game::command::{self, Command};
use game::darstellung::{self, Camera, Tiles};
use game::hauptschirm::{self, Hauptschirm, InfoTexts};
use game::hintergrund::{self, Island};
use game::simulation::{TICK_MS, Tick};
use game::startform::{self, Action, Settings, StartForm};
use game::text::Font;
use game::widgets::ShiftState;
use game::world::{Tables, World};
use platform::{App, Button, Frame, Input};
use vcl::form::{Form, Kind};
use vcl::random::Random;
use vcl::surface::Surface;
use vcl::{Palette, Rect};

/// The layout `hintergrund_timer` picks for an 800×600 screen.
const SCREEN: (usize, usize) = (800, 600);
/// How long the title stays up (state 440's interval).
const TITLE_TIME: Duration = Duration::from_secs(3);
const VK_SHIFT: u16 = 0x10;
const VK_CONTROL: u16 = 0x11;
const VK_MENU: u16 = 0x12;
const VK_ESCAPE: u16 = 0x1b;

/// `TStartform`: the game's widgets, and the VCL form whose `Bild` shows
/// them.
struct StartMenu {
    menu: StartForm,
    form: Form,
    /// The mouse went down on `Bild`, which captures it until every button
    /// is up again, as the VCL does.
    captured: bool,
}

impl StartMenu {
    /// What `startform_show` does: centre the form, draw the menu and show
    /// it. // 1058:1a30
    fn show(&mut self) {
        let (w, h) = (i32::try_from(SCREEN.0).unwrap(), i32::try_from(SCREEN.1).unwrap());
        let (left, top) = StartForm::position(w, h);
        self.form.components[0].bounds = Rect::new(left, top, startform::WIDTH, startform::HEIGHT);
        self.form.components[0].visible = true;
        if let Some(bild) = self.form.component_mut("Bild") {
            bild.bounds = Rect::new(0, 0, startform::WIDTH, startform::HEIGHT);
        }
        self.menu.show();
    }

    fn hide(&mut self) {
        self.form.components[0].visible = false;
    }

    fn visible(&self) -> bool {
        self.form.form().visible
    }

    /// `x`, `y` on the screen in `Bild`'s coordinates.
    fn to_bild(&self, x: i32, y: i32) -> (i32, i32) {
        let form = self.form.form().bounds;
        let bild = self.form.component("Bild").map_or(Rect::default(), |b| b.bounds);
        (x - form.left - bild.left, y - form.top - bild.top)
    }

    fn inside(x: i32, y: i32) -> bool {
        (0..startform::WIDTH).contains(&x) && (0..startform::HEIGHT).contains(&y)
    }

    fn paint(&mut self, screen: &mut Surface, palette: &Palette) {
        if let Some(bild) = self.form.component_mut("Bild")
            && let Kind::Image { picture } = &mut bild.kind
        {
            *picture = Some(self.menu.bild().clone());
        }
        self.form.paint(screen, palette);
    }
}

/// A game in progress: the world and the main screen showing it.
struct Game {
    world: World,
    tiles: Tiles,
    screen: Hauptschirm,
    font: Font,
    info: InfoTexts,
    /// Time since the last tick of the clock.
    since_tick: Duration,
    /// `THauptschirm`'s VCL form, whose images show `screen`'s.
    form: Form,
    /// `TAuktionsform`, and its VCL form.
    auction: Auktionsform,
    auction_form: Form,
    /// `FILENAME.DAT`: each message's picture, the auction's item's too.
    pictures: Vec<Vec<u8>>,
    dir: PathBuf,
    /// The auction form has the mouse captured.
    auction_captured: bool,
    /// The player's commands, applied before the next tick.
    commands: Vec<Command>,
}

impl Game {
    /// Runs the player's commands and the clock's ticks that are due
    /// (`AnimationsTimer`, 1020:1e9b, and the auction form's timers) and
    /// says whether the screen changed.
    fn update(&mut self, elapsed: Duration) -> bool {
        let tick = Duration::from_millis(u64::from(TICK_MS));
        self.since_tick += elapsed;
        let mut ticked = false;
        let mut changed = false;
        for command in std::mem::take(&mut self.commands) {
            let t = self.world.apply(command);
            changed |= self.show(&t);
        }
        while self.since_tick >= tick {
            self.since_tick -= tick;
            // The original's people code reads the player's view.
            self.world.set_viewpoint(self.screen.viewpoint());
            let t = self.world.tick();
            changed |= self.show(&t);
            if self.auction.visible && self.world.auction.open {
                self.auction.draw_auctioneer(&self.world); // 1028:0221
                changed = true;
            }
            ticked = true;
        }
        let ms = u32::try_from(elapsed.as_millis()).unwrap_or(u32::MAX);
        changed |= self.auction.advance(ms, &self.world, &self.font, &self.info);
        if ticked {
            // 1088:385d, 1088:38c4: the people where they moved to.
            self.screen
                .show_people(&self.world, &self.tiles, &self.font, &self.info);
            changed = true;
        }
        let info = ticked && self.screen.show_info(&self.world, &self.font, &self.info);
        changed || info
    }

    /// Shows what a tick or command did, and says whether anything changed.
    fn show(&mut self, t: &Tick) -> bool {
        let mut changed = if t.redraw_all {
            self.screen.paint_map(&self.world, &self.tiles);
            true
        } else {
            self.screen.redraw_squares(&t.redraw, &self.world, &self.tiles)
        };
        if t.auction_changes.contains(&Change::Opened) {
            self.open_auction();
        }
        if !t.auction_changes.is_empty() {
            self.auction
                .update(&self.world, &t.auction_changes, &self.font, &self.info);
            changed = true;
        }
        changed
    }

    /// The auction form opens with the item's picture. // 1028:0dde
    fn open_auction(&mut self) {
        let item = usize::try_from(self.world.auction.item).ok();
        let picture = item.and_then(|n| self.pictures.get(n)).and_then(|name| {
            let path = self.dir.join("GRAFIK").join(auktionsform::picture_file(name));
            let bitmap = Bitmap::parse(&std::fs::read(path).ok()?).ok()?;
            Some(Surface::from_bitmap(&bitmap))
        });
        let screen = (i32::try_from(SCREEN.0).unwrap(), i32::try_from(SCREEN.1).unwrap());
        self.auction
            .open(&self.world, &self.font, &self.info, picture.as_ref(), screen);
        self.auction_captured = false;
    }

    /// The auction form's buttons as the human's commands (the original's
    /// player 1).
    fn auction_actions(&mut self, actions: Vec<auction::Action>) {
        self.commands.extend(actions.into_iter().map(|a| Command {
            player: 1,
            action: command::Action::Auction(a),
        }));
    }

    /// A mouse event on the auction form while it's shown, and whether it
    /// took it.
    fn auction_mouse(&mut self, input: Input, shift: ShiftState, buttons_up: bool) -> bool {
        if !self.auction.visible {
            return false;
        }
        let (x, y) = match input {
            Input::MouseMove { x, y } | Input::MouseDown { x, y, .. } | Input::MouseUp { x, y, .. } => {
                (x - self.auction.left, y - self.auction.top)
            }
            _ => return false,
        };
        let over = self.auction_captured || self.auction.contains(x, y);
        if !over {
            return false;
        }
        let screen = (i32::try_from(SCREEN.0).unwrap(), i32::try_from(SCREEN.1).unwrap());
        match input {
            Input::MouseDown { .. } => {
                self.auction_captured = true;
                let actions = self
                    .auction
                    .mouse_down(x, y, shift, &self.world, &self.font, &self.info);
                self.auction_actions(actions);
            }
            Input::MouseUp { .. } => {
                if buttons_up {
                    self.auction_captured = false;
                }
                let actions = self.auction.mouse_up(x, y, &self.world, &self.font);
                self.auction_actions(actions);
            }
            _ => {
                self.auction.mouse_move(x, y, shift, screen);
            }
        }
        true
    }

    fn paint(&mut self, screen: &mut Surface, palette: &Palette) {
        for (name, image) in self.screen.images() {
            if let Some(c) = self.form.component_mut(name)
                && let Kind::Image { picture } = &mut c.kind
            {
                *picture = Some(image.clone());
            }
        }
        self.form.paint(screen, palette);
        self.auction_form.components[0].bounds = self.auction.bounds();
        self.auction_form.components[0].visible = self.auction.visible;
        for (name, image) in self.auction.images() {
            if let Some(c) = self.auction_form.component_mut(name)
                && let Kind::Image { picture } = &mut c.kind
            {
                *picture = Some(image.clone());
            }
        }
        self.auction_form.paint(screen, palette);
    }
}

struct Holiday {
    dir: PathBuf,
    exe: Vec<u8>,
    form: Form,
    game: Option<Game>,
    background: Bitmap,
    start: StartMenu,
    palette: Palette,
    rgb: [[u8; 3]; 256],
    screen: Surface,
    elapsed: Duration,
    showing_title: bool,
    /// Mouse buttons and Shift, Ctrl and Alt, as a `TShiftState`.
    buttons: ShiftState,
    keys: ShiftState,
    quit: bool,
}

impl Holiday {
    fn set_picture(&mut self, bitmap: &Bitmap) {
        self.palette = bitmap.palette;
        self.rgb = bitmap.palette.map(|c| [c.r, c.g, c.b]);
        if let Some(image) = self.form.component_mut("backgroundimage")
            && let Kind::Image { picture } = &mut image.kind
        {
            *picture = Some(Surface::from_bitmap(bitmap));
        }
        self.repaint();
    }

    fn repaint(&mut self) {
        self.screen.pixels.fill(0);
        self.form.paint(&mut self.screen, &self.palette);
        if let Some(game) = &mut self.game {
            game.paint(&mut self.screen, &self.palette);
        }
        self.start.paint(&mut self.screen, &self.palette);
    }

    fn shift(&self) -> ShiftState {
        self.buttons | self.keys
    }

    /// Passes a mouse event on to `Bild` if it's over it or has the mouse
    /// captured.
    fn mouse(&mut self, input: Input) {
        if !self.start.visible() {
            self.game_mouse(input);
            return;
        }
        let (x, y, button) = match input {
            Input::MouseMove { x, y } => (x, y, None),
            Input::MouseDown { x, y, button } | Input::MouseUp { x, y, button } => (x, y, Some(button)),
            _ => return,
        };
        let bit = button.map_or(ShiftState::default(), |b| match b {
            Button::Left => ShiftState::LEFT,
            Button::Right => ShiftState::RIGHT,
            Button::Middle => ShiftState::MIDDLE,
        });
        let (x, y) = self.start.to_bild(x, y);
        let over = self.start.captured || StartMenu::inside(x, y);
        let response = match input {
            Input::MouseDown { .. } => {
                self.buttons = self.buttons | bit;
                if !over {
                    return;
                }
                self.start.captured = true;
                self.start.menu.mouse_down(x, y, self.shift())
            }
            Input::MouseUp { .. } => {
                self.buttons = ShiftState(self.buttons.0 & !bit.0);
                let mouse = ShiftState::LEFT | ShiftState::RIGHT | ShiftState::MIDDLE;
                if self.buttons.0 & mouse.0 == 0 {
                    self.start.captured = false;
                }
                if !over {
                    return;
                }
                self.start.menu.mouse_up(x, y)
            }
            _ if over => self.start.menu.mouse_move(x, y, self.shift()),
            _ => return,
        };
        self.respond(response);
    }

    /// A mouse event in the game: so far only the auction form takes them.
    fn game_mouse(&mut self, input: Input) {
        let bit = |b: Button| match b {
            Button::Left => ShiftState::LEFT,
            Button::Right => ShiftState::RIGHT,
            Button::Middle => ShiftState::MIDDLE,
        };
        match input {
            Input::MouseDown { button, .. } => self.buttons = self.buttons | bit(button),
            Input::MouseUp { button, .. } => self.buttons = ShiftState(self.buttons.0 & !bit(button).0),
            _ => {}
        }
        let shift = self.shift();
        let mouse = ShiftState::LEFT | ShiftState::RIGHT | ShiftState::MIDDLE;
        let up = self.buttons.0 & mouse.0 == 0;
        if let Some(game) = &mut self.game
            && game.auction_mouse(input, shift, up)
        {
            self.repaint();
        }
    }

    fn respond(&mut self, response: startform::Response) {
        match response.action {
            Some(Action::Start { scenario }) => {
                // `startform_start_click` hides the form, and hintergrund's
                // states 1000-1500 start the game.
                self.start.hide();
                self.game = Some(start_game(&self.dir, &self.exe, self.start.menu.settings(), scenario));
            }
            Some(Action::Load) => println!("load a saved game (TLadenForm): not ported yet"),
            Some(Action::Quit) => self.quit = true,
            None => {}
        }
        if response.refresh || response.action.is_some() {
            self.repaint();
        }
    }
}

impl App for Holiday {
    fn update(&mut self, elapsed: Duration) {
        self.elapsed += elapsed;
        if let Some(game) = &mut self.game
            && game.update(elapsed)
        {
            self.repaint();
        }
        if self.showing_title && self.elapsed >= TITLE_TIME {
            // States 450 and 500.
            self.showing_title = false;
            let background = self.background.clone();
            self.start.show();
            self.set_picture(&background);
        }
    }

    fn input(&mut self, input: Input) {
        match input {
            Input::Close => self.quit = true,
            Input::KeyDown(VK_ESCAPE) if !self.game.as_ref().is_some_and(|g| g.auction.visible) => self.quit = true,
            Input::KeyDown(key) | Input::KeyUp(key) => {
                let down = matches!(input, Input::KeyDown(_));
                let bit = match key {
                    VK_SHIFT => ShiftState::SHIFT,
                    VK_CONTROL => ShiftState::CTRL,
                    VK_MENU => ShiftState::ALT,
                    _ => ShiftState::default(),
                };
                if down {
                    self.keys = self.keys | bit;
                    if self.start.visible() {
                        let response = self.start.menu.key_down(key, self.keys);
                        self.respond(response);
                    } else if let Some(game) = &mut self.game
                        && game.auction.visible
                    {
                        // `TAuktionsform` is on top and has the keys.
                        let shift = self.buttons | self.keys;
                        if let Some(action) = game.auction.key_down(key, shift) {
                            game.auction_actions(vec![action]);
                        }
                    } else if let Some(game) = &mut self.game {
                        let shift = self.buttons | self.keys;
                        if game.screen.key_down(key, shift, &game.world, &game.tiles) == hauptschirm::KeyResult::Redrawn
                        {
                            game.screen
                                .show_people(&game.world, &game.tiles, &game.font, &game.info);
                            self.repaint();
                        }
                    }
                } else {
                    self.keys = ShiftState(self.keys.0 & !bit.0);
                }
            }
            _ => self.mouse(input),
        }
    }

    fn frame(&self) -> Frame<'_> {
        Frame {
            width: u32::try_from(self.screen.width).expect("screen width"),
            height: u32::try_from(self.screen.height).expect("screen height"),
            pixels: &self.screen.pixels,
            palette: &self.rgb,
        }
    }

    fn finished(&self) -> bool {
        self.quit
    }
}

fn read(dir: &Path, name: &str) -> Vec<u8> {
    let path = dir.join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("can't read {}: {e}", path.display()))
}

fn bitmap(dir: &Path, name: &str) -> Bitmap {
    Bitmap::parse(&read(dir, name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn load_form(exe: &[u8], class: &str) -> Form {
    let res = resources(exe)
        .expect("HOLIDAY.EXE isn't an NE executable")
        .into_iter()
        .find(|r| r.kind == ResId::Id(RT_RCDATA) && r.name == ResId::Name(class.as_bytes().to_vec()))
        .unwrap_or_else(|| panic!("no {class} form"));
    Form::load(&Object::parse(res.data).expect("bad form")).expect("bad form")
}

/// The start menu as `TStartform.Create` and `startform_init` make it.
/// // 1058:27e6
fn start_menu(dir: &Path, exe: &[u8]) -> StartMenu {
    let font = Font::new(
        &bitmap(dir, "GRAFIK/FONT.BMP"),
        &fontdat(&read(dir, "DATA/FONTDAT1.DAT")).expect("FONTDAT1.DAT"),
        &fontdat(&read(dir, "DATA/FONTDAT2.DAT")).expect("FONTDAT2.DAT"),
    );
    let bitmaps = startform::BITMAP_FILES
        .iter()
        .map(|f| Surface::from_bitmap(&bitmap(dir, &format!("GRAFIK/{}", f.to_ascii_uppercase()))))
        .collect();
    let menu = StartForm::new(
        font,
        bitmaps,
        &start_txt(&read(dir, "TEXTE/START.TXT")),
        &szenario_txt(&read(dir, "TEXTE/SZENARIO.TXT")),
        Settings::default(),
    );
    StartMenu {
        menu,
        form: load_form(exe, "TSTARTFORM"),
        captured: false,
    }
}

/// The tables and bitmaps the island is drawn with (1030:3932).
fn load_tiles(dir: &Path) -> Tiles {
    let data = |name: &str| read(dir, &format!("DATA/{name}"));
    Tiles::new(
        hauspos(&data("HAUSPOS.DAT")).expect("HAUSPOS.DAT"),
        hausnr(&data("HAUSNR.DAT")).expect("HAUSNR.DAT"),
        grounpos(&data("GROUNPOS.DAT")).expect("GROUNPOS.DAT"),
        groundnr(&data("GROUNDNR.DAT")).expect("GROUNDNR.DAT"),
        gfxpos(&data("GFXPOS.DAT")).expect("GFXPOS.DAT"),
        straspos(&data("STRASPOS.DAT")).expect("STRASPOS.DAT"),
        flusspos(&data("FLUSSPOS.DAT")).expect("FLUSSPOS.DAT"),
        darstell(&data("DARSTELL.DAT")).expect("DARSTELL.DAT"),
        darstellung::BITMAP_FILES
            .iter()
            .map(|(_, f)| Surface::from_bitmap(&bitmap(dir, &format!("GRAFIK/{}", f.to_ascii_uppercase()))))
            .collect(),
    )
}

/// The tables the simulation reads (1030:2e48), for a scenario's world.
fn world_tables(dir: &Path) -> Arc<Tables> {
    Arc::new(Tables {
        hausnr: hausnr(&read(dir, "DATA/HAUSNR.DAT")).expect("HAUSNR.DAT"),
        hausdata: hausdata(&read(dir, "DATA/HAUSDATA.DAT")).expect("HAUSDATA.DAT"),
        hausprei: hausprei(&read(dir, "DATA/HAUSPREI.DAT")).expect("HAUSPREI.DAT"),
        groundnr: groundnr(&read(dir, "DATA/GROUNDNR.DAT")).expect("GROUNDNR.DAT"),
        animdat1: animdat1(&read(dir, "DATA/ANIMDAT1.DAT")).expect("ANIMDAT1.DAT"),
        animdat4: animdat4(&read(dir, "DATA/ANIMDAT4.DAT")).expect("ANIMDAT4.DAT"),
        form_korrektur: darstell(&read(dir, "DATA/DARSTELL.DAT"))
            .expect("DARSTELL.DAT")
            .form_korrektur,
        road_cost: modatyp(&read(dir, "DATA/MODATYP.DAT"))
            .expect("MODATYP.DAT")
            .strassenbaukosten,
    })
}

/// The world a game starts with and the main screen's camera: a
/// scenario's map (and its view origin) or a generated island. // 1000:2466
fn start_world(dir: &Path, settings: &Settings, scenario_page: bool) -> (World, Camera, Option<(i16, i16)>) {
    // `Randomize` (1108:1e21) seeds RandSeed from the clock.
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| {
            d.subsec_nanos() ^ u32::try_from(d.as_secs() & 0xffff_ffff).unwrap_or(0)
        });
    match hintergrund::new_island(settings, scenario_page) {
        Island::Scenario(file) => {
            let path = format!("SZENARIO/{}", file.to_ascii_uppercase());
            let map = SaveGame::parse(&read(dir, &path)).unwrap_or_else(|e| panic!("{path}: {e}"));
            let mut world =
                hintergrund::load_world(&map, settings, scenario_page).unwrap_or_else(|e| panic!("{path}: {e}"));
            world.random = Random::new(seed);
            world.tables = world_tables(dir);
            let byte = |dest| map.field(dest).map_or(0, |d| d[0]);
            let origin = map.field("1110:5530").map_or((0, 0), |d| {
                (i16::from_le_bytes([d[0], d[1]]), i16::from_le_bytes([d[2], d[3]]))
            });
            let camera = Camera {
                view: byte("1110:007c"),
                origin,
                parity: 0,
                owner_pointers: byte("1110:0058") != 0,
                occupancy_pointers: byte("1110:0059") != 0,
            };
            (world, camera, Some(origin))
        }
        Island::Generated => {
            let data = hintergrund::GameData::load(|name| std::fs::read(dir.join(name)).ok())
                .unwrap_or_else(|e| panic!("{e}"));
            // States 100, 1210 and 1220.
            let world = hintergrund::new_world(settings, &data, seed);
            // The options as the initialised data has them (1000:1ea0).
            let camera = Camera {
                view: 0,
                origin: world.viewpoint.origin,
                parity: world.viewpoint.parity,
                owner_pointers: false,
                occupancy_pointers: true,
            };
            (world, camera, None)
        }
    }
}

/// What hintergrund's timer does from state 1000 to 1500 once Start is
/// clicked, without the video (`titel.avi`), the progress window
/// (`TWasNunForm`) and what isn't ported yet: load the scenario's map or
/// generate an island, set up the main screen and show it. // 1000:2466
fn start_game(dir: &Path, exe: &[u8], settings: &Settings, scenario_page: bool) -> Game {
    let (world, camera, origin) = start_world(dir, settings, scenario_page);
    let font = Font::new(
        &bitmap(dir, "GRAFIK/FONT.BMP"),
        &fontdat(&read(dir, "DATA/FONTDAT1.DAT")).expect("FONTDAT1.DAT"),
        &fontdat(&read(dir, "DATA/FONTDAT2.DAT")).expect("FONTDAT2.DAT"),
    );
    let texts = sonstige(&read(dir, "TEXTE/SONSTIGE.TXT")).expect("SONSTIGE.TXT");
    let bitmaps = hauptschirm::BITMAP_FILES
        .iter()
        .map(|f| Surface::from_bitmap(&bitmap(dir, &format!("GRAFIK/{}", f.to_ascii_uppercase()))))
        .collect();
    let title = texts.texts.first().cloned().unwrap_or_default();
    let mut screen = match origin {
        Some(origin) => Hauptschirm::new(camera, origin, bitmaps, &font, &title),
        None => Hauptschirm::new_game(camera, bitmaps, &font, &title),
    };
    let tiles = load_tiles(dir);
    screen.paint_map(&world, &tiles);
    let info = InfoTexts {
        months: monate(&read(dir, "TEXTE/MONATE.TXT")).expect("MONATE.TXT"),
        land: modatyp(&read(dir, "DATA/MODATYP.DAT")).expect("MODATYP.DAT").land,
        currency_factor: texts.waehrungsfaktor,
        currency: texts.texts.get(5).cloned().unwrap_or_default(), // 1020:3ea6
    };
    screen.show_info(&world, &font, &info);

    // 1020:1ce9 and 1020:283b: centred, laid out and shown.
    let mut form = load_form(exe, "THAUPTSCHIRM");
    let (w, h) = (i32::try_from(SCREEN.0).unwrap(), i32::try_from(SCREEN.1).unwrap());
    let (left, top) = Hauptschirm::position(w, h);
    form.components[0].bounds = Rect::new(left, top, hauptschirm::WIDTH, hauptschirm::HEIGHT);
    form.components[0].visible = true;
    for (name, bounds) in hauptschirm::LAYOUT {
        if let Some(c) = form.component_mut(name) {
            c.bounds = bounds;
            c.visible = true;
        }
    }
    let auction_bitmaps = auktionsform::BITMAP_FILES
        .iter()
        .map(|f| Surface::from_bitmap(&bitmap(dir, &format!("GRAFIK/{}", f.to_ascii_uppercase()))))
        .collect();
    let auction = Auktionsform::new(auction_bitmaps, auktion(&read(dir, "TEXTE/AUKTION.TXT")));
    let mut auction_form = load_form(exe, "TAUKTIONSFORM");
    for (name, bounds) in auktionsform::LAYOUT {
        if let Some(c) = auction_form.component_mut(name) {
            c.bounds = bounds;
            c.visible = true;
        }
    }
    Game {
        world,
        tiles,
        screen,
        font,
        info,
        since_tick: Duration::ZERO,
        form,
        auction,
        auction_form,
        pictures: message_media_names(&read(dir, "DATA/FILENAME.DAT")),
        dir: dir.to_path_buf(),
        auction_captured: false,
        commands: Vec::new(),
    }
}

fn main() {
    let dir = std::env::args_os()
        .nth(1)
        .map_or_else(|| PathBuf::from("orig"), PathBuf::from);
    let exe = read(&dir, "HOLIDAY.EXE");
    let mut form = load_form(&exe, "THINTERGRUND");

    // What states 110 and 400 do to the form and its image.
    let (w, h) = (i32::try_from(SCREEN.0).unwrap(), i32::try_from(SCREEN.1).unwrap());
    form.components[0].bounds = Rect::new(0, 0, w, h);
    form.components[0].visible = true;
    if let Some(image) = form.component_mut("backgroundimage") {
        image.bounds = Rect::new(0, 0, w, h);
        image.visible = true;
    }

    let title = bitmap(&dir, "GRAFIK/TIT80X60.BMP");
    let start = start_menu(&dir, &exe);
    let background = bitmap(&dir, "GRAFIK/BCK80X60.BMP");
    let mut holiday = Holiday {
        dir,
        exe,
        form,
        game: None,
        background,
        start,
        palette: title.palette,
        rgb: [[0; 3]; 256],
        screen: Surface::new(SCREEN.0, SCREEN.1, 0),
        elapsed: Duration::ZERO,
        showing_title: true,
        buttons: ShiftState::default(),
        keys: ShiftState::default(),
        quit: false,
    };
    holiday.set_picture(&title);
    if let Err(e) = platform::run("Holiday Island", holiday) {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
