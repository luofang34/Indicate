# Changelog

Consumers pin this repository by revision. A bare revision says nothing
about what it contains, so a revision meant to be pinned is given an
annotated tag and an entry here naming the contract versions it carries.

Five values decide whether a given revision is the one a consumer wants.
Each entry states all five, and `scripts/check-release-markers.sh` fails
the build when the newest entry disagrees with the code it describes — a
changelog that has to be checked against the source is the archaeology
it was written to remove.

| Value | Where it lives |
|---|---|
| State ABI | `abi::v6::VERSION` in `indicate-instrument-state` |
| Scene format | `SCENE_FORMAT_VERSION` in `indicate-instrument-scene` |
| Corpus | `corpusVersion` in `corpus/scene-conformance-corpus.json` |
| Composition digest | `BUILTIN_SCENE_DIGEST` in `indicate-instrument-panels` |
| Panel set | `BUILTIN_PANELS` in `indicate-instrument-panels` |

A release is cut whenever any of the five moves — review's job, since a
guard comparing the newest entry to the tree cannot tell an added entry
from a rewritten one. Entries are newest first, and the tag's message
repeats the same five values so `git show <tag>` answers the question
without a checkout. `CONTRIBUTING.md` has the release steps.

## [Unreleased]

Screen composition ([`AIR-OUT-011`](docs/instruments/requirements.md)).
None of the five values above moved, so this is not a release — but a
consumer gains a sixth pinnable value and two new refusals, and both are
worth knowing before the next one is cut.

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
  a composition validates obscuration against. The monitor's is `None`,
  which is a measurement rather than an omission.
- The scene digest, the corpus, and every REN-03 raster frame hash are
  unchanged.

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
