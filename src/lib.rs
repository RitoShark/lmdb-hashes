use heed::types::{Bytes, Str};
use heed::{Database, EnvOpenOptions};
use std::error::Error;
use std::io::{self, BufRead};
use std::path::Path;

pub const SCHEMA: &str = "lmdb-hashes-separated-v2";
pub type Entry = (Vec<u8>, String);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Table {
    Wad,
    BinEntries,
    BinHashes,
    BinFields,
    BinTypes,
}

impl Table {
    pub const ALL: [Self; 5] = [
        Self::Wad,
        Self::BinEntries,
        Self::BinHashes,
        Self::BinFields,
        Self::BinTypes,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Wad => "wad",
            Self::BinEntries => "binentries",
            Self::BinHashes => "binhashes",
            Self::BinFields => "binfields",
            Self::BinTypes => "bintypes",
        }
    }

    pub fn source(self) -> &'static str {
        match self {
            Self::Wad => "hashes.game.txt",
            Self::BinEntries => "hashes.binentries.txt",
            Self::BinHashes => "hashes.binhashes.txt",
            Self::BinFields => "hashes.binfields.txt",
            Self::BinTypes => "hashes.bintypes.txt",
        }
    }

    pub fn key_width(self) -> usize {
        if self == Self::Wad {
            8
        } else {
            4
        }
    }
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

pub fn read_source(reader: impl BufRead, table: Table) -> io::Result<Vec<Entry>> {
    let width = table.key_width() * 2;
    let mut entries = Vec::new();
    for (index, line) in reader.lines().enumerate() {
        let line = line?;
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fail = || {
            invalid(format!(
                "{}:{}: invalid hash/name record",
                table.source(),
                index + 1
            ))
        };
        let hash_text = line.get(..width).ok_or_else(fail)?;
        if !hash_text.bytes().all(|byte| byte.is_ascii_hexdigit())
            || line.as_bytes().get(width) != Some(&b' ')
        {
            return Err(fail());
        }
        let name = line.get(width + 1..).ok_or_else(fail)?;
        let key = hex::decode(hash_text).map_err(|_| fail())?;
        entries.push((key, name.to_owned()));
    }
    entries.sort_unstable();
    entries.dedup();
    if entries.is_empty() {
        return Err(invalid(format!("{} contains no records", table.source())));
    }
    if let Some(pair) = entries.windows(2).find(|pair| pair[0].0 == pair[1].0) {
        return Err(invalid(format!(
            "{}: conflicting names for hash {}",
            table.source(),
            hex::encode(&pair[0].0)
        )));
    }
    Ok(entries)
}

pub fn build_lmdb(directory: &Path, tables: &[(Table, Vec<Entry>)]) -> Result<(), Box<dyn Error>> {
    let env = unsafe {
        EnvOpenOptions::new()
            .map_size(2 * 1024 * 1024 * 1024)
            .max_dbs(5)
            .open(directory)?
    };
    let mut txn = env.write_txn()?;
    for (table, entries) in tables {
        let db: Database<Bytes, Str> = env.create_database(&mut txn, Some(table.name()))?;
        for (key, name) in entries {
            db.put(&mut txn, key, name)?;
        }
    }
    txn.commit()?;
    env.force_sync()?;
    env.prepare_for_closing().wait();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_supplied_hash_and_name_bytes() {
        let entries = read_source(
            b"10e25123126b83b0 assets/perks/styles/style5/futuresmarket/futures.ps4.market.dds\n"
                .as_slice(),
            Table::Wad,
        )
        .unwrap();
        assert_eq!(entries[0].0, 0x10e25123126b83b0u64.to_be_bytes());
        let entries = read_source(
            b"811c9dc5 \r\n00000000  padded \r\n".as_slice(),
            Table::BinHashes,
        )
        .unwrap();
        assert_eq!(entries[0].1, " padded ");
        assert_eq!(entries[1].1, "");
    }

    #[test]
    fn deduplicates_identical_pairs_but_rejects_conflicting_names() {
        assert_eq!(
            read_source(
                b"00000001 Name\n00000001 Name\n".as_slice(),
                Table::BinTypes
            )
            .unwrap()
            .len(),
            1
        );
        assert!(read_source(
            b"00000001 Name\n00000001 Other\n".as_slice(),
            Table::BinTypes
        )
        .is_err());
    }

    #[test]
    fn rejects_malformed_sources() {
        for input in [
            "",
            "# empty\n",
            "12345678\n",
            "not-hex! Name\n",
            "????? name\n",
            "0000000000000000 Name\n",
        ] {
            assert!(read_source(input.as_bytes(), Table::BinTypes).is_err());
        }
        assert!(read_source(b"00000001 \xff\n".as_slice(), Table::BinTypes).is_err());
    }
}
