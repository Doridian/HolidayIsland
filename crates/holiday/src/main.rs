//! Holiday Island. This plays the intro's film (`sunflow.avi`), shows the
//! main form with the title picture and then the background, like the
//! intro (`docs/startup.md`, states 400–450), and then the start menu on
//! top (state 500). Start (after `titel.avi`, states 1111–1120)
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
//! `Last.map`) and quits. The button bar's buttons 1 to 5 open the pause,
//! the options, the TV, the money menu and the overview map (with the
//! compass in the main screen's corner), the sabotage tool its window and
//! the pointer the info box and from there the buildings' overview
//! (`forms`); Escape, `Q`, Alt+F4 and the main screen's close button ask
//! whether to quit (`TEndeform`), and the end of a game shows its results
//! and the high scores.
//!
//! The window's client area, divided by the UI scale, is the desktop the
//! game lays out for, again whenever it changes (`docs/layout.md`).
//!
//! `holiday [--save-dir=DIR] [--soundfont=FILE.sf2] [--sound=hi|lo|off]
//! [--films=hi|lo] [--no-audio] [--window-size=WxH] [--fullscreen|--windowed]
//! [--ui-scale=N] [GAME_DIR]`, where `GAME_DIR` holds `HOLIDAY.EXE`,
//! `GRAFIK`, `DATA` and `TEXTE` (default `orig`). Saved games go to `DIR`
//! (default `MAPS` in the platform's data directory,
//! `platform::storage::save_dir`); the lists show those and the game's own
//! `MAPS` (the tutorial), which is only read. The sound plays the waves
//! from `SOUND` (or `SOUNDLO`, or none: `--sound`, else `SETUP.DAT`'s
//! choice if the game's directory has one) and the music from `MUSIK`
//! with the `SoundFont` `--soundfont` or `HOLIDAY_SOUNDFONT` names, else the
//! one `tools/get-soundfont.sh` installed or one in the usual places
//! (`platform::audio::find_soundfont`); without
//! one the music is silent, and without a sound device (or with
//! `--no-audio`) everything is. The films come from `FILMHI` (or
//! `FILMLO`: `--films`, else `SETUP.DAT`'s choice if there is one), and
//! play with their sound (`docs/films.md`).

mod film;
mod forms;
mod sound;
mod stacking;

use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use formats::bmp::Bitmap;
use formats::data::{
    animdat1, animdat4, auktion, auswahl, baumenu_dat, baumenu_txt, darstell, fehler, flusspos, fontdat, gfxpos,
    groundnr, grounpos, hausdata, hausname, hausnr, hauspos, hausprei, message_media_names, modatyp, monate, sonstige,
    start_txt, straspos, szenario_txt,
};
use formats::dfm::Object;
use formats::highscore::HighScores;
use formats::ne::{RT_RCDATA, ResId, resources};
use formats::savegame::SaveGame;
use formats::setup;
use game::auction::{self, Change};
use game::auktionsform::{self, Auktionsform};
use game::baumenue::{self, Baumenue};
use game::command::{self, Command};
use game::darstellung::{self, Camera, Tiles};
use game::dateimenue::{self, Dateimenue};
use game::endeform;
use game::enhance::{self, Config};
use game::hauptschirm::{self, Hauptschirm, InfoTexts, KeyResult};
use game::hintergrund::{self, Island};
use game::ladenform::{self, LadenForm};
use game::optionen::Settings as Options;
use game::savegame::{self, Fields, MapView};
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

use crate::forms::{Focus, Forms, Request};
use crate::sound::Sound;
use crate::stacking::{Stack, Win};

/// After the intro's film: when states 422 to 424 play `holiday.mid`
/// (420's interval 10, 422's 1), when 430 shows the title (424's 1000)
/// and when 500 shows the start menu (440's 3000, 450's 1).
/// // 1000:2466
const INTRO_MUSIC_AT: Duration = Duration::from_millis(11);
const TITLE_AT: Duration = Duration::from_millis(1012);
const MENU_AT: Duration = Duration::from_millis(4013);
/// The intro's film, in `FilmPanel`. // 1000:2323 (state 400)
const INTRO_FILM: &str = "sunflow.avi";
/// The film before a game starts. // 1000:23bd (state 1111)
const START_FILM: &str = "titel.avi";
/// `FilmPanel`'s size; state 110 centres it. // 1000:2466
const FILM_PANEL: (i32, i32) = (0x1b0, 0x140);
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
    /// Where the high scores go: `HIGHSCOR.WAV` next to the saves'
    /// directory (the original writes the game's `SOUND\HIGHSCOR.WAV`).
    fn high_scores(&self) -> PathBuf {
        self.dir.with_file_name("HIGHSCOR.WAV")
    }

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
    /// What `startform_show` does: centre the form on the `screen` the
    /// game lays out for, draw the menu and show it. // 1058:1a30
    fn show(&mut self, screen: (i32, i32)) {
        self.place(screen);
        self.form.components[0].visible = true;
        self.menu.show();
    }

    /// The form centred on `screen` (1058:1a30, and `startform_init`,
    /// 1058:27e6); the port also does it when the window's size changes.
    fn place(&mut self, (w, h): (i32, i32)) {
        let (left, top) = StartForm::position(w, h);
        self.form.components[0].bounds = Rect::new(left, top, startform::WIDTH, startform::HEIGHT);
        if let Some(bild) = self.form.component_mut("Bild") {
            bild.bounds = Rect::new(0, 0, startform::WIDTH, startform::HEIGHT);
        }
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
    /// The other forms (`forms`).
    forms: Forms,
    /// Which window lies over which.
    stack: Stack,
    /// The sound, shared with the start menu.
    sound: sound::Shared,
    /// The screen size the game lays out for (`screen_width`,
    /// `screen_height`, `hintergrund::screen_size`).
    size: (i32, i32),
    /// The desktop (the window), which `big_map`'s main screen fills.
    desktop: (i32, i32),
}

