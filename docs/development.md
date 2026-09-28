# How the builder works

## Repository map

| Path | Purpose |
| --- | --- |
| `src/main.rs` | CLI, downloads, fingerprinting, compression, output files |
| `src/lib.rs` | Universes, hashing, source validation, text reader and writer |
| `examples/read.rs` | Streaming reader example |
| `examples/audit.rs` | Compare every source hash with its calculated value |
| `tests/builder.rs` | Offline CLI integration test |
| `.github/workflows/check.yml` | Windows and Linux checks |
| `.github/workflows/update-hashes.yml` | Scheduled builds and release publication |

## Build pipeline

1. Fetch `hashes.game.txt.0`, `.1`, and so on until a 404. A 404 on part zero,
   any other HTTP failure, or an empty response fails the build. Fetch the four
   BIN source files separately. Downloads have a five-minute request timeout.
2. Calculate the versioned fingerprint from the five expected source files.
   Unrelated files in the work directory are ignored. Skip only if the fingerprint
   matches and all three output assets exist.
3. Parse every source and verify every supplied hash. Reject invalid UTF-8,
   malformed records, unsupported names, mismatches, and empty tables. Empty
   source lines and lines starting with `#` are ignored.
4. Sort names within their own universe and remove exact duplicates. No names
   are deduplicated by hash or across universes.
5. Stream the three variants into zstd encoders. Each asset is written to a
   temporary file, flushed, and renamed. Write the fingerprint after all succeed.

Sources and individual assets use temporary files beside their destination.
The complete three-file output is not one atomic transaction; if interrupted,
rerun the builder before consuming or publishing it. Do not run two builders
against the same directories. An unchanged build checks asset existence, not
asset integrity; remove damaged assets to force regeneration.

The builder holds parsed names for all universes in memory while sorting, but
does not build LMDB indexes or buffer whole compressed outputs. Consumers can
stream parsing and hashing; memory use then depends on their chosen index.

## Commands

Requires stable Rust and the platform's Rust C/C++ build tools for zstd.
Linux also needs OpenSSL development packages for reqwest's default TLS backend.

```bash
cargo run --locked --release -- output work
cargo run --locked --release -- output work --offline
cargo run --locked --release --example audit -- work
cargo fmt --check
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
```

Offline mode expects `hashes.game.txt` and the four BIN source files in `work`.
Their input format is hexadecimal hash, one ASCII space, then the original name.
GAME hashes have 16 hex digits and BIN hashes have 8. Offline mode makes no
network requests. Running Cargo itself may require downloading dependencies
unless they are already cached.

## Automation

The publication workflow runs every six hours and on manual dispatch. It runs
checks, downloads sources, and builds the assets. A release tag contains the
first 16 fingerprint digits; if that tag already exists, publication is skipped.
Fresh runners still download and build before checking the release tag.
Publication is serialized and old releases are retained.

Bump the package version when changing builder output behavior so unchanged
source data cannot reuse an older fingerprint. Commit `Cargo.lock` with dependency
updates. Changes to the public format also need a new magic/version and a
coordinated consumer migration.
