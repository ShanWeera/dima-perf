//! `remove_temporaries` is process-wide, so it is tested in its own process.

use std::io::Write;

use dima_core::output::{self, OutputCompression, Sink, Target};

#[test]
fn forced_cleanup_removes_live_temporaries() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("x.json");
    std::fs::write(&p, "previous").unwrap();
    let mut s = Sink::create(&Target::Path(p.clone()), OutputCompression::None).unwrap();
    s.write_all(b"partial").unwrap();
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);
    output::remove_temporaries();
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    assert_eq!(
        std::fs::read_to_string(&p).unwrap(),
        "previous",
        "the destination is untouched"
    );
    drop(s);
}
