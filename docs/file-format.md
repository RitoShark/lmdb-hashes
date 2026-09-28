# File format: version 2

Version 2 stores names in a zstd-compressed UTF-8 text file. It stores no numeric
game or BIN hashes, database pages, offsets, or indexes. Readers calculate hashes
while loading names and build whatever lookup structure they need.

## Layout

```text
RITO-HASHES 2
--GAME--
data/example.bin
--ENTRIES--
Example/Entry
--HASHES--
ExampleValue
--FIELDS--
ExampleField
--TYPES--
ExampleType
```

These are illustrative names. `--HASHES--` identifies BIN hash-value names;
its records are still names, not hexadecimal numbers.

| Section | Source file | Hash |
| --- | --- | --- |
| `--GAME--` | `hashes.game.txt.*` | XXH64, seed 0, 64 bits |
| `--ENTRIES--` | `hashes.binentries.txt` | FNV-1a, 32 bits |
| `--HASHES--` | `hashes.binhashes.txt` | FNV-1a, 32 bits |
| `--FIELDS--` | `hashes.binfields.txt` | FNV-1a, 32 bits |
| `--TYPES--` | `hashes.bintypes.txt` | FNV-1a, 32 bits |

The combined variant contains all five sections in this order. WAD contains
only GAME. BIN contains ENTRIES, HASHES, FIELDS, TYPES in that order. Combining
files is packaging only: lookup keys must include the universe.

## Text rules

- The first line is exactly `RITO-HASHES 2`, without a byte order mark.
- Writers use LF line endings, including after the last line. Readers also
  accept CRLF and a final line without a terminator.
- At least one section is required. Sections appear once, in the order above.
  A general reader may accept any ordered subset; each release variant has
  the exact sections listed above.
- A section ends at the next section header or end of file. Empty sections are
  valid, although the builder rejects empty upstream tables.
- Each remaining line is one name, with original case and internal spaces
  preserved. Names must be nonempty, have no leading or trailing whitespace,
  and contain no CR, LF, or NUL characters.
- Lines beginning and ending with `--` are reserved for section headers.
  There is no escaping, comment syntax, or blank-line separator in this format.
- Names are sorted lexicographically by UTF-8 bytes within each section.
  Only exact duplicate names within the same section are removed.
- Readers reject unsupported versions, unknown sections, repeated or unordered
  sections, invalid names, duplicate names, and unordered names.

## Reconstructing hashes

Convert ASCII `A` through `Z` to `a` through `z`, leaving every other UTF-8 byte
unchanged. Do not apply locale-dependent or Unicode case folding, normalize
slashes, trim names, or add a WAD filename prefix.

For GAME, apply XXH64 with seed 0 to those bytes. For each BIN universe, start
with `0x811c9dc5`; for each byte, XOR it into the accumulator and multiply by
`0x01000193`, wrapping at 32 bits. There is no byte order in the text format;
byte order matters only if a consumer serializes its own numeric index.

Distinct names may have the same hash, including names differing only in case.
Keep those names. A lossless index maps `(universe, hash)` to a list of names.
Never use one BIN map for all four universes: a collision in one category must
not replace a name in another.

The builder reads upstream hexadecimal hashes only to verify that each can be
reconstructed. A mismatch stops the build with its source file and line number;
it is not silently omitted or written under a different hash.

## Compression and integrity

Each `.txt.zst` asset is one zstd frame with its content checksum enabled,
compressed at level 19. Decompress before parsing. Treat decompression or parsing
errors as a failed load, and only install a new lookup after the entire load
succeeds. The streaming callback may have visited names before a later error.

`fingerprint-v2.sha256` is a build identity, not an asset checksum. Its SHA-256
input is the concatenation of:

1. The magic string without a newline, then the Cargo package version.
2. For each universe in canonical order: the source filename, its byte length
   as a little-endian `u64`, and the complete downloaded source bytes.

The downloaded GAME source is the concatenation of sequential numbered parts.
A missing terminal LF is added to each downloaded file or part. Changing the
format, builder version, source names, or source bytes changes the fingerprint.

## Reference implementation

[`src/lib.rs`](../src/lib.rs) provides `Universe::hash`, `read_source`,
`write_tables`, and `visit_names`. [`examples/read.rs`](../examples/read.rs)
decompresses a table, validates it, calculates its hashes, and prints counts:

```bash
cargo run --release --example read -- output/lol-names-v2-bin.txt.zst
```
