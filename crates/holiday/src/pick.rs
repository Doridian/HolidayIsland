//! The game's files when the command line names none (a packaged app
//! started from its icon): `orig` in the working directory, as always;
//! else the game the player picked before; else the system's file dialog
//! asks for it, the CD image (`.iso`, `.bin`) or `HOLIDAY.EXE` in the
//! game's folder, and then, if the music has no `SoundFont` yet, for one
//! (`.sf2`, optional), which goes where `tools/get-soundfont.sh` puts
//! its (`platform::audio::soundfont_dir`). The choice is kept in the
//! port's data directory (`platform::storage::data_dir`, `game-path.txt`).
//! Natively only; the browser build's page asks for the files (`web`).

use std::io::IsTerminal;
use std::path::{Path, PathBuf};

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
    let picked = loop {
        let path = rfd::FileDialog::new()
            .set_title("Choose the Holiday Island CD image, or HOLIDAY.EXE in the game's folder")
            .add_filter("CD image or HOLIDAY.EXE", &["iso", "ISO", "bin", "BIN", "exe", "EXE"])
            .add_filter("All files", &["*"])
            .pick_file()?;
        // HOLIDAY.EXE stands for its folder.
        let path = match path.parent() {
            Some(dir) if path.file_name().is_some_and(|n| n.eq_ignore_ascii_case("HOLIDAY.EXE")) => dir.to_path_buf(),
            _ => path,
        };
        match check(&path) {
            Ok(()) => break path,
            Err(e) => error_box(&format!(
                "That isn't Holiday Island's CD image or HOLIDAY.EXE ({e}). Choose another."
            )),
        }
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
    soundfont();
    Some(picked)
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
        let Some(path) = rfd::FileDialog::new()
            .set_title("Choose a SoundFont (.sf2) for the music; cancel for no music")
            .add_filter("SoundFont", &["sf2", "SF2"])
            .pick_file()
        else {
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

/// `message` in a message box, as an error.
pub fn error_box(message: &str) {
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
