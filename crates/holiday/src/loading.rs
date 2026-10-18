//! States 1210 to 1500 of `hintergrund`'s timer once Start (or a saved
//! game) was picked and `titel.avi` is over: the game set up a step at a
//! time, each shown on the progress window (`TWasNunForm`,
//! `game::wasnunform`) as the original shows it once its work is done
//! (en 1010:0591, `docs/startup.md`). [`Setup::step`] does one step;
//! `Holiday` does one a frame, so the gauge moves as the work goes.
//!
//! | State | Step | Page | Gauge after it |
//! |---|---|---|---|
//! | 1210, a new island | `generate_island`'s stages (en 10b8:52d4) | 1 | 5, 26, 32, 48, 51, 70, 74, 100 |
//! | 1210, a scenario or a saved game | the map cleared | 3 | 100 |
//! | 1220 | `load_game_data` (en 1038:3615) | 2 | 1 |
//! | | `load_tables` | | 7 |
//! | | `load_game_bitmaps` | | 50 |
//! | | `load_game_rest`'s loaders (en 1038:33c8) | | 0x37 to 0x63 |
//! | | the rest | | 100 |
//! | 1500 | `load_map` (en 1038:357f) | 0 | 100 |
//!
//! The port's work at each point is its own counterpart of the
//! original's: the world's tables and its set-up at `load_tables` (for a
//! scenario or a saved game the map's world too, which the original loads
//! in state 1500 into what 1220 made), the main screen's bitmaps and the
//! island's tiles at `load_game_bitmaps`, and each of `load_game_rest`'s
//! loaders that makes a part of the view (the dates' texts, the sabotage
//! window, the file menu, the TV, the auction form, the build menu, the
//! overview map, the info box, the money menu, the playlist, the tooltip,
//! the high scores). Its loaders of the world (`create_people`,
//! `create_computers`, `init_players`, `load_ereignis`, ...) are part of
//! `load_tables`' set-up, so their percentages pass with the next step.

use std::collections::VecDeque;

use super::{
    Auktionsform, Baumenue, Command, Config, Duration, Fields, FileMenu, Font, Form, Forms, Game, Hauptschirm,
    InfoTexts, Island, MapView, Options, Path, PathBuf, Pending, Random, Rc, SaveGame, ScoreTable, Settings, Stack,
    Start, Surface, Tiles, Tools, World, auktion, auktionsform, auswahl, baumenu_dat, baumenu_txt, baumenue, bitmap,
    bitmaps, enhance, fehler, file_menu, folder_land, font, generated_start, hauptschirm, hausname, hintergrund, lang,
    lang_text, load_form, load_tiles, message_media_names, monate, read, read_opt, sabowin, scenario_start, seed,
    sound, upgrade, wasnunform,
};
use crate::forms::{FormParts, FormStep};

/// What a step shows on the progress window, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Show {
    /// `wasnun_set_page` (en 1028:39e3): page `n` of `WASNUN.TXT`, the
    /// gauge empty.
    Page(usize),
    /// The gauge at a percentage (en 1028:3a09).
    Percent(i32),
}

/// A step of the set-up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Step {
    /// State 1210 for a new island: stage `n` of `generate_island`
    /// (en 10b8:52d4); before the first, the game's data and state 100.
    Island(usize),
    /// State 1210 for a scenario or a saved game: `clear_map`, which
    /// leaves the empty island a world starts with.
    MapCleared,
    /// State 1220's start: page 2 and 1 % (en 1038:3615).
    GameData,
    /// `load_tables`: the world's tables and set-up.
    Tables,
    /// `load_game_bitmaps`: the main screen and the island's tiles.
    Bitmaps,
    /// One of `load_game_rest`'s loaders (en 1038:33c8).
    Rest(Rest),
    /// The rest of the forms and the game put together.
    Assembled,
    /// State 1500: page 0 around `load_map` (en 1038:357f).
    LoadMap,
}

/// `load_game_rest`'s loaders that make a part of the view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Rest {
    /// `monate_create`: the dates' months and the info bar.
    Dates,
    /// `load_sabotage`: the sabotage window.
    Sabotage,
    /// `dateimenue_reset`: the file menu.
    FileMenu,
    /// `load_auktion`: the auction form.
    Auction,
    /// `baumenue_create`: the build menu and the tools.
    BuildMenu,
    /// `music_init_playlist`.
    Music,
    /// One of the forms (`crate::forms::FormStep`).
    Form(FormStep),
}

