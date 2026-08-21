# Panel contract

What every panel author must honour. `backend-contract.md` is the other
side of the same boundary: it tells an author interpreting the scene
stream what they may assume. This tells an author *producing* one what
they must deliver.

Everything here is enforced by the admission harness or by
`Registry::new`/`Registry::from_sets`, except where it says otherwise —
and where it says otherwise, it says so plainly, because a contract that
blurs "checked" into "expected" teaches authors to trust the wrong
things.

The worked example is a real crate: `sets/indicate-instrument-template`
is the smallest panel that exercises the honesty rules below — a panel
claiming nothing passes admission more easily and teaches less — it is
admitted by its own test in CI, and its comments explain each field
where it stands. Read it
alongside this document; if the two ever disagree, the crate is right,
because the crate is the one that has to compile.

## Where a panel lives

Under `sets/`, one crate per set family. A crate exports at least one
`PanelSet` and may export more than one: the crate is the compilation
and dependency unit, a `PanelSet` is the composition unit, and a shell
selects sets rather than crates. Two sets belong in one crate when they
carry the same tier obligations and share their geometry; they belong in
separate crates when a consumer of one should not have to compile the
other.

`indicate-instrument-panels` exports two. `BUILTIN_SET` is the shipped
flight set. `CONFIG_SET` holds the configuration panel, which a shell
composes only when the airframe has the sensors — keeping it out of
`BUILTIN_PANELS` is what leaves the composition digest, the
screen-composition digest and the panel-set release value unmoved by a
panel most airframes do not show.

A set's normal dependencies are kernel-only — `crates/README.md` states the tier
law and `check-structure.sh` enforces it, so a set cannot *ship* against
the registry, the rasterizer, or the conformance harness that judges it.
The whole verification tier is permitted as **dev**-dependencies, and a
set needs two of them: the registry to compose itself and pin its own
scene digest, and the conformance harness to run its own admission test.
Neither reaches the shipped artifact.

A set is `no_std`. CI compiles every set for `thumbv7em-none-eabihf` in
the closure check, so reaching for `std` outside `#[cfg(test)]` breaks
the build; the template shows the `#[cfg(test)] extern crate std` idiom
that keeps tests comfortable without loosening the crate.

A set can therefore live outside this repository entirely. Nothing in
the mechanism requires a panel to be written here.

## The descriptor, field by field

A `PanelDescriptor` is the whole of what a shell knows about a panel.
Three tiers of obligation, and the difference matters:

| Field | Obligation |
|---|---|
| `required_layers` | **Load-bearing.** Admission asserts every declared band is present in every case. |
| `required_groups` | **Load-bearing.** This *is* the withholding matrix: admission withholds each declared group in turn. |
| `frame_min`, `frame_max`, `frame_step`, `aspect_min`, `aspect_max`, `canonical_frames` | **Load-bearing.** They decide which frames a shell may ask for, and admission runs the whole matrix at each canonical one. |
| `background` | **Load-bearing.** An `Opaque` or `Cedeable` claim is proven against the emitted ink. |
| `draw` | **Load-bearing.** Refusing to draw any case fails admission. |
| `extreme_states` | **Load-bearing.** Each one multiplies the case matrix. |
| `id`, `title` | Validated at registry init: charset, non-empty. |
| `config_schema` | Validated at init (keys strictly ascending). Admission always draws the empty config. |
| `group_regions` | **Load-bearing.** Admission asserts every declared region is populated by claimed ink, and screen composition plans obscuration around them. |
| `raster_baselines` | Inert unless the panel is in a set the raster tests cover. Each entry must name a canonical frame. |

### `required_layers` — declare what you always emit

A panel requires a band when it *opens* that band on every frame, not
when the band always carries content. The PFD requires `Guidance` and
opens it unconditionally; the flight-director bars inside it disappear
under degradation and declutter, and the empty band is still a complete
frame. `scene-layer-protocol.md` carries the shipped table, checked
against these masks.

Declaring a band you emit only sometimes is the most common way to fail
admission: the harness draws states you did not have in mind, and
`MissingRequiredLayers` names the case.

