# Indicate

> **⚠️ Work in progress — experimental.** Indicate is early-stage,
> experimental software under active development, provided **as is**
> with **no warranty or guarantee of any kind**, express or implied —
> including, without limitation, fitness for a particular purpose,
> correctness, reliability, availability, or safety. Interfaces and
> behavior may change without notice.
>
> **SIM / NOT FOR FLIGHT.** Nothing here is certified, approved, or airworthy.
> Nothing may be used for operational control of a real vehicle or for any
> safety-critical purpose. Use at your own risk.

The standalone instrument crate family: `no_std` flight instrument
panels that emit an immediate-mode scene command stream, plus the
machinery that keeps every backend honest — a reference rasterizer with
bit-exact pinned frame hashes, a shared conformance corpus, a panel
admission harness, and the instrument requirement registry with its
evidence graph.

## The boundary

This repository is upstream of every shell that displays its panels.
The dependency closure contains no wire protocol, no host, no client:
consumers (the Pilotage cockpit, avionics-link, native shells) pin this
repository by git rev and integrate through three published surfaces:

- the state ABI (`indicate-instrument-state::abi::v8`) — a
  self-delimiting tagged-group frame; presence is meaning, sources with
  different group sets drive the same panels;
- the scene IR (`indicate-instrument-scene`) — the opcode vocabulary a
  backend interprets;
- the registry (`indicate-instrument-registry`) — panel descriptors,
  required groups, baselines, and the cross-shell scene digest.

CI enforces the direction: a step fails if the closure ever reaches a
consumer crate. See [`crates/README.md`](crates/README.md) for the crate
map, [`docs/instruments/backend-contract.md`](docs/instruments/backend-contract.md)
for the contract every backend author must read, and
[`docs/instruments/panel-contract.md`](docs/instruments/panel-contract.md)
for its mirror: what a panel author must deliver.

## Pinning and advancing

Consumers pin an exact rev (the same discipline this ecosystem uses for
Aviate and Navigate). A bare rev says nothing about what it contains, so
a rev meant to be pinned is given an annotated tag and an entry in
[`CHANGELOG.md`](CHANGELOG.md), both naming the five values that decide
whether it is the rev you want: state ABI, scene format, corpus,
composition digest, and the panel set. `git show <tag>` then answers
"which rev has ABI v8 and corpus v4?" without a checkout. CI fails when
the newest entry disagrees with the tree it describes; cutting the tag
itself is a release step in [`CONTRIBUTING.md`](CONTRIBUTING.md),
because a release tags its own merge commit and so cannot be verified
by the build that creates it.

The five values are the human index. The full set this revision pins is
[`release-manifest.json`](release-manifest.json): the state ABI version,
the scene format version, the corpus version and sha256, the composition
digest, the screen-composition digest, the per-panel raster baselines,
the glyph-pack content hash, and the criticality bands. It is generated
from the definitions themselves by `cargo xtask gen-release-manifest`
and CI regenerates and diffs it, so it cannot drift from the code it
describes.

Be clear about what it does and does not do. It **records what this
revision pins** — one file a consumer can fetch at a rev and diff
between revs. It **cannot verify a consumer's pins**: nothing here knows
what any consumer wrote down. That check belongs in the consuming
repository, written against this file — compare your pinned values to
the manifest at the rev you pin, and fail your own build on a
disagreement. Vendoring a copy of the corpus or reading it out of the
pinned checkout are both fine; the manifest is what lets either prove it
is current.

The pilot of a change that moves the cross-shell
scene digest advances the pin in the consuming repositories as part of
that change; the advance is complete exactly when every consumer
reproduces the new digest. The scene-conformance corpus
(`crates/indicate-instrument-scene/corpus/`) is versioned and
sha256-pinned by every interpreter, so a corpus edit here turns pinned
consumers red at their next advance instead of drifting silently.

## Gates

`cargo test --all-targets` plus, in CI: the eleven-crate `no_std`
closure compiled standalone for `thumbv7em-none-eabihf`, REN-03 frame
hashes, REN-04 target timing, REN-02 glyph-pack integrity, the
admission/digest bench smoke, the AIR-* requirement registry check, the
standards-registry drift guard, the certification-claim guard, and the
evidence trace gate over `docs/instruments/evidence-graph.evg`.
