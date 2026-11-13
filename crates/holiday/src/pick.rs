//! The game's files when the command line names none (a packaged app
//! started from its icon): `orig` in the working directory, as always;
//! else the game the player picked before; else the system's file dialog
//! asks for it, the CD image (`.iso`, `.bin`) or `HOLIDAY.EXE` in the
//! game's folder, and then, if the music has no `SoundFont` yet, for one
//! (`.sf2`, optional), which goes where `tools/get-soundfont.sh` puts
//! its (`platform::audio::soundfont_dir`). Where there is no dialog to
//! ask with (Linux with neither the desktop portal's nor zenity: a
//! minimal system, a sandbox like the `AppImage` catalog's test), a window
//! of the port's own takes the game dropped on it or its path typed
//! (`platform::ask_drop`), and nothing asks for a `SoundFont`. The choice
//! is kept in the port's data directory (`platform::storage::data_dir`,
//! `game-path.txt`). Natively only; the browser build's page asks for
//! the files (`web`).

use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// The game's CD image or folder: `default` if it's there, else the one
/// picked before, else the one the player picks now. `None` if the
/// player cancels.
pub fn game(default: &Path) -> Option<PathBuf> {
    if looks_like_game(default) {
        return Some(default.to_path_buf());
    }
    let kept = platform::storage::data_dir().map(|d| d.join("game-path.txt"));
    let before = kept
        .as_deref()
        .and_then(|k| std::fs::read_to_string(k).ok())
        .map(|s| PathBuf::from(s.trim_end_matches(['\r', '\n'])))
        .filter(|p| looks_like_game(p));
    if before.is_some() {
        return before;
    }
    let (picked, dialog) = match pick_game() {
        Ok(picked) => (picked?, true),
        Err(NoDialog) => (drop_game()?, false),
    };
    if let Some(kept) = kept {
        let saved = kept
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(&kept, format!("{}\n", picked.display())));
        if let Err(e) = saved {
            eprintln!("{}: {e}", kept.display());
        }
    }
    if dialog {
        soundfont();
    }
    Some(picked)
}

/// The game from the file dialog; `None` if the player cancels.
fn pick_game() -> Result<Option<PathBuf>, NoDialog> {
    loop {
        let Some(path) = pick_file(
            rfd::FileDialog::new()
                .set_title("Choose the Holiday Island CD image, or HOLIDAY.EXE in the game's folder")
                .add_filter("CD image or HOLIDAY.EXE", &["iso", "ISO", "bin", "BIN", "exe", "EXE"])
                .add_filter("All files", &["*"]),
        )?
        else {
            return Ok(None);
        };
        let path = game_path(path);
        match check(&path) {
            Ok(()) => return Ok(Some(path)),
            Err(e) => error_box(&format!(
                "That isn't Holiday Island's CD image or HOLIDAY.EXE ({e}). Choose another."
            )),
        }
    }
}

/// The game dropped on the port's own window (or its path typed);
/// `None` if the player closes it.
fn drop_game() -> Option<PathBuf> {
    let take = |path: &Path| {
        let name = path
            .file_name()
            .unwrap_or(path.as_os_str())
            .to_string_lossy()
            .into_owned();
        let path = game_path(path.to_path_buf());
        check(&path)
            .map(|()| path)
            .map_err(|e| format!("{name} isn't Holiday Island's CD image or folder ({e}). Drop another."))
    };
    let lines = [
        "Drop the game on this window: its CD image (.iso, .bin), its folder, or HOLIDAY.EXE in its folder. \
         Or type its path and press Enter.",
        "(There is no file dialog here to choose it with.)",
    ];
    platform::ask_drop("Holiday Island", &lines, take).unwrap_or_else(|e| {
        eprintln!("no file dialog, and no window either: {e}");
        None
    })
}

/// `HOLIDAY.EXE` stands for its folder.
fn game_path(path: PathBuf) -> PathBuf {
    match path.parent() {
        Some(dir) if path.file_name().is_some_and(|n| n.eq_ignore_ascii_case("HOLIDAY.EXE")) => dir.to_path_buf(),
        _ => path,
    }
}

/// The file dialog found nothing to show itself with.
struct NoDialog;

/// Set once there was no dialog: no message box can show either.
static NO_DIALOG: AtomicBool = AtomicBool::new(false);

