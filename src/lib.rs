use std::io::{self, BufRead, Write};

pub const MAGIC: &str = "RITO-HASHES 2";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Universe {
    Game,
    Entries,
    Hashes,
    Fields,
    Types,
}

impl Universe {
    pub const ALL: [Self; 5] = [
        Self::Game,
        Self::Entries,
        Self::Hashes,
        Self::Fields,
        Self::Types,
    ];

    pub fn header(self) -> &'static str {
        match self {
            Self::Game => "--GAME--",
            Self::Entries => "--ENTRIES--",
            Self::Hashes => "--HASHES--",
            Self::Fields => "--FIELDS--",
            Self::Types => "--TYPES--",
        }
    }

    pub fn source(self) -> &'static str {
        match self {
            Self::Game => "hashes.game.txt",
            Self::Entries => "hashes.binentries.txt",
            Self::Hashes => "hashes.binhashes.txt",
            Self::Fields => "hashes.binfields.txt",
            Self::Types => "hashes.bintypes.txt",
        }
    }

    pub fn hash(self, name: &str) -> u64 {
        let lower = name.to_ascii_lowercase();
        match self {
            Self::Game => xxhash_rust::xxh64::xxh64(lower.as_bytes(), 0),
            _ => lower.bytes().fold(0x811c9dc5u32, |hash, byte| {
                (hash ^ u32::from(byte)).wrapping_mul(0x01000193)
            }) as u64,
        }
    }
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.trim() == name
        && !name.contains(['\r', '\n', '\0'])
        && !(name.starts_with("--") && name.ends_with("--"))
}

