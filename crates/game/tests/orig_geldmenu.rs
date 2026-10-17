//! The money menu on a scenario's island after a year and some months:
//! each of its commands applied to the world (the loan, advertising,
//! renovation, insurance, showing it), its buttons, tabs and keys, and the
//! year's total while it's shown. Skipped when `orig/` isn't there. With
//! `GELDMENU_PPM=dir` it also writes each page, as the screen shows it,
//! to `dir/geldmenu-<page>.ppm`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use formats::bmp::Bitmap;
use formats::data::{
    animdat1, animdat4, darstell, fontdat, geldmenu_dat, geldmenu_txt, groundnr, hausdata, hausnr, hausprei, modatyp,
    modatyp_txt, monate, sonstige,
};
use formats::dfm::Object;
use formats::ne::{RT_RCDATA, ResId, resources};
use formats::savegame::SaveGame;
use game::auction;
use game::command::{Action, Command};
use game::geldmenu::{self, BITMAP_FILES, Geldmenuform, Response, Texts};
use game::hauptschirm::InfoTexts;
use game::hintergrund::{SCENARIO_FILES, load_world};
use game::simulation::Tick;
use game::startform::Settings;
use game::text::Font;
use game::widgets::ShiftState;
use game::world::{Pos, Tables, World};
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

/// "Geld": the human builds a few houses, then the game runs into the
/// second year, past its first year's end.
fn world() -> World {
    let file = SCENARIO_FILES[0];
    let map = SaveGame::parse(&read(&format!("SZENARIO/{file}"))).unwrap();
    let settings = Settings {
        scenario: 1,
        difficulty: 1,
        ..Settings::default()
    };
    let mut w = load_world(&map, &settings, true).unwrap();
    w.tables = tables();
    w.random = Random::new(0x6e1d_1996);
    w.clock.paused = false;
    let house = u8::try_from(w.build_menu.rows[0][1][0]).unwrap();
    let mut built = 0;
    'scan: for i in (20..230).step_by(3) {
        for j in (20..230).step_by(3) {
            let at: Pos = (i, j);
            if w.can_build(at, house, 1).0 == 0 {
                let tick = w.apply(human(Action::Build { building: house, at }));
                assert!(tick.books);
                built += 1;
                if built == 6 {
                    break 'scan;
                }
            }
        }
    }
    assert_eq!(built, 6);
    let year = w.date.year;
    let mut new_year = false;
    while w.date.year == year || w.date.month < 4 {
        new_year |= step(&mut w).new_year;
    }
    assert!(new_year);
    w
}

fn texts() -> Texts {
    let s = sonstige(&read("TEXTE/SONSTIGE.TXT")).unwrap();
    let (geldmenu, close) = geldmenu_txt(&read("TEXTE/GELDMENU.TXT")).unwrap();
    let (advertising, renovation) = modatyp_txt(&read("TEXTE/MODATYP.TXT"));
    Texts {
        geldmenu,
        close,
        info: InfoTexts {
            months: monate(&read("TEXTE/MONATE.TXT")).unwrap(),
            land: modatyp(&read("DATA/MODATYP.DAT")).unwrap().land,
            currency_factor: s.waehrungsfaktor,
            currency: s.texts[5].clone(),
        },
        sonstige: s.texts,
        short: s.short_texts,
        advertising,
        renovation,
    }
}

fn form() -> Geldmenuform {
    let font = Font::new(
        &bitmap("FONT.BMP"),
        &fontdat(&read("DATA/FONTDAT1.DAT")).unwrap(),
        &fontdat(&read("DATA/FONTDAT2.DAT")).unwrap(),
    );
    let bitmaps = BITMAP_FILES.iter().map(|f| Surface::from_bitmap(&bitmap(f))).collect();
    let rects = geldmenu_dat(&read("DATA/GELDMENU.DAT")).unwrap();
    Geldmenuform::new(font, bitmaps, rects, texts(), 800)
}

/// Applies a response's commands for the human, then redraws the page if
/// it asks, as the game would.
fn apply(w: &mut World, f: &mut Geldmenuform, r: &Response) -> Vec<Tick> {
    let ticks = r.commands.iter().map(|a| w.apply(human(a.clone()))).collect();
    if r.update {
        f.update(w);
    }
    ticks
}

/// The middle of `GELDMENU.DAT`'s rectangle `n` in the form, on the image
/// placed by rectangle `place`.
fn at(n: usize, place: usize) -> (i32, i32) {
    let r = geldmenu_dat(&read("DATA/GELDMENU.DAT")).unwrap();
    let (p, e) = (r[place], r[n]);
    (
        i32::from(p[0]) + i32::midpoint(i32::from(e[0]), i32::from(e[2])),
        i32::from(p[1]) + i32::midpoint(i32::from(e[1]), i32::from(e[3])),
    )
}

