//! Builds every form in `orig/HOLIDAY.EXE`. Skipped when it isn't there.

use formats::dfm::Object;
use formats::ne::{RT_RCDATA, ResId, resources};
use vcl::form::{Form, Kind};

#[test]
fn all_forms_load() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../orig/HOLIDAY.EXE");
    let Ok(exe) = std::fs::read(&path) else {
        eprintln!("skipping: {} not found", path.display());
        return;
    };
    let mut images = 0;
    for res in resources(&exe).unwrap() {
        if res.kind == ResId::Id(RT_RCDATA) {
            let form = Form::load(&Object::parse(res.data).unwrap()).unwrap();
            images += form
                .components
                .iter()
                .filter(|c| matches!(c.kind, Kind::Image { .. }))
                .count();
            if form.form().class == b"TStartform" {
                let bild = form.component("Bild").unwrap();
                assert_eq!(bild.handler("OnMouseDown"), Some(&b"BildMouseDown"[..]));
                assert_eq!((bild.bounds.width(), bild.bounds.height()), (425, 353));
            }
        }
    }
    assert_eq!(images, 42);
}
