//! The game's forms besides the main screen, the build menu, the auction
//! form and the file menu: the pause (button 1), the options (2), the TV
//! (3), the money menu (4), the overview map and the compass (5), the
//! sabotage window (21), the pointer's info box and the buildings'
//! overview (22), the quit question, the end of the game, the high scores,
//! the credits, the port's list of enhancements and the tooltips. Each is a `game` view type and its VCL form from the
//! DFM; this module loads them, routes the mouse and the keys to them,
//! hands their commands to the world as the human's (player 1's) and
//! paints them.

// Short names for the forms and the mouse, as in the forms' own modules.
#![allow(clippy::many_single_char_names)]

use std::path::{Path, PathBuf};

use formats::bmp::Bitmap;
use formats::data::{
    geldmenu_dat, geldmenu_txt, hausname, infobox_dat, infobox_txt, message_media_names, modatyp, modatyp_txt,
    nachrich, sabotage, sabotext, text_lines, uebermap_dat, uebermap_txt,
};
use formats::highscore::HighScores;
use game::auktionsform;
use game::command::{self, Command};
use game::creditform::{self, Creditform};
use game::endeform::{self, Endeform};
use game::enhanceform::{self, Enhanceform};
use game::fernseher::{self, Fernseher};
use game::geldmenu::{self, Geldmenuform};
use game::hauptschirm::{self, Hauptschirm};
use game::helpform::{self, Helpform};
use game::highscoreform::{self, Highscoreform, ScoreTable};
use game::infobox::{self, Infobox};
use game::kompassform::{self, Kompassform};
use game::optionen::{self, Optionenform, Settings};
use game::pauseform::{self, Pauseform};
use game::sabowin::{self, Sabowin};
use game::savegame::Fields;
use game::schlussform::{self, Results, Schlussform};
use game::simulation::Tick;
use game::text::Font;
use game::uebersichtsmap::{self, Image, Uebersichtsmap};
use game::umenuform::{self, Redraw, Umenuform};
use game::widgets::ShiftState;
use platform::{Button, Input};
use vcl::form::{Form, Kind};
use vcl::surface::Surface;
use vcl::{Palette, Rect};

use super::{Game, dateimenue, load_form, read};
use crate::stacking::Win;

/// The button bar's buttons whose forms this module opens. // 1010:2c8b
const PAUSE_BUTTON: u8 = 1;
const OPTIONS_BUTTON: u8 = 2;
const TV_BUTTON: u8 = 3;
const MONEY_BUTTON: u8 = 4;
const OVERVIEW_BUTTON: u8 = 5;
/// How often the overview map's scroll timers fire while an arrow is
/// held (`Interval` 25 ms).
const SCROLL_TIMER_MS: u32 = 25;
/// How often `TSchlussform.Timer1` fires (`Interval` 1000 ms).
const TIMER1_MS: u32 = 1000;

/// The form that has the keyboard, besides the modal ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Focus {
    Main,
    Tv,
    Money,
    Overview,
    Infobox,
    Umenu,
}

/// The form a press went down on, which keeps the mouse until every
/// button is up again (the VCL's capture).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Grab {
    Pause,
    Options,
    Tv,
    Money,
    Overview(Image),
    Compass,
    Sabowin,
    Infobox,
    Umenu,
    Ende,
    Schluss,
    High,
    Credits,
    Enhance,
}

/// What the caller (`Holiday`) does after an event, because it needs more
/// than the game: the saves, the start menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Request {
    /// Open the file menu (button 0).
    FileMenu,
    /// The quit question's "OK": save `Last.map`, then the end film
    /// (1020:3a1b, 1068:0f52).
    Quit,
    /// The game exits (1068:17ad).
    Exit,
    /// "Neues Spiel" on the end form: back to the start menu (1010:25dc).
    NewGame,
    /// Write the high scores (`HIGHSCOR.WAV`).
    SaveHighScores,
    /// The player switched an enhancement in the list of them: the
    /// player's settings change, are applied and saved.
    Enhance(enhanceform::Change),
}

/// The forms and their VCL forms.
pub(crate) struct Forms {
    pub settings: Settings,
    pub pause: Pauseform,
    pause_form: Form,
    pub options: Optionenform,
    options_form: Form,
    pub tv: Fernseher,
    tv_form: Form,
    pub money: Geldmenuform,
    money_form: Form,
    pub overview: Uebersichtsmap,
    overview_form: Form,
    pub compass: Kompassform,
    compass_form: Form,
    sabowin_form: Form,
    pub infobox: Infobox,
    infobox_form: Form,
    pub umenu: Umenuform,
    umenu_form: Form,
    pub ende: Endeform,
    ende_form: Form,
    pub schluss: Schlussform,
    schluss_form: Form,
    pub high: Highscoreform,
    high_form: Form,
    pub credits: Creditform,
    credits_form: Form,
    pub enhance: Enhanceform,
    /// What the list of enhancements says about saving (`Holiday`'s).
    pub enhance_notes: enhanceform::Notes,
    pub(crate) help: Helpform,
    help_form: Form,
    pub focus: Focus,
    pub grab: Option<Grab>,
    /// The caller's clock in ms, for the TV (`GetTickCount`).
    pub now: u32,
    /// Time since the overview map's scroll timers last fired.
    since_scroll: u32,
    /// The ms since `TSchlussform.Timer1` last fired.
    timer1_ms: u32,
    /// The money menu's page needs the world again once the commands
    /// are applied ([`geldmenu::Response::update`]).
    money_update: bool,
    /// The info box draws itself again once its command is applied.
    infobox_redraw: bool,
    /// The buildings' overview draws this again once its command is
    /// applied.
    umenu_redraw: Option<Redraw>,
    /// The settings of a file the file menu is loading, for when it's
    /// loaded.
    pub loading_settings: Option<Settings>,
    /// The last mouse x on `ButtonImage` or `Hauptbild`, for the tooltips
    /// (`1110:7044`).
    bar_x: i32,
    pub requests: Vec<Request>,
}

/// `name` under `dir`, whatever the case of its letters.
pub(crate) fn find(dir: &Path, name: &str) -> Option<PathBuf> {
    let mut path = dir.to_path_buf();
    for part in name.split('/') {
        let found = std::fs::read_dir(&path)
            .ok()?
            .filter_map(Result::ok)
            .find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(part))?;
        path = found.path();
    }
    Some(path)
}

/// The bitmap `GRAFIK/name`, if it's there.
fn picture(dir: &Path, name: &str) -> Option<Surface> {
    let path = find(dir, &format!("GRAFIK/{name}"))?;
    let bitmap = Bitmap::parse(&std::fs::read(path).ok()?).ok()?;
    Some(Surface::from_bitmap(&bitmap))
}

fn bitmaps(dir: &Path, files: &[&str]) -> Vec<Surface> {
    files
        .iter()
        .map(|f| picture(dir, f).unwrap_or_else(|| panic!("GRAFIK/{f} not found")))
        .collect()
}

/// A data or text file the forms read, whatever the case of its name.
fn text(dir: &Path, name: &str) -> Vec<u8> {
    find(dir, name).map_or_else(|| read(dir, name), |p| std::fs::read(&p).unwrap_or_default())
}

/// The high scores: the player's file, else the CD's; with the later
/// build's `UPGRADE\` folder also its scenarios' table, the player's
/// `HIGHSCO2.WAV` next to `own`, else `UPGRADE\HIGHSCO2.WAV`.
/// // 1040:1ae0, en 1068:1adf
pub(crate) fn high_scores(dir: &Path, own: &Path) -> ScoreTable {
    let first = std::fs::read(own)
        .ok()
        .and_then(|d| HighScores::parse(&d))
        .or_else(|| {
            ["SOUND/HIGHSCOR.WAV", "SOUNDLO/HIGHSCOR.WAV"]
                .iter()
                .find_map(|n| HighScores::parse(&std::fs::read(find(dir, n)?).ok()?))
        })
        .expect("HIGHSCOR.WAV");
    let second = crate::upgrade::present(dir)
        .then(|| {
            std::fs::read(second_high_scores(own))
                .ok()
                .and_then(|d| HighScores::parse(&d))
                .or_else(|| HighScores::parse(&std::fs::read(find(dir, "UPGRADE/HIGHSCO2.WAV")?).ok()?))
        })
        .flatten();
    ScoreTable::new(&first, second.as_ref())
}

/// The player's `HIGHSCO2.WAV`, next to their `HIGHSCOR.WAV`.
pub(crate) fn second_high_scores(own: &Path) -> std::path::PathBuf {
    own.with_file_name("HIGHSCO2.WAV")
}

/// Puts `images` into `form` at `bounds` and paints it, if `visible`.
fn paint_form(
    form: &mut Form,
    bounds: Rect,
    visible: bool,
    images: &[(&str, Rect, Option<&Surface>)],
    screen: &mut Surface,
    palette: &Palette,
) {
    form.components[0].bounds = bounds;
    form.components[0].visible = visible;
    if !visible {
        return;
    }
    for &(name, rect, picture) in images {
        if let Some(c) = form.component_mut(name) {
            c.bounds = rect;
            c.visible = picture.is_some();
            if let Kind::Image { picture: p } = &mut c.kind {
                *p = picture.cloned();
            }
        }
    }
    form.paint(screen, palette);
}

/// The mouse event's position and whether the left button changed.
fn position(input: Input) -> Option<(i32, i32)> {
    match input {
        Input::MouseMove { x, y } | Input::MouseDown { x, y, .. } | Input::MouseUp { x, y, .. } => Some((x, y)),
        _ => None,
    }
}

fn left(input: Input) -> bool {
    matches!(
        input,
        Input::MouseDown {
            button: Button::Left,
            ..
        } | Input::MouseUp {
            button: Button::Left,
            ..
        }
    )
}

/// Today's date (the system's, as the original's `GetDate`), for the
/// high scores: day, month, year.
fn today() -> (u32, u32, i32) {
    let days = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() / 86_400);
    // Howard Hinnant's `civil_from_days`.
    let z = i64::try_from(days).unwrap_or(0) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (
        u32::try_from(day).unwrap_or(1),
        u32::try_from(month).unwrap_or(1),
        i32::try_from(year).unwrap_or(1996),
    )
}

