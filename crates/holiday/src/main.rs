//! Holiday Island. For now this shows the main form with the title picture
//! and then the background, like the end of the intro (`docs/startup.md`,
//! states 430–450), and then the start menu on top (state 500). Start
//! loads the scenario (or, until island generation is ported, the tutorial
//! island) and shows the main screen with the island, which the arrow keys
//! scroll and Insert and Delete turn, and runs the simulation's clock
//! (`World::tick`, every 100 ms), with the date and money on the info bar.
//! Loading a game is only logged.
//!
//! `holiday [GAME_DIR]`, where `GAME_DIR` holds `HOLIDAY.EXE`, `GRAFIK`,
//! `DATA` and `TEXTE` (default `orig`).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use formats::bmp::Bitmap;
use formats::data::{
    darstell, flusspos, fontdat, gfxpos, groundnr, grounpos, hausdata, hausnr, hauspos, hausprei, modatyp, monate,
    sonstige, start_txt, straspos, szenario_txt,
};
use formats::dfm::Object;
use formats::ne::{RT_RCDATA, ResId, resources};
use formats::savegame::SaveGame;
use game::darstellung::{self, Camera, Tiles};
use game::hauptschirm::{self, Hauptschirm, InfoTexts};
use game::hintergrund::{self, Island};
use game::simulation::TICK_MS;
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
}

impl Game {
    /// Runs the clock's ticks that are due (`AnimationsTimer`, 1020:1e9b)
    /// and says whether the screen changed.
    fn update(&mut self, elapsed: Duration) -> bool {
        let tick = Duration::from_millis(u64::from(TICK_MS));
        self.since_tick += elapsed;
        let mut ticked = false;
        let mut changed = false;
        while self.since_tick >= tick {
            self.since_tick -= tick;
            let t = self.world.tick();
            if t.redraw_all {
                self.screen.paint_map(&self.world, &self.tiles);
                changed = true;
            } else {
                changed |= self.screen.redraw_squares(&t.redraw, &self.world, &self.tiles);
            }
            ticked = true;
        }
        let info = ticked && self.screen.show_info(&self.world, &self.font, &self.info);
        changed || info
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
            Input::Close | Input::KeyDown(VK_ESCAPE) => self.quit = true,
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
                    } else if let Some(game) = &mut self.game {
                        let shift = self.buttons | self.keys;
                        if game.screen.key_down(key, shift, &game.world, &game.tiles) == hauptschirm::KeyResult::Redrawn
                        {
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

/// What hintergrund's timer does from state 1000 to 1500 once Start is
/// clicked, without the video (`titel.avi`), the progress window
/// (`TWasNunForm`) and what isn't ported yet: load the map, set up the
/// main screen and show it. // 1000:2466
fn start_game(dir: &Path, exe: &[u8], settings: &Settings, scenario_page: bool) -> Game {
    let path = match hintergrund::new_island(settings, scenario_page) {
        Island::Scenario(file) => format!("SZENARIO/{}", file.to_ascii_uppercase()),
        Island::Generated => {
            println!("generating an island isn't ported yet (10b0:52d4); here's the tutorial island");
            "MAPS/TUTORIAL.MAP".to_owned()
        }
    };
    let map = SaveGame::parse(&read(dir, &path)).unwrap_or_else(|e| panic!("{path}: {e}"));
    let mut world = hintergrund::load_world(&map, settings, scenario_page).unwrap_or_else(|e| panic!("{path}: {e}"));
    // `Randomize` (1108:1e21) seeds RandSeed from the clock.
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| {
            d.subsec_nanos() ^ u32::try_from(d.as_secs() & 0xffff_ffff).unwrap_or(0)
        });
    world.random = Random::new(seed);
    world.tables = Arc::new(Tables {
        hausnr: hausnr(&read(dir, "DATA/HAUSNR.DAT")).expect("HAUSNR.DAT"),
        hausdata: hausdata(&read(dir, "DATA/HAUSDATA.DAT")).expect("HAUSDATA.DAT"),
        hausprei: hausprei(&read(dir, "DATA/HAUSPREI.DAT")).expect("HAUSPREI.DAT"),
    });
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
    let mut screen = Hauptschirm::new(camera, origin, bitmaps, &font, &title);
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
    Game {
        world,
        tiles,
        screen,
        font,
        info,
        since_tick: Duration::ZERO,
        form,
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
