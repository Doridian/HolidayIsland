//! The forms' windows a map file keeps (1010:08d7, 1010:0799): loading
//! puts them back, from the start menu and from the file menu, kept on
//! the screen; saving writes the forms' own. Skipped without the game's
//! files in `orig/`.

// The program, its game and the forms, as in the other tests.
#![allow(clippy::many_single_char_names)]

use game::savegame::{Window, Windows, window};

use super::*;

/// A map file of the game's.
fn map_file(dir: &Path, name: &str) -> SaveGame {
    SaveGame::parse(&read(dir, name)).unwrap()
}

/// The `n`th field at `dest` of `map` becomes `data`.
fn set_field(map: &mut SaveGame, dest: &str, n: usize, data: &[u8]) {
    let field = map
        .units
        .iter_mut()
        .flat_map(|u| &mut u.fields)
        .filter(|f| f.dest == dest)
        .nth(n)
        .unwrap();
    field.data = data.to_vec();
}

/// `map` with the windows `windows`.
fn with_windows(mut map: SaveGame, windows: &Windows) -> SaveGame {
    for (n, dest) in [0, 1, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]
        .into_iter()
        .enumerate()
    {
        let w = windows.0[dest];
        let words = [w.top, w.left, w.width, w.height];
        for (d, v) in ["Top", "Left", "Width", "Height"].into_iter().zip(words) {
            set_field(&mut map, &format!("[1110:646c[i]].{d}"), n, &v.to_le_bytes());
        }
        set_field(&mut map, "[1110:646c[i]].Visible", n, &[w.visible]);
        set_field(&mut map, "[1110:646c[i]].Enabled", n, &[w.enabled]);
    }
    map
}

/// A game started from `map` as the load form starts one (states
/// 1000–1500), on a `size` screen.
fn loaded_game(dir: &Path, map: SaveGame, size: (i32, i32), config: Config) -> Game {
    let exe = read(dir, "HOLIDAY.EXE");
    let high = forms::high_scores(dir, Path::new("/nonexistent/HIGHSCOR.WAV"));
    let sound = Sound::new(dir, setup::Sound::Hi, platform::audio::Audio::silent()).shared();
    loading::Setup::new(
        dir,
        &exe,
        Pending::Load(map),
        &Settings::default(),
        Options::default(),
        high,
        &sound,
        size,
        &config,
        seed(),
    )
    .run()
}

/// Loads `map` into `game` as the file menu's "Laden" does (1040:0f27).
fn load_from_the_file_menu(game: &mut Game, map: SaveGame) {
    game.view_rest = MapView::from_map(&map).unported;
    game.forms.loading_settings = Some(game.forms.settings.with_map(&map));
    game.commands.push(Command {
        player: 1,
        action: command::Action::Load(Arc::new(map)),
    });
    let _ = game.update(Duration::ZERO);
}

/// The windows `game` saves, with the file menu open as it saves.
fn saved_windows(game: &mut Game) -> Windows {
    game.file_menu.visible = true;
    let map = game.map();
    game.file_menu.visible = false;
    Windows::from_map(&map).unwrap()
}

/// The tutorial's file, saved on 800×600, loaded from the file menu over
/// a scenario with the TV, the money menu, the overview map and the
/// compass open: each form goes where the file has it and opens or closes
/// as the file has it; the TV closes, which ends its pause. Saved again
/// at once, the windows are the file's, but the compass's, which loading
/// leaves where it is, the money menu's, which the file has a column past
/// the screen's edge (1070:2e6c) and loading keeps on it, and the empty
/// slots': as the original saves them too.
#[test]
fn the_tutorials_windows_come_back() {
    let Some(dir) = game_dir() else { return };
    let mut h = holiday_in_a_game(&dir, (800, 600), Config::ORIGINAL);
    let g = h.game.as_mut().unwrap();
    assert!(g.forms.tv.visible && g.forms.money.shown() && g.forms.overview.visible());
    let compass = g.forms.compass.bounds();
    let map = map_file(&dir, "MAPS/TUTORIAL.MAP");
    let file = Windows::from_map(&map).unwrap();
    load_from_the_file_menu(g, map);
    let _ = g.update(Duration::ZERO);
    assert_eq!((g.screen.left, g.screen.top), (80, 61));
    assert_eq!(
        (g.tools.menu.left, g.tools.menu.top, g.tools.menu.height),
        (313, 119, 0)
    );
    assert!(!g.forms.money.shown() && !g.world.money_menu_shown);
    assert!(!g.forms.overview.visible() && !g.forms.tv.visible);
    assert!(!g.world.clock.paused, "the TV's pause ends with it");
    assert_eq!((g.forms.tv.left, g.forms.tv.top), (165, 55));
    assert_eq!(g.forms.compass.bounds(), compass, "the compass stays");
    let saved = saved_windows(g);
    for form in (0..17).filter(|&f| ![window::COMPASS, window::MONEY_MENU, 10, 11].contains(&f)) {
        assert_eq!(saved.0[form], file.0[form], "form {form}");
    }
    // The CD's files have windows in the empty slots, which the game
    // reads and drops, and saves as zeros (1010:0799).
    assert_ne!(file.0[10], Window::default());
    assert_eq!((saved.0[10], saved.0[11]), (Window::default(), Window::default()));
    assert_eq!(file.0[window::MONEY_MENU].left, 360);
    assert_eq!(
        saved.0[window::MONEY_MENU],
        Window {
            left: 359,
            ..file.0[window::MONEY_MENU]
        }
    );
    assert_eq!(saved.0[window::COMPASS].left, 740);
    assert_eq!(saved.0[window::COMPASS].top, 0);
}