/// The forms the game's set-up makes one at a time, each a step of the
/// progress window's gauge, in `load_game_rest`'s order (en 1038:33c8):
/// the TV (`init_messages`, 0x48 %), the overview map (0x50), the info
/// box and the buildings' overview (0x52), the money menu (0x55), the
/// tooltip (0x62) and the high scores (0x63); then the rest, which the
/// original makes when it starts (the pause, the options, the compass,
/// the end forms, the credits and the port's list of enhancements).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FormStep {
    Tv,
    Overview,
    Infobox,
    Money,
    Help,
    HighScores,
}

/// The forms made so far ([`FormStep`]), until [`Forms::assemble`] makes
/// the rest.
pub(crate) struct FormParts {
    high_scores: Option<ScoreTable>,
    tv: Option<Fernseher>,
    overview: Option<Uebersichtsmap>,
    infobox: Option<Infobox>,
    umenu: Option<Umenuform>,
    money: Option<Geldmenuform>,
    help: Option<Helpform>,
    high: Option<Highscoreform>,
}

impl FormParts {
    /// None made yet; `high_scores` for the high scores' form.
    pub fn new(high_scores: ScoreTable) -> FormParts {
        FormParts {
            high_scores: Some(high_scores),
            tv: None,
            overview: None,
            infobox: None,
            umenu: None,
            money: None,
            help: None,
            high: None,
        }
    }

    /// Makes the forms of `step`, on the main screen `screen` (for the
    /// overview's scroll position and view).
    #[allow(clippy::too_many_lines)] // One form after the other.
    pub fn make(
        &mut self,
        step: FormStep,
        dir: &Path,
        font: &Font,
        info: &hauptschirm::InfoTexts,
        screen: &Hauptschirm,
        size: (i32, i32),
    ) {
        // The view's texts are in the player's language.
        let language = info.language;
        let texts = crate::upgrade::sonstige(dir, language);
        match step {
            FormStep::Tv => {
                // 1048:1ffd: the messages' pictures, by `FILENAME.DAT`.
                let pictures = message_media_names(&text(dir, "DATA/FILENAME.DAT"))
                    .iter()
                    .map(|name| picture(dir, &auktionsform::picture_file(name)))
                    .collect();
                let mut tv = Fernseher::new(
                    font.clone(),
                    bitmaps(dir, &fernseher::BITMAP_FILES),
                    pictures,
                    nachrich(&crate::lang_text(language, "TEXTE/NACHRICH.TXT")),
                    message_media_names(&text(dir, "DATA/FILMNAME.DAT")),
                    size,
                );
                tv.set_speech(message_media_names(&text(dir, "DATA/SOUNNAME.DAT")));
                self.tv = Some(tv);
            }
            FormStep::Overview => {
                let modatyp = modatyp(&text(dir, "DATA/MODATYP.DAT")).expect("MODATYP.DAT");
                // 10b8:4e9e, 1038:3557
                let (wo, ns) = screen.scroll_position();
                self.overview = Some(Uebersichtsmap::new(
                    font.clone(),
                    bitmaps(dir, &uebersichtsmap::BITMAP_FILES),
                    &uebermap_dat(&text(dir, "DATA/UEBERMAP.DAT")).expect("UEBERMAP.DAT"),
                    &uebermap_txt(&crate::lang_text(language, "TEXTE/UEBERMAP.TXT")).expect("UEBERMAP.TXT"),
                    texts.texts.get(13).map_or(&[][..], |t| &t[..]),
                    modatyp.max_grueneinfluss,
                    (wo, ns, screen.camera().view),
                ));
            }
            FormStep::Infobox => {
                let (short_names, long_names) =
                    hausname(&crate::lang_text(language, "TEXTE/HAUSNAME.TXT")).expect("HAUSNAME.TXT");
                // 1038:3c2b, 1080:0ea8
                let ib_texts = infobox_txt(&crate::lang_text(language, "TEXTE/INFOBOX.TXT")).expect("INFOBOX.TXT");
                let short_texts = ib_texts.short_texts.clone();
                self.infobox = Some(Infobox::new(
                    infobox_dat(&crate::lang_text(language, "DATA/INFOBOX.DAT")).expect("INFOBOX.DAT"),
                    ib_texts,
                    long_names,
                    texts.texts.clone(),
                    bitmaps(dir, &infobox::BITMAP_FILES),
                ));
                self.umenu = Some(Umenuform::new(
                    bitmaps(dir, &umenuform::BITMAP_FILES),
                    text_lines(&crate::lang_text(language, "TEXTE/UMENU.TXT")),
                    short_texts,
                    short_names,
                ));
            }
            FormStep::Money => {
                // 1070:2e6c
                let (geldmenu_texts, close) =
                    geldmenu_txt(&crate::lang_text(language, "TEXTE/GELDMENU.TXT")).expect("GELDMENU.TXT");
                let (advertising, renovation) = modatyp_txt(&crate::lang_text(language, "TEXTE/MODATYP.TXT"));
                self.money = Some(Geldmenuform::new(
                    font.clone(),
                    bitmaps(dir, &geldmenu::BITMAP_FILES),
                    geldmenu_dat(&crate::lang_text(language, "DATA/GELDMENU.DAT")).expect("GELDMENU.DAT"),
                    geldmenu::Texts {
                        geldmenu: geldmenu_texts,
                        close,
                        sonstige: texts.texts.clone(),
                        short: texts.short_texts.clone(),
                        advertising,
                        renovation,
                        info: info.clone(),
                    },
                    size.0,
                ));
            }
            FormStep::Help => {
                self.help = Some(Helpform::new(
                    font.clone(),
                    picture(dir, helpform::BITMAP_FILE).expect("Help.bmp"),
                    texts.short_texts.clone(),
                ));
            }
            FormStep::HighScores => {
                let mut high_bitmaps = bitmaps(dir, &highscoreform::BITMAP_FILES);
                let upgrade = crate::upgrade::present(dir);
                if upgrade {
                    high_bitmaps.extend(crate::upgrade::bitmaps(dir, &highscoreform::UPGRADE_BITMAP_FILES));
                }
                let mut high = Highscoreform::new(
                    font.clone(),
                    high_bitmaps,
                    highscoreform::highscor_lines(&crate::lang_text(language, "TEXTE/HIGHSCOR.TXT")),
                    self.high_scores.take().unwrap_or_default(),
                );
                if upgrade {
                    // en 1068:2493: `UPGRADE\HIGHSCO2.TXT`'s six names.
                    let names = text_lines(&crate::upgrade::file(
                        dir,
                        language,
                        "HIGHSCO2.TXT",
                        "TEXTE/HIGHSCOR.TXT",
                    ));
                    high = high.with_upgrade(names.into_iter().take(6).collect());
                }
                self.high = Some(high);
            }
        }
    }
}

impl Forms {
    /// The forms of a game, as the game's set-up creates them: those of
    /// `parts` and the rest, which the original makes when it starts: the pause, the options, the compass, the end forms, the
    /// credits and the port's list of enhancements; and the VCL forms.
    ///
    /// # Panics
    /// If a step of `parts` wasn't made.
    #[allow(clippy::too_many_lines, clippy::too_many_arguments)] // One form after the other.
    pub fn assemble(
        mut parts: FormParts,
        dir: &Path,
        exe: &[u8],
        font: &Font,
        info: &hauptschirm::InfoTexts,
        settings: Settings,
        size: (i32, i32),
    ) -> Forms {
        let language = info.language;
        let texts = crate::upgrade::sonstige(dir, language);
        // 1040:3a45, 1060:0789
        let pause = Pauseform::new(bitmaps(dir, &pauseform::BITMAP_FILES));
        let options = Optionenform::new(
            font.clone(),
            bitmaps(dir, &optionen::BITMAP_FILES),
            text_lines(&crate::lang_text(language, "TEXTE/OPTIONEN.TXT")),
        );
        let compass = Kompassform::new(font.clone(), bitmaps(dir, &kompassform::BITMAP_FILES), size.0);
        let ende = Endeform::new(font.clone(), bitmaps(dir, &endeform::BITMAP_FILES), texts.texts.clone());
        let schluss = Schlussform::new(
            font.clone(),
            bitmaps(dir, &schlussform::BITMAP_FILES),
            if crate::upgrade::present(dir) {
                // en 1070:0294
                schlussform::schluss_lines_upgrade(&crate::upgrade::file(
                    dir,
                    language,
                    "SCHLUSS.TXT",
                    "TEXTE/SCHLUSS.TXT",
                ))
            } else {
                schlussform::schluss_lines(&crate::lang_text(language, "TEXTE/SCHLUSS.TXT"))
            },
        );
        // 1030:3c42: "Schließen", `SONSTIGE.TXT`'s 14th text.
        let schliessen = texts.texts.get(13).cloned().unwrap_or_default();
        let mut credit_bitmaps = bitmaps(dir, &creditform::BITMAP_FILES);
        if crate::upgrade::present(dir) {
            // en 1038:3925: the picture from `UPGRADE\`.
            if let Some(picture) = crate::upgrade::bitmaps(dir, &["CREDITS.BMP"])
                .pop()
                .filter(|p| p.width > 0)
            {
                credit_bitmaps[0] = picture;
            }
        }
        let credits = Creditform::new(font.clone(), credit_bitmaps, &schliessen);
        let enhance = Enhanceform::new(
            font.clone(),
            bitmaps(dir, &enhanceform::BITMAP_FILES),
            &schliessen,
            info.language,
        );
        // The quit question and the end form are where their DFMs put
        // them until they're first shown, which a map file keeps of them
        // (1010:0799).
        let ende_form = load_form(exe, "TENDEFORM");
        let schluss_form = load_form(exe, "TSCHLUSSFORM");
        let mut ende = ende;
        let mut schluss = schluss;
        let at = |f: &Form| f.components.first().map_or((0, 0), |c| (c.bounds.left, c.bounds.top));
        (ende.left, ende.top) = at(&ende_form);
        (schluss.left, schluss.top) = at(&schluss_form);
        Forms {
            settings,
            pause,
            pause_form: load_form(exe, "TPAUSEFORM"),
            options,
            options_form: load_form(exe, "TOPTIONENFORM"),
            tv: parts.tv.take().expect("the TV is made"),
            tv_form: load_form(exe, "TFERNSEHER"),
            money: parts.money.take().expect("the money menu is made"),
            money_form: load_form(exe, "TGELDMENUFORM"),
            overview: parts.overview.take().expect("the overview map is made"),
            overview_form: load_form(exe, "TUEBERSICHTSMAP2"),
            compass,
            compass_form: load_form(exe, "TKOMPASSFORM"),
            sabowin_form: load_form(exe, "TSABOWIN"),
            infobox: parts.infobox.take().expect("the info box is made"),
            infobox_form: load_form(exe, "TINFOBOX2"),
            umenu: parts.umenu.take().expect("the buildings' overview is made"),
            umenu_form: load_form(exe, "TUMENUFORM"),
            ende,
            ende_form,
            schluss,
            schluss_form,
            high: parts.high.take().expect("the high scores are made"),
            high_form: load_form(exe, "THIGHSCOREFORM"),
            credits,
            credits_form: load_form(exe, "TCREDITFORM"),
            enhance,
            enhance_notes: enhanceform::Notes::default(),
            help: parts.help.take().expect("the tooltip is made"),
            help_form: load_form(exe, "THELPFORM"),
            focus: Focus::Main,
            grab: None,
            now: 0,
            since_scroll: 0,
            timer1_ms: 0,
            money_update: false,
            infobox_redraw: false,
            umenu_redraw: None,
            loading_settings: None,
            bar_x: 0,
            requests: Vec::new(),
        }
    }

