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
    // The group sizes from the file's own notes; the game's come from memory.
    let mut last_row = [-1; data::ANIMDAT3_GROUPS];
    last_row[..2].copy_from_slice(&[7, 15]);
    let animdat3 = data::animdat3(&read("DATA/ANIMDAT3.DAT").unwrap(), &last_row).unwrap();
    assert_eq!(animdat3[0][0], [47, 23, 0]);
    assert_eq!(animdat3[1][0], [31, 31, 1]);
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
    use formats::savegame::{SaveGame, VERSION};
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
        assert_eq!(save.version, VERSION, "{name}");
        assert!(save.trailing.is_empty(), "{name}");
        assert_eq!(save.players(), players, "{name}");
        assert!(save.to_bytes() == data, "{name} doesn't round-trip");
    }
}
