# Changelog

Consumers pin this repository by revision. A bare revision says nothing
about what it contains, so a revision meant to be pinned is given an
annotated tag and an entry here naming the contract versions it carries.

Five values decide whether a revision's *contract* differs from the one
you already pinned. Each entry states all five, and
`scripts/check-release-markers.sh` fails the build when the newest entry
disagrees with the code it describes — a
changelog that has to be checked against the source is the archaeology
it was written to remove.

| Value | Where it lives |
|---|---|
| State ABI | `abi::v7::VERSION` in `indicate-instrument-state` |
| Scene format | `SCENE_FORMAT_VERSION` in `indicate-instrument-scene` |
| Corpus | `corpusVersion` in `corpus/scene-conformance-corpus.json` |
| Composition digest | `BUILTIN_SCENE_DIGEST` in `indicate-instrument-panels` |
| Panel set | `BUILTIN_PANELS` in `indicate-instrument-panels` |

The table above is the human summary. Its machine-readable form is
[`release-manifest.json`](release-manifest.json) at the repository root,
generated from those same definitions by `cargo xtask
gen-release-manifest` and diffed against the tree in CI by
`scripts/check-release-manifest.sh`. The two cannot contradict each
other: both are checked against the same code, and neither is written by
hand. The manifest carries the values this table has no room for — the
corpus sha256, the screen-composition digest, the per-panel raster
baselines, the glyph-pack content hash, and the criticality bands — and
states the panel set through the per-panel keys rather than as a row. A
consumer's CI can diff it; prose it cannot.

A release is cut whenever any of the five moves, and also when a public
API addition lands that consumers are expected to call — a predicate or
a file published for consumers is of no use to them if reaching it means
pinning a bare revision. Either way it is review's job to notice, since
a guard comparing the newest entry to the tree cannot tell an added
entry from a rewritten one. An entry whose five values match the one
below it is the second kind, and says so.

Entries are newest first, and the tag's message repeats the same five
values so `git show <tag>` answers the question without a checkout.
`CONTRIBUTING.md` has the release steps.

## [0.5.1] — 2026-08-18

`PanelDescriptor::choose_frame` answers which frame a shell should ask a
panel for. `accepts` can only refuse a frame a shell already holds, so a
shell that had to produce one walked the step grid itself. That is the
arithmetic the contract tells shells not to write, because two shells
write it differently and each stays green against its own tests.

| Value | This release |
|---|---|
| State ABI | 7 |
| Scene format | 1 |
| Corpus | 5 |
| Composition digest | `5cded14978b2e5ba3a17b61959ed0b35061334adf3fde4242f47e214f0f07aef` |
| Panel set | `pfd`, `hsi`, `monitor` |

Panel set changed since the previous release: no. The five values match
the entry below: this release is a public API addition, cut so that
consumers can reach the new predicate without pinning a bare revision.

### Choosing a frame ([#32](https://github.com/luofang34/Indicate/issues/32))

- **`choose_frame(space) -> Result<DesignFrame, FrameRefusal>`** gives
  the largest frame by area that fits `space` on both axes and that
  `accepts` admits. It walks the declared width grid and computes each
  width's tallest admissible height, so its cost is one axis and not
  the product of both.
- **`space` is in logical units**, the units a `DesignFrame` is in, not
  device pixels. A shell with a surface in physical pixels divides by
  its own scale factor before it asks.
- **A space below `frame_min` is refused** with the bound that refused
  it. The panel does not name a frame below its readability floor. To
  scale, letterbox, or show a different panel stays the shell's choice.
- **A shell under no constraint asks with `frame_max`** and gets it
  back, so that case needs no separate rule.

Every shipped panel declares one frame today, so `choose_frame` returns
480x360 for every space that admits it. The value is in what happens
when a panel declares a real range.

## [0.5.0] — 2026-08-18

The altitude readout becomes a rolling-digit drum. The final digit pair
steps in 20 ft faces. Each face scrolls through a window that clips it.
Everything above the pair rolls with it across the 80-to-00 boundary.
The readout therefore shows vertical rate by itself, which a value
rounded to 10 ft cannot do. Each position is a function of the altitude
value only, never of a clock. All backends thus put the digits in the
same place.

| Value | This release |
|---|---|
| State ABI | 7 |
| Scene format | 1 |
| Corpus | 5 |
| Composition digest | `5cded14978b2e5ba3a17b61959ed0b35061334adf3fde4242f47e214f0f07aef` |
| Panel set | `pfd`, `hsi`, `monitor` |

Panel set changed since the previous release: no.

### Rolling-digit altitude readout ([#56](https://github.com/luofang34/Indicate/issues/56))

- **The PFD emits more text runs for one altitude value.** The readout
  interior is a sign, the number above the final pair, and the pair
  itself. Each rolling column paints through its own clip window. A
  backend must apply `clip_rect`. If it does not, the faces above and
  below the text line become visible.