    /// The sabotage window (`TSabowin`, 1028:361f), which the tools keep.
    pub fn sabowin(dir: &Path, font: &Font, info: &hauptschirm::InfoTexts) -> Sabowin {
        let language = info.language;
        let (labels, kinds) = sabotage(&crate::lang_text(language, "TEXTE/SABOTAGE.TXT")).expect("SABOTAGE.TXT");
        let pictures = sabowin::picture_files(&kinds)
            .iter()
            .map(|f| picture(dir, f).unwrap_or_else(|| panic!("GRAFIK/{f} not found")))
            .collect();
        let texts = sabowin::Texts {
            sabotext: sabotext(&crate::lang_text(language, "TEXTE/SABOTEXT.TXT")),
            labels,
            kinds,
            info: info.clone(),
        };
        Sabowin::new(font.clone(), bitmaps(dir, &sabowin::BITMAP_FILES), pictures, texts)
    }

    /// A form is up that stops the clock (1020:24a0): the pause, the
    /// options, the quit question, the end form and the high scores (and
    /// the credits and the list of enhancements, which only open over the
    /// options). Single human: in the original this stops the game for
    /// everyone; the view just doesn't tick meanwhile.
    pub fn clock_stopped(&self) -> bool {
        self.pause.visible
            || self.options.visible
            || self.ende.visible
            || self.schluss.visible()
            || self.high.visible
            || self.credits.visible
            || self.enhance.visible
    }

    /// A form is up that disables the others (`disable_forms`, 1010:2970):
    /// those that stop the clock, and the info box with buttons.
    pub fn modal(&self) -> bool {
        self.clock_stopped() || (self.infobox.visible && self.infobox.modal)
    }

    /// Moves each form with `follow` (`Game::resize`), which gets its
    /// bounds and sets its top left.
    pub fn follow(&mut self, follow: impl Fn(Rect, &mut i32, &mut i32)) {
        let p = &mut self.pause;
        follow(p.bounds(), &mut p.left, &mut p.top);
        let o = &mut self.options;
        follow(o.bounds(), &mut o.left, &mut o.top);
        let t = &mut self.tv;
        follow(t.bounds(), &mut t.left, &mut t.top);
        // Where it shows next moves along.
        let (mut kl, mut kt) = t.kept;
        follow(Rect::new(kl, kt, t.bounds().width(), t.height), &mut kl, &mut kt);
        t.kept = (kl, kt);
        let m = &mut self.money;
        follow(m.bounds(), &mut m.left, &mut m.top);
        let o = &mut self.overview;
        follow(o.bounds(), &mut o.left, &mut o.top);
        let c = &mut self.compass;
        follow(c.bounds(), &mut c.left, &mut c.top);
        let b = &mut self.infobox;
        follow(b.bounds(), &mut b.left, &mut b.top);
        let u = &mut self.umenu;
        follow(u.bounds(), &mut u.left, &mut u.top);
        let e = &mut self.ende;
        follow(e.bounds(), &mut e.left, &mut e.top);
        let s = &mut self.schluss;
        follow(s.bounds(), &mut s.left, &mut s.top);
        let h = &mut self.high;
        follow(h.bounds(), &mut h.left, &mut h.top);
        let c = &mut self.credits;
        follow(c.bounds(), &mut c.left, &mut c.top);
        let e = &mut self.enhance;
        follow(e.bounds(), &mut e.left, &mut e.top);
        let h = &mut self.help;
        follow(h.bounds(), &mut h.left, &mut h.top);
    }

    /// `big_map`: the forms the original puts at the screen's top edge
    /// (the money menu, 1070:2e6c; the compass, 1038:2c04; the options,
    /// 1060:14f1; the overview map, 10b8:4538) go just below `bars` when
    /// one of them opens over them, so the main screen's title, info and
    /// button bars, which are at the top of the screen when it fills it,
    /// stay free. Kept on a screen `size` high.
    pub fn below_bars(&mut self, w: Win, bars: i32, size: (i32, i32)) {
        let down = |r: Rect, top: &mut i32| {
            if r.top < bars {
                *top = bars.min(size.1 - r.height()).max(0);
            }
        };
        match w {
            Win::Money => down(self.money.bounds(), &mut self.money.top),
            Win::Compass => down(self.compass.bounds(), &mut self.compass.top),
            Win::Options => down(self.options.bounds(), &mut self.options.top),
            Win::Overview => down(self.overview.bounds(), &mut self.overview.top),
            _ => {}
        }
    }

    /// Paints the form `w` over the screen, if it's one of this
    /// module's and shown; the caller goes through them in the order
    /// they lie ([`crate::stacking::Stack::order`]).
    #[allow(clippy::too_many_lines)] // One form after the other.
    pub fn paint_window(&mut self, w: Win, tools_sabowin: Option<&Sabowin>, screen: &mut Surface, palette: &Palette) {
        match w {
            Win::Money => {
                let m = &self.money;
                let layout = m.layout();
                let images: Vec<_> = m
                    .images()
                    .iter()
                    .zip(layout)
                    .map(|(&(name, s), (_, r))| (name, r, Some(s)))
                    .collect();
                paint_form(&mut self.money_form, m.bounds(), m.shown(), &images, screen, palette);
            }
            Win::Overview => {
                let o = &self.overview;
                let images: Vec<_> = o.images().iter().map(|&(i, r, s)| (i.name(), r, Some(s))).collect();
                paint_form(
                    &mut self.overview_form,
                    o.bounds(),
                    o.visible(),
                    &images,
                    screen,
                    palette,
                );
            }
            Win::Sabowin => {
                if let Some(s) = tools_sabowin {
                    let images = [(
                        "Saboimage",
                        Rect::new(0, 0, sabowin::WIDTH, sabowin::HEIGHT),
                        Some(s.image()),
                    )];
                    paint_form(&mut self.sabowin_form, s.bounds(), s.visible, &images, screen, palette);
                }
            }
            Win::Umenu => {
                let u = &self.umenu;
                let images: Vec<_> = u.images().iter().map(|&(n, r, s)| (n, r, Some(s))).collect();
                paint_form(&mut self.umenu_form, u.bounds(), u.visible, &images, screen, palette);
            }
            Win::Infobox => {
                let b = &self.infobox;
                let images: Vec<_> = b.images().iter().map(|&(n, r, s)| (n, r, Some(s))).collect();
                paint_form(&mut self.infobox_form, b.bounds(), b.visible, &images, screen, palette);
            }
            Win::Tv => {
                let t = &self.tv;
                let images: Vec<_> = t
                    .images()
                    .iter()
                    .zip(fernseher::LAYOUT)
                    .map(|(&(n, s), (_, r))| (n, r, Some(s)))
                    .collect();
                paint_form(&mut self.tv_form, t.bounds(), t.visible, &images, screen, palette);
            }
            Win::Compass => {
                let k = &self.compass;
                let images = [(
                    "Bild",
                    Rect::new(0, 0, kompassform::WIDTH, kompassform::HEIGHT),
                    Some(&k.image),
                )];
                paint_form(
                    &mut self.compass_form,
                    k.bounds(),
                    k.visible(),
                    &images,
                    screen,
                    palette,
                );
            }
            Win::Pause => {
                let p = &self.pause;
                let images = [(pauseform::LAYOUT[0].0, pauseform::LAYOUT[0].1, Some(p.image()))];
                paint_form(&mut self.pause_form, p.bounds(), p.visible, &images, screen, palette);
            }
            Win::Options => {
                let o = &self.options;
                let images = [(optionen::LAYOUT[0].0, optionen::LAYOUT[0].1, Some(o.image()))];
                paint_form(&mut self.options_form, o.bounds(), o.visible, &images, screen, palette);
            }
            Win::Ende => {
                let e = &self.ende;
                let images = [(
                    endeform::IMAGE,
                    Rect::new(0, 0, endeform::WIDTH, endeform::HEIGHT),
                    Some(e.image()),
                )];
                paint_form(&mut self.ende_form, e.bounds(), e.visible, &images, screen, palette);
            }
            Win::Schluss => {
                let s = &self.schluss;
                let images = s.components();
                paint_form(
                    &mut self.schluss_form,
                    s.bounds(),
                    s.visible(),
                    &images,
                    screen,
                    palette,
                );
            }
            Win::High => {
                let h = &self.high;
                let images = [(
                    highscoreform::IMAGE,
                    Rect::new(0, 0, highscoreform::WIDTH, highscoreform::HEIGHT),
                    Some(h.image()),
                )];
                paint_form(&mut self.high_form, h.bounds(), h.visible, &images, screen, palette);
            }
            Win::Help => {
                let h = &self.help;
                let images = [(
                    helpform::IMAGE,
                    Rect::new(0, 0, helpform::WIDTH, helpform::HEIGHT),
                    Some(h.image()),
                )];
                paint_form(&mut self.help_form, h.bounds(), h.visible(), &images, screen, palette);
            }
            Win::Credits => {
                let c = &self.credits;
                let images = [(
                    creditform::IMAGE,
                    Rect::new(0, 0, creditform::WIDTH, creditform::HEIGHT),
                    Some(c.image()),
                )];
                paint_form(&mut self.credits_form, c.bounds(), c.visible, &images, screen, palette);
            }
            Win::Enhance => {
                // The port's form has no DFM: its image is the form.
                let e = &self.enhance;
                if e.visible {
                    let clip = screen.bounds();
                    screen.draw(e.left, e.top, e.image(), clip);
                }
            }
            Win::Background | Win::Main | Win::Menu | Win::Auction | Win::FileMenu => {}
        }
    }

