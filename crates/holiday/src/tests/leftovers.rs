//! The leftovers the docs listed as not ported: the forms' tooltips, the
//! earthquake's shaking, the progress window's gauge, the TV archive's
//! dates in the player's language and the money menu's saved page. With
//! `HOLIDAY_PNG` set, the screens go there as `leftovers-*.png`.

// The program, its game and the forms, as in the other tests.
#![allow(clippy::many_single_char_names)]

use super::*;

/// The first point of `area` where `f` finds something.
fn find_in<T>(area: Rect, f: impl Fn(i32, i32) -> Option<T>) -> Option<((i32, i32), T)> {
    (area.left..area.right).find_map(|x| (area.top..area.bottom).find_map(|y| f(x, y).map(|t| ((x, y), t))))
}

/// The money menu's tabs and the overview map's colourings show their
/// tooltips under them from the main screen's clock (1020:1e9b,
/// 1020:1fe2) while their form has the focus, with the tooltips on.
#[test]
fn form_tooltips() {
    let Some(dir) = game_dir() else { return };
    // On 1280 × 1024 the main screen leaves the forms at the edges free.
    let mut h = holiday_in_a_game(&dir, (1280, 1024), Config::ORIGINAL);
    let g = h.game.as_mut().unwrap();
    let r = g.forms.tv.close();
    g.tv_response(r);
    g.forms.settings.button_help = true;

    // The money menu's tab "Bilanz".
    let m = &g.forms.money;
    let (at, (text, left, top)) = find_in(m.bounds(), |x, y| m.tooltip(x, y, true)).unwrap();
    assert!(game::helpform::MONEY_MENU_TEXTS.contains(&text));
    g.set_focus(Focus::Money);
    g.mouse = at;
    tick(g, 1);
    assert!(g.forms.help_visible());
    assert_eq!((g.forms.help.left, g.forms.help.top), (left, top));
    h.repaint();
    write_png(&h, "leftovers-tooltip-money");

    // Without the focus, or with the option off, none.
    let g = h.game.as_mut().unwrap();
    g.set_focus(Focus::Main);
    tick(g, 1);
    assert!(!g.forms.help_visible());

    // The overview map's colourings.
    let o = &g.forms.overview;
    let menu = o
        .images()
        .iter()
        .find(|(i, _, _)| *i == game::uebersichtsmap::Image::Menu)
        .map(|&(_, r, _)| r)
        .unwrap();
    let (ol, ot) = (o.left, o.top);
    let area = Rect::new(ol + menu.left, ot + menu.top, menu.width(), menu.height());
    let (at, (text, left, bottom)) = find_in(area, |x, y| {
        o.help_text_at(x - o.left - menu.left, y - o.top - menu.top)
    })
    .unwrap();
    assert!(game::helpform::OVERVIEW_TEXTS.contains(&text));
    g.set_focus(Focus::Overview);
    g.mouse = at;
    tick(g, 1);
    assert!(g.forms.help_visible());
    let help = &g.forms.help;
    assert_eq!((help.left, help.top), (left + menu.left + ol, bottom + menu.top + ot));
    h.repaint();
    write_png(&h, "leftovers-tooltip-overview");
    let g = h.game.as_mut().unwrap();
    g.forms.settings.button_help = false;
    tick(g, 1);
    assert!(!g.forms.help_visible());
}

