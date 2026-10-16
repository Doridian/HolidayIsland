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
}