    /// The tooltip is shown.
    pub fn help_visible(&self) -> bool {
        self.help.visible()
    }

    /// The form's bounds on the screen while the grab's form is shown.
    fn grab_bounds(&self, g: Grab, sabowin: Option<&Sabowin>) -> Option<Rect> {
        let shown = |v: bool, r: Rect| v.then_some(r);
        match g {
            Grab::Pause => shown(self.pause.visible, self.pause.bounds()),
            Grab::Options => shown(self.options.visible, self.options.bounds()),
            Grab::Tv => shown(self.tv.visible, self.tv.bounds()),
            Grab::Money => shown(self.money.shown(), self.money.bounds()),
            Grab::Overview(_) => shown(self.overview.visible(), self.overview.bounds()),
            Grab::Compass => shown(self.compass.visible(), self.compass.bounds()),
            Grab::Sabowin => sabowin.and_then(|s| shown(s.visible, s.bounds())),
            Grab::Infobox => shown(self.infobox.visible, self.infobox.bounds()),
            Grab::Umenu => shown(self.umenu.visible, self.umenu.bounds()),
            Grab::Ende => shown(self.ende.visible, self.ende.bounds()),
            Grab::Schluss => shown(self.schluss.visible(), self.schluss.bounds()),
            Grab::High => shown(self.high.visible, self.high.bounds()),
            Grab::Credits => shown(self.credits.visible, self.credits.bounds()),
            Grab::Enhance => shown(self.enhance.visible, self.enhance.bounds()),
        }
    }
}

fn inside(r: Rect, x: i32, y: i32) -> bool {
    (r.left..r.right).contains(&x) && (r.top..r.bottom).contains(&y)
}

/// The grab of the window `w`, if it's one of this module's forms.
fn grab_of(w: Win) -> Option<Grab> {
    Some(match w {
        Win::Pause => Grab::Pause,
        Win::Options => Grab::Options,
        Win::Tv => Grab::Tv,
        Win::Money => Grab::Money,
        Win::Overview => Grab::Overview(Image::Map),
        Win::Compass => Grab::Compass,
        Win::Sabowin => Grab::Sabowin,
        Win::Infobox => Grab::Infobox,
        Win::Umenu => Grab::Umenu,
        Win::Ende => Grab::Ende,
        Win::Schluss => Grab::Schluss,
        Win::High => Grab::High,
        Win::Credits => Grab::Credits,
        Win::Enhance => Grab::Enhance,
        Win::Background | Win::Main | Win::Menu | Win::Auction | Win::Help | Win::FileMenu => return None,
    })
}

impl Game {
    /// Plays `sounds` (`play_sound`, 10a0:30b6), in order.
    pub(crate) fn play<N: Into<i16>>(&self, sounds: impl IntoIterator<Item = N>) {
        self.sound.borrow_mut().play_all(sounds, &self.forms.settings);
    }

    /// Hands `actions` to the world as the human's commands.
    fn commands(&mut self, actions: impl IntoIterator<Item = command::Action>) {
        // Single human: the original's player 1.
        self.commands
            .extend(actions.into_iter().map(|action| Command { player: 1, action }));
    }

    /// The form that has the keyboard: the build menu's and the sabotage
    /// window's flags are the tools'.
    pub(crate) fn set_focus(&mut self, focus: Focus) {
        self.forms.focus = focus;
        self.tools.menu_focused = false;
        self.tools.sabowin_focused = false;
    }

    /// After the main screen's view moved or turned: the overview map's
    /// frame (10b8:1485) and the compass's buttons follow.
    pub(crate) fn main_view_changed(&mut self) {
        // Where the TV goes for news without a square (`fix.view_news`).
        self.forms.tv.view_origin = self.screen.camera().origin;
        let (wo, ns) = self.screen.scroll_position();
        let view = self.screen.camera().view;
        // `big_map`'s bigger view has a frame of its own size.
        let frame = self.screen.overview_frame(&self.tiles);
        self.forms.overview.set_big_frame(frame);
        self.forms.overview.set_main_view(wo, ns, view);
        if self.forms.compass.visible() {
            self.forms.compass.update(view);
        }
    }

    /// Button `b` of the button bar, 1 to 5 (1010:2c8b): its form opens,
    /// or closes if it's open.
    pub(crate) fn form_button(&mut self, b: u8) {
        if self.raise_buried(b) {
            return;
        }
        let size = self.size;
        self.tools.button = b; // 1110:5fbe
        match b {
            PAUSE_BUTTON => {
                // 1040:3a45: the clock stops, the button help hides.
                self.tools.down[usize::from(b)] = true;
                self.screen.draw_buttons(&self.tools.down);
                self.forms.help.hide();
                self.forms.pause.show(size);
                self.pause_world(true);
            }
            OPTIONS_BUTTON => {
                if !self.forms.options.visible {
                    self.tools.down[usize::from(b)] = true;
                    self.screen.draw_buttons(&self.tools.down);
                    self.forms.help.hide();
                    let settings = self.forms.settings;
                    let sound = self.sound.borrow().effects.enabled; // 1110:0062
                    self.forms.options.show(&settings, &self.world, sound, size);
                }
            }
            TV_BUTTON => {
                let r = if self.forms.tv.visible {
                    self.forms.tv.close()
                } else {
                    self.forms.tv.show(&self.world, &self.info, self.forms.now)
                };
                self.tv_response(r);
            }
            MONEY_BUTTON => {
                let r = if self.forms.money.shown() {
                    self.forms.money.close()
                } else {
                    self.tools.down[usize::from(b)] = true;
                    self.screen.draw_buttons(&self.tools.down);
                    self.forms.money.show(0, b)
                };
                self.money_response(r);
            }
            OVERVIEW_BUTTON => {
                let down = !self.tools.down[usize::from(b)];
                self.tools.down[usize::from(b)] = down;
                self.screen.draw_buttons(&self.tools.down);
                let r = if down {
                    let r = self.forms.overview.open(&self.world);
                    self.set_focus(Focus::Overview);
                    r
                } else {
                    self.forms.overview.close()
                };
                self.overview_response(r);
            }
            _ => {}
        }
    }

    /// The pointer's press or release asked for the info box
    /// The main screen's client area on the screen, which the info box
    /// is placed by: `ClientWidth` and `ClientHeight` of the borderless
    /// form, one less than its height. // 1038:1ccf
    fn main_client(&self) -> Rect {
        let main = self.screen.bounds();
        Rect::new(main.left, main.top, main.width(), main.height() - 1)
    }

    /// ([`game::tools::Tools::info_box`]). // 1020:04bc, 1020:050b
    pub(crate) fn take_info_box(&mut self) {
        let Some(request) = self.tools.info_box.take() else {
            return;
        };
        match request {
            infobox::Request::Show { square, mouse, modal } => {
                let main = self.main_client();
                self.forms
                    .infobox
                    .show(&self.world, &self.font, &self.info, square, mouse, main, modal);
                self.set_focus(Focus::Infobox);
            }
            infobox::Request::Hide => {
                let r = self.forms.infobox.hide();
                self.infobox_response(r);
            }
        }
    }

    /// What the forms make of a tick or command: the overview map, the TV,
    /// the info box, the buildings' overview, the money menu, the year's
    /// balance and the end of the game. Whether anything changed.
    pub(crate) fn forms_tick(&mut self, t: &Tick) -> bool {
        let mut changed = false;
        if t.loaded {
            // A game loaded from the file menu: the view's fields of its
            // file (1048:2a19).
            if let Some(s) = self.forms.loading_settings.take() {
                self.forms.settings = s;
            }
            let rest = self.view_rest.clone();
            // The TV and the buildings' overview were closed with the
            // windows (`Game::load_windows`, 1010:08d7).
            self.forms.overview.load(&rest);
            self.forms.tv.load_fields(&rest);
            self.forms.money.load_fields(&rest);
            let r = self.forms.money.loaded(&self.world);
            self.money_response(r);
            self.main_view_changed();
            changed = true;
        } else if self.forms.money.shown() && t.books {
            self.forms.money.update(&self.world);
            changed = true;
        }
        changed |= self.forms.overview.show_tick(&self.world, t);
        self.forms.tv.speech_option = self.forms.settings.speech; // 1110:0066
        let r = self.forms.tv.news(
            &t.news,
            self.forms.settings.news,
            &self.world,
            &self.info,
            self.forms.now,
        );
        changed |= self.tv_response(r);
        if let Some(r) = self.forms.infobox.world_changed(&self.world, t) {
            self.infobox_response(r);
            changed = true;
        }
        if t.umenu && self.forms.umenu.visible {
            changed |= self.forms.umenu.refresh(&self.world, &self.font, &self.info);
        }
        if t.new_year && self.forms.settings.year_balance {
            // 1008:4f87: the balance sheet.
            // BUG(original): the button bar's button 4 stays up; `money_menu_button` presses it.
            if self.tools.enhance.has(game::enhance::Enhancement::MoneyMenuButton) {
                self.tools.down[usize::from(MONEY_BUTTON)] = true;
                self.screen.draw_buttons(&self.tools.down);
            }
            let r = self.forms.money.show(1, self.tools.button);
            self.money_response(r);
            changed = true;
        }
        // The view's seat's results (with one human, the game's end).
        if let Some(over) = t.results_for(game::seats::HOST_SEAT) {
            self.game_over(&over);
            changed = true;
        }
        changed
    }

