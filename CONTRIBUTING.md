# Contributing to Indicate

## Toolchain setup

1. Install the Rust toolchain pinned by `rust-toolchain.toml` (stable
   channel); `rustup` picks it up automatically.
2. Add the bare-metal check target: `rustup target add thumbv7em-none-eabihf`.
3. Install `shellcheck` if you touch `scripts/*.sh`.

## Local gate battery

Run the full battery before pushing; CI runs the same set.

```sh
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
RUSTDOCFLAGS="-D missing_docs -D rustdoc::broken_intra_doc_links" cargo doc --no-deps
cargo build --release
bash scripts/check-structure.sh
bash scripts/check-instrument-requirements.sh
bash scripts/check-certification-claims.sh
bash scripts/check-standards-registry.sh
cargo check -p pilotage-frames -p pilotage-alerts -p pilotage-sha256 \
  -p pilotage-instrument-state -p pilotage-instrument-scene \
  -p pilotage-instrument-glyphs -p pilotage-instrument-symbology \
  -p pilotage-instrument-panels -p pilotage-instrument-raster \
  -p pilotage-instrument-feeder -p pilotage-instrument-registry \
  --target thumbv7em-none-eabihf
cargo run -q -p instrument-bench
```

## Discipline that is easy to miss

- REN-03 frame hashes and the scene digest are pinned invariants, not
  values to refresh: a mismatch is a determinism regression unless the
  change deliberately moves paint, and a deliberate move re-pins once,
  with the reason in the commit message.
- A corpus edit is a versioned event: bump `corpusVersion`, expect every
  pinned consumer to go red at its next pin advance, and treat that red
  as the sync mechanism working.
- The evidence graph (`docs/instruments/evidence-graph.evg`) binds test
  sources by content digest and its baseline by commit: editing a
  recorded source file requires re-recording that evidence, and history
  rewrites that orphan the baseline commit are forbidden.
- Panels are authored in the design frame their descriptor declares;
  unclipped ink past the frame edge is counted and ratcheted by the
  admission harness — growth is a deliberate decision, not drift.