/// `load_game_rest`'s loaders, in its order, with the gauge after each.
/// // en 1038:33c8
pub(crate) const REST: [(Rest, i32); 12] = [
    (Rest::Dates, 0x37),
    (Rest::Sabotage, 0x41),
    (Rest::FileMenu, 0x43),
    (Rest::Form(FormStep::Tv), 0x48),
    (Rest::Auction, 0x4b),
    (Rest::BuildMenu, 0x4d),
    (Rest::Form(FormStep::Overview), 0x50),
    (Rest::Form(FormStep::Infobox), 0x52),
    (Rest::Form(FormStep::Money), 0x55),
    (Rest::Music, 0x60),
    (Rest::Form(FormStep::Help), 0x62),
    (Rest::Form(FormStep::HighScores), 0x63),
];

/// What is set up.
enum Kind {
    /// A new island from the start menu's settings.
    Island,
    /// A scenario's map (the start menu's scenario page, or its file).
    Scenario { path: String, scenario_page: bool },
    /// A saved game.
    Load(Box<SaveGame>),
    /// A network game's world, which a client joins, read from the
    /// host's snapshot (`network::joined_world`), and the snapshot.
    Join(Box<World>, Box<SaveGame>),
}

/// A game being set up.
pub(crate) struct Setup {
    dir: PathBuf,
    exe: Vec<u8>,
    settings: Settings,
    options: Options,
    sound: sound::Shared,
    size: (i32, i32),
    config: Config,
    seed: u32,
    kind: Kind,
    steps: VecDeque<Step>,
    /// The high scores, until the forms take them.
    high: Option<ScoreTable>,
    /// A new island's game data and world, and its plots.
    data: Option<hintergrund::GameData>,
    world: Option<World>,
    plots: u16,
    /// What the game starts from, once the world is set up, and the
    /// player's language setting.
    start: Option<(Start, lang::Setting)>,
    /// The game's parts, once its bitmaps are loaded.
    parts: Option<GameParts>,
    game: Option<Game>,
}