    /// The end of the game (1068:1058): the high score for a fair win,
    /// the TV off, the end form with its film and results. The clock
    /// stands while it's up.
    fn game_over(&mut self, over: &game::simulation::GameOver) {
        let results = Results::of(&self.world, over, self.info.currency_factor, self.info.language);
        if let Some(score) = over.score {
            let (d, m, y) = today();
            let name = self.world.player_names.get(1).cloned().unwrap_or_default();
            let date = highscoreform::date_text(d, m, y, self.info.language);
            let r = self
                .forms
                .high
                .add(self.world.scenario, &name, &date, &results.net_worth, score);
            if r.save {
                self.forms.requests.push(Request::SaveHighScores);
            }
        }
        let r = self.forms.tv.close();
        self.tv_response(r);
        self.forms.help.hide();
        let music = self.forms.settings.music;
        let r = self.forms.schluss.open_end(results, music, self.size);
        self.schluss_response(r);
    }

    /// The overview map's scroll timers (10b8:21c7), and with
    /// `scroll_bars` the repeat of the scroll arrows held down on the
    /// options, the buildings' overview and the list of enhancements, for
    /// `ms` of time.
    pub(crate) fn forms_timers(&mut self, ms: u32) -> bool {
        self.forms.now = self.forms.now.wrapping_add(ms);
        let mut changed = false;
        if self.forms.options.visible {
            let r = self.forms.options.repeat(ms);
            changed |= r.refresh;
            self.options_response(r);
        }
        if self.forms.umenu.visible {
            let r = self.forms.umenu.repeat(ms, &self.world, &self.font, &self.info);
            changed |= r.refresh;
            self.umenu_response(r);
        }
        if self.forms.enhance.visible {
            let r = self.forms.enhance.repeat(ms);
            changed |= r.refresh;
            self.enhance_response(r);
        }
        if self.forms.overview.scrolling() {
            self.forms.since_scroll += ms;
            while self.forms.since_scroll >= SCROLL_TIMER_MS {
                self.forms.since_scroll -= SCROLL_TIMER_MS;
                changed |= self.forms.overview.scroll_timers(&self.world);
            }
        } else {
            self.forms.since_scroll = 0;
        }
        changed
    }

    /// The clock's step for the TV (8, 1048:1b3e) and the tooltips (3,
    /// 1020:1e9b). Whether anything changed.
    pub(crate) fn forms_clock_tick(&mut self) -> bool {
        let mut changed = false;
        if self.forms.tv.visible {
            let (done, film_done) = {
                let s = self.sound.borrow();
                (s.speaker_done(), s.film_done())
            };
            let r = self.forms.tv.timer(&self.world, self.forms.now, done, film_done);
            changed |= self.tv_response(r);
        }
        changed | self.tooltips()
    }

    /// The tooltips (`1110:0060`, 1020:1e9b): the button bar's under the
    /// mouse, else the money menu's tabs, else the overview map's buttons.
    fn tooltips(&mut self) -> bool {
        if !self.forms.settings.button_help {
            return self.forms.help.hide();
        }
        let (width, height) = self.screen.size();
        // The button bar's rows are 0x22 to 0x3b from the form's top; the
        // form as if it had its title bar (`no_main_title`).
        let bar = self.screen.rects()[2].1;
        let top = self.screen.top + bar.top - 0x22;
        let main = Rect::new(self.screen.left, top, width, height);
        // Not while the main screen is rolled up (`+0x1da`, 1020:2115).
        let ready = !self.forms.modal() && !self.screen.rolled_up;
        // The build menu and the sabotage window take the focus from the
        // main screen.
        let focus = if self.tools.menu_focused || self.tools.sabowin_focused {
            None
        } else {
            Some(self.forms.focus)
        };
        // In the original's order, the last one found wins: the money
        // menu's tabs (1020:1e9b), the overview map's buttons (1020:1fe2)
        // and the button bar (1020:2115).
        let mut tip = None;
        if ready {
            tip = self
                .forms
                .money
                .tooltip(self.mouse.0, self.mouse.1, focus == Some(Focus::Money))
                .map(|(t, l, tp)| (t, tp, l));
        }
        if ready && focus == Some(Focus::Overview) && self.forms.overview.visible() {
            let o = &self.forms.overview;
            let (x, y) = (self.mouse.0 - o.left, self.mouse.1 - o.top);
            let menu = o
                .images()
                .iter()
                .find(|(i, _, _)| *i == Image::Menu)
                .map(|&(_, r, _)| r);
            if let Some(menu) = menu
                && let Some((t, l, bottom)) = o.help_text_at(x - menu.left, y - menu.top)
            {
                tip = Some((t, bottom + menu.top + o.top, l + menu.left + o.left));
            }
        }
        let bar = helpform::button_bar_tooltip(
            main,
            self.mouse,
            self.forms.bar_x,
            self.world.scenario,
            ready && focus == Some(Focus::Main),
        );
        tip = bar.or(tip);
        match tip {
            Some((t, tp, l)) => self.forms.help.show(t, tp, l, self.size),
            None => self.forms.help.hide(),
        }
    }

    /// Remembers the mouse's x on `ButtonImage` or `Hauptbild`, for the
    /// tooltips. // 1018:3e46
    pub(crate) fn bar_mouse(&mut self, fx: i32) {
        self.forms.bar_x = fx;
    }

    /// After the commands were applied: what the forms draw again from the
    /// world.
    pub(crate) fn forms_after_commands(&mut self) -> bool {
        let mut changed = false;
        if std::mem::take(&mut self.forms.money_update) {
            self.forms.money.update(&self.world);
            changed = true;
        }
        if std::mem::take(&mut self.forms.infobox_redraw) && self.forms.infobox.visible {
            self.forms.infobox.redraw(&self.world, &self.font, &self.info);
            changed = true;
        }
        if let Some(r) = self.forms.umenu_redraw.take() {
            self.forms.umenu.redraw(r, &self.world, &self.font, &self.info);
            changed = true;
        }
        changed
    }

    /// Where the film that plays shows on the screen: the TV's `Panel1`
    /// (cut off by the form, which may be rolled up) or the end form's.
    pub(crate) fn film_panel(&self) -> Option<Rect> {
        if self.forms.tv.film_playing() {
            let tv = self.forms.tv.bounds();
            let p = fernseher::FILM_PANEL;
            let left = tv.left + p.left;
            let top = tv.top + p.top;
            let right = (left + p.width()).min(tv.left + tv.width());
            let bottom = (top + p.height()).min(tv.top + tv.height());
            return Some(Rect::new(left, top, (right - left).max(0), (bottom - top).max(0)));
        }
        self.forms.schluss.film_panel()
    }

    pub(crate) fn tv_response(&mut self, mut r: fernseher::Response) -> bool {
        self.sound.borrow_mut().tv_media(&r.media, &self.forms.settings);
        if let Some(panel) = self.film_panel() {
            self.sound.borrow_mut().set_film_panel(panel);
        }
        if self.sound.borrow_mut().take_speaker_failed() {
            r.actions.extend(self.forms.tv.speaker_missing());
        }
        self.play(r.sounds.iter().copied());
        // The speaker's state is the original's global `1110:662d`, set at
        // once: the world has it before its next tick.
        let (speaker, actions): (Vec<_>, Vec<_>) = r
            .actions
            .into_iter()
            .partition(|a| matches!(a, command::Action::Speaker(_)));
        for action in speaker {
            let _ = self.world.apply(Command { player: 1, action });
        }
        self.commands(actions);
        if r.shown {
            self.tools.down[usize::from(TV_BUTTON)] = true;
            self.screen.draw_buttons(&self.tools.down);
            self.set_focus(Focus::Tv);
            let h = self.forms.infobox.hide();
            self.infobox_response(h);
        }
        if r.hidden {
            self.tools.down[usize::from(TV_BUTTON)] = false;
            self.screen.draw_buttons(&self.tools.down);
            if self.forms.focus == Focus::Tv {
                self.set_focus(Focus::Main);
            }
        }
        if let Some(p) = r.scroll {
            self.screen.jump_to(p, &self.world, &self.tiles); // 1020:0787
            self.screen
                .show_people(&self.world, &self.tiles, &self.font, &self.info);
            self.main_view_changed();
        }
        r.refresh || r.shown || r.hidden || r.scroll.is_some()
    }

    fn money_response(&mut self, r: geldmenu::Response) {
        self.play(r.sounds.iter().copied());
        self.commands(r.commands);
        self.forms.money_update |= r.update;
        if r.opened {
            self.set_focus(Focus::Money);
            let h = self.forms.infobox.hide();
            self.infobox_response(h);
        }
        if r.closed {
            self.tools.down[usize::from(MONEY_BUTTON)] = false; // 1010:2ade
            self.screen.draw_buttons(&self.tools.down);
            self.set_focus(Focus::Main);
        }
        if r.overview {
            self.open_umenu();
        }
        if r.focus_main {
            // Space: `BringToFront` of the main screen (1070:39f9, 10b8:4def).
            self.stack.raise(Win::Main);
            self.set_focus(Focus::Main);
        }
        if let Some((key, shift)) = r.main_screen_key {
            self.main_screen_key(key, shift);
        }
    }

