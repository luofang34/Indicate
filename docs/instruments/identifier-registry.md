# Identifier registry

Code and documents in this repository cite identifiers from several
families. This registry says what each family is, where its members are
defined, and what every work-package and decision identifier that the
Rust sources cite means. `scripts/check-identifier-registry.sh` fails CI
on an identifier cited in a `.rs` file that has no row here, so a
citation always resolves to a definition a reader can reach from a
clone.

## Families

| Family | Shape | Defined in |
|---|---|---|
| Requirements | `AIR-<AREA>-<NNN>` | [requirements.md](requirements.md); every reference links to its anchor. |
| Failure conditions | `FC-<AREA>-<NN>` | [fha.md](fha.md), the hazard log. |
| Standards rows | `STD-<NNN>` | [standards-applicability.md](standards-applicability.md). |
| Evidence plan items | `EVP-<NN>` | [evidence-plan.md](evidence-plan.md). |
| Work packages | `<AREA>-<NN>` | This registry, below. A work package names one vertical slice of behaviour and the guardrails that hold it; the name is stable so a comment can cite it. |
| Architecture decisions | `ADR-<NNNN>` | The Pilotage repository, `docs/adr/`. The texts are not in this repository; the rows below state what each decision settles as it is cited here. |

## Work packages

| Identifier | Meaning | Where it lives |
|---|---|---|
| `ALR-01` | Deterministic central flightcrew alert manager and aural contract: typed conditions in, priority-ordered alert list and one aural token out, with no interior clock. | `indicate-alerts` |
| `ALT-01` | Datum-qualified altitude resolution: an altitude is displayed only with its class, origin, and geoid model, and never falls back to another datum. | `indicate-instrument-state::altitude`, `resolve::altitude_signal` |
| `ASSURE-01` | Standard-neutral lifecycle evidence graph and the scoped no-orphan gate over it. | `indicate-evidence` |
| `ASSURE-04` | Durable baselines: a recorded result names a commit reachable from `HEAD`, never a lane commit a rewrite would orphan. | `indicate-evidence::gate::baseline` |
| `ATT-01` | The attitude presentation vertical slice: unusual-attitude thresholds, declutter, and the horizon, traced from hazard to recorded result in the evidence graph. | `indicate-instrument-state::presentation`, `indicate-instrument-panels::pfd::horizon`, `docs/instruments/evidence-graph.evg` |
| `AV-01` | Measurement identity stamps and the per-role legality rules: role, clock domain, integrity vocabulary, and wrap-safe serial ordering, judged once for every shell. | `indicate-instrument-feeder::stamp`, `avionics` |
| `DISP-01` | Renderer-health display reasons: the append-only registry of reasons a shell reports, each with a wire code and an alert class. | `indicate-alerts::condition::DisplayFault`, [display-reason-registry.md](display-reason-registry.md) |
| `DISP-02` | Readout extents: a tape's readout stays inside its declared extent across the representable value range, so a backend needs no per-value clipping. | `indicate-instrument-panels::pfd::tapes` |
| `DYN-01` | Measurement-coherent turn-rate derivation: heading is differenced only between two accepted measurements of one stream over the acquisition clock. | `indicate-instrument-feeder::turn`, `indicate-instrument-state::dynamics` |
| `FD-01` | Flight-director command presentation: command bars from validated pitch and roll commands, removed under degradation and declutter. | `indicate-instrument-state::director` |
| `FRAME-01` | Typed reference frames and six-degree-of-freedom vehicle state: every attitude, position, and velocity carries its frame, epoch, clock, and scale; unit quaternions are the one rotation kernel. | `indicate-frames` |
| `HOST-01` | The reference host: admit one snapshot per generation, paint on the display clock, resolve once and step the alert manager once per frame, watch renderer liveness, paint the failure page, and route operator intents. | `tools/indicate-host` |
| `INT-01` | Typed operator intents: the return path from a human controller to the system, admitted by device identity, sequence, clock, and bounds, and confirmed only by echo through the selections group. | `indicate-intent`, [intent-contract.md](intent-contract.md) |
| `IXN-01` | Deterministic spatial interaction algebra: bounded orbit, pan, zoom, and reset transitions over `ViewPose3D` without mutating vehicle state. | `indicate-interaction` |
| `NAV-01` | Navigation quantities converted into the display reference: course, bearings, and the heading bug render only after conversion into the rose reference. | `indicate-instrument-state::heading`, `resolve::heading_signal`, `resolve::bearings_signal` |
| `NAV-02` | One freshness policy for every converted navigation quantity, so no converted value outlives the source it was converted from. | `indicate-instrument-state::resolve` |
| `PLUG-01` | Signed `.indpack` WebAssembly instrument packages, zero-import fuel-metered guest execution, and host admission before registration. | `indicate-plugin-abi`, `indicate-plugin-host` |
| `REN-02` | The controlled glyph pack: mandatory instrument text renders from a project-authored, hash-pinned bitmap font with no system-font fallback. | `indicate-instrument-glyphs`, `indicate-instrument-raster::text`, [glyph-pack.md](glyph-pack.md) |
| `REN-03` | Bit-exact reference rasterizer frames: every shipped panel's frame hash is pinned and re-asserted in CI. | `indicate-instrument-raster::raster::tests::frame_hashes`, [renderer-verification.md](renderer-verification.md) |
| `REN-04` | Target timing and conformance: the per-class work budget, the provisional target timing model, and the conformance corpus every backend replays. | `indicate-instrument-raster::timing`, `report`, [renderer-verification.md](renderer-verification.md) |
| `SRC-01` | Per-function source comparison and reversion: dual-source agree, miscompare, and return-to-primary with hysteresis, and the typed alert on each edge. | `indicate-instrument-state::source_compare`, `source_monitor` |
| `VAL-01` | Fail-safe decoding and resolution: trust is declared, unknown wire values fail rather than map to benign ones, and no non-finite value reaches scene generation. | `indicate-instrument-state::abi`, `validate`, `resolve` |
| `VOL-01` | Bounded three-dimensional volumetric scene IR (`VolumeScene`), provenance-bearing spatial billboards, and volume admission checks. | `indicate-instrument-scene::volume`, `indicate-instrument-volumes`, [set-volumes.md](set-volumes.md) |
| `XPLANE-01` | The X-Plane source adapter: the simulator's UDP data interface decoded sans-IO, attitude and kinematics admitted through the truth-role gate on the simulation clock, the remaining groups aged from receipt, and operator intents written back as datarefs. | `tools/indicate-source-xplane`, [source-xplane.md](source-xplane.md) |

## Architecture decisions cited from code

| Identifier | What it settles, as cited here |
|---|---|
| `ADR-0009` | Clock-domain discipline: readings from different clocks are never compared without an explicit correlation step; freshness thresholds resolve an age into a status. |
| `ADR-0015` | Structural limits enforced by tooling: file and function size, module naming, and the disallowed diagnostic macros. |
| `ADR-0017` | The versioned scene IR: a small append-only opcode vocabulary, backend-owned text metrics, and the glyph pack behind a manifest. |
| `ADR-0026` | Posture acceptance: sources with different group sets drive the same panels, and an unfed group resolves `Missing` by construction. |
| `ADR-0029` | Extensible state groups and admission by conformance: the tagged-group ABI, foreign panels admitted by the harness rather than by trust, and one feeder implementation linked by every shell. |
| `ADR-0031` | The navigation display profile: the one place wire meters become instrument dots, with absent guidance displayed as absent. |
| `ADR-0033` | The panel descriptor vocabulary and configuration as a bounded key-TLV blob. |
