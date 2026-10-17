//! Holiday Island. For now this shows the main form with the title picture
//! and then the background, like the end of the intro (`docs/startup.md`,
//! states 430–450), and then the start menu on top (state 500). Start
//! loads the scenario (or, until island generation is ported, the tutorial
//! island) and shows the main screen with the island, which the arrow keys
//! scroll and Insert and Delete turn, and runs the simulation's clock
//! (`World::tick`, every 100 ms), with the date and money on the info bar.
//! When an event starts an auction the auction form opens over it; its
//! buttons and keys become commands to the world. The button bar picks
//! the tools (`game::tools`): building from the build menu
//! (`TBaumenue`), the road, raising and lowering the land and
//! demolishing, whose clicks on the island become commands too; a right
//! click on the island moves the view there.
//!
//! The start menu's "Altes Spiel fortsetzen" opens the load form
//! (`TLadenForm`) with the saved games; in a game, the button bar's first
//! button (or `1`, `D`) opens the file menu (`TDateimenue`), which saves,
//! loads (a command to the world), goes back to the start menu (saving
//! `Last.map`) and quits.
//!
//! `holiday [--save-dir=DIR] [GAME_DIR]`, where `GAME_DIR` holds
//! `HOLIDAY.EXE`, `GRAFIK`, `DATA` and `TEXTE` (default `orig`). Saved
//! games go to `DIR` (default `MAPS` in the platform's data directory,
//! `platform::storage::save_dir`); the lists show those and the game's own
//! `MAPS` (the tutorial), which is only read.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use formats::bmp::Bitmap;
use formats::data::{
    animdat1, animdat4, auktion, auswahl, baumenu_dat, baumenu_txt, darstell, fehler, flusspos, fontdat, gfxpos,
    groundnr, grounpos, hausdata, hausname, hausnr, hauspos, hausprei, message_media_names, modatyp, monate, sonstige,
    start_txt, straspos, szenario_txt,
};
use formats::dfm::Object;
use formats::ne::{RT_RCDATA, ResId, resources};
use formats::savegame::SaveGame;
use game::auction::{self, Change};
use game::auktionsform::{self, Auktionsform};
use game::baumenue::{self, Baumenue};
use game::command::{self, Command};
use game::darstellung::{self, Camera, Tiles};
use game::dateimenue::{self, Dateimenue};
use game::hauptschirm::{self, Hauptschirm, InfoTexts, KeyResult};
use game::hintergrund::{self, Island};
use game::ladenform::{self, LadenForm};
use game::savegame::{Fields, MapView};
use game::simulation::{TICK_MS, Tick};
use game::startform::{self, Action, Settings, StartForm};
use game::text::Font;
use game::tools::Tools;
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
/// Windows' default double-click time and distance (`GetDoubleClickTime`,
/// `SM_CXDOUBLECLK`), for the list boxes' `ssDouble`.
const DOUBLE_CLICK: Duration = Duration::from_millis(500);
const DOUBLE_CLICK_DISTANCE: i32 = 2;

/// Where the saved games are: the player's directory, which saves go to,
/// and the game's `MAPS`, which is only read.
struct Saves {
    dir: PathBuf,
    game_maps: PathBuf,
}

impl Saves {
    /// The `*.map` files' names, the player's first, each name once
    /// (ignoring case), sorted: the port's stand-in for the order
    /// `FindFirst` gives.
    fn files(&self) -> Vec<Vec<u8>> {
        let mut names: Vec<Vec<u8>> = Vec::new();
        for dir in [&self.dir, &self.game_maps] {
            let Ok(entries) = std::fs::read_dir(dir) else { continue };
            for e in entries.flatten() {
                if !e.file_type().is_ok_and(|t| t.is_file()) {
                    continue;
                }
                let name = e.file_name().to_string_lossy().as_bytes().to_vec();
                if !names.iter().any(|n| n.eq_ignore_ascii_case(&name)) {
                    names.push(name);
                }
            }
        }
        names.sort_by_key(|n| n.to_ascii_lowercase());
        names
    }

    /// The list boxes' entries. // 1038:3983
    fn games(&self) -> Vec<Vec<u8>> {
        ladenform::saved_games(&self.files())
    }

    /// The file called `name` (any case), the player's first.
    fn find(&self, name: &[u8]) -> Option<PathBuf> {
        [&self.dir, &self.game_maps].into_iter().find_map(|dir| {
            std::fs::read_dir(dir).ok()?.flatten().find_map(|e| {
                let n = e.file_name();
                n.to_string_lossy()
                    .as_bytes()
                    .eq_ignore_ascii_case(name)
                    .then(|| e.path())
            })
        })
    }

