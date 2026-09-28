# Migrating from LMDB

Version 2 changes both storage and lookup structure. Update a consumer's reader
before changing its download URL.

| Legacy | Version 2 |
| --- | --- |
| `lol-hashes-*.zst` | `lol-names-v2-*.txt.zst` |
| Decompress to LMDB `data.mdb` | Decompress to UTF-8 text |
| `wad` database with big-endian `u64` keys | GAME section; calculate XXH64 |
| One `bin` database with big-endian `u32` keys | Four BIN sections; calculate FNV-1a separately |
| One surviving name per hash | All distinct names retained per universe |

## Consumer changes

1. Download the desired version 2 variant from a `names-v2-*` release.
2. Stream the decompressed text through a version-aware reader.
3. Calculate each name's hash using the [format rules](file-format.md).
4. Build separate indexes for GAME, ENTRIES, HASHES, FIELDS, and TYPES. Preserve
   collision candidates instead of overwriting them.
5. Route BIN entry paths to ENTRIES, class names to TYPES, field names to FIELDS,
   and hash-valued data to HASHES. GAME is for WAD paths.
6. Replace the active cache only after decompression and validation succeed.

Flint, Jade, Hematite, and other LMDB consumers need their own reader changes.
This repository does not update those applications. Once consumers are migrated,
the release workflow can promote version 2 to the stable release channel.

## Release transition

The workflow publishes version 2 as prereleases with `--latest=false`. Existing
releases are retained, so `/releases/latest/download/lol-hashes-*.zst` continues
to use the last stable LMDB release. That legacy dataset is frozen; this builder
only produces version 2 tables.

Choose a `names-v2-*` release from the [release list](https://github.com/RitoShark/lmdb-hashes/releases).
Do not use `/releases/latest` to discover version 2 during the transition.
For scripts, list releases with the GitHub API and select the newest nondraft
release whose tag begins with `names-v2-`, then use its asset download URLs.

Retain old local LMDB caches until the consumer's migration succeeds. Version 2
output names do not overwrite legacy assets or LMDB directories.