/// An earthquake shakes the main screen by the world's `Random(7) - 3`
/// every tick (en 1020:2367), from where it began, and its end puts the
/// main screen back there and the TV where it was last hidden
/// (en 1028:05f7). With `HOLIDAY_PNG` set, four frames of the shaking,
/// ten ticks apart, go there as `leftovers-quake-N.png`.
#[test]
fn the_earthquake_shakes_the_screen() {
    let Some(dir) = game_dir() else { return };
    let mut h = holiday_in_a_game(&dir, (800, 600), Config::ORIGINAL);
    let g = h.game.as_mut().unwrap();
    // The money menu and the overview out of the way of the picture.
    g.click_button(4);
    g.click_button(5);
    // The TV stays open and is dragged off where it was shown.
    let kept_tv = g.forms.tv.kept;
    g.forms.tv.left += 40;
    let home = (g.screen.left, g.screen.top);
    let _ = g.show(&Tick {
        quake_began: true,
        ..Tick::default()
    });
    g.world.earthquake = true;
    g.world.clock.paused = false;
    let mut at = home;
    for n in 0..40 {
        let g = h.game.as_mut().unwrap();
        let t = g.world.clone().tick();
        let (dx, dy) = t.shake.expect("it shakes");
        assert!((-3..=3).contains(&dx) && (-3..=3).contains(&dy));
        tick(g, 1);
        at = (at.0 + dx, at.1 + dy);
        assert_eq!((g.screen.left, g.screen.top), at, "tick {n}");
        if n % 10 == 9 {
            h.repaint();
            write_png(&h, &format!("leftovers-quake-{}", n / 10));
        }
    }
    let g = h.game.as_mut().unwrap();
    g.world.earthquake = false;
    let _ = g.show(&Tick {
        quake_ended: true,
        ..Tick::default()
    });
    assert_eq!((g.screen.left, g.screen.top), home);
    assert_eq!((g.forms.tv.left, g.forms.tv.top), kept_tv);
    tick(g, 1);
    assert_eq!((g.screen.left, g.screen.top), home, "no more shaking");
}

/// A game loaded in the middle of an earthquake: the original's end puts
/// the main screen where the last one began (here none, 0, 0);
/// `quake_screen` keeps where it was when the game was loaded.
#[test]
fn quake_screen_after_a_load() {
    let Some(dir) = game_dir() else { return };
    for on in [false, true] {
        let config = if on {
            only(enhance::Enhancement::QuakeScreen)
        } else {
            Config::ORIGINAL
        };
        let mut h = holiday_in_a_game(&dir, (800, 600), config);
        let g = h.game.as_mut().unwrap();
        let home = (g.screen.left, g.screen.top);
        g.world.earthquake = true;
        let _ = g.show(&Tick {
            loaded: true,
            ..Tick::default()
        });
        let _ = g.show(&Tick {
            quake_ended: true,
            ..Tick::default()
        });
        let expected = if on { home } else { (0, 0) };
        assert_eq!((g.screen.left, g.screen.top), expected, "quake_screen {on}");
    }
}

/// The TV shows the news archive's dates in the player's language: a
/// world made in German keeps `6. Januar 1996` (as its map file does),
/// and an English player's TV shows `6th January 1996`, the same picture
/// as a world made in English. With `HOLIDAY_PNG` set, the screen goes
/// there as `leftovers-tv-date.png`.
#[test]
fn the_tv_dates_in_the_players_language() {
    use lang::{Language, Setting};
    let Some(dir) = game_dir() else { return };
    let config = Config {
        language: Setting::Is(Language::En),
        ..Config::ORIGINAL
    };
    let mut h = holiday_in_a_game(&dir, (800, 600), config);
    let g = h.game.as_mut().unwrap();
    assert_eq!((g.world.language, g.info.language), (Language::En, Language::En));
    let n = g.forms.tv.cursor;
    let shown = |g: &mut Game| {
        let r = g.forms.tv.show(&g.world, &g.info, 0);
        g.tv_response(r);
        g.forms.tv.images()[1].1.pixels.clone()
    };
    let english = shown(g);
    let index = usize::try_from(n).unwrap();
    let date = g.world.news.entries[index].date.clone();
    assert!(date.ends_with(b" January 1996"), "{date:?}");
    // The same news in a world made in German.
    g.world.language = Language::De;
    let german = game::monate::Date::parse(&date, Language::En)
        .unwrap()
        .text(Language::De.months(), Language::De);
    assert!(german.ends_with(b". Januar 1996"));
    g.world.news.entries[index].date.clone_from(&german);
    assert_eq!(shown(g), english);
    h.repaint();
    write_png(&h, "leftovers-tv-date");
    // The map file keeps what the world wrote.
    let g = h.game.as_ref().unwrap();
    assert_eq!(g.world.news.entries[index].date, german);
}

