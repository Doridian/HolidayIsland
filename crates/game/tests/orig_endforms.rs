//! The end of a game and the forms around it: a scenario played for a
//! while and then won (and one lost), the end form's texts and buttons,
//! the score going into the high scores, the quit question and the
//! button bar's tooltips. Skipped when `orig/` isn't there. With
//! `ENDFORMS_PPM=dir` it also writes each form, as the screen shows it,
//! into `dir`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use formats::bmp::Bitmap;
use formats::data::{animdat1, animdat4, darstell, fontdat, groundnr, hausdata, hausnr, hausprei, modatyp, sonstige};
use formats::dfm::Object;
use formats::highscore::HighScores;
use formats::ne::{RT_RCDATA, ResId, resources};
use formats::savegame::SaveGame;
use game::auction;
use game::command::{self, Command};
use game::endeform::{self, Endeform};
use game::helpform::{self, Helpform, button_bar_tooltip};
use game::highscoreform::{self, Highscoreform, ScoreTable, date_text, highscor_lines};
use game::hintergrund::{SCENARIO_FILES, load_world};
use game::monate::Date;
use game::schlussform::{self, Action, Results, Schlussform, Stage, schluss_lines};
use game::simulation::GameOver;
use game::startform::Settings;
use game::text::Font;
use game::widgets::ShiftState;
use game::world::{Tables, World};
use vcl::form::{Form, Kind};
use vcl::random::Random;
use vcl::surface::{Rop, Surface};
use vcl::{Palette, Rect};

const SCREEN: (i32, i32) = (800, 600);

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

fn surfaces(files: &[&str]) -> (Vec<Surface>, Palette) {
    let bitmaps: Vec<Bitmap> = files.iter().map(|f| bitmap(f)).collect();
    (bitmaps.iter().map(Surface::from_bitmap).collect(), bitmaps[0].palette)
}

fn font() -> Font {
    Font::new(
        &bitmap("font.bmp"),
        &fontdat(&read("DATA/FONTDAT1.DAT")).unwrap(),
        &fontdat(&read("DATA/FONTDAT2.DAT")).unwrap(),
    )
}

