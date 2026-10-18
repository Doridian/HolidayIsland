//! The later build's `UPGRADE\` folder (`docs/versions.md`): six more
//! scenarios and the files that go with them, some of which replace the
//! CD's. The port reads it when it's there; without it a game folder
//! (the German v1.0 CD's) plays with v1.0's files.
//!
//! What the later build reads from `UPGRADE\` instead of its own folder:
//! `SONSTIGE.TXT` (en 1010:3e42), `SZENARIO.TXT` (en 1058:2c38),
//! `SCHLUSS.TXT` (en 1070:0294), `HAUSPREI.DAT` (en 1048:341c),
//! `CREDITS.BMP` (en 1038:3925), `DATWIN1D.BMP` (en 1040:159e),
//! `STARTSZ1.BMP` (en 1058:02cf); and new: the scenarios' maps and
//! events, `STRAKN2*.BMP`, `HIGHSCO2.*`.

use std::path::Path;

use formats::bmp::Bitmap;
use formats::data::{self, Sonstige};
use vcl::surface::Surface;

/// The texts the later build can't start without: a folder that lacks
/// one of them doesn't count.
const NEEDED: [&str; 3] = ["SONSTIGE.TXT", "SZENARIO.TXT", "SCHLUSS.TXT"];

/// Whether the game in `dir` has the later build's `UPGRADE\` folder.
#[must_use]
pub fn present(dir: &Path) -> bool {
    NEEDED
        .iter()
        .all(|f| crate::forms::find(dir, &format!("UPGRADE/{f}")).is_some_and(|p| p.is_file()))
}

/// `UPGRADE\name` if the folder is there and has it, else `fallback` (a
/// path in the game's folder), whatever the case of their letters.
#[must_use]
pub fn file(dir: &Path, name: &str, fallback: &str) -> Vec<u8> {
    let upgraded = present(dir)
        .then(|| crate::forms::find(dir, &format!("UPGRADE/{name}")))
        .flatten()
        .and_then(|p| std::fs::read(p).ok());
    upgraded.unwrap_or_else(|| crate::read(dir, fallback))
}

/// `SONSTIGE.TXT`: the later build's 25 texts from `UPGRADE\`, else the
/// CD's 24 (1020:3dc4, en 1010:3e42).
#[must_use]
pub fn sonstige(dir: &Path) -> Sonstige {
    let parsed = if present(dir) {
        data::sonstige_with(
            &file(dir, "SONSTIGE.TXT", "TEXTE/SONSTIGE.TXT"),
            data::SONSTIGE_TEXTS_UPGRADE,
        )
    } else {
        data::sonstige(&crate::read(dir, "TEXTE/SONSTIGE.TXT"))
    };
    parsed.expect("SONSTIGE.TXT")
}

/// The bitmaps `UPGRADE\name`, each empty if it's missing.
#[must_use]
pub fn bitmaps(dir: &Path, files: &[&str]) -> Vec<Surface> {
    files
        .iter()
        .map(|f| {
            crate::forms::find(dir, &format!("UPGRADE/{f}"))
                .and_then(|p| std::fs::read(p).ok())
                .and_then(|b| Bitmap::parse(&b).ok())
                .map_or_else(|| Surface::new(0, 0, 0), |b| Surface::from_bitmap(&b))
        })
        .collect()
}
