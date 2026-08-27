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
| State ABI | `abi::v8::VERSION` in `indicate-instrument-state` |
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

## [0.6.2] — 2026-08-26

Speed and altitude ladders now omit labels whose nominal ink crosses a
visible tape edge. The altitude backdrop and clip now start below the
selected-altitude box.

This change deliberately moves PFD paint. The PFD raster baseline and
three composed-frame hashes move. The composition digest and screen
composition digest also move.

The conformance corpus advances to 7. Two built-in PFD replay fixtures
now contain the changed tape commands. The corpus keeps the same entries.

| Value | This release |
|---|---|
| State ABI | 8 |
| Scene format | 1 |
| Corpus | 7 |
| Composition digest | `ef0f5fd1eb5299611383950cf6853b1c61653fb70af78ab5133d312407410863` |
| Panel set | `pfd`, `hsi`, `autoflight`, `monitor` |

Panel set changed since the previous release: no.

## [0.6.1] — 2026-08-26

`GroupSet` now assigns bit positions through `GroupId::index()`, not
sparse wire tags. The representation therefore supports 32 defined
group slots regardless of their tag values.

The public `bits()` value now uses dense group positions. Consumers
that read descriptor group masks through FFI must use the same mapping.
A static assertion binds the word width to `GroupId::COUNT`. A test
also round-trips the highest allocated group.

No panel paint moved. The composition digest moved because it includes
the required-group mask. The screen-composition digest moved because it
includes the composition digest. Raster baselines remain unchanged.

| Value | This release |
|---|---|
| State ABI | 8 |
| Scene format | 1 |
| Corpus | 6 |
| Composition digest | `c8cbcd92c71bf55fe73788c1455f4ee5435082dd4afd7940e0480a96fb9611d1` |
| Panel set | `pfd`, `hsi`, `autoflight`, `monitor` |

Panel set changed since the previous release: no.

## [0.6.0] — 2026-08-21

The state ABI moves to v8. The bump is the coordination point for the
allocation batch that issue #58 directs. Six issues need wire changes:
#50, #51, #52, #53, #54 with the #55 follow-on, and #57. One coordinated
revision carries all the agreed layouts. Each allocation lands as its
own change onto this version before it is released, so the number names
exactly one wire format. The registry table in `group_id.rs` records the
agreed allocations and is the layout contract for the batch.

This release carries every group the batch allocates and all but one of
its field appends: true airspeed on the Air group, the airspeed trend on
the Dynamics group, the deflection scale on the Nav group, and the
bearing-pointer, airframe-configuration and autoflight groups.
`facility_type` on Nav follows with its consumer.

| Value | This release |
|---|---|
| State ABI | 8 |
| Scene format | 1 |
| Corpus | 6 |
| Composition digest | `8291c73e7c92e8b04963f743c9d68d9e1f590aa576f3776d0a0cfdc3183ae35b` |
| Panel set | `pfd`, `hsi`, `autoflight`, `monitor` |

Panel set changed since the previous release: yes — the autoflight
annunciator joins the set. The configuration panel ships in its own
set, `config`, which a shell composes when the airframe has the
sensors, so it does not move `BUILTIN_PANELS`.

### State ABI v8 ([#58](https://github.com/luofang34/Indicate/issues/58))

- **v8 replaces v7.** `abi::v7` is gone rather than kept alongside. The
  golden frames are now `state-abi-v8.*.hex`.
- **Group 0x13, `AirframeConfig`, is on the wire.** A stamped group of
  24 bytes: sensed flap ratio, selected flap detent, and elevator,
  aileron and rudder trim ratios, each optional and NaN-absent. A ratio
  outside the range its axis is defined over faults the group rather
  than being clamped — a clamped pointer would sit at a limit the
  airframe never reached.
- **Assigned group ids stop being contiguous.** 0x0E to 0x11 have no
  variant, so `GroupId::index` and the wire tag now genuinely disagree,
  and the test that could only describe that difference proves it.
