# Development

## Repository layout

| Path | Purpose |
| --- | --- |
| `src/main.rs` | Downloads, fingerprint, CLI, compression |
| `src/lib.rs` | Source parsing, table definitions, LMDB construction |
| `examples/read.rs` | Direct LMDB reader |
| `tests/builder.rs` | Offline build, table isolation, archive and rebuild tests |
| `tests/corpus.rs` | Optional full-corpus archive verification |
| `.github/workflows/check.yml` | Windows and Linux checks |
| `.github/workflows/update-hashes.yml` | Scheduled builds and publication |

## Build pipeline

1. Download numbered GAME parts until a terminal 404, plus the four BIN files.
   Part zero must exist. Empty responses and other HTTP failures abort.
2. Fingerprint the schema, package version, and five expected source files.
   Skip only when that fingerprint matches and all three archives exist.
3. Parse each source separately, retaining supplied keys and original names.
   Deduplicate identical pairs and reject conflicting names within a category.
4. Build each variant in a fresh temporary LMDB directory under the output
   directory. A single transaction creates its named databases and inserts records.
5. Commit, sync, and close LMDB. Stream its `data.mdb` into a checksummed zstd
   archive, then rename the completed archive into place.
6. Write the fingerprint only after all variants finish.

The LMDB map size is a 2 GiB virtual address reservation, not a 2 GiB allocation
of resident memory. The builder holds parsed records while sorting; readers do
not need to load all records into private memory.

Only archives and the fingerprint are retained. Temporary LMDB directories are
removed after compression. Each archive is replaced independently, so the three
outputs are not an atomic set. Rerun an interrupted build before publication.
Run only one builder per work/output directory. Existing output checks test
presence, not integrity; remove a damaged archive to force regeneration.

## Commands

Requires stable Rust and C/C++ build tools for LMDB and zstd. Linux needs OpenSSL
development packages for reqwest's default TLS backend.

```bash
cargo run --locked --release -- output work
cargo run --locked --release -- output work --offline
cargo fmt --check
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
```

Offline mode expects `hashes.game.txt` and all four BIN source files in `work/`.
The builder makes no requests in offline mode; Cargo may still need dependencies.

Tests decompress real builder output, open it with heed, and check that the same
numeric hash resolves independently in every category. They also cover source
validation, supplied hash preservation, duplicate handling, unchanged builds,
missing output regeneration, and failed builds retaining prior outputs.

After building the full corpus into the default directories, verify every
source mapping in all three archives:

```bash
cargo test --locked --release --test corpus -- --ignored --nocapture
```

## Releases

The workflow runs every six hours and on manual dispatch. Checks run before the
build. Tags use `hashes-v2-` plus the first 16 fingerprint digits; an existing tag
skips publication. Fresh runners download and build before checking the tag.
Publication is serialized and existing releases are retained during migration.

Bump the package version when output behavior changes so the fingerprint cannot
reuse an older build. Commit Cargo.lock with dependency changes. Changes to table
names or key encodings require coordinated reader updates and new asset names.
