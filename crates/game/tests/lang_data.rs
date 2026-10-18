//! The port's language data (`lang/`, `game::lang`): every language's
//! files load with the game's loaders without a game folder, the world's
//! tables that come from text files are the same in every language, and
//! the files of the official releases are the original's, byte for byte
//! (skipped when `orig/` or `orig-en/` isn't there).

use std::path::{Path, PathBuf};

use formats::data;
use game::lang::Language;
use game::{highscoreform, schlussform};

/// Every text file of every language parses with its loader, as the
/// game reads it: no game folder needed.
#[test]
fn every_file_loads() {
    for language in Language::ALL {
        let file = |name: &str| {
            language
                .file(name)
                .unwrap_or_else(|| panic!("{}: no {name}", language.code()))
        };
        let code = language.code();
        assert_eq!(data::auktion(&file("TEXTE/AUKTION.TXT")).len(), 12, "{code}");
        assert_eq!(data::auswahl(&file("TEXTE/AUSWAHL.TXT")).len(), 8, "{code}");
        data::baumenu_txt(&file("TEXTE/BAUMENU.TXT")).unwrap();
        assert_eq!(data::fehler(&file("TEXTE/FEHLER.TXT")).unwrap().len(), 31, "{code}");
        data::geldmenu_txt(&file("TEXTE/GELDMENU.TXT")).unwrap();
        let (short, long) = data::hausname(&file("TEXTE/HAUSNAME.TXT")).unwrap();
        assert!(!short.is_empty() && !long.is_empty());
        assert_eq!(highscoreform::highscor_lines(&file("TEXTE/HIGHSCOR.TXT")).len(), 16);
        data::infobox_txt(&file("TEXTE/INFOBOX.TXT")).unwrap();
        assert_ne!(data::modatyp_txt(&file("TEXTE/MODATYP.TXT")).0.len(), 0);
        assert_eq!(data::monate(&file("TEXTE/MONATE.TXT")).unwrap().len(), 12, "{code}");
        assert_eq!(language.months().len(), 12);
        let (_, messages) = data::nachrich(&file("TEXTE/NACHRICH.TXT"));
        assert_eq!(messages.len(), 90, "{code}");
        let (nobody, _, names) = data::namen(&file("TEXTE/NAMEN.TXT")).unwrap();
        assert!(
            !nobody.is_empty() && names.len() >= 7,
            "{code}: a name for each computer"
        );
        assert!(data::text_lines(&file("TEXTE/OPTIONEN.TXT")).len() >= 26);
        data::sabotage(&file("TEXTE/SABOTAGE.TXT")).unwrap();
        assert!(
            data::sabotext(&file("TEXTE/SABOTEXT.TXT"))
                .iter()
                .all(|t| !t.is_empty())
        );
        assert!(schlussform::schluss_lines(&file("TEXTE/SCHLUSS.TXT")).len() >= 32);
        let sonstige = data::sonstige(&file("TEXTE/SONSTIGE.TXT")).unwrap();
        assert_ne!(data::start_txt(&file("TEXTE/START.TXT")).len(), 0);
        assert_ne!(data::szenario_txt(&file("TEXTE/SZENARIO.TXT")).len(), 0);
        data::uebermap_txt(&file("TEXTE/UEBERMAP.TXT")).unwrap();
        assert!(data::text_lines(&file("TEXTE/UMENU.TXT")).len() >= 10);
        assert!(data::text_lines(&file("TEXTE/WASNUN.TXT")).len() >= 4);
        // The later build's `UPGRADE\` texts.
        let upgrade = data::sonstige_with(&file("UPGRADE/SONSTIGE.TXT"), data::SONSTIGE_TEXTS_UPGRADE).unwrap();
        assert_eq!(
            upgrade.waehrungsfaktor, sonstige.waehrungsfaktor,
            "{code}: one currency"
        );
        assert_eq!(upgrade.texts[5], sonstige.texts[5], "{code}: one currency");
        let szenario = data::szenario_txt_lines(&file("UPGRADE/SZENARIO.TXT"), data::SZENARIO_LINES_UPGRADE);
        assert!(
            szenario.iter().take(25).all(|l| !l.is_empty()),
            "{code}: 12 scenarios and the standard game"
        );
        assert!(
            schlussform::schluss_lines_upgrade(&file("UPGRADE/SCHLUSS.TXT"))
                .iter()
                .take(44)
                .all(|l| !l.is_empty())
        );
        assert_eq!(data::text_lines(&file("UPGRADE/HIGHSCO2.TXT")).len(), 6);
        // The label layouts that differ by translation.
        data::baumenu_dat(&file("DATA/BAUMENU.DAT")).unwrap();
        data::geldmenu_dat(&file("DATA/GELDMENU.DAT")).unwrap();
        data::infobox_dat(&file("DATA/INFOBOX.DAT")).unwrap();
    }
}