/// A click of the left button at `p` in the form, with what its press
/// and its release did, the commands applied.
fn click(w: &mut World, f: &mut Geldmenuform, p: (i32, i32)) -> (Response, Response) {
    let down = f.mouse_down(p.0, p.1, true, ShiftState::LEFT);
    apply(w, f, &down);
    let up = f.mouse_up(p.0, p.1, true);
    apply(w, f, &up);
    (down, up)
}

const BILD: usize = 2;
/// The insurance's check box (1070:2e6c), on the page at (0xf2, 0x7c).
const CHECK_BOX: (i32, i32) = (0xf2 + 5, 16 + 0x7c + 5);
const MENU: usize = 3;

fn mm(a: geldmenu::Action) -> Action {
    Action::MoneyMenu(a)
}

#[test]
#[allow(clippy::too_many_lines)] // One scripted visit to the money menu.
fn the_money_menu() {
    if !orig().join("SZENARIO").exists() {
        eprintln!("skipping: {} not found", orig().display());
        return;
    }
    let mut w = world();
    let mut f = form();
    let out = std::env::var_os("GELDMENU_PPM").map(PathBuf::from);
    assert!(!f.shown());
    assert_eq!((f.left, f.top, f.width()), (360, 0, 441));
    // Last year's books: the houses' costs and the interest.
    assert!(w.book_value(0x56) < 0 && w.book_value(0x5c) < 0);
    assert_ne!(w.book_value(0x5e), 0);

    // Button 4 opens it at page 0.
    let r = f.show(0, 4);
    assert_eq!(r.commands, vec![mm(geldmenu::Action::Shown(true))]);
    assert_eq!(r.sounds, vec![0xd, 0xd]);
    assert!(r.opened && r.update);
    apply(&mut w, &mut f, &r);
    assert!(w.money_menu_shown && f.shown());
    assert_eq!(f.height, 385);
    let this_year: i32 = (0x5f..=0x6a).map(|k| w.book_value(k)).sum();
    assert_eq!(w.book_value(0x6b), this_year);
    let money = w.finances.money[0];
    let loan = w.finances.loan[0];
    assert_eq!(
        [w.book_value(0x6c), w.book_value(0x6d), w.book_value(0x6e)],
        [money, -loan, money - loan]
    );
    let page0 = f.bild.clone();

    // "Geld leihen" until the bank says no.
    let size = w.finances.credit_size;
    assert!(size > 0);
    let (down, up) = click(&mut w, &mut f, at(0x17, BILD));
    assert_eq!((down.sounds, up.sounds.clone()), (vec![8], vec![10]));
    assert_eq!(up.commands, vec![mm(geldmenu::Action::TakeLoan)]);
    assert_eq!(w.finances.money[0], money + size);
    assert_eq!(w.finances.loan[0], loan + size);
    assert_eq!(w.book_value(0x6c), money + size);
    assert_eq!(w.book_value(0x6d), -(loan + size));
    assert_ne!(f.bild, page0, "the balance redrawn");
    let mut n = 1;
    while !w.finances.credit_refused[0] {
        let _ = click(&mut w, &mut f, at(0x17, BILD));
        n += 1;
        assert!(n < 1000);
    }
    assert!(w.finances.loan[0] <= w.finances.max_debt);
    assert!(w.finances.loan[0] + size > w.finances.max_debt);
    let refused = w.clone();
    let _ = click(&mut w, &mut f, at(0x17, BILD));
    assert_eq!(w.finances, refused.finances, "the bank says no");
    // Let go elsewhere: nothing.
    let _ = f.mouse_down(at(0x18, BILD).0, at(0x18, BILD).1, true, ShiftState::LEFT);
    let r = f.mouse_up(5, 100, true);
    assert_eq!(r.commands, vec![]);
    // "Kredit tilgen" pays a step back and clears the no.
    let (money, loan) = (w.finances.money[0], w.finances.loan[0]);
    let (_, up) = click(&mut w, &mut f, at(0x18, BILD));
    assert_eq!(up.commands, vec![mm(geldmenu::Action::RepayLoan)]);
    assert_eq!((w.finances.money[0], w.finances.loan[0]), (money - size, loan - size));
    assert!(!w.finances.credit_refused[0]);
    // Without the money, nothing.
    let mut poor = w.clone();
    poor.finances.money[0] = size - 1;
    let before = poor.finances.clone();
    poor.apply(human(mm(geldmenu::Action::RepayLoan)));
    assert_eq!(poor.finances, before);

    // Advertising: up to the most, and what it costs.
    assert_eq!(w.advertising_level(1), 0);
    for _ in 0..3 {
        let _ = click(&mut w, &mut f, at(0x15, BILD));
    }
    assert_eq!(w.advertising_level(1), 3);
    assert!(w.advertising_cost(1) > 0);
    let _ = click(&mut w, &mut f, at(0x14, BILD));
    assert_eq!(w.advertising_level(1), 2);
    // Renovation: from never, every five years, then four.
    assert_eq!(w.renovation_years[0], 0);
    let _ = click(&mut w, &mut f, at(0x13, BILD));
    let _ = click(&mut w, &mut f, at(0x13, BILD));
    assert_eq!(w.renovation_years[0], 4);
    let _ = click(&mut w, &mut f, at(0x12, BILD));
    assert_eq!(w.renovation_years[0], 5);
    // Insurance: the check box.
    assert!(!w.insured);
    let (down, _) = click(&mut w, &mut f, CHECK_BOX);
    assert_eq!(down.commands, vec![mm(geldmenu::Action::Insurance(true))]);
    assert_eq!(down.sounds, vec![9]);
    assert!(w.insured && w.insurance_cost() > 0);
    let insured_page = f.bild.clone();
    let (down, _) = click(&mut w, &mut f, CHECK_BOX);
    assert_eq!(down.commands, vec![mm(geldmenu::Action::Insurance(false))]);
    assert!(!w.insured);
    let _ = click(&mut w, &mut f, CHECK_BOX);
    assert_eq!(f.bild, insured_page);

    // While it's shown the bookings work the year's total out.
    let mut booked = 0;
    for _ in 0..3000 {
        let tick = step(&mut w);
        if tick.books {
            booked += 1;
            f.update(&w);
            let this_year: i32 = (0x5f..=0x6a).map(|k| w.book_value(k)).sum();
            assert_eq!(w.book_value(0x6b), this_year);
        }
    }
    assert!(booked > 0);
    let page_shots = |f: &Geldmenuform, page: u8| {
        if let Some(dir) = &out {
            write_ppm(&dir.join(format!("geldmenu-{page}.ppm")), &paint(f), &palette());
        }
    };
    page_shots(&f, 0);

    // The tabs and keys turn the pages.
    let (down, _) = click(&mut w, &mut f, at(0xa, MENU));
    assert_eq!(down.sounds, vec![9]);
    assert_eq!(f.page(), 1);
    page_shots(&f, 1);
    // The same tab again: nothing.
    let (down, _) = click(&mut w, &mut f, at(0xa, MENU));
    assert!(down.sounds.is_empty() && !down.update);
    for page in 2..=5u8 {
        let r = f.key_down(0x31 + u16::from(page), ShiftState::default());
        assert!(r.update && r.sounds.is_empty());
        apply(&mut w, &mut f, &r);
        assert_eq!(f.page(), page);
        page_shots(&f, page);
    }
    let r = f.key_down(0x27, ShiftState::default());
    apply(&mut w, &mut f, &r);
    assert_eq!(f.page(), 0);
    let r = f.key_down(0x25, ShiftState::SHIFT);
    apply(&mut w, &mut f, &r);
    assert_eq!(f.page(), 5);
    // Other keys go to the main screen; Space gives it the focus.
    let r = f.key_down(0x48, ShiftState::default());
    assert_eq!(r.main_screen_key, Some((0x48, ShiftState::default())));
    assert!(f.key_down(0x20, ShiftState::default()).focus_main);
    assert_eq!(f.key_down(0x48, ShiftState::CTRL), Response::default());
    // The tooltips of the tabs.
    let (tx, ty) = at(0xb, MENU);
    let tip = f.tooltip(f.left + tx, f.top + ty, true).unwrap();
    assert_eq!(tip.0, 0x1f);
    assert_eq!(f.tooltip(f.left + tx, f.top + ty, false), None);

    // "Gebäudeübersicht" opens the overview.
    let (down, up) = click(&mut w, &mut f, (60, at(0xf, MENU).1));
    assert_eq!((down.sounds, up.sounds), (vec![8], vec![10]));
    assert!(up.overview && !down.overview);

    // Dragged by the title bar: rolled up meanwhile, and kept on the screen.
    let r = f.mouse_down(100, 5, true, ShiftState::LEFT);
    assert!(r.moved);
    assert_eq!(f.height, 16);
    let r = f.mouse_move(50, 20, ShiftState::LEFT, (800, 600));
    assert!(r.moved);
    assert_eq!((f.left, f.top), (310, 15));
    let _ = f.mouse_move(5000, -100, ShiftState::LEFT, (800, 600));
    assert_eq!((f.left, f.top), (800 - 441, 0));
    let _ = f.mouse_up(5000, -100, true);
    assert_eq!(f.height, 387, "the full height after a drag");
    // A double click leaves it rolled up.
    let _ = f.mouse_down(100, 5, true, ShiftState::LEFT | ShiftState::DOUBLE);
    let _ = f.mouse_up(100, 5, true);
    assert_eq!(f.height, 16);
    assert_eq!(f.tooltip(f.left + tx, f.top + ty, true), None);
    let _ = f.mouse_down(100, 5, true, ShiftState::LEFT);
    let _ = f.mouse_up(100, 5, true);
    assert_eq!(f.height, 387);

    // Escape closes it: the books stop following.
    let r = f.key_down(0x1b, ShiftState::default());
    assert!(r.closed);
    assert_eq!(r.sounds, vec![0xe]);
    assert_eq!(r.commands, vec![mm(geldmenu::Action::Shown(false))]);
    apply(&mut w, &mut f, &r);
    assert!(!w.money_menu_shown && !f.shown());
    let total = w.book_value(0x6b);
    while !step(&mut w).books {}
    assert_eq!(w.book_value(0x6b), total);
    // The balance sheet opens with its own sound; its close button closes it.
    let r = f.show(1, 4);
    assert_eq!(r.sounds, vec![0xd, 0x25]);
    apply(&mut w, &mut f, &r);
    let this_year: i32 = (0x5f..=0x6a).map(|k| w.book_value(k)).sum();
    assert_eq!(w.book_value(0x6b), this_year);
    let (down, up) = click(&mut w, &mut f, at(5, 1));
    assert_eq!(down.sounds, vec![8]);
    assert_eq!(up.sounds, vec![10, 0xe]);
    assert!(up.closed && !w.money_menu_shown);
    // And "Schließen" on the menu bar.
    let r = f.show(1, 4);
    apply(&mut w, &mut f, &r);
    let (_, up) = click(&mut w, &mut f, at(0xf, MENU));
    assert!(up.closed && !w.money_menu_shown);
    let r = f.show(0, 4);
    apply(&mut w, &mut f, &r);
    assert!(f.key_down(0x73, ShiftState::ALT).closed);
}