/// `dialog`'s file, `None` if the player cancels. On Linux rfd asks the
/// desktop portal (over D-Bus) and, failing that, runs zenity; both
/// failing look like a cancel. A portal that isn't there is a warning
/// in rfd's log, so a `None` after one, with no zenity to run, is
/// [`NoDialog`].
fn pick_file(dialog: rfd::FileDialog) -> Result<Option<PathBuf>, NoDialog> {
    let watched = watch::start();
    let picked = dialog.pick_file();
    if picked.is_none() && watched && watch::warned() && !watch::on_path("zenity") {
        NO_DIALOG.store(true, Ordering::Relaxed);
        return Err(NoDialog);
    }
    Ok(picked)
}

/// What rfd logs, where it may find no dialog (not on Windows or macOS,
/// whose dialogs are the system's own).
#[cfg(all(unix, not(target_vendor = "apple")))]
mod watch {
    use std::sync::OnceLock;
    use std::sync::atomic::{AtomicBool, Ordering};

    struct Watch;

    static WARNED: AtomicBool = AtomicBool::new(false);

    impl log::Log for Watch {
        fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
            metadata.level() <= log::Level::Warn && metadata.target().starts_with("rfd")
        }

        fn log(&self, record: &log::Record<'_>) {
            if self.enabled(record.metadata()) {
                WARNED.store(true, Ordering::Relaxed);
            }
        }

        fn flush(&self) {}
    }

    /// Starts watching anew; false if something else has the log.
    pub fn start() -> bool {
        static WATCHING: OnceLock<bool> = OnceLock::new();
        let watching = *WATCHING.get_or_init(|| {
            let set = log::set_logger(&Watch).is_ok();
            if set {
                log::set_max_level(log::LevelFilter::Warn);
            }
            set
        });
        WARNED.store(false, Ordering::Relaxed);
        watching
    }

    /// Whether rfd warned since [`start`].
    pub fn warned() -> bool {
        WARNED.load(Ordering::Relaxed)
    }

    /// Whether `name` is a program on `PATH`.
    pub fn on_path(name: &str) -> bool {
        use std::os::unix::fs::PermissionsExt;
        std::env::var_os("PATH").is_some_and(|path| {
            std::env::split_paths(&path)
                .filter_map(|dir| std::fs::metadata(dir.join(name)).ok())
                .any(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        })
    }
}

#[cfg(not(all(unix, not(target_vendor = "apple"))))]
mod watch {
    pub fn start() -> bool {
        false
    }

    pub fn warned() -> bool {
        false
    }

    pub fn on_path(_: &str) -> bool {
        true
    }
}

/// Whether `path` is a file (a CD image, which [`check`]s when it's
/// mounted) or a folder with `HOLIDAY.EXE`.
fn looks_like_game(path: &Path) -> bool {
    path.is_file() || crate::forms::find(path, "HOLIDAY.EXE").is_some()
}

/// Whether `path` is the game's CD image or folder; why not if it isn't.
fn check(path: &Path) -> Result<(), String> {
    if path.is_file() {
        platform::data::CdImage::open_file(path).map(drop)
    } else if crate::forms::find(path, "HOLIDAY.EXE").is_some() {
        Ok(())
    } else {
        Err(format!("no HOLIDAY.EXE in {}", path.display()))
    }
}

/// Asks for a `SoundFont` if the music has none, and copies it to
/// `platform::audio::soundfont_dir`, where the port finds it from then
/// on. Cancelled, the music stays silent.
fn soundfont() {
    if platform::audio::find_soundfont(None).is_some() {
        return;
    }
    let Some(dir) = platform::audio::soundfont_dir() else {
        return;
    };
    loop {
        let Ok(Some(path)) = pick_file(
            rfd::FileDialog::new()
                .set_title("Choose a SoundFont (.sf2) for the music; cancel for no music")
                .add_filter("SoundFont", &["sf2", "SF2"]),
        ) else {
            return;
        };
        if let Err(e) = platform::audio::load_soundfont(&path) {
            error_box(&format!("That isn't a SoundFont ({e}). Choose another."));
            continue;
        }
        let to = dir.join(path.file_name().unwrap_or("music.sf2".as_ref()));
        if let Err(e) = std::fs::create_dir_all(&dir).and_then(|()| std::fs::copy(&path, &to)) {
            error_box(&format!("Can't copy the SoundFont to {}: {e}", to.display()));
        }
        return;
    }
}

/// `message` in a message box, as an error; none where there's no
/// dialog to show it with (it went to the standard error too).
pub fn error_box(message: &str) {
    if NO_DIALOG.load(Ordering::Relaxed) {
        return;
    }
    rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_title("Holiday Island")
        .set_description(message)
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
}

/// `message` on the standard error, and in a message box too when that
/// goes nowhere to be read (started from the icon).
pub fn fatal(message: &str) {
    eprintln!("{message}");
    if !std::io::stderr().is_terminal() {
        error_box(message);
    }
}
