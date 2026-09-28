# lmdb-hashes

LMDB hash databases for League of Legends tools, built from
[CommunityDragon](https://github.com/CommunityDragon/Data/tree/master/hashes/lol).

Open the database and look up a hash directly. Names stay in memory-mapped LMDB
pages, so applications do not need to parse a text corpus or rebuild an index.
Processes opening the same database file can share its pages through the OS.

## Organized by hash universe

The combined database contains five independent named databases:

```text
data.mdb
??? wad
??? binentries
??? binhashes
??? binfields
??? bintypes
```

BIN entries, hash values, fields, and types stay separate. The same numeric hash
can resolve to a different name in each category without one replacing another.
Hashes remain the lookup keys; values contain only the original names.

## Downloads

Choose a `hashes-v2-*` prerelease from [Releases](https://github.com/RitoShark/lmdb-hashes/releases).
Decompress the chosen archive to `data.mdb` inside a database directory.

| Asset | Named databases |
| --- | --- |
| `lol-hashes-v2-combined.zst` | All five |
| `lol-hashes-v2-wad.zst` | `wad` |
| `lol-hashes-v2-bin.zst` | `binentries`, `binhashes`, `binfields`, `bintypes` |

Version 2 changes the BIN schema, not the storage engine. Existing BIN readers
must select the appropriate table instead of opening `bin`. The previous stable
release stays available at the old download URLs during migration.

## Read a name

Using Rust and `heed = "0.20"`, after decompressing to `hashes/data.mdb`:

```rust
use heed::types::{Bytes, Str};
use heed::{Database, EnvFlags, EnvOpenOptions};

let env = unsafe {
    EnvOpenOptions::new()
        .max_dbs(5)
        .flags(EnvFlags::READ_ONLY)
        .open("hashes")?
};
let txn = env.read_txn()?;
let types: Database<Bytes, Str> = env
    .open_database(&txn, Some("bintypes"))?
    .ok_or("missing bintypes database")?;
let name = types.get(&txn, &0x4247cbb9u32.to_be_bytes())?;
```

Keep the environment open and reuse it. Returned strings borrow from the read
transaction. See [migration](docs/migration.md) for shared caches and table routing.

## Build locally

Requires stable Rust and the platform's C/C++ build tools.

```bash
cargo run --locked --release -- output work
```

Sources are downloaded into `work/`. The three archives and
`fingerprint-v2.sha256` are written to `output/`. To reuse downloaded sources:

```bash
cargo run --locked --release -- output work --offline
```

The release workflow runs every six hours. It publishes changed fingerprints and
retains existing releases during migration.

## Documentation

- [File format](docs/file-format.md): LMDB layout, keys, values, and collisions.
- [Migration](docs/migration.md): table selection and shared database files.
- [Development](docs/development.md): build pipeline, repository layout, and tests.
- [Contributing](CONTRIBUTING.md), [Code of Conduct](CODE_OF_CONDUCT.md), and [Security](SECURITY.md).

The source corpus comes from CommunityDragon. Source corrections belong upstream.
This project is not affiliated with Riot Games.