#[test]
fn deterministic_commands() {
    if !orig().join("SZENARIO").exists() {
        eprintln!("skipping: {} not found", orig().display());
        return;
    }
    let run = || {
        let mut w = world();
        for a in [
            geldmenu::Action::Shown(true),
            geldmenu::Action::TakeLoan,
            geldmenu::Action::Advertising { more: true },
            geldmenu::Action::Insurance(true),
            geldmenu::Action::Renovation { more: true },
        ] {
            w.apply(human(mm(a)));
        }
        for _ in 0..2000 {
            step(&mut w);
        }
        w
    };
    assert_eq!(run(), run());
}

fn palette() -> Palette {
    bitmap("GELDFIN.BMP").palette
}

/// The form on an 800 × 600 screen, through the VCL form from the DFM.
fn paint(f: &Geldmenuform) -> Surface {
    let exe = read("HOLIDAY.EXE");
    let res = resources(&exe)
        .unwrap()
        .into_iter()
        .find(|r| r.kind == ResId::Id(RT_RCDATA) && r.name == ResId::Name(b"TGELDMENUFORM".to_vec()))
        .unwrap();
    let mut vcl = Form::load(&Object::parse(res.data).unwrap()).unwrap();
    vcl.components[0].bounds = f.bounds();
    vcl.components[0].visible = true;
    for ((name, bounds), (_, image)) in f.layout().into_iter().zip(f.images()) {
        let c = vcl.component_mut(name).unwrap();
        c.bounds = bounds;
        c.visible = true;
        if let Kind::Image { picture } = &mut c.kind {
            *picture = Some(image.clone());
        }
    }
    let mut out = Surface::new(800, 600, 0);
    vcl.paint(&mut out, &palette());
    out
}

fn write_ppm(path: &Path, s: &Surface, palette: &Palette) {
    let mut ppm = format!("P6\n{} {}\n255\n", s.width, s.height).into_bytes();
    for &i in &s.pixels {
        let c = palette[usize::from(i)];
        ppm.extend_from_slice(&[c.r, c.g, c.b]);
    }
    std::fs::write(path, ppm).unwrap();
}