- **The Nav group grows from 42 bytes to 43 and declares its deflection
  scale.** `scale` appends after the group's existing tail, per the
  stamped-lane growth policy. Enroute, terminal and approach are 0, 1
  and 2; every other value is Unknown, and an Unknown scale fails the
  whole nav group. A deflection in dots means nothing until the scale
  says what a dot is worth, so guidance at an undeclared scale is not
  drawn at a guessed one — and the needle carries its own gate on the
  scale as well, so a group whose status says show still draws nothing
  without one.

  The feeder's public `LATERAL_M_PER_DOT` splits with it, into
  `LATERAL_M_PER_DOT_ENROUTE`, `_TERMINAL` and `_APPROACH`. A single
  constant could only be right at one scale; a caller that held the old
  name was reading terminal metres whatever scale it flew.

  The append is what makes an undeclared scale fail closed. Taking the
  spare byte at offset 3 would have kept the length still and made a
  producer that never wrote the field decode as `Enroute` — the loosest
  scale, worth the most distance per dot.
- **The Dynamics group grows from 16 bytes to 20.** `ias_trend` follows
  the trailing `age_ms`, NaN-absent, and Trust valid bit 9 declares it.
  A producer that stamps version 8 and keeps writing 16 bytes has the
  whole frame refused, not that group.
- **The Air group grows from 12 bytes to 16.** `tas_mps` follows the
  trailing `age_ms`, NaN-absent like the altimeter setting beside it.
  Its minimum length rises with it.