/// What the world takes from text files besides text (the sabotage's
/// costs and the piranhas' pools, the build menu's lists) is the same in
/// every language, so a world doesn't change with its language. The
/// releases differ in one number the game never reads: the German
/// `SABOTAGE.TXT` gives the shark diver the target 254, the English 255
/// (only the piranhas' targets are looked at, 10a0:1f65).
#[test]
fn the_worlds_numbers_dont_change_with_the_language() {
    let kinds = |l: Language| {
        let kinds = game::sabotage::kinds(&data::sabotage(&l.file("TEXTE/SABOTAGE.TXT").unwrap()).unwrap().1);
        let costs: Vec<i32> = kinds.iter().map(|k| k.cost).collect();
        (costs, kinds[0].targets)
    };
    let lists = |l: Language| {
        let d = data::baumenu_dat(&l.file("DATA/BAUMENU.DAT").unwrap()).unwrap();
        format!("{:?}", game::build_menu::BuildMenu::new(&d))
    };
    for language in Language::ALL {
        assert_eq!(kinds(language), kinds(Language::De), "{}", language.code());
        assert_eq!(lists(language), lists(Language::De), "{}", language.code());
    }
}

fn folder(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").join(name)
}

/// `name` under `root`, whatever the case of its letters.
fn read_in(root: &Path, name: &str) -> Option<Vec<u8>> {
    let mut path = root.to_path_buf();
    for part in name.split('/') {
        let entry = std::fs::read_dir(&path)
            .ok()?
            .filter_map(Result::ok)
            .find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(part))?;
        path = entry.path();
    }
    std::fs::read(path).ok()
}

/// The language's files of a release are the release's files: the same
/// bytes once the trailing NULs some German files end with are dropped
/// (`lang/README.md`). The port's own translations aren't compared.
#[test]
fn the_releases_files_are_the_originals() {
    for (language, name, own) in [
        (Language::De, "orig", &["UPGRADE/"][..]),
        (Language::En, "orig-en", &[][..]),
    ] {
        let root = folder(name);
        if !root.join("DATA").exists() {
            eprintln!("skipping: {} not found", root.display());
            continue;
        }
        for file in language.file_names() {
            if own.iter().any(|p| file.starts_with(p)) {
                continue;
            }
            let Some(original) = read_in(&root, file) else {
                panic!("{name} has no {file}");
            };
            let end = original.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
            assert!(
                language.file(file).unwrap() == original[..end],
                "lang/{}/{file} isn't {name}'s",
                language.code()
            );
        }
        // And every text file of the release's TEXTE\ is in the language data.
        let texte = std::fs::read_dir(root.join("TEXTE")).unwrap();
        for entry in texte.filter_map(Result::ok) {
            let n = entry.file_name().to_string_lossy().to_ascii_uppercase();
            assert!(language.file(&format!("TEXTE/{n}")).is_some(), "{name}'s TEXTE/{n}");
        }
    }
}
