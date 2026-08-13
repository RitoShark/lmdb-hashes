# lmdb-hashes

Automated LMDB hash database builder for League of Legends file path lookups. Downloads hash tables from [CommunityDragon](https://github.com/CommunityDragon/Data/tree/master/hashes/lol), packs them into LMDB databases, and publishes zstd-compressed releases.

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
cargo run --release -- output work
```

Produces `output/*.zst` files and a `fingerprint.sha256`.
