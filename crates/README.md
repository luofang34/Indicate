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

A new identifier of that kind is minted in the same scheme rather than a
fresh one, so the table below is the whole convention and not a list of
exceptions to it.

| String | Why it reads `pilotage` |
|---|---|
| `SCENE_DIGEST_DOMAIN` (`b"pilotage-scene-digest-v1"`) | Hashed into every composition digest; changing it moves `BUILTIN_SCENE_DIGEST` and reddens every consumer pin. |
| `COMPOSITION_DIGEST_DOMAIN` (`b"pilotage-screen-composition-digest-v1"`) | **Newly minted**, not carried over: a domain separator's whole job is to be one fixed string consumers pin, so it was spelled to match the row above rather than starting a second convention. |
| `sha256(b"pilotage")` in the sha256 unit tests | A hash input, not a name. The vector and its expected digest are self-consistent whatever the bytes spell. |
| Recorded run records that were not re-executed — the reference CDC capture notes, and the evidence crate's own fixtures | A record is a statement about a run that happened. Records whose suites *were* re-run under the new names were re-captured instead, and say so. |
| `Pilotage` in prose naming the cockpit, its repository, or its browser shell | Correct — those name a consumer, which is what the word is for. |

Renaming *is* right for anything a reader would take as a claim about
what this repository is: crate and package names, workspace members,
`-p` flags, module paths, and doc prose about the family itself.

The corpus's `generatedBy` provenance line is renamed, which looks like
an exception and is not one: `corpusSha256` hashes the concatenated case
bytes only, so the header sits outside it, and interpreters pin
`corpusVersion` and `corpusSha256` rather than the file. Naming a
generator crate that no longer exists would have been the oversight. An
interpreter that hashes the whole JSON document instead of the published
field is the one consumer this would disturb.

## Tiers

The tree has three library tiers and a tools directory. Each tier states
what it may depend on, and `check-structure.sh` fails the build on a
manifest that reaches outside its tier — the dependency law is the
structure, so it is enforced rather than described. The same check keeps
the tables below from drifting: every workspace library crate needs a
row, and every row must name a crate that exists.

### Kernel — `crates/`

The `no_std` closure an instrument may draw against. Depends on the
kernel only.

| Crate | Role |
|---|---|
| `indicate-frames` | Frame and rotation vocabulary (`Quat`); dependency-free leaf. |
| `indicate-alerts` | Alert model (`AlertOutput`, stack semantics); leaf. |
| `indicate-sha256` | Streaming SHA-256 (`Sha256Ctx`) for `no_std` digest pinning; leaf. |
| `indicate-instrument-state` | Aircraft display state, the self-delimiting tagged-group ABI (`abi::v6`), group statuses, posture fixtures. |
| `indicate-instrument-scene` | Scene IR: layers, opcodes, budgets, structural validation; owns the scene-conformance corpus (`corpus/`). |
| `indicate-instrument-glyphs` | Controlled glyph pack: manifest, integrity hashes, coverage requirements. |
| `indicate-instrument-symbology` | Shared symbology: palette, never-skinnable safety constants, status paint, annunciations. |
| `indicate-instrument-descriptor` | The vocabulary a set is written against: panel identity, group sets, config blobs and their key schema, canonical states, and the `PanelSet` a provider exports. |
| `indicate-instrument-feeder` | Source admission ladder shared by every feeding shell. |

### Verification and registry — `crates/`

Composition, admission, the rendering reference, and lifecycle evidence.
Depends on the kernel and on this tier. **Consumes sets; is never a
normal dependency of one.**

| Crate | Role |
|---|---|
| `indicate-instrument-registry` | Composition of descriptors into a validated registry, and the cross-shell scene digest. |
| `indicate-instrument-raster` | Reference software rasterizer: bit-exact pinned frame hashes, the composed-screen harness, corpus authorship, target-timing evidence (`evidence/`). |
| `indicate-instrument-conformance` | Panel admission harness (host-side, allocates; deliberately outside the `no_std` closure). |
| `indicate-evidence` | Standard-neutral lifecycle evidence graph and gate (`evidence-gate` binary); guards `docs/instruments/evidence-graph.evg`. |

### Sets — `sets/`

Panel providers, one crate per set, each exporting one `PanelSet`. A set
lives here whether or not it was written in this repository.

Normal dependencies are kernel-only, so a set cannot reach the machinery
that judges it. The registry is allowed as a **dev**-dependency: that
lets a set pin its own scene digest without standing up a shell, and a
test-graph edge is not a shipping one.

Writing one: `../docs/instruments/panel-contract.md` is what a set must
honour, and `indicate-instrument-template` is the smallest panel that
passes admission — copy it rather than starting from a shipped panel.

| Crate | Role |
|---|---|
| `indicate-instrument-panels` | The shipped panels (PFD, HSI, monitor): immediate-mode scene emission per frame. |
| `indicate-instrument-template` | The smallest set that passes admission, written to be read and copied; its own test admits it, so the worked example cannot drift from the contract. |

### Tools — `tools/`

Shells, not a library tier, and unconstrained: `tools/instrument-bench`
(a registry-only shell that reproduces the composition digest and runs
admission) and `tools/xtask` (`gen-state-fixture` golden-frame
generation).
