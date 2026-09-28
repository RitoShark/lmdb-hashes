use heed::types::{Bytes, Str};
use heed::{Database, EnvFlags, EnvOpenOptions};
use lmdb_hash_builder::Table;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("expected an LMDB directory containing data.mdb")?;
    let env = unsafe {
        EnvOpenOptions::new()
            .max_dbs(5)
            .flags(EnvFlags::READ_ONLY)
            .open(path)?
    };
    let txn = env.read_txn()?;
    for table in Table::ALL {
        let db: Option<Database<Bytes, Str>> = env.open_database(&txn, Some(table.name()))?;
        if let Some(db) = db {
            println!("{}: {} records", table.name(), db.len(&txn)?);
        }
    }
    let types: Option<Database<Bytes, Str>> = env.open_database(&txn, Some("bintypes"))?;
    if let Some(db) = types {
        if let Some(name) = db.get(&txn, &0x4247cbb9u32.to_be_bytes())? {
            println!("bintypes[4247cbb9] = {name}");
        }
    }
    Ok(())
}