/// The progress window's gauge moves a step a frame at the original's
/// points (en 10b8:52d4, en 1038:3615, en 1038:33c8), and the game the
/// steps set up is the one set up at once. With `HOLIDAY_PNG` set, the
/// window goes there in the generator (`leftovers-gauge-island.png`, at
/// 48 %) and in `load_game_rest` (`leftovers-gauge-rest.png`, at 0x4d).
#[test]
fn the_gauge_moves_with_the_work() {
    let Some(dir) = game_dir() else { return };
    for scenario in [false, true] {
        let mut h = fixed_holiday(&dir);
        if scenario {
            h.start.menu = start_menu(
                &dir,
                &h.exe,
                Settings {
                    scenario: 3,
                    ..Settings::default()
                },
                h.language(),
            )
            .menu;
        }
        let settings = h.start.menu.settings().clone();
        h.start.hide();
        h.pending = Some(Pending::Start { scenario });
        h.start_pending();
        let mut percents = Vec::new();
        while h.loading.is_some() {
            h.update(Duration::ZERO);
            if let Some(l) = &h.loading {
                percents.push(l.form.percent());
                if !scenario && (l.form.percent() == 48 || l.form.percent() == 0x4d) {
                    let name = if l.form.percent() == 48 { "island" } else { "rest" };
                    write_png(&h, &format!("leftovers-gauge-{name}"));
                }
            }
        }
        let first = if scenario {
            vec![100]
        } else {
            vec![5, 26, 32, 48, 51, 70, 74, 100]
        };
        let mut expected = first;
        expected.extend([
            1, 7, 50, 0x37, 0x41, 0x43, 0x48, 0x4b, 0x4d, 0x50, 0x52, 0x55, 0x60, 0x62, 0x63, 100, 100,
        ]);
        assert_eq!(percents, expected, "scenario {scenario}");
        // The same game as the set-up in one go.
        let staged = h.game.take().unwrap();
        let high = forms::high_scores(&dir, &h.saves.high_scores());
        let once = start_game_seeded(
            &dir,
            &h.exe,
            &settings,
            scenario,
            h.options,
            high,
            &h.sound,
            h.size,
            &h.enhance,
            (h.seed)(),
        );
        assert_eq!(world_hash(&staged.world), world_hash(&once.world));
        assert_eq!(staged.forms.settings, once.forms.settings);
        assert_eq!(staged.screen.camera(), once.screen.camera());
    }
}

/// `TSchlussform.Timer1` looks at the film once a second (1068:187f): the
/// quit film's last frame stays until it fires.
#[test]
fn the_end_forms_timer_fires_once_a_second() {
    let Some(dir) = game_dir() else { return };
    if !dir.join("FILMHI").exists() {
        return;
    }
    let mut h = holiday_in_a_game(&dir, (800, 600), Config::ORIGINAL);
    let g = h.game.as_mut().unwrap();
    let r = g.forms.schluss.open_quit(g.size);
    g.schluss_response(r);
    assert!(g.forms.schluss.film_panel().is_some());
    // Past the film's end.
    g.sound.borrow_mut().advance(600_000);
    assert!(g.sound.borrow().film_done());
    let _ = g.update(Duration::from_millis(600));
    assert!(!matches!(g.forms.requests.last(), Some(Request::Exit)), "not yet");
    let _ = g.update(Duration::from_millis(400));
    assert!(matches!(g.forms.requests.last(), Some(Request::Exit)));
}