fn tables() -> Arc<Tables> {
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

/// A tick, leaving every auction at once.
fn step(world: &mut World) -> Option<GameOver> {
    let tick = world.tick();
    if world.auction.open {
        let _ = world.apply(Command {
            player: 1,
            action: command::Action::Auction(auction::Action::Leave),
        });
    }
    tick.game_over
}

/// The "Geld" scenario (1) played for a while, then to the next 2nd of
/// January from the last day of `year`; `rich` gives the human the
/// scenario's 50 000 000.
fn play(year: u16, rich: bool) -> (World, GameOver) {
    let map = SaveGame::parse(&read(&format!("SZENARIO/{}", SCENARIO_FILES[0]))).unwrap();
    let settings = Settings {
        scenario: 1,
        difficulty: 1,
        ..Settings::default()
    };
    let mut world = load_world(&map, &settings, true).unwrap();
    world.tables = tables();
    world.random = Random::new(0x1996);
    for _ in 0..3000 {
        assert_eq!(step(&mut world), None);
    }
    world.date = Date {
        day: 31,
        month: 12,
        year,
    };
    if rich {
        world.finances.money[0] = 60_000_000;
    }
    for _ in 0..5000 {
        if let Some(over) = step(&mut world) {
            return (world, over);
        }
    }
    panic!("the game didn't end");
}

/// A DFM's form with `components` (name, bounds, picture) shown, painted
/// on the screen and cut to `bounds`.
fn paint(class: &[u8], bounds: Rect, components: &[(&str, Rect, Option<&Surface>)], palette: &Palette) -> Surface {
    let exe = read("HOLIDAY.EXE");
    let res = resources(&exe)
        .unwrap()
        .into_iter()
        .find(|r| r.kind == ResId::Id(RT_RCDATA) && r.name == ResId::Name(class.to_vec()))
        .unwrap();
    let mut vcl = Form::load(&Object::parse(res.data).unwrap()).unwrap();
    vcl.components[0].bounds = bounds;
    vcl.components[0].visible = true;
    for c in vcl.components.iter_mut().skip(1) {
        c.visible = false;
    }
    for (name, r, picture) in components {
        let c = vcl.component_mut(name).unwrap();
        c.bounds = *r;
        c.visible = true;
        if let (Kind::Image { picture: p }, Some(s)) = (&mut c.kind, picture) {
            *p = Some((*s).clone());
        }
    }
    let mut screen = Surface::new(800, 600, 0);
    vcl.paint(&mut screen, palette);
    let mut out = Surface::new(
        usize::try_from(bounds.width()).unwrap(),
        usize::try_from(bounds.height()).unwrap(),
        0,
    );
    let clip = out.bounds();
    out.blit(0, 0, &screen, bounds, Rop::Copy, clip);
    out
}

fn write_ppm(name: &str, s: &Surface, palette: &Palette) {
    let Some(dir) = std::env::var_os("ENDFORMS_PPM") else {
        return;
    };
    let mut ppm = format!("P6\n{} {}\n255\n", s.width, s.height).into_bytes();
    for &i in &s.pixels {
        let c = palette[usize::from(i)];
        ppm.extend_from_slice(&[c.r, c.g, c.b]);
    }
    std::fs::write(Path::new(&dir).join(format!("{name}.ppm")), ppm).unwrap();
}

fn have_orig() -> bool {
    let ok = orig().join("GRAFIK").exists();
    if !ok {
        eprintln!("skipping: {} not found", orig().display());
    }
    ok
}

fn sonstige_texts() -> formats::data::Sonstige {
    sonstige(&read("TEXTE/SONSTIGE.TXT")).unwrap()
}

/// Money as the game shows it in Germany.
fn results(world: &World, over: &GameOver) -> Results {
    let factor = sonstige_texts().waehrungsfaktor;
    Results::of(world, over, factor, modatyp(&read("DATA/MODATYP.DAT")).unwrap().land)
}

fn new_schlussform() -> (Schlussform, Palette) {
    let (bitmaps, palette) = surfaces(&schlussform::BITMAP_FILES);
    (
        Schlussform::new(font(), bitmaps, schluss_lines(&read("TEXTE/SCHLUSS.TXT"))),
        palette,
    )
}

fn new_highscoreform(table: &HighScores) -> (Highscoreform, Palette) {
    let (bitmaps, palette) = surfaces(&highscoreform::BITMAP_FILES);
    (
        Highscoreform::new(
            font(),
            bitmaps,
            highscor_lines(&read("TEXTE/HIGHSCOR.TXT")),
            ScoreTable::new(table, None),
        ),
        palette,
    )
}

#[test]
fn a_won_game_ends_with_a_high_score() {
    if !have_orig() {
        return;
    }
    let (world, over) = play(2005, true);
    assert!(over.won && world.won);
    let score = over.score.unwrap();
    assert!((0..=100).contains(&score));
    let r = results(&world, &over);
    assert_eq!(r.score, Some(score));

    // The film, then the results.
    let (mut form, palette) = new_schlussform();
    let open = form.open_end(r.clone(), true, SCREEN);
    assert_eq!((open.film, open.music), (Some("ABgeld.avi"), Some("gewonn.mid")));
    assert!(open.stop_background_music);
    assert_eq!(form.stage, Stage::Film);
    let film = paint(b"TSCHLUSSFORM", form.bounds(), &form.components(), &palette);
    write_ppm("schlussform-film", &film, &palette);
    let _ = form.key_down(SCREEN);
    assert_eq!(form.stage, Stage::Results);
    assert_eq!(form.bounds(), Rect::new(184, 140, 432, 320));
    let texts = schlussform::schluss_load_texts(&schluss_lines(&read("TEXTE/SCHLUSS.TXT")), &r);
    assert_eq!(texts.labels[3], b"Sie haben genug Geld verdient.");
    assert!(texts.labels[4].starts_with(b"Ihr Verm"));
    assert!(texts.labels[6].ends_with(format!(" {score} Punkte erreicht.").as_bytes()));
    let page = paint(b"TSCHLUSSFORM", form.bounds(), &form.components(), &palette);
    write_ppm("schlussform-won", &page, &palette);

    // The score goes into the developers' test table, scenario 1's.
    let test = HighScores::parse(&read("SOUNDLO/HIGHSCOR.WAV")).unwrap();
    let (mut high, hpalette) = new_highscoreform(&test);
    let name = world.player_names[1].clone();
    let date = date_text(17, 10, 2026, game::lang::Language::De);
    let r2 = high.add(world.scenario, &name, &date, &r.net_worth, score);
    assert!(r2.save);
    let rank = (0..10)
        .find(|&rank| high.table.entries[rank][1].name.as_bytes() == name)
        .unwrap();
    assert_eq!(high.table.entries[rank][1].score, score);
    assert_eq!(high.table.entries[rank][1].money_text.as_bytes(), r.net_worth);
    let scores: Vec<_> = (0..10).map(|r| high.table.entries[r][1].score).collect();
    assert!(scores.windows(2).all(|w| w[0] >= w[1]), "{scores:?}");

    // "High-Score" opens the table on the scenario's page.
    let button = |i: i32| (0xf + i * 104 + 40, 0x126 + 10);
    let (x, y) = button(1);
    let _ = form.mouse_down(x, y);
    assert_eq!(form.mouse_up(x, y).action, Some(Action::HighScores));
    let shown = high.show(world.scenario, false, SCREEN);
    assert!(shown.save && high.visible);
    assert_eq!((high.game, high.left, high.top), (1, 80, 150));
    let table = paint(
        b"THIGHSCOREFORM",
        high.bounds(),
        &[(highscoreform::IMAGE, Rect::new(0, 0, 640, 300), Some(high.image()))],
        &hpalette,
    );
    write_ppm("highscoreform", &table, &hpalette);
    // The standard game's box ("Standardspiel", at 392, 237).
    let r3 = high.mouse_down(400, 240, ShiftState::LEFT);
    assert!(r3.refresh);
    assert_eq!(high.game, 0);
    // "Schließen".
    let _ = high.mouse_down(0x210 + 10, 0x109 + 5, ShiftState::LEFT);
    assert_eq!(
        high.mouse_up(0x210 + 10, 0x109 + 5).action,
        Some(highscoreform::Action::Close)
    );

    // "Weiterspielen" goes on, the music with it.
    let (x, y) = button(0);
    let _ = form.mouse_down(x, y);
    let cont = form.mouse_up(x, y);
    assert_eq!(cont.action, Some(Action::Continue { restart_music: true }));
    assert!(cont.stop_music && !form.visible());
}

#[test]
fn a_lost_game_and_quitting() {
    if !have_orig() {
        return;
    }
    let (world, over) = play(2016, false);
    assert!(!over.won && over.score.is_none());
    let r = results(&world, &over);
    let (mut form, palette) = new_schlussform();
    let open = form.open_end(r, false, SCREEN);
    assert_eq!((open.film, open.music), (Some("ABPleite.avi"), Some("pleite.mid")));
    let _ = form.film_ended(SCREEN);
    let page = paint(b"TSCHLUSSFORM", form.bounds(), &form.components(), &palette);
    write_ppm("schlussform-lost", &page, &palette);
    // No "Weiterspielen".
    let (x, y) = (0xf + 40, 0x126 + 10);
    assert_eq!(form.mouse_down(x, y).sounds, []);
    // "Beenden": the quit film, then the exit.
    let (x, y) = (0xdf + 40, 0x126 + 10);
    let _ = form.mouse_down(x, y);
    let quit = form.mouse_up(x, y);
    assert_eq!(quit.film, Some(schlussform::QUIT_FILM));
    assert_eq!(form.film_ended(SCREEN).action, Some(Action::Exit));
}

#[test]
fn the_quit_question() {
    if !have_orig() {
        return;
    }
    let (bitmaps, palette) = surfaces(&endeform::BITMAP_FILES);
    let mut form = Endeform::new(font(), bitmaps, sonstige_texts().texts);
    assert!(form.open(SCREEN));
    // "OK" held down.
    let r = form.mouse_down(120, 65, true);
    assert!(r.refresh);
    let out = paint(
        b"TENDEFORM",
        form.bounds(),
        &[(
            endeform::IMAGE,
            Rect::new(0, 0, endeform::WIDTH, endeform::HEIGHT),
            Some(form.image()),
        )],
        &palette,
    );
    write_ppm("endeform", &out, &palette);
    assert_eq!(form.mouse_up(120, 65, true).action, Some(endeform::Action::Quit));
    // Then the quit film.
    let (mut end, _) = new_schlussform();
    assert_eq!(end.open_quit(SCREEN).film, Some(schlussform::QUIT_FILM));
    assert_eq!(end.key_down(SCREEN).action, Some(Action::Exit));
}

#[test]
fn the_button_bar_tooltips() {
    if !have_orig() {
        return;
    }
    let texts = sonstige_texts().short_texts;
    let bubble = Surface::from_bitmap(&bitmap(helpform::BITMAP_FILE));
    let palette = bitmap(helpform::BITMAP_FILE).palette;
    let mut help = Helpform::new(font(), bubble, texts.clone());
    // The main screen at 80, 60 on 800 × 600; the mouse on the build
    // menu's "Häuser" (button 9).
    let main = Rect::new(80, 60, 640, 480);
    let (text, top, left) = button_bar_tooltip(main, (80 + 270, 60 + 40), 270, 0, true).unwrap();
    assert_eq!(texts[usize::from(text)], b"H\xe4user");
    assert!(help.show(text, top, left, SCREEN));
    assert!(!help.show(text, top, left, SCREEN), "already shown");
    assert_eq!(help.bounds(), Rect::new(80 + 264, 60 + 0x3b, 150, 16));
    let out = paint(
        b"THELPFORM",
        help.bounds(),
        &[(helpform::IMAGE, Rect::new(0, 0, 150, 16), Some(help.image()))],
        &palette,
    );
    write_ppm("helpform", &out, &palette);
    assert!(help.hide() && !help.visible());
}
