# Indicate

SIM / NOT FOR FLIGHT. Nothing here is certified, approved, or airworthy.

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

- the state ABI (`pilotage-instrument-state::abi::v6`) — a
  self-delimiting tagged-group frame; presence is meaning, sources with
  different group sets drive the same panels;
- the scene IR (`pilotage-instrument-scene`) — the opcode vocabulary a
  backend interprets;
- the registry (`pilotage-instrument-registry`) — panel descriptors,
  required groups, baselines, and the cross-shell scene digest.

CI enforces the direction: a step fails if the closure ever reaches a
consumer crate. See [`crates/README.md`](crates/README.md) for the crate
map and [`docs/instruments/backend-contract.md`](docs/instruments/backend-contract.md)
for the contract every backend author must read.

## Pinning and advancing

Consumers pin an exact rev (the same discipline this ecosystem uses for
Aviate and Navigate). The pilot of a change that moves the cross-shell
scene digest advances the pin in the consuming repositories as part of
that change; the advance is complete exactly when every consumer
reproduces the new digest. The scene-conformance corpus
(`crates/pilotage-instrument-scene/corpus/`) is versioned and
sha256-pinned by every interpreter, so a corpus edit here turns pinned
consumers red at their next advance instead of drifting silently.

## Gates

`cargo test --all-targets` plus, in CI: the eleven-crate `no_std`
closure compiled standalone for `thumbv7em-none-eabihf`, REN-03 frame
hashes, REN-04 target timing, REN-02 glyph-pack integrity, the
admission/digest bench smoke, the AIR-* requirement registry check, the
standards-registry drift guard, the certification-claim guard, and the
evidence trace gate over `docs/instruments/evidence-graph.evg`.