Do not declare `Background`. It is the one band a compositor may drop,
so requiring it would forbid the composition it exists for — paint it if
you want it, and let the mask stay quiet about it.

### `required_groups` — declare what you consume, honestly

Admission runs your panel once fully fed and then once per declared
group with that group withheld. Declaring a group you do not use costs
cases and proves nothing. Declaring fewer than you use is worse: the
group you left out is never withheld, so the one case that would have
caught a fabricated value never runs.

Follow the data, not the drawing. If a readout folds a second group into
its resolved status — the template's airspeed folds `Trust`, so
withholding `Trust` genuinely dashes the number — that group belongs in
the mask.

### The frame range — declare what you can lay out against

`draw` receives the frame as an argument, alongside the state, the
config blob, and the alerts. Read it. A geometry constant that should
have been `frame.width` is a panel that quietly ignores the size it was
given, and no test can tell the difference while the declared range has
one frame in it.

The frame is deliberately *not* a config key. Configuration is an
optional schema-gated blob and the admission harness draws the empty one
on purpose, so a panel that took its size from configuration would be
unadmittable by construction.

What the registry checks at init:

- `frame_min` and `frame_max` finite and positive, `min <= max` per
  axis. `frame_min` is the readability floor — it is where conspicuity
  has to hold, and it is the frame `group_regions` are validated
  against.
- `frame_step` finite and positive per axis. Admissible dimensions are
  `frame_min + k * step`, exactly: the check applies no tolerance, so a
  step that cannot express your frames is a declaration to fix.
- Aspect bounds finite, positive, `min <= max`.
- `canonical_frames` non-empty, containing both `frame_min` and
  `frame_max`, with every entry in range, on the grid, and inside the
  aspect bounds. The corners alone are not enough: 600×360 sits inside a
  480×360-to-600×450 range on both axes and is a shape a 4:3 layout
  never declared, which is what the aspect check is for.

**A panel may treat its `DesignFrame` as a constant.** Nothing in the
framework lays out for you, and no author owes a size-adaptive layout.
The honest way to say so is a degenerate range: `frame_min ==
frame_max`, one canonical frame, and any positive step. The only size a
*conforming* shell may then ask for is the one the geometry was authored
for — `accepts` is the predicate that says so, and a shell is expected
to consult it, but nothing in the draw path re-checks the argument. Every
shipped panel declares a degenerate range today.

Which frame a shell asks for is the shell's half of this contract, and
your descriptor answers it. `PanelDescriptor::choose_frame` gives the
largest frame that fits a shell's space and that `accepts` admits, so
two shells with the same space and your descriptor ask for the same
frame. You get this from declaring the range honestly; there is nothing
more for you to write.

What is refused is declaring a range and not using it. Admission renders
a non-degenerate panel at both ends of its range, across the whole case
matrix, and requires the bytes to differ in at least one case. One
witness settles it, so a panel whose `nothing-fed` frame is a fixed
placard is not accused, and neither is one whose size only shows under
an alert or in an extreme state. A panel that takes the parameter and
ignores it fails rather than passing on the strength of a range it does
not honour: a shell asking for the larger frame is owed more instrument,
not the same picture for the backend to stretch.

Differing bytes are a weak proof of a good layout and a sufficient proof
of the thing that was otherwise unprovable: that the argument reached
the geometry at all. What each axis should *extend* — tape range versus
tick density, line count, radius versus margin — is a content policy per
panel, and belongs beside the panel that adopts a range.

### The honesty rules, and their asymmetry

This is the part of the contract that is easiest to get exactly
backwards, so it is stated twice.

- **A numeral must carry a claim.** Any digits you paint must go through
  `text_attributed` with the group the number came from. An unclaimed
  numeral fails as `UntaggedNumeral` — even when it is clipped out of
  sight, because the check is over emitted runs, not visible pixels.
- **A dash must not.** The `---` you paint *instead of* a value is not a
  value, and tagging it claims a group that was withheld. It fails as
  `FabricatedNumeral` in precisely the case the dashes exist to serve.
  Paint them with plain `text`.

Two corollaries fall out of the same machinery:

