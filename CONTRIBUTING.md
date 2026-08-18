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
bash scripts/check-release-manifest.sh
bash scripts/check-instrument-requirements.sh
bash scripts/check-certification-claims.sh
bash scripts/check-standards-registry.sh
bash scripts/trace-report.sh
bash scripts/detect-target.sh
cargo check --locked -p indicate-frames -p indicate-alerts -p indicate-sha256 \
  -p indicate-instrument-state -p indicate-instrument-scene \
  -p indicate-instrument-glyphs -p indicate-instrument-symbology \
  -p indicate-instrument-descriptor -p indicate-instrument-panels \
  -p indicate-instrument-raster -p indicate-instrument-feeder \
  -p indicate-instrument-registry \
  --target thumbv7em-none-eabihf
cargo run --locked -q -p instrument-bench
cargo run --locked -q -p indicate-evidence --bin evidence-gate -- \
  --graph docs/instruments/evidence-graph.evg --repo-root . --resolve-selectors
cargo run --locked -q -p indicate-evidence --bin evidence-gate -- \
  --graph docs/instruments/evidence-graph.evg --repo-root . --require-resolvable
bash scripts/check-recorded-counts.sh --selftest
bash scripts/check-recorded-counts.sh
```

The evidence gate binds recorded test sources by content digest: editing
a recorded test file (the attitude-behavior and presentation suites
among them) reddens the gate until that evidence is re-recorded, so run
the two gate invocations locally after touching any recorded source.

The gate runs no build, so it cannot see a recorded pass count that
drifted when a test was added away from a bound source.
`check-recorded-counts.sh` re-runs each artifact's own recorded command
and compares. The re-record procedure is four steps, written once in
`docs/instruments/evidence-plan.md`.

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

## Cutting a release

Consumers pin a bare revision, so a revision meant to be pinned carries
an annotated tag and a `CHANGELOG.md` entry naming what it contains.

Cut one whenever any of the five contract values moves — state ABI,
scene format, corpus, composition digest, or the panel set — **or when a
public API addition lands that consumers are expected to call.**

The second trigger exists because the first one misses the additions
made *for* consumers. A predicate published so nobody writes their own
copy does not stop the second copy from being written if it cannot be
pinned by name; a manifest published for a consumer's CI to diff does
not get diffed if reaching it means pinning a bare revision and
explaining in a comment which commit it is. That is the archaeology tags
were introduced to remove, reappearing through the trigger rather than
through the absence of tags.

A tag therefore marks a revision worth pinning, not only one whose
contract values moved. That means some tags carry no contract change,
which is harmless here: the changelog entry and the release manifest
both state exactly what did and did not move, so a consumer can see at a
glance that it has nothing to re-verify.

1. Regenerate the manifest and commit it with the entry:

   ```sh
   cargo run --locked -q -p xtask -- gen-release-manifest
   ```

   `release-manifest.json` is the machine-readable form of what the
   revision pins, and `scripts/check-release-manifest.sh` regenerates
   and diffs it, so a moved pin that was not regenerated fails the
   build. Regenerating is also the right step mid-change, not only at
   release: the guard runs on every push.
2. Add a `## [x.y.z]` entry at the top of `CHANGELOG.md` with all five
   values, and anything a consumer re-pinning across it must know.
   `scripts/check-release-markers.sh` fails the build if a value
   disagrees with the tree, so run it before pushing.
3. Merge. The release names the *merge* commit.
4. Tag that commit, with the same five values in the message:

   ```
   git tag -a v0.1.0 -m 'Indicate v0.1.0
   state ABI: 6
   scene format: 1
   corpus: 4
   composition digest: bd85b853…
   panel set: pfd, hsi, monitor'
   git push origin v0.1.0
   ```

The tagging step is deliberately manual and unguarded. A release tags its own
merge commit, which does not exist while the pull request creating the
entry is open, so a CI check for the tag would fail every release on the
one run that matters. The changelog entry is what CI can hold honest;
the tag is what a human owes it.

## Documentation language

Project-authored documentation uses ASD-STE100 Simplified Technical
English. `AGENTS.md` at the repository root holds the rules, and they
apply to new text and to text you change, not to legacy prose you happen
to be near. No gate checks this; review does.

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
- Panels are authored against the frame range their descriptor declares
  and receive the chosen frame as a draw argument; unclipped ink past
  that frame's edge is counted and ratcheted per frame by the admission
  harness — growth is a deliberate decision, not drift.
