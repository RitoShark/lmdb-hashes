# LMDB schema, version 2

The storage engine remains LMDB, built with `heed 0.20`. Each release archive is
one zstd frame containing the bytes of an LMDB `data.mdb`. There is no custom text
header, line-based database format, or Mimir dependency.

Version 2 identifies the named-database schema through its asset names and release
tags. It does not change LMDB's own file format or add a header before LMDB data.

## Named databases

| Database | Source | Key |
| --- | --- | --- |
| `wad` | `hashes.game.txt.*` | 8-byte unsigned integer, big-endian |
| `binentries` | `hashes.binentries.txt` | 4-byte unsigned integer, big-endian |
| `binhashes` | `hashes.binhashes.txt` | 4-byte unsigned integer, big-endian |
| `binfields` | `hashes.binfields.txt` | 4-byte unsigned integer, big-endian |
| `bintypes` | `hashes.bintypes.txt` | 4-byte unsigned integer, big-endian |

Values are the source names encoded as UTF-8, without a length prefix or trailing
NUL added by this application. The hash is stored once as the key; it is not
repeated in the value. LMDB manages record lengths and its B-tree index.

The combined variant contains all five named databases. WAD contains only `wad`.
BIN contains the four `bin*` databases. There is no merged `bin` database.
The unnamed LMDB database is reserved for LMDB's named-database catalog.

All named databases use LMDB's default byte ordering, with no integer-key or
duplicate-value flags. Big-endian encoding preserves numeric ordering.
Configure readers with `max_dbs(5)` for the combined schema.

## Hash semantics

League WAD paths generally use XXH64 with seed zero. BIN names generally use
FNV-1a 32. Hashing new names requires ASCII lowercasing first.

The builder preserves the supplied source hash, rather than deriving it again.
This keeps existing mappings, including any unusual source hash/name pair.
For example, the audited CommunityDragon corpus contains
`10e25123126b83b0 assets/perks/styles/style5/futuresmarket/futures.ps4.market.dds`.
Its supplied hash is retained even though recalculating the path produces
`23e64b6f85cdef58`.

The input has 16 hexadecimal digits for WAD or 8 for BIN, followed by one ASCII
space and the name. The separator is removed, and the remaining name is preserved,
including its case, leading/trailing spaces, or an empty string. CRLF and LF source
line endings are accepted. Empty lines and lines beginning with `#` are ignored.
Malformed records, invalid UTF-8, and empty source tables fail the build.

## Collisions

Each database is an independent hash universe. Identical numeric keys across
different tables are valid and never overwrite each other.

Within one table, identical hash/name pairs are deduplicated. Different names
for the same hash fail the build rather than silently selecting one. This differs
from the old builder, which kept the first record after sorting by hash.

## Compression and fingerprint

Archives use zstd level 19 with the content checksum enabled. Decompress once at
installation time, then open `data.mdb` directly for lookups.

`fingerprint-v2.sha256` identifies the build inputs; it is not an archive checksum.
It hashes the schema identifier `lmdb-hashes-separated-v2`, the package version,
and then each expected source's filename, byte length as little-endian `u64`,
and complete source bytes. Source order is wad, binentries, binhashes, binfields,
bintypes. Downloaded GAME parts are concatenated in numeric order, adding a
terminal LF to a part if missing.

LMDB's on-disk representation depends on platform architecture and page size.
Published files are built on the workflow's Ubuntu runner. Validate compatibility
before supporting a new architecture or page size.