    pub(crate) fn overview_response(&mut self, r: uebersichtsmap::Response) {
        self.play(r.sounds.iter().copied());
        if let Some((wo, ns)) = r.scroll {
            self.screen.set_scroll(wo, ns, &self.world, &self.tiles);
            self.screen
                .show_people(&self.world, &self.tiles, &self.font, &self.info);
            self.main_view_changed();
        }
        if r.draw_main {
            self.screen.paint_map(&self.world, &self.tiles);
            self.screen
                .show_people(&self.world, &self.tiles, &self.font, &self.info);
        }
        if r.closed {
            self.tools.down[usize::from(OVERVIEW_BUTTON)] = false; // 1010:2b37
            self.screen.draw_buttons(&self.tools.down);
            self.set_focus(Focus::Main);
        }
        if r.focus_main {
            // Space: `BringToFront` of the main screen (1070:39f9, 10b8:4def).
            self.stack.raise(Win::Main);
            self.set_focus(Focus::Main);
        }
        self.commands(uebersichtsmap::command_for(r.action, &self.world));
        if let Some((key, shift)) = r.key {
            self.main_screen_key(key, shift);
        }
    }

    fn compass_response(&mut self, r: &kompassform::Response) {
        self.play(r.sounds.iter().copied());
        if r.knob {
            self.forms.compass.draw_knob(&mut self.screen.corner);
        }
        if let Some(view) = r.view {
            self.set_view(view);
        }
    }

    /// The view turns to `view` (1010:254d): the island, the build menu's
    /// picture, the overview map and the compass.
    pub(crate) fn set_view(&mut self, view: u8) {
        self.screen.set_view(view, &self.world, &self.tiles);
        let view = self.screen.camera().view;
        self.tools
            .menu
            .view_turned(&self.world, &self.font, &self.tiles, &self.info, view);
        // 1010:258c: `people_refresh` too.
        self.view_moved();
    }

    pub(crate) fn infobox_response(&mut self, r: infobox::Response) {
        self.play(r.sounds.iter().copied());
        if let Some(a) = r.action {
            self.commands([a]);
            self.forms.infobox_redraw = true;
        }
        if r.closed && self.forms.focus == Focus::Infobox {
            self.set_focus(Focus::Main);
        }
        if r.open_overview {
            self.open_umenu();
        }
    }

    /// The buildings' overview opens (1080:0ea8), with the focus.
    pub(crate) fn open_umenu(&mut self) {
        self.forms.umenu.open(&self.world, &self.font, &self.info, self.size);
        self.set_focus(Focus::Umenu);
    }

    pub(crate) fn umenu_response(&mut self, r: umenuform::Response) {
        self.play(r.sounds.iter().copied());
        if let Some(a) = r.action {
            self.commands([a]);
            self.forms.umenu_redraw = r.redraw;
        } else if let Some(redraw) = r.redraw {
            self.forms.umenu.redraw(redraw, &self.world, &self.font, &self.info);
        }
        if let Some(pos) = r.jump {
            self.screen.jump_to(pos, &self.world, &self.tiles); // 1020:0787
            self.screen
                .show_people(&self.world, &self.tiles, &self.font, &self.info);
            self.main_view_changed();
        }
        if r.closed {
            self.set_focus(Focus::Main);
        }
    }

    fn options_response(&mut self, r: optionen::Response) {
        self.commands(r.commands);
        self.forms.settings = *self.forms.options.settings();
        // The widgets' sounds with "Geräusche" as it was before the click.
        self.play(r.sounds.iter().map(|s| s.number()));
        for change in r.changes {
            let mut sound = self.sound.borrow_mut();
            match change {
                optionen::Change::Pointers => {
                    drop(sound);
                    // 1060:0656
                    self.screen.camera.owner_pointers = self.forms.settings.owner_pointers;
                    self.screen.camera.occupancy_pointers = self.forms.settings.occupancy_pointers;
                    self.screen.paint_map(&self.world, &self.tiles);
                    self.screen
                        .show_people(&self.world, &self.tiles, &self.font, &self.info);
                }
                optionen::Change::Music(on) => sound.music_on(on, &self.forms.settings),
                optionen::Change::Track(n) => sound.select_track(n, &mut self.forms.settings),
                optionen::Change::MusicVolume(v) => sound.set_music_volume(v),
                optionen::Change::SoundVolume(v) => sound.set_wave_volume(v),
            }
        }
        match r.action {
            Some(optionen::Action::Close) => {
                self.forms.options.visible = false;
                self.tools.down[usize::from(OPTIONS_BUTTON)] = false;
                self.screen.draw_buttons(&self.tools.down);
                self.set_focus(Focus::Main);
            }
            Some(optionen::Action::Highscores) => {
                // 1040:2579: over the options, which wait for it.
                let r = self.forms.high.show(self.world.scenario, true, self.size);
                self.high_response(&r);
            }
            Some(optionen::Action::Credits) => self.open_credits(),
            Some(optionen::Action::Enhancements) => {
                // `enhancements_screen`: over the options, which wait for
                // it, with the world's fixes and the player's set.
                let view = self.forms.options.enhance;
                let notes = self.forms.enhance_notes;
                let r = self.forms.enhance.show(self.world.fixes, view, notes, self.size);
                self.enhance_response(r);
            }
            None => {}
        }
    }

    /// `TCreditform` opens over the options or the list of enhancements,
    /// which wait for it. // 1060:01ab, 1030:3c42
    fn open_credits(&mut self) {
        let r = self.forms.credits.show(self.size);
        self.credits_response(&r);
    }

    /// The credits' sounds; their close enables the form under them
    /// again (1030:3e6f), which [`Game::modal_grabs`] sees.
    fn credits_response(&mut self, r: &creditform::Response) {
        self.play(r.sounds.iter().map(|s| s.number()));
    }

    /// The list of enhancements: its sounds, the world's fixes as the
    /// human's commands, the player's settings for the caller to change
    /// and save, and the credits.
    pub(crate) fn enhance_response(&mut self, r: enhanceform::Response) {
        self.play(r.sounds.iter().map(|s| s.number()));
        self.commands(r.commands);
        self.forms.requests.extend(r.changes.into_iter().map(Request::Enhance));
        match r.action {
            Some(enhanceform::Action::Credits) => self.open_credits(),
            Some(enhanceform::Action::Close) | None => {}
        }
    }

    fn ende_response(&mut self, r: &endeform::Response) {
        self.play(r.sounds.iter().map(|s| s.number()));
        match r.action {
            // The form hid itself; the clock runs again (1020:24b7).
            Some(endeform::Action::Close) => self.set_focus(Focus::Main),
            Some(endeform::Action::Quit) => self.forms.requests.push(Request::Quit),
            None => {}
        }
    }

    pub(crate) fn schluss_response(&mut self, r: schlussform::Response) {
        self.schluss_sound(&r);
        let mut r = r;
        if r.stop_film {
            // 1068:17ad: `Filmplayer` stops and closes, the session takes
            // the wave device back (10a0:254f).
            let mut sound = self.sound.borrow_mut();
            sound.stop_film();
            sound.effects_on();
        }
        if let Some(film) = r.film {
            // 1068:12b7, 1068:0ec9: the `WaveMix` session gives the film
            // the wave device, and it plays in `Panel1`.
            let panel = self.forms.schluss.film_panel().unwrap_or_default();
            let opened = {
                let mut sound = self.sound.borrow_mut();
                sound.effects_off();
                sound.play_film(film, panel)
            };
            if !opened {
                // The original raises an exception in the middle of
                // opening the form; the port ends the film at once.
                let first = r.action;
                let ended = self.forms.schluss.film_ended(self.size);
                self.sound.borrow_mut().effects_on();
                self.schluss_sound(&ended);
                r = schlussform::Response {
                    action: ended.action.or(first),
                    ..ended
                };
            }
        }
        match r.action {
            Some(schlussform::Action::Continue { restart_music }) => {
                if restart_music {
                    // 1068:0180: `music_start` (1018:3850).
                    self.forms.settings.music = true;
                    self.sound.borrow_mut().music_on(true, &self.forms.settings);
                }
                self.set_focus(Focus::Main);
            }
            Some(schlussform::Action::HighScores) => {
                let r = self.forms.high.show(self.world.scenario, false, self.size);
                self.high_response(&r);
            }
            Some(schlussform::Action::NewGame) => self.forms.requests.push(Request::NewGame),
            Some(schlussform::Action::Exit) => self.forms.requests.push(Request::Exit),
            None => {}
        }
    }

    /// The end form's music and sounds: the background music stops
    /// (`music_stop`, 1018:38c5, which turns the option off), the end's
    /// plays or stops.
    fn schluss_sound(&mut self, r: &schlussform::Response) {
        let mut sound = self.sound.borrow_mut();
        if r.stop_background_music {
            sound.music_on(false, &self.forms.settings);
            self.forms.settings.music = false;
        }
        if r.stop_music {
            sound.stop_end_music();
        }
        if let Some(file) = r.music {
            sound.play_end_music(file);
        }
        sound.play_all(r.sounds.iter().map(|s| s.number()), &self.forms.settings);
    }

    /// `TSchlussform.Timer1`, once a second (`Interval` 1000) of the `ms`
    /// that passed: the film's end, and when the end's music is over it
    /// plays the loop. // 1068:187f
    pub(crate) fn end_music_tick(&mut self, ms: u32) {
        self.forms.timer1_ms = self.forms.timer1_ms.saturating_add(ms);
        if self.forms.timer1_ms < TIMER1_MS {
            return;
        }
        self.forms.timer1_ms %= TIMER1_MS;
        if !self.forms.schluss.visible() {
            return;
        }
        // The film's end: `Position = Length`, so the last frame stays up
        // to a second.
        if self.forms.schluss.film_panel().is_some() && self.sound.borrow().film_done() {
            let r = self.forms.schluss.film_ended(self.size);
            self.schluss_response(r);
        }
        let ended = {
            let s = self.sound.borrow();
            s.end_music.is_open() && s.end_music.at_end()
        };
        if ended && let Some(file) = self.forms.schluss.music_ended() {
            self.sound.borrow_mut().play_end_music(file);
        }
    }

