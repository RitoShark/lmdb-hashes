# lol-hash-cracker

An open-source League of Legends hash research CLI. It builds the RitoShark LMDB releases, inventories every hash and string reference in installed game WAD/BIN data, and tests bounded, corpus-derived guesses against unresolved FNV-1a and XXH64 values.

It uses real format parsing from [RitoShark-Crates](https://github.com/RitoShark/RitoShark-Crates), accepts CommunityDragon text tables or RitoShark LMDBs, and can enrich its dictionary with [LoL Meta Wiki](https://github.com/LeagueToolkit/lol-meta-wiki) class/property names.

## CLI

```text
lol-hash-cracker build --output output --work work
lol-hash-cracker scan "C:/Riot Games/League of Legends" --hashes hashes --meta meta.db.json --output scan.json
lol-hash-cracker crack unresolved.txt --dictionary hashes --meta meta.db.json --report scan.json --output cracked.txt
lol-hash-cracker run "C:/Riot Games/League of Legends" --hashes hashes --meta meta.db.json --dictionary hashes --output hash-run
```

`--hashes` accepts individual CommunityDragon text files, a directory containing them, an LMDB directory, or a parent directory containing `hashes-bin.lmdb` and `hashes-wad.lmdb`.

The scanner reports:

- every WAD chunk path hash;
- every BIN entry, class, field, hash value, link, file reference, and patch key;
- all literal BIN strings with occurrence counts and example source files;
- resolved and unresolved status against the supplied dictionaries.

Alongside the JSON report, `scan` writes an `.unresolved.txt` target list ready for `crack`.

The cracker starts with exact names and strings from the supplied tables, meta schema, and scan. Later passes generate casing/prefix/suffix variants, combinations of frequent corpus tokens, common League paths/extensions, and three-token class/property names. `--depth` is capped at four so a run remains finite and reproducible. Hashes are not encrypted; a match proves a candidate has the same 32-bit or 64-bit hash, but a 32-bit FNV collision is still possible and should be validated from usage context.

## Variants

| File | Contents | Key format |
|------|----------|------------|
| `lol-hashes-combined.zst` | WAD + BIN hashes | Both |
| `lol-hashes-wad.zst` | WAD path hashes only | `u64` xxhash64 (BE) |
| `lol-hashes-bin.zst` | BIN name hashes only | `u32` FNV1a (BE) |

Each `.zst` file decompresses to an LMDB `data.mdb` with named databases (`wad`, `bin`, or both).

## Usage

Download the latest release:

```
https://github.com/RitoShark/lmdb-hashes/releases/latest/download/lol-hashes-combined.zst
```

Decompress with zstd and open with any LMDB client (e.g. `heed` in Rust):

```rust
let data = zstd::decode_all(compressed_bytes.as_slice())?;
std::fs::write("hashes.mdb", &data)?;
let env = EnvOpenOptions::new().max_dbs(2).open("hashes_dir")?;
let wad_db: Database<Bytes, Str> = env.open_database(&rtxn, Some("wad"))?;
let path = wad_db.get(&rtxn, &hash.to_be_bytes())?; // "data/characters/ahri/skins/skin0.bin"
```

## Automation

A GitHub Action runs every 6 hours, rebuilds only if CommunityDragon hashes have changed (SHA-256 fingerprint check), and publishes a new release while cleaning up old ones.

## Building locally

```bash
cargo build --release
cargo run --release -- build --output output --work work
```

Produces `output/*.zst` files and a `fingerprint.sha256`.
