# Crate map

Leaf-first index of the instrument crate family. Every crate here is
consumer-agnostic in its dependencies: nothing in this workspace depends
on a host, a wire protocol, or a shell (CI's downstream-agnostic gate
holds that closed). Consumers pin this repository by git rev and depend
on the published crate surfaces only.

| Crate | Role |
|---|---|
| `pilotage-frames` | Frame and rotation vocabulary (`Quat`); dependency-free leaf. |
| `pilotage-alerts` | Alert model (`AlertOutput`, stack semantics); leaf. |
| `pilotage-sha256` | Streaming SHA-256 (`Sha256Ctx`) for `no_std` digest pinning; leaf. |
| `pilotage-instrument-state` | Aircraft display state, the self-delimiting tagged-group ABI (`abi::v6`), group statuses, posture fixtures. |
| `pilotage-instrument-scene` | Scene IR: layers, opcodes, budgets, structural validation; owns the scene-conformance corpus (`corpus/`). |
| `pilotage-instrument-glyphs` | Controlled glyph pack: manifest, integrity hashes, coverage requirements. |
| `pilotage-instrument-symbology` | Shared symbology: palette, never-skinnable safety constants, status paint, annunciations. |
| `pilotage-instrument-panels` | The shipped panels (PFD, HSI, monitor): immediate-mode scene emission per frame. |
| `pilotage-instrument-raster` | Reference software rasterizer: bit-exact pinned frame hashes, corpus authorship, target-timing evidence (`evidence/`). |
| `pilotage-instrument-registry` | Panel descriptors, group sets, config blobs, canonical states, the cross-shell scene digest. |
| `pilotage-instrument-feeder` | Source admission ladder shared by every feeding shell. |
| `pilotage-instrument-conformance` | Panel admission harness (host-side, allocates; deliberately outside the `no_std` closure). |
| `pilotage-evidence` | Standard-neutral lifecycle evidence graph and gate (`evidence-gate` binary); guards `docs/instruments/evidence-graph.evg`. |

Tools: `tools/instrument-bench` (registry-only shell that reproduces the
composition digest and runs admission), `tools/xtask`
(`gen-state-fixture` golden-frame generation).