impl Setup {
    /// The set-up of `pending`: a new island or a scenario from the start
    /// menu's `settings`, or a saved game, with the player's `options`,
    /// `high` scores, `config` and `RandSeed` (`seed`), on a screen of
    /// `size`. // en 1010:0591
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        dir: &Path,
        exe: &[u8],
        pending: Pending,
        settings: &Settings,
        options: Options,
        high: ScoreTable,
        sound: &sound::Shared,
        size: (i32, i32),
        config: &Config,
        seed: u32,
    ) -> Setup {
        let (kind, options) = match pending {
            Pending::Start { scenario } => match hintergrund::new_island(settings, scenario) {
                Island::Generated => (Kind::Island, options),
                Island::Scenario(path) => (
                    Kind::Scenario {
                        path,
                        scenario_page: scenario,
                    },
                    options,
                ),
            },
            Pending::Load(map) => {
                let options = options.with_map(&map);
                (Kind::Load(Box::new(map)), options)
            }
            Pending::Join(world, map) => (Kind::Join(world, map), options),
        };
        let mut steps: VecDeque<Step> = match kind {
            Kind::Island => (0..game::generator::STAGES.len()).map(Step::Island).collect(),
            _ => [Step::MapCleared].into(),
        };
        steps.extend([Step::GameData, Step::Tables, Step::Bitmaps]);
        steps.extend(REST.iter().map(|&(r, _)| Step::Rest(r)));
        steps.extend([Step::Assembled, Step::LoadMap]);
        Setup {
            dir: dir.to_path_buf(),
            exe: exe.to_vec(),
            settings: settings.clone(),
            options,
            sound: Rc::clone(sound),
            size,
            config: *config,
            seed,
            kind,
            steps,
            high: Some(high),
            data: None,
            world: None,
            plots: 0,
            start: None,
            parts: None,
            game: None,
        }
    }

    /// Does the next step and says which it was and what the progress
    /// window shows after it; none when the set-up is done.
    pub fn step(&mut self) -> Option<(Step, Vec<Show>)> {
        let step = self.steps.pop_front()?;
        let shows = match step {
            Step::Island(n) => {
                let mut shows = Vec::new();
                if n == 0 {
                    // State 100 (the world's text in the player's
                    // language), then 1210: page 1 and `clear_map`.
                    let language = self.config.language.language(folder_land(&self.dir));
                    let data = hintergrund::GameData::load(|name| read_opt(&self.dir, name), language)
                        .unwrap_or_else(|e| panic!("{e}"));
                    self.world = Some(hintergrund::set_up_begin(
                        &self.settings,
                        &data,
                        self.seed,
                        self.config.fixes,
                    ));
                    self.data = Some(data);
                    shows.push(Show::Page(wasnunform::page::NEW_MAP));
                }
                let (Some(world), Some(data)) = (&mut self.world, &self.data) else {
                    return Some((step, shows));
                };
                let percent = world.generate_stage(n, &self.settings, &data.maperste, &mut self.plots);
                shows.push(Show::Percent(percent));
                shows
            }
            Step::MapCleared => vec![Show::Page(wasnunform::page::INIT_MAP), Show::Percent(100)],
            Step::GameData => vec![Show::Page(wasnunform::page::INIT_GAME), Show::Percent(1)],
            Step::Tables => {
                self.tables();
                vec![Show::Percent(7)]
            }
            Step::Bitmaps => {
                if let Some((start, language)) = self.start.take() {
                    self.parts = Some(GameParts::new(
                        &self.dir,
                        &self.exe,
                        start,
                        self.high.take().unwrap_or_default(),
                        language,
                    ));
                }
                vec![Show::Percent(50)]
            }
            Step::Rest(r) => {
                if let Some(parts) = &mut self.parts {
                    parts.rest(r, &self.dir, &self.exe, &mut self.options, &self.sound, self.size);
                }
                let percent = REST.iter().find(|&&(s, _)| s == r).map_or(100, |&(_, p)| p);
                vec![Show::Percent(percent)]
            }
            Step::Assembled => {
                if let Some(parts) = self.parts.take() {
                    self.game = Some(self.assemble(parts));
                }
                vec![Show::Percent(100)]
            }
            Step::LoadMap => vec![Show::Page(wasnunform::page::LOAD_MAP), Show::Percent(100)],
        };
        Some((step, shows))
    }

    /// `load_tables`: the world's tables and its set-up, and what the game
    /// starts from (`start`).
    fn tables(&mut self) {
        let dir = self.dir.clone();
        // The set-up's text is in the player's language.
        let language = self.config.language.language(folder_land(&dir));
        let start = match &self.kind {
            Kind::Island => {
                let (Some(mut world), Some(data)) = (self.world.take(), self.data.take()) else {
                    return;
                };
                // States 1220 and 1500's world.
                hintergrund::set_up_finish(&mut world, &data, self.plots);
                generated_start(world, &self.options)
            }
            Kind::Scenario { path, scenario_page } => scenario_start(
                &dir,
                path,
                &self.settings,
                *scenario_page,
                &mut self.options,
                self.config.fixes,
                self.seed,
                language,
            ),
            Kind::Load(map) => {
                let data = hintergrund::GameData::load(|name| read_opt(&dir, name), language)
                    .unwrap_or_else(|e| panic!("{e}"));
                // The file's fixes; the player's for those it doesn't name
                // (all of them in an original file).
                let world = hintergrund::load_saved_game(map, &self.settings, &data, self.seed, self.config.fixes)
                    .unwrap_or_else(|e| panic!("the saved game: {e}"));
                let view = MapView::from_map(map);
                let camera = view.camera(&world);
                (world, camera, Some(camera.origin), view.unported)
            }
            // The host's world as it is: not a load, which would change
            // it (`World::load_snapshot`).
            Kind::Join(world, map) => {
                let view = MapView::from_map(map);
                let camera = view.camera(world);
                ((**world).clone(), camera, Some(camera.origin), view.unported)
            }
        };
        self.start = Some((start, self.config.language));
    }

    /// The game put together from `parts`, as the player set it: a
    /// scenario's build menu, a saved game's view people, the player's
    /// view enhancements.
    fn assemble(&mut self, parts: GameParts) -> Game {
        let mut game = parts.finish(&self.dir, &self.exe, self.options, &self.sound, self.size);
        match &self.kind {
            Kind::Scenario {
                scenario_page: true, ..
            } => game.tools.menu.start_scenario(game.world.scenario), // en 1018:015f
            Kind::Load(map) => {
                // With `fix.saved_state`, the world's records the view's
                // own people had in the file get back what they held.
                game.world.take_saved_free_people(&map.trailing);
            }
            _ => {}
        }
        game.set_view_enhancements(self.config.view);
        if !matches!(self.kind, Kind::Island) {
            game.start_from_file();
        }
        game
    }

    /// Runs the set-up to its end, and the game.
    #[cfg(test)]
    pub fn run(mut self) -> Game {
        while self.step().is_some() {}
        self.finish().expect("the set-up made a game")
    }

    /// The game, once the steps are done.
    pub fn finish(self) -> Option<Game> {
        self.game
    }
}

