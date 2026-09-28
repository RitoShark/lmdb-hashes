use lmdb_hash_builder::{visit_names, Universe};
use std::fs;
use std::path::Path;
use std::process::Command;

fn run(output: &Path, work: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_lmdb-hash-builder"))
        .arg(output)
        .arg(work)
        .arg("--offline")
        .output()
        .unwrap()
}

#[test]
fn offline_build_skip_repair_and_reject_invalid_source() {
    let root = std::env::temp_dir().join(format!(
        "name-tables-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let work = root.join("work");
    let output = root.join("output");
    fs::create_dir_all(&work).unwrap();
    for universe in Universe::ALL {
        let width = if universe == Universe::Game { 16 } else { 8 };
        fs::write(
            work.join(universe.source()),
            format!("{:0width$x} Example\n", universe.hash("Example")),
        )
        .unwrap();
    }
    let result = run(&output, &work);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    for (variant, expected) in [("combined", 5), ("wad", 1), ("bin", 4)] {
        let bytes = fs::read(output.join(format!("lol-names-v2-{variant}.txt.zst"))).unwrap();
        let decoded = zstd::decode_all(bytes.as_slice()).unwrap();
        let mut universes = Vec::new();
        visit_names(decoded.as_slice(), |universe, name| {
            assert_eq!(name, "Example");
            universes.push(universe);
            Ok(())
        })
        .unwrap();
        assert_eq!(universes.len(), expected);
        assert_eq!(universes.contains(&Universe::Game), variant != "bin");
    }
    assert!(String::from_utf8_lossy(&run(&output, &work).stderr).contains("unchanged"));
    let bin = output.join("lol-names-v2-bin.txt.zst");
    fs::remove_file(&bin).unwrap();
    assert!(run(&output, &work).status.success());
    let before = fs::read(&bin).unwrap();
    let fingerprint = fs::read(output.join("fingerprint-v2.sha256")).unwrap();
    fs::write(work.join(Universe::Types.source()), "00000000 Wrong\n").unwrap();
    assert!(!run(&output, &work).status.success());
    assert_eq!(before, fs::read(bin).unwrap());
    assert_eq!(
        fingerprint,
        fs::read(output.join("fingerprint-v2.sha256")).unwrap()
    );
    fs::remove_dir_all(root).unwrap();
}
