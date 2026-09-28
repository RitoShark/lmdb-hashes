use heed::types::{Bytes, Str};
use heed::{Database, EnvFlags, EnvOpenOptions};
use lmdb_hash_builder::{read_source, Table};
use std::fs::File;
use std::io::{self, BufReader};

#[test]
#[ignore = "requires downloaded sources in work and built archives in output"]
fn archives_preserve_every_source_mapping() -> Result<(), Box<dyn std::error::Error>> {
    for variant in ["combined", "wad", "bin"] {
        let directory = tempfile::tempdir()?;
        let archive = File::open(format!("output/lol-hashes-v2-{variant}.zst"))?;
        let mut decoder = zstd::Decoder::new(archive)?;
        let mut data = File::create(directory.path().join("data.mdb"))?;
        io::copy(&mut decoder, &mut data)?;
        drop(data);
        let env = unsafe {
            EnvOpenOptions::new()
                .max_dbs(5)
                .flags(EnvFlags::READ_ONLY)
                .open(directory.path())?
        };
        let txn = env.read_txn()?;
        for table in Table::ALL {
            let expected = variant == "combined" || (variant == "wad") == (table == Table::Wad);
            let db: Option<Database<Bytes, Str>> = env.open_database(&txn, Some(table.name()))?;
            assert_eq!(db.is_some(), expected);
            if let Some(db) = db {
                let entries = read_source(
                    BufReader::new(File::open(format!("work/{}", table.source()))?),
                    table,
                )?;
                assert_eq!(db.len(&txn)?, entries.len() as u64);
                for (key, value) in &entries {
                    assert_eq!(db.get(&txn, key)?, Some(value.as_str()));
                }
                println!(
                    "{variant}/{}: verified {} mappings",
                    table.name(),
                    entries.len()
                );
            }
        }
        drop(txn);
        env.prepare_for_closing().wait();
    }
    Ok(())
}
