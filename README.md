# lmdb-hashes

Compact name tables for League of Legends hash lookups, built from
[CommunityDragon](https://github.com/CommunityDragon/Data/tree/master/hashes/lol).

Version 2 stores **only names**. Applications calculate hashes while loading the
tables. WAD paths and each BIN category stay separate, so entries, types, fields,
and hash-value names cannot overwrite one another.

```text
RITO-HASHES 2
--ENTRIES--
Example/Entry
--HASHES--
ExampleValue
--FIELDS--
ExampleField
--TYPES--
ExampleType
```

## Get the tables

Choose a `names-v2-*` prerelease from [Releases](https://github.com/RitoShark/lmdb-hashes/releases).
Each asset decompresses with zstd to a readable UTF-8 text file.

| Asset | Sections |
| --- | --- |
| `lol-names-v2-combined.txt.zst` | GAME, ENTRIES, HASHES, FIELDS, TYPES |
| `lol-names-v2-wad.txt.zst` | GAME |
| `lol-names-v2-bin.txt.zst` | ENTRIES, HASHES, FIELDS, TYPES |

The combined file keeps the same separate sections. Only exact duplicate names
within a section are removed; distinct names sharing a hash are retained.

**Existing LMDB consumers need a reader update.** Version 2 uses new filenames
and a separate prerelease channel. The last stable LMDB release stays available
at the old download URLs. See the [migration guide](docs/migration.md).

## Build locally

Install stable Rust and your platform's C/C++ build tools, then run:

```bash
cargo run --locked --release -- output work
```

The builder downloads source files into `work/` and writes the three compressed
tables plus `fingerprint-v2.sha256` into `output/`. Reuse downloaded sources with:

```bash
cargo run --locked --release -- output work --offline
```

Read and hash a generated table with the included example:

```bash
cargo run --locked --release --example read -- output/lol-names-v2-bin.txt.zst
```

The release workflow checks CommunityDragon every six hours. Existing release
fingerprints skip publication; old releases are retained during migration.

## Documentation

- [File format](docs/file-format.md): layout, sections, hash algorithms, and collisions.
- [Migration](docs/migration.md): updating LMDB consumers and release URLs.
- [Development](docs/development.md): repository layout, pipeline, testing, and automation.
- [Contributing](CONTRIBUTING.md), [Code of Conduct](CODE_OF_CONDUCT.md), and [Security](SECURITY.md).

## Source

The name corpus comes from [CommunityDragon's hash tables](https://github.com/CommunityDragon/Data/tree/master/hashes/lol).
Corrections to source names belong upstream. This repository builds and distributes
the tables; it is not affiliated with Riot Games.