    fn high_response(&mut self, r: &highscoreform::Response) {
        self.play(r.sounds.iter().map(|s| s.number()));
        if r.save {
            self.forms.requests.push(Request::SaveHighScores);
        }
    }

    fn sabowin_response(&mut self, r: &sabowin::Response) {
        self.play(r.sounds.iter().copied());
        if r.close.is_some() {
            self.set_focus(Focus::Main);
        }
        self.show_tools();
    }

    /// Opens the quit question, unless the game is quitting.
    /// // 1020:0480, 1020:0577
    pub(crate) fn open_ende(&mut self) {
        if self.forms.ende.open(self.size) {
            self.forms.help.hide();
        }
    }

    /// The form of this module under the screen's `x`, `y` that takes a
    /// press: the top window there ([`Game::window_at`]), if it's one of
    /// this module's and the modal forms leave it. A disabled form takes
    /// no press, even where it lies over the modal one.
    fn form_at(&self, x: i32, y: i32) -> Option<Grab> {
        let f = &self.forms;
        let g = grab_of(self.window_at(x, y))?;
        if self.modal_grabs().is_some_and(|a| !a.contains(&g)) {
            return None;
        }
        match g {
            Grab::Overview(_) => {
                let r = f.overview.bounds();
                let (image, _, _) = f.overview.image_at(x - r.left, y - r.top)?;
                Some(Grab::Overview(image))
            }
            _ => Some(g),
        }
    }