pub fn read_source(reader: impl BufRead, universe: Universe) -> io::Result<Vec<String>> {
    let width = if universe == Universe::Game { 16 } else { 8 };
    let mut names = Vec::new();
    for (index, line) in reader.lines().enumerate() {
        let line = line?;
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fail = || {
            invalid(format!(
                "{}:{}: invalid hash/name record",
                universe.source(),
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
        if !valid_name(name) {
            return Err(fail());
        }
        let expected = u64::from_str_radix(hash_text, 16).map_err(|_| fail())?;
        if universe.hash(name) != expected {
            return Err(invalid(format!(
                "{}:{}: hash cannot be reconstructed for {name:?}",
                universe.source(),
                index + 1
            )));
        }
        names.push(name.to_owned());
    }
    names.sort_unstable();
    names.dedup();
    if names.is_empty() {
        return Err(invalid(format!("{} contains no names", universe.source())));
    }
    Ok(names)
}

pub fn write_tables(mut writer: impl Write, tables: &[(Universe, Vec<String>)]) -> io::Result<()> {
    if tables.is_empty() {
        return Err(invalid("at least one section is required"));
    }
    writeln!(writer, "{MAGIC}")?;
    let mut previous = None;
    for (universe, names) in tables {
        if previous.is_some_and(|previous| previous >= *universe) {
            return Err(invalid("sections must be unique and in canonical order"));
        }
        previous = Some(*universe);
        writeln!(writer, "{}", universe.header())?;
        let mut previous_name: Option<&str> = None;
        for name in names {
            if !valid_name(name) || previous_name.is_some_and(|previous| previous >= name.as_str())
            {
                return Err(invalid("names must be valid, unique and sorted"));
            }
            writeln!(writer, "{name}")?;
            previous_name = Some(name);
        }
    }
    Ok(())
}

pub fn visit_names(
    reader: impl BufRead,
    mut visit: impl FnMut(Universe, &str) -> io::Result<()>,
) -> io::Result<()> {
    let mut lines = reader.lines();
    if lines.next().transpose()?.as_deref() != Some(MAGIC) {
        return Err(invalid("unsupported hash table format"));
    }
    let mut current = None;
    let mut previous_name = String::new();
    for line in lines {
        let line = line?;
        if let Some(universe) = Universe::ALL.into_iter().find(|u| u.header() == line) {
            if current.is_some_and(|previous| previous >= universe) {
                return Err(invalid("sections must be unique and in canonical order"));
            }
            current = Some(universe);
            previous_name.clear();
        } else {
            let universe = current.ok_or_else(|| invalid("name before first section"))?;
            if !valid_name(&line) || line <= previous_name {
                return Err(invalid("names must be valid, unique and sorted"));
            }
            visit(universe, &line)?;
            previous_name = line;
        }
    }
    if current.is_none() {
        return Err(invalid("at least one section is required"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(universe: Universe, names: &[&str]) -> String {
        let width = if universe == Universe::Game { 16 } else { 8 };
        names
            .iter()
            .map(|name| format!("{:0width$x} {name}\n", universe.hash(name)))
            .collect()
    }

    #[test]
    fn known_hashes() {
        assert_eq!(Universe::Types.hash("AIGenericCommon"), 0x4247cbb9);
        assert_eq!(Universe::Fields.hash("Hello"), 0x4f9f2cab);
        assert_eq!(Universe::Game.hash(""), 0xef46db3751d8e999);
        assert_eq!(Universe::Game.hash("Hello"), 0x26c7827d889f6da3);
    }

    #[test]
    fn names_only_round_trip_keeps_universes_and_aliases() {
        let mut tables = Vec::new();
        for universe in Universe::ALL {
            let input = source(universe, &["same", "Same", "same", "path with spaces"]);
            tables.push((universe, read_source(input.as_bytes(), universe).unwrap()));
        }
        let mut bytes = Vec::new();
        write_tables(&mut bytes, &tables).unwrap();
        assert!(
            String::from_utf8_lossy(&bytes).contains("--ENTRIES--\nSame\npath with spaces\nsame\n")
        );
        let compressed = zstd::encode_all(bytes.as_slice(), 1).unwrap();
        let decoded = zstd::decode_all(compressed.as_slice()).unwrap();
        let mut actual = Vec::new();
        visit_names(decoded.as_slice(), |universe, name| {
            actual.push((universe, name.to_owned()));
            Ok(())
        })
        .unwrap();
        let expected: Vec<_> = tables
            .into_iter()
            .flat_map(|(u, names)| names.into_iter().map(move |n| (u, n)))
            .collect();
        assert_eq!(actual, expected);
    }

    #[test]
    fn preserves_distinct_names_with_the_same_hash() {
        let universe = Universe::Entries;
        assert_eq!(universe.hash("costarring"), universe.hash("liquid"));
        let input = source(universe, &["liquid", "costarring", "liquid"]);
        assert_eq!(
            read_source(input.as_bytes(), universe).unwrap(),
            ["costarring", "liquid"]
        );
    }

    #[test]
    fn rejects_lossy_sources() {
        for input in [
            "00000000 wrong\n",
            "ééééé bad\n",
            "12345678\n",
            "",
            "# empty\n",
        ] {
            assert!(read_source(input.as_bytes(), Universe::Types).is_err());
        }
        for name in ["--TYPES--", " padded", "trailing ", "nul\0name"] {
            assert!(
                read_source(source(Universe::Types, &[name]).as_bytes(), Universe::Types).is_err()
            );
        }
    }

    #[test]
    fn rejects_invalid_tables() {
        for input in [
            "RITO-HASHES 1\n--GAME--\n",
            "RITO-HASHES 2\nname\n",
            "RITO-HASHES 2\n--UNKNOWN--\n",
            "RITO-HASHES 2\n--TYPES--\n--TYPES--\n",
            "RITO-HASHES 2\n--TYPES--\n--ENTRIES--\n",
            "RITO-HASHES 2\n--TYPES--\nz\na\n",
            "RITO-HASHES 2\n--TYPES--\na\na\n",
            "RITO-HASHES 2\n--TYPES--\n\n",
            "RITO-HASHES 2\n",
        ] {
            assert!(
                visit_names(input.as_bytes(), |_, _| Ok(())).is_err(),
                "{input}"
            );
        }
    }
}
