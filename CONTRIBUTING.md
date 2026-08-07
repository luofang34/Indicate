# Contributing to Indicate

## Toolchain setup

1. Install the Rust toolchain pinned by `rust-toolchain.toml` (stable
   channel); `rustup` picks it up automatically.
2. Add the bare-metal check target: `rustup target add thumbv7em-none-eabihf`.
3. Install `shellcheck` if you touch `scripts/*.sh`.

## Local gate battery

Run the full battery before pushing; CI runs this set plus the
downstream-agnostic closure check.

```sh
cargo fmt --all --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
RUSTDOCFLAGS="-D missing_docs -D rustdoc::broken_intra_doc_links" cargo doc --locked --no-deps
cargo build --locked --release
bash scripts/check-structure.sh
bash scripts/check-instrument-requirements.sh
bash scripts/check-certification-claims.sh
bash scripts/check-standards-registry.sh
bash scripts/trace-report.sh
bash scripts/detect-target.sh
cargo check --locked -p indicate-frames -p indicate-alerts -p indicate-sha256 \
  -p indicate-instrument-state -p indicate-instrument-scene \
  -p indicate-instrument-glyphs -p indicate-instrument-symbology \
  -p indicate-instrument-panels -p indicate-instrument-raster \
  -p indicate-instrument-feeder -p indicate-instrument-registry \
  --target thumbv7em-none-eabihf
cargo run --locked -q -p instrument-bench
cargo run --locked -q -p indicate-evidence --bin evidence-gate -- \
  --graph docs/instruments/evidence-graph.evg --repo-root . --resolve-selectors
cargo run --locked -q -p indicate-evidence --bin evidence-gate -- \
  --graph docs/instruments/evidence-graph.evg --repo-root . --require-resolvable
```

The evidence gate binds recorded test sources by content digest: editing
a recorded test file (the attitude-behavior and presentation suites
among them) reddens the gate until that evidence is re-recorded, so run
the two gate invocations locally after touching any recorded source.

## PR discipline

- One issue per PR. Break large refactors into independently revertible
  steps.
- Every PR lands the fix **and** the guardrail that prevents its
  regression (a test, a lint, or a CI script change) in the same PR. A
  fix without a guardrail is temporary.
- Do not skip hooks, force-push shared branches, or bypass the gates to
  land a PR faster; if a gate is wrong, fix the gate in its own PR
  first.
- `ADR-NNNN` identifiers cited in code, scripts, and docs are
  architecture decision records living in the Pilotage repository's
  `docs/adr/`.

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
