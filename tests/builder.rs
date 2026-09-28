use heed::types::{Bytes, Str};
use heed::{Database, EnvFlags, EnvOpenOptions};
use lmdb_hash_builder::Table;
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
fn offline_build_isolates_universes_and_supports_direct_lmdb_reads() {
    let root = tempfile::tempdir().unwrap();
    let work = root.path().join("work");
    let output = root.path().join("output");
    fs::create_dir_all(&work).unwrap();
    for table in Table::ALL {
        fs::write(
            work.join(table.source()),
            format!(
                "{:0width$x} {}\n",
                1,
                table.name(),
                width = table.key_width() * 2
            ),
        )
        .unwrap();
    }
    let result = run(&output, &work);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    for variant in ["combined", "wad", "bin"] {
        let bytes = fs::read(output.join(format!("lol-hashes-v2-{variant}.zst"))).unwrap();
        let decoded = zstd::decode_all(bytes.as_slice()).unwrap();
        let directory = root.path().join(variant);
        fs::create_dir(&directory).unwrap();
        fs::write(directory.join("data.mdb"), decoded).unwrap();
        let env = unsafe {
            EnvOpenOptions::new()
                .max_dbs(6)
                .flags(EnvFlags::READ_ONLY)
                .open(&directory)
                .unwrap()
        };
        let txn = env.read_txn().unwrap();
        for table in Table::ALL {
            let db: Option<Database<Bytes, Str>> =
                env.open_database(&txn, Some(table.name())).unwrap();
            let expected = variant == "combined" || (variant == "wad") == (table == Table::Wad);
            assert_eq!(db.is_some(), expected);
            if let Some(db) = db {
                let key = 1u64.to_be_bytes();
                assert_eq!(
                    db.get(&txn, &key[8 - table.key_width()..]).unwrap(),
                    Some(table.name())
                );
                assert_eq!(db.len(&txn).unwrap(), 1);
            }
        }
        let merged: Option<Database<Bytes, Str>> = env.open_database(&txn, Some("bin")).unwrap();
        assert!(merged.is_none());
        drop(txn);
        env.prepare_for_closing().wait();
    }
    assert!(String::from_utf8_lossy(&run(&output, &work).stderr).contains("unchanged"));
    let bin = output.join("lol-hashes-v2-bin.zst");
    fs::remove_file(&bin).unwrap();
    let result = run(&output, &work);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let before = fs::read(&bin).unwrap();
    let fingerprint = fs::read(output.join("fingerprint-v2.sha256")).unwrap();
    fs::write(work.join(Table::BinTypes.source()), "malformed\n").unwrap();
    assert!(!run(&output, &work).status.success());
    assert_eq!(before, fs::read(bin).unwrap());
    assert_eq!(
        fingerprint,
        fs::read(output.join("fingerprint-v2.sha256")).unwrap()
    );
}
