# Migrating to separate LMDB tables

The storage engine, hash key encoding, and direct lookup model stay the same.
The change is that the old merged `bin` database becomes four named databases.

| Lookup | Database | Key |
| --- | --- | --- |
| WAD path | `wad` | `u64::to_be_bytes()` |
| BIN entry path | `binentries` | `u32::to_be_bytes()` |
| BIN class or struct type | `bintypes` | `u32::to_be_bytes()` |
| BIN field name | `binfields` | `u32::to_be_bytes()` |
| BIN hash-valued data | `binhashes` | `u32::to_be_bytes()` |

## Reader changes

1. Download a `lol-hashes-v2-*.zst` asset and decompress it to `data.mdb`.
2. Open the environment with capacity for five named databases.
3. Open the appropriate database for each lookup category.
4. Keep the environment open and borrow names through read transactions.
5. Treat an unknown hash as unresolved within its category. Do not fall back to
   searching all BIN tables, which would reintroduce ambiguity.

The `wad` database has the same name and key encoding as before. BIN callers need
category information; a generic `resolve_bin(hash)` should become a lookup that
takes a category or separate category-specific methods.

No text parsing, bulk rehashing, or resident hash map is needed. The reference
reader in [examples/read.rs](../examples/read.rs) opens LMDB directly:

```bash
cargo run --locked --release --example read -- path/to/database-directory
```

Flint, Jade, Hematite, and other consumers need their BIN table selections updated
in their own repositories. This change does not modify those apps.

## Sharing memory across apps

LMDB maps the database file. Apps opening the same physical `data.mdb` can share
its file-backed pages through the operating system. Decompressing separate copies
into each application's cache does not provide that same file-backed sharing.

Point apps at one shared database directory. Open it read-only and leave LMDB's
normal locking enabled. The directory must allow LMDB to manage its `lock.mdb`
file; distribute `data.mdb` only, not a builder's lock file.

Install updates into new versioned directories and switch the shared cache's
active path only after successful decompression and validation. Existing readers
can finish using the old version. Do not overwrite or truncate a database that
apps have mapped. On Windows, wait for readers to close before removing an old
version.

The builder does not install or manage an application-wide shared cache; consumers
must agree on its path and update ownership.

## Release transition

New assets use `lol-hashes-v2-{combined,wad,bin}.zst`, published in `hashes-v2-*`
prereleases with `--latest=false`. Existing stable releases are retained so apps
expecting the old `bin` database continue to download the legacy schema.

The old stable corpus is frozen by this builder. Once consumers are updated,
version 2 can be promoted to the stable channel. During migration, select a
`hashes-v2-*` release from the release list instead of `/releases/latest`.
