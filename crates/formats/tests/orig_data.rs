//! Parses the real data files from `orig/` (see the README). Skipped when
//! they aren't there.

use std::path::PathBuf;

use formats::data;

fn read(rel: &str) -> Option<Vec<u8>> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../orig").join(rel);
    let data = std::fs::read(&path).ok();
    if data.is_none() {
        eprintln!("skipping: {} not found", path.display());
    }
    data
}

#[test]
fn data_tables() {
    let Some(hausdata) = read("DATA/HAUSDATA.DAT") else {
        return;
    };
    let hausdata = data::hausdata(&hausdata).unwrap();
    assert_eq!(hausdata[0], [2, 2, 1, 15, 5, 0, 0, 1, 0, 100]);

    let hausprei = data::hausprei(&read("DATA/HAUSPREI.DAT").unwrap()).unwrap();
    assert_eq!(hausprei[0], [800, 80, 16, 16]);
    assert_eq!(hausprei.len(), data::BUILDING_TYPES);

    assert_eq!(data::hauspos(&read("DATA/HAUSPOS.DAT").unwrap()).unwrap().len(), 673);
    assert_eq!(data::hausnr(&read("DATA/HAUSNR.DAT").unwrap()).unwrap().len(), 127);
    assert_eq!(data::grounpos(&read("DATA/GROUNPOS.DAT").unwrap()).unwrap().len(), 141);
    assert_eq!(data::groundnr(&read("DATA/GROUNDNR.DAT").unwrap()).unwrap().len(), 95);
    assert_eq!(data::gfxpos(&read("DATA/GFXPOS.DAT").unwrap()).unwrap().len(), 133);
    assert_eq!(data::straspos(&read("DATA/STRASPOS.DAT").unwrap()).unwrap().len(), 45);
    assert_eq!(data::flusspos(&read("DATA/FLUSSPOS.DAT").unwrap()).unwrap().len(), 40);
    assert_eq!(data::fontdat(&read("DATA/FONTDAT1.DAT").unwrap()).unwrap()[0], [74; 8]);
    assert_eq!(data::fontdat(&read("DATA/FONTDAT2.DAT").unwrap()).unwrap().len(), 32);
    assert_eq!(data::animdat1(&read("DATA/ANIMDAT1.DAT").unwrap()).unwrap().len(), 683);
    assert_eq!(data::animdat2(&read("DATA/ANIMDAT2.DAT").unwrap()).unwrap().len(), 40);
    assert_eq!(
        data::neintrag(&read("DATA/NEINTRAG.DAT").unwrap()).unwrap()[0],
        [0, 0, 2, 3]
    );
    assert_eq!(data::animdat4(&read("DATA/ANIMDAT4.DAT").unwrap()).unwrap().len(), 8);
    // The groups' last rows, the first words of 1110:3620's entries.
    let last_row = [7, 15, 7, 15, 7, 15, 7, 15, 7, 14, 7, 7, 14, 7, 14, 14];
    let animdat3 = data::animdat3(&read("DATA/ANIMDAT3.DAT").unwrap(), &last_row).unwrap();
    assert_eq!(animdat3[0][0], [47, 23, 0]);
    assert_eq!(animdat3[1][0], [31, 31, 1]);
    // Each track passes the square's centre, where the planes turn.
    assert!(animdat3.iter().all(|g| g.iter().any(|r| r[..2] == [31, 15])));
    for name in ["FILENAME", "FILMNAME", "SOUNNAME"] {
        let names = data::message_media_names(&read(&format!("DATA/{name}.DAT")).unwrap());
        assert_eq!(names.len(), data::MESSAGES);
    }
    let ereignis = data::ereignis(&read("DATA/EREIGNIS.DAT").unwrap()).unwrap();
    assert_eq!(ereignis[0], [1_100_000, 3802, 3900, 0, 0, 100_000, 7, 0, 0, 0, 0, 0]);
    for n in 1..=6 {
        assert_eq!(
            data::ereignis(&read(&format!("DATA/EREIGN{n}.DAT")).unwrap())
                .unwrap()
                .len(),
            41
        );
    }
    let darstell = data::darstell(&read("DATA/DARSTELL.DAT").unwrap()).unwrap();
    assert_eq!(
        darstell.spielerzeiger_pos[2][7],
        [16, 32, 340, 128, 340, 0, -16, 30, 0, 0, 0, 0]
    );
    let infobox = data::infobox_dat(&read("DATA/INFOBOX.DAT").unwrap()).unwrap();
    assert_eq!(
        infobox.textzentriert[2],
        [data::TextAlign::Center, data::TextAlign::Left, data::TextAlign::Right]
    );
    assert_eq!(infobox.ib_buttonzeile.len(), 8);
    let abrechnung = data::abrechne(&read("DATA/ABRECHNE.DAT").unwrap()).unwrap();
    assert_eq!(abrechnung.factors[0], [1, 2, 0, 255]);
    assert_eq!((abrechnung.min_rent, abrechnung.max_rent), (0, 800));
    assert_eq!(abrechnung.max_price, [2000, 1000, 1000, 500, 1000, 100, 200, 4, 4, 4]);
    let compbau = data::compbau(&read("DATA/COMPBAU.DAT").unwrap()).unwrap();
    assert_eq!(compbau[0], [255, 9, 10, 255, 255, 255, 255]);
    assert_eq!(compbau[12], [50, 0, 1, 255, 255, 255, 255]);
    let maperste = data::maperste(&read("DATA/MAPERSTE.DAT").unwrap()).unwrap();
    assert_eq!(maperste[0][0], [35, 28, 28, 28, 35]);
    let modatyp = data::modatyp(&read("DATA/MODATYP.DAT").unwrap()).unwrap();
    assert_eq!(modatyp.land, data::Land::Deutschland);
    assert_eq!(
        (
            modatyp.max_lageeinfluss,
            modatyp.startgeld,
            modatyp.klassenmischungdivisor
        ),
        (30, 500_000, 20)
    );
    assert_eq!(modatyp.klassenmischung[9], [0, 0, 0, 0, 0, 0, 0, 2, 4, 14]);
    assert_eq!(modatyp.angeschlossendivisor, 10);
    assert_eq!(modatyp.angeschlossenumrechnung, [0, 100, 100, 80, 55, 30]);
    assert_eq!(modatyp.verfallwahrscheinlichkeit, [25; 3]);
    let baumenu = data::baumenu_dat(&read("DATA/BAUMENU.DAT").unwrap()).unwrap();
    assert_eq!(baumenu.menus[0].rows[0], [0, -1, -1, -1]);
    assert_eq!(baumenu.menus[1].rows[0], [54, -1, -1, -1]);
    let geldmenu = data::geldmenu_dat(&read("DATA/GELDMENU.DAT").unwrap()).unwrap();
    assert_eq!(geldmenu[..2], [[0, 0, 440, 386], [0, 0, 440, 15]]);
    let uebermap = data::uebermap_dat(&read("DATA/UEBERMAP.DAT").unwrap()).unwrap();
    assert_eq!(uebermap.pressed[..2], [[0, 0, 31, 31], [36, 0, 67, 31]]);
    let names = data::message_media_names(&read("DATA/FILENAME.DAT").unwrap());
    assert_eq!(names[0], b"osterv  .bmp");
}