    /// The forms a modal form leaves enabled, while one is up.
    fn modal_grabs(&self) -> Option<&'static [Grab]> {
        let f = &self.forms;
        if f.high.visible {
            Some(&[Grab::High])
        } else if f.credits.visible {
            Some(&[Grab::Credits])
        } else if f.enhance.visible {
            Some(&[Grab::Enhance])
        } else if f.schluss.visible() {
            Some(&[Grab::Schluss])
        } else if f.ende.visible {
            Some(&[Grab::Ende])
        } else if f.options.visible {
            Some(&[Grab::Options])
        } else if f.pause.visible {
            Some(&[Grab::Pause])
        } else if f.infobox.visible && f.infobox.modal {
            Some(&[Grab::Tv, Grab::Infobox])
        } else {
            None
        }
    }

    /// A press on the window `w`, the top one under the mouse. In Windows
    /// it activates `w`, which raises it, unless it's disabled (every
    /// other form while a modal one is up); a press on the background is
    /// [`crate::stacking::Stack::background_pressed`]. Whether the order
    /// changed.
    pub(crate) fn press_window(&mut self, w: Win) -> bool {
        let before = self.stack.order(self.infobox_modal());
        self.stack.before_press = Some(before.clone());
        let enabled = match self.modal_grabs() {
            None => true,
            Some(a) => grab_of(w).is_some_and(|g| a.contains(&g)),
        };
        if w == Win::Background {
            let (infobox, schluss) = (self.forms.infobox.visible, self.forms.schluss.visible());
            self.stack.background_pressed(true, infobox, schluss);
        } else if enabled {
            self.stack.raise(w);
        }
        before != self.stack.order(self.infobox_modal())
    }

    /// Whether the window `w` is shown.
    pub(crate) fn shown(&self, w: Win) -> bool {
        let f = &self.forms;
        match w {
            Win::Background | Win::Main => true,
            Win::Menu => self.tools.menu.height > 0,
            Win::Auction => self.auction.visible,
            Win::FileMenu => self.file_menu.visible,
            Win::Sabowin => self.tools.sabowin.as_ref().is_some_and(|s| s.visible),
            Win::Help => f.help_visible(),
            w => grab_of(w).is_some_and(|g| f.grab_bounds(g, None).is_some()),
        }
    }

    /// Whether the screen's `x`, `y` is on the window `w` as it's shown.
    /// The tooltip (`Enabled = False` in its DFM) never is: presses go to
    /// what lies under it.
    fn on_window(&self, w: Win, x: i32, y: i32) -> bool {
        match w {
            Win::Background => true,
            Win::Main => inside(self.screen.bounds(), x, y),
            Win::Menu => self.tools.menu.contains(x, y),
            Win::Auction => self.auction.contains(x - self.auction.left, y - self.auction.top),
            Win::FileMenu => {
                let m = &self.file_menu;
                m.visible
                    && inside(
                        Rect::new(m.menu.left, m.menu.top, dateimenue::WIDTH, dateimenue::HEIGHT),
                        x,
                        y,
                    )
            }
            Win::Help => false,
            Win::Sabowin => self.tools.sabowin.as_ref().is_some_and(|s| {
                let r = s.bounds();
                s.contains(x - r.left, y - r.top)
            }),
            w => grab_of(w)
                .and_then(|g| self.forms.grab_bounds(g, None))
                .is_some_and(|r| inside(r, x, y)),
        }
    }

    /// Where the window `w` is on the screen, while it's shown; none for
    /// the background.
    fn window_bounds(&self, w: Win) -> Option<Rect> {
        if !self.shown(w) {
            return None;
        }
        match w {
            Win::Background => None,
            Win::Main => Some(self.screen.bounds()),
            Win::Menu => Some(self.tools.menu.bounds()),
            Win::Auction => Some(self.auction.bounds()),
            Win::FileMenu => {
                let m = &self.file_menu.menu;
                Some(Rect::new(m.left, m.top, dateimenue::WIDTH, dateimenue::HEIGHT))
            }
            Win::Help => Some(self.forms.help.bounds()),
            Win::Sabowin => self.tools.sabowin.as_ref().map(Sabowin::bounds),
            w => grab_of(w).and_then(|g| self.forms.grab_bounds(g, None)),
        }
    }

    /// With the `window_stacking` enhancement, the button of a form
    /// that's open but lies under another window raises it (with the
    /// compass for the overview map's) instead of closing it, as a task
    /// bar does. Under the order before the press that clicked the
    /// button: the press raised the main screen. Whether it did.
    fn raise_buried(&mut self, b: u8) -> bool {
        let w = match b {
            TV_BUTTON => Win::Tv,
            MONEY_BUTTON => Win::Money,
            OVERVIEW_BUTTON => Win::Overview,
            _ => return false,
        };
        let Some(r) = self.window_bounds(w).filter(|_| self.stack.enhanced) else {
            return false;
        };
        let order = self
            .stack
            .before_press
            .clone()
            .unwrap_or_else(|| self.stack.order(self.infobox_modal()));
        let covered = order
            .iter()
            .skip_while(|&&o| o != w)
            .skip(1)
            .filter(|&&o| o != Win::Help)
            .filter_map(|&o| self.window_bounds(o))
            .any(|o| o.left < r.right && r.left < o.right && o.top < r.bottom && r.top < o.bottom);
        if covered {
            self.stack.raise(w);
            if w == Win::Overview {
                self.stack.raise(Win::Compass);
            }
        }
        covered
    }

    /// The top window under the screen's `x`, `y`; the background where
    /// there's none.
    pub(crate) fn window_at(&self, x: i32, y: i32) -> Win {
        self.stack
            .order(self.infobox_modal())
            .into_iter()
            .rev()
            .find(|&w| self.shown(w) && self.on_window(w, x, y))
            .unwrap_or(Win::Background)
    }

    /// The info box has buttons and disables the other forms.
    pub(crate) fn infobox_modal(&self) -> bool {
        self.forms.infobox.visible && self.forms.infobox.modal
    }

    /// Tells the stack which windows are shown now
    /// ([`crate::stacking::Stack::sync`]).
    pub(crate) fn sync_stack(&mut self) {
        let shown: Vec<Win> = crate::stacking::ALL.into_iter().filter(|&w| self.shown(w)).collect();
        if self.stack.main_desktop {
            // `big_map`'s main screen fills the screen: what opens at its
            // top edge goes below its bars.
            let bars = self.screen.hauptbild().top + self.screen.top;
            for &w in shown.iter().filter(|&&w| !self.stack.was_shown(w)) {
                self.forms.below_bars(w, bars, self.size);
            }
        }
        self.stack.sync(|w| shown.contains(&w));
    }

    /// A mouse event for the forms of this module: whether one of them
    /// took it (and the screen needs painting), or the event goes on to
    /// the auction form, the build menu and the main screen. A modal form
    /// takes every press, wherever it is.
    #[allow(clippy::too_many_lines)] // One form after the other.
    pub(crate) fn forms_mouse(&mut self, input: Input, shift: ShiftState, buttons_up: bool) -> bool {
        let Some((sx, sy)) = position(input) else { return false };
        let grab = match (self.forms.grab, input) {
            (Some(g), _) => Some(g),
            (None, Input::MouseDown { .. }) => self.form_at(sx, sy),
            _ => None,
        };
        if grab.is_none()
            && let Input::MouseMove { .. } = input
            && self.modal_grabs() == Some(&[Grab::Enhance])
        {
            // The list of enhancements describes the row under the mouse.
            let e = &self.forms.enhance;
            let r = self.forms.enhance.mouse_move(sx - e.left, sy - e.top);
            let refresh = r.refresh;
            self.enhance_response(r);
            return refresh;
        }
        let Some(grab) = grab else {
            // Over a modal form's back, nothing else takes the mouse.
            return self.forms.modal() && !matches!(input, Input::MouseMove { .. });
        };
        let Some(bounds) = self.forms.grab_bounds(grab, self.tools.sabowin.as_ref()) else {
            self.forms.grab = None;
            return false;
        };
        match input {
            Input::MouseDown { .. } => self.forms.grab = Some(grab),
            Input::MouseUp { .. } if buttons_up => self.forms.grab = None,
            _ => {}
        }
        let (x, y) = (sx - bounds.left, sy - bounds.top);
        let size = self.size;
        let down = matches!(input, Input::MouseDown { .. });
        let up = matches!(input, Input::MouseUp { .. });
        let l = left(input);
        match grab {
            Grab::Pause => {
                // `PausebildClick`: a release over the picture.
                if up && inside(Rect::new(0, 0, pauseform::WIDTH, pauseform::HEIGHT), x, y) {
                    let pauseform::Action::Close = self.forms.pause.click();
                    self.close_pause();
                }
            }
            Grab::Options => {
                let r = if down {
                    self.forms.options.mouse_down(x, y, shift)
                } else if up {
                    self.forms.options.mouse_up(x, y)
                } else {
                    self.forms.options.mouse_move(x, y, shift, size)
                };
                self.options_response(r);
            }
            Grab::Tv => {
                self.set_focus(Focus::Tv);
                if down {
                    let r = self.forms.tv.mouse_down(x, y, shift);
                    self.tv_response(r);
                } else if up {
                    let r = self.forms.tv.mouse_up(x, y, &self.world, &self.info);
                    self.tv_response(r);
                } else {
                    self.forms.tv.mouse_move(x, y, shift, size);
                }
            }
            Grab::Money => {
                self.set_focus(Focus::Money);
                let r = if down {
                    self.forms.money.mouse_down(x, y, l, shift)
                } else if up {
                    self.forms.money.mouse_up(x, y, l)
                } else {
                    self.forms.money.mouse_move(x, y, shift, size)
                };
                self.money_response(r);
            }
            Grab::Overview(image) => {
                self.set_focus(Focus::Overview);
                let r = self.overview_mouse(image, input, (x, y), (sx, sy), shift);
                self.overview_response(r);
            }
            Grab::Compass => {
                let r = if down {
                    self.forms.compass.mouse_down(x, y, shift, (sx, sy))
                } else if up {
                    self.forms.compass.mouse_up(x, y)
                } else {
                    self.forms.compass.mouse_move(shift, (sx, sy), size)
                };
                self.compass_response(&r);
            }
            Grab::Sabowin => {
                self.tools.sabowin_focused = true;
                self.tools.menu_focused = false;
                let r = if down {
                    self.tools.sabowin_mouse_down(x, y, l)
                } else if up {
                    self.tools.sabowin_mouse_up(x, y, l)
                } else {
                    self.tools.sabowin_mouse_move(x, y, shift, size)
                };
                self.sabowin_response(&r);
            }
            Grab::Infobox => {
                self.set_focus(Focus::Infobox);
                if down {
                    let r = self.forms.infobox.mouse_down(x, y, &self.font);
                    self.infobox_response(r);
                } else if up {
                    let main = self.main_client();
                    let r = self
                        .forms
                        .infobox
                        .mouse_up(x, y, &self.world, &self.font, &self.info, main);
                    self.infobox_response(r);
                }
            }
            Grab::Umenu => {
                self.set_focus(Focus::Umenu);
                let (w, f, i) = (&self.world, &self.font, &self.info);
                let r = if down {
                    self.forms.umenu.mouse_down(x, y, shift, w, f, i)
                } else if up {
                    self.forms.umenu.mouse_up(x, y, w, f, i)
                } else {
                    self.forms.umenu.mouse_move(x, y, shift, w, f, i, size)
                };
                self.umenu_response(r);
            }
            Grab::Ende => {
                let r = if down {
                    self.forms.ende.mouse_down(x, y, l)
                } else if up {
                    self.forms.ende.mouse_up(x, y, l)
                } else {
                    self.forms.ende.mouse_move(x, y, shift, size)
                };
                self.ende_response(&r);
            }
            Grab::Schluss => {
                let r = if down {
                    self.forms.schluss.mouse_down(x, y)
                } else if up {
                    self.forms.schluss.mouse_up(x, y)
                } else {
                    schlussform::Response::default()
                };
                self.schluss_response(r);
            }
            Grab::High => {
                let r = if down {
                    self.forms.high.mouse_down(x, y, shift)
                } else if up {
                    self.forms.high.mouse_up(x, y)
                } else {
                    highscoreform::Response::default()
                };
                self.high_response(&r);
            }
            Grab::Credits => {
                let r = if down {
                    self.forms.credits.mouse_down(x, y, shift)
                } else if up {
                    self.forms.credits.mouse_up(x, y)
                } else {
                    creditform::Response::default()
                };
                self.credits_response(&r);
            }
            Grab::Enhance => {
                let r = if down {
                    self.forms.enhance.mouse_down(x, y, shift)
                } else if up {
                    self.forms.enhance.mouse_up(x, y)
                } else {
                    self.forms.enhance.mouse_move(x, y)
                };
                self.enhance_response(r);
            }
        }
        true
    }

    /// A mouse event on one of the overview map's images, at `local` in
    /// it (`cursor` on the screen).
    fn overview_mouse(
        &mut self,
        image: Image,
        input: Input,
        (fx, fy): (i32, i32),
        cursor: (i32, i32),
        shift: ShiftState,
    ) -> uebersichtsmap::Response {
        let o = &mut self.forms.overview;
        let r = o.images().iter().find(|(i, _, _)| *i == image).map(|&(_, r, _)| r);
        let r = r.unwrap_or_default();
        let (x, y) = (fx - r.left, fy - r.top);
        let w = &self.world;
        let l = left(input);
        match (image, input) {
            (Image::Title, Input::MouseDown { .. }) => o.title_mouse_down(x, shift, cursor),
            (Image::Title, Input::MouseUp { .. }) => o.title_mouse_up(x, y, w),
            (Image::Title, _) => o.title_mouse_move(shift, cursor, self.size),
            (Image::Toggle, Input::MouseDown { .. }) => o.toggle_mouse_down(l),
            (Image::Toggle, Input::MouseUp { .. }) => o.toggle_mouse_up(x, y, l, w),
            (Image::Scroll, Input::MouseDown { .. }) => o.scroll_mouse_down(y, w),
            (Image::Scroll, Input::MouseUp { .. }) => o.scroll_mouse_up(),
            (Image::Scroll, _) => o.scroll_mouse_move(y, shift, w),
            (Image::Menu, Input::MouseDown { .. }) => o.menu_mouse_down(x, y, l, w),
            (Image::Menu, Input::MouseUp { .. }) => o.menu_mouse_up(x, y, w),
            (Image::Menu, _) => o.menu_mouse_move(x, shift, w),
            (Image::Map, Input::MouseDown { .. }) => o.map_mouse_down(x, y, shift, w),
            (Image::Map, Input::MouseUp { .. }) => o.map_mouse_up(l),
            (Image::Map, _) => o.map_mouse_move(x, y, shift),
            _ => uebersichtsmap::Response::default(),
        }
    }

    /// With several human clients the pause form stops the world for
    /// everyone, a command (`Action::Pause`, the host's); with one the
    /// form stops this view's clock (`Game::clock_stopped`), as the
    /// original (1020:24a0).
    fn pause_world(&mut self, on: bool) {
        if self.world.seats.multiplayer() {
            self.commands.push(game::command::Command {
                player: game::seats::HOST_SEAT,
                action: game::command::Action::Pause(on),
            });
        }
    }

    /// The pause ends (1040:3bac): the form hides, the button comes up,
    /// the clock runs again.
    fn close_pause(&mut self) {
        self.pause_world(false);
        self.forms.pause.visible = false;
        self.tools.down[usize::from(PAUSE_BUTTON)] = false;
        self.screen.draw_buttons(&self.tools.down);
        self.set_focus(Focus::Main);
    }

    /// A key for the forms of this module: the modal ones take every key,
    /// the others the keys while they have the focus. Whether one took it;
    /// if not it goes to the build menu or the main screen.
    pub(crate) fn forms_key(&mut self, key: u16, shift: ShiftState) -> bool {
        let size = self.size;
        if self.forms.high.visible || self.forms.credits.visible {
            // `Thighscoreform` and `TCreditform` have no keys.
            return true;
        }
        if self.forms.enhance.visible {
            let r = self.forms.enhance.key_down(key, shift);
            self.enhance_response(r);
            return true;
        }
        if self.forms.schluss.visible() {
            let r = self.forms.schluss.key_down(size);
            self.schluss_response(r);
            return true;
        }
        if self.forms.ende.visible {
            let r = self.forms.ende.key_down(key);
            self.ende_response(&r);
            return true;
        }
        if self.forms.options.visible {
            let r = self.forms.options.key_down(key, shift);
            self.options_response(r);
            return true;
        }
        if self.forms.pause.visible {
            let pauseform::Action::Close = self.forms.pause.key_down(key);
            self.close_pause();
            return true;
        }
        if self.forms.infobox.visible && self.forms.infobox.modal {
            if let Some(r) = self.forms.infobox.key_down(key, shift) {
                self.infobox_response(r);
            }
            return true;
        }
        if self.tools.sabowin_focused {
            let r = self.tools.sabowin_key_down(key, shift);
            self.sabowin_response(&r);
            return true;
        }
        if self.tools.menu_focused {
            return false;
        }
        match self.forms.focus {
            Focus::Main => false,
            Focus::Tv => {
                let r = self.forms.tv.key_down(key, shift, &self.world, &self.info);
                self.tv_response(r);
                true
            }
            Focus::Money => {
                let r = self.forms.money.key_down(key, shift);
                self.money_response(r);
                true
            }
            Focus::Overview => {
                let r = self.forms.overview.key_down(key, shift, &self.world);
                self.overview_response(r);
                true
            }
            Focus::Infobox => {
                if let Some(r) = self.forms.infobox.key_down(key, shift) {
                    self.infobox_response(r);
                }
                true
            }
            Focus::Umenu => {
                let r = self
                    .forms
                    .umenu
                    .key_down(key, shift, &self.world, &self.font, &self.info);
                self.umenu_response(r);
                true
            }
        }
    }

    /// What the forms add to the view's fields of the map file.
    pub(crate) fn forms_save(&self, rest: &mut Fields) {
        self.save_windows(rest);
        self.forms.overview.save(rest);
        self.forms.tv.store_fields(rest);
        self.forms.money.store_fields(rest);
    }
}