/// `KOENIG.HIS`, saved on 1024×768, loaded on 800×600: the original puts
/// each window where the file has it, moved up and left only as far as
/// the screen's right and bottom edges need (1010:08d7); the compass
/// stays at the top right. With `saved_windows` the main screen stays
/// centred and the others keep their places over it, whole on the
/// screen.
#[test]
fn windows_from_a_bigger_screen() {
    let Some(dir) = game_dir() else { return };
    let map = map_file(&dir, "SZENARIO/KOENIG.HIS");
    let file = Windows::from_map(&map).unwrap();
    assert_eq!(file.0[window::MAIN_SCREEN].rect(), Rect::new(196, 132, 640, 479));
    let o = loaded_game(&dir, map.clone(), (800, 600), Config::ORIGINAL);
    assert_eq!((o.screen.left, o.screen.top), (160, 121));
    assert_eq!((o.tools.menu.left, o.tools.menu.top), (800 - 422, 203));
    assert_eq!((o.forms.money.left, o.forms.money.top), (800 - 441, 0));
    assert_eq!((o.forms.compass.left, o.forms.compass.top), (740, 0));
    assert_eq!((o.forms.help.left, o.forms.help.top), (284, 191));
    let f = loaded_game(&dir, map, (800, 600), only(enhance::Enhancement::SavedWindows));
    assert_eq!((f.screen.left, f.screen.top), (80, 61));
    // Moved by (80 - 196, 61 - 132), kept whole on the screen.
    assert_eq!((f.tools.menu.left, f.tools.menu.top), (800 - 422, 203 - 71));
    assert_eq!((f.forms.money.left, f.forms.money.top), (800 - 441, 0));
    assert_eq!((f.forms.help.left, f.forms.help.top), (284 - 116, 191 - 71));
    assert_eq!((f.forms.compass.left, f.forms.compass.top), (740, 0));
}

/// A file with the money menu, the overview map, the sabotage window, the
/// tooltip, the info box and the end form saved open, loaded from the
/// file menu over a game with the buildings' overview open: the money
/// menu, the overview map and the sabotage window open as they were
/// saved, the buildings' overview closes, and the main screen goes over
/// the forms of its tier (1010:2819). The end form, the info box and the
/// tooltip come back empty, but not with `loaded_forms`.
#[test]
fn forms_saved_open() {
    let Some(dir) = game_dir() else { return };
    let mut windows = Windows::from_map(&map_file(&dir, "MAPS/TUTORIAL.MAP")).unwrap();
    windows.0[window::MONEY_MENU].height = 0x100;
    windows.0[window::OVERVIEW_MAP].height = 1;
    windows.0[window::SABOTAGE].visible = 1;
    windows.0[window::TOOLTIP].height = 16;
    windows.0[window::INFO_BOX].height = 40;
    windows.0[window::END_FORM].visible = 1;
    let map = with_windows(map_file(&dir, "MAPS/TUTORIAL.MAP"), &windows);
    for ghosts in [true, false] {
        let config = if ghosts {
            Config::ORIGINAL
        } else {
            only(enhance::Enhancement::LoadedForms)
        };
        let mut h = holiday_in_a_game(&dir, (800, 600), config);
        let g = h.game.as_mut().unwrap();
        g.forms.umenu.open(&g.world, &g.font, &g.info, g.size);
        load_from_the_file_menu(g, map.clone());
        let _ = g.update(Duration::ZERO);
        assert!(g.forms.money.shown() && g.world.money_menu_shown);
        // The file's 360, kept on the screen.
        assert_eq!((g.forms.money.left, g.forms.money.top), (359, 0));
        assert!(g.forms.overview.visible() && g.forms.overview.bounds().height() > 1);
        assert!(g.tools.sabowin.as_ref().unwrap().visible);
        assert!(!g.forms.umenu.visible && !g.forms.tv.visible);
        assert_eq!(g.forms.schluss.visible(), ghosts);
        assert_eq!(g.forms.infobox.visible, ghosts);
        assert_eq!(g.forms.help_visible(), ghosts);
        if ghosts {
            assert_eq!((g.forms.infobox.left, g.forms.infobox.top), (350, 296));
            g.sync_stack();
            let order = g.stack.order(false);
            let at = |w| order.iter().position(|&o| o == w).unwrap();
            assert!(at(Win::Main) > at(Win::Money) && at(Win::Main) > at(Win::Overview));
        }
    }
}

