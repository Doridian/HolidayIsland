//! Holiday Island. This plays the intro's film (`sunflow.avi`), shows the
//! main form with the title picture and then the background, like the
//! intro (`docs/startup.md`, states 400–450), and then the start menu on
//! top (state 500). Start (after `titel.avi`, states 1111–1120, and the
//! progress window of states 1200–1500, `TWasNunForm`) loads the scenario
//! or makes a new island and shows the main screen with it, which the arrow keys
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
//! and the high scores. The game's message boxes (`MessageDlg`,
//! `game::msgdlg`) open over everything and take the input until they
//! answer.
//!
//! The window's client area, divided by the UI scale, is the desktop the
//! game lays out for, again whenever it changes (`docs/layout.md`).
//!
//! `holiday [--save-dir=DIR] [--soundfont=FILE.sf2] [--sound=hi|lo|off]
//! [--films=hi|lo] [--no-audio] [--window-size=WxH] [--fullscreen|--windowed]
//! [--ui-scale=N] [--lang=auto|de|en] [--original] [--enhance=KEY=on|off]
//! [--enhancements] [--help] [GAME_DIR]` ([`USAGE`]), where `GAME_DIR` holds `HOLIDAY.EXE`,
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
mod upgrade;

use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use formats::bmp::Bitmap;
use formats::data::{
    animdat1, animdat4, auktion, auswahl, baumenu_dat, baumenu_txt, darstell, fehler, flusspos, fontdat, gfxpos,
    groundnr, grounpos, hausdata, hausname, hausnr, hauspos, message_media_names, modatyp, monate, start_txt, straspos,
    szenario_txt, szenario_txt_lines,
};
use formats::dfm::Object;
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
use game::enhance::{self, Config, Fix, Fixes};
use game::enhanceform;
use game::hauptschirm::{self, Axis, Hauptschirm, InfoTexts, KeyResult};
use game::highscoreform::ScoreTable;
use game::hintergrund::{self, Island};
use game::ladenform::{self, LadenForm};
use game::lang;
use game::msgdlg::{self, Btn, DlgType, MessageDlg, ModalResult};
use game::optionen::Settings as Options;
use game::sabowin;
use game::savegame::{self, Fields, MapView};
use game::simulation::{TICK_MS, Tick};
use game::startform::{self, Action, Settings, StartForm};
use game::text::Font;
use game::tools::Tools;
use game::wasnunform::{self, WasNunForm};
use game::widgets::ShiftState;
use game::world::{Tables, World};
use platform::{App, Button, Cursor, Frame, Input};
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

    /// The list boxes' entries, with the player's view `enhance`ments.
    /// // 1038:3983
    fn games(&self, enhance: enhance::Enhancements) -> Vec<Vec<u8>> {
        ladenform::saved_games(&self.files(), enhance)
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

    /// Erases the player's game `name` (any case); the game's own `MAPS\`
    /// isn't touched. The path erased, none if the player has no such
    /// file. // en 1040:110c
    fn delete(&self, name: &[u8]) -> std::io::Result<Option<PathBuf>> {
        match self.find(name) {
            Some(p) if p.starts_with(&self.dir) => std::fs::remove_file(&p).map(|()| Some(p)),
            _ => Ok(None),
        }
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
    /// The DFM's position, where the form is made.
    at: (i32, i32),
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
    /// The TV's lights' random numbers with `fix.tv_lights`, the view's
    /// own instead of the world's `RandSeed`.
    lights_random: Random,
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
        self.sound.borrow_mut().music_frame(&mut self.forms.settings);
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
            // A scroll bar held scrolls again (the `Tag`'s cases). The
            // original does it after its simulation's steps and the
            // building slice, before the people's; the port before the
            // world's tick, which then sees the new view a step early.
            if self.screen.bar_tick(&self.world, &self.tiles) {
                self.view_moved();
                changed = true;
            }
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
        self.tools.hauptbild = self.screen.hauptbild();
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

    /// The player's view enhancements for this game: the stacking,
    /// `big_map`'s main screen, and each form's.
    /// Also in the middle of a game, from the list of enhancements: the
    /// windows keep their order.
    fn set_view_enhancements(&mut self, view: enhance::Enhancements) {
        self.stack.enhanced = view.has(enhance::Enhancement::WindowStacking);
        self.screen.big_map = view.has(enhance::Enhancement::BigMap);
        self.screen.enhance = view;
        let f = &mut self.forms;
        f.options.enhance = view;
        f.tv.enhance = view;
        f.money.set_enhance(view, self.size.0);
        f.overview.enhance = view;
        f.compass.enhance = view;
        f.infobox.enhance = view;
        f.umenu.enhance = view;
        f.ende.enhance = view;
        f.credits.enhance = view;
        f.enhance.enhance = view;
        self.auction.enhance = view;
        self.tools.enhance = view;
        self.tools.menu.set_enhance(view, self.size.1);
        // `form_positions`: the forms the original leaves where the DFM
        // put them (`1110:54fc` users) go over the centred main screen.
        let positions = view.has(enhance::Enhancement::FormPositions);
        let main = Hauptschirm::position(self.size.0, self.size.1);
        if let Some(w) = &mut self.tools.sabowin {
            w.enhance = view;
            // BUG(original): the DFM's place, not over the centred main screen.
            if positions && (w.left, w.top) == (sabowin::LEFT, sabowin::TOP) {
                (w.left, w.top) = (sabowin::LEFT + main.0, sabowin::TOP + main.1);
            }
        }
        let d = &mut self.file_menu.menu;
        d.enhance = view;
        if positions && (d.left, d.top) == self.file_menu.at {
            // BUG(original): the DFM's place (305, 139), not over the centred main screen.
            (d.left, d.top) = (self.file_menu.at.0 + main.0, self.file_menu.at.1 + main.1);
        }
        self.sound.borrow_mut().music.enhance = view;
        self.place_main();
    }

    /// After the main screen's view moved (`hauptschirm_scroll`,
    /// 1020:07dc, from the keys, a right click, the scroll bars or the
    /// wheel) or turned (`set_view`, 1010:254d): both end in
    /// `people_refresh` (1088:7fb1), which places the people for the new
    /// view and spawns new ones where it looks. That changes the world, so
    /// it's a command carrying the view ([`command::Action::ViewMoved`]),
    /// applied at once as the original does it; then the people over the
    /// island again, and the overview map's frame
    /// (`uebersichtsmap_draw_frame`, as 1020:0cf7 does).
    fn view_moved(&mut self) {
        // Single human: the original's player 1.
        self.commands.push(Command {
            player: 1,
            action: command::Action::ViewMoved(self.screen.viewpoint()),
        });
        self.apply_commands();
        self.screen
            .show_people(&self.world, &self.tiles, &self.font, &self.info);
        self.main_view_changed();
    }

    /// The mouse wheel at `x`, `y` on the screen (`mouse_wheel`, a port
    /// extension), `dx` notches right and `dy` up, with Shift turning
    /// the wheel's up and down into left and right: the top window under
    /// the mouse scrolls, or the modal list of enhancements. Whether the
    /// screen changed.
    fn wheel(&mut self, (x, y): (i32, i32), dx: i32, dy: i32, shift: ShiftState) -> bool {
        let (dx, dy) = if shift.contains(ShiftState::SHIFT) {
            (dx - dy, 0)
        } else {
            (dx, dy)
        };
        if self.forms.modal() {
            if !self.forms.enhance.visible {
                return false;
            }
            let e = &self.forms.enhance;
            let (ex, ey) = (x - e.left, y - e.top);
            let r = self.forms.enhance.wheel(ex, ey, dy);
            let refresh = r.refresh;
            self.enhance_response(r);
            return refresh;
        }
        self.sync_stack();
        match self.window_at(x, y) {
            Win::Main => {
                if self.screen.wheel(dx, dy, &self.world, &self.tiles) {
                    self.view_moved();
                    return true;
                }
                false
            }
            Win::Overview => {
                let r = self.forms.overview.wheel(&self.world, dx, dy);
                let refresh = r.refresh;
                self.overview_response(r);
                refresh
            }
            Win::Umenu => {
                let r = self.forms.umenu.wheel(dy, &self.world, &self.font, &self.info);
                let refresh = r.refresh;
                self.umenu_response(r);
                refresh
            }
            _ => false,
        }
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

    /// `show_error_message` (1030:232d, en 1030:3495): `FEHLER.TXT`'s
    /// text `n` on the status bar, with sound 0xc.
    fn show_error(&mut self, n: u8) {
        if let Some(text) = self.tools.status_text(game::simulation::Status::Error(n)) {
            self.screen.show_status(&text, &self.font);
        }
        let settings = self.forms.settings;
        self.sound.borrow_mut().play(0xc, &settings);
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
            // BUG(original): only `people_tick` counts the sounds' times down, with the animations on.
            let every_tick = s.enhance.has(enhance::Enhancement::SoundTimes);
            let countdown = t.sound_countdown.or(every_tick.then_some(0));
            let split = countdown.unwrap_or(t.sounds.len()).min(t.sounds.len());
            s.play_all(t.sounds[..split].iter().copied(), &settings);
            if countdown.is_some() {
                s.effects.count_down();
            }
            s.play_all(t.sounds[split..].iter().copied(), &settings);
            if t.new_day {
                s.next_day(&mut self.forms.settings);
            }
        }
        // With `fix.tv_lights` the world draws none; the view does.
        let own = self.world.fixes.has(Fix::TvLights) && self.world.news.speaker == game::news::Speaker::Reading;
        let lights = t.tv_lights.or_else(|| {
            own.then(|| {
                let r = &mut self.lights_random;
                let lower = r.random(5);
                let upper = (r.random(20) == 1).then(|| r.random(3) + 1);
                (lower, upper)
            })
        });
        if let Some((lower, upper)) = lights
            && self.forms.tv.visible
        {
            self.forms.tv.draw_lights(lower, upper);
        }
    }

    /// The auction form opens with the item's picture. // 1028:0dde
    fn open_auction(&mut self) {
        let item = usize::try_from(self.world.auction.item).ok();
        let picture = item.and_then(|n| self.pictures.get(n)).and_then(|name| {
            let path = format!("GRAFIK/{}", auktionsform::picture_file(name));
            let bitmap = Bitmap::parse(&read_opt(&self.dir, &path)?).ok()?;
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
                if let Some(n) = self.auction.status_error.take() {
                    self.show_error(n);
                }
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
                    // en 1018:363c: no button there in scenario 10.
                    Some(b) if hauptschirm::button_blocked(self.world.scenario, b) => {}
                    Some(b) => self.click_button(b),
                    None => {}
                }
                (true, false)
            }
            ("Hauptbild", Input::MouseDown { .. }) => {
                if shift == ShiftState::RIGHT {
                    self.screen.right_click(x, y, &self.world, &self.tiles); // 1020:1984
                    self.view_moved();
                } else {
                    let cam = self.screen.drawn_camera();
                    let actions = self.tools.press(x, y, shift, &self.world, &self.tiles, &cam);
                    self.tool_actions(actions);
                    self.take_info_box();
                }
                (true, false)
            }
            ("Hauptbild", Input::MouseUp { button, .. }) => {
                let cam = self.screen.drawn_camera();
                let actions = self
                    .tools
                    .release(button == Button::Left, &self.world, &self.tiles, &cam, (x, y));
                self.tool_actions(actions);
                let shown = self.forms.infobox.visible;
                self.take_info_box();
                (shown != self.forms.infobox.visible, false)
            }
            // The scroll bars. // 1020:0d92, 1020:0fa2, 1020:10b6 (the
            // right one, `y` down it), 1020:151a, 1020:172a, 1020:181f
            // (the lower one, `x` across)
            ("NSScrollbild" | "WOScrollbild", Input::MouseDown { .. } | Input::MouseMove { .. }) => {
                let (axis, along) = if target == "NSScrollbild" {
                    (Axis::Ns, y)
                } else {
                    (Axis::Wo, x)
                };
                let moved = if matches!(input, Input::MouseDown { .. }) {
                    let held = self.screen.bar_mouse_down(axis, along, shift, &self.world, &self.tiles);
                    // An arrow shows pressed, moved or not.
                    if held {
                        self.view_moved();
                    }
                    true
                } else {
                    let moved = self.screen.bar_mouse_move(axis, along, shift, &self.world, &self.tiles);
                    if moved {
                        self.view_moved();
                    }
                    moved
                };
                (moved, false)
            }
            ("NSScrollbild" | "WOScrollbild", Input::MouseUp { .. }) => (self.screen.bar_mouse_up(), false),
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
            let response = self.tools.menu.key_down(key.unwrap_or(0), self.world.scenario);
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
                // `set_view` (1010:254d) turns the build menu's picture
                // too.
                let view = self.screen.camera().view;
                self.tools
                    .menu
                    .view_turned(&self.world, &self.font, &self.tiles, &self.info, view);
                self.view_moved();
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

/// States 1200 to 1500 after `titel.avi`: the progress window
/// (`TWasNunForm`) while the game is set up, one step a frame.
/// // en 1010:0591
struct Loading {
    form: WasNunForm,
    step: LoadStep,
    /// The game to start, until the work is done.
    pending: Option<Pending>,
    /// The game, once it's set up.
    game: Option<Game>,
}

/// Where states 1200 to 1500 are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoadStep {
    /// State 1200: the window shown, empty.
    Shown,
    /// State 1210: the map made or cleared (`generate_island_busy`,
    /// en 1038:35f1); the port's work is done after this frame.
    Map,
    /// State 1220: the game's data loaded (`load_game_data`,
    /// en 1038:3615).
    Data,
    /// State 1500: the map loaded and the main screen set up.
    Setup,
}

/// An open message box (`MessageDlg`), over everything, with the input
/// to itself, and what its answer leads to.
struct MessageBox {
    dlg: MessageDlg,
    then: AfterMessage,
}

/// What a message box's answer does.
#[derive(Debug, Clone, PartialEq, Eq)]
enum AfterMessage {
    /// Nothing: a message with only OK.
    Nothing,
    /// The file menu's "Löschen": OK erases the game. // en 1040:110c
    Delete(Vec<u8>),
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
    /// `settings.ini`, where the list of enhancements saves its switches;
    /// none where there's no config directory.
    settings_file: Option<PathBuf>,
    /// `--original` or `--enhance=` was given.
    command_line: bool,
    /// The game's font, for the message boxes and the progress window.
    font: Font,
    /// The message boxes' bitmaps (`msgdlg::BITMAP_FILES`).
    message_bitmaps: Vec<Surface>,
    /// The message box that's open.
    message: Option<MessageBox>,
    /// States 1200 to 1500: the progress window and the game set up.
    loading: Option<Loading>,
    /// `Screen.Cursor`, and how many more updates it stays when it was
    /// only set around work that took no time.
    cursor: Cursor,
    cursor_updates: u8,
    /// The mouse on the screen.
    mouse: (i32, i32),
    /// `last_player_name`: the start menu's name, kept from game to
    /// game. // 1110:0a02, en 1118:0a02
    last_player_name: Vec<u8>,
    /// The start menu's settings while going back to it waits for the
    /// CD.
    next_menu: Option<Settings>,
    /// `Randomize`: where a game's `RandSeed` comes from.
    seed: fn() -> u32,
}

impl Holiday {
    /// `backgroundimage` shows `picture` for the screen size: the
    /// title's or the background's of [`hintergrund::layout`], or none
    /// for other sizes (states 430, 450 and 1130). The palette, which the
    /// whole game is drawn with, is the picture's; without one (or with
    /// `Picture::None`) it's the 800×600 one's, the port's stand-in for
    /// the palette Windows would have had then. With
    /// `desktop_background` every size has its nearest layout's,
    /// centred. // 1000:2466
    fn set_picture(&mut self, picture: Picture) {
        self.picture = picture;
        let layout = hintergrund::layout(self.size, self.enhance.view);
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
        let (w, h) = self.size;
        let bounds = match layout {
            Some(l) if self.enhance.view.has(enhance::Enhancement::DesktopBackground) => {
                Rect::new((w - l.size.0) / 2, (h - l.size.1) / 2, l.size.0, l.size.1)
            }
            _ => Rect::new(0, 0, w, h),
        };
        if let Some(image) = self.form.component_mut("backgroundimage") {
            image.bounds = bounds;
            if let Kind::Image { picture } = &mut image.kind {
                *picture = shown;
            }
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
        self.size = hintergrund::screen_size(w, h, self.enhance.view);
        self.form.components[0].bounds = Rect::new(0, 0, w, h);
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
        if let Some(loading) = &mut self.loading {
            loading.form.place(self.size);
        }
        if let Some(m) = &mut self.message {
            m.dlg.place(self.size);
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
        if let Some(loading) = &self.loading {
            let f = &loading.form;
            self.screen.draw(f.left, f.top, f.image(), self.screen.bounds());
        }
        if let Some(m) = &self.message {
            let b = m.dlg.bounds();
            self.screen.draw(b.left, b.top, m.dlg.image(), self.screen.bounds());
        }
    }

    /// The mouse pointer: `Screen.Cursor` if it's set, else the
    /// progress window's own (its DFM's `crNoDrop`) over it. The
    /// background form's DFM has `crNoDrop` too, but its picture, with
    /// the default, covers it.
    fn cursor_shape(&self) -> Cursor {
        if self.cursor != Cursor::Default {
            return self.cursor;
        }
        match &self.loading {
            Some(l) if inside_rect(l.form.bounds(), self.mouse) => Cursor::NoDrop,
            _ => Cursor::Default,
        }
    }

    /// The view's language outside a game: the setting on the folder's
    /// `land` (`game::lang`).
    fn language(&self) -> lang::Language {
        match &self.game {
            Some(game) => game.info.language(),
            None => self.enhance.language.language(folder_land(&self.dir)),
        }
    }

    /// Opens a message box (`MessageDlg(text, kind, buttons, 0)`) over
    /// everything; `then` says what its answer does.
    fn open_message(&mut self, text: &[u8], kind: DlgType, buttons: &[Btn], then: AfterMessage) {
        let dlg = MessageDlg::new(
            text,
            kind,
            buttons,
            self.language(),
            &self.font,
            self.message_bitmaps.clone(),
            self.size,
        );
        self.message = Some(MessageBox { dlg, then });
        self.repaint();
    }

    /// Input while a message box is open: the box is modal and gets it
    /// all. // en 10d8:07ee (`ShowModal`)
    fn message_input(&mut self, input: Input) {
        let Some(m) = &mut self.message else { return };
        let screen = self.size;
        let mut r = msgdlg::Response::default();
        match input {
            Input::MouseMove { x, y } => r = m.dlg.mouse_move(x, y, screen),
            Input::MouseDown {
                x,
                y,
                button: Button::Left,
            } => r = m.dlg.mouse_down(x, y),
            Input::MouseUp {
                x,
                y,
                button: Button::Left,
            } => r = m.dlg.mouse_up(x, y),
            Input::KeyDown(key) => {
                if key == VK_MENU {
                    self.keys = self.keys | ShiftState::ALT;
                }
                r.answer = m.dlg.key_down(key, self.keys.contains(ShiftState::ALT));
            }
            Input::KeyUp(key) => {
                let bit = match key {
                    VK_SHIFT => ShiftState::SHIFT,
                    VK_CONTROL => ShiftState::CTRL,
                    VK_MENU => ShiftState::ALT,
                    _ => ShiftState::default(),
                };
                self.keys = ShiftState(self.keys.0 & !bit.0);
            }
            _ => {}
        }
        if let Some(n) = r.sound {
            let settings = self.game.as_ref().map_or(self.options, |g| g.forms.settings);
            self.sound.borrow_mut().play(n.number(), &settings);
        }
        if let Some(answer) = r.answer {
            self.message_answered(answer);
        } else if r.repaint {
            self.repaint();
        }
    }

    /// The message box answered `r`: it closes, and what it was for
    /// goes on.
    fn message_answered(&mut self, r: ModalResult) {
        let Some(m) = self.message.take() else { return };
        match m.then {
            AfterMessage::Nothing => {}
            AfterMessage::Delete(name) => {
                // en 1040:1190: only OK erases.
                if r == ModalResult::Ok {
                    self.delete_game(&name);
                }
            }
        }
        self.repaint();
    }

    /// `Screen.Cursor := crHourGlass` around work on the files (en
    /// 1040:0f11, en 1040:1069, en 1040:1198) and back: the work takes no
    /// time here, so the hourglass shows for the next frame.
    fn hourglass(&mut self) {
        self.cursor = Cursor::HourGlass;
        self.cursor_updates = 2;
    }

    /// The file menu's "Löschen" after OK: the game's file is erased and
    /// the list filled again, under the hourglass. The port only erases
    /// the player's own files, never the game's `MAPS\`. // en 1040:1198
    fn delete_game(&mut self, name: &[u8]) {
        self.hourglass();
        match self.saves.delete(name) {
            Ok(Some(path)) => println!("deleted {}", path.display()),
            Ok(None) => eprintln!("{}: not one of the player's games", String::from_utf8_lossy(name)),
            Err(e) => eprintln!("can't delete {}: {e}", String::from_utf8_lossy(name)),
        }
        let games = self.saves.games(self.enhance.view);
        if let Some(game) = &mut self.game {
            game.file_menu.menu.set_games(games);
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
                self.start.menu.mouse_up(x, y, button == Some(Button::Left))
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

    /// The mouse wheel (`mouse_wheel`, a port extension): the load
    /// form's list over the start menu, the file menu's while it's open,
    /// else the game's window under the mouse (`Game::wheel`).
    fn wheel(&mut self, at: (i32, i32), dx: i32, dy: i32) {
        if !self.enhance.view.has(enhance::Enhancement::MouseWheel) {
            return;
        }
        let shift = self.shift();
        let changed = if let Some(load) = &mut self.load {
            load.menu.wheel(dy).refresh
        } else if self.start.visible() {
            false
        } else if let Some(game) = &mut self.game {
            if game.file_menu.visible {
                let m = &game.file_menu.menu;
                let (x, y) = (at.0 - m.left, at.1 - m.top);
                let over = (0..dateimenue::WIDTH).contains(&x) && (0..dateimenue::HEIGHT).contains(&y);
                let r = if over {
                    game.file_menu.menu.wheel(dy)
                } else {
                    dateimenue::Response::default()
                };
                let refresh = r.refresh;
                self.file_menu_response(r);
                refresh
            } else {
                let changed = game.wheel(at, dx, dy, shift);
                self.process_requests();
                changed
            }
        } else {
            false
        };
        if changed {
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
                    let games = self.saves.games(self.enhance.view);
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
                    // en 1068:1e08: the second file with `UPGRADE\`.
                    let path = self.saves.high_scores();
                    let table = &game.forms.high.table;
                    let mut files = vec![(path.clone(), table.first_file())];
                    if upgrade::present(&self.dir) {
                        files.push((forms::second_high_scores(&path), table.second_file()));
                    }
                    for (path, scores) in files {
                        if let Err(e) = std::fs::write(&path, scores.to_bytes()) {
                            eprintln!("can't save {}: {e}", path.display());
                        }
                    }
                }
                Request::Enhance(change) => self.enhance_changed(change),
            }
        }
    }

    /// What the list of enhancements says about saving.
    fn enhance_notes(&self) -> enhanceform::Notes {
        enhanceform::Notes {
            saved: self.settings_file.is_some(),
            command_line: self.command_line,
        }
    }

    /// A switch from the list of enhancements (a port extension): the
    /// player's settings change, a fix as the default for new games (the
    /// form sent the running game's command), a view enhancement applied
    /// at once; and `settings.ini` keeps it, the other lines as they are.
    /// "Alle an" and "Alle aus" set the master switch and drop the keys'
    /// lines. With `--original` or `--enhance=` this run has them too;
    /// the next run's command line goes over the file again.
    fn enhance_changed(&mut self, change: enhanceform::Change) {
        use enhanceform::Change;
        let mut config = self.enhance;
        let (original, set, remove): (Option<bool>, Vec<(&str, bool)>, Vec<&str>) = match change {
            Change::Fix(f, on) => {
                config.fixes.set(f, on);
                (None, vec![(f.info().key, on)], Vec::new())
            }
            Change::View(e, on) => {
                config.view.set(e, on);
                (None, vec![(e.info().key, on)], Vec::new())
            }
            Change::All(on) => {
                config = if on { Config::default() } else { Config::ORIGINAL };
                (Some(!on), Vec::new(), enhance::all().map(|i| i.key).collect())
            }
        };
        if let Some(path) = &self.settings_file
            && let Err(e) = platform::settings::EnhanceSettings::save(path, original, &set, &remove)
        {
            eprintln!("can't save {}: {e}", path.display());
        }
        let view_changed = config.view != self.enhance.view;
        self.enhance = config;
        if view_changed {
            self.apply_view_enhancements();
        }
    }

    /// The player's view enhancements changed in the middle of the
    /// program: the start menu, the load form, the sound and the game get
    /// them, and everything is laid out again (`big_map`,
    /// `desktop_background`).
    fn apply_view_enhancements(&mut self) {
        let view = self.enhance.view;
        self.start.menu.enhance = view;
        if let Some(load) = &mut self.load {
            load.menu.enhance = view;
        }
        self.sound.borrow_mut().set_view_enhancements(view);
        if let Some(game) = &mut self.game {
            game.set_view_enhancements(view);
        }
        let (w, h) = self.desktop;
        self.layout(w, h);
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
                // to the world, under the hourglass (en 1040:1069).
                Ok(map) => {
                    self.cursor = Cursor::HourGlass;
                    self.cursor_updates = 2;
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
                // 1040:0e03, under the hourglass (en 1040:0f11).
                self.cursor = Cursor::HourGlass;
                self.cursor_updates = 2;
                match self.saves.write(&name, &game.map()) {
                    Ok(path) => println!("saved {}", path.display()),
                    Err(e) => eprintln!("can't save {}: {e}", String::from_utf8_lossy(&name)),
                }
                let games = self.saves.games(self.enhance.view);
                if let Some(game) = &mut self.game {
                    game.file_menu.menu.set_games(games);
                }
            }
            // `MessageDlg(…, mtError, [mbOK], 0)` with the message in the
            // view's language. // 1040:0e03, en 1040:0eef
            Some(dateimenue::Action::InvalidName) => {
                let text = dateimenue::invalid_name_text(game.info.land);
                self.open_message(text, DlgType::Error, &[Btn::Ok], AfterMessage::Nothing);
            }
            // `MessageDlg(…, mtConfirmation, [mbOK, mbCancel], 0)`: OK
            // erases the game ([`Holiday::delete_game`]). // en 1040:110c
            Some(dateimenue::Action::Delete(name)) => {
                let text = dateimenue::delete_question(game.info.land);
                self.open_message(
                    text,
                    DlgType::Confirmation,
                    &[Btn::Ok, Btn::Cancel],
                    AfterMessage::Delete(name),
                );
            }
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

    /// "Neues Spiel" (`back_to_start_menu`, en 1010:3582, de 1010:25dc):
    /// saves `Last.map`, hides the game's forms and frees the game
    /// (en 1038:3657, de 1030:3974: the clock, the forms, the people, the
    /// computers, the news, the music closed, ...) and shows a new start
    /// menu ([`Holiday::new_start_menu`]). The later build first waits
    /// for the CD (en 1010:35e4: until the film directory has
    /// `surfwm.avi`, `INSERT_CD` with OK to look again and Cancel to
    /// quit); the port doesn't check for a CD.
    fn back_to_start_menu(&mut self) {
        let Some(game) = self.game.take() else { return };
        if let Err(e) = self.saves.write(hintergrund::LAST_MAP.as_bytes(), &game.map()) {
            eprintln!("can't save {}: {e}", hintergrund::LAST_MAP);
        }
        {
            let mut s = self.sound.borrow_mut();
            s.speaker.close();
            s.stop_film();
            s.close_music();
        }
        // What `init_game_defaults` leaves for the start menu
        // (en 1048:2a05): the opponents and the name.
        self.next_menu = Some(Settings::after_game(&game.world, self.last_player_name.clone()));
        // And the options: `init_game_defaults` and `init_sound`.
        let sound_on = self.sound.borrow().effects.enabled;
        self.options = game.forms.settings.after_game(sound_on);
        drop(game);
        self.repaint();
        self.new_start_menu();
    }

    /// The rest of `back_to_start_menu`: the
    /// globals back to a new game's (en 1048:2a05: `init_game_defaults`,
    /// en 1000:22b4, which sets the start menu's island, difficulty,
    /// scenario and colours and the options back; `clear_game_flags`;
    /// the spectators' flag), the data reloaded (en 1038:3586, de
    /// 1030:38a3: `Randomize`, the game's tables and texts, `init_sound`,
    /// which empties the sounds and sets the volumes to the system's),
    /// `holiday.mid` from its start and a new `TStartform` set from the
    /// globals (`startform_init`, en 1058:378f). So a second game starts
    /// as the first did; the port's world and tables are made anew for
    /// each game anyway, and `RandSeed` is drawn when it starts.
    fn new_start_menu(&mut self) {
        let settings = self.next_menu.take().unwrap_or_default();
        {
            let mut s = self.sound.borrow_mut();
            s.reset_effects(); // `init_sound`, en 1040:3c33
            s.intro_music();
        }
        self.start = start_menu(&self.dir, &self.exe, settings);
        self.start.menu.enhance = self.enhance.view;
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
    /// the wave device back (10a0:254f), and state 1200 shows the
    /// progress window (`TWasNunForm`, en 1028:3b3c); the game is set up
    /// over the next frames ([`Holiday::advance_loading`]).
    fn start_pending(&mut self) {
        {
            let mut s = self.sound.borrow_mut();
            s.stop_film();
            s.effects_on();
        }
        self.overlay = None;
        let Some(pending) = self.pending.take() else { return };
        let form = WasNunForm::show(
            bitmaps(&self.dir, &wasnunform::BITMAP_FILES),
            wasnun_texts(&self.dir),
            &self.font,
            self.size,
        );
        self.loading = Some(Loading {
            form,
            step: LoadStep::Shown,
            pending: Some(pending),
            game: None,
        });
        self.repaint();
    }

    /// One step of states 1210 to 1500 a frame, each shown on the
    /// progress window as the original shows it when its work is done
    /// (`docs/startup.md`). The port's work takes a frame or so, all of
    /// it after the step of state 1210 is drawn: its page stays on the
    /// screen while the game is set up. Then the window goes and the main
    /// screen shows. // en 1010:0591
    fn advance_loading(&mut self) {
        let Some(mut loading) = self.loading.take() else { return };
        match loading.step {
            LoadStep::Shown => {
                // State 1210: `Screen.Cursor := crNoDrop` around
                // `generate_island` (en 1038:35f1), which shows page 1 and
                // 5 % before it makes an island, page 3 and 100 % once it
                // has cleared the map for a scenario or a saved game
                // (en 10b8:52d4).
                self.cursor = Cursor::NoDrop;
                let new_island = match &loading.pending {
                    Some(Pending::Start { scenario }) => {
                        hintergrund::new_island(self.start.menu.settings(), *scenario) == Island::Generated
                    }
                    _ => false,
                };
                if new_island {
                    loading.form.set_page(wasnunform::page::NEW_MAP, &self.font);
                    loading.form.set_percent(5, &self.font);
                } else {
                    loading.form.set_page(wasnunform::page::INIT_MAP, &self.font);
                    loading.form.set_percent(100, &self.font);
                }
                loading.step = LoadStep::Map;
            }
            LoadStep::Map => {
                // The port's work, then state 1220's page 2 at 100 %
                // (`load_game_data`, en 1038:3615, with `crNoDrop` in
                // `load_game_rest`, en 1038:33c8).
                loading.game = loading.pending.take().map(|p| self.set_up_game(p));
                loading.form.set_page(wasnunform::page::INIT_GAME, &self.font);
                loading.form.set_percent(100, &self.font);
                loading.step = LoadStep::Data;
            }
            LoadStep::Data => {
                // State 1500: page 0 at 100 % around `load_map`; the cursor
                // is back (en 1038:357f).
                self.cursor = Cursor::Default;
                loading.form.set_page(wasnunform::page::LOAD_MAP, &self.font);
                loading.form.set_percent(100, &self.font);
                loading.step = LoadStep::Setup;
            }
            LoadStep::Setup => {
                // The window hides and goes, the main screen shows
                // (`wasnun_hide`, en 1028:3baa; `hauptschirm_show`).
                self.game = loading.game.take();
                // `big_map`'s main screen fills the desktop, which can be
                // a little bigger or smaller than the screen size.
                let (size, desktop) = (self.size, self.desktop);
                let notes = self.enhance_notes();
                if let Some(game) = &mut self.game {
                    game.resize(size, desktop);
                    game.forms.enhance_notes = notes;
                }
                self.repaint();
                return;
            }
        }
        self.loading = Some(loading);
        self.repaint();
    }

    /// Runs states 1210 to 1500 to their end at once.
    #[cfg(test)]
    fn finish_loading(&mut self) {
        while self.loading.is_some() {
            self.advance_loading();
        }
    }

    /// The game `pending` sets up: a new island or a scenario from the
    /// start menu's settings, or a saved game. // en 1010:0591
    fn set_up_game(&mut self, pending: Pending) -> Game {
        let high = forms::high_scores(&self.dir, &self.saves.high_scores());
        let seed = (self.seed)();
        match pending {
            Pending::Start { scenario } => start_game_seeded(
                &self.dir,
                &self.exe,
                self.start.menu.settings(),
                scenario,
                self.options,
                high,
                &self.sound,
                self.size,
                &self.enhance,
                seed,
            ),
            Pending::Load(map) => {
                let settings = self.start.menu.settings().clone();
                let options = self.options.with_map(&map);
                load_game(
                    &self.dir,
                    &self.exe,
                    &map,
                    &settings,
                    options,
                    high,
                    &self.sound,
                    self.size,
                    &self.enhance,
                    seed,
                )
            }
        }
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
                // `startform_start_click` keeps the name as
                // `last_player_name`, hides the form, and hintergrund's
                // states 1000-1500 start the game. // 1058:021a
                self.last_player_name.clone_from(&self.start.menu.settings().name);
                self.start.hide();
                self.play_start_film(Pending::Start { scenario });
            }
            Some(Action::Load) => {
                // 1058:0c7d: the start menu is disabled and TLadenForm
                // opens over it.
                let mut load = load_menu(&self.dir, &self.exe, self.saves.games(self.enhance.view), self.size);
                load.menu.enhance = self.enhance.view;
                self.load = Some(load);
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
        if self.cursor_updates > 0 {
            self.cursor_updates -= 1;
            if self.cursor_updates == 0 {
                self.cursor = Cursor::Default;
            }
        }
        if self.loading.is_some() {
            self.advance_loading();
        }
        // `scroll_bars`: the start menu's scroll arrows held repeat.
        if self.start.visible() && self.load.is_none() && self.start.menu.scroll_held() {
            let response = self.start.menu.repeat(ms);
            if response.refresh || response.action.is_some() {
                self.respond(response);
            }
        }
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
        if let Input::MouseMove { x, y } | Input::MouseDown { x, y, .. } | Input::MouseUp { x, y, .. } = input {
            self.mouse = (x, y);
        }
        if !matches!(input, Input::Close) && self.message.is_some() {
            self.message_input(input);
            return;
        }
        // States 1200 to 1500: the background form is disabled, and the
        // progress window takes no input.
        if !matches!(input, Input::Close) && self.loading.is_some() {
            return;
        }
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
            Input::Wheel { x, y, dx, dy } => self.wheel((x, y), dx, dy),
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
            cursor: self.cursor_shape(),
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

/// The game's file `name` (`DATA/HAUSDATA.DAT`), whatever the case of
/// its letters: the German CD's names are upper case, the English
/// install's mixed (`TEXTE/Hausname.txt`).
fn read(dir: &Path, name: &str) -> Vec<u8> {
    read_opt(dir, name).unwrap_or_else(|| panic!("can't read {}", dir.join(name).display()))
}

/// Whether `x`, `y` is in `r`.
fn inside_rect(r: Rect, (x, y): (i32, i32)) -> bool {
    (r.left..r.right).contains(&x) && (r.top..r.bottom).contains(&y)
}

/// [`read`], `None` for a file that isn't there.
fn read_opt(dir: &Path, name: &str) -> Option<Vec<u8>> {
    std::fs::read(dir.join(name))
        .ok()
        .or_else(|| std::fs::read(forms::find(dir, name)?).ok())
}

/// The folder's `land` (`MODATYP.DAT`, `load_modatyp`, en 1000:23ed),
/// which `auto` follows (`game::lang`); `D` if the file can't be read.
fn folder_land(dir: &Path) -> formats::data::Land {
    read_opt(dir, "DATA/MODATYP.DAT")
        .and_then(|d| modatyp(&d).ok())
        .map_or(formats::data::Land::Deutschland, |m| m.land)
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

/// The progress window's four lines of `TEXTE\WASNUN.TXT`, read line by
/// line (`wasnun_load_texts`, en 1028:374a).
fn wasnun_texts(dir: &Path) -> Vec<Vec<u8>> {
    let data = read_opt(dir, &format!("TEXTE/{}", wasnunform::TEXT_FILE)).unwrap_or_default();
    data.split(|&b| b == b'\n')
        .map(|l| l.strip_suffix(b"\r").unwrap_or(l).to_vec())
        .take(4)
        .collect()
}

/// The start menu as `TStartform.Create` and `startform_init` make it,
/// from the globals in `settings`. // 1058:27e6
///
/// With the later build's `UPGRADE\` folder its `SZENARIO.TXT` and
/// bitmaps make the scenario page the later build's, with twelve
/// scenarios. // en 1058:378f
fn start_menu(dir: &Path, exe: &[u8], settings: Settings) -> StartMenu {
    let mut images = bitmaps(dir, &startform::BITMAP_FILES);
    let szenario = if upgrade::present(dir) {
        images.extend(upgrade::bitmaps(dir, &startform::UPGRADE_BITMAP_FILES));
        szenario_txt_lines(
            &upgrade::file(dir, "SZENARIO.TXT", "TEXTE/SZENARIO.TXT"),
            formats::data::SZENARIO_LINES_UPGRADE,
        )
    } else {
        szenario_txt(&read(dir, "TEXTE/SZENARIO.TXT"))
    };
    let menu = StartForm::new(
        font(dir),
        images,
        &start_txt(&read(dir, "TEXTE/START.TXT")),
        &szenario,
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
    let texts = upgrade::sonstige(dir);
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
    let texts = upgrade::sonstige(dir);
    // BUG(original): the DFM's place on the screen (305, 139), which isn't over the centred main screen on a desktop bigger than 640×480.
    let mut form = load_form(exe, "TDATEIMENUE");
    let at = form.components[0].bounds;
    if let Some(image) = form.component_mut("Dateimenuebild") {
        image.bounds = Rect::new(0, 0, dateimenue::WIDTH, dateimenue::HEIGHT);
        image.visible = true;
    }
    let mut images = bitmaps(dir, &dateimenue::BITMAP_FILES);
    let upgrade = upgrade::present(dir);
    if upgrade {
        // en 1040:159e: `UPGRADE\datwin1d.BMP`, with "Löschen".
        if let Some(background) = upgrade::bitmaps(dir, &["datwin1d.BMP"]).pop().filter(|b| b.width > 0) {
            images[0] = background;
        }
    }
    let mut menu = Dateimenue::new(font(dir), images, texts.texts, at.left, at.top);
    if upgrade {
        menu = menu.with_upgrade();
    }
    FileMenu {
        menu,
        form,
        visible: false,
        captured: false,
        at: (at.left, at.top),
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
        hausprei: hintergrund::hausprei_table(|name| read_opt(dir, name)).unwrap_or_else(|e| panic!("{e}")),
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
/// scenario's map (and its view origin) or a generated island. Either
/// plays with `fixes`, which a generated island is made with: some change
/// the generator. // 1000:2466
#[cfg(test)]
fn start_world(dir: &Path, settings: &Settings, scenario_page: bool, options: &mut Options, fixes: Fixes) -> Start {
    start_world_seeded(dir, settings, scenario_page, options, fixes, seed())
}

/// [`start_world`] with `RandSeed` as `Randomize` left it (`seed`).
fn start_world_seeded(
    dir: &Path,
    settings: &Settings,
    scenario_page: bool,
    options: &mut Options,
    fixes: Fixes,
    seed: u32,
) -> Start {
    match hintergrund::new_island(settings, scenario_page) {
        Island::Scenario(path) => {
            let map = SaveGame::parse(&read(dir, &path)).unwrap_or_else(|e| panic!("{path}: {e}"));
            let mut world =
                hintergrund::load_world(&map, settings, scenario_page).unwrap_or_else(|e| panic!("{path}: {e}"));
            // A scenario's file has no fixes: the player's.
            world.set_loaded_fixes(fixes);
            world.random = Random::new(seed);
            world.tables = world_tables(dir);
            world.news.date_format = game::news::DateFormat {
                months: monate(&read(dir, "TEXTE/MONATE.TXT")).expect("MONATE.TXT"),
                land: modatyp(&read(dir, "DATA/MODATYP.DAT")).expect("MODATYP.DAT").land as u8,
            };
            // The rest of state 1500: the scenario's events, its build
            // menu and the computers.
            if scenario_page {
                let events = hintergrund::scenario_events(world.scenario, |name| read_opt(dir, name))
                    .unwrap_or_else(|e| panic!("{e}"));
                hintergrund::start_scenario(&mut world, events.as_deref());
            }
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
            let data = hintergrund::GameData::load(|name| read_opt(dir, name)).unwrap_or_else(|e| panic!("{e}"));
            // States 100, 1210 and 1220.
            let world = hintergrund::new_world(settings, &data, seed, fixes);
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
    high: ScoreTable,
    sound: &sound::Shared,
    size: (i32, i32),
    config: &Config,
    seed: u32,
) -> Game {
    let data = hintergrund::GameData::load(|name| read_opt(dir, name)).unwrap_or_else(|e| panic!("{e}"));
    // The file's fixes; the player's for those it doesn't name (all of
    // them in an original file).
    let world = hintergrund::load_saved_game(map, settings, &data, seed, config.fixes)
        .unwrap_or_else(|e| panic!("the saved game: {e}"));
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
        config.language,
    );
    game.set_view_enhancements(config.view);
    game
}

/// What hintergrund's timer does from state 1200 to 1500 once Start is
/// clicked and `titel.avi` is over, without the progress window
/// (`TWasNunForm`, [`Holiday::advance_loading`]): load the scenario's map
/// or generate an island, set up the main screen and show it.
/// // 1000:2466, en 1010:0591
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
fn start_game(
    dir: &Path,
    exe: &[u8],
    settings: &Settings,
    scenario_page: bool,
    options: Options,
    high: ScoreTable,
    sound: &sound::Shared,
    size: (i32, i32),
    config: &Config,
) -> Game {
    start_game_seeded(
        dir,
        exe,
        settings,
        scenario_page,
        options,
        high,
        sound,
        size,
        config,
        seed(),
    )
}

/// [`start_game`] with `RandSeed` as `Randomize` left it (`seed`).
#[allow(clippy::too_many_arguments)]
fn start_game_seeded(
    dir: &Path,
    exe: &[u8],
    settings: &Settings,
    scenario_page: bool,
    mut options: Options,
    high: ScoreTable,
    sound: &sound::Shared,
    size: (i32, i32),
    config: &Config,
    seed: u32,
) -> Game {
    // A new game, and a scenario (whose file has no fixes), plays with
    // the player's, in place before the world is set up.
    let start = start_world_seeded(dir, settings, scenario_page, &mut options, config.fixes, seed);
    let mut game = make_game(dir, exe, start, options, high, sound, size, config.language);
    if scenario_page {
        game.tools.menu.start_scenario(game.world.scenario); // en 1018:015f
    }
    game.set_view_enhancements(config.view);
    game
}

/// The main screen and the forms of a game that starts from `start`,
/// with the dates and numbers and the port's texts in the player's
/// `language` (`game::lang`). // 1020:2501, 1010:2820
#[allow(clippy::too_many_lines, clippy::too_many_arguments)]
fn make_game(
    dir: &Path,
    exe: &[u8],
    (world, camera, origin, view_rest): Start,
    mut options: Options,
    high: ScoreTable,
    sound: &sound::Shared,
    size: (i32, i32),
    language: lang::Setting,
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
    let texts = upgrade::sonstige(dir);
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
        // The view's `land`: the folder's, or the forced language's.
        land: language.land(folder_land(dir)),
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
    // 10b8:50de, from `load_map` only: a generated island's overview map
    // keeps what its constructor (10b8:4538) set, no button down and the
    // land by heights, where the missing fields' zeros would put button 0
    // down and draw a picture over the menu's left scroll arrow.
    if origin.is_some() {
        forms.overview.load(&view_rest);
    }
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
        lights_random: Random::new(seed()),
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

/// `--help`. The command line is in English only.
const USAGE: &str = "\
usage: holiday [OPTIONS] [GAME_DIR]

GAME_DIR holds HOLIDAY.EXE, GRAFIK, DATA and TEXTE (default: orig).

  --lang=auto|de|en      the language of the dates, numbers and the port's
                         texts (default auto: the game folder's; settings.ini
                         language=); the folder's own texts stay as they are
  --original             every enhancement off, as the original
  --enhance=KEY=on|off   one enhancement on or off
  --enhancements         list the enhancements and exit
  --save-dir=DIR         where saved games go
  --soundfont=FILE.sf2   the SoundFont for the music
  --sound=hi|lo|off      the sound effects' quality
  --films=hi|lo          the films' quality
  --no-audio             no sound at all
  --window-size=WxH      the window's size
  --fullscreen, --windowed
  --ui-scale=N           1 to 4
  --help                 this
";

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
    /// `--original` or `--enhance=` was given.
    command_line: bool,
}

/// The enhancements, one per line: key, whose, on or off in `config`, and
/// what it does (`--enhancements`), in English like the rest of the
/// command line.
fn enhancement_list(config: &Config) -> String {
    use std::fmt::Write;
    enhance::all().fold(String::new(), |mut out, i| {
        let side = match i.side {
            enhance::Side::World => "world",
            enhance::Side::View => "view",
        };
        let on = if config.is_on(i.key) == Some(true) { "on" } else { "off" };
        let _ = writeln!(
            out,
            "{:<24} {side:<5} {on:<3}  {}",
            i.key,
            i.description(lang::Language::En)
        );
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

/// `settings.ini`'s `language=`; `auto` without one.
fn language_setting(settings_file: Option<&Path>) -> lang::Setting {
    let Some(value) = settings_file.and_then(|f| platform::settings::load_value(f, "language")) else {
        return lang::Setting::Auto;
    };
    lang::Setting::parse(&value).unwrap_or_else(|| {
        eprintln!("settings.ini: language={value} isn't auto, de or en");
        lang::Setting::Auto
    })
}

#[allow(clippy::too_many_lines)] // One option after the other.
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
    let mut language = None;
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
        } else if let Some(l) = a.strip_prefix("--lang=") {
            language = Some(lang::Setting::parse(l).unwrap_or_else(|| fail("--lang= takes auto, de or en")));
        } else if a == "--help" || a == "-h" {
            print!("{USAGE}");
            std::process::exit(0);
        } else if a == "--original" {
            original = true;
        } else if a == "--enhancements" {
            list = true;
        } else if let Some(e) = a.strip_prefix("--enhance=") {
            let (key, on) = e.split_once('=').unwrap_or((e, "on"));
            let on = platform::settings::switch(on).unwrap_or_else(|| fail("--enhance= takes KEY=on or KEY=off"));
            if enhance::RETIRED.contains(&key) {
                // The later build fixed its bug itself: always on now.
                eprintln!("--enhance=: {key} is no longer a switch (the game's later build fixes it)");
            } else if Config::default().is_on(key).is_none() {
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
    let installed = read_opt(&dir, "SETUP.DAT").map(|d| setup::Setup::parse(&d));
    let quality = quality.unwrap_or_else(|| installed.as_ref().map_or(setup::Sound::Hi, |s| s.sound));
    let films = films.unwrap_or_else(|| installed.as_ref().map_or(setup::Film::Hi, |s| s.film));
    let mut enhance = enhance_config(window.settings_file.as_deref(), original, &overrides);
    enhance.language = language.unwrap_or_else(|| language_setting(window.settings_file.as_deref()));
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
        command_line: original || !overrides.is_empty(),
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
        command_line,
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
    holiday.settings_file.clone_from(&window.settings_file);
    holiday.command_line = command_line;
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
        let font = font(&dir);
        let message_bitmaps = bitmaps(&dir, &msgdlg::BITMAP_FILES);
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
            settings_file: None,
            command_line: false,
            font,
            message_bitmaps,
            message: None,
            loading: None,
            cursor: Cursor::Default,
            cursor_updates: 0,
            mouse: (0, 0),
            last_player_name: Settings::default().name,
            next_menu: None,
            seed,
        };
        holiday.start.menu.enhance = enhance.view;
        holiday.sound.borrow_mut().set_view_enhancements(enhance.view);
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

    /// The English release's folder (`orig-en/`), the later build's with
    /// `UPGRADE\`: its mixed-case names are found, the start menu has the
    /// twelve scenarios, scenario 10 (B&B only) starts from `UPGRADE\`
    /// without the hotels' button, and the file menu has "Löschen". With
    /// `HOLIDAY_PPM` set, the screen goes there as `versions-bbonly.ppm`.
    /// Skipped without `orig-en/`.
    #[test]
    fn the_english_folder() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../orig-en");
        if !upgrade::present(&dir) {
            eprintln!("skipping: no {}", dir.join("UPGRADE").display());
            return;
        }
        let exe = read(&dir, "HOLIDAY.EXE");
        // `TEXTE\Hausname.txt` in mixed case.
        assert!(read_opt(&dir, "TEXTE/HAUSNAME.TXT").is_some());
        let start = start_menu(&dir, &exe, Settings::default());
        assert!(!start.menu.scenario_mode());
        let settings = Settings {
            scenario: 10,
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
            &Config::default(),
        );
        assert_eq!((game.world.scenario, game.world.player_count), (10, 8));
        game.world.clock.paused = false;
        let _ = game.update(Duration::from_millis(u64::from(TICK_MS) * 5));
        // F2, the hotels, does nothing in scenario 10.
        assert!(matches!(
            game.screen
                .key_down(0x71, ShiftState::default(), &game.world, &game.tiles),
            KeyResult::Ignored
        ));
        let file = file_menu(&dir, &exe);
        assert_eq!(file.menu.name(), b"");
        let background = bitmap(&dir, "GRAFIK/BCK80X60.BMP");
        let mut screen = Surface::new(800, 600, 0);
        game.paint(&mut screen, &background.palette, None);
        if let Some(out) = std::env::var_os("HOLIDAY_PPM") {
            let mut ppm = format!("P6\n{} {}\n255\n", screen.width, screen.height).into_bytes();
            for &i in &screen.pixels {
                let c = background.palette[usize::from(i)];
                ppm.extend_from_slice(&[c.r, c.g, c.b]);
            }
            std::fs::write(PathBuf::from(out).join("versions-bbonly.ppm"), ppm).unwrap();
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

    /// A scenario's oil slick (10a0:074d): its message opens the TV, and
    /// once the view is at the slick's square the oil lies on the sea,
    /// placed again when the view turns (1088:7fb1). With `HOLIDAY_PNG`
    /// set, the screens go there as `step3-oil-slick-message.png` and
    /// `step3-oil-slick.png`. Skipped without the game's files in `orig/`.
    #[test]
    fn oil_slick_on_screen() {
        const OIL_SEED: u32 = 7;
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../orig");
        if !dir.join("GRAFIK").exists() {
            eprintln!("skipping: {} not found", dir.display());
            return;
        }
        let exe = read(&dir, "HOLIDAY.EXE");
        let settings = Settings {
            scenario: 2,
            difficulty: 1,
            ..Settings::default()
        };
        let high = forms::high_scores(&dir, Path::new("/nonexistent/HIGHSCOR.WAV"));
        let sound = Sound::new(&dir, setup::Sound::Hi, platform::audio::Audio::silent()).shared();
        // A fixed `RandSeed`: where the oil drifts by the time the view
        // gets there is the random numbers'.
        let mut game = start_game_seeded(
            &dir,
            &exe,
            &settings,
            true,
            Options::default(),
            high,
            &sound,
            SIZE,
            &Config::ORIGINAL,
            OIL_SEED,
        );
        game.forms.settings.speech = false;
        // Only the scenario's oil slick, tomorrow.
        let days = game.world.days;
        for e in &mut game.world.events {
            if e.kind == 2 && e.start == 5 {
                (e.chance, e.first_day, e.last_day) = (0x10000, days + 1, days + 1);
            } else {
                e.chance = 0;
            }
        }
        for _ in 0..100_000 {
            if game.world.oil_slick {
                break;
            }
            game.world.clock.paused = false;
            let t = game.world.tick();
            let _ = game.show(&t);
        }
        assert!(game.world.oil_slick);
        assert!(game.forms.tv.visible, "the message opens the TV");
        let background = bitmap(&dir, "GRAFIK/BCK80X60.BMP");
        let rgb = background.palette.map(|c| [c.r, c.g, c.b]);
        let mut screen = Surface::new(800, 600, 0);
        game.paint(&mut screen, &background.palette, None);
        write_surface_png(&screen, &rgb, "step3-oil-slick-message");

        let r = game.forms.tv.close();
        let _ = game.tv_response(r);
        let slick = game.world.damage.slick;
        game.screen
            .jump_to((slick.0 - 4, slick.1 + 6), &game.world, &game.tiles);
        game.view_moved();
        assert_eq!(game.world.viewpoint, game.screen.viewpoint());
        let oil: Vec<_> = game
            .world
            .people
            .records
            .iter()
            .filter(|p| game::events::OIL.contains(&p.kind()))
            .map(game::people::Person::pos)
            .collect();
        assert!(
            oil.iter()
                .any(|&p| game::people::sees(&game.world.viewpoint, &game.world.island, p))
        );
        let _ = game.update(Duration::from_millis(u64::from(TICK_MS) * 3));
        let mut screen = Surface::new(800, 600, 0);
        game.paint(&mut screen, &background.palette, None);
        write_surface_png(&screen, &rgb, "step3-oil-slick");
        // A turn places everyone again for the new rotation.
        game.set_view(1);
        assert_eq!(game.world.viewpoint.rotation, 1);
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
        write_surface_png(&holiday.screen, &holiday.rgb, name);
    }

    /// [`write_png`] for `screen` with the colours `rgb`.
    fn write_surface_png(screen: &Surface, rgb: &[[u8; 3]; 256], name: &str) {
        let Some(out) = std::env::var_os("HOLIDAY_PNG") else {
            return;
        };
        let mut raw = Vec::with_capacity((screen.width * 3 + 1) * screen.height);
        for row in screen.pixels.chunks(screen.width) {
            raw.push(0);
            for &i in row {
                raw.extend_from_slice(&rgb[usize::from(i)]);
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

    /// The original's config with only the view enhancement `e` on.
    fn only(e: enhance::Enhancement) -> Config {
        let mut c = Config::ORIGINAL;
        c.view.set(e, true);
        c
    }

    /// The view enhancements of "UI quirks" and "Sound"
    /// (`docs/original-bugs.md`) in the whole program on 800×600, each
    /// against the original (every view enhancement but `big_map` on).
    #[test]
    #[allow(clippy::too_many_lines)] // One game each way.
    fn ui_enhancements() {
        use enhance::Enhancement as E;
        let Some(dir) = game_dir() else { return };
        let mut all = Config::ORIGINAL;
        for &e in E::ALL {
            all.view.set(e, e != E::BigMap);
        }
        let mut o_h = holiday_in_a_game(&dir, SIZE, Config::ORIGINAL);
        let mut f_h = holiday_in_a_game(&dir, SIZE, all);
        let o = o_h.game.as_mut().unwrap();
        let f = f_h.game.as_mut().unwrap();
        let main = Hauptschirm::position(SIZE.0, SIZE.1);

        // `form_positions`: the money menu a column further left, whole on
        // the screen, and two pixels higher; the build menu a row lower,
        // level with the island; the sabotage window and the file menu
        // over the main screen.
        let (om, fm) = (o.forms.money.bounds(), f.forms.money.bounds());
        assert_eq!((om.right, fm.right), (SIZE.0 + 1, SIZE.0));
        assert_eq!(fm.height(), om.height() + 2);
        assert_eq!(f.tools.menu.top, o.tools.menu.top + 1);
        assert_eq!(f.tools.menu.top, main.1 + 0x3b);
        let (os, fs) = (o.tools.sabowin.as_ref().unwrap(), f.tools.sabowin.as_ref().unwrap());
        assert_eq!((fs.left, fs.top), (os.left + main.0, os.top + main.1));
        assert_eq!(
            (f.file_menu.menu.left, f.file_menu.menu.top),
            (o.file_menu.menu.left + main.0, o.file_menu.menu.top + main.1)
        );

        // `mouse_buttons`: the right button on the money menu's title bar
        // rolls it up for a drag only in the original.
        for (g, rolls) in [(&mut *o, true), (&mut *f, false)] {
            let title = g.forms.money.layout()[0].1;
            let at = (title.left + title.width() / 2, title.top + 4);
            let height = g.forms.money.bounds().height();
            let _ = g.forms.money.mouse_down(at.0, at.1, false, ShiftState::RIGHT);
            assert_eq!(g.forms.money.bounds().height() != height, rolls);
            let _ = g.forms.money.mouse_up(at.0, at.1, false);
        }

        // `money_menu_button`: the year's end shows the money menu, with
        // its button down only with the fix.
        for (g, down) in [(&mut *o, false), (&mut *f, true)] {
            let _ = g.forms.money.close();
            g.tools.down[4] = false;
            assert!(!g.forms.money.shown());
            g.forms.settings.year_balance = true;
            let _ = g.forms_tick(&Tick {
                new_year: true,
                ..Tick::default()
            });
            assert!(g.forms.money.shown());
            assert_eq!(g.tools.down[4], down);
        }

        // `form_drags`: a left drag that didn't start on the main screen's
        // title bar, or on the compass's after it was let go, moves them
        // only in the original.
        for (g, moves) in [(&mut *o, true), (&mut *f, false)] {
            let at = (g.screen.left + 300, g.screen.top + 5);
            assert_eq!(g.screen.title_mouse_move(ShiftState::LEFT, at, SIZE), moves);
            let c = g.forms.compass.bounds();
            let grab = (30, 5);
            let cursor = (c.left + grab.0, c.top + grab.1);
            let _ = g.forms.compass.mouse_down(grab.0, grab.1, ShiftState::LEFT, cursor);
            let _ = g.forms.compass.mouse_up(grab.0, grab.1);
            let r = g
                .forms
                .compass
                .mouse_move(ShiftState::LEFT, (cursor.0 - 20, cursor.1), SIZE);
            assert_eq!(r.moved, moves);
        }

        // `overview_scrolling`: the overview map's click past the main
        // screen's scroll bars' ends.
        for (g, at) in [(&mut *o, (-10, -13)), (&mut *f, (0, 0))] {
            g.screen.set_view(0, &g.world, &g.tiles);
            g.screen.set_scroll(-10, -13, &g.world, &g.tiles);
            assert_eq!(g.screen.scroll_position(), at);
        }

        // `build_menu`: picking a tool with the build menu closed makes
        // its closing sound only in the original; the arrow keys go on to
        // the main screen only with the fix.
        for (g, sound) in [(&mut *o, true), (&mut *f, false)] {
            let view = g.screen.camera().view;
            let _ = g
                .tools
                .click(game::tools::DEMOLISH, &g.world, &g.font, &g.tiles, &g.info, view, SIZE);
            g.tools.menu.sounds.clear();
            let _ = g
                .tools
                .click(game::tools::RAISE, &g.world, &g.font, &g.tiles, &g.info, view, SIZE);
            assert_eq!(g.tools.menu.sounds.contains(&0xe), sound);
            let key = g.tools.menu.key_down(0x26, g.world.scenario);
            assert_eq!(key == baumenue::Response::MainScreenKey(0x26), !sound);
        }

        // `sound_times`: with the animations off, a sound with a time
        // plays again after its 50 ticks only with the fix.
        for (g, again) in [(&mut *o, false), (&mut *f, true)] {
            let sound = Rc::clone(&g.sound);
            let _ = sound.borrow_mut().effects.play(1, true);
            for _ in 0..50 {
                g.tick_sounds(&Tick::default());
            }
            assert_eq!(sound.borrow_mut().effects.play(1, true).is_some(), again);
        }

        // `modifier_clicks`: the demolish tool with Shift held.
        for (g, works) in [(&mut *o, false), (&mut *f, true)] {
            let cam = g.screen.drawn_camera();
            let actions = g
                .tools
                .press(300, 200, ShiftState::LEFT | ShiftState::SHIFT, &g.world, &g.tiles, &cam);
            assert_eq!(!actions.is_empty(), works);
        }
    }

    /// `desktop_background` on 1366×700, which has no picture in the
    /// original. With `HOLIDAY_PNG` set, the screens go there as
    /// `ui-desktop_background-original.png` and `-fixed.png`.
    #[test]
    fn desktop_background() {
        let Some(dir) = game_dir() else { return };
        let picture = |h: &Holiday| {
            let c = h.form.components.iter().find(|c| c.name == b"backgroundimage").unwrap();
            let shown = matches!(&c.kind, Kind::Image { picture: Some(_) });
            (shown, c.bounds)
        };
        let size = (1366, 700);
        let o = holiday_in_a_game(&dir, size, Config::ORIGINAL);
        let f = holiday_in_a_game(&dir, size, only(enhance::Enhancement::DesktopBackground));
        write_png(&o, "ui-desktop_background-original");
        write_png(&f, "ui-desktop_background-fixed");
        // 800×600's background, the biggest that fits, centred.
        assert!(!picture(&o).0);
        let at = Rect::new((1366 - 800) / 2, (700 - 600) / 2, 800, 600);
        assert_eq!(picture(&f), (true, at));
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

    /// A generated island's overview map is the form as its constructor
    /// left it (10b8:4538: no button down, the land alone, by heights):
    /// there's no map to read its fields from (10b8:50de runs only from
    /// `load_map`). Reading a map's missing fields as zeros put button 0
    /// down, whose picture went over the menu's top left, the horizontal
    /// scroll bar's left arrow. With `HOLIDAY_PPM` set, the overview goes
    /// there as `overview-new-game-<size>-<config>.ppm`.
    #[test]
    fn new_games_overview_has_no_button_down() {
        let Some(dir) = game_dir() else { return };
        let exe = read(&dir, "HOLIDAY.EXE");
        let menu = Surface::from_bitmap(&bitmap(&dir, "GRAFIK/UEBIGMEN.BMP"));
        let palette = bitmap(&dir, "GRAFIK/BCK80X60.BMP").palette;
        let sound = Sound::new(&dir, setup::Sound::Hi, platform::audio::Audio::silent()).shared();
        for (size, config, name) in [
            ((640, 480), Config::ORIGINAL, "original"),
            ((640, 480), Config::default(), "enhanced"),
            ((1280, 1024), Config::default(), "enhanced"),
        ] {
            let high = forms::high_scores(&dir, Path::new("/nonexistent/HIGHSCOR.WAV"));
            let mut game = start_game(
                &dir,
                &exe,
                &Settings::default(),
                false,
                Options::default(),
                high,
                &sound,
                size,
                &config,
            );
            game.click_button(5);
            if let Some(out) = std::env::var_os("HOLIDAY_PPM") {
                let mut screen = Surface::new(usize::try_from(size.0).unwrap(), usize::try_from(size.1).unwrap(), 0);
                game.paint(&mut screen, &palette, None);
                let b = game.forms.overview.bounds();
                let mut ppm = format!("P6\n{} {}\n255\n", b.width(), b.height()).into_bytes();
                for y in b.top..b.bottom {
                    for x in b.left..b.right {
                        let c = palette
                            [usize::from(screen.pixel(usize::try_from(x).unwrap(), usize::try_from(y).unwrap()))];
                        ppm.extend_from_slice(&[c.r, c.g, c.b]);
                    }
                }
                let file = format!("overview-new-game-{}x{}-{name}.ppm", size.0, size.1);
                std::fs::write(PathBuf::from(out).join(file), ppm).unwrap();
            }
            // The left arrow and the menu's wood below it, as the bitmap
            // has them.
            let o = &game.forms.overview;
            assert!(o.visible() && !o.small());
            assert_eq!((o.mode(), o.heights()), (game::uebersichtsmap::mode::TERRAIN, true));
            for (x, y) in (0..32)
                .flat_map(|y| (0..16).map(move |x| (x, y)))
                .chain((16..32).flat_map(|x| (16..20).map(move |y| (x, y))))
            {
                assert_eq!(o.menu.pixel(x, y), menu.pixel(x, y), "{x}, {y} at {size:?} ({name})");
            }
        }
    }

    /// A new game's world is set up with the player's fixes in place (the
    /// generator's among them), a scenario's gets them too.
    #[test]
    fn games_start_with_the_fixes() {
        let Some(dir) = game_dir() else { return };
        for fixes in [Fixes::ORIGINAL, Fixes::all_on()] {
            for (scenario, page) in [(0, false), (1, true)] {
                let settings = Settings {
                    scenario,
                    ..Settings::default()
                };
                let (world, ..) = start_world(&dir, &settings, page, &mut Options::default(), fixes);
                assert_eq!(world.fixes, fixes);
            }
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

    /// `big_map` and `no_main_title`, as the defaults have them.
    fn no_title_only() -> Config {
        let mut c = big_map_only();
        c.view.set(enhance::Enhancement::NoMainTitle, true);
        c
    }

    /// A press of the left button at `at` on the main screen, with
    /// `shift`, and whether the screen changed.
    fn main_press(game: &mut Game, at: (i32, i32), shift: ShiftState) -> bool {
        let down = Input::MouseDown {
            x: at.0,
            y: at.1,
            button: Button::Left,
        };
        game.main_mouse(down, shift, false).0
    }

    fn main_release(game: &mut Game, at: (i32, i32)) -> bool {
        let up = Input::MouseUp {
            x: at.0,
            y: at.1,
            button: Button::Left,
        };
        game.main_mouse(up, ShiftState::default(), true).0
    }

    fn main_move(game: &mut Game, at: (i32, i32)) -> bool {
        game.main_mouse(Input::MouseMove { x: at.0, y: at.1 }, ShiftState::LEFT, false)
            .0
    }

    /// One tick of the clock.
    fn tick(game: &mut Game, n: u32) {
        for _ in 0..n {
            let _ = game.update(Duration::from_millis(u64::from(TICK_MS)));
        }
    }

    /// `no_main_title`: the main screen that fills the window has no
    /// title bar; the info bar is at the top, the button bar under it,
    /// and the island 16 pixels taller. The world sees the same view, the
    /// forms at the top edge open under the button bar (43 down), the
    /// tooltips and the info box are where they belong, the top row
    /// neither drags nor rolls the form up. At 640×480, and without the
    /// enhancement, it keeps its title bar. With `HOLIDAY_PNG` set, the
    /// screens go there as `notitle-WxH.png` (the forms closed) and
    /// `notitle-WxH-forms.png`.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn no_main_title() {
        let Some(dir) = game_dir() else { return };
        for size in [(800, 600), (1024, 768), (1280, 1024), (1366, 700)] {
            let mut titled_h = holiday_in_a_game(&dir, size, big_map_only());
            let mut h = holiday_in_a_game(&dir, size, no_title_only());
            write_png(&h, &format!("notitle-{}x{}-forms", size.0, size.1));
            let titled = titled_h.game.as_mut().unwrap();
            let game = h.game.as_mut().unwrap();
            assert!(titled.screen.titled() && !game.screen.titled());
            assert_eq!(game.screen.bounds(), Rect::new(0, 0, size.0, size.1));
            let rects = *game.screen.rects();
            assert_eq!(rects[0].1.width() * rects[0].1.height(), 0, "no title bar");
            assert_eq!(rects[1].1, Rect::new(0, 0, size.0, 18));
            assert_eq!(rects[2].1, Rect::new(0, 18, size.0, 25));
            let bild = game.screen.hauptbild();
            assert_eq!(bild, Rect::new(0, 43, size.0 - 16, size.1 - 59));
            assert_eq!((game.tools.view, game.tools.hauptbild), (game.screen.view_size(), bild));
            assert_eq!(game.screen.view_size().height, titled.screen.view_size().height + 16);
            // The world sees the same view, and the original's lies in
            // the bigger one's middle, square for square.
            assert_eq!(game.screen.viewpoint(), titled.screen.viewpoint());
            assert_eq!(game.screen.camera(), titled.screen.camera());
            let core = |g: &Game| {
                let (dc, dr) = g.screen.view_size().core_offset();
                (i32::from(dc) * 32, i32::from(dr) * 16)
            };
            let ((ax, ay), (bx, by)) = (core(game), core(titled));
            for (x, y) in [(0, 0), (300, 200), (623, 403), (17, 391)] {
                assert_eq!(
                    darstellung::square_at_in(
                        &game.world,
                        &game.tiles,
                        &game.screen.drawn_camera(),
                        &game.screen.view_size(),
                        x + ax,
                        y + ay
                    ),
                    darstellung::square_at_in(
                        &titled.world,
                        &titled.tiles,
                        &titled.screen.drawn_camera(),
                        &titled.screen.view_size(),
                        x + bx,
                        y + by
                    ),
                );
            }
            // The forms opened at the top edge went under the bars.
            for (g, bars) in [(&*game, 43), (&*titled, 59)] {
                assert_eq!(g.forms.compass.top, bars);
                assert_eq!(g.forms.overview.top, bars);
                assert!(g.forms.money.top >= bars);
            }
            // The button bar's tooltip goes under it.
            for (g, bars) in [(&mut *game, 43), (&mut *titled, 59)] {
                g.forms.settings.button_help = true;
                g.set_focus(Focus::Main);
                let at = (g.screen.left + 6 * 25 + 30, g.screen.top + bars - 10);
                let _ = g.main_mouse(Input::MouseMove { x: at.0, y: at.1 }, ShiftState::default(), false);
                g.mouse = at;
                tick(g, 1);
                assert!(g.forms.help_visible(), "{size:?} {bars}");
                assert_eq!(g.forms.help.top, bars);
            }
            // The pointer's info box lies next to the mouse alike.
            let mut boxes = Vec::new();
            for g in [&mut *game, &mut *titled] {
                let r = g.forms.tv.close();
                g.tv_response(r);
                g.click_button(game::tools::POINTER);
                let b = g.screen.hauptbild();
                let at = (b.left + 100, b.top + 100);
                g.mouse = at;
                let _ = main_press(g, at, ShiftState::LEFT);
                assert!(g.forms.infobox.visible);
                boxes.push((g.forms.infobox.left - at.0, g.forms.infobox.top - at.1));
                // With one click, it goes when the button comes up.
                let _ = main_release(g, at);
            }
            assert_eq!(boxes[0], boxes[1]);
            // The top row is the info bar: no drag, no rolling up.
            let top = (game.screen.left + 300, game.screen.top + 5);
            let _ = main_press(game, top, ShiftState::LEFT | ShiftState::DOUBLE);
            let _ = main_move(game, (top.0 + 50, top.1 + 50));
            let _ = main_release(game, top);
            assert!(!game.screen.rolled_up);
            assert_eq!(game.screen.bounds(), Rect::new(0, 0, size.0, size.1));
            // A clean screen for the picture.
            game.click_button(4);
            game.click_button(5);
            let _ = game.forms.help.hide();
            let _ = game.update(Duration::ZERO);
            h.repaint();
            write_png(&h, &format!("notitle-{}x{}", size.0, size.1));
            // At 640×480 the main screen is the original's, with its
            // title bar; back on a bigger window it goes again.
            h.resize(640, 480);
            let game = h.game.as_mut().unwrap();
            assert!(game.screen.titled());
            assert_eq!(game.screen.hauptbild(), hauptschirm::LAYOUT[3].1);
            assert_eq!(game.tools.hauptbild, hauptschirm::LAYOUT[3].1);
            h.resize(u32::try_from(size.0).unwrap(), u32::try_from(size.1).unwrap());
            let game = h.game.as_mut().unwrap();
            assert!(!game.screen.titled());
            // Switched off in the game, the title bar comes back.
            game.set_view_enhancements(big_map_only().view);
            assert!(game.screen.titled());
            assert_eq!(game.screen.hauptbild(), Rect::new(0, 59, size.0 - 16, size.1 - 75));
        }
    }

    /// The main screen's scroll bars with the mouse: the arrows step one
    /// (16 pixels down, a column across), the trough beside the thumb
    /// pages 16 half rows down and 18 columns across, both again in each
    /// tick while held, and the thumb drags; in the original's main
    /// screen (800×600, every enhancement off) and in the title-less
    /// `big_map` one (1024×768, every enhancement on), whose stretched
    /// bars map the same way.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn main_scroll_bars() {
        let Some(dir) = game_dir() else { return };
        for (size, config) in [((800, 600), Config::ORIGINAL), ((1024, 768), Config::default())] {
            let fixed = config.view.has(enhance::Enhancement::ScrollBars);
            let mut h = holiday_in_a_game(&dir, size, config);
            let game = h.game.as_mut().unwrap();
            let main = game.screen.bounds();
            let ns = game.screen.rects()[4].1.offset(main.left, main.top);
            let wo = game.screen.rects()[6].1.offset(main.left, main.top);
            assert_eq!(ns.top, main.top + if fixed { 43 } else { 59 });
            game.screen.set_view(0, &game.world, &game.tiles);
            let middle = |g: &mut Game| g.screen.set_scroll(240, 238, &g.world, &g.tiles);
            middle(game);
            let pos = |g: &Game| g.screen.scroll_position();
            let start = pos(game);
            // The down arrow: a step, drawn pressed until let go.
            let down_arrow = (ns.left + 8, ns.bottom - 8);
            let up_bar = game.screen.ns_image.clone();
            assert!(main_press(game, down_arrow, ShiftState::LEFT));
            assert_eq!(pos(game), (start.0, start.1 + 1));
            assert_ne!(game.screen.ns_image.pixels, up_bar.pixels, "pressed");
            let _ = main_release(game, down_arrow);
            assert_eq!(pos(game), (start.0, start.1 + 1));
            let _ = main_press(game, (ns.left + 8, ns.top + 8), ShiftState::LEFT);
            let _ = main_release(game, (ns.left + 8, ns.top + 8));
            assert_eq!(pos(game), start);
            assert_eq!(game.screen.ns_image.pixels, up_bar.pixels, "up again");
            // The right arrow: a column.
            let right_arrow = (wo.right - 8, wo.top + 8);
            let _ = main_press(game, right_arrow, ShiftState::LEFT);
            let _ = main_release(game, right_arrow);
            assert_eq!(pos(game), (start.0 + 1, start.1));
            middle(game);
            // Held: the original steps in every tick; the fix waits two.
            let _ = main_press(game, down_arrow, ShiftState::LEFT);
            tick(game, 5);
            assert_eq!(pos(game).1 - start.1, if fixed { 4 } else { 6 });
            // Off the arrow the original goes on, the fix waits.
            let _ = main_move(game, (ns.left + 8, ns.bottom - 100));
            let before = pos(game).1;
            tick(game, 2);
            assert_eq!(pos(game).1 - before, if fixed { 0 } else { 2 });
            let _ = main_release(game, down_arrow);
            tick(game, 2);
            assert_eq!(pos(game).1 - before, if fixed { 0 } else { 2 }, "stopped");
            middle(game);
            // The trough below the thumb pages 16, across 18.
            let below = (ns.left + 8, ns.bottom - 40);
            let _ = main_press(game, below, ShiftState::LEFT);
            let _ = main_release(game, below);
            assert_eq!(pos(game), (start.0, start.1 + 16));
            let right = (wo.right - 40, wo.top + 8);
            let _ = main_press(game, right, ShiftState::LEFT);
            let _ = main_release(game, right);
            assert_eq!(pos(game), (start.0 + 18, start.1 + 16));
            middle(game);
            // Held, the original pages on to the end; the fix stops where
            // the thumb reaches the mouse.
            let bar_len = ns.height();
            let at = (ns.left + 8, ns.top + bar_len * 3 / 4);
            let _ = main_press(game, at, ShiftState::LEFT);
            tick(game, 40);
            let end = pos(game).1;
            let _ = main_release(game, at);
            let max = 476 - game.screen.view_size().core_offset().1;
            if fixed {
                assert!(end < max, "{end}");
                // The thumb lies under the mouse.
                let t = 16 + (i32::from(end) - (476 - i32::from(max))) * (bar_len - 51) / (2 * i32::from(max) - 476);
                assert!(
                    (t..t + 20).contains(&(at.1 - ns.top)),
                    "thumb {t}, mouse {}",
                    at.1 - ns.top
                );
            } else {
                assert_eq!(end, max);
            }
            middle(game);
            // The thumb dragged: from its middle, 100 pixels down at once.
            let range = |g: &Game| {
                let (_, dr) = g.screen.view_size().core_offset();
                i32::from(476 - 2 * dr)
            };
            let travel = bar_len - 51;
            let thumb_at = |g: &Game| {
                16 + (i32::from(pos(g).1) - i32::from(g.screen.view_size().core_offset().1)) * travel / range(g)
            };
            let grab = (ns.left + 8, ns.top + thumb_at(game) + 8);
            let before = pos(game).1;
            let _ = main_press(game, grab, ShiftState::LEFT);
            assert_eq!(pos(game).1, before, "a press on the thumb doesn't scroll");
            let _ = main_move(game, (grab.0, grab.1 + 100));
            let moved = i32::from(pos(game).1 - before);
            if fixed {
                assert!((thumb_at(game) - (grab.1 - ns.top - 8 + 100)).abs() <= 1);
            } else {
                assert_eq!(moved, (100 * range(game) + (bar_len - 47) / 2) / (bar_len - 48));
            }
            // Back and forth a pixel at a time: the fix comes back where it
            // was; the original's rounding creeps.
            let here = pos(game).1;
            for k in 1..=30 {
                let _ = main_move(game, (grab.0, grab.1 + 100 + k));
            }
            for k in (0..30).rev() {
                let _ = main_move(game, (grab.0, grab.1 + 100 + k));
            }
            if fixed {
                assert_eq!(pos(game).1, here);
            } else {
                assert_ne!(pos(game).1, here, "BUG(original): the drag drifts");
            }
            let _ = main_release(game, grab);
            middle(game);
            // Shift held or a double click's second press: the original
            // does nothing (`modifier_clicks` does).
            for shift in [
                ShiftState::LEFT | ShiftState::SHIFT,
                ShiftState::LEFT | ShiftState::DOUBLE,
            ] {
                let before = pos(game);
                let _ = main_press(game, down_arrow, shift);
                let _ = main_release(game, down_arrow);
                assert_eq!(pos(game).1 - before.1, i16::from(fixed));
            }
            // The right button does nothing.
            let before = pos(game);
            let _ = game.main_mouse(
                Input::MouseDown {
                    x: down_arrow.0,
                    y: down_arrow.1,
                    button: Button::Right,
                },
                ShiftState::RIGHT,
                false,
            );
            let _ = main_release(game, down_arrow);
            assert_eq!(pos(game), before);
            // Turned half round, the right bar runs the other way.
            game.screen.set_view(2, &game.world, &game.tiles);
            let before = pos(game);
            let _ = main_press(game, down_arrow, ShiftState::LEFT);
            let _ = main_release(game, down_arrow);
            assert_eq!(pos(game), (before.0, before.1 - 1));
        }
    }

    /// The mouse wheel (`mouse_wheel`): over the island it scrolls two
    /// steps a notch (Shift: across), over the overview map that map, over
    /// the list of enhancements the list under it; in the original, no
    /// wheel.
    #[test]
    fn mouse_wheel() {
        let Some(dir) = game_dir() else { return };
        for config in [Config::default(), Config::ORIGINAL] {
            let on = config.view.has(enhance::Enhancement::MouseWheel);
            let mut h = holiday_in_a_game(&dir, (1024, 768), config);
            let game = h.game.as_mut().unwrap();
            game.screen.set_view(0, &game.world, &game.tiles);
            game.screen.set_scroll(240, 238, &game.world, &game.tiles);
            let main = game.screen.bounds();
            let bild = game.screen.hauptbild().offset(main.left, main.top);
            let at = (bild.left + 20, bild.bottom - 20);
            game.sync_stack();
            assert_eq!(game.window_at(at.0, at.1), Win::Main);
            let wheel = |h: &mut Holiday, (x, y): (i32, i32), dx, dy| h.input(Input::Wheel { x, y, dx, dy });
            let pos = |h: &Holiday| h.game.as_ref().unwrap().screen.scroll_position();
            wheel(&mut h, at, 0, 1);
            assert_eq!(pos(&h), if on { (240, 236) } else { (240, 238) });
            wheel(&mut h, at, 0, -3);
            assert_eq!(pos(&h), if on { (240, 242) } else { (240, 238) });
            h.input(Input::KeyDown(VK_SHIFT));
            wheel(&mut h, at, 0, 1);
            h.input(Input::KeyUp(VK_SHIFT));
            assert_eq!(pos(&h), if on { (238, 242) } else { (240, 238) });
            wheel(&mut h, at, 2, 0);
            assert_eq!(pos(&h), if on { (242, 242) } else { (240, 238) });
            // Over the overview map: its own window moves, not the island.
            let o = &h.game.as_ref().unwrap().forms.overview;
            let map = o
                .images()
                .iter()
                .find(|(i, _, _)| *i == game::uebersichtsmap::Image::Map)
                .map(|&(_, r, _)| r)
                .unwrap();
            let over = (o.left + map.left + 20, o.top + map.top + 20);
            let (origin, before) = (o.origin(), pos(&h));
            assert_eq!(h.game.as_ref().unwrap().window_at(over.0, over.1), Win::Overview);
            wheel(&mut h, over, 0, -1);
            let o = &h.game.as_ref().unwrap().forms.overview;
            assert_eq!(o.origin() != origin, on);
            assert_eq!(pos(&h), before);
            if !on {
                continue;
            }
            // The list of enhancements: the list under the mouse.
            press_info(&mut h);
            let e = &h.game.as_ref().unwrap().forms.enhance;
            assert!(e.visible);
            let (rx, ry) = e.row_of("window_stacking").unwrap();
            let row = (e.left + rx + 40, e.top + ry);
            wheel(&mut h, row, 0, -1);
            let e = &h.game.as_ref().unwrap().forms.enhance;
            assert_eq!((e.scrolled(0), e.scrolled(1)), (0, 3));
            // The island under it doesn't move while it's open.
            let before = pos(&h);
            wheel(&mut h, at, 0, 1);
            assert_eq!(pos(&h), before);
        }
    }

    /// The other forms' scroll bars take the mouse, and with
    /// `scroll_bars` their arrows held repeat: the options' music volume,
    /// the list of enhancements, the buildings' overview and the overview
    /// map (whose arrows repeat in the original too, every 25 ms).
    #[test]
    #[allow(clippy::many_single_char_names)]
    fn form_scroll_bars() {
        let Some(dir) = game_dir() else { return };
        for config in [Config::ORIGINAL, Config::default()] {
            let fixed = config.view.has(enhance::Enhancement::ScrollBars);
            let mut h = holiday_in_a_game(&dir, (1024, 768), config);
            let press = |h: &mut Holiday, (x, y): (i32, i32)| {
                h.input(Input::MouseDown {
                    x,
                    y,
                    button: Button::Left,
                });
            };
            let release = |h: &mut Holiday, (x, y): (i32, i32)| {
                h.input(Input::MouseUp {
                    x,
                    y,
                    button: Button::Left,
                });
            };
            let hold = |h: &mut Holiday, ms: u64| h.update(Duration::from_millis(ms));
            // The overview map's down arrow: 5 at once, 5 every 25 ms.
            let game = h.game.as_mut().unwrap();
            // The TV lies over the overview's bar; out of the way.
            game.forms.tv.left = 600;
            let o = &game.forms.overview;
            let bar = o
                .images()
                .iter()
                .find(|(i, _, _)| *i == game::uebersichtsmap::Image::Scroll)
                .map(|&(_, r, _)| r)
                .unwrap();
            let arrow = (o.left + bar.left + 8, o.top + bar.bottom - 8);
            let before = o.origin().1;
            press(&mut h, arrow);
            hold(&mut h, 50);
            release(&mut h, arrow);
            assert_eq!(h.game.as_ref().unwrap().forms.overview.origin().1 - before, 15);
            // The options' music volume, from 100: the back arrow.
            h.game.as_mut().unwrap().click_button(2);
            // Where it opens (under the bars, with `big_map`).
            h.game.as_mut().unwrap().sync_stack();
            let o = &h.game.as_ref().unwrap().forms.options;
            assert!(o.visible);
            let arrow = (o.left + 0xc + 4, o.top + 0xbe + 4);
            press(&mut h, arrow);
            hold(&mut h, 600);
            let volume = h.game.as_ref().unwrap().forms.options.settings().music_volume;
            release(&mut h, arrow);
            let after = h.game.as_ref().unwrap().forms.options.settings().music_volume;
            if fixed {
                // A step at once, then every 50 ms after 400.
                assert_eq!((volume, after), (100 - 1 - 5, 94));
            } else {
                // BUG(original): a step when let go, never more.
                assert_eq!((volume, after), (100, 99));
            }
            // The list of enhancements (`enhancements_screen`).
            if config.view.has(enhance::Enhancement::EnhancementsScreen) {
                h.input(Input::KeyDown(0x20));
                h.input(Input::KeyUp(0x20));
                let e = &h.game.as_ref().unwrap().forms.enhance;
                assert!(e.visible);
                let f = e.bar_frame(1).unwrap();
                let arrow = (e.left + f.left + 4, e.top + f.top + f.height - 4);
                press(&mut h, arrow);
                hold(&mut h, 450);
                release(&mut h, arrow);
                assert_eq!(h.game.as_ref().unwrap().forms.enhance.scrolled(1), 3);
                let e = &mut h.game.as_mut().unwrap().forms.enhance;
                e.visible = false;
            }
            let game = h.game.as_mut().unwrap();
            game.forms.options.visible = false;
            // The buildings' overview: its bar takes the mouse when there's
            // anything to scroll.
            game.open_umenu();
            game.sync_stack();
            let u = &game.forms.umenu;
            assert!(u.visible);
            let f = u.scroll_bar().frame;
            let arrow = (u.left + f.left + 4, u.top + f.top + f.height - 4);
            let (min, max) = (u.scroll_bar().min, u.scroll_bar().max);
            press(&mut h, arrow);
            hold(&mut h, 450);
            release(&mut h, arrow);
            let p = h.game.as_ref().unwrap().forms.umenu.scroll_bar().position;
            let steps = if fixed { 3 } else { 1 };
            assert_eq!(p, (min + steps).min(max));
        }
    }

    /// A press and release of the left button at `x`, `y` on the screen.
    fn click_at(h: &mut Holiday, (x, y): (i32, i32)) {
        h.input(Input::MouseDown {
            x,
            y,
            button: Button::Left,
        });
        h.input(Input::MouseUp {
            x,
            y,
            button: Button::Left,
        });
    }

    /// The options open and "INFO" pressed with its key, Space.
    fn press_info(h: &mut Holiday) {
        h.game.as_mut().unwrap().click_button(2);
        assert!(h.game.as_ref().unwrap().forms.options.visible);
        h.input(Input::KeyDown(0x20));
        h.input(Input::KeyUp(0x20));
    }

    /// The main screen with its forms, the auction, the options and the
    /// list of enhancements on the German (`orig/`) and the English
    /// (`orig-en/`) folder, each in its own language (`auto`) and in the
    /// other one forced (`docs/language.md`). The view's dates, numbers
    /// and the port's texts follow the language; the world's text (the
    /// news archive's dates) stays the folder's. With `HOLIDAY_PNG` set,
    /// the screens go there as `lang-<folder>[-lang-<forced>]-<screen>.png`.
    /// Skipped for a folder that isn't there.
    #[test]
    fn language_screens() {
        use formats::data::Land;
        use lang::{Language, Setting};
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for (folder, land, setting, tag) in [
            ("orig", Land::Deutschland, Setting::Auto, "de"),
            ("orig-en", Land::England, Setting::Auto, "en"),
            ("orig", Land::Deutschland, Setting::Is(Language::En), "de-lang-en"),
            ("orig-en", Land::England, Setting::Is(Language::De), "en-lang-de"),
        ] {
            let dir = root.join(folder);
            if !dir.join("GRAFIK").exists() && !dir.join("Grafik").exists() {
                eprintln!("skipping: {} not found", dir.display());
                continue;
            }
            assert_eq!(folder_land(&dir), land);
            let mut config = Config::default();
            config.view.set(enhance::Enhancement::BigMap, false);
            // The forms' buttons close them rather than raise them.
            config.view.set(enhance::Enhancement::WindowStacking, false);
            config.language = setting;
            let mut h = holiday_in_a_game(&dir, SIZE, config);
            let game = h.game.as_mut().unwrap();
            let language = setting.language(land);
            assert_eq!(game.info.language(), language);
            assert_eq!(game.info.land, language.land(land));
            // The world's date format is the folder's, whatever the view's.
            assert_eq!(game.world.news.date_format.land, land as u8);
            let date = game.world.date.text(&game.info.months, game.info.land);
            // `6. Januar 1996`, `6th January 1996`.
            let dot = date.iter().take_while(|b| *b != &b' ').any(|&b| b == b'.');
            assert_eq!(dot, language == Language::De, "{}", String::from_utf8_lossy(&date));
            assert_eq!(
                game::money::format_money(1_234_567, 1, game.info.land),
                language.pick(&b"1.234.567"[..], &b"1,234,567"[..])
            );
            write_png(&h, &format!("lang-{tag}-main"));
            // The info bar's date and money, with the forms closed.
            let game = h.game.as_mut().unwrap();
            for button in [3, 4, 5] {
                game.click_button(button);
            }
            let _ = game.update(Duration::ZERO);
            assert!(!game.forms.tv.visible && !game.forms.money.shown());
            h.repaint();
            write_png(&h, &format!("lang-{tag}-infobar"));
            // An auction: the item's picture, its texts and the price.
            let game = h.game.as_mut().unwrap();
            game.world.auction.item = 9;
            game.world.auction.price = 12_500;
            game.world.auction.start_price = 12_500;
            game.open_auction();
            assert!(game.auction.visible);
            h.repaint();
            write_png(&h, &format!("lang-{tag}-auction"));
            let game = h.game.as_mut().unwrap();
            game.auction.visible = false;
            // The options, and from their "INFO" the enhancements.
            game.click_button(2);
            assert!(game.forms.options.visible);
            h.repaint();
            write_png(&h, &format!("lang-{tag}-options"));
            h.input(Input::KeyDown(0x20));
            h.input(Input::KeyUp(0x20));
            assert!(h.game.as_ref().unwrap().forms.enhance.visible);
            h.repaint();
            write_png(&h, &format!("lang-{tag}-enhance"));
        }
    }

    /// The middle of `r`.
    fn middle(r: Rect) -> (i32, i32) {
        (i32::midpoint(r.left, r.right), i32::midpoint(r.top, r.bottom))
    }

    /// The options' "INFO": in the original (every enhancement off) the
    /// credits (`TCreditform`), which disable the options until their
    /// close button closes them; with `enhancements_screen` the list of
    /// enhancements, from which the credits are a button away. Both are
    /// modal and lie on top in either stacking, with `big_map` too. With
    /// `HOLIDAY_PNG` set, the screens go there as `enhance-screen-*.png`.
    #[test]
    fn info_opens_the_credits_or_the_enhancements() {
        let Some(dir) = game_dir() else { return };
        // The original.
        let mut h = holiday_in_a_game(&dir, SIZE, Config::ORIGINAL);
        press_info(&mut h);
        let game = h.game.as_mut().unwrap();
        assert!(game.forms.credits.visible && !game.forms.enhance.visible);
        let credits = game.forms.credits.bounds();
        assert_eq!(credits, Rect::new(80, 60, 640, 480), "centred");
        assert_eq!(game.window_at(middle(credits).0, middle(credits).1), Win::Credits);
        // The options are disabled, and keys do nothing.
        let options = game.forms.options.bounds();
        assert!(game.forms_key(0x1b, ShiftState::default()));
        assert!(game.forms.credits.visible && game.forms.options.visible);
        h.repaint();
        write_png(&h, "enhance-screen-credits");
        // The hidden "Schließen" doesn't close them; the close button does.
        click_at(&mut h, (credits.left + 0xf8 + 20, credits.top + 0x1c4 + 8));
        assert!(h.game.as_ref().unwrap().forms.credits.visible);
        click_at(&mut h, (credits.left + 4, credits.top + 4));
        let game = h.game.as_mut().unwrap();
        assert!(!game.forms.credits.visible && game.forms.options.visible);
        assert_eq!(game.window_at(middle(options).0, middle(options).1), Win::Options);

        // Every enhancement on, `big_map` at a bigger size.
        let mut h = holiday_in_a_game(&dir, (1024, 768), Config::default());
        press_info(&mut h);
        let game = h.game.as_mut().unwrap();
        assert!(game.forms.enhance.visible && !game.forms.credits.visible);
        assert!(game.stack.main_desktop, "big_map's main screen fills the screen");
        let list = game.forms.enhance.bounds();
        assert_eq!(list, Rect::new(192, 144, 640, 480));
        assert_eq!(game.window_at(middle(list).0, middle(list).1), Win::Enhance);
        assert!(game.clock_stopped());
        h.repaint();
        write_png(&h, "enhance-screen");
        // "Mitwirkende": the credits over the list, then back to it.
        let game = h.game.as_ref().unwrap();
        let (bx, by) = game.forms.enhance.button_at(3).unwrap();
        click_at(&mut h, (list.left + bx, list.top + by));
        let game = h.game.as_mut().unwrap();
        assert!(game.forms.credits.visible && game.forms.enhance.visible);
        assert_eq!(game.window_at(middle(list).0, middle(list).1), Win::Credits);
        h.repaint();
        write_png(&h, "enhance-screen-credits-over-list");
        let credits = h.game.as_ref().unwrap().forms.credits.bounds();
        click_at(&mut h, (credits.left + 4, credits.top + 4));
        let game = h.game.as_mut().unwrap();
        assert!(!game.forms.credits.visible && game.forms.enhance.visible);
        assert_eq!(game.window_at(middle(list).0, middle(list).1), Win::Enhance);
        // Escape closes the list; the options are back.
        h.input(Input::KeyDown(0x1b));
        let game = h.game.as_mut().unwrap();
        assert!(!game.forms.enhance.visible && game.forms.options.visible);

        // The original's stacking, without `big_map`, keeps them on top too.
        let mut config = Config::ORIGINAL;
        config.view.set(enhance::Enhancement::EnhancementsScreen, true);
        let mut h = holiday_in_a_game(&dir, (640, 480), config);
        press_info(&mut h);
        let game = h.game.as_mut().unwrap();
        assert!(game.forms.enhance.visible);
        assert_eq!(game.forms.enhance.bounds(), Rect::new(0, 0, 640, 480));
        let tv = game.forms.tv.bounds();
        assert_eq!(game.window_at(middle(tv).0, middle(tv).1), Win::Enhance);
    }

    /// The world's hash, as a desync check would take it.
    fn world_hash(world: &World) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        world.hash(&mut h);
        h.finish()
    }

    /// The list of enhancements in a game: a world fix goes through the
    /// command path (`World::apply`, which changes the world's hash) and
    /// becomes the default for new games; a view enhancement applies at
    /// once (`big_map`'s main screen, `window_stacking`'s stack); each is
    /// saved in `settings.ini`, the file's other lines kept, and read back
    /// as the same settings. "Alle aus" is the original, also from the
    /// file. With `HOLIDAY_PNG` set, the list scrolled to its last rows
    /// goes there as `enhance-screen-scrolled.png`.
    #[test]
    #[allow(clippy::too_many_lines)] // One game, step by step.
    fn switching_enhancements_in_a_game() {
        let Some(dir) = game_dir() else { return };
        let tmp = std::env::temp_dir().join(format!("holiday-enhance-screen-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        let ini = tmp.join("settings.ini");
        std::fs::write(&ini, "# mine\nwindow_width=1024\nenhance.music=off\n").unwrap();
        let config = enhance_config(Some(&ini), false, &[]);
        assert!(!config.view.has(enhance::Enhancement::Music));
        let mut h = holiday_in_a_game(&dir, (1024, 768), config);
        h.settings_file = Some(ini.clone());
        h.command_line = true;
        h.game.as_mut().unwrap().forms.enhance_notes = h.enhance_notes();
        press_info(&mut h);
        let list = h.game.as_ref().unwrap().forms.enhance.bounds();
        let at = |h: &Holiday, key: &str| {
            let (x, y) = h.game.as_ref().unwrap().forms.enhance.row_of(key).unwrap();
            (list.left + x, list.top + y)
        };

        // A world fix: a command, applied before the next tick.
        let game = h.game.as_mut().unwrap();
        assert!(game.world.fixes.has(Fix::ShipDetour));
        let before = world_hash(&game.world);
        let p = at(&h, "ship_detour");
        click_at(&mut h, p);
        let game = h.game.as_mut().unwrap();
        assert_eq!(
            game.commands.last().map(|c| (c.player, &c.action)),
            Some((1, &command::Action::SetFix(Fix::ShipDetour, false)))
        );
        assert!(game.world.fixes.has(Fix::ShipDetour), "not yet applied");
        let _ = game.update(Duration::ZERO);
        assert!(!game.world.fixes.has(Fix::ShipDetour));
        assert_ne!(world_hash(&game.world), before);
        assert!(!h.enhance.fixes.has(Fix::ShipDetour), "the default for new games");

        // A view enhancement, at once: `big_map` off puts the original's
        // main screen in the middle of the screen.
        assert_ne!(
            h.game.as_ref().unwrap().screen.size(),
            (hauptschirm::WIDTH, hauptschirm::HEIGHT)
        );
        let p = at(&h, "big_map");
        click_at(&mut h, p);
        let game = h.game.as_ref().unwrap();
        assert_eq!(game.screen.size(), (hauptschirm::WIDTH, hauptschirm::HEIGHT));
        assert!(!game.stack.main_desktop && game.forms.enhance.visible);
        assert!(!h.enhance.view.has(enhance::Enhancement::BigMap));
        // `window_stacking`, in the middle of the game.
        assert!(h.game.as_ref().unwrap().stack.enhanced);
        let p = at(&h, "window_stacking");
        click_at(&mut h, p);
        assert!(!h.game.as_ref().unwrap().stack.enhanced);

        // settings.ini: the three switches, the other lines kept, and
        // the same settings read back.
        let text = std::fs::read_to_string(&ini).unwrap();
        assert!(
            text.starts_with("# mine\nwindow_width=1024\nenhance.music=off\n"),
            "{text}"
        );
        for line in [
            "enhance.ship_detour=off",
            "enhance.big_map=off",
            "enhance.window_stacking=off",
        ] {
            assert!(text.lines().any(|l| l == line), "{line} in {text}");
        }
        assert_eq!(enhance_config(Some(&ini), false, &[]), h.enhance);
        // The command line goes over the file at the next start.
        assert_eq!(enhance_config(Some(&ini), true, &[]), Config::ORIGINAL);

        // The right list scrolled to its end, by its scroll bar's trough.
        let game = h.game.as_mut().unwrap();
        for _ in 0..3 {
            game.forms.enhance.scroll_to(1, usize::MAX);
        }
        assert!(game.forms.enhance.scrolled(1) > 0);
        let p = at(&h, "enhancements_screen");
        // The row under the mouse is described below the lists.
        h.input(Input::MouseMove { x: p.0, y: p.1 });
        h.repaint();
        write_png(&h, "enhance-screen-scrolled");

        // The list's own switch off: "INFO" opens the credits next time.
        click_at(&mut h, p);
        assert!(!h.enhance.view.has(enhance::Enhancement::EnhancementsScreen));
        h.repaint();
        write_png(&h, "enhance-screen-own-switch-off");
        let game = h.game.as_ref().unwrap();
        let (bx, by) = game.forms.enhance.button_at(1).unwrap();
        // "Alle an", then "Alle aus (Original)".
        click_at(&mut h, (list.left + bx, list.top + by));
        assert_eq!(h.enhance, Config::default());
        let (bx, by) = h.game.as_ref().unwrap().forms.enhance.button_at(2).unwrap();
        click_at(&mut h, (list.left + bx, list.top + by));
        assert_eq!(h.enhance, Config::ORIGINAL);
        let game = h.game.as_mut().unwrap();
        let _ = game.update(Duration::ZERO);
        assert_eq!(game.world.fixes, Fixes::ORIGINAL);
        assert!(!game.stack.enhanced);
        let text = std::fs::read_to_string(&ini).unwrap();
        assert_eq!(text, "# mine\nwindow_width=1024\noriginal=true\n");
        assert_eq!(enhance_config(Some(&ini), false, &[]), Config::ORIGINAL);
        // Closed, the options' "INFO" now opens the credits.
        h.input(Input::KeyDown(0x0d));
        h.input(Input::KeyDown(0x20));
        let game = h.game.as_ref().unwrap();
        assert!(game.forms.credits.visible && !game.forms.enhance.visible);
        std::fs::remove_dir_all(&tmp).unwrap();
    }

    /// `language=` in `settings.ini`: `auto` without one or with a value
    /// that isn't a language.
    #[test]
    fn language_from_settings() {
        let tmp = std::env::temp_dir().join(format!("holiday-language-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        let ini = tmp.join("settings.ini");
        assert_eq!(language_setting(None), lang::Setting::Auto);
        assert_eq!(language_setting(Some(&ini)), lang::Setting::Auto);
        std::fs::write(&ini, "window_width=800\nlanguage=en\n").unwrap();
        assert_eq!(language_setting(Some(&ini)), lang::Setting::Is(lang::Language::En));
        std::fs::write(&ini, "language=klingon\n").unwrap();
        assert_eq!(language_setting(Some(&ini)), lang::Setting::Auto);
        std::fs::remove_dir_all(&tmp).unwrap();
    }

    /// Every key's label, German and English, fits its row in one line
    /// of the game's font, and the descriptions their two lines.
    #[test]
    fn labels_fit_their_rows() {
        let Some(dir) = game_dir() else { return };
        let font = font(&dir);
        for i in enhance::all() {
            for language in [lang::Language::De, lang::Language::En] {
                let text = lang::latin1(i.label(language));
                // A row's text is 280 - 16 - 3 wide (`ui_radio_draw`).
                let label = i.label(language);
                assert_eq!(font.wrap(&text, 261, game::text::Align::Left).len(), 1, "{label}");
                let line = lang::latin1(&format!("{}: {}", i.key, i.description(language)));
                assert!(font.wrap(&line, 616, game::text::Align::Left).len() <= 2, "{}", i.key);
            }
        }
    }

    /// The two game folders, each in its own language and in the other
    /// forced, with a tag for the screenshots; those not there skipped.
    fn folders() -> Vec<(PathBuf, lang::Setting, &'static str)> {
        use lang::{Language, Setting};
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        [
            ("orig", Setting::Auto, "de"),
            ("orig-en", Setting::Auto, "en"),
            ("orig", Setting::Is(Language::En), "de-lang-en"),
            ("orig-en", Setting::Is(Language::De), "en-lang-de"),
        ]
        .into_iter()
        .filter_map(|(folder, setting, tag)| {
            let dir = root.join(folder);
            if dir.join("GRAFIK").exists() || dir.join("Grafik").exists() {
                Some((dir, setting, tag))
            } else {
                eprintln!("skipping: {} not found", dir.display());
                None
            }
        })
        .collect()
    }

    /// A game with the forms `holiday_in_a_game` opens closed again, its
    /// saves in a directory of its own.
    fn quiet_game(dir: &Path, setting: lang::Setting, saves: &str) -> Holiday {
        let mut config = Config::default();
        config.view.set(enhance::Enhancement::BigMap, false);
        config.view.set(enhance::Enhancement::WindowStacking, false);
        config.language = setting;
        let mut h = holiday_in_a_game(dir, SIZE, config);
        let game = h.game.as_mut().unwrap();
        for button in [3, 4, 5] {
            game.click_button(button);
        }
        let _ = game.update(Duration::ZERO);
        h.saves.dir = std::env::temp_dir().join(format!("holiday-{saves}-{}", std::process::id()));
        std::fs::create_dir_all(&h.saves.dir).unwrap();
        h.repaint();
        h
    }

    fn key(h: &mut Holiday, key: u16) {
        h.input(Input::KeyDown(key));
        h.input(Input::KeyUp(key));
    }

    /// The file menu's message boxes: an invalid name's error, and the
    /// question before "Löschen", which erases the game only on OK, in the
    /// view's language; the box is modal. With `HOLIDAY_PNG` set they go
    /// there as `step4-invalid-name-<tag>.png` and `step4-delete-<tag>.png`.
    #[test]
    fn file_menu_message_boxes() {
        use lang::Language;
        for (dir, setting, tag) in folders() {
            let mut h = quiet_game(&dir, setting, &format!("msgbox-{tag}"));
            let language = h.language();
            h.game.as_mut().unwrap().forms.requests.push(Request::FileMenu);
            h.process_requests();
            assert!(h.game.as_ref().unwrap().file_menu.visible);
            // An invalid name: an error with OK, which Enter answers.
            h.file_menu_response(dateimenue::Response {
                action: Some(dateimenue::Action::InvalidName),
                ..dateimenue::Response::default()
            });
            let m = h.message.as_ref().expect("the error");
            assert_eq!(m.then, AfterMessage::Nothing);
            assert_eq!(m.dlg.buttons(), [Btn::Ok]);
            write_png(&h, &format!("step4-invalid-name-{tag}"));
            // Modal: the file menu doesn't get the keys meanwhile.
            key(&mut h, 0x51);
            assert!(h.message.is_some() && h.game.as_ref().unwrap().file_menu.visible);
            key(&mut h, 0x0d);
            assert!(h.message.is_none());
            // "Löschen": Cancel (Escape) keeps the file, OK (Enter, the
            // focused button) erases it.
            let map = h.game.as_ref().unwrap().map();
            let path = h.saves.write(b"TESTDEL", &map).unwrap();
            let delete = || dateimenue::Response {
                action: Some(dateimenue::Action::Delete(b"TESTDEL".to_vec())),
                ..dateimenue::Response::default()
            };
            h.file_menu_response(delete());
            let m = h.message.as_ref().expect("the question");
            assert_eq!(m.dlg.buttons(), [Btn::Ok, Btn::Cancel]);
            assert!(path.exists(), "nothing erased before the answer");
            write_png(&h, &format!("step4-delete-{tag}"));
            // Dragged by its title bar.
            let before = h.message.as_ref().unwrap().dlg.bounds();
            let grab = (before.left + 60, before.top + 5);
            h.input(Input::MouseDown {
                x: grab.0,
                y: grab.1,
                button: Button::Left,
            });
            h.input(Input::MouseMove {
                x: grab.0 + 30,
                y: grab.1 + 20,
            });
            h.input(Input::MouseUp {
                x: grab.0 + 30,
                y: grab.1 + 20,
                button: Button::Left,
            });
            let after = h.message.as_ref().expect("still open").dlg.bounds();
            assert_eq!((after.left, after.top), (before.left + 30, before.top + 20));
            key(&mut h, 0x1b);
            assert!(h.message.is_none() && path.exists());
            h.file_menu_response(delete());
            // The OK button clicked with the mouse.
            let ok_at = middle(h.message.as_ref().unwrap().dlg.button_bounds(0));
            h.input(Input::MouseDown {
                x: ok_at.0,
                y: ok_at.1,
                button: Button::Left,
            });
            write_png(&h, &format!("step4-delete-pressed-{tag}"));
            h.input(Input::MouseUp {
                x: ok_at.0,
                y: ok_at.1,
                button: Button::Left,
            });
            assert!(h.message.is_none(), "answered");
            assert!(!path.exists(), "erased on OK");
            assert_eq!(h.cursor, Cursor::HourGlass, "under the hourglass");
            h.update(Duration::ZERO);
            h.update(Duration::ZERO);
            assert_eq!(h.cursor, Cursor::Default);
            // The texts are the view's.
            let expected: &[u8] = match language {
                Language::De => b"Wirklich loeschen?",
                Language::En => b"Are you sure?",
            };
            assert_eq!(
                dateimenue::delete_question(h.game.as_ref().unwrap().info.land),
                expected
            );
            std::fs::remove_dir_all(&h.saves.dir).unwrap();
        }
    }

    /// The auction's bid without the money: `FEHLER.TXT`'s error 0xe on the
    /// status bar, as the original shows it (not a message box), and no
    /// bid. With `HOLIDAY_PNG` set the screen goes there as
    /// `step4-auction-no-money-<tag>.png`.
    #[test]
    fn auction_without_the_money() {
        for (dir, setting, tag) in folders() {
            let mut h = quiet_game(&dir, setting, &format!("auction-{tag}"));
            let game = h.game.as_mut().unwrap();
            game.world.auction.item = 9;
            game.world.auction.price = 12_500;
            game.world.auction.start_price = 12_500;
            game.world.auction.joined = true;
            game.world.auction.limits[0] = 0;
            game.open_auction();
            let commands = game.commands.len();
            let (x, y) = (
                game.auction.left + 248,
                game.auction.top + auktionsform::LAYOUT[2].1.top + 20,
            );
            let press = Input::MouseDown {
                x,
                y,
                button: Button::Left,
            };
            let release = Input::MouseUp {
                x,
                y,
                button: Button::Left,
            };
            assert!(game.auction_mouse(press, ShiftState::LEFT, false));
            assert!(game.auction_mouse(release, ShiftState::default(), true));
            assert_eq!(game.commands.len(), commands, "no bid");
            assert_eq!(game.screen.status_timeout(), 20, "the error on the status bar");
            h.repaint();
            assert!(h.message.is_none());
            write_png(&h, &format!("step4-auction-no-money-{tag}"));
        }
    }

    /// States 1200 to 1500: the progress window (`TWasNunForm`) shows a
    /// step a frame, the island made after the frame of state 1210, with
    /// `crNoDrop` meanwhile, then the main screen. With `HOLIDAY_PNG` set
    /// the window at state 1210 goes there as `step4-progress-<tag>.png`.
    #[test]
    fn the_progress_window() {
        for (dir, setting, tag) in folders() {
            let sound = Sound::new(&dir, setup::Sound::Hi, platform::audio::Audio::silent()).shared();
            let saves = Saves {
                dir: std::env::temp_dir().join("holiday-progress-test-saves"),
                game_maps: dir.join("MAPS"),
            };
            let config = Config {
                language: setting,
                ..Config::default()
            };
            let mut h = Holiday::new(dir.clone(), saves, sound, config);
            h.resize(800, 600);
            h.intro_step(Duration::ZERO, MENU_AT);
            h.start.hide();
            h.pending = Some(Pending::Start { scenario: false });
            h.start_pending();
            let form = |h: &Holiday| h.loading.as_ref().map(|l| (l.form.text().to_vec(), l.form.percent()));
            assert_eq!(form(&h), Some((Vec::new(), 0)), "state 1200");
            h.update(Duration::ZERO);
            let texts = wasnun_texts(&dir);
            assert_eq!(
                form(&h),
                Some((texts[wasnunform::page::NEW_MAP].clone(), 5)),
                "state 1210"
            );
            assert_eq!(h.frame().cursor, Cursor::NoDrop);
            assert!(h.game.is_none());
            write_png(&h, &format!("step4-progress-{tag}"));
            h.update(Duration::ZERO);
            assert_eq!(
                form(&h),
                Some((texts[wasnunform::page::INIT_GAME].clone(), 100)),
                "state 1220"
            );
            h.update(Duration::ZERO);
            assert_eq!(
                form(&h),
                Some((texts[wasnunform::page::LOAD_MAP].clone(), 100)),
                "state 1500"
            );
            assert_eq!(h.frame().cursor, Cursor::Default);
            h.update(Duration::ZERO);
            assert!(h.loading.is_none() && h.game.is_some());
            // A scenario's state 1210 clears the map: page 3 at 100 %.
            h.game = None;
            h.start.menu = start_menu(
                &dir,
                &h.exe,
                Settings {
                    scenario: 2,
                    ..Settings::default()
                },
            )
            .menu;
            h.pending = Some(Pending::Start { scenario: true });
            h.start_pending();
            h.update(Duration::ZERO);
            assert_eq!(form(&h), Some((texts[wasnunform::page::INIT_MAP].clone(), 100)));
            h.finish_loading();
            assert_eq!(h.game.as_ref().unwrap().world.scenario, 2);
        }
    }

    /// "Neues Spiel" puts back what the original's `back_to_start_menu`
    /// does: the start menu's settings but the opponents and the name,
    /// and the options, so the next game starts as it would on a fresh
    /// start with that menu. With `HOLIDAY_PNG` set, the CD question goes
    /// there as `step4-insert-cd-<tag>.png`.
    /// A program on `dir` past the intro, its games' `RandSeed` fixed.
    fn fixed_holiday(dir: &Path) -> Holiday {
        let sound = Sound::new(dir, setup::Sound::Hi, platform::audio::Audio::silent()).shared();
        let saves = Saves {
            dir: std::env::temp_dir().join(format!("holiday-back-test-{}", std::process::id())),
            game_maps: dir.join("MAPS"),
        };
        let mut h = Holiday::new(dir.to_path_buf(), saves, sound, Config::default());
        h.seed = || 0x1996_0611;
        h.resize(800, 600);
        h.intro_step(Duration::ZERO, MENU_AT);
        h
    }

    #[test]
    fn back_to_the_start_menu_starts_clean() {
        let Some(dir) = game_dir() else { return };
        let new = || fixed_holiday(&dir);
        let start = |h: &mut Holiday| {
            h.respond(startform::Response {
                action: Some(Action::Start { scenario: false }),
                ..startform::Response::default()
            });
            h.start_pending();
            h.finish_loading();
        };
        // The first game from a menu that isn't the defaults, played a
        // little with the options changed.
        let mut h = new();
        let custom = Settings {
            relief: 4,
            land: 0,
            rivers: 9,
            difficulty: 2,
            opponents: 3,
            players: 4,
            colours: [3, 1, 2, 4, 5, 6, 7, 8],
            name: b"Tester".to_vec(),
            ..Settings::default()
        };
        h.start = start_menu(&dir, &h.exe, custom);
        h.start.show(h.size);
        start(&mut h);
        let game = h.game.as_mut().unwrap();
        game.forms.settings.music = false;
        game.forms.settings.news = [false; 4];
        game.forms.settings.speech = false;
        game.forms.settings.sound_volume = 30;
        game.world.clock.paused = false;
        let _ = game.update(Duration::from_millis(u64::from(TICK_MS) * 20));
        h.back_to_start_menu();
        assert!(h.message.is_none(), "the CD is there");
        assert!(h.start.visible() && h.game.is_none());
        // `init_game_defaults`: the defaults, but the opponents and the name.
        let menu = h.start.menu.settings().clone();
        assert_eq!(
            menu,
            Settings {
                opponents: 3,
                players: 4,
                name: b"Tester".to_vec(),
                ..Settings::default()
            }
        );
        // The options: the defaults, but the speech.
        assert_eq!(
            h.options,
            Options {
                speech: false,
                ..Options::default()
            }
        );
        start(&mut h);
        let second = h.game.take().unwrap();
        // A fresh start with that menu and those options.
        let mut fresh = new();
        fresh.start = start_menu(&dir, &fresh.exe, menu);
        fresh.start.show(fresh.size);
        fresh.options.speech = false;
        start(&mut fresh);
        let first = fresh.game.take().unwrap();
        assert_eq!(world_hash(&second.world), world_hash(&first.world));
        assert_eq!(second.forms.settings, first.forms.settings);
        assert_eq!(second.screen.camera(), first.screen.camera());
        assert_eq!(second.tools.down, first.tools.down);
        std::fs::remove_dir_all(&h.saves.dir).ok();
    }
}