impl Game {
    /// Runs the player's commands and the clock's ticks that are due
    /// (`AnimationsTimer`, 1020:1e9b, and the auction form's timers) and
    /// says whether the screen changed.
    fn update(&mut self, elapsed: Duration) -> bool {
        let tick = Duration::from_millis(u64::from(TICK_MS));
        self.since_tick += elapsed;
        let mut ticked = false;
        let ms = u32::try_from(elapsed.as_millis()).unwrap_or(u32::MAX);
        let mut changed = self.forms_timers(ms);
        self.end_music_tick();
        changed |= self.apply_commands();
        while self.since_tick >= tick {
            if self.clock_stopped() {
                // The file menu, the pause, the options, the quit question
                // and the end form stop the clock (1020:24a0) until they
                // close. Single human: in the original this stops the game
                // for everyone; the view just doesn't tick meanwhile.
                self.since_tick = Duration::ZERO;
                break;
            }
            self.since_tick -= tick;
            // Step 2: the status bar's message times out; step 3 the
            // tooltips; step 8 the TV.
            changed |= self.screen.status_tick(&self.font);
            changed |= self.forms_clock_tick();
            // The original's people code reads the player's view.
            // The original's 20 × 27 view's, with `big_map` too.
            self.world.set_viewpoint(self.screen.viewpoint());
            let t = self.world.tick();
            changed |= self.show(&t);
            // What the tick's forms asked of the world, before the next.
            changed |= self.apply_commands();
            if self.auction.visible && self.world.auction.open {
                self.auction.draw_auctioneer(&self.world); // 1028:0221
                changed = true;
            }
            ticked = true;
        }
        changed |= self.auction.advance(ms, &self.world, &self.font, &self.info);
        if ticked {
            // 1088:385d, 1088:38c4: the people where they moved to.
            self.screen
                .show_people(&self.world, &self.tiles, &self.font, &self.info);
            // 1090:124f, 1090:155b: the tool's pictures, with the people.
            if self.world.options.animations && self.world.options.people_spawn {
                let cam = self.screen.drawn_camera();
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

    /// Applies the player's commands, and draws again what the forms show
    /// of the world after them.
    fn apply_commands(&mut self) -> bool {
        let mut changed = false;
        for command in std::mem::take(&mut self.commands) {
            let t = self.world.apply(command);
            changed |= self.show(&t);
        }
        changed | self.forms_after_commands()
    }

    /// A form is up that stops the clock.
    fn clock_stopped(&self) -> bool {
        self.file_menu.visible || self.forms.clock_stopped()
    }

    /// The game's layout for a new screen `size` (a port extension: the
    /// original lays out once, for the desktop it starts on). The main
    /// screen is centred again as at the start (`hauptschirm_layout`,
    /// 1020:1ce9); every other form moves with it, so it stays where it
    /// was over the main screen, and is then kept on the screen the way
    /// loading a game keeps the saved windows on it
    /// (`savegame::keep_on_screen`, 1010:08d7), and off the top and left
    /// edges too.
    ///
    /// With `big_map` the main screen fills a `desktop` bigger than
    /// 640×480 instead (`Hauptschirm::place`), and the other forms move
    /// as they would with the original's main screen centred.
    fn resize(&mut self, size: (i32, i32), desktop: (i32, i32)) {
        if size == self.size && (desktop == self.desktop || !self.screen.big_map) {
            self.desktop = desktop;
            return;
        }
        let old = self.size;
        self.size = size;
        self.desktop = desktop;
        let (left, top) = (self.screen.left, self.screen.top);
        let filled_before = self.screen.size() != (hauptschirm::WIDTH, hauptschirm::HEIGHT);
        self.place_main();
        let filled = self.screen.size() != (hauptschirm::WIDTH, hauptschirm::HEIGHT);
        let delta = if filled || filled_before {
            let (was, now) = (
                Hauptschirm::position(old.0, old.1),
                Hauptschirm::position(size.0, size.1),
            );
            (now.0 - was.0, now.1 - was.1)
        } else {
            (self.screen.left - left, self.screen.top - top)
        };
        let follow = |bounds: Rect, l: &mut i32, t: &mut i32| {
            let moved = Rect::new(
                bounds.left + delta.0,
                bounds.top + delta.1,
                bounds.width(),
                bounds.height(),
            );
            let (x, y) = savegame::keep_on_screen(moved, size);
            (*l, *t) = (x.max(0), y.max(0));
        };
        let a = &mut self.auction;
        follow(a.bounds(), &mut a.left, &mut a.top);
        let m = &mut self.tools.menu;
        follow(m.bounds(), &mut m.left, &mut m.top);
        if let Some(s) = &mut self.tools.sabowin {
            follow(s.bounds(), &mut s.left, &mut s.top);
        }
        let d = &mut self.file_menu.menu;
        let bounds = Rect::new(d.left, d.top, dateimenue::WIDTH, dateimenue::HEIGHT);
        follow(bounds, &mut d.left, &mut d.top);
        self.forms.follow(follow);
    }

    /// Lays the main screen out for the screen and the desktop
    /// (`Hauptschirm::place`); when that changed its size (`big_map`),
    /// the tools pick in its new view, and the island, the people, the
    /// buttons and the info bar are drawn again. The overview map's frame
    /// follows.
    fn place_main(&mut self) {
        let changed = self.screen.place(self.size, self.desktop);
        self.stack.main_desktop = self.screen.size() != (hauptschirm::WIDTH, hauptschirm::HEIGHT);
        if changed {
            self.tools.view = self.screen.view_size();
            self.screen.paint_map(&self.world, &self.tiles);
            self.screen
                .show_people(&self.world, &self.tiles, &self.font, &self.info);
            self.screen.draw_buttons(&self.tools.down);
            self.screen.show_info(&self.world, &self.font, &self.info);
        }
        self.main_view_changed();
    }

    /// The player's view enhancements for this game: the stacking, and
    /// `big_map`'s main screen.
    fn set_view_enhancements(&mut self, view: enhance::Enhancements) {
        self.stack = Stack::new(view);
        self.screen.big_map = view.has(enhance::Enhancement::BigMap);
        self.place_main();
    }

    /// The mouse in `Hauptbild`'s coordinates.
    fn hauptbild_mouse(&self) -> (i32, i32) {
        let bild = self.screen.hauptbild();
        (
            self.mouse.0 - self.screen.left - bild.left,
            self.mouse.1 - self.screen.top - bild.top,
        )
    }

    /// Hands the tools' actions to the world as the human's commands.
    fn tool_actions(&mut self, actions: Vec<command::Action>) {
        // Single human: the original's player 1.
        self.commands
            .extend(actions.into_iter().map(|action| Command { player: 1, action }));
    }

    /// Button `b` of the button bar is clicked (1010:2c8b): the file menu
    /// is the caller's, buttons 1 to 5 open forms (`forms`), the rest are
    /// the tools'.
    fn click_button(&mut self, b: u8) {
        if (1..=5).contains(&b) {
            self.form_button(b);
            return;
        }
        let view = self.screen.camera().view;
        let actions = self
            .tools
            .click(b, &self.world, &self.font, &self.tiles, &self.info, view, self.size);
        self.tool_actions(actions);
        self.show_tools();
    }

    /// What the build menu's click or key asks, and the key it passes on
    /// to the main screen.
    fn menu_response(&mut self, response: baumenue::Response) -> Option<u16> {
        if response == baumenue::Response::FocusMainScreen {
            // Space: `BringToFront` of the main screen (1018:3347).
            self.stack.raise(Win::Main);
        }
        let view = self.screen.camera().view;
        let (actions, key) = self.tools.menu_response(
            response,
            &self.world,
            &self.font,
            &self.tiles,
            &self.info,
            view,
            self.size,
        );
        self.tool_actions(actions);
        self.show_tools();
        key
    }

    /// The button bar and the status bar after the tools changed.
    fn show_tools(&mut self) {
        if self.tools.menu_focused || self.tools.sabowin_focused {
            self.forms.focus = Focus::Main;
        }
        if self.tools.buttons_changed {
            self.screen.draw_buttons(&self.tools.down);
            self.tools.buttons_changed = false;
        }
        if let Some(text) = self.tools.status.take() {
            self.screen.show_status(&text, &self.font);
        }
        let settings = self.forms.settings;
        let mut s = self.sound.borrow_mut();
        s.play_all(std::mem::take(&mut self.tools.menu.sounds), &settings);
        s.play_all(std::mem::take(&mut self.tools.sounds), &settings);
    }

    /// Shows what a tick or command did, and says whether anything changed.
    fn show(&mut self, t: &Tick) -> bool {
        let view = self.screen.camera().view;
        self.tools
            .world_changed(t, 1, &self.world, &self.font, &self.tiles, &self.info, view, self.size);
        self.show_tools();
        if t.loaded {
            self.stack.loaded();
            // `game_show_state` (1010:26fa): the view from the file.
            if let Some(s) = self.forms.loading_settings.take() {
                self.forms.settings = s;
            }
            self.sound.borrow_mut().game_loaded(&self.forms.settings);
            let camera = MapView {
                owner_pointers: self.forms.settings.owner_pointers,
                occupancy_pointers: self.forms.settings.occupancy_pointers,
                ..MapView::default()
            }
            .camera(&self.world);
            self.screen.show_loaded(camera, &self.world, &self.tiles);
            if self.screen.view_size().is_big() {
                // `big_map`'s frame on the overview map.
                self.main_view_changed();
            }
            // The tools and the build menu from the file (1018:2a17).
            let rest = self.view_rest.clone();
            self.tools
                .start(&rest, &self.world, &self.font, &self.tiles, &self.info, camera.view);
            self.screen.draw_buttons(&self.tools.down);
            self.show_tools();
            self.screen
                .show_people(&self.world, &self.tiles, &self.font, &self.info);
            self.screen.show_info(&self.world, &self.font, &self.info);
            self.forms_tick(t);
            return true;
        }
        if t.load_failed {
            eprintln!("the saved game couldn't be loaded");
        }
        self.tick_sounds(t);
        let mut changed = !t.status.is_empty() || t.pointer_tool;
        changed |= if t.redraw_all {
            self.screen.paint_map(&self.world, &self.tiles);
            true
        } else {
            self.screen.redraw_squares(&t.redraw, &self.world, &self.tiles)
        };
        if t.auction_changes.contains(&Change::Opened) {
            self.open_auction();
            let r = self.forms.infobox.hide();
            self.play(r.sounds.iter().copied());
            changed |= r.refresh;
        }
        if !t.auction_changes.is_empty() {
            self.auction
                .update(&self.world, &t.auction_changes, &self.font, &self.info);
            changed = true;
        }
        changed | self.forms_tick(t)
    }

    /// A tick's sounds (`play_sound`, 10a0:30b6), with the countdown at
    /// the start of `people_tick` (1090:5293) where it came; the TV's
    /// lights while its speaker reads; and after a day the music's next
    /// track (1018:367e).
    fn tick_sounds(&mut self, t: &Tick) {
        let settings = self.forms.settings;
        {
            let mut s = self.sound.borrow_mut();
            let split = t.sound_countdown.unwrap_or(t.sounds.len()).min(t.sounds.len());
            s.play_all(t.sounds[..split].iter().copied(), &settings);
            if t.sound_countdown.is_some() {
                s.effects.count_down();
            }
            s.play_all(t.sounds[split..].iter().copied(), &settings);
            if t.new_day {
                s.next_day(&mut self.forms.settings);
            }
        }
        if let Some((lower, upper)) = t.tv_lights
            && self.forms.tv.visible
        {
            self.forms.tv.draw_lights(lower, upper);
        }
    }

    /// The auction form opens with the item's picture. // 1028:0dde
    fn open_auction(&mut self) {
        let item = usize::try_from(self.world.auction.item).ok();
        let picture = item.and_then(|n| self.pictures.get(n)).and_then(|name| {
            let path = self.dir.join("GRAFIK").join(auktionsform::picture_file(name));
            let bitmap = Bitmap::parse(&std::fs::read(path).ok()?).ok()?;
            Some(Surface::from_bitmap(&bitmap))
        });
        self.auction
            .open(&self.world, &self.font, &self.info, picture.as_ref(), self.size);
        self.auction_captured = false;
        // 1028:0dde hides the TV (1048:077a).
        let r = self.forms.tv.close();
        self.tv_response(r);
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
        let screen = self.size;
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
                self.tools.menu.mouse_move(shift, (sx, sy), self.size);
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
    #[allow(clippy::too_many_lines)] // Each image's handlers, as the original's.
    fn main_mouse(&mut self, input: Input, shift: ShiftState, buttons_up: bool) -> (bool, bool) {
        let (Input::MouseMove { x: sx, y: sy }
        | Input::MouseDown { x: sx, y: sy, .. }
        | Input::MouseUp { x: sx, y: sy, .. }) = input
        else {
            return (false, false);
        };
        let main = self.screen.bounds();
        let (fx, fy) = (sx - main.left, sy - main.top);
        // While it's rolled up to its title bar only that part is there.
        let rects = *self.screen.rects();
        let target = self.captured.or_else(|| {
            rects
                .iter()
                .filter(|_| (0..main.height()).contains(&fy))
                .find(|(_, r)| (r.left..r.right).contains(&fx) && (r.top..r.bottom).contains(&fy))
                .map(|(name, _)| *name)
        });
        let Some(target) = target else {
            return (false, false);
        };
        let bounds = rects
            .iter()
            .find(|(n, _)| *n == target)
            .map_or(Rect::default(), |(_, r)| *r);
        let (x, y) = (fx - bounds.left, fy - bounds.top);
        if target == "ButtonImage" || target == "Hauptbild" {
            self.bar_mouse(x); // 1018:3e46
        }
        match input {
            Input::MouseDown { .. } => {
                self.captured = Some(target);
                // The main screen takes the focus from the build menu,
                // the sabotage window and the other forms.
                self.set_focus(Focus::Main);
            }
            Input::MouseUp { .. } if buttons_up => self.captured = None,
            _ => {}
        }
        match (target, input) {
            // Dragging the form by its title bar. // 1020:1b8e, 1020:1bd9, 1020:1c58
            ("Ueberschriftbild", Input::MouseDown { .. }) => {
                self.screen.title_mouse_down(shift, (sx, sy));
                (true, false)
            }
            ("Ueberschriftbild", Input::MouseMove { .. }) => {
                (self.screen.title_mouse_move(shift, (sx, sy), self.size), false)
            }
            ("Ueberschriftbild", Input::MouseUp { .. }) => {
                self.screen.title_mouse_up();
                (true, false)
            }
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
                    self.main_view_changed();
                } else {
                    let cam = self.screen.drawn_camera();
                    let actions = self.tools.press(x, y, shift, &self.world, &self.tiles, &cam);
                    self.tool_actions(actions);
                    self.take_info_box();
                }
                (true, false)
            }
            ("Hauptbild", Input::MouseUp { button, .. }) => {
                let actions = self.tools.release(button == Button::Left);
                self.tool_actions(actions);
                let shown = self.forms.infobox.visible;
                self.take_info_box();
                (shown != self.forms.infobox.visible, false)
            }
            // The compass's knob (1038:3557).
            ("eckimage", Input::MouseDown { .. }) => {
                let view = self.screen.camera().view;
                let r = self.forms.compass.knob_mouse_down(x, y, view);
                self.play(r.sounds.iter().copied());
                if r.knob {
                    self.forms.compass.draw_knob(&mut self.screen.corner);
                }
                (true, false)
            }
            // The close button asks whether to quit (1020:0577).
            (
                "CloseBild",
                Input::MouseUp {
                    button: Button::Left, ..
                },
            ) if (0..bounds.width()).contains(&x) && (0..bounds.height()).contains(&y) => {
                self.open_ende();
                (true, false)
            }
            _ => (false, false),
        }
    }

    /// What `save_map` writes of this game. // 1048:2bae
    fn map(&self) -> SaveGame {
        let mut rest = self.view_rest.clone();
        self.tools.save(&mut rest);
        self.forms_save(&mut rest);
        self.world
            .to_map(&MapView::of(&self.screen, &self.forms.settings, rest))
    }

    /// A key in the game: the forms first (`forms`), then the build menu
    /// while it has the focus, then the main screen.
    fn game_key(&mut self, key: u16, shift: ShiftState) {
        // A key isn't a press: the buttons it clicks go by the order now.
        self.stack.before_press = None;
        if self.forms_key(key, shift) {
            return;
        }
        let mut key = Some(key);
        if self.tools.menu_focused {
            // `TBaumenue` has the focus. // 1018:3290
            let response = self.tools.menu.key_down(key.unwrap_or(0));
            key = self.menu_response(response);
        }
        if let Some(key) = key {
            self.main_screen_key(key, shift);
        }
    }

    /// The main screen's `FormKeyDown` (1020:2e09): the quit question,
    /// the view's keys and the button bar's.
    fn main_screen_key(&mut self, key: u16, shift: ShiftState) {
        if endeform::main_screen_opens(key, shift) {
            self.open_ende();
            return;
        }
        match self.screen.key_down(key, shift, &self.world, &self.tiles) {
            KeyResult::Redrawn => {
                self.screen
                    .show_people(&self.world, &self.tiles, &self.font, &self.info);
                // `set_view` (1010:254d) turns the build menu's picture
                // too.
                let view = self.screen.camera().view;
                self.tools
                    .menu
                    .view_turned(&self.world, &self.font, &self.tiles, &self.info, view);
                self.main_view_changed();
            }
            KeyResult::Button(hauptschirm::FILE_MENU_BUTTON) => self.forms.requests.push(Request::FileMenu),
            KeyResult::Button(b) => self.click_button(b),
            KeyResult::Ignored => {}
        }
    }

    /// `FormShow` of the file menu, from the button bar's first button
    /// (1010:2c8b): the clock stops, the other forms are disabled.
    /// // 1040:12be
    fn open_file_menu(&mut self, games: Vec<Vec<u8>>) {
        self.file_menu.menu.show(games);
        self.file_menu.visible = true;
        self.file_menu.captured = false;
    }

    /// Paints the game's windows in the order they lie
    /// ([`Stack::order`]). `background` is `THintergrund`, which the
    /// caller has painted under them; it's painted again where the
    /// original's stacking has raised it over some.
    fn paint(&mut self, screen: &mut Surface, palette: &Palette, mut background: Option<&mut Form>) {
        self.sync_stack();
        let order = self.stack.order(self.infobox_modal());
        for (n, w) in order.into_iter().enumerate() {
            match w {
                Win::Background => {
                    if n > 0
                        && let Some(b) = background.as_deref_mut()
                    {
                        b.paint(screen, palette);
                    }
                }
                Win::Main => {
                    self.form.components[0].bounds = self.screen.bounds();
                    for (name, bounds) in *self.screen.rects() {
                        if let Some(c) = self.form.component_mut(name) {
                            c.bounds = bounds;
                        }
                    }
                    for (name, image) in self.screen.images() {
                        if let Some(c) = self.form.component_mut(name)
                            && let Kind::Image { picture } = &mut c.kind
                        {
                            *picture = Some(image.clone());
                        }
                    }
                    self.form.paint(screen, palette);
                }
                Win::Menu => {
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
                }
                Win::Auction => {
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
                Win::FileMenu => {
                    let m = &mut self.file_menu;
                    m.form.components[0].bounds =
                        Rect::new(m.menu.left, m.menu.top, dateimenue::WIDTH, dateimenue::HEIGHT);
                    m.form.components[0].visible = m.visible;
                    if let Some(c) = m.form.component_mut("Dateimenuebild")
                        && let Kind::Image { picture } = &mut c.kind
                    {
                        *picture = Some(m.menu.image().clone());
                    }
                    m.form.paint(screen, palette);
                }
                w => self.forms.paint_window(w, self.tools.sabowin.as_ref(), screen, palette),
            }
        }
    }
}

/// Where `hintergrund_timer` (1000:2466) is in the intro.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Intro {
    /// States 400 and 410: `sunflow.avi` plays.
    Film,
    /// States 420 to 450, this long after the film.
    After(Duration),
    /// State 500 on: the start menu.
    Done,
}

/// A game to start once `titel.avi` is over (states 1111 to 1120).
enum Pending {
    /// The start menu's Start, on its scenario page or not.
    Start { scenario: bool },
    /// The load form's game.
    Load(SaveGame),
}

/// What `backgroundimage` shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Picture {
    /// Nothing yet: states 110 to 420.
    None,
    /// `tit<size>.BMP`: states 430 and 440.
    Title,
    /// `bck<size>.BMP`: state 450 on.
    Background,
}

struct Holiday {
    dir: PathBuf,
    exe: Vec<u8>,
    form: Form,
    game: Option<Game>,
    /// The desktop: the window's client area divided by the UI scale, at
    /// least [`platform::MIN_DESKTOP`] (a smaller window is letterboxed).
    /// The maximized `THintergrund` covers it, and the framebuffer is
    /// this big.
    desktop: (i32, i32),
    /// The screen size the game lays out for, from the desktop
    /// (`hintergrund::screen_size`, state 110).
    size: (i32, i32),
    picture: Picture,
    start: StartMenu,
    palette: Palette,
    rgb: [[u8; 3]; 256],
    screen: Surface,
    intro: Intro,
    /// `titel.avi` plays, and then this game starts.
    pending: Option<Pending>,
    /// The film's picture on the screen, in true colour.
    overlay: Option<(Rect, Vec<u8>)>,
    /// Mouse buttons and Shift, Ctrl and Alt, as a `TShiftState`.
    buttons: ShiftState,
    keys: ShiftState,
    quit: bool,
    /// `TLadenForm`, while it's open over the start menu.
    load: Option<LoadMenu>,
    saves: Saves,
    /// The last left press, for double clicks.
    last_press: Option<(Instant, i32, i32)>,
    /// The player's options, from game to game (`TOptionenform`).
    options: Options,
    sound: sound::Shared,
    /// The player's enhancements (`game::enhance`): the view's for each
    /// game, and the world's fixes a new game starts with.
    enhance: Config,
}

impl Holiday {
    /// `backgroundimage` shows `picture` for the screen size: the
    /// title's or the background's of [`hintergrund::layout`], or none
    /// for other sizes (states 430, 450 and 1130). The palette, which the
    /// whole game is drawn with, is the picture's; without one (or with
    /// `Picture::None`) it's the 800×600 one's, the port's stand-in for
    /// the palette Windows would have had then. // 1000:2466
    fn set_picture(&mut self, picture: Picture) {
        self.picture = picture;
        let layout = hintergrund::layout(self.size);
        let fallback = &hintergrund::LAYOUTS[0];
        let file = |l: &hintergrund::Layout| match picture {
            Picture::None | Picture::Title => l.title,
            Picture::Background => l.background,
        };
        let bitmap = bitmap(
            &self.dir,
            &format!("GRAFIK/{}", file(layout.unwrap_or(fallback)).to_ascii_uppercase()),
        );
        self.palette = bitmap.palette;
        self.rgb = bitmap.palette.map(|c| [c.r, c.g, c.b]);
        let shown = (picture != Picture::None && layout.is_some()).then(|| Surface::from_bitmap(&bitmap));
        if let Some(image) = self.form.component_mut("backgroundimage")
            && let Kind::Image { picture } = &mut image.kind
        {
            *picture = shown;
        }
        self.repaint();
    }