- A claim naming a group outside `required_groups` fails as
  `ForeignClaim`. The mask is the vocabulary of claims you may make.
- A numeral derived from configuration is unadmittable by construction.
  Admission draws the empty config deliberately, so a visible
  config-sourced claim fails as `ConfigClaim`. Values come from state.

### `background` — an `Opaque` claim is checked exactly

`Opaque` and `Cedeable` both promise a full-frame opaque cover, and
admission proves it against the ink rather than taking the word: it
looks for one axis-aligned, full-alpha rect fill covering the whole
frame the panel was drawn at, in the `Background` band. A ground assembled from polygons,
painted at alpha below full, drawn under a rotated transform, or under a
clip that does not contain the frame **does not count**, and fails as
`BackgroundContract`. A clip that does contain the frame is fine.
Coverage is exact, not conservative.

A panel that paints nothing in the background band declares `NotUsed`
and composes as an overlay.

### `group_regions` — a surface the readout really uses

Regions say which surface of the panel a group's *value* is drawn on,
and they are asserted. Registry init still checks the geometry — inside
`frame_min`, non-degenerate, only for groups the panel requires — and
admission adds the assertion that gives the field teeth:

> **Every declared region must be populated: somewhere in the panel's
> case matrix, a visible run claiming that group is drawn at it.**

A region nothing populates fails as `GroupRegionEmpty`, naming the
panel, the group, and the rectangle that caught nothing.

Draw the boundary around the value's own ink and nothing else. Scale
ladders, tick labels, and neighbouring boxes belong to other groups or
to no group; a region generous enough to swallow them is the vacuous
case the mechanism exists to prevent. That guidance has not changed —
what changed is that it is now enforced from the other side as well. A
region must be **tight enough to be honest and populated enough to be
real**, and the second half is what admission checks.

Note what is deliberately *not* asserted: that all of a group's claimed
ink sits inside its regions. It never could be. A numeral must carry a
claim, so every ladder rung and every compass tick carries the claim of
the group it measures, and those sit outside the readout box on purpose.
A rule demanding they be inside would be unsatisfiable for every tape
and every rose, and satisfiable only by inflating regions until they
described nothing.

The hazard the assertion does address is the opposite one. A region
pointing at empty space is worse than no region, because the composition
layer plans obscuration around regions: it would protect a surface the
readout does not use and leave the surface it does use undeclared. A
region is a claim that this is where the group's value appears, and
admission makes the claim answerable.

Four scoping rules, each a consequence of what a region means rather
than a tolerance:

- **A group that declares no region is not judged.** Some readouts share
  a strip with a neighbouring group's ink and no geometry separates
  them; silence is the honest declaration there, and the provenance
  claim on the run is what keeps it honest. The PFD's selected-altitude
  box is the shipped example.
- **The witness may come from any case.** A readout that dashes out
  under withholding paints no claimed run at all in that case, and it is
  still the same readout. One case in the matrix is enough.
- **The search runs at `frame_min`,** the frame regions are declared and
  validated against. A panel laid out at a larger frame puts its
  readouts somewhere else, and floor coordinates describe that layout no
  better than they describe another panel's.
- **A region holds a run when it holds the centre of that run's ink,**
  which is neither whole-rectangle containment nor bare overlap. Run
  rectangles here are conservative nominal metrics, deliberately wider
  than the glyphs, so a readout centred in its own tightly-drawn box
  overhangs it by a few units and containment would call it empty. Bare
  overlap is too weak in the other direction: a rung grazing a region's
  corner is not that region's readout. Clipped-away runs paint nothing
  and are skipped, the same visibility rule the provenance family uses.

### Criticality bands are measured, not declared

A panel does not declare where its warnings go. The admission harness
measures it: the union design-space ink of the `Annunciation` and
`Failure` bands across the whole canonical × extreme × withheld ×
**alerted** matrix, per canonical frame, exposed on the admission report
and pinned beside the raster baselines. A composition above uses that
bound and no declaration, because a panel able to name its own warning
surface could also understate it.

