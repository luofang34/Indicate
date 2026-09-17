# Architecture overview

This page is the first thing to read. It shows how data moves from a
source to a display and back, which crate owns each stage, and which
documents to open next. Every other document under `docs/instruments/`
is a contract for one stage.

## Data flow

```text
  source (simulator, vehicle bus, replay)           untrusted
       │  wire bytes, decoded by the shell
       ▼
  indicate-instrument-feeder                        crates/
       │  admission ladder: identity, order, clock, authorization
       │  aircraft_state::apply_* writes the admitted lanes
       ▼
  indicate-instrument-state                         crates/
       │  AircraftState, tagged-group ABI v8, resolve → PanelData
       │  every signal carries Valid / Degraded / Stale / Missing / Failed
       ▼
  indicate-host                                     tools/
       │  one snapshot per generation, paint clock, one resolve and one
       │  alert step per frame, liveness watchdog, failure page
       ▼
  sets/*  PanelDescriptor::draw                     sets/
       │  scene IR into a caller buffer, six z-ordered layers
       ▼
  indicate-instrument-registry                      crates/
       │  composition: slots, occlusion floor, digests
       ▼
  backend                                           here: reference rasterizer
       │  RGBA8 frame                               elsewhere: Canvas2D, Core Graphics
       ▼
  display surface

  human ──▶ indicate-intent ──▶ indicate-host ──▶ source
            request admitted    routed            echoed back through the
            by device, order,                     selections group; the display
            clock, bounds                         paints only the echo
```

## The three published surfaces

Consumers pin this repository by revision and integrate through three
surfaces. Everything else is internal.

| Surface | Crate | Contract |
|---|---|---|
| State ABI | `indicate-instrument-state::abi::v8` | A self-delimiting tagged-group frame. Presence is meaning. |
| Scene IR | `indicate-instrument-scene` | The opcode vocabulary a backend interprets. [Backend contract](backend-contract.md), [scene-layer protocol](scene-layer-protocol.md). |
| Registry | `indicate-instrument-registry` | Panel descriptors, composition, and the cross-shell digests. [Panel contract](panel-contract.md). |

## Tiers

| Tier | Directory | May depend on | Holds |
|---|---|---|---|
| Kernel | `crates/` | Kernel | State, scene IR, glyphs, symbology, descriptors, feeder, intents. `no_std`. |
| Verification | `crates/` | Kernel, verification | Registry, reference rasterizer, admission harness, evidence gate. Consumes sets. |
| Sets | `sets/` | Kernel | Panel providers. Each has a [set document](README.md#set-documents). |
| Tools | `tools/` | Anything | Shells: the reference host, the X-Plane source, the benches, xtask. |

`scripts/check-structure.sh` enforces the dependency direction.

## Fail-safe rules that hold everywhere

- An absent group resolves `Missing` by construction; no producer opts
  in ([`AIR-FLAG-004`](requirements.md#air-flag-004)).
- An unknown wire value fails its group; it never maps to a benign one.
- A clock the code cannot trust (not a number, negative, backwards)
  refuses the publication and ages the state as `Missing`.
- A numeral a panel paints must claim the group it came from; a dash
  must not. Admission proves both.
- A frame is made on the display clock, never on data arrival. A frame
  made from stale data is a live frame that carries stale data.
- A request from a human is a request, never a value. The display
  paints only what the source echoes back.

## Where to start

| You want to | Read | Then run |
|---|---|---|
| See pixels | [cookbook-run.md](cookbook-run.md) | `cargo run -p instrument-bench -- --out target/frames` |
| Feed a simulator | [cookbook-feeder.md](cookbook-feeder.md) | `cargo run -p indicate-source-xplane -- --frames 100 --out target/xp` |
| Write a backend | [cookbook-backend.md](cookbook-backend.md) | `cargo test -p indicate-instrument-scene` |
| Write a set | [cookbook-set.md](cookbook-set.md) | `cargo test -p indicate-instrument-template` |
| Understand the safety baseline | [README.md](README.md) | `bash scripts/check-instrument-requirements.sh` |
