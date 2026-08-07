# Crate map

Leaf-first index of the instrument crate family. Every crate here is
consumer-agnostic in its dependencies: nothing in this workspace depends
on a host, a wire protocol, or a shell (CI's downstream-agnostic gate
holds that closed). Consumers pin this repository by git rev and depend
on the published crate surfaces only.

## Names and identifiers

Crates are named for this repository — `indicate-*` — because Pilotage is
one consumer of the family and not its owner. `check-structure.sh` fails
on a `pilotage-`-named crate directory or package so the scheme cannot
drift back.

A handful of `pilotage` strings survive on purpose, and they are
**identifiers rather than names**: values that were minted once, are
hashed or pinned by someone downstream, and do not track what the crates
are called. Rewriting one buys a tidier grep and costs every consumer a
re-pin for no change in what is painted.

| Survivor | Why it stays |
|---|---|
| `SCENE_DIGEST_DOMAIN` (`b"pilotage-scene-digest-v1"`) | Hashed into every composition digest; changing it moves `BUILTIN_SCENE_DIGEST` and reddens every consumer pin. |
| Recorded evidence run records under `docs/instruments/evidence-artifacts/` | Captured output of runs that really executed under the old package names; a record is a statement about the past. |
| `Pilotage` in prose naming the cockpit, its repository, or its browser shell | Correct — those name a consumer, which is what the word is for. |

Renaming *is* right for anything a reader would take as a claim about
what this repository is: crate and package names, workspace members,
`-p` flags, module paths, and doc prose about the family itself.

| Crate | Role |
|---|---|
| `indicate-frames` | Frame and rotation vocabulary (`Quat`); dependency-free leaf. |
| `indicate-alerts` | Alert model (`AlertOutput`, stack semantics); leaf. |
| `indicate-sha256` | Streaming SHA-256 (`Sha256Ctx`) for `no_std` digest pinning; leaf. |
| `indicate-instrument-state` | Aircraft display state, the self-delimiting tagged-group ABI (`abi::v6`), group statuses, posture fixtures. |
| `indicate-instrument-scene` | Scene IR: layers, opcodes, budgets, structural validation; owns the scene-conformance corpus (`corpus/`). |
| `indicate-instrument-glyphs` | Controlled glyph pack: manifest, integrity hashes, coverage requirements. |
| `indicate-instrument-symbology` | Shared symbology: palette, never-skinnable safety constants, status paint, annunciations. |
| `indicate-instrument-panels` | The shipped panels (PFD, HSI, monitor): immediate-mode scene emission per frame. |
| `indicate-instrument-raster` | Reference software rasterizer: bit-exact pinned frame hashes, corpus authorship, target-timing evidence (`evidence/`). |
| `indicate-instrument-registry` | Panel descriptors, group sets, config blobs, canonical states, the cross-shell scene digest. |
| `indicate-instrument-feeder` | Source admission ladder shared by every feeding shell. |
| `indicate-instrument-conformance` | Panel admission harness (host-side, allocates; deliberately outside the `no_std` closure). |
| `indicate-evidence` | Standard-neutral lifecycle evidence graph and gate (`evidence-gate` binary); guards `docs/instruments/evidence-graph.evg`. |

Tools: `tools/instrument-bench` (registry-only shell that reproduces the
composition digest and runs admission), `tools/xtask`
(`gen-state-fixture` golden-frame generation).