The alert axis matters more than it looks. A composed frame fans one
`AlertOutput` to every slot, so every panel that draws the shared alert
stack draws it at run time; the harness therefore draws every case twice,
once quiet and once with a saturated stack, and folds both into the
bound. A band measured only on quiet frames would exclude every alert
row and tell a composition it may cover warnings.

Three things follow for an author:

- **Paint your warnings in `Annunciation` or `Failure`.** A caution
  drawn into `Tapes` is outside the measured band, and a composition
  will be told it may be covered. The floor is the two bands and nothing
  else — including the simulation labelling
  [`AIR-BAS-001`](requirements.md#air-bas-001) and
  [`AIR-FLAG-007`](requirements.md#air-flag-007) require, which is
  protected exactly when you paint it into a criticality band and not
  otherwise.
- **A band nothing was witnessed in refuses obscuration outright.** An
  empty measurement is the absence of a witness, not a proof of absence,
  so composition treats it as unknown and refuses any overlap. That is
  fail-closed, and it is not a substitute for measuring.
- **A cue no case drives is not in your band.** The shipped monitor is
  the example: its `MON` flag and full-frame failure X are gated on a
  channel status no corpus or extreme state produces, so its band covers
  its alert stack and nothing else. Contribute an extreme state that
  drives the cue if you want the protection.

## What the admission harness actually asserts

Run it before opening a pull request, not after CI tells you:

```
cargo test -p <your set crate>
```

Per case — every canonical frame × canonical states plus your extreme
states, each fully fed and then once per withheld group — the harness
checks that the draw returns, that the scene decodes and satisfies the
layer envelope, that every required layer is present, that the
background claim holds, that every glyph is in the controlled
vocabulary, and that the provenance rules above hold. Once per panel it
also checks that every declared region caught claimed ink somewhere in
the matrix. Every geometry
check uses the frame being drawn, not a constant. Ink outside that frame
is **counted, not refused** —
and the ratchet that keeps the count from growing is an assertion *you*
write in your own admission test, not something the harness does for
you. A new set has no ratchet until its author pins one.

Three limits worth knowing, because they bound what a green run means:

- **A green run does not mean your failure cues are painted.** The
  canonical corpus does drive the degraded branches — `nothing-fed` and
  `source-unusable` are two of the four states every panel meets, so
  your dash-out path really is drawn and its output really is policed
  for untagged numerals, glyphs, layers, and background. What no check
  asserts is that anything appears there at all: delete the contents of
  your dash branch and admission still passes. Drawing nothing where a
  dash belongs is admissible, and it is a defect.
- **The provenance check fires against withholding, not against
  resolved status.** A stale value painted with its group's claim in
  `source-unusable`, with nothing withheld, is not what
  `FabricatedNumeral` looks for. Cover that with your own tests.
- **Admission draws the empty config and no alerts.** No configured
  behaviour is covered by it at all, and neither is any alert-driven
  drawing — the digest passes no alerts either.

## Symbology you may not invent

`indicate-instrument-symbology` owns the never-skinnable safety
constants. A panel that paints its own failure red, caution amber, or
reference yellow is a defect even if it looks identical today, because
the point is that a future theme cannot make them skinnable.
`check-structure.sh` fails a file outside the symbology crate that
reaches for the palette aliases; use the `safety::` constants.

Text is equally constrained: the glyph pack is a controlled vocabulary,
and a character outside it fails admission as `GlyphCoverage` rather
than falling back to a substitute. It is `PANEL_VOCABULARY` in
`indicate-instrument-glyphs`, and it is smaller than it looks — space,
`-`, `.`, `°`, the digits, the uppercase letters, and of the lowercase
only `k` and `t`. A label reading `kts`, `fpm`, `%`, or `/` fails; check
the vocabulary before inventing one.

## Digests, baselines, and who re-pins them

- **The composition digest** covers the panels a registry composes, in
  order, with their contract-relevant descriptor fields — the frame
  range among them — and their bytes emitted per canonical state ×
  canonical frame. Adding a panel to a shipped set moves it, and so does
  widening a declared range. Set identity is deliberately *not* in the
  digest, so publishing a new set does not disturb a shell that does not
  compose it. Raster baselines are deliberately *not* in it either: they
  pin one backend's pixels, and re-pinning them must not move
  cross-shell identity.
- **Who re-pins:** the author of the change that moves it, in the same
  change, with the reason in the commit message. Consumers advance their
  pins on the discipline in `README.md`. A digest that moves without a
  stated reason is indistinguishable from a regression.
- **Raster baselines** are per-panel, per-canonical-frame pinned frame
  hashes over the shared typical state, asserted by the reference
  rasterizer (REN-03). A mismatch means the paint changed. It is a
  regression unless the change deliberately moved paint, in which case
  it re-pins once, for a stated reason — never refreshed to make a red
  build green.
- A new set outside `BUILTIN_PANELS` may leave `raster_baselines` empty
  until it has rasterizer coverage; nothing asserts a baseline that was
  never declared.

Contract values that a consumer pins — the state ABI, the scene format,
the corpus, the composition digest, the shipped panel set — are named
per release in `CHANGELOG.md`, and cutting that release is a step in
`CONTRIBUTING.md`.

## What composition asks of you

A panel may be placed beside, inset into, or under another on one
screen. `backend-contract.md` carries the compositor's side; this is
what a composed panel owes.

- **Lay out against the frame you are handed, every time.** A slot's
  dimensions *are* the frame the panel is asked to emit — nothing
  rescales a scene, because there is no `SCALE` opcode. A geometry
  constant that should have read `frame.width` becomes visible the day
  someone slots your panel at a size other than the one you tested.
  `validate_composition` refuses a slot whose dimensions are not a frame
  you declared, so the range in your descriptor is the whole of what a
  screen may ask.
- **Your `background` declaration decides whether you occlude.**
  `Opaque` and `Cedeable` prove a full-frame opaque cover at admission,
  so a composition treats your whole rect as covering what lies beneath;
  `NotUsed` makes you an overlay lower slots show through. Declaring
  `Opaque` because it seemed safer will bury the panel underneath you,
  and the composition will be refused as a dead slot.
- **Declare your readout regions accurately, and paint criticality in
  the criticality bands.** Those two are what a composition may and may
  not cover. Regions may be covered when the screen declares it;
  measured `Annunciation`/`Failure` ink may not be covered at all. A
  region drawn over blank space is worse than none: the screen would
  protect it and cover the readout instead.
- **Expect one snapshot and one alert state per composed frame.** Do not
  cache a previous frame's data to smooth a readout: two panels showing
  the same quantity from different snapshots is the disagreement the
  single-resolve rule exists to prevent.
- **Fail inside your own rect.** Your failure presentation is drawn in
  your own scene, in your own frame's coordinates, and a compositor
  clips it to your slot. A panel that tries to annunciate globally
  annunciates nowhere.
- **Configuration still arrives at draw time.** A composition declares
  layout only, so being slotted changes nothing about how your config
  blob reaches you.

## Where the vocabulary stops

There is no `SCALE` opcode, and that is deliberate rather than missing:
panels compose, instruments inside them do not. An author who wants an
instrument at two-thirds size beside another is reaching for the
composition layer, not for a transform — the answer is a finer-grained
panel placed at its own frame, not a scaled replay of a coarser one.
Sub-instrument reuse is therefore a decision about which panels a set
exports, never a change to the opcode vocabulary.

## Wiring a set into this repository

A set written here needs four registrations, each of which is a CI
failure if missed — which is the discovery-by-red-build this contract
exists to replace:

- `members` and `[workspace.dependencies]` in the root `Cargo.toml`;
- a row in `crates/README.md` under the Sets tier, which
  `check-structure.sh` requires for every crate;
- the `-p` list of the `thumbv7em-none-eabihf` closure step in CI;
- the `known` allowlist of the downstream-agnostic step in CI.

A set published elsewhere needs none of them, and rewrites the
`workspace = true` inheritance the template uses for its edition,
dependencies, and lints.

## The worked example

`sets/indicate-instrument-template`. One panel, one band of content, a
readout that dashes honestly, and an admission test that fails the build
if any of the above stops being true. Copy it, rename it, and grow it —
it is the shortest path from nothing to a set that passes.
