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

## Where the vocabulary deliberately stops

Two properties of the opcode set decide what an instrument can be. One
is settled contract, and a backend author who finds it missing should
stop looking for the version that adds it. The other is genuinely open,
and is recorded here with its price rather than left to be discovered.

**There is no `SCALE`, and there will not be one.** The transform ops
are translate and rotate. A panel is authored in the frame its
descriptor declares and a backend maps that whole frame to the viewport,
so an instrument cannot be drawn at a different size *within* a scene.
That is the contract: panels compose, instruments inside them do not.

The limitation this looks like is answered a layer up rather than by a
transform. An instrument wanted at two-thirds size beside another is a
finer-grained *panel*, placed at its own frame by the composition layer
— reuse by decomposition, not by rescaling a replay. Which finer-grained
panels exist is a decision about what a set exports, and costs no
vocabulary change. A compositor therefore never resizes a scene by
rewriting its commands, and `panel-contract.md` tells authors the same
thing from the other side.

**A filled annular band has no opcode, and that one is genuinely open.**
Paint mode splits the vocabulary by shape, not by oversight: the
closed-area ops — `RECT`, `CIRCLE`, `POLYGON` — carry a `PaintMode`,
while the open-path ops — `LINE`, `POLYLINE`, `ARC` — are stroke-only,
having no interior to fill. `ARC` is not the odd one out.

That consistency is what makes the question a real one. An engine dial's
green/yellow/red sweeps are expressible today, because a thick stroked
arc reproduces a band — but the band's inner and outer radii then come
out of the stroke width and the radius together, which is the geometry
expressed sideways. A band is an area between two radii, so what it
wants is not a mode byte on `ARC` — a stroke-only op has nothing to fill
— but its own geometry. Whether the sideways expression is sufficient
cannot be decided without the instrument set that needs it, so it stays
open.

What it would cost, recorded now so the price is known when that day
comes: a *new* filled annular-band opcode is **not** a scene-format
version bump, because an unknown ordinary opcode inside a layer is
counted and skipped, so older backends degrade gracefully. It is a
corpus bump, an implementation in every backend, and new admission
geometry — the background-coverage check counts only the painting
commands it knows, so a new one is invisible to it until it is added.

Changing the existing `ARC` payload, by contrast, is off the table, and
for a worse reason than "a hard break": the decoder reads its five
floats from fixed offsets and ignores trailing bytes, so prepending a
mode byte would not raise an error on a pinned interpreter. It would
silently mis-read the centre and radius from shifted bytes and paint the
wrong arc. Opcodes are append-only. The cheap path
exists; take it only with the set in hand.

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