/// A game's parts while `load_game_rest` makes them, from the world it
/// starts with (`Start`).
pub(crate) struct GameParts {
    world: World,
    origin: Option<(i16, i16)>,
    view_rest: Fields,
    /// The player's language for every text of the view.
    language: lang::Language,
    font: Font,
    texts: formats::data::Sonstige,
    screen: Hauptschirm,
    tiles: Tiles,
    form: Form,
    info: Option<InfoTexts>,
    sabowin: Option<sabowin::Sabowin>,
    file_menu: Option<FileMenu>,
    auction: Option<(Auktionsform, Form)>,
    tools: Option<(Tools, Form)>,
    forms: FormParts,
}

impl GameParts {
    /// `load_game_bitmaps`' counterpart: the main screen of a game that
    /// starts from `start`, with its bitmaps and the island's tiles, the
    /// island drawn, laid out later. `language` is the player's setting.
    /// // 1020:2501
    pub fn new(
        dir: &Path,
        exe: &[u8],
        (mut world, camera, origin, view_rest): Start,
        high: ScoreTable,
        language: lang::Setting,
    ) -> Self {
        // Every text of the view is in the player's language.
        let language = language.language(folder_land(dir));
        let font = font(dir);
        let texts = upgrade::sonstige(dir, language);
        let frame = bitmaps(dir, &hauptschirm::BITMAP_FILES);
        let title = texts.texts.first().cloned().unwrap_or_default();
        let mut screen = match origin {
            Some(origin) => Hauptschirm::new(camera, origin, frame, &font, &title),
            None => Hauptschirm::new_game(camera, frame, &font, &title),
        };
        // With `fix.view_people` the file's spectators and the like are
        // the view's own.
        screen.people = game::people::view::ViewPeople::new(seed());
        let handed = world.split_off_view_people();
        screen.people.adopt(&handed);
        let tiles = load_tiles(dir);
        screen.paint_map(&world, &tiles);
        GameParts {
            world,
            origin,
            view_rest,
            language,
            font,
            texts,
            screen,
            tiles,
            form: load_form(exe, "THAUPTSCHIRM"),
            info: None,
            sabowin: None,
            file_menu: None,
            auction: None,
            tools: None,
            forms: FormParts::new(high),
        }
    }

    /// One of `load_game_rest`'s loaders, with the player's `options` and
    /// the `sound`, on a screen of `size`. // en 1038:33c8
    pub fn rest(
        &mut self,
        step: Rest,
        dir: &Path,
        exe: &[u8],
        options: &mut Options,
        sound: &sound::Shared,
        size: (i32, i32),
    ) {
        let language = self.language;
        match step {
            Rest::Dates => {
                let info = InfoTexts {
                    months: monate(&lang_text(language, "TEXTE/MONATE.TXT")).expect("MONATE.TXT"),
                    language,
                    currency_factor: self.texts.waehrungsfaktor,
                    currency: self.texts.texts.get(5).cloned().unwrap_or_default(), // 1020:3ea6
                };
                self.screen.show_info(&self.world, &self.font, &info);
                self.info = Some(info);
            }
            Rest::Sabotage => {
                let info = self.info();
                self.sabowin = Some(Forms::sabowin(dir, &self.font, &info));
            }
            Rest::FileMenu => self.file_menu = Some(file_menu(dir, exe, language)),
            Rest::Auction => {
                let auction_bitmaps = auktionsform::BITMAP_FILES
                    .iter()
                    .map(|f| Surface::from_bitmap(&bitmap(dir, &format!("GRAFIK/{}", f.to_ascii_uppercase()))))
                    .collect();
                let auction = Auktionsform::new(auction_bitmaps, auktion(&lang_text(language, "TEXTE/AUKTION.TXT")));
                let mut auction_form = load_form(exe, "TAUKTIONSFORM");
                for (name, bounds) in auktionsform::LAYOUT {
                    if let Some(c) = auction_form.component_mut(name) {
                        c.bounds = bounds;
                        c.visible = true;
                    }
                }
                self.auction = Some((auction, auction_form));
            }
            Rest::BuildMenu => {
                // 1030:36e5 (1018:2fa2) and `game_start_setup` (1010:2820):
                // the build menu, and the tools from the map.
                let info = self.info();
                let names = hausname(&lang_text(language, "TEXTE/HAUSNAME.TXT")).expect("HAUSNAME.TXT");
                let menu = Baumenue::new(
                    baumenu_txt(&lang_text(language, "TEXTE/BAUMENU.TXT")).expect("BAUMENU.TXT"),
                    &baumenu_dat(&lang_text(language, "DATA/BAUMENU.DAT")).expect("BAUMENU.DAT"),
                    names,
                    auswahl(&lang_text(language, "TEXTE/AUSWAHL.TXT")),
                    bitmaps(dir, &baumenue::BITMAP_FILES),
                    size,
                );
                let mut tools = Tools::new(
                    menu,
                    fehler(&lang_text(language, "TEXTE/FEHLER.TXT")).expect("FEHLER.TXT"),
                );
                tools.sabowin = Some(match self.sabowin.take() {
                    Some(s) => s,
                    None => Forms::sabowin(dir, &self.font, &info),
                });
                let view = self.screen.camera().view;
                tools.start(&self.view_rest, &self.world, &self.font, &self.tiles, &info, view);
                self.screen.draw_buttons(&tools.down);
                tools.buttons_changed = false;
                let mut menu_form = load_form(exe, "TBAUMENUE");
                menu_form.components[0].visible = false;
                self.tools = Some((tools, menu_form));
            }
            Rest::Music => {
                let mut s = sound.borrow_mut();
                s.music.init_playlist(options); // 1018:37de, from 1030:36e5
                // A scenario's or a saved game's `game_start_setup`
                // (1010:2820).
                if self.origin.is_some() {
                    s.game_start(options);
                }
            }
            Rest::Form(f) => {
                let info = self.info();
                self.forms.make(f, dir, &self.font, &info, &self.screen, size);
            }
        }
    }

