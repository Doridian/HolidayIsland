//! Unpacks the CD's `_FILES.1` and compares every file with the one
//! `tools/extract_install.py` wrote into `orig/`. Skipped without them.

use std::path::PathBuf;

use formats::install;

#[test]
fn files_archive_unpacks_like_the_tool() {
    let orig = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../orig");
    let Ok(archive) = std::fs::read(orig.join("_FILES.1")) else {
        eprintln!("skipping: no {}", orig.join("_FILES.1").display());
        return;
    };
    let entries = install::entries(&archive).unwrap();
    assert_eq!(entries.len(), 109);
    let mut compared = 0;
    for e in &entries {
        let data = install::unpack(&archive, e).unwrap();
        assert_eq!(data.len(), usize::try_from(e.size).unwrap());
        let path = orig.join(e.install_path());
        // The CD's loose copies are the same; compare what's there.
        if let Ok(on_disk) = std::fs::read(&path) {
            assert!(data == on_disk, "{} differs", path.display());
            compared += 1;
        }
    }
    assert!(compared > 100, "only {compared} files compared");
}