#[test]
fn text_tables() {
    let Some(hausname) = read("TEXTE/HAUSNAME.TXT") else {
        return;
    };
    let (first, second) = data::hausname(&hausname).unwrap();
    assert!(!first.is_empty() && !second.is_empty());
    assert_eq!(data::auswahl(&read("TEXTE/AUSWAHL.TXT").unwrap()).len(), 8);
    let start = data::start_txt(&read("TEXTE/START.TXT").unwrap());
    assert_eq!(start[2], b"Start");
    let szenario = data::szenario_txt(&read("TEXTE/SZENARIO.TXT").unwrap());
    assert_eq!(szenario[0], b"Money-Maker");
    assert_eq!(szenario[12], b"Standardspiel");
    assert_eq!(data::fehler(&read("TEXTE/FEHLER.TXT").unwrap()).unwrap().len(), 31);
    let (title, messages) = data::nachrich(&read("TEXTE/NACHRICH.TXT").unwrap());
    assert!(!title.is_empty() && messages.len() == data::MESSAGES);
    assert!(
        data::sabotext(&read("TEXTE/SABOTEXT.TXT").unwrap())
            .iter()
            .all(|s| !s.is_empty())
    );
}

#[test]
fn maps_and_scenarios() {
    use formats::savegame::{SaveGame, Source, VERSION, VERSION_V10};
    /// Gives back a parsed file's fields.
    struct Copy<'a>(&'a SaveGame);
    impl Source for Copy<'_> {
        fn field(&mut self, dest: &'static str, n: usize, _size: usize) -> Vec<u8> {
            self.0.nth(dest, n).map(<[u8]>::to_vec).unwrap_or_default()
        }
        fn records(&mut self, dest: &'static str) -> Vec<Vec<u8>> {
            self.0.fields(dest).map(<[u8]>::to_vec).collect()
        }
    }
    let files = [
        "MAPS/TUTORIAL.MAP",
        "SZENARIO/GELD.HIS",
        "SZENARIO/KOENIG.HIS",
        "SZENARIO/LUXUS.HIS",
    ];
    let more = ["SZENARIO/OEKO.HIS", "SZENARIO/SURFER.HIS", "SZENARIO/URLAUBER.HIS"];
    for (name, players) in files.into_iter().chain(more).zip([3, 2, 8, 3, 1, 4, 4]) {
        let Some(data) = read(name) else {
            return;
        };
        let save = SaveGame::parse(&data).unwrap();
        // The CD's files are v1.0's; the port writes the later build's.
        assert_eq!(save.version, VERSION_V10, "{name}");
        assert!(save.trailing.is_empty(), "{name}");
        assert_eq!(save.players(), players, "{name}");
        assert!(save.to_bytes() == data, "{name} doesn't round-trip");
        // `save_map`'s order is `load_map`'s.
        let built = SaveGame::build(&mut Copy(&save));
        assert!(built.units == save.units, "{name} doesn't rebuild");
        assert_eq!(built.version, VERSION);
    }
}

