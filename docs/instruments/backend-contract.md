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
  panel; the Apple Core Graphics backend is frozen against the opcodes,
  not the panel set.

There is no retained scene graph, no diffing, and no per-frame heap
traffic to tune. If a backend is slow, the cost is in the backend.

## The design frame

A panel is authored against a *range* of logical frames, and it is drawn
at one of them. The frame is an argument to the draw call, not a
descriptor constant and not a configuration key — configuration is an
optional schema-gated blob that admission deliberately leaves empty, so
a panel taking its size from a key could never be admitted.

The descriptor declares what a shell may ask for:

| Field | What it says |
|---|---|
| `frame_min` | The readability floor. Conspicuity must hold here ([`AIR-OUT-004`](requirements.md#air-out-004)), and group regions are validated against it. |
| `frame_max` | The ceiling the work budget allows. |
| `frame_step` | Per-axis quantization: admissible dimensions are `frame_min + k * step`. |
| `aspect_min`, `aspect_max` | The width/height ratios the layout supports. The per-axis corners alone admit shapes no layout declared. |
| `canonical_frames` | The pinned evidence sizes. Must include both ends of the range, and every entry must satisfy every rule above. |

`Registry::new` refuses a *declaration* that breaks any of them, and
checks every canonical frame against all of them, so the sizes the
evidence is pinned at are shapes the panel really declared.

Note how far that reaches, and how far it does not: a valid range may
still *contain* an in-range, on-grid frame that violates the aspect
bounds. Choosing a frame inside the range, and clamping to what the
panel supports, is the shell's job — nothing in the draw path re-checks
the argument.

**Do not re-derive the rule.** `PanelDescriptor::accepts(DesignFrame)`
is the predicate over those fields, and it is the only copy of it. It
returns a typed `FrameRefusal` naming the bound that was violated —
`Degenerate`, `OutOfRange`, `OffStep`, or `Aspect` — so a shell can tell
its operator what to ask for instead, and the diagnostic reads the same
on every shell. The step tolerance is `FRAME_STEP_TOLERANCE`, which is
zero, decided once where the constants live: a step that cannot express
a frame exactly is a declaration to fix, not a rounding to absorb.

A shell that writes its own version of this rule will differ from
another shell's — on the tolerance, on whether a bound is inclusive, on
whether the step is measured from the minimum or from zero — and each
will be locally green, because each only ever tests its own. The panel
is the one thing that knows which frames it accepts, so ask the panel.

Which admissible frame to ask for is decided once, here, for the same
reason. A shell left to choose would invent a policy, and two shells
given the same space and the same descriptor would ask for different
frames.

- **Ask for the largest admissible frame that fits the space.** A
  frame fits a space when its width and height are not larger than the
  space's. Test each candidate with `accepts`; the predicate is the
  authority, and the arithmetic above is not to be re-implemented.
- **Break ties by area, then by width, then by height.** Choose the
  fitting admissible frame with the largest area. If two have equal
  area, choose the wider one. If the widths are equal, choose the
  taller one. The order is total, so the choice is deterministic.
- **Refuse a space smaller than `frame_min`.** `frame_min` is the
  readability floor. Do not ask for a frame below it, and do not serve
  the space by shrinking the frame: there is no `SCALE` opcode, and
  nothing on the draw path re-checks the argument.
- **When no space constrains the choice, ask for the first canonical
  frame.** A bench harness is the example.

Every shipped panel currently declares a degenerate range —
`frame_min == frame_max == 480×360`, one canonical frame — so the only
frame a conforming shell may ask any of them for is 480×360. The
choosing policy resolves to that one frame today; it exists so that
when a panel ships a real range, every shell makes the same choice.

- Every backend clips at the frame it asked for: ink outside it never
  reaches a pixel, on any backend.
- Inside the frame, coordinates are logical units. Backends scale to
  their surface; they never reinterpret geometry. A larger surface is
  served either by scaling the frame up or, once a panel declares a real
  range, by asking for a larger frame — the choice is the shell's, and
  the mapping stays one uniform scale either way.
- Unclipped text whose nominal ink extends past the frame edge is a
  counted admission warning, ratcheted per panel and per canonical frame
  by the conformance harness. Growing the count is a deliberate,
  reviewed decision — never drift. Fixing overflowing paint moves frame
  hashes and is its own change, at which point the ratchet steps down.

## Screen composition

Several panels may share one surface ([`AIR-OUT-011`](requirements.md#air-out-011)).
A `CompositionDescriptor` in `indicate-instrument-registry` declares the
logical screen and an ordered list of `Slot`s — a panel id, a rect in
screen units, and the `occludes` list naming the panels below whose
ordinary symbology this slot is allowed to cover. **Slot index is z**,
exactly as `LayerId`'s discriminant is z within a scene: later slots
paint above earlier ones, and declaration order is paint order
everywhere.

What a backend does with it:

- **Paint slots in index order.** Per slot: clip to the slot rect,
  translate to the slot origin, and replay that panel's validated scene.
  That is the whole of the placement — the `stride_bytes` sub-window the
  reference rasterizer already supports, generalized to overlap.
- **One global uniform mapping** from screen-logical units to the
  surface. A slot gets no mapping of its own; there is no per-slot
  rotation, and no slot is scaled to fit its rect. The slot's dimensions
  *are* the frame its panel was asked to emit, which is why
  `validate_composition` refuses a slot sized outside the panel's
  declared frame range.
- **No offscreen surfaces and no retained graph.** A slot's cost is a
  function of its own scene and never of what lies beneath it, so
  overdraw from stacking is bounded by construction. The ceilings are
  sums: total encoded bytes at most `MAX_COMPOSITION_SLOTS ×
  MAX_SCENE_BYTES`, composed-frame work at most the sum of the per-slot
  `RenderWork` budgets, and the composed timing envelope the sum of the
  slot envelopes against the same liveness-derived deadline (REN-04).
- **One resolved snapshot and one `AlertOutput` per composed frame**,
  fanned to every slot. Two overlapping panels disagreeing because they
  resolved different snapshots is a misleading display
  ([`AIR-BAS-007`](requirements.md#air-bas-007)); resolving once is the
  rule, not an optimization.
- **A slot that fails at run time paints its failure inside its own rect
  and nowhere else,** and suppresses, delays, and alters nothing in any
  other slot.
- **Per-slot configuration is shell-supplied at draw time,** not
  declared in the composition. The descriptor declares layout; the feed
  supplies data and configuration. It therefore does not join the
  screen-composition digest, and a shell that reconfigures a panel still
  composes the same screen.

What `validate_composition` refuses at init, before first paint — a
composition fault is a declaration error, never a rendering curiosity:
an unregistered panel id; a rect that is not finite, is degenerate, or
leaves the screen; a slot sized to a frame its panel does not support;
more than `MAX_COMPOSITION_SLOTS` (8) slots; a slot wholly covered by
the opaque slots above it; and undeclared obscuration.

Overlap rides `BackgroundCapability` rather than any new alpha
mechanism. `Opaque` and `Cedeable` panels prove a full-frame opaque
cover at admission, so they occlude their whole rect and count toward
burying a slot below; a `NotUsed` panel paints nothing in the background
band and functions as an overlay through which lower slots show.

**The obscuration floor.** Two rules, and the difference between them is
what [`AIR-OUT-011`](requirements.md#air-out-011) requires:

| What may be covered | Where it comes from | Rule |
|---|---|---|
| Ordinary symbology | the lower panel's declared `group_regions`, placed at its slot origin | covered only where the covering slot names the lower panel in `occludes` |
| Criticality content | the *measured* union `Annunciation`/`Failure` ink bound, per panel × frame | may not be covered at all |

A declaration reaches the first row and never the second:

> **Declaring buys you readouts, never warnings.**

Naming a panel in `occludes` permits covering its ordinary symbology; it
does not permit covering its warnings or its failure indications,
because a declaration that could conceal a warning would be a
declaration that the warning does not matter. The criticality bound is
measured rather than declared for the same reason: a panel able to name
its own warning surface could also understate it.

**The floor is exactly the two bands, and no wider.** It protects what a
panel paints into `Annunciation` and `Failure`, whatever that is, and it
does not protect anything a panel paints elsewhere, whatever that says.
Two consequences worth stating rather than discovering:

- The simulation labelling [`AIR-BAS-001`](requirements.md#air-bas-001)
  and [`AIR-FLAG-007`](requirements.md#air-flag-007) require is covered
  by this floor **only if the panel paints it into a criticality band**.
  No shipped panel emits that labelling at all today, so this is an
  obligation on a panel author, not a property the composition layer
  supplies.
- A band is only as wide as the cases that measured it. A failure cue no
  corpus or extreme state drives is not in the bound and is not
  protected; `panel-contract.md` says the same thing to the author who
  can fix it.

The bound a composition validates against is what admission measured
over the whole canonical × extreme × withheld case matrix, pinned beside
the raster baselines. A panel with no band pinned at the size its slot
asks for is refused rather than assumed quiet.

The **screen-composition digest** has its own domain string and covers
the screen frame, the ordered slots — panel id, rect, and `occludes`
list — and the scene digest beneath, so it is strictly stronger than the
scene digest: two shells agreeing here agree both about what their
panels paint and about where. Slot rects are in it from day one, so
relaxing which rects are admissible changes what validates and never
what the digest covers. Pin-advance discipline is the same as for the
scene digest.

Composition does not change a panel: its modes, its reversion, and its
failure presentation are what they were standalone.

## Where the vocabulary deliberately stops

Two properties of the opcode set decide what an instrument can be. One
is settled contract, and a backend author who finds it missing should
stop looking for the version that adds it. The other is genuinely open,
and is recorded here with its price rather than left to be discovered.

**There is no `SCALE`, and there will not be one.** The transform ops
are translate and rotate. A panel is authored against the frame it is
handed and a backend maps that whole frame to the viewport, so an
instrument cannot be drawn at a different size *within* a scene. That is
the contract: panels compose, instruments inside them do not. Sizing is
a layout decision inside the panel — which is why the frame is an
emission input rather than a transform over the output.

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

A source declares the state groups it supplies (ABI v7 tagged groups —
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