- **The number above the pair rolls as one.** Every digit that changes
  at a boundary changes together. A carry that stopped at the hundreds
  would make the last 20 ft below each thousand read a thousand low.
- **The composition digest moves**, because the PFD emits different
  bytes for the same state. The PFD raster baseline, the
  screen-composition digest, and the three composed-frame hashes move
  for the same reason.
- **The corpus moves to version 5.** Two entries pin a mid-roll value
  and a negative cascade. Each pinned consumer fails at its next pin
  advance. That failure is the synchronization mechanism.

### Notes for anyone re-pinning

- The digits shrink to fit the box. The drum fits the full advance row,
  not only the ink, because each column's clip window must stay inside
  the box body. A wide value, such as -99,990 ft, therefore renders one
  step smaller than before.
- No state ABI, scene format, or panel-set value changes. A consumer
  that does not compare frame bytes needs no change.

## [0.4.1] — 2026-08-18

The HSI annunciates which receiver drives the CDI. The source was
encoded as hue alone — magenta for GPS, green for a nav radio — so Nav1
and Nav2 read identically, and the distinction failed under color-vision
deficiency. The panel now draws `GPS` / `NAV1` / `NAV2` beside the rose
in the source color, under the same gate as the CDI: the label and the
needle appear and disappear in the same frame
([#55](https://github.com/luofang34/Indicate/issues/55)).

| Value | This release |
|---|---|
| State ABI | 7 |
| Scene format | 1 |
| Corpus | 4 |
| Composition digest | `91767280cad68734f5859ad17edee1540bce47ec32866a0d784e1f30f34e4757` |
| Panel set | `pfd`, `hsi`, `monitor` |

Panel set changed since the previous release: no.

### What moved and why

- **The composition digest moved on paint and corpus, not on wire.**
  The label adds a claimed text run to the HSI scene for every fed nav
  source, and the shared corpus gains a `nav2-source` state so each of
  the three sources is exercised. The state ABI, the scene format, and
  the conformance corpus are byte-identical.
- **The HSI raster baseline (REN-03) and the screen-composition digest
  moved with it.** The label paints in the shared `typical` state, and
  the screen digest hashes the composition digest beneath it. The
  baseline re-pin names this change as its reason.
- **The label claims the nav group.** Its numerals (`NAV1`, `NAV2`)
  carry the provenance claim the honesty rules require, and a new HSI
  group region declares the surface it paints on. A withheld or failed
  nav group removes the label with the needle.

### Notes for anyone re-pinning

- Advance the composition digest, the screen-composition digest, and the
  HSI raster baseline together. The PFD and monitor baselines, the
  criticality bands, the glyph pack, and the corpus are unchanged.
- Telling VOR from LOC needs the tuned facility type in state. That is a
  Nav layout addition and belongs in the coordinated ABI batch, not in
  this release.

## [0.4.0] — 2026-08-08

The state ABI moves to v7: velocity validity splits into a horizontal
and a vertical declaration. A source with a horizontal solution and no
vertical-speed estimate could not say so, and both ways of writing the
frame anyway were wrong — a zeroed down component painted a live VSI
needle at 0 fpm, and a non-finite one took groundspeed and track down
with it. The split is at the seam consumers differ at, not per
component: no panel reads north or east alone.

| Value | This release |
|---|---|
| State ABI | 7 |
| Scene format | 1 |
| Corpus | 4 |
| Composition digest | `f82d905643b48822de25665761ad3e29daa334d937f18b1e98a3e215353cb704` |
| Panel set | `pfd`, `hsi`, `monitor` |

Panel set changed since the previous release: no.

### State ABI v7 ([#30](https://github.com/luofang34/Indicate/issues/30))

- **`ValidFlags::velocity` becomes `velocity_horizontal` and
  `velocity_vertical`.** On the wire, bit 3 now means horizontal
  velocity and bit 8 means vertical speed; bits 0–7 keep their v6
  assignments and bits 9–15 are spare. The trust payload stays eight
  bytes — the new bit sits in the existing `u16`.
- **`StateIntegrity::velocity` becomes `velocity_horizontal` and
  `velocity_vertical`.** The horizontal check is finiteness of north and
  east, the vertical of down, so a NaN down component faults vertical
  alone.
- **`gs_kt` and `track_rad` fold the horizontal status; `vsi_fpm` folds
  the vertical one.** The `GroupId::Kinematics` group status is the
  worst of position and both axes, unchanged in spirit: a group with
  several members reports its worst member.
- **v7 replaces v6.** `abi::v6` is gone rather than kept alongside, and
  the golden frames are now `state-abi-v7.*.hex`. Fail-closed defaults
  are unchanged: an absent trust group declares nothing valid, and the
  encoder still omits the group exactly when it equals that default.

### Notes for anyone re-pinning

- A shell that gates a panel on `groups.status(GroupId::Kinematics)`
  will blank the whole group for exactly the source this release
  enables. That status folds both axes and is `Failed` for a
  horizontal-only source **by design** — fail-closed at the group level
  while `gs_kt`, `track_rad`, and `altitude` each read `Valid`. Gate on
  the signal you are drawing, not on the group.

- **Both digests moved and no paint did.** The composition digest hashes
  the ABI version byte, and the screen-composition digest hashes the
  composition digest beneath it, so `9a80dbcd…` and `34dc4332…` are the
  version byte and nothing else. The three REN-03 raster baselines, the
  criticality bands, the glyph pack, and the corpus are byte-identical:
  every canonical state declares both velocity axes valid, so resolved
  output for the corpus is what it was.
- **A feeder must set both bits to keep its VSI.** A writer that set the
  old bit 3 and stopped now declares horizontal velocity only, and its
  vertical speed resolves `Failed`. That is the fail-closed direction,
  but it is a behaviour change for any source that meant both.

## [0.3.0] — 2026-08-08

Two additions made for consumers, neither of which moves a contract
value — which is why the release trigger now covers a public API
addition as well as the five. A predicate nobody can pin by name does
not stop the second copy being written, and a manifest nobody can pin by
name does not get diffed.

| Value | This release |
|---|---|
| State ABI | 6 |
| Scene format | 1 |
| Corpus | 4 |
| Composition digest | `3efb08c55eadadc2b006ee6b006b29e4b3a3f8d4ec3ce1324f401dbc16dc85ca` |
| Panel set | `pfd`, `hsi`, `monitor` |

Panel set changed since the previous release: no.

- **`PanelDescriptor::accepts(DesignFrame) -> Result<(), FrameRefusal>`.**
  The rule over the declared frame bounds, in the crate where the
  constants live, so a shell does not write its own. The refusal names
  the bound it broke, and `FRAME_STEP_TOLERANCE` settles the one
  parameter two shells would otherwise each choose. `Registry::new` and
  composition's slot check both call it, so it cannot rot as an unused
  API.
- **`release-manifest.json` and `scripts/check-release-manifest.sh`.**
  Every pinned value this revision holds down, generated from the
  constants rather than grepped for, with CI failing when the file
  disagrees with the tree. It states what *this* revision pins;
  comparing that against its own is each consumer's check to write.
- **Admission refuses a panel that declares a frame range and emits
  identically across it.** Inert for every shipped panel, all of which
  declare a degenerate range, and the panel contract now says outright
  that treating the frame as a constant is allowed.
- **sha1 and sha2 at 0.11.** No pinned value moved; the algorithms did
  not change, only the crate API around them, and this tree's use of
  `Digest` was untouched by it.

### Notes for anyone re-pinning

- Nothing to re-verify. Every value in the table above is what `v0.2.0`
  carried, the raster baselines and criticality bands are unchanged, and
  the corpus is untouched. Advancing a pin across this release is a
  manifest bump and nothing else.
- If you wrote your own frame-bounds check while migrating to `v0.2.0`,
  `accepts` replaces it. The tolerance it applies to the step is zero,
  which is the value the descriptor's own canonical frames are validated
  against — a local check using an epsilon would accept frames this
  repository refuses.

## [0.2.0] — 2026-08-07

The design frame becomes an emission input. `DrawFn` gains a
`DesignFrame` parameter, and `PanelDescriptor` declares the range of
frames a shell may ask for — `frame_min`, `frame_max`, `frame_step`,
aspect bounds, and the `canonical_frames` the evidence is pinned at —
in place of the single `design_frame` constant. `raster_baseline`
becomes `raster_baselines`, one per canonical frame.

| Value | This release |
|---|---|
| State ABI | 6 |
| Scene format | 1 |
| Corpus | 4 |
| Composition digest | `3efb08c55eadadc2b006ee6b006b29e4b3a3f8d4ec3ce1324f401dbc16dc85ca` |
| Panel set | `pfd`, `hsi`, `monitor` |

Panel set changed since the previous release: no.

### Screen composition ([`AIR-OUT-011`](docs/instruments/requirements.md))

None of the five values above moved for this part, but a consumer gains
a sixth pinnable value and several new refusals.

- **New pinnable value: the screen-composition digest.** Its own domain
  string (`pilotage-screen-composition-digest-v1`), covering the screen
  frame, the ordered slots (panel id, rect, `occludes`), and the scene
  digest beneath. `tools/instrument-bench` composes a fixture screen and
  reproduces `071bd35c…`. Per-slot configuration is shell-supplied at
  draw time and is deliberately not in it.
- **New ceiling: `MAX_COMPOSITION_SLOTS` = 8.** Every composed-frame
  budget is a sum over slots, so this is what makes those sums finite.
  The value is a declared ceiling, not a measured one: no full-screen
  six-pack has been benched against it.
- **`group_regions` became load-bearing.** Admission asserts
  non-vacuity: every declared region must be populated by a visible run
  claiming its group, somewhere in the panel's case matrix, at
  `frame_min`. A region over blank space fails as `GroupRegionEmpty`.
  What is deliberately *not* asserted is that all of a group's claimed
  ink sits inside its regions — a numeral must carry a claim, so every
  ladder rung and compass tick carries its group's, and those sit
  outside the readout box by design. All three shipped panels satisfy
  the rule as authored; no region and no panel changed.
- **New pinned data: `BUILTIN_CRITICALITY_BANDS`.** The measured
  `Annunciation`/`Failure` ink bound per panel × canonical frame, which
  a composition validates obscuration against, measured with the alert
  stack drawn.
- **The criticality band now folds in the alert stack, and this is a
  safety fix.** Admission drew every case with no alerts, so the
  measured `Annunciation`/`Failure` bound excluded the shared alert
  stack entirely — while a composed frame fans one `AlertOutput` to
  every slot. A declared obscuration could therefore cover warning rows,
  which is exactly what AIR-OUT-011 forbids and what the contract says
  is impossible. The case matrix gains an alert axis (each case drawn
  quiet and with a saturated stack), so admission runs 242 cases instead
  of 121 and counts 166 frame-overflow warnings instead of 83. All three
  `BUILTIN_CRITICALITY_BANDS` entries moved; the monitor's is no longer
  `None`.
- **An unwitnessed band refuses obscuration.** A band measured empty is
  the absence of a witness, not a proof of absence, and it now refuses
  any overlap (`CriticalityUnwitnessed`) instead of admitting it.
- **New pinned data: three composed-frame hashes.** The reference
  rasterizer gains `render_composition`, which paints a validated
  composition into one framebuffer, and pins REN-03-style hashes for a
  side-by-side screen, an opaque inset over a PFD, and a `NotUsed`
  overlay. The inset and overlay fixtures moved above the PFD's band
  once alerts were folded in, so two of the three hashes are new. The
  overlay's show-through is asserted as a property, not only as a hash.
  `indicate-instrument-raster` gains normal
  dependencies on the registry, the state crate, and the alert model;
  the arrow points that way and no crate gained a dependency on the
  rasterizer.
- The scene digest, the corpus, and every REN-03 per-panel frame hash
  are unchanged by the composition work.

### Notes for anyone re-pinning

- The composition digest **moved**, and this is the deliberate move: the
  digest's per-panel contract block now carries the frame constraints,
  and scenes are drawn per canonical state × canonical frame. It was
  `bd85b853…` and is now `3efb08c5…`. No paint changed — the reference
  rasterizer's REN-03 frame hashes are byte-identical, which is what
  proves the move is format and not regression.
- Every shipped panel declares a degenerate range: `frame_min` equals
  `frame_max` equals 480×360, with one canonical frame. The only frame a
  conforming shell may ask them for is that one, and behaviour there is
  unchanged. What the registry refuses is a *declaration* whose bounds,
  step, aspect, or canonical frames break its rules; deriving a frame
  for a placement and clamping it into the declared range stays the
  shell's job.
- A shell calling `(panel.draw)(…)` passes the frame it chose, between
  the alerts and the scene writer. A shell that wants exactly today's
  behaviour asks for `frame_min` and scales, as before.
- Out-of-repo panel sets must add the new descriptor fields; the
  registry refuses a composition whose canonical frames do not include
  both ends of its declared range.

## [0.1.0] — 2026-08-07

First tagged revision. The contract surfaces already versioned
themselves individually; this is the first marker that says which
*combination* a commit carries.

| Value | This release |
|---|---|
| State ABI | 6 |
| Scene format | 1 |
| Corpus | 4 |
| Composition digest | `bd85b8537f0b3e4abf8cf3ad3d36c6abfdceac15355639af2804d58dd9c61931` |
| Panel set | `pfd`, `hsi`, `monitor` |

Panel set changed since the previous release: n/a, this is the first.

### Notes for anyone re-pinning

- Crates are now named `indicate-*`. A consumer advancing a pin across
  this release changes every crate name in its manifest. Revisions
  before it keep the old names, so nothing breaks mid-history.
- The composition digest is **unchanged** by the rename: it was
  `bd85b853…` before and after, because the digest domain separator is
  an identifier and was deliberately not renamed. A consumer that pins
  the digest does not need to re-verify against this release.
- The required-layer table in `scene-layer-protocol.md` was corrected to
  match the shipped descriptors: the PFD requires `Guidance`, and the
  monitor panel has a row. No descriptor changed, so no digest moved —
  the document was wrong, not the code.
