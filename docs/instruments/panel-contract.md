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

Under `sets/`, one crate per set, exporting one `PanelSet`. A set's
normal dependencies are kernel-only — `crates/README.md` states the tier
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
| `design_frame` | **Load-bearing.** Every geometry check is expressed in its units. |
| `background` | **Load-bearing.** An `Opaque` or `Cedeable` claim is proven against the emitted ink. |
| `draw` | **Load-bearing.** Refusing to draw any case fails admission. |
| `extreme_states` | **Load-bearing.** Each one multiplies the case matrix. |
| `id`, `title` | Validated at registry init: charset, non-empty. |
| `config_schema` | Validated at init (keys strictly ascending). Admission always draws the empty config. |
| `group_regions` | **Declared, not currently read by admission.** See below. |
| `raster_baseline` | Inert unless the panel is in a set the raster tests cover. |

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
design frame in the `Background` band. A ground assembled from polygons,
painted at alpha below full, drawn under a rotated transform, or under a
clip that does not contain the frame **does not count**, and fails as
`BackgroundContract`. A clip that does contain the frame is fine.
Coverage is exact, not conservative.

A panel that paints nothing in the background band declares `NotUsed`
and composes as an overlay.

### `group_regions` — declared, and not yet load-bearing

Regions say which surface of the panel a group's readout owns. They are
validated for geometry at registry init — inside the frame, non-
degenerate, only for groups the panel requires — and **admission does
not currently read them**. The conformance crate says so in its own
module documentation, and this contract will not paper over it.

What actually keeps a panel honest today is the provenance machinery
above, which tests every claimed run wherever the ink lands. Declare
regions accurately anyway: they are a shell's statement of readout
ownership, and giving them teeth is the first phase of the screen
composition work. A region drawn wrong will become a failure the day it
is checked, and the person who has to fix it is the one who drew it.

Draw the boundary around the value's own ink and nothing else. Scale
ladders, tick labels, and neighbouring boxes belong to other groups or
to no group; a region generous enough to swallow them is the vacuous
case the mechanism exists to prevent.

## What the admission harness actually asserts

Run it before opening a pull request, not after CI tells you:

```
cargo test -p <your set crate>
```

Per case — canonical states plus your extreme states, each fully fed and
then once per withheld group — the harness checks that the draw returns,
that the scene decodes and satisfies the layer envelope, that every
required layer is present, that the background claim holds, that every
glyph is in the controlled vocabulary, and that the provenance rules
above hold. Ink outside the design frame is **counted, not refused** —
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
  order, with their contract-relevant descriptor fields and their
  emitted bytes. Adding a panel to a shipped set moves it. Set identity
  is deliberately *not* in the digest, so publishing a new set does not
  disturb a shell that does not compose it.
- **Who re-pins:** the author of the change that moves it, in the same
  change, with the reason in the commit message. Consumers advance their
  pins on the discipline in `README.md`. A digest that moves without a
  stated reason is indistinguishable from a regression.
- **Raster baselines** are per-panel pinned frame hashes over the shared
  typical state, asserted by the reference rasterizer (REN-03). A
  mismatch means the paint changed. It is a regression unless the change
  deliberately moved paint, in which case it re-pins once, for a stated
  reason — never refreshed to make a red build green.
- A new set outside `BUILTIN_PANELS` may leave `raster_baseline` at
  `None` until it has rasterizer coverage; nothing asserts a baseline
  that was never declared.

Contract values that a consumer pins — the state ABI, the scene format,
the corpus, the composition digest, the shipped panel set — are named
per release in `CHANGELOG.md`, and cutting that release is a step in
`CONTRIBUTING.md`.

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
