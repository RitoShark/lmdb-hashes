# Contributing

Issues and pull requests are welcome. For format changes, describe the proposed
layout and how existing consumers will migrate before implementing it.

## Get started

```bash
git clone https://github.com/RitoShark/lmdb-hashes
cd lmdb-hashes
cargo test --locked --all-targets
```

See [development](docs/development.md) for the repository layout, build pipeline,
and offline workflow. See the [file format](docs/file-format.md) for the contract
that readers depend on.

## Before submitting

```bash
cargo fmt --check
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
```

Add focused tests when changing parsing, hashing, category handling, or output
behavior. Use synthetic fixtures and keep downloaded tables and build artifacts
out of commits. Never commit game assets.

Keep code simple and comments minimal. Put explanations in the documentation.
Keep commits small with short, imperative subjects, such as `Preserve BIN type
names` or `Reject malformed source records`.

Pull requests should explain the behavior change and the checks run. Update the
format and migration docs when readers are affected, and bump the builder version
when output behavior changes. Preserve hash universes and collision candidates.

Participation follows the [Code of Conduct](CODE_OF_CONDUCT.md).