    /// Lays everything out for a desktop of `width` × `height`: the
    /// framebuffer, `THintergrund` maximized over it, the screen size
    /// and its pictures (state 110), the film panel centred on the form
    /// (state 110), the start menu (1058:1a30), the load form (1038:3b46)
    /// and the game (`Game::resize`). The original does this once at the
    /// start; the port again whenever the window's size changes.
    fn layout(&mut self, width: i32, height: i32) {
        let min = (
            i32::try_from(platform::MIN_DESKTOP.0).unwrap_or(640),
            i32::try_from(platform::MIN_DESKTOP.1).unwrap_or(480),
        );
        self.desktop = (width.max(min.0), height.max(min.1));
        let (w, h) = self.desktop;
        self.screen = Surface::new(usize::try_from(w).unwrap_or(0), usize::try_from(h).unwrap_or(0), 0);
        // State 110: the maximized form's size, snapped to a layout, is
        // the screen size; `backgroundimage` gets it at 0, 0.
        self.size = hintergrund::screen_size(w, h);
        self.form.components[0].bounds = Rect::new(0, 0, w, h);
        if let Some(image) = self.form.component_mut("backgroundimage") {
            image.bounds = Rect::new(0, 0, self.size.0, self.size.1);
        }
        if self.pending.is_some() || self.intro == Intro::Film {
            let panel = self.film_panel();
            self.sound.borrow_mut().set_film_panel(panel);
        }
        self.start.place(self.size);
        if let Some(load) = &mut self.load {
            load.place(self.size);
        }
        if let Some(game) = &mut self.game {
            game.resize(self.size, self.desktop);
        }
        self.set_picture(self.picture);
    }

    fn repaint(&mut self) {
        self.screen.pixels.fill(0);
        self.form.paint(&mut self.screen, &self.palette);
        if let Some(game) = &mut self.game {
            game.paint(&mut self.screen, &self.palette, Some(&mut self.form));
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
        // Without a window holding the mouse, the event goes to the top
        // window under it, and a press raises that one.
        game.sync_stack();
        let held = game.forms.grab.is_some() || game.auction_captured || game.menu_captured || game.captured.is_some();
        let top = match input {
            Input::MouseMove { x, y } | Input::MouseDown { x, y, .. } | Input::MouseUp { x, y, .. } if !held => {
                Some(game.window_at(x, y))
            }
            _ => None,
        };
        let mut raised = false;
        if let (Some(w), Input::MouseDown { .. }) = (top, input) {
            raised = game.press_window(w);
        }
        let to = |w: Win| top.is_none_or(|t| t == w);
        if game.forms_mouse(input, shift, up) {
            self.process_requests();
            self.repaint();
            return;
        }
        if to(Win::Auction) && game.auction_mouse(input, shift, up) {
            self.repaint();
            return;
        }
        if to(Win::Menu) && game.menu_mouse(input, shift, up) {
            self.repaint();
            return;
        }
        if !to(Win::Main) {
            if raised {
                self.repaint();
            }
            return;
        }
        let (changed, file_menu) = game.main_mouse(input, shift, up);
        if matches!(input, Input::MouseUp { .. }) && up {
            game.stack.before_press = None;
        }
        if file_menu {
            game.forms.requests.push(Request::FileMenu);
        }
        self.process_requests();
        if changed || file_menu || raised {
            self.repaint();
        }
    }

    /// Does what the game asks that needs more than the game.
    fn process_requests(&mut self) {
        loop {
            let Some(game) = &mut self.game else { return };
            if game.forms.requests.is_empty() {
                return;
            }
            let request = game.forms.requests.remove(0);
            match request {
                Request::FileMenu => {
                    let games = self.saves.games();
                    game.open_file_menu(games);
                }
                Request::Quit => {
                    // 1020:3a1b: `Last.map`, then the end film (1068:0f52).
                    if let Err(e) = self.saves.write(hintergrund::LAST_MAP.as_bytes(), &game.map()) {
                        eprintln!("can't save {}: {e}", hintergrund::LAST_MAP);
                    }
                    let r = game.forms.schluss.open_quit(game.size);
                    game.schluss_response(r);
                }
                Request::Exit => {
                    self.quit = true;
                    return;
                }
                Request::NewGame => {
                    self.back_to_start_menu();
                    return;
                }
                Request::SaveHighScores => {
                    let path = self.saves.high_scores();
                    if let Err(e) = std::fs::write(&path, game.forms.high.table.to_bytes()) {
                        eprintln!("can't save {}: {e}", path.display());
                    }
                }
            }
        }
    }

    /// A mouse event while the load form is open: the start menu is
    /// disabled, so only the form gets it.
    fn load_mouse(&mut self, input: Input) {
        let Some(load) = &mut self.load else { return };
        let (lx, ly) = LadenForm::position(self.size.0, self.size.1);
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
        if let Some(n) = response.sound {
            self.sound.borrow_mut().play(n.number(), &self.options);
        }
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
                        self.play_start_film(Pending::Load(map));
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
        let size = game.size;
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
            _ => m.menu.mouse_move(x, y, shift, size),
        };
        self.file_menu_response(response);
    }