    /// The info bar's texts, made by [`Rest::Dates`].
    fn info(&self) -> InfoTexts {
        self.info.clone().expect("the dates' texts come first")
    }

    /// The game: the main screen laid out, the rest of the forms, and
    /// what the map holds of them. // 1020:2501, 1010:2820
    ///
    /// # Panics
    /// If a loader of [`REST`] didn't run.
    pub fn finish(self, dir: &Path, exe: &[u8], options: Options, sound: &sound::Shared, size: (i32, i32)) -> Game {
        let info = self.info();
        let GameParts {
            world,
            origin,
            view_rest,
            font,
            mut screen,
            tiles,
            mut form,
            file_menu,
            auction,
            tools,
            forms,
            ..
        } = self;
        // 1020:1ce9 and 1020:283b: centred, laid out and shown.
        screen.layout(size);
        form.components[0].bounds = screen.bounds();
        form.components[0].visible = true;
        for (name, bounds) in hauptschirm::LAYOUT {
            if let Some(c) = form.component_mut(name) {
                c.bounds = bounds;
                c.visible = true;
            }
        }
        let (auction, auction_form) = auction.expect("the auction form is made");
        let (tools, menu_form) = tools.expect("the build menu is made");
        // The other forms, and what the map holds of them.
        let mut forms = Forms::assemble(forms, dir, exe, &font, &info, options, size);
        let mut commands = Vec::new();
        // A scenario's or a saved game's forms get the file's windows
        // and fields once the view's enhancements are set
        // (`Game::start_from_file`). A generated island's overview map
        // keeps what its constructor (10b8:4538) set, no button down and
        // the land by heights, where the missing fields' zeros would put
        // button 0 down and draw a picture over the menu's left scroll
        // arrow.
        if origin.is_none() {
            forms.overview.game_start(&world);
            forms.tv.load_fields(&view_rest);
            let closed = forms.tv.close();
            sound.borrow_mut().tv_media(&closed.media, &forms.settings);
            commands = closed
                .actions
                .into_iter()
                .map(|action| Command { player: 1, action })
                .collect();
            forms.money.load_fields(&view_rest);
            let _ = forms.money.loaded(&world);
        }
        forms.compass.draw_knob(&mut screen.corner);
        Game {
            lights_random: Random::new(seed()),
            seat: game::seats::HOST_SEAT,
            net: None,
            net_over: None,
            forms,
            // The caller sets the player's (`Setup::assemble`).
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
            file_menu: file_menu.expect("the file menu is made"),
            view_rest,
            sound: Rc::clone(sound),
            size,
            desktop: size,
        }
    }
}