- **The batch allocates four group ids and three field appends.** The
  ids are 0x12 BearingPointers (stamped,
  [#53](https://github.com/luofang34/Indicate/issues/53)), 0x13
  AirframeConfig (stamped,
  [#57](https://github.com/luofang34/Indicate/issues/57)), 0x14 ApModes
  (stamped, [#50](https://github.com/luofang34/Indicate/issues/50)), and
  0x15 ApTargets (declared, #50). 0x0E keeps its engine charter: flap
  and trim are airframe configuration, not engine. The appends are
  `tas_mps` on Air (0x03,
  [#52](https://github.com/luofang34/Indicate/issues/52)), `ias_trend`
  and Trust valid bit 9 on Dynamics (0x0B,
  [#51](https://github.com/luofang34/Indicate/issues/51)), and
  `scale_mode` on Nav (0x04, with `facility_type` to follow,
  [#54](https://github.com/luofang34/Indicate/issues/54) and
  [#55](https://github.com/luofang34/Indicate/issues/55)). Each append
  goes after the trailing `age_ms`. An older decoder accepts the longer
  payload and counts the tail.
- **0x12 BearingPointers is a stamped group of 20 bytes.** Each of the
  two pointers writes a source byte, a heading-reference byte, a
  validity byte, one pad byte, and a `f32` bearing in radians. The
  trailing `age_ms` follows the pair. A pointer names the north its own
  receiver measured against, so the reference travels with the bearing
  rather than being inherited from the heading group: a receiver that
  reports magnetic bearings and an attitude source that reports true
  heading are a normal pairing, and the display converts between them
  or draws nothing.
- **0x14 ApModes is a stamped group of 12 bytes.** Engagement, active
  and armed lateral mode, active and armed vertical mode, three
  reserved zero bytes, then the trailing `age_ms`. Each byte this build
  cannot name decodes to its fail-closed `Unknown`, and one unknown
  fails the whole group: annunciating the modes that did decode would
  say the unreadable axis is holding nothing, which nobody claimed. The
  group is stamped because an annunciation that outlives its source
  says the automation is doing something it stopped doing.
- **0x15 ApTargets is a declared group of 20 bytes.** Target airspeed,
  the altitude target with the same reference-identity trio as the
  selected altitude (class, geoid model, origin), and target vertical
  speed. The layout mirrors the selections layout field for field. An
  altitude target whose identity does not match the displayed datum is
  withheld rather than drawn: a number the pilot could read against the
  wrong datum is worse than no number.
- **Group-status indexing no longer assumes contiguous ids.**
  `GroupStatuses` was a dense table keyed by `tag - 1`. The batch
  allocates 0x12 to 0x15 while 0x0E to 0x11 stay reserved, so the
  arithmetic would break. The mapping is now an explicit match. This is
  an internal change; it moves no wire byte.

### Notes for anyone re-pinning

- **Both digests moved, and so did the paint.** The composition digest
  hashes the ABI version byte, and the screen-composition digest hashes
  the composition digest beneath it, so both move on the version alone.
  The PFD also gains a true-airspeed box at the head of its speed tape,
  which moves its raster baseline and the three composed-frame hashes.
- **A state writer must stamp version byte 8 AND write the longer Air
  group.** A writer that changes only the version byte emits a 12-byte
  Air payload, which is now below the group's minimum. The decoder
  rejects the whole frame, not that group, so every panel blanks — the
  failure looks like total signal loss rather than one short group. Emit
  the 16-byte payload, with a NaN in the true-airspeed slot when the
  source has none.
- **The HSI gains two bearing needles, so its raster baseline and the
  composed-frame hashes move.** The shared `typical` state now feeds
  both pointers, and the HSI contributes a `bearing-split-references`
  extreme state, which raises the admission case count.
- **A state writer must also write the longer Nav group.** A writer
  that omits the scale emits a 42-byte payload, which is now below the
  group's minimum, and the decoder rejects the whole frame exactly as
  it does for a short Air group. Declare the scale the guidance is
  actually flown to; there is no value that means "not stated".
- **The panel set gains the autoflight annunciator, which moves the
  composition digest and the screen-composition digest.** A surface of
  its own rather than a band inside the PFD: the PFD already
  annunciates the flight director's mode, and a second mode vocabulary
  competing for the same strip would leave a reader deciding which
  annunciation answers which question. The shared `typical` state does
  not feed the new groups, matching the posture — no feeder publishes
  autoflight data — so the panel's pinned raster baseline is its
  unfed presentation, and the panel's own extreme states carry the
  engaged case.
- **`Theme` gains `mode_active`, and a theme validation rule with it.**
  The active-mode color must stay the same distance from both guidance
  colors that those two must keep from each other. A mode annunciation
  wearing the radio-nav green would say a receiver is guiding when what
  it names is an automation state.
- **The speed tape starts 25 units lower.** The true-airspeed box is
  opaque and owns the strip above the tape, so the tape no longer paints
  under it. The visible speed range above the pointer shrinks by about
  3.5 kt.
- **The scene conformance corpus moves to version 6.** The two entries
  that replay the built-in PFD re-pin, because the panel above them
  emits different bytes for the same altitude. No entry is added or
  removed, and every hand-built scene keeps its bytes. Each pinned
  consumer fails at its next pin advance; that failure is the
  synchronization mechanism.
- **The HSI paints two labels beside the rose, not one.** The receiver
  label names which receiver drives the needle and the scale label names
  what a dot is worth. They stack in one column under the needle's own
  gate, and a test holds them clear of the rose rim and of each other.

## [0.5.3] — 2026-08-21

Every shell that draws a display failure now reads its reason from one
registry. `DisplayFault` gains `ALL`, `code()` and `from_code()`, so a
reason has one wire code and one identity wherever it is shown, and a
shell no longer carries a private mapping that can drift from another
shell's ([#46](https://github.com/luofang34/Indicate/issues/46)).

A reason added outside `ALL` would take a code no Rust test can see —
the type still compiles, the suite still passes, and the new variant
silently shares an existing reason's code, identity and class. No
compiler check can catch it, so `check-structure.sh` counts the
variants against the list and refuses a disagreement.

| Value | This release |
|---|---|
| State ABI | 7 |
| Scene format | 1 |
| Corpus | 5 |
| Composition digest | `5cded14978b2e5ba3a17b61959ed0b35061334adf3fde4242f47e214f0f07aef` |
| Panel set | `pfd`, `hsi`, `monitor` |

Panel set changed since the previous release: no.

## [0.5.2] — 2026-08-21

`SnapshotMeta::generation` counts snapshots admitted, not source groups
advanced. The meaning is stated once, for every shell, in
`backend-contract.md`
([#47](https://github.com/luofang34/Indicate/issues/47)).

**This changes what a consumer holding the old meaning reads.** A
generation that stops advancing used to mean the sources stopped
changing; it now means the gate stopped admitting. A consumer that
raised a liveness fault on a stalled generation was reporting quiet
data and now reports a stalled producer, which is the question liveness
asks. A consumer that compared generations to detect a content change
was always wrong under both meanings and is now wrong more often: two
admissions of identical content advance the counter twice. Compare the
content.

The gate assigns the counter, never a source. A source sits on the
untrusted side of the boundary the snapshot describes, so a counter a
source supplied would be the thing being judged rather than the
judgement.

No wire byte moves: the field, its width and its offset are unchanged.
The ABI number therefore does not move either, which is why this note
exists — no gate can see a meaning change, and `check-release-markers`
compares numbers.

| Value | This release |
|---|---|
| State ABI | 7 |
| Scene format | 1 |
| Corpus | 5 |
| Composition digest | `5cded14978b2e5ba3a17b61959ed0b35061334adf3fde4242f47e214f0f07aef` |
| Panel set | `pfd`, `hsi`, `monitor` |

Panel set changed since the previous release: no.

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