    /// Reads and parses the saved game `name`.
    fn read(&self, name: &[u8]) -> Result<SaveGame, String> {
        let shown = String::from_utf8_lossy(name).into_owned();
        let path = self.find(name).ok_or_else(|| format!("{shown}: not found"))?;
        let data = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        SaveGame::parse(&data).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Writes `map` as `name` into the player's directory, replacing a file
    /// of that name in any case.
    fn write(&self, name: &[u8], map: &SaveGame) -> std::io::Result<PathBuf> {
        std::fs::create_dir_all(&self.dir)?;
        let path = match self.find(name) {
            Some(p) if p.starts_with(&self.dir) => p,
            _ => self.dir.join(String::from_utf8_lossy(name).as_ref()),
        };
        let temp = path.with_extension("map.tmp");
        std::fs::write(&temp, map.to_bytes())?;
        std::fs::rename(&temp, &path)?;
        Ok(path)
    }
}

/// `TLadenForm` over the start menu, and its VCL form.
struct LoadMenu {
    menu: LadenForm,
    form: Form,
    captured: bool,
}

/// `TDateimenue`, and its VCL form.
struct FileMenu {
    menu: Dateimenue,
    form: Form,
    visible: bool,
    captured: bool,
}

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
    file_menu: FileMenu,
    /// The view's fields of the map file the game came from that the port
    /// doesn't use, saved back as they were.
    view_rest: Fields,
    /// The main screen's tools and the build menu (`TBaumenue`), and the
    /// build menu's VCL form.
    tools: Tools,
    menu_form: Form,
    /// The build menu has the mouse captured.
    menu_captured: bool,
    /// The main screen's image that has the mouse captured.
    captured: Option<&'static str>,
    /// The mouse on the screen, which `GetCursorPos` gives the original.
    mouse: (i32, i32),
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
        if self.file_menu.visible {
            // The file menu stops the clock (1020:24a0) until it closes.
            // Single human: in the original this stops the game for
            // everyone; the view just doesn't tick meanwhile.
            self.since_tick = Duration::ZERO;
            return changed;
        }
        while self.since_tick >= tick {
            self.since_tick -= tick;
            // Step 2: the status bar's message times out.
            changed |= self.screen.status_tick(&self.font);
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
            // 1090:124f, 1090:155b: the tool's pictures, with the people.
            if self.world.options.animations && self.world.options.people_spawn {
                let cam = self.screen.camera();
                let mouse = self.hauptbild_mouse();
                self.tools.preview(&self.world, &self.tiles, &cam, mouse);
                self.tools
                    .draw_preview(&mut self.screen.map, &self.world, &self.tiles, &cam);
            }
            changed = true;
        }
        let info = ticked && self.screen.show_info(&self.world, &self.font, &self.info);
        changed || info
    }

    /// The mouse in `Hauptbild`'s coordinates.
    fn hauptbild_mouse(&self) -> (i32, i32) {
        let (left, top) = Hauptschirm::position(screen_size().0, screen_size().1);
        let bild = hauptschirm::LAYOUT[3].1;
        (self.mouse.0 - left - bild.left, self.mouse.1 - top - bild.top)
    }

    /// Hands the tools' actions to the world as the human's commands.
    fn tool_actions(&mut self, actions: Vec<command::Action>) {
        // Single human: the original's player 1.
        self.commands
            .extend(actions.into_iter().map(|action| Command { player: 1, action }));
    }

    /// Button `b` of the button bar is clicked (1010:2c8b): the file menu
    /// is the caller's, the rest the tools'.
    fn click_button(&mut self, b: u8) {
        let view = self.screen.camera().view;
        let actions = self
            .tools
            .click(b, &self.world, &self.font, &self.tiles, &self.info, view, screen_size());
        self.tool_actions(actions);
        self.show_tools();
    }

    /// What the build menu's click or key asks, and the key it passes on
    /// to the main screen.
    fn menu_response(&mut self, response: baumenue::Response) -> Option<u16> {
        let view = self.screen.camera().view;
        let (actions, key) = self.tools.menu_response(
            response,
            &self.world,
            &self.font,
            &self.tiles,
            &self.info,
            view,
            screen_size(),
        );
        self.tool_actions(actions);
        self.show_tools();
        key
    }

    /// The button bar and the status bar after the tools changed.
    fn show_tools(&mut self) {
        if self.tools.buttons_changed {
            self.screen.draw_buttons(&self.tools.down);
            self.tools.buttons_changed = false;
        }
        if let Some(text) = self.tools.status.take() {
            self.screen.show_status(&text, &self.font);
        }
        // The sounds aren't played yet.
        self.tools.sounds.clear();
        self.tools.menu.sounds.clear();
    }

    /// Shows what a tick or command did, and says whether anything changed.
    fn show(&mut self, t: &Tick) -> bool {
        let view = self.screen.camera().view;
        self.tools.world_changed(
            t,
            1,
            &self.world,
            &self.font,
            &self.tiles,
            &self.info,
            view,
            screen_size(),
        );
        self.show_tools();
        if t.loaded {
            // `game_show_state` (1010:26fa): the view from the file.
            let camera = MapView {
                owner_pointers: self.screen.camera().owner_pointers,
                occupancy_pointers: self.screen.camera().occupancy_pointers,
                ..MapView::default()
            }
            .camera(&self.world);
            self.screen.show_loaded(camera, &self.world, &self.tiles);
            // The tools and the build menu from the file (1018:2a17).
            let rest = self.view_rest.clone();
            self.tools
                .start(&rest, &self.world, &self.font, &self.tiles, &self.info, camera.view);
            self.screen.draw_buttons(&self.tools.down);
            self.show_tools();
            self.screen
                .show_people(&self.world, &self.tiles, &self.font, &self.info);
            self.screen.show_info(&self.world, &self.font, &self.info);
            return true;
        }
        if t.load_failed {
            eprintln!("the saved game couldn't be loaded");
        }
        let mut changed = !t.status.is_empty() || t.pointer_tool;
        changed |= if t.redraw_all {
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

    /// A mouse event on the build menu while it's shown, and whether it
    /// took it. // 1018:0208, 1018:0461, 1018:04e7
    fn menu_mouse(&mut self, input: Input, shift: ShiftState, buttons_up: bool) -> bool {
        let (Input::MouseMove { x: sx, y: sy }
        | Input::MouseDown { x: sx, y: sy, .. }
        | Input::MouseUp { x: sx, y: sy, .. }) = input
        else {
            return false;
        };
        let m = &self.tools.menu;
        if !self.menu_captured && !m.contains(sx, sy) {
            return false;
        }
        let (x, y) = (sx - m.left, sy - m.top);
        let view = self.screen.camera().view;
        let response = match input {
            Input::MouseDown { button, .. } => {
                self.menu_captured = true;
                self.tools.menu_focused = true;
                self.tools.menu.mouse_down(
                    x,
                    y,
                    button == Button::Left,
                    shift,
                    (sx, sy),
                    &self.world,
                    &self.font,
                    &self.tiles,
                    &self.info,
                    view,
                )
            }
            Input::MouseUp { button, .. } => {
                if buttons_up {
                    self.menu_captured = false;
                }
                self.tools.menu.mouse_up(
                    x,
                    y,
                    button == Button::Left,
                    &self.world,
                    &self.font,
                    &self.tiles,
                    &self.info,
                    view,
                )
            }
            _ => {
                self.tools.menu.mouse_move(shift, (sx, sy), screen_size());
                baumenue::Response::Nothing
            }
        };
        let _ = self.menu_response(response);
        true
    }

    /// A mouse event on the main screen: a click on the button bar
    /// (`ButtonImage`'s `OnClick`, 1020:1946) or the island (`Hauptbild`,
    /// 1020:1984, 1020:1b31). Whether the screen changed, and whether the
    /// file menu's button was clicked.
    fn main_mouse(&mut self, input: Input, shift: ShiftState, buttons_up: bool) -> (bool, bool) {
        let (Input::MouseMove { x: sx, y: sy }
        | Input::MouseDown { x: sx, y: sy, .. }
        | Input::MouseUp { x: sx, y: sy, .. }) = input
        else {
            return (false, false);
        };
        let (left, top) = Hauptschirm::position(screen_size().0, screen_size().1);
        let (fx, fy) = (sx - left, sy - top);
        let target = self.captured.or_else(|| {
            hauptschirm::LAYOUT
                .iter()
                .find(|(_, r)| (r.left..r.right).contains(&fx) && (r.top..r.bottom).contains(&fy))
                .map(|(name, _)| *name)
        });
        let Some(target) = target else {
            return (false, false);
        };
        let bounds = hauptschirm::LAYOUT
            .iter()
            .find(|(n, _)| *n == target)
            .map_or(Rect::default(), |(_, r)| *r);
        let (x, y) = (fx - bounds.left, fy - bounds.top);
        match input {
            Input::MouseDown { .. } => {
                self.captured = Some(target);
                // The main screen takes the focus from the build menu.
                self.tools.menu_focused = false;
            }
            Input::MouseUp { .. } if buttons_up => self.captured = None,
            _ => {}
        }
        match (target, input) {
            // `OnClick`: the left button let go over the button bar.
            (
                "ButtonImage",
                Input::MouseUp {
                    button: Button::Left, ..
                },
            ) if (0..bounds.width()).contains(&x) && (0..bounds.height()).contains(&y) => {
                match Hauptschirm::button_at(x, y) {
                    Some(hauptschirm::FILE_MENU_BUTTON) => return (true, true),
                    Some(b) => self.click_button(b),
                    None => {}
                }
                (true, false)
            }
            ("Hauptbild", Input::MouseDown { .. }) => {
                if shift == ShiftState::RIGHT {
                    self.screen.right_click(x, y, &self.world, &self.tiles); // 1020:1984
                    self.screen
                        .show_people(&self.world, &self.tiles, &self.font, &self.info);
                } else {
                    let cam = self.screen.camera();
                    let actions = self.tools.press(x, y, shift, &self.world, &self.tiles, &cam);
                    self.tool_actions(actions);
                }
                (true, false)
            }
            ("Hauptbild", Input::MouseUp { button, .. }) => {
                let actions = self.tools.release(button == Button::Left);
                self.tool_actions(actions);
                (false, false)
            }
            _ => (false, false),
        }
    }

    /// What `save_map` writes of this game. // 1048:2bae
    fn map(&self) -> SaveGame {
        let mut rest = self.view_rest.clone();
        self.tools.save(&mut rest);
        self.world.to_map(&MapView::of(&self.screen, rest))
    }

    /// `FormShow` of the file menu, from the button bar's first button
    /// (1010:2c8b): the clock stops, the other forms are disabled.
    /// // 1040:12be
    fn open_file_menu(&mut self, games: Vec<Vec<u8>>) {
        self.file_menu.menu.show(games);
        self.file_menu.visible = true;
        self.file_menu.captured = false;
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
        let m = &self.tools.menu;
        self.menu_form.components[0].bounds = m.bounds();
        self.menu_form.components[0].visible = m.height > 0;
        if let Some(c) = self.menu_form.component_mut("Baumenuebild") {
            c.bounds = Rect::new(0, 0, m.width, m.full_height());
            c.visible = true;
            if let Kind::Image { picture } = &mut c.kind {
                *picture = Some(m.image.clone());
            }
        }
        self.menu_form.paint(screen, palette);
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
        let m = &mut self.file_menu;
        m.form.components[0].bounds = Rect::new(m.menu.left, m.menu.top, dateimenue::WIDTH, dateimenue::HEIGHT);
        m.form.components[0].visible = m.visible;
        if let Some(c) = m.form.component_mut("Dateimenuebild")
            && let Kind::Image { picture } = &mut c.kind
        {
            *picture = Some(m.menu.image().clone());
        }
        m.form.paint(screen, palette);
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
    /// `TLadenForm`, while it's open over the start menu.
    load: Option<LoadMenu>,
    saves: Saves,
    /// The last left press, for double clicks.
    last_press: Option<(Instant, i32, i32)>,
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
        if let Some(load) = &mut self.load {
            if let Some(bild) = load.form.component_mut("Bild")
                && let Kind::Image { picture } = &mut bild.kind
            {
                *picture = Some(load.menu.bild().clone());
            }
            load.form.paint(&mut self.screen, &self.palette);
        }
    }

    fn shift(&self) -> ShiftState {
        self.buttons | self.keys
    }

    /// The `TShiftState` of a press of `button` at `x`, `y`: with
    /// `ssDouble` for a second left press soon after the first.
    fn press_shift(&mut self, x: i32, y: i32, button: Button) -> ShiftState {
        let mut shift = self.shift();
        if button == Button::Left {
            let now = Instant::now();
            let double = self.last_press.is_some_and(|(t, px, py)| {
                now.duration_since(t) <= DOUBLE_CLICK
                    && (px - x).abs() <= DOUBLE_CLICK_DISTANCE
                    && (py - y).abs() <= DOUBLE_CLICK_DISTANCE
            });
            if double {
                shift = shift | ShiftState::DOUBLE;
                self.last_press = None;
            } else {
                self.last_press = Some((now, x, y));
            }
        }
        shift
    }

    /// Passes a mouse event on to `Bild` if it's over it or has the mouse
    /// captured.
    fn mouse(&mut self, input: Input) {
        if !self.start.visible() {
            self.game_mouse(input);
            return;
        }
        if self.load.is_some() {
            self.load_mouse(input);
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

    /// A mouse event in the game: the file menu while it's open (the
    /// other forms are disabled), the auction form, and the button bar's
    /// first button.
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
        let shift = match input {
            Input::MouseDown { x, y, button } => {
                // `press_shift` adds the button, which the state has now.
                let s = self.press_shift(x, y, button);
                ShiftState(s.0 | self.buttons.0)
            }
            _ => self.shift(),
        };
        let mouse = ShiftState::LEFT | ShiftState::RIGHT | ShiftState::MIDDLE;
        let up = self.buttons.0 & mouse.0 == 0;
        let Some(game) = &mut self.game else { return };
        if let Input::MouseMove { x, y } | Input::MouseDown { x, y, .. } | Input::MouseUp { x, y, .. } = input {
            game.mouse = (x, y);
        }
        if game.file_menu.visible {
            self.file_menu_mouse(input, shift, up);
            return;
        }
        if game.auction_mouse(input, shift, up) {
            self.repaint();
            return;
        }
        if game.menu_mouse(input, shift, up) {
            self.repaint();
            return;
        }
        let (changed, file_menu) = game.main_mouse(input, shift, up);
        if file_menu {
            let games = self.saves.games();
            if let Some(game) = &mut self.game {
                game.open_file_menu(games);
            }
        }
        if changed || file_menu {
            self.repaint();
        }
    }

    /// A mouse event while the load form is open: the start menu is
    /// disabled, so only the form gets it.
    fn load_mouse(&mut self, input: Input) {
        let Some(load) = &mut self.load else { return };
        let (lx, ly) = LadenForm::position(screen_size().0, screen_size().1);
        let bit = |b: Button| match b {
            Button::Left => ShiftState::LEFT,
            Button::Right => ShiftState::RIGHT,
            Button::Middle => ShiftState::MIDDLE,
        };
        let inside = |x: i32, y: i32| (0..ladenform::WIDTH).contains(&x) && (0..ladenform::HEIGHT).contains(&y);
        let response = match input {
            Input::MouseDown { x, y, button } => {
                self.buttons = self.buttons | bit(button);
                let (x, y) = (x - lx, y - ly);
                if !inside(x, y) {
                    return;
                }
                load.captured = true;
                let shift = self.press_shift(x + lx, y + ly, button);
                let shift = ShiftState(shift.0 | self.buttons.0);
                let Some(load) = &mut self.load else { return };
                load.menu.mouse_down(x, y, shift)
            }
            Input::MouseUp { x, y, button } => {
                self.buttons = ShiftState(self.buttons.0 & !bit(button).0);
                let (x, y) = (x - lx, y - ly);
                let was = load.captured;
                if self.buttons.0 & (ShiftState::LEFT | ShiftState::RIGHT | ShiftState::MIDDLE).0 == 0 {
                    load.captured = false;
                }
                if !was && !inside(x, y) {
                    return;
                }
                load.menu.mouse_up(x, y)
            }
            _ => return,
        };
        match response.action {
            Some(ladenform::Action::Close) => {
                // 1038:3817: the start menu is enabled again.
                self.load = None;
            }
            Some(ladenform::Action::Load(name)) => {
                // 1038:3a95: the start menu hides and states 1000-1500
                // load the game.
                self.load = None;
                match self.saves.read(&name) {
                    Ok(map) => {
                        self.start.hide();
                        let settings = self.start.menu.settings().clone();
                        self.game = Some(load_game(&self.dir, &self.exe, &map, &settings));
                    }
                    Err(e) => eprintln!("{e}"),
                }
            }
            None => {}
        }
        self.repaint();
    }

    /// A mouse event while the file menu is open: the other forms are
    /// disabled. // 1040:073c, 1040:09ca, 1040:0a4c
    fn file_menu_mouse(&mut self, input: Input, shift: ShiftState, buttons_up: bool) {
        let Some(game) = &mut self.game else { return };
        let m = &mut game.file_menu;
        let (x, y) = match input {
            Input::MouseMove { x, y } | Input::MouseDown { x, y, .. } | Input::MouseUp { x, y, .. } => {
                (x - m.menu.left, y - m.menu.top)
            }
            _ => return,
        };
        let inside = (0..dateimenue::WIDTH).contains(&x) && (0..dateimenue::HEIGHT).contains(&y);
        if !inside && !m.captured {
            return;
        }
        let response = match input {
            Input::MouseDown { button, .. } => {
                m.captured = true;
                m.menu.mouse_down(x, y, button == Button::Left, shift)
            }
            Input::MouseUp { button, .. } => {
                if buttons_up {
                    m.captured = false;
                }
                m.menu.mouse_up(x, y, button == Button::Left)
            }
            _ => m.menu.mouse_move(x, y, shift, screen_size()),
        };
        self.file_menu_response(response);
    }

    /// Does what the file menu asks.
    fn file_menu_response(&mut self, response: dateimenue::Response) {
        let Some(game) = &mut self.game else { return };
        let refresh = response.refresh || response.moved || response.action.is_some();
        match response.action {
            Some(dateimenue::Action::Close) => game.file_menu.visible = false,
            Some(dateimenue::Action::Load(name)) => match self.saves.read(&name) {
                // 1040:0f27: load_map in the middle of the game, a command
                // to the world.
                Ok(map) => {
                    if World::from_map(&map).is_ok() {
                        game.view_rest = MapView::from_map(&map).unported;
                    }
                    game.commands.push(Command {
                        player: 1,
                        action: command::Action::Load(Arc::new(map)),
                    });
                }
                Err(e) => eprintln!("{e}"),
            },
            Some(dateimenue::Action::Save(name)) => {
                // 1040:0e03
                match self.saves.write(&name, &game.map()) {
                    Ok(path) => println!("saved {}", path.display()),
                    Err(e) => eprintln!("can't save {}: {e}", String::from_utf8_lossy(&name)),
                }
                let games = self.saves.games();
                if let Some(game) = &mut self.game {
                    game.file_menu.menu.set_games(games);
                }
            }
            // The message box (`MessageDlg`) isn't ported.
            Some(dateimenue::Action::InvalidName) => eprintln!("Dateiname ungültig"),
            Some(dateimenue::Action::NewGame) => self.back_to_start_menu(),
            Some(dateimenue::Action::Quit) => {
                // TEndeform, which asks first, isn't ported.
                self.quit = true;
            }
            None => {}
        }
        if refresh {
            self.repaint();
        }
    }

    /// "Neues Spiel": saves `Last.map`, ends the game and shows a new
    /// start menu, set from the globals the game left. Reloading the
    /// data (1030:38a3) and the music aren't ported. // 1010:25dc
    fn back_to_start_menu(&mut self) {
        let Some(game) = self.game.take() else { return };
        if let Err(e) = self.saves.write(hintergrund::LAST_MAP.as_bytes(), &game.map()) {
            eprintln!("can't save {}: {e}", hintergrund::LAST_MAP);
        }
        self.start = start_menu(&self.dir, &self.exe, Settings::from_world(&game.world));
        self.start.show();
        self.repaint();
    }

    fn respond(&mut self, response: startform::Response) {
        match response.action {
            Some(Action::Start { scenario }) => {
                // `startform_start_click` hides the form, and hintergrund's
                // states 1000-1500 start the game.
                self.start.hide();
                self.game = Some(start_game(&self.dir, &self.exe, self.start.menu.settings(), scenario));
            }
            Some(Action::Load) => {
                // 1058:0c7d: the start menu is disabled and TLadenForm
                // opens over it.
                self.load = Some(load_menu(&self.dir, &self.exe, self.saves.games()));
            }
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
            Input::KeyDown(VK_ESCAPE)
                if self.load.is_none()
                    && !self
                        .game
                        .as_ref()
                        .is_some_and(|g| g.auction.visible || g.file_menu.visible || g.tools.menu_focused) =>
            {
                self.quit = true;
            }
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
                    if self.load.is_some() {
                        // TLadenForm has no OnKeyDown, and the start menu
                        // is disabled.
                    } else if let Some(game) = &mut self.game
                        && game.file_menu.visible
                    {
                        let shift = self.buttons | self.keys;
                        let response = game.file_menu.menu.key_down(key, shift);
                        self.file_menu_response(response);
                    } else if self.start.visible() {
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
                        let mut key = Some(key);
                        if game.tools.menu_focused {
                            // `TBaumenue` has the focus. // 1018:3290
                            let response = game.tools.menu.key_down(key.unwrap_or(0));
                            key = game.menu_response(response);
                            self.repaint();
                        }
                        let Some(game) = &mut self.game else { return };
                        let result = key.map_or(KeyResult::Ignored, |key| {
                            game.screen.key_down(key, shift, &game.world, &game.tiles)
                        });
                        match result {
                            KeyResult::Redrawn => {
                                game.screen
                                    .show_people(&game.world, &game.tiles, &game.font, &game.info);
                                // `set_view` (1010:254d) turns the build
                                // menu's picture too.
                                let view = game.screen.camera().view;
                                game.tools
                                    .menu
                                    .view_turned(&game.world, &game.font, &game.tiles, &game.info, view);
                                self.repaint();
                            }
                            KeyResult::Button(hauptschirm::FILE_MENU_BUTTON) => {
                                let games = self.saves.games();
                                if let Some(game) = &mut self.game {
                                    game.open_file_menu(games);
                                }
                                self.repaint();
                            }
                            KeyResult::Button(b) => {
                                game.click_button(b);
                                self.repaint();
                            }
                            KeyResult::Ignored => {}
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

/// The screen's size, as the game lays out for it.
fn screen_size() -> (i32, i32) {
    (i32::try_from(SCREEN.0).unwrap(), i32::try_from(SCREEN.1).unwrap())
}

fn font(dir: &Path) -> Font {
    Font::new(
        &bitmap(dir, "GRAFIK/FONT.BMP"),
        &fontdat(&read(dir, "DATA/FONTDAT1.DAT")).expect("FONTDAT1.DAT"),
        &fontdat(&read(dir, "DATA/FONTDAT2.DAT")).expect("FONTDAT2.DAT"),
    )
}

fn bitmaps(dir: &Path, files: &[&str]) -> Vec<Surface> {
    files
        .iter()
        .map(|f| Surface::from_bitmap(&bitmap(dir, &format!("GRAFIK/{}", f.to_ascii_uppercase()))))
        .collect()
}

/// The start menu as `TStartform.Create` and `startform_init` make it,
/// from the globals in `settings`. // 1058:27e6
fn start_menu(dir: &Path, exe: &[u8], settings: Settings) -> StartMenu {
    let menu = StartForm::new(
        font(dir),
        bitmaps(dir, &startform::BITMAP_FILES),
        &start_txt(&read(dir, "TEXTE/START.TXT")),
        &szenario_txt(&read(dir, "TEXTE/SZENARIO.TXT")),
        settings,
    );
    StartMenu {
        menu,
        form: load_form(exe, "TSTARTFORM"),
        captured: false,
    }
}

/// `TLadenForm` as 1038:3b46 creates and shows it, with `games`: at the
/// bottom of the screen, `Bild` filling it. // 1038:3b46
fn load_menu(dir: &Path, exe: &[u8], games: Vec<Vec<u8>>) -> LoadMenu {
    let texts = sonstige(&read(dir, "TEXTE/SONSTIGE.TXT")).expect("SONSTIGE.TXT");
    let load = texts.texts.get(7).cloned().unwrap_or_default();
    let menu = LadenForm::new(font(dir), bitmaps(dir, &ladenform::BITMAP_FILES), &load, games);
    let mut form = load_form(exe, "TLADENFORM");
    let (left, top) = LadenForm::position(screen_size().0, screen_size().1);
    form.components[0].bounds = Rect::new(left, top, ladenform::WIDTH, ladenform::HEIGHT);
    form.components[0].visible = true;
    if let Some(bild) = form.component_mut("Bild") {
        bild.bounds = Rect::new(0, 0, ladenform::WIDTH, ladenform::HEIGHT);
        bild.visible = true;
    }
    LoadMenu {
        menu,
        form,
        captured: false,
    }
}

/// `TDateimenue` as `Application.CreateForm` makes it, hidden, at the
/// DFM's position. // 1040:154d
fn file_menu(dir: &Path, exe: &[u8]) -> FileMenu {
    let texts = sonstige(&read(dir, "TEXTE/SONSTIGE.TXT")).expect("SONSTIGE.TXT");
    let mut form = load_form(exe, "TDATEIMENUE");
    let at = form.components[0].bounds;
    if let Some(image) = form.component_mut("Dateimenuebild") {
        image.bounds = Rect::new(0, 0, dateimenue::WIDTH, dateimenue::HEIGHT);
        image.visible = true;
    }
    let menu = Dateimenue::new(
        font(dir),
        bitmaps(dir, &dateimenue::BITMAP_FILES),
        texts.texts,
        at.left,
        at.top,
    );
    FileMenu {
        menu,
        form,
        visible: false,
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

/// `Randomize` (1108:1e21): `RandSeed` from the clock.
fn seed() -> u32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| {
            d.subsec_nanos() ^ u32::try_from(d.as_secs() & 0xffff_ffff).unwrap_or(0)
        })
}

/// What a game starts from: its world, the main screen's camera, the
/// map's view origin (none for a new island) and the view's other fields
/// of the map.
type Start = (World, Camera, Option<(i16, i16)>, Fields);

/// The world a game starts with and the main screen's camera: a
/// scenario's map (and its view origin) or a generated island. // 1000:2466
fn start_world(dir: &Path, settings: &Settings, scenario_page: bool) -> Start {
    let seed = seed();
    match hintergrund::new_island(settings, scenario_page) {
        Island::Scenario(file) => {
            let path = format!("SZENARIO/{}", file.to_ascii_uppercase());
            let map = SaveGame::parse(&read(dir, &path)).unwrap_or_else(|e| panic!("{path}: {e}"));
            let mut world =
                hintergrund::load_world(&map, settings, scenario_page).unwrap_or_else(|e| panic!("{path}: {e}"));
            world.random = Random::new(seed);
            world.tables = world_tables(dir);
            let view = MapView::from_map(&map);
            let camera = Camera {
                parity: 0,
                ..view.camera(&world)
            };
            (world, camera, Some(camera.origin), view.unported)
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
            (world, camera, None, Fields::default())
        }
    }
}

/// States 1000 to 1500 with `hintergrund+0x190`: the saved game `map`
/// over a newly set up game, and the main screen at the file's view.
/// // 1000:2466
fn load_game(dir: &Path, exe: &[u8], map: &SaveGame, settings: &Settings) -> Game {
    let data = hintergrund::GameData::load(|name| std::fs::read(dir.join(name)).ok()).unwrap_or_else(|e| panic!("{e}"));
    let world =
        hintergrund::load_saved_game(map, settings, &data, seed()).unwrap_or_else(|e| panic!("the saved game: {e}"));
    let view = MapView::from_map(map);
    let camera = view.camera(&world);
    make_game(dir, exe, (world, camera, Some(camera.origin), view.unported))
}

/// What hintergrund's timer does from state 1000 to 1500 once Start is
/// clicked, without the video (`titel.avi`), the progress window
/// (`TWasNunForm`) and what isn't ported yet: load the scenario's map or
/// generate an island, set up the main screen and show it. // 1000:2466
fn start_game(dir: &Path, exe: &[u8], settings: &Settings, scenario_page: bool) -> Game {
    make_game(dir, exe, start_world(dir, settings, scenario_page))
}

/// The main screen and the forms of a game that starts from `start`.
/// // 1020:2501, 1010:2820
fn make_game(dir: &Path, exe: &[u8], (world, camera, origin, view_rest): Start) -> Game {
    let font = font(dir);
    let texts = sonstige(&read(dir, "TEXTE/SONSTIGE.TXT")).expect("SONSTIGE.TXT");
    let frame = bitmaps(dir, &hauptschirm::BITMAP_FILES);
    let title = texts.texts.first().cloned().unwrap_or_default();
    let mut screen = match origin {
        Some(origin) => Hauptschirm::new(camera, origin, frame, &font, &title),
        None => Hauptschirm::new_game(camera, frame, &font, &title),
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
    // 1030:36e5 (1018:2fa2) and `game_start_setup` (1010:2820): the
    // build menu, and the tools from the map.
    let names = hausname(&read(dir, "TEXTE/HAUSNAME.TXT")).expect("HAUSNAME.TXT");
    let menu = Baumenue::new(
        baumenu_txt(&read(dir, "TEXTE/BAUMENU.TXT")).expect("BAUMENU.TXT"),
        &baumenu_dat(&read(dir, "DATA/BAUMENU.DAT")).expect("BAUMENU.DAT"),
        names,
        auswahl(&read(dir, "TEXTE/AUSWAHL.TXT")),
        bitmaps(dir, &baumenue::BITMAP_FILES),
        screen_size(),
    );
    let mut tools = Tools::new(menu, fehler(&read(dir, "TEXTE/FEHLER.TXT")).expect("FEHLER.TXT"));
    tools.start(&view_rest, &world, &font, &tiles, &info, screen.camera().view);
    screen.draw_buttons(&tools.down);
    tools.buttons_changed = false;
    let mut menu_form = load_form(exe, "TBAUMENUE");
    menu_form.components[0].visible = false;
    Game {
        tools,
        menu_form,
        menu_captured: false,
        captured: None,
        mouse: (0, 0),
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
        file_menu: file_menu(dir, exe),
        view_rest,
    }
}

fn main() {
    let mut dir = PathBuf::from("orig");
    let mut save_dir = None;
    for arg in std::env::args_os().skip(1) {
        match arg.to_str().and_then(|a| a.strip_prefix("--save-dir=")) {
            Some(d) => save_dir = Some(PathBuf::from(d)),
            None => dir = PathBuf::from(arg),
        }
    }
    let saves = Saves {
        dir: save_dir
            .or_else(platform::storage::save_dir)
            .unwrap_or_else(|| dir.join("MAPS")),
        game_maps: dir.join("MAPS"),
    };
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
    let start = start_menu(&dir, &exe, Settings::default());
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
        load: None,
        saves,
        last_press: None,
    };
    holiday.set_picture(&title);
    if let Err(e) = platform::run("Holiday Island", holiday) {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
