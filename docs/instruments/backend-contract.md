# Backend contract

What every backend author — and every consumer advancing a pin — must
know before touching this family. The properties here are structural:
they hold because of how the crates are shaped, and the gates named
below keep them from eroding.

## Zero-overhead is structural, not an optimization

A panel emits its **complete** command stream every frame, in immediate
mode, into a caller-supplied buffer:

- `SceneWriter::new(buf: &mut [u8])` borrows the caller's bytes and
  returns a typed error rather than an unusable writer; `finish()`
  returns the encoded length. A command that does not fit rolls back
  whole (`SceneError::BufferFull`), so the stream always ends at a
  command boundary — never a truncated frame. No allocation happens
  anywhere on the emit path — the closure crates are `no_std` and CI
  compiles them standalone for `thumbv7em-none-eabihf` to prove it.
- The stream is bounded by compile-time ceilings in
  `indicate-instrument-scene::layer`: `MAX_LAYER_COMMANDS` (4096 per
  layer), `MAX_STACK_DEPTH` (32 save/restore levels), and
  `MAX_SCENE_BYTES` (64 KiB per scene). `validate_layers` enforces them
  as typed `LayerError`s (`OverCapacity`, `StackOverCapacity`,
  `SceneTooLarge`): run it on every scene before the bytes reach a
  backend — the encoder deliberately does not re-check the layer
  ceilings.
- Adding a panel adds commands — never a new mechanism. A backend that
  interprets the opcode vocabulary renders every current and future
  panel; the Swift SceneKit backend is frozen against the opcodes, not
  the panel set.

There is no retained scene graph, no diffing, and no per-frame heap
traffic to tune. If a backend is slow, the cost is in the backend.

## The design frame

A panel is authored in the logical frame its descriptor declares
(`PanelDescriptor::design_frame`; 480×360 for every shipped panel).

- Every backend clips at the design frame: ink outside it never reaches
  a pixel, on any backend.
- Inside the frame, coordinates are logical units. Backends scale to
  their surface; they never reinterpret geometry.
- Unclipped text whose nominal ink extends past the frame edge is a
  counted admission warning, ratcheted per panel by the conformance
  harness. Growing the count is a deliberate, reviewed decision — never
  drift. Fixing overflowing paint moves frame hashes and is its own
  change, at which point the ratchet steps down.

## Two disciplines every backend must follow

1. **Never rebuild path objects per frame.** Glyph outlines and reusable
   geometry are identity-stable; a backend builds its native path/mesh
   objects once per identity and replays them. Rebuilding per frame
   turns a bounded command replay into unbounded allocator and driver
   churn, and every platform's profiler will point somewhere else first.
2. **Repaint on the display refresh clock, never on data arrival.** Data
   arrival updates state; the paint clock samples it. Painting per
   packet couples frame rate to telemetry rate in both directions —
   pyG5's repaint-per-packet loop is the named anti-pattern — and makes
   liveness indistinguishable from link health. The liveness deadline
   (REN-04's 1000 ms frame budget derives from it) assumes a repaint
   clock that keeps ticking when data stops.

## Conformance: what "correct" means

An interpreter is correct only **relative to a corpus version**. The
scene-conformance corpus lives at
`crates/indicate-instrument-scene/corpus/scene-conformance-corpus.json`,
authored by the reference rasterizer and replayed by every backend.
Every interpreter pins `corpusVersion` + `corpusSha256`; a corpus edit
here reddens each pinned consumer at its next pin advance. That red is
the sync mechanism working — resolve it by re-running the consumer's
suite against the new corpus, never by unpinning.

The reference rasterizer's own frames are pinned by SHA-256 (REN-03) in
the panel descriptors. A hash mismatch is a determinism regression
unless the change deliberately moved paint; a deliberate move re-pins
once, with the reason recorded in the change.

## Feeding: sources declare, panels require

A source declares the state groups it supplies (ABI v6 tagged groups —
presence is meaning); a panel descriptor declares the groups it
requires. An unfed group renders `Missing` by construction, not because
a producer remembered to flag it. Sources with different group sets — a
flight controller posture and a data-gateway posture, neither a subset
of the other — drive the same panels; the posture tests in
`indicate-instrument-state` pin exactly that.

## Pin advance

Consumers pin this repository by exact git rev. The pilot of a change
that moves the cross-shell scene digest advances the pin in the
consuming repositories as part of that change; the advance is complete
exactly when every consumer reproduces the new digest.