    /// Does what the file menu asks.
    fn file_menu_response(&mut self, response: dateimenue::Response) {
        let Some(game) = &mut self.game else { return };
        game.sound
            .borrow_mut()
            .play_all(response.sounds.iter().map(|s| s.number()), &game.forms.settings);
        let refresh = response.refresh || response.moved || response.action.is_some();
        match response.action {
            Some(dateimenue::Action::Close) => game.file_menu.visible = false,
            Some(dateimenue::Action::Load(name)) => match self.saves.read(&name) {
                // 1040:0f27: load_map in the middle of the game, a command
                // to the world.
                Ok(map) => {
                    if World::from_map(&map).is_ok() {
                        game.view_rest = MapView::from_map(&map).unported;
                        game.forms.loading_settings = Some(game.forms.settings.with_map(&map));
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
                // 1040:0a4c: the menu hides and `TEndeform` asks.
                game.file_menu.visible = false;
                game.open_ende();
            }
            None => {}
        }
        if refresh {
            self.repaint();
        }
    }

    /// "Neues Spiel": saves `Last.map`, ends the game and shows a new
    /// start menu, set from the globals the game left, with `holiday.mid`
    /// again. Reloading the data (1030:38a3) isn't ported but for the
    /// sounds. // 1010:25dc
    fn back_to_start_menu(&mut self) {
        let Some(game) = self.game.take() else { return };
        if let Err(e) = self.saves.write(hintergrund::LAST_MAP.as_bytes(), &game.map()) {
            eprintln!("can't save {}: {e}", hintergrund::LAST_MAP);
        }
        self.options = game.forms.settings;
        // 1030:38a3 opens the sounds again (10a0:2f5e), and `holiday.mid`
        // plays from the start.
        {
            let mut s = self.sound.borrow_mut();
            s.reset_effects();
            s.speaker.close();
            s.stop_film();
            s.intro_music();
        }
        self.start = start_menu(&self.dir, &self.exe, Settings::from_world(&game.world));
        self.start.show(self.size);
        self.repaint();
    }

    /// `FilmPanel`, centred on the maximized form, which is the desktop
    /// (state 110).
    fn film_panel(&self) -> Rect {
        let (w, h) = self.desktop;
        Rect::new(
            (w - FILM_PANEL.0) / 2,
            (h - FILM_PANEL.1) / 2,
            FILM_PANEL.0,
            FILM_PANEL.1,
        )
    }

    /// State 1111: the `WaveMix` session gives up the wave device and
    /// `titel.avi` plays in `FilmPanel`, then `pending` starts; if the
    /// film doesn't open, at once (state 1120). // 1000:2466
    fn play_start_film(&mut self, pending: Pending) {
        let opened = {
            let mut s = self.sound.borrow_mut();
            s.effects_off(); // 10a0:2518
            s.play_film(START_FILM, self.film_panel())
        };
        self.pending = Some(pending);
        if !opened {
            self.start_pending();
        }
    }

    /// State 1120 and on: the film stops and closes, the session takes
    /// the wave device back (10a0:254f), and the game starts.
    fn start_pending(&mut self) {
        {
            let mut s = self.sound.borrow_mut();
            s.stop_film();
            s.effects_on();
        }
        self.overlay = None;
        let high = forms::high_scores(&self.dir, &self.saves.high_scores());
        match self.pending.take() {
            Some(Pending::Start { scenario }) => {
                self.game = Some(start_game(
                    &self.dir,
                    &self.exe,
                    self.start.menu.settings(),
                    scenario,
                    self.options,
                    high,
                    &self.sound,
                    self.size,
                    &self.enhance,
                ));
            }
            Some(Pending::Load(map)) => {
                let settings = self.start.menu.settings().clone();
                let options = self.options.with_map(&map);
                self.game = Some(load_game(
                    &self.dir,
                    &self.exe,
                    &map,
                    &settings,
                    options,
                    high,
                    &self.sound,
                    self.size,
                    &self.enhance,
                ));
            }
            None => {}
        }
        // `big_map`'s main screen fills the desktop, which can be a
        // little bigger or smaller than the screen size.
        let (size, desktop) = (self.size, self.desktop);
        if let Some(game) = &mut self.game {
            game.resize(size, desktop);
        }
        self.repaint();
    }

    /// State 420: the intro's film stops and closes.
    fn end_intro_film(&mut self) {
        self.sound.borrow_mut().stop_film();
        self.overlay = None;
        self.intro = Intro::After(Duration::ZERO);
        self.repaint();
    }

    /// The intro's states after the film, at `t` after it: the music, the
    /// title, then the background and the start menu, with the `WaveMix`
    /// session back (state 500).
    fn intro_step(&mut self, from: Duration, to: Duration) {
        if from < INTRO_MUSIC_AT && to >= INTRO_MUSIC_AT {
            // States 422 and 424: `holiday.mid`.
            self.sound.borrow_mut().intro_music();
        }
        if from < TITLE_AT && to >= TITLE_AT {
            // States 430 and 440.
            self.set_picture(Picture::Title);
        }
        if to >= MENU_AT {
            // States 450 and 500.
            self.intro = Intro::Done;
            self.sound.borrow_mut().effects_on(); // 10a0:254f
            self.start.show(self.size);
            self.set_picture(Picture::Background);
        } else {
            self.intro = Intro::After(to);
        }
    }

    /// `FormKeyDown` (1000:3d0c, 1000:3938) and the clicks on `FilmPanel`
    /// and `backgroundimage` (1000:39b7, 1000:3a36) during the intro or
    /// `titel.avi`: the film ends; a key also makes the timer's next
    /// state come at once. Whether it took the input.
    fn intro_input(&mut self, input: Input) -> bool {
        let key = matches!(input, Input::KeyDown(_));
        let click = matches!(
            input,
            Input::MouseUp {
                button: Button::Left,
                ..
            }
        );
        if self.pending.is_some() {
            if key || click {
                self.start_pending();
            }
            return true;
        }
        match self.intro {
            Intro::Film => {
                if key || click {
                    self.end_intro_film();
                }
                true
            }
            Intro::After(t) => {
                // BUG(original): `FormKeyDown` sets the timer's interval to 1 whatever the state, so a key cuts the wait for the title or the start menu short.
                if key {
                    let next = if t < TITLE_AT { TITLE_AT } else { MENU_AT };
                    self.intro_step(t, next);
                }
                true
            }
            Intro::Done => false,
        }
    }

    fn respond(&mut self, response: startform::Response) {
        if let Some(n) = response.sound {
            self.sound.borrow_mut().play(n.number(), &self.options);
        }
        match response.action {
            Some(Action::Start { scenario }) => {
                // `startform_start_click` hides the form, and hintergrund's
                // states 1000-1500 start the game.
                self.start.hide();
                self.play_start_film(Pending::Start { scenario });
            }
            Some(Action::Load) => {
                // 1058:0c7d: the start menu is disabled and TLadenForm
                // opens over it.
                self.load = Some(load_menu(&self.dir, &self.exe, self.saves.games(), self.size));
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
        let ms = u32::try_from(elapsed.as_millis()).unwrap_or(u32::MAX);
        self.sound.borrow_mut().advance(ms);
        if let Some(game) = &mut self.game
            && game.update(elapsed)
        {
            self.process_requests();
            self.repaint();
        }
        let film_done = self.sound.borrow().film_done();
        match self.intro {
            // State 410: `Position = Length`.
            Intro::Film if film_done => self.end_intro_film(),
            Intro::After(t) => self.intro_step(t, t + elapsed),
            _ => {}
        }
        // State 1113.
        if self.pending.is_some() && film_done {
            self.start_pending();
        }
        if let Some(panel) = self.game.as_ref().and_then(Game::film_panel) {
            self.sound.borrow_mut().set_film_panel(panel);
        }
        self.overlay = self.sound.borrow().film_picture(self.desktop);
    }

    fn input(&mut self, input: Input) {
        if !matches!(input, Input::Close) && self.intro_input(input) {
            return;
        }
        match input {
            Input::Close => self.quit = true,
            // In a game, Escape asks first (`TEndeform`).
            Input::KeyDown(VK_ESCAPE) if self.load.is_none() && self.game.is_none() => {
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
                        && !game.forms.modal()
                    {
                        // `TAuktionsform` is on top and has the keys.
                        let shift = self.buttons | self.keys;
                        if let Some(action) = game.auction.key_down(key, shift) {
                            game.auction_actions(vec![action]);
                        }
                    } else if let Some(game) = &mut self.game {
                        let shift = self.buttons | self.keys;
                        game.game_key(key, shift);
                        self.process_requests();
                        self.repaint();
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
            overlay: self.overlay.as_ref().map(|(r, rgb)| platform::Overlay {
                x: r.left,
                y: r.top,
                width: u32::try_from(r.width()).unwrap_or(0),
                height: u32::try_from(r.height()).unwrap_or(0),
                rgb,
            }),
        }
    }

    fn finished(&self) -> bool {
        self.quit
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.layout(
            i32::try_from(width).unwrap_or(i32::MAX),
            i32::try_from(height).unwrap_or(i32::MAX),
        );
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
/// bottom of the `screen`, `Bild` filling it. // 1038:3b46
fn load_menu(dir: &Path, exe: &[u8], games: Vec<Vec<u8>>, screen: (i32, i32)) -> LoadMenu {
    let texts = sonstige(&read(dir, "TEXTE/SONSTIGE.TXT")).expect("SONSTIGE.TXT");
    let load = texts.texts.get(7).cloned().unwrap_or_default();
    let menu = LadenForm::new(font(dir), bitmaps(dir, &ladenform::BITMAP_FILES), &load, games);
    let mut form = load_form(exe, "TLADENFORM");
    form.components[0].visible = true;
    if let Some(bild) = form.component_mut("Bild") {
        bild.bounds = Rect::new(0, 0, ladenform::WIDTH, ladenform::HEIGHT);
        bild.visible = true;
    }
    let mut load = LoadMenu {
        menu,
        form,
        captured: false,
    };
    load.place(screen);
    load
}

impl LoadMenu {
    /// At the bottom of the `screen`, centred across it (1038:3b46); the
    /// port also does it when the window's size changes.
    fn place(&mut self, (w, h): (i32, i32)) {
        let (left, top) = LadenForm::position(w, h);
        self.form.components[0].bounds = Rect::new(left, top, ladenform::WIDTH, ladenform::HEIGHT);
    }
}

/// `TDateimenue` as `Application.CreateForm` makes it, hidden, at the
/// DFM's position. // 1040:154d
fn file_menu(dir: &Path, exe: &[u8]) -> FileMenu {
    let texts = sonstige(&read(dir, "TEXTE/SONSTIGE.TXT")).expect("SONSTIGE.TXT");
    // BUG(original): the DFM's place on the screen (305, 139), which isn't over the centred main screen on a desktop bigger than 640×480.
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
        sabotage: game::sabotage::kinds(
            &formats::data::sabotage(&read(dir, "TEXTE/SABOTAGE.TXT"))
                .expect("SABOTAGE.TXT")
                .1,
        ),
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
fn start_world(dir: &Path, settings: &Settings, scenario_page: bool, options: &mut Options) -> Start {
    let seed = seed();
    match hintergrund::new_island(settings, scenario_page) {
        Island::Scenario(file) => {
            let path = format!("SZENARIO/{}", file.to_ascii_uppercase());
            let map = SaveGame::parse(&read(dir, &path)).unwrap_or_else(|e| panic!("{path}: {e}"));
            let mut world =
                hintergrund::load_world(&map, settings, scenario_page).unwrap_or_else(|e| panic!("{path}: {e}"));
            world.random = Random::new(seed);
            world.tables = world_tables(dir);
            world.news.date_format = game::news::DateFormat {
                months: monate(&read(dir, "TEXTE/MONATE.TXT")).expect("MONATE.TXT"),
                land: modatyp(&read(dir, "DATA/MODATYP.DAT")).expect("MODATYP.DAT").land as u8,
            };
            *options = options.with_map(&map);
            // State 1500 for a scenario: the music and the speech on.
            options.music = true;
            options.speech = true;
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
                owner_pointers: options.owner_pointers,
                occupancy_pointers: options.occupancy_pointers,
            };
            (world, camera, None, Fields::default())
        }
    }
}

/// States 1000 to 1500 with `hintergrund+0x190`: the saved game `map`
/// over a newly set up game, and the main screen at the file's view.
/// // 1000:2466
#[allow(clippy::too_many_arguments)]
fn load_game(
    dir: &Path,
    exe: &[u8],
    map: &SaveGame,
    settings: &Settings,
    options: Options,
    high: HighScores,
    sound: &sound::Shared,
    size: (i32, i32),
    config: &Config,
) -> Game {
    let data = hintergrund::GameData::load(|name| std::fs::read(dir.join(name)).ok()).unwrap_or_else(|e| panic!("{e}"));
    let mut world =
        hintergrund::load_saved_game(map, settings, &data, seed()).unwrap_or_else(|e| panic!("the saved game: {e}"));
    // The file's fixes; the player's for those it doesn't name (all of
    // them in an original file).
    world.fixes = enhance::fixes_in(&map.trailing, config.fixes);
    let view = MapView::from_map(map);
    let camera = view.camera(&world);
    let mut game = make_game(
        dir,
        exe,
        (world, camera, Some(camera.origin), view.unported),
        options,
        high,
        sound,
        size,
    );
    game.set_view_enhancements(config.view);
    game
}

/// What hintergrund's timer does from state 1200 to 1500 once Start is
/// clicked and `titel.avi` is over, without the progress window
/// (`TWasNunForm`) and what isn't ported yet: load the scenario's map or
/// generate an island, set up the main screen and show it. // 1000:2466
#[allow(clippy::too_many_arguments)]
fn start_game(
    dir: &Path,
    exe: &[u8],
    settings: &Settings,
    scenario_page: bool,
    mut options: Options,
    high: HighScores,
    sound: &sound::Shared,
    size: (i32, i32),
    config: &Config,
) -> Game {
    let mut start = start_world(dir, settings, scenario_page, &mut options);
    // A new game, and a scenario (whose file has no fixes), plays with
    // the player's.
    start.0.fixes = config.fixes;
    let mut game = make_game(dir, exe, start, options, high, sound, size);
    game.set_view_enhancements(config.view);
    game
}

/// The main screen and the forms of a game that starts from `start`.
/// // 1020:2501, 1010:2820
#[allow(clippy::too_many_lines)]
fn make_game(
    dir: &Path,
    exe: &[u8],
    (world, camera, origin, view_rest): Start,
    mut options: Options,
    high: HighScores,
    sound: &sound::Shared,
    size: (i32, i32),
) -> Game {
    {
        let mut s = sound.borrow_mut();
        s.music.init_playlist(&mut options); // 1018:37de, from 1030:36e5
        // A scenario's or a saved game's `game_start_setup` (1010:2820).
        if origin.is_some() {
            s.game_start(&options);
        }
    }
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
    screen.layout(size);
    form.components[0].bounds = screen.bounds();
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
        size,
    );
    let mut tools = Tools::new(menu, fehler(&read(dir, "TEXTE/FEHLER.TXT")).expect("FEHLER.TXT"));
    tools.sabowin = Some(Forms::sabowin(dir, &font, &info));
    tools.start(&view_rest, &world, &font, &tiles, &info, screen.camera().view);
    screen.draw_buttons(&tools.down);
    tools.buttons_changed = false;
    let mut menu_form = load_form(exe, "TBAUMENUE");
    menu_form.components[0].visible = false;
    // The other forms, and what the map holds of them.
    let mut forms = Forms::load(dir, exe, &font, &info, options, high, &screen, size);
    forms.overview.load(&view_rest); // 10b8:4f1c
    forms.overview.game_start(&world);
    forms.tv.load_fields(&view_rest);
    // 1010:0ac1: the TV closes, which lets the game go on.
    let closed = forms.tv.close();
    sound.borrow_mut().tv_media(&closed.media, &forms.settings);
    let commands = closed
        .actions
        .into_iter()
        .map(|action| Command { player: 1, action })
        .collect();
    forms.compass.draw_knob(&mut screen.corner);
    let _ = forms.money.loaded(&world);
    Game {
        forms,
        // The caller sets the player's (`start_game`, `load_game`).
        stack: Stack::new(enhance::Enhancements::ORIGINAL),
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
        commands,
        file_menu: file_menu(dir, exe),
        view_rest,
        sound: Rc::clone(sound),
        size,
        desktop: size,
    }
}

/// The command line's options.
struct Args {
    dir: PathBuf,
    save_dir: Option<PathBuf>,
    soundfont: Option<PathBuf>,
    quality: setup::Sound,
    films: setup::Film,
    no_audio: bool,
    window: platform::WindowOptions,
    enhance: Config,
}

/// The enhancements, one per line: key, whose, on or off in `config`, and
/// what it does (`--enhancements`).
fn enhancement_list(config: &Config) -> String {
    use std::fmt::Write;
    enhance::all().fold(String::new(), |mut out, i| {
        let side = match i.side {
            enhance::Side::World => "world",
            enhance::Side::View => "view",
        };
        let on = if config.is_on(i.key) == Some(true) { "on" } else { "off" };
        let _ = writeln!(out, "{:<24} {side:<5} {on:<3}  {}", i.key, i.description);
        out
    })
}

/// The enhancements: settings.ini's switches, then the command line's.
/// `--original` (`original`) switches everything off whatever the file
/// says, and each `--enhance=` (`overrides`) goes over both.
fn enhance_config(settings_file: Option<&Path>, original: bool, overrides: &[(String, bool)]) -> Config {
    let saved = settings_file
        .map(platform::settings::EnhanceSettings::load)
        .unwrap_or_default();
    for line in &saved.bad {
        eprintln!("settings.ini: not on or off: {line}");
    }
    let from_file = if original { Vec::new() } else { saved.overrides };
    let (config, unknown) = Config::resolve(
        original || saved.original,
        from_file.iter().chain(overrides).map(|(k, on)| (k.as_str(), *on)),
    );
    for key in unknown {
        eprintln!("settings.ini: no enhancement {key}");
    }
    config
}

fn args() -> Args {
    let mut dir = PathBuf::from("orig");
    let mut save_dir = None;
    let mut soundfont = None;
    let mut quality = None;
    let mut films = None;
    let mut no_audio = false;
    let mut original = false;
    let mut overrides: Vec<(String, bool)> = Vec::new();
    let mut list = false;
    let mut window = platform::WindowOptions {
        settings_file: platform::settings::settings_file(),
        ..platform::WindowOptions::default()
    };
    let fail = |message: &str| -> ! {
        eprintln!("{message}");
        std::process::exit(2);
    };
    for arg in std::env::args_os().skip(1) {
        let a = arg.to_str().unwrap_or_default();
        if let Some(size) = a.strip_prefix("--window-size=") {
            let parsed = size
                .split_once(['x', 'X'])
                .and_then(|(w, h)| Some((w.parse::<u32>().ok()?, h.parse::<u32>().ok()?)))
                .filter(|&(w, h)| w > 0 && h > 0);
            window.size = Some(parsed.unwrap_or_else(|| fail("--window-size= takes WIDTHxHEIGHT")));
        } else if a == "--fullscreen" {
            window.fullscreen = Some(true);
        } else if a == "--windowed" {
            window.fullscreen = Some(false);
        } else if let Some(n) = a.strip_prefix("--ui-scale=") {
            let n = n
                .parse::<u32>()
                .ok()
                .filter(|n| (1..=platform::MAX_UI_SCALE).contains(n));
            window.ui_scale = Some(n.unwrap_or_else(|| fail("--ui-scale= takes 1 to 4")));
        } else if let Some(d) = a.strip_prefix("--save-dir=") {
            save_dir = Some(PathBuf::from(d));
        } else if let Some(f) = a.strip_prefix("--soundfont=") {
            soundfont = Some(PathBuf::from(f));
        } else if let Some(q) = a.strip_prefix("--sound=") {
            quality = match q {
                "hi" => Some(setup::Sound::Hi),
                "lo" => Some(setup::Sound::Lo),
                "off" => Some(setup::Sound::Off),
                _ => {
                    eprintln!("--sound= takes hi, lo or off");
                    std::process::exit(2);
                }
            };
        } else if let Some(q) = a.strip_prefix("--films=") {
            films = match q {
                "hi" => Some(setup::Film::Hi),
                "lo" => Some(setup::Film::Lo),
                _ => {
                    eprintln!("--films= takes hi or lo");
                    std::process::exit(2);
                }
            };
        } else if a == "--no-audio" {
            no_audio = true;
        } else if a == "--original" {
            original = true;
        } else if a == "--enhancements" {
            list = true;
        } else if let Some(e) = a.strip_prefix("--enhance=") {
            let (key, on) = e.split_once('=').unwrap_or((e, "on"));
            let on = platform::settings::switch(on).unwrap_or_else(|| fail("--enhance= takes KEY=on or KEY=off"));
            if Config::default().is_on(key).is_none() {
                fail(&format!(
                    "--enhance=: no enhancement {key}; there are:\n{}",
                    enhancement_list(&Config::default())
                ));
            }
            overrides.push((key.to_owned(), on));
        } else {
            dir = PathBuf::from(arg);
        }
    }
    // The installer's choices (`SETUP.DAT`, 1000:17bc), if there are
    // any; else the full quality: its fifth line for the sound, its
    // fourth for the films (`film_dir`, 1110:502a).
    let installed = std::fs::read(dir.join("SETUP.DAT"))
        .ok()
        .map(|d| setup::Setup::parse(&d));
    let quality = quality.unwrap_or_else(|| installed.as_ref().map_or(setup::Sound::Hi, |s| s.sound));
    let films = films.unwrap_or_else(|| installed.as_ref().map_or(setup::Film::Hi, |s| s.film));
    let enhance = enhance_config(window.settings_file.as_deref(), original, &overrides);
    if list {
        print!("{}", enhancement_list(&enhance));
        std::process::exit(0);
    }
    Args {
        dir,
        save_dir,
        soundfont,
        quality,
        films,
        no_audio,
        window,
        enhance,
    }
}

fn main() {
    let Args {
        dir,
        save_dir,
        soundfont,
        quality,
        films,
        no_audio,
        window,
        enhance,
    } = args();
    let audio = if no_audio {
        platform::audio::Audio::silent()
    } else {
        platform::audio::Audio::open(platform::audio::find_soundfont(soundfont.as_deref()).as_deref())
    };
    let mut sound = Sound::new(&dir, quality, audio);
    sound.film_quality = films;
    let sound = sound.shared();
    let saves = Saves {
        dir: save_dir
            .or_else(platform::storage::save_dir)
            .unwrap_or_else(|| dir.join("MAPS")),
        game_maps: dir.join("MAPS"),
    };
    let mut holiday = Holiday::new(dir, saves, sound, enhance);
    holiday.start_intro();
    if let Err(e) = platform::run("Holiday Island", window, holiday) {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

impl Holiday {
    /// The game before the intro: `THintergrund` from its DFM, shown
    /// maximized over an 800×600 desktop until the platform says how big
    /// the window is ([`App::resize`]), and the start menu, hidden.
    fn new(dir: PathBuf, saves: Saves, sound: sound::Shared, enhance: Config) -> Holiday {
        let exe = read(&dir, "HOLIDAY.EXE");
        let mut form = load_form(&exe, "THINTERGRUND");
        // What states 110 and 400 do to the form and its image.
        form.components[0].visible = true;
        if let Some(image) = form.component_mut("backgroundimage") {
            image.visible = true;
        }
        let start = start_menu(&dir, &exe, Settings::default());
        let mut holiday = Holiday {
            dir,
            exe,
            form,
            game: None,
            desktop: (0, 0),
            size: (0, 0),
            picture: Picture::None,
            start,
            palette: [vcl::Rgb::default(); 256],
            rgb: [[0; 3]; 256],
            screen: Surface::new(0, 0, 0),
            intro: Intro::Done,
            pending: None,
            overlay: None,
            buttons: ShiftState::default(),
            keys: ShiftState::default(),
            quit: false,
            load: None,
            saves,
            last_press: None,
            options: Options::default(),
            sound,
            enhance,
        };
        let (w, h) = platform::DEFAULT_DESKTOP;
        holiday.layout(i32::try_from(w).unwrap_or(800), i32::try_from(h).unwrap_or(600));
        holiday
    }

    /// State 100: the `WaveMix` session gives up the wave device until the
    /// start menu (10a0:2518); state 400: `sunflow.avi`, or state 420 if
    /// it doesn't open. The title's palette, with no picture until state
    /// 430.
    fn start_intro(&mut self) {
        let panel = self.film_panel();
        self.intro = {
            let mut s = self.sound.borrow_mut();
            s.effects_off();
            if s.play_film(INTRO_FILM, panel) {
                Intro::Film
            } else {
                Intro::After(Duration::ZERO)
            }
        };
        self.set_picture(Picture::None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The screen the older tests lay out for.
    const SIZE: (i32, i32) = (800, 600);

    /// A scenario's main screen with the TV (opened by a message), the
    /// money menu, the overview map, the compass and the sabotage window
    /// open, through the game's own wiring. With `HOLIDAY_PPM` set, the
    /// screen goes there as `integrated.ppm`. Skipped without the game's
    /// files in `orig/`.
    #[test]
    fn forms_on_the_main_screen() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../orig");
        if !dir.join("GRAFIK").exists() {
            eprintln!("skipping: {} not found", dir.display());
            return;
        }
        let exe = read(&dir, "HOLIDAY.EXE");
        let settings = Settings {
            scenario: 1,
            ..Settings::default()
        };
        let high = forms::high_scores(&dir, Path::new("/nonexistent/HIGHSCOR.WAV"));
        let sound = Sound::new(&dir, setup::Sound::Hi, platform::audio::Audio::silent()).shared();
        let mut game = start_game(
            &dir,
            &exe,
            &settings,
            true,
            Options::default(),
            high,
            &sound,
            SIZE,
            &Config::ORIGINAL,
        );
        game.world.clock.paused = false;
        let _ = game.update(Duration::from_millis(u64::from(TICK_MS) * 5));
        // A message of kind 0 opens the TV, which asks for the pause.
        let mut tick = Tick::default();
        let pos = game.world.viewpoint.origin;
        game.world.post_message(
            game::news::Message {
                number: 0,
                a: 1,
                pos: Some((pos.0 + 12, pos.1 + 8)),
                ..game::news::Message::default()
            },
            &mut tick,
        );
        assert!(game.show(&tick) || game.forms.tv.visible);
        assert!(game.forms.tv.visible && game.tools.down[3]);
        let _ = game.update(Duration::ZERO);
        assert!(game.world.clock.paused, "the TV pauses the game");
        // Buttons 4 and 5, the compass's knob and the sabotage tool.
        game.click_button(4);
        assert!(game.forms.money.shown() && game.tools.down[4]);
        game.click_button(5);
        assert!(game.forms.overview.visible() && game.tools.down[5]);
        let (left, top) = (game.screen.left, game.screen.top);
        let corner = hauptschirm::LAYOUT[7].1;
        let at = (left + corner.left + 8, top + corner.top + 8);
        let press = Input::MouseDown {
            x: at.0,
            y: at.1,
            button: Button::Left,
        };
        let release = Input::MouseUp {
            x: at.0,
            y: at.1,
            button: Button::Left,
        };
        let _ = game.main_mouse(press, ShiftState::LEFT, false);
        let _ = game.main_mouse(release, ShiftState::default(), true);
        assert!(game.forms.compass.visible());
        game.click_button(21);
        assert!(game.tools.sabowin.as_ref().is_some_and(|s| s.visible));
        let _ = game.update(Duration::from_millis(u64::from(TICK_MS)));
        // The pause button stops the clock until a key ends it.
        game.click_button(1);
        assert!(game.clock_stopped());
        game.game_key(0x41, ShiftState::default());
        assert!(!game.clock_stopped() && !game.tools.down[1]);
        // The money menu's button closes it again; the save keeps the
        // news options.
        game.forms.settings.news = [true, false, true, false];
        let map = game.map();
        assert_eq!(MapView::from_map(&map).news, [true, false, true, false]);

        let background = bitmap(&dir, "GRAFIK/BCK80X60.BMP");
        let mut screen = Surface::new(800, 600, 0);
        game.paint(&mut screen, &background.palette, None);
        if let Some(out) = std::env::var_os("HOLIDAY_PPM") {
            let mut ppm = format!("P6\n{} {}\n255\n", screen.width, screen.height).into_bytes();
            for &i in &screen.pixels {
                let c = background.palette[usize::from(i)];
                ppm.extend_from_slice(&[c.r, c.g, c.b]);
            }
            std::fs::write(PathBuf::from(out).join("integrated.ppm"), ppm).unwrap();
        }
    }

    /// A message with a film (4, `haitauch.avi`) opens the TV, which
    /// plays the film in `Panel1` 6 s later and closes when it's over,
    /// which ends the pause. With `HOLIDAY_PPM` set, the screen with the
    /// film goes there as `films-tv.ppm`. Skipped without the game's files
    /// in `orig/`.
    #[test]
    fn a_news_film_on_the_tv() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../orig");
        if !dir.join("FILMHI").exists() {
            eprintln!("skipping: {} not found", dir.display());
            return;
        }
        let exe = read(&dir, "HOLIDAY.EXE");
        let settings = Settings {
            scenario: 1,
            ..Settings::default()
        };
        let high = forms::high_scores(&dir, Path::new("/nonexistent/HIGHSCOR.WAV"));
        let sound = Sound::new(&dir, setup::Sound::Hi, platform::audio::Audio::silent()).shared();
        let mut game = start_game(
            &dir,
            &exe,
            &settings,
            true,
            Options::default(),
            high,
            &sound,
            SIZE,
            &Config::ORIGINAL,
        );
        game.forms.settings.speech = false;
        game.world.clock.paused = false;
        let mut tick = Tick::default();
        game.world.post_message(
            game::news::Message {
                number: 4,
                ..game::news::Message::default()
            },
            &mut tick,
        );
        let _ = game.show(&tick);
        assert!(game.forms.tv.visible);
        let step = Duration::from_millis(u64::from(TICK_MS));
        let mut ms = 0;
        while !game.forms.tv.film_playing() {
            sound.borrow_mut().advance(TICK_MS);
            let _ = game.update(step);
            ms += TICK_MS;
            assert!(ms <= 7000, "the film starts 6 s after the TV opened");
        }
        assert!(game.world.clock.paused, "the TV pauses the game");
        sound.borrow_mut().advance(4000);
        let panel = game.film_panel().unwrap();
        sound.borrow_mut().set_film_panel(panel);
        let (rect, rgb) = sound.borrow().film_picture(SIZE).unwrap();
        // 335 × 257, cut to the panel's 332 × 256.
        let tv = game.forms.tv.bounds();
        assert_eq!(rect, Rect::new(tv.left + 9, tv.top + 25, 332, 256));
        if let Some(out) = std::env::var_os("HOLIDAY_PPM") {
            let background = bitmap(&dir, "GRAFIK/BCK80X60.BMP");
            let mut screen = Surface::new(800, 600, 0);
            game.paint(&mut screen, &background.palette, None);
            let mut ppm = format!("P6\n{} {}\n255\n", screen.width, screen.height).into_bytes();
            for (i, &p) in screen.pixels.iter().enumerate() {
                let (x, y) = (
                    i32::try_from(i % screen.width).unwrap(),
                    i32::try_from(i / screen.width).unwrap(),
                );
                if (rect.left..rect.left + rect.width()).contains(&x)
                    && (rect.top..rect.top + rect.height()).contains(&y)
                {
                    let at = usize::try_from((y - rect.top) * rect.width() + x - rect.left).unwrap() * 3;
                    ppm.extend_from_slice(&rgb[at..at + 3]);
                } else {
                    let c = background.palette[usize::from(p)];
                    ppm.extend_from_slice(&[c.r, c.g, c.b]);
                }
            }
            std::fs::write(PathBuf::from(out).join("films-tv.ppm"), ppm).unwrap();
        }
        // `haitauch.avi` is 8 s long; the next clock tick after its end
        // closes the TV.
        while game.forms.tv.visible {
            sound.borrow_mut().advance(TICK_MS);
            let _ = game.update(step);
            ms += TICK_MS;
            assert!(ms <= 20_000, "the TV closes after the film");
        }
        assert!(game.film_panel().is_none() && sound.borrow().film_picture(SIZE).is_none());
        let _ = game.update(step);
        assert!(!game.world.clock.paused, "the pause ends with the TV");
    }

    /// The quit film (`ABEnde.avi`) plays in the end form's `Panel1`; a
    /// key skips it, and the game exits. Skipped without the game's files.
    #[test]
    fn the_quit_film() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../orig");
        if !dir.join("FILMHI").exists() {
            eprintln!("skipping: {} not found", dir.display());
            return;
        }
        let exe = read(&dir, "HOLIDAY.EXE");
        let high = forms::high_scores(&dir, Path::new("/nonexistent/HIGHSCOR.WAV"));
        let sound = Sound::new(&dir, setup::Sound::Hi, platform::audio::Audio::silent()).shared();
        let mut game = start_game(
            &dir,
            &exe,
            &Settings::default(),
            false,
            Options::default(),
            high,
            &sound,
            SIZE,
            &Config::ORIGINAL,
        );
        let r = game.forms.schluss.open_quit(SIZE);
        game.schluss_response(r);
        let panel = game.forms.schluss.film_panel().unwrap();
        assert_eq!((panel.width(), panel.height()), (432, 320));
        sound.borrow_mut().advance(1000);
        let (rect, _) = sound.borrow().film_picture(SIZE).unwrap();
        assert_eq!(rect, panel);
        assert!(!sound.borrow().film_done() && !sound.borrow().effects.active());
        assert!(game.forms_key(0x20, ShiftState::default()));
        assert!(sound.borrow().film_done() && sound.borrow().effects.active());
        assert!(matches!(game.forms.requests.last(), Some(Request::Exit)));
    }

    /// The game's files, or none (the test is skipped).
    fn game_dir() -> Option<PathBuf> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../orig");
        if dir.join("GRAFIK").exists() {
            Some(dir)
        } else {
            eprintln!("skipping: {} not found", dir.display());
            None
        }
    }

    /// The whole program on a `width` × `height` desktop, past the intro,
    /// with scenario 1 started and the TV (opened by a message), the money
    /// menu, the overview map and the compass open.
    fn holiday_in_a_game(dir: &Path, (width, height): (i32, i32), enhance: Config) -> Holiday {
        let sound = Sound::new(dir, setup::Sound::Hi, platform::audio::Audio::silent()).shared();
        let saves = Saves {
            dir: std::env::temp_dir().join("holiday-layout-test-saves"),
            game_maps: dir.join("MAPS"),
        };
        let mut h = Holiday::new(dir.to_path_buf(), saves, sound, enhance);
        h.resize(u32::try_from(width).unwrap(), u32::try_from(height).unwrap());
        h.intro_step(Duration::ZERO, MENU_AT);
        let settings = Settings {
            scenario: 1,
            ..Settings::default()
        };
        let high = forms::high_scores(dir, Path::new("/nonexistent/HIGHSCOR.WAV"));
        h.start.hide();
        let mut game = start_game(
            dir,
            &h.exe,
            &settings,
            true,
            Options::default(),
            high,
            &h.sound,
            h.size,
            &enhance,
        );
        let mut tick = Tick::default();
        let pos = game.world.viewpoint.origin;
        game.world.post_message(
            game::news::Message {
                number: 0,
                a: 1,
                pos: Some((pos.0 + 12, pos.1 + 8)),
                ..game::news::Message::default()
            },
            &mut tick,
        );
        let _ = game.show(&tick);
        game.click_button(4);
        game.click_button(5);
        let corner = game.screen.rects()[7].1;
        let at = (game.screen.left + corner.left + 8, game.screen.top + corner.top + 8);
        let press = Input::MouseDown {
            x: at.0,
            y: at.1,
            button: Button::Left,
        };
        let release = Input::MouseUp {
            x: at.0,
            y: at.1,
            button: Button::Left,
        };
        let _ = game.main_mouse(press, ShiftState::LEFT, false);
        let _ = game.main_mouse(release, ShiftState::default(), true);
        // The money menu's page once its command is applied.
        let _ = game.update(Duration::ZERO);
        assert!(game.forms.tv.visible && game.forms.money.shown());
        assert!(game.forms.overview.visible() && game.forms.compass.visible());
        h.game = Some(game);
        h.repaint();
        h
    }

    /// The bounds of the forms that are shown.
    fn shown_forms(game: &Game) -> Vec<(&'static str, Rect)> {
        let f = &game.forms;
        [
            ("main", true, game.screen.bounds()),
            ("tv", f.tv.visible, f.tv.bounds()),
            ("money", f.money.shown(), f.money.bounds()),
            ("overview", f.overview.visible(), f.overview.bounds()),
            ("compass", f.compass.visible(), f.compass.bounds()),
            ("menu", game.tools.menu.height > 0, game.tools.menu.bounds()),
            ("auction", game.auction.visible, game.auction.bounds()),
        ]
        .into_iter()
        .filter(|&(_, shown, _)| shown)
        .map(|(name, _, r)| (name, r))
        .collect()
    }

    /// Writes the framebuffer as `name.ppm` into `HOLIDAY_PPM`, if set.
    fn write_ppm(h: &Holiday, name: &str) {
        let Some(out) = std::env::var_os("HOLIDAY_PPM") else {
            return;
        };
        let mut ppm = format!("P6\n{} {}\n255\n", h.screen.width, h.screen.height).into_bytes();
        for &i in &h.screen.pixels {
            ppm.extend_from_slice(&h.rgb[usize::from(i)]);
        }
        std::fs::write(PathBuf::from(out).join(format!("{name}.ppm")), ppm).unwrap();
    }

    /// The main screen with the TV, the money menu, the overview map and
    /// the compass open on four desktops: the original's 640×480, 800×600
    /// and 1024×768, and 1366×700, which has no pictures. With
    /// `HOLIDAY_PPM` set, each goes there as `layout-WxH.ppm`.
    #[test]
    fn layouts() {
        let Some(dir) = game_dir() else { return };
        for (size, main_at, background) in [
            ((640, 480), (0, 1), true),
            ((800, 600), (80, 61), true),
            ((1024, 768), (192, 145), true),
            ((1366, 700), (363, 111), false),
        ] {
            let h = holiday_in_a_game(&dir, size, Config::ORIGINAL);
            assert_eq!(h.size, size, "{size:?} is laid out for as it is");
            let game = h.game.as_ref().unwrap();
            assert_eq!((game.screen.left, game.screen.top), main_at, "centred");
            // The money menu and the compass at the top right edge
            // (1070:2e6c, 1038:2c04); the TV where the scenario's file
            // put it.
            assert_eq!(game.forms.compass.bounds().right, size.0);
            assert_eq!(game.forms.money.bounds().top, 0);
            let has_picture = h
                .form
                .component("backgroundimage")
                .is_some_and(|c| matches!(c.kind, Kind::Image { picture: Some(_) }));
            assert_eq!(has_picture, background);
            write_ppm(&h, &format!("layout-{}x{}", size.0, size.1));
        }
    }

    /// A smaller window than 640×480 gets a 640×480 desktop (which the
    /// platform letterboxes), and one a little off a layout snaps to it.
    #[test]
    fn small_and_snapped_desktops() {
        let Some(dir) = game_dir() else { return };
        let h = holiday_in_a_game(&dir, (500, 400), Config::ORIGINAL);
        assert_eq!((h.desktop, h.size), ((640, 480), (640, 480)));
        assert_eq!((h.screen.width, h.screen.height), (640, 480));
        let h = holiday_in_a_game(&dir, (790, 590), Config::ORIGINAL);
        assert_eq!((h.desktop, h.size), ((790, 590), (800, 600)));
        // BUG(original): laid out for 800×600 on 790×590, the compass's right edge is off the screen.
        assert_eq!(h.game.as_ref().unwrap().forms.compass.bounds().right, 800);
    }

    /// Resizing in the middle of a game: the world doesn't change, nor
    /// does the scroll position (the main screen shows the same part of
    /// the island at every size), the main screen is centred again and
    /// every form stays on the screen.
    #[test]
    fn resize_mid_game() {
        let Some(dir) = game_dir() else { return };
        let mut h = holiday_in_a_game(&dir, (800, 600), Config::ORIGINAL);
        {
            let game = h.game.as_mut().unwrap();
            game.screen.scroll(5, -3, &game.world, &game.tiles);
            game.forms.compass.left = 0;
            game.forms.compass.top = 400;
        }
        let game = h.game.as_ref().unwrap();
        let world = game.world.clone();
        let scroll = game.screen.scroll_position();
        let camera = game.screen.camera();
        let compass = game.forms.compass.bounds();
        h.resize(1024, 768);
        {
            let game = h.game.as_ref().unwrap();
            assert_eq!((game.screen.left, game.screen.top), (192, 145));
            // It follows the main screen.
            let moved = game.forms.compass.bounds();
            assert_eq!((moved.left, moved.top), (compass.left + 112, compass.top + 84));
        }
        for (w, hh) in [(1024, 768), (640, 480), (1366, 700), (700, 500), (800, 600)] {
            h.resize(w, hh);
            let game = h.game.as_ref().unwrap();
            let size = h.size;
            assert!(game.world == world, "the world doesn't see the window");
            assert_eq!(game.screen.scroll_position(), scroll);
            assert_eq!(game.screen.camera(), camera);
            assert_eq!(game.screen.bounds().left, size.0 / 2 - 320);
            for (name, r) in shown_forms(game) {
                assert!(
                    r.left >= 0 && r.top >= 0 && r.right <= size.0 && r.bottom <= size.1,
                    "{name} {r:?} on {size:?}"
                );
            }
            write_ppm(&h, &format!("layout-resized-{w}x{hh}"));
        }
    }

    /// A press on the main screen's status or title bar, with the order of the
    /// windows over the TV's middle and over the money menu's corner
    /// before and after, in the original's stacking and with
    /// `window_stacking`. With `HOLIDAY_PPM` set, the screens go there as
    /// `stacking-original.ppm` and `stacking-enhanced.ppm`.
    #[test]
    fn window_stacking() {
        let Some(dir) = game_dir() else { return };
        for enhanced in [false, true] {
            // `big_map`'s main screen would fill the screen and lie under
            // every form (`big_map`).
            let mut config = if enhanced { Config::default() } else { Config::ORIGINAL };
            config.view.set(enhance::Enhancement::BigMap, false);
            let mut h = holiday_in_a_game(&dir, (800, 600), config);
            let game = h.game.as_mut().unwrap();
            let tv = game.forms.tv.bounds();
            let tv_mid = (i32::midpoint(tv.left, tv.right), i32::midpoint(tv.top, tv.bottom));
            let money = game.forms.money.bounds();
            let money_corner = (money.right.min(game.screen.bounds().right) - 4, money.bottom - 2);
            assert!(inside_rect(game.screen.bounds(), tv_mid) && inside_rect(game.screen.bounds(), money_corner));
            // Shown after the main screen, both lie over it.
            assert_eq!(game.window_at(tv_mid.0, tv_mid.1), Win::Tv);
            assert_eq!(game.window_at(money_corner.0, money_corner.1), Win::Money);
            // A spot on the status bar or the title bar no form covers.
            let (l, t) = (game.screen.left, game.screen.top);
            let at = game
                .screen
                .rects()
                .iter()
                .filter(|(name, _)| ["Statusimage", "Ueberschriftbild"].contains(name))
                .map(|&(_, r)| r)
                .flat_map(|r| (r.left..r.right).step_by(4).map(move |x| (l + x, t + r.top + 4)))
                .find(|&(x, y)| game.window_at(x, y) == Win::Main)
                .unwrap();
            h.input(Input::MouseDown {
                x: at.0,
                y: at.1,
                button: Button::Left,
            });
            h.input(Input::MouseUp {
                x: at.0,
                y: at.1,
                button: Button::Left,
            });
            let game = h.game.as_mut().unwrap();
            // The main screen goes over the money menu either way; over
            // the TV (fsStayOnTop) only with the enhancement.
            assert_eq!(game.window_at(money_corner.0, money_corner.1), Win::Main);
            let over_tv = if enhanced { Win::Main } else { Win::Tv };
            assert_eq!(game.window_at(tv_mid.0, tv_mid.1), over_tv);
            write_ppm(
                &h,
                if enhanced {
                    "stacking-enhanced"
                } else {
                    "stacking-original"
                },
            );
            let game = h.game.as_mut().unwrap();
            if enhanced {
                // The TV's button raises the buried TV, and closes it once
                // it's on top.
                game.click_button(3);
                assert!(game.forms.tv.visible);
                assert_eq!(game.window_at(tv_mid.0, tv_mid.1), Win::Tv);
                game.click_button(3);
                assert!(!game.forms.tv.visible);
            } else {
                // A press on the background raises it over the main screen
                // and the money menu, and the game raises them back.
                let corner = (799, 599);
                assert_eq!(game.window_at(corner.0, corner.1), Win::Background);
                assert!(
                    game.press_window(Win::Background) || game.window_at(money_corner.0, money_corner.1) == Win::Main
                );
                assert_eq!(game.window_at(money_corner.0, money_corner.1), Win::Main);
            }
        }
    }

    /// The CRC-32 of a PNG chunk.
    fn crc32(data: &[u8]) -> u32 {
        let mut crc = !0_u32;
        for &byte in data {
            crc ^= u32::from(byte);
            for _ in 0..8 {
                crc = if crc & 1 == 0 {
                    crc >> 1
                } else {
                    0xedb8_8320 ^ (crc >> 1)
                };
            }
        }
        !crc
    }

    /// A PNG chunk of `kind` with `data`.
    fn png_chunk(png: &mut Vec<u8>, kind: [u8; 4], data: &[u8]) {
        png.extend(u32::try_from(data.len()).unwrap().to_be_bytes());
        let mut body = kind.to_vec();
        body.extend_from_slice(data);
        png.extend_from_slice(&body);
        png.extend(crc32(&body).to_be_bytes());
    }

    /// Writes the framebuffer as `name.png` into `HOLIDAY_PNG`, if set:
    /// an uncompressed PNG (stored deflate blocks), which needs no crate.
    fn write_png(holiday: &Holiday, name: &str) {
        let Some(out) = std::env::var_os("HOLIDAY_PNG") else {
            return;
        };
        let screen = &holiday.screen;
        let mut raw = Vec::with_capacity((screen.width * 3 + 1) * screen.height);
        for row in screen.pixels.chunks(screen.width) {
            raw.push(0);
            for &i in row {
                raw.extend_from_slice(&holiday.rgb[usize::from(i)]);
            }
        }
        let mut zlib = vec![0x78, 0x01];
        let blocks: Vec<&[u8]> = raw.chunks(0xffff).collect();
        for (k, block) in blocks.iter().enumerate() {
            zlib.push(u8::from(k + 1 == blocks.len()));
            let len = u16::try_from(block.len()).unwrap();
            zlib.extend(len.to_le_bytes());
            zlib.extend((!len).to_le_bytes());
            zlib.extend_from_slice(block);
        }
        let (mut sum1, mut sum2) = (1_u32, 0_u32);
        for &byte in &raw {
            sum1 = (sum1 + u32::from(byte)) % 65521;
            sum2 = (sum2 + sum1) % 65521;
        }
        zlib.extend(((sum2 << 16) | sum1).to_be_bytes());
        let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
        let mut ihdr = Vec::new();
        ihdr.extend(u32::try_from(screen.width).unwrap().to_be_bytes());
        ihdr.extend(u32::try_from(screen.height).unwrap().to_be_bytes());
        ihdr.extend([8, 2, 0, 0, 0]);
        png_chunk(&mut png, *b"IHDR", &ihdr);
        png_chunk(&mut png, *b"IDAT", &zlib);
        png_chunk(&mut png, *b"IEND", &[]);
        let dir = PathBuf::from(out);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(format!("{name}.png")), png).unwrap();
    }

    /// The original's config with only `big_map` on.
    fn big_map_only() -> Config {
        let mut c = Config::ORIGINAL;
        c.view.set(enhance::Enhancement::BigMap, true);
        c
    }

    /// The box the squares at the corners of a view `cols` × `rows`
    /// span on the overview map's picture.
    fn overview_box(tiles: &Tiles, cam: &Camera, cols: i16, rows: i16) -> (i32, i32) {
        let at = |(i, j): (i16, i16)| ((i32::from(i) - i32::from(j)) * 2, i32::from(i) + i32::from(j));
        let c =
            [(0, 0), (cols, 0), (0, rows), (cols, rows)].map(|(c, r)| at(darstellung::view_square(tiles, cam, c, r)));
        let w = c.iter().map(|p| p.0).max().unwrap() - c.iter().map(|p| p.0).min().unwrap();
        let h = c.iter().map(|p| p.1).max().unwrap() - c.iter().map(|p| p.1).min().unwrap();
        (w, h)
    }

    /// `big_map` on 1024×768 and 1280×1024, on and off: off, the
    /// original's main screen, centred; on, one filling the screen whose
    /// map view has the original's in its middle, square for square and
    /// almost pixel for pixel in every rotation, with the same scroll
    /// position, camera and viewpoint for the world; the scroll bars'
    /// ends, the overview map's frame and the mouse follow the bigger
    /// view. With `HOLIDAY_PNG` set, each screen goes there as
    /// `bigmap-WxH-on.png` and `-off.png`.
    #[test]
    fn big_map() {
        let Some(dir) = game_dir() else { return };
        for size in [(1024, 768), (1280, 1024)] {
            let mut off_h = holiday_in_a_game(&dir, size, Config::ORIGINAL);
            let mut on_h = holiday_in_a_game(&dir, size, big_map_only());
            write_png(&off_h, &format!("bigmap-{}x{}-off", size.0, size.1));
            write_png(&on_h, &format!("bigmap-{}x{}-on", size.0, size.1));
            let off = off_h.game.as_mut().unwrap();
            let on = on_h.game.as_mut().unwrap();
            let (left, top) = Hauptschirm::position(size.0, size.1);
            assert_eq!(off.screen.bounds(), Rect::new(left, top, 640, 479));
            assert_eq!(off.screen.view_size(), darstellung::ViewSize::ORIGINAL);
            assert_eq!(on.screen.bounds(), Rect::new(0, 0, size.0, size.1));
            let view = on.screen.view_size();
            let bild = on.screen.hauptbild();
            assert_eq!(bild, Rect::new(0, 59, size.0 - 16, size.1 - 75));
            assert!(
                i32::from(view.columns) * 32 >= bild.width() && view.half_rows > 27,
                "{view:?}"
            );
            assert_eq!(on.tools.view, view);
            // The world sees the same either way.
            assert_eq!(on.screen.scroll_position(), off.screen.scroll_position());
            assert_eq!(on.screen.camera(), off.screen.camera());
            assert_eq!(on.screen.viewpoint(), off.screen.viewpoint());
            assert!(on.forms.overview.visible());
            assert!(on.screen.overview_frame(&on.tiles).is_some());
            assert!(off.screen.overview_frame(&off.tiles).is_none());
            let (dc, dr) = view.core_offset();
            let (dx, dy) = (i32::from(dc) * 32, i32::from(dr) * 16);
            for rotation in 0..4 {
                for g in [&mut *off, &mut *on] {
                    g.screen.set_view(rotation, &g.world, &g.tiles);
                }
                // The original's view lies in the middle of the bigger one.
                let small = off.screen.camera();
                let big = on.screen.drawn_camera();
                assert_eq!(on.screen.camera(), small);
                for y in (0..404).step_by(7) {
                    for x in (0..624).step_by(11) {
                        assert_eq!(
                            darstellung::square_at(&off.world, &off.tiles, &small, x, y),
                            darstellung::square_at_in(&on.world, &on.tiles, &big, &view, x + dx, y + dy),
                            "rotation {rotation} at {x}, {y}"
                        );
                    }
                }
                // Drawn alike, pixel for pixel.
                let (a, b) = (&off.screen.map, &on.screen.map);
                let mut differ = 0;
                for y in 0..404 {
                    for x in 0..624 {
                        let p = a.pixels[y * a.width + x];
                        let q =
                            b.pixels[(y + usize::try_from(dy).unwrap()) * b.width + x + usize::try_from(dx).unwrap()];
                        differ += usize::from(p != q);
                    }
                }
                assert_eq!(differ, 0, "rotation {rotation}: pixels differ");
                // The bigger view's frame on the overview map covers what
                // it shows; turned a quarter that's wider than high.
                let frame = on.screen.overview_frame(&on.tiles).unwrap();
                let (cols, rows) = (
                    i16::try_from(bild.width() / 32).unwrap(),
                    i16::try_from(bild.height() / 16).unwrap(),
                );
                let (w, h) = overview_box(&on.tiles, &big, cols, rows);
                assert_eq!((frame.width(), frame.height()), (w, h));
                if rotation % 2 == 1 {
                    assert!(w > 2 * h);
                }
            }
            // The original's overview frame is 40 × 25 in every rotation
            // (`uebersichtsmap`'s quirk), but its view turned a quarter
            // spans 50 × 20 of the map.
            assert_eq!(overview_box(&off.tiles, &off.screen.camera(), 19, 25), (50, 20));
            off.screen.set_view(0, &off.world, &off.tiles);
            assert_eq!(overview_box(&off.tiles, &off.screen.camera(), 19, 25), (40, 25));
            // The scroll bars end where the bigger view reaches the
            // original's ends.
            on.screen.set_view(0, &on.world, &on.tiles);
            on.screen.scroll(-1000, -1000, &on.world, &on.tiles);
            assert_eq!(on.screen.scroll_position(), (dc, dr));
            on.screen.scroll(1000, 1000, &on.world, &on.tiles);
            assert_eq!(on.screen.scroll_position(), (481 - dc, 476 - dr));
            // A right click in the middle of the island stays.
            let before = on.screen.scroll_position();
            on.screen.scroll(-100, -100, &on.world, &on.tiles);
            let before = (before, on.screen.scroll_position());
            on.screen
                .right_click(bild.width() / 2, bild.height() / 2, &on.world, &on.tiles);
            assert_eq!(on.screen.scroll_position(), before.1);
        }
    }

    /// With `big_map` the world ticks the same whatever the window's
    /// size: two games from one world, one on 1280×1024 with the bigger
    /// view, one with the original's, stay equal.
    #[test]
    fn big_map_world_is_the_same() {
        let Some(dir) = game_dir() else { return };
        let exe = read(&dir, "HOLIDAY.EXE");
        let settings = Settings {
            scenario: 1,
            ..Settings::default()
        };
        let sound = Sound::new(&dir, setup::Sound::Hi, platform::audio::Audio::silent()).shared();
        let game = |size, config: &Config| {
            let high = forms::high_scores(&dir, Path::new("/nonexistent/HIGHSCOR.WAV"));
            start_game(
                &dir,
                &exe,
                &settings,
                true,
                Options::default(),
                high,
                &sound,
                size,
                config,
            )
        };
        let mut off = game((800, 600), &Config::ORIGINAL);
        let mut on = game((1280, 1024), &big_map_only());
        assert!(on.screen.view_size().is_big());
        on.world = off.world.clone();
        for g in [&mut off, &mut on] {
            g.world.clock.paused = false;
            g.screen.scroll(3, -4, &g.world, &g.tiles);
        }
        for _ in 0..200 {
            let _ = off.update(Duration::from_millis(u64::from(TICK_MS)));
            let _ = on.update(Duration::from_millis(u64::from(TICK_MS)));
        }
        assert!(on.world == off.world, "the world doesn't see big_map");
    }

    /// Resizing with `big_map`: the main screen fills each new size, and
    /// goes back to the original's at 640×480; the world, the scroll
    /// position and the camera stay, and every form stays on the screen.
    #[test]
    fn big_map_resize() {
        let Some(dir) = game_dir() else { return };
        let mut h = holiday_in_a_game(&dir, (1024, 768), big_map_only());
        let game = h.game.as_ref().unwrap();
        let world = game.world.clone();
        let scroll = game.screen.scroll_position();
        let camera = game.screen.camera();
        for (w, hh) in [(1280, 1024), (640, 480), (1366, 700), (790, 590), (1024, 768)] {
            h.resize(w, hh);
            let game = h.game.as_ref().unwrap();
            assert!(game.world == world, "the world doesn't see the window");
            assert_eq!(game.screen.scroll_position(), scroll);
            assert_eq!(game.screen.camera(), camera);
            if (w, hh) == (640, 480) {
                assert_eq!(game.screen.bounds(), Rect::new(0, 1, 640, 479));
                assert!(!game.screen.view_size().is_big());
            } else {
                assert_eq!(game.screen.bounds(), Rect::new(0, 0, h.desktop.0, h.desktop.1));
                assert_eq!(game.tools.view, game.screen.view_size());
            }
            for (name, r) in shown_forms(game) {
                assert!(
                    r.left >= 0 && r.top >= 0 && r.right <= h.size.0 && r.bottom <= h.size.1,
                    "{name} {r:?} on {:?}",
                    h.size
                );
            }
        }
    }

    fn inside_rect(r: Rect, (x, y): (i32, i32)) -> bool {
        (r.left..r.right).contains(&x) && (r.top..r.bottom).contains(&y)
    }

    /// The main screen's title bar drags it, kept on the screen, rolled up
    /// to its title bar meanwhile; a double click leaves it rolled up.
    #[test]
    fn dragging_the_main_screen() {
        let Some(dir) = game_dir() else { return };
        let mut h = holiday_in_a_game(&dir, (1024, 768), Config::ORIGINAL);
        let game = h.game.as_mut().unwrap();
        let (x, y) = (game.screen.left + 100, game.screen.top + 5);
        let down = Input::MouseDown {
            x,
            y,
            button: Button::Left,
        };
        let _ = game.main_mouse(down, ShiftState::LEFT, false);
        assert_eq!(game.screen.height, hauptschirm::TITLE_HEIGHT);
        let _ = game.main_mouse(Input::MouseMove { x: 5000, y: 5000 }, ShiftState::LEFT, false);
        assert_eq!(
            (game.screen.left, game.screen.top),
            (1024 - 640, 768 - hauptschirm::TITLE_HEIGHT)
        );
        let _ = game.main_mouse(Input::MouseMove { x: 400, y: 30 }, ShiftState::LEFT, false);
        assert_eq!((game.screen.left, game.screen.top), (300, 25));
        let up = Input::MouseUp {
            x: 400,
            y: 30,
            button: Button::Left,
        };
        let _ = game.main_mouse(up, ShiftState::default(), true);
        assert_eq!(game.screen.height, hauptschirm::HEIGHT);
        // A double click rolls it up.
        let down = Input::MouseDown {
            x: 400,
            y: 30,
            button: Button::Left,
        };
        let _ = game.main_mouse(down, ShiftState::LEFT | ShiftState::DOUBLE, false);
        let _ = game.main_mouse(up, ShiftState::default(), true);
        assert!(game.screen.rolled_up && game.screen.height == hauptschirm::TITLE_HEIGHT);
        // The island isn't there while it's rolled up.
        let on_map = Input::MouseDown {
            x: 400,
            y: 200,
            button: Button::Left,
        };
        assert_eq!(game.main_mouse(on_map, ShiftState::LEFT, false), (false, false));
    }
}