#[test]
fn high_scores() {
    use formats::highscore::HighScores;
    let Some(cleared) = read("SOUND/HIGHSCOR.WAV") else {
        return;
    };
    let table = HighScores::parse(&cleared).unwrap();
    assert!(table.to_bytes() == cleared, "doesn't round-trip");
    assert!(
        table
            .entries
            .iter()
            .flatten()
            .all(|e| e.name.as_bytes().is_empty() && e.score == 0)
    );

    let test = read("SOUNDLO/HIGHSCOR.WAV").unwrap();
    let table = HighScores::parse(&test).unwrap();
    assert!(table.to_bytes() == test, "doesn't round-trip");
    let first = &table.entries[0][0];
    assert_eq!((first.name.as_bytes(), first.score), (&b"Spieler1"[..], 58));
    assert_eq!(first.money_text.as_bytes(), b"5.893.223.530");
}

#[test]
fn forms() {
    use formats::dfm::Object;
    use formats::ne::{RT_RCDATA, ResId, resources};
    let Some(exe) = read("HOLIDAY.EXE") else {
        return;
    };
    let forms: Vec<_> = resources(&exe)
        .unwrap()
        .into_iter()
        .filter(|r| r.kind == ResId::Id(RT_RCDATA))
        .collect();
    assert_eq!(forms.len(), 23);
    for res in forms {
        let form = Object::parse(res.data).unwrap();
        assert_eq!(ResId::Name(form.class.to_ascii_uppercase()), res.name);
        if form.class == b"THintergrund" {
            let caption = form.property("Caption").and_then(|v| v.as_bytes());
            assert_eq!(caption, Some(&b"Holiday Island"[..]));
        }
    }
}

#[test]
fn bitmaps() {
    use formats::bmp::Bitmap;
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../orig/GRAFIK");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        eprintln!("skipping: {} not found", dir.display());
        return;
    };
    let mut count = 0;
    for entry in entries {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("bmp")) {
            let b = Bitmap::parse(&std::fs::read(&path).unwrap());
            // A cut-off copy of GFXGROUN.BMP that the game doesn't use.
            if path.ends_with("GFXGROU2.BMP") {
                assert_eq!(b, Err(formats::bmp::Error::Truncated));
            } else {
                let b = b.unwrap();
                assert_eq!(b.pixels.len(), b.width * b.height, "{}", path.display());
            }
            count += 1;
        }
    }
    assert_eq!(count, 322);
    let title = Bitmap::parse(&read("GRAFIK/TIT80X60.BMP").unwrap()).unwrap();
    assert_eq!((title.width, title.height), (800, 600));
}