/// A game saved with the TV open has the game paused (`1110:0067`) and
/// the TV's button down; loading it shows no TV. The original stays
/// paused until the TV is opened and closed again; with `loaded_pause`
/// the load ends the pause and lets the button up. From the start menu,
/// and from the file menu over a game whose TV is closed.
#[test]
fn the_tv_pause_after_a_load() {
    let Some(dir) = game_dir() else { return };
    let mut windows = Windows::from_map(&map_file(&dir, "MAPS/TUTORIAL.MAP")).unwrap();
    windows.0[window::TV].visible = 1;
    let mut map = with_windows(map_file(&dir, "MAPS/TUTORIAL.MAP"), &windows);
    set_field(&mut map, "1110:0067", 0, &[1]);
    let mut buttons = map.fields("1110:651a").next().unwrap().to_vec();
    buttons[3] = 1;
    set_field(&mut map, "1110:651a", 0, &buttons);
    for fixed in [false, true] {
        let config = if fixed {
            only(enhance::Enhancement::LoadedPause)
        } else {
            Config::ORIGINAL
        };
        let mut g = loaded_game(&dir, map.clone(), (800, 600), config);
        let date = g.world.date;
        tick(&mut g, 20);
        assert!(!g.forms.tv.visible);
        assert_eq!(g.world.clock.paused, !fixed, "loaded_pause {fixed}");
        assert_eq!(g.world.date == date, !fixed);
        assert_eq!(g.tools.down[3], !fixed);

        let mut h = holiday_in_a_game(&dir, (800, 600), config);
        let g = h.game.as_mut().unwrap();
        let r = g.forms.tv.close();
        g.tv_response(r);
        let _ = g.update(Duration::ZERO);
        assert!(!g.world.clock.paused);
        load_from_the_file_menu(g, map.clone());
        let _ = g.update(Duration::ZERO);
        assert_eq!(g.world.clock.paused, !fixed, "loaded_pause {fixed}");
        assert_eq!(g.tools.down[3], !fixed);
    }
}

/// A new game's file has the forms' windows, not zeros, which the
/// original would load as a main screen 0 wide: the main screen centred,
/// 640 × 479 and shown, the forms closed by their height shown 0 high.
/// With `big_map` the original's main screen is saved. Loaded again, the
/// windows come back.
#[test]
fn a_new_games_windows() {
    let Some(dir) = game_dir() else { return };
    let exe = read(&dir, "HOLIDAY.EXE");
    let high = forms::high_scores(&dir, Path::new("/nonexistent/HIGHSCOR.WAV"));
    let sound = Sound::new(&dir, setup::Sound::Hi, platform::audio::Audio::silent()).shared();
    let settings = Settings::default();
    for config in [Config::ORIGINAL, big_map_only()] {
        let mut g = start_game(
            &dir,
            &exe,
            &settings,
            false,
            Options::default(),
            high.clone(),
            &sound,
            (1024, 768),
            &config,
        );
        let saved = saved_windows(&mut g);
        let main = saved.0[window::MAIN_SCREEN];
        assert_eq!(
            main,
            Window {
                top: 145,
                left: 192,
                width: 640,
                height: 479,
                visible: 1,
                enabled: 0,
            }
        );
        let money = saved.0[window::MONEY_MENU];
        assert_eq!((money.visible, money.height), (1, 0));
        assert!(money.width > 0);
        assert_eq!(saved.0[window::TV].visible, 0);
        assert_eq!(saved.0[10], Window::default());
        // The money menu opened and moved, saved and loaded again.
        g.click_button(4);
        g.forms.money.left -= 100;
        g.forms.money.top += 30;
        let at = (g.forms.money.left, g.forms.money.top);
        g.file_menu.visible = true;
        let map = SaveGame::parse(&g.map().to_bytes()).unwrap();
        g.file_menu.visible = false;
        let mut again = loaded_game(&dir, map, (1024, 768), config);
        let _ = again.update(Duration::ZERO);
        assert!(again.forms.money.shown() && again.world.money_menu_shown);
        // `big_map`'s main screen fills the screen: a form that opens
        // over its bars goes below them.
        let below = if config.view.has(enhance::Enhancement::BigMap) {
            59
        } else {
            at.1
        };
        assert_eq!((again.forms.money.left, again.forms.money.top), (at.0, below));
    }
}
