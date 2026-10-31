# Set document: head-up instruments

This document describes the `indicate-instrument-hud` sets. The
[panel contract](panel-contract.md) states what every set owes; this
document states what these sets are for, what they draw, how they fail,
and which requirements they answer to. The descriptor facts at the end
are generated from the code and checked in CI.

## Intended function

The crate draws one head-up display in two profiles. Both profiles are
**SIM / NOT FOR FLIGHT** ([`AIR-BAS-001`](requirements.md#air-bas-001)),
and both share one drawing path fixed to the aircraft nose.

- **Screen profile** (`hud-screen` set). A simulator or game display on a
  phone, tablet, or computer. The host renders the outside world through
  the camera that it declares to the set, so world and symbology share one
  projection. The profile adds simulated contacts and weapon cues
  ([`AIR-OUT-012`](requirements.md#air-out-012)). It makes no `HUD-SIM`
  claim and conveys no HUD, sensor, or weapon credit.
- **Combiner profile** (`hud-combiner` set). A design for a reflective
  combiner. A combiner shows one colour and cannot show dark ink, so this
  profile draws every symbol in one colour and draws no contrast halo.
  Its cues are conformal and labelled `HUD-SIM` only while the design-eye,
  boresight, and time-alignment calibration is valid
  ([`AIR-OUT-006`](requirements.md#air-out-006),
  [`AIR-MODE-005`](requirements.md#air-mode-005)). Without it, the profile
  is a repeater labelled `NON-CONFORMAL / NOT A HUD`
  ([`AIR-OUT-007`](requirements.md#air-out-007),
  [`AIR-MODE-006`](requirements.md#air-mode-006)). The combiner profile
  draws no contacts and no weapon cues. An installed airborne HUD stays
  outside the boundary ([`AIR-OUT-008`](requirements.md#air-out-008)).

## Inputs

The sets consume the resolved aircraft snapshot and the central alert
snapshot, one of each per frame. Attitude, air data, altitude, heading,
and velocity arrive under [`AIR-IN-001`](requirements.md#air-in-001)
through [`AIR-IN-005`](requirements.md#air-in-005); time and validity
evidence under [`AIR-IN-008`](requirements.md#air-in-008) and
[`AIR-IN-009`](requirements.md#air-in-009); alerts under
[`AIR-IN-012`](requirements.md#air-in-012).

The host also supplies a `HudContext` for each frame:

- The profile.
- The projection: the vertical field of view of a camera on the body x
  axis, the world source (synthetic or video), and a status. For a
  combiner or for video, the status is the calibration
  ([`AIR-IN-010`](requirements.md#air-in-010)). For a synthetic world on
  a screen, it states that world and symbology come from one camera and
  one sample.
- The weapon mode and the simulated weapon limits.
- Up to 16 contacts under [`AIR-IN-017`](requirements.md#air-in-017), each
  relative to the aircraft in north, east, and down axes, air or surface,
  with one optional designated contact.
- The terrain elevation around the weapon impact point, in the altitude's
  datum, for rockets and bombs.
- The simulated directed-energy weapon state: heat, firing, and cut-out.
- The body rates, for the gun funnel.
- An optional landing runway from the host's scenery under
  [`AIR-IN-011`](requirements.md#air-in-011): threshold position, landing
  course, length, width, glide path angle, and a status. A moving carrier
  deck is the same input, updated each frame.
- An optional on-speed angle-of-attack band for the approach.

Receiver deviations for an approach come from the navigation group under
[`AIR-IN-005`](requirements.md#air-in-005), not from the runway geometry.

## Approach and synthetic vision

Both profiles draw approach symbology while a valid runway is supplied:

- `APP` beside the weapon label.
- The conformal runway outline and dashes along the extended centreline.
- A dashed glide path reference line, labelled `GP`, at the glide path
  angle below the horizon along the runway course. With the flight path
  marker on the line, the aircraft flies the glide path angle.
- An on-speed angle-of-attack bracket beside the flight path marker, when
  the host declares an on-speed band. The bracket is fixed to the body at
  the band's depression below the boresight, so with the wings level the
  marker sits inside it exactly when the angle of attack does. The top edge
  is the fast limit. The set reads no angle-of-attack signal for it.
- Lateral and vertical deviation scales from an approach-scale receiver,
  labelled `LOC` and `GS`, or `LNAV` and `VNAV` for a GPS source. Without
  such a receiver, `LOC GS UNAVAILABLE` or the group status shows, and the
  runway geometry stays.

On a screen, the outside view is synthetic vision
([`AIR-OUT-005`](requirements.md#air-out-005)), so the screen profile
labels a conformal frame `SVS`.

## What the screen profile draws

- A conformal pitch ladder and horizon, a flight path marker, a bank
  scale, and a waterline at the boresight. Climb rungs are solid. Dive
  rungs are dashed, and their outer ends droop by half the dive angle, so
  each pair forms a caret that points to the horizon. Symbols mark the
  zenith and the nadir.
- A dashed ghost horizon at the field edge nearest the horizon while the
  horizon is outside the field. It gives a direction, not a registered
  line.
- Moving speed and altitude tapes. Position and motion on the tape show
  the value and its trend; a box at the index gives the exact reading. A
  `Missing` or `Failed` value stops its tape.
- An analog vertical speed scale with a moving caret, linear to 1000 feet
  per minute and compressed to 6000. Its ticks carry no numerals.
- Groundspeed, and a heading scale.
- One symbol for each contact in the frame. The shape gives the declared
  identity: diamond for hostile, circle for friendly, square for unknown.
  A stale contact is amber and labelled `STALE`.
- A designation box on the designated contact. When that contact is
  outside the frame, a locator line points to it; `AFT` marks a contact
  behind the aircraft. Other contacts outside the frame get a chevron on
  the frame edge.
- A range scale for the designated contact, with gun or missile envelope
  marks and a closure arrow. No contact quantity is drawn as a numeral,
  because no state group gives contacts provenance.
- In gun mode, a gun cross and a funnel. The funnel shows where the
  rounds fired over the last 1.3 seconds are now, from the current body
  rates, velocity, muzzle speed, and gravity. Its edges are one target
  wingspan apart at each round's range, so a target whose wingtips touch
  both edges is at the range the rounds pass. Without valid body rates
  the funnel is removed and `FUNNEL OFF` shows. A designated contact
  inside twice the gun limit also gets a lead pipper; its ring shows range
  against the gun limit, and `IN RNG` shows a solution inside it.
- In short-range missile mode, an infrared seeker circle. The circle
  moves onto a designated contact inside the seeker lock limit.
- In medium-range missile mode, a steering circle at the nose and a dot
  toward a tracked contact. `STEER` shows until the dot is inside the
  circle.
- For both missiles, `NO LOCK`, `LOCK`, `MIN RNG`, or `SHOOT` from lock
  and range, and minimum, no-escape, and maximum marks on the range
  scale.
- In rocket mode, a continuously computed impact point: the rocket leaves
  along the gun line at its launch speed, falls under gravity, and meets
  the supplied terrain elevation. `IN RNG` or `RNG` compares the slant
  range with the rocket limit. An impact point outside the field stays at
  the field edge with `CCIP LIMIT`. Without a terrain elevation,
  `CCIP INVALID` shows and no point is drawn.
- In guided-bomb mode, a steering line through the bearing of the
  designated surface target, and a release cue on it. The cue moves down
  the line to the flight path marker as the target comes within the
  bomb's reach, the ballistic range of a free fall times a glide ratio.
  `STEER`, `REL`, or `MIN RNG` shows the release state, and an `L` marks
  the lased target. Without a surface designation, `NO DESIG` shows.
- In directed-energy mode, an aim reticle at the boresight and a heat bar
  that turns amber near the cut-out and red after it. `LASE`, `HOT`, or
  `ON TGT` shows the weapon state.
- A surface contact stands on a stem with a ground tick, so it never
  reads as an aircraft.

## Modes and failure presentation

- **Projection loss.** An invalid projection removes the ladder, the
  flight path marker, contacts, and weapon cues. A boxed attitude
  repeater with numeric pitch and bank replaces them
  ([`AIR-UNAV-006`](requirements.md#air-unav-006)). A screen over a
  synthetic world shows `CAMERA INVALID`; a combiner or camera video
  shows `NON-CONFORMAL / NOT A HUD`. `TGT CUES OFF` shows when contacts
  or a weapon mode are present but not drawn.
- **Unusual attitude.** The airframe profile sets the thresholds and the
  hysteresis ([`AIR-MODE-003`](requirements.md#air-mode-003)).
  `fighter_profile` engages near the vertical nose up, in a steep dive, or
  past inverted, so routine air-combat attitudes keep their cues. In the
  mode, a compressed ladder fits 90 degrees into the field, so the horizon
  and both hemispheres stay in view. It draws rungs every 30 degrees only,
  because compressed 10-degree rungs are too dense to read. Red chevrons point to the horizon at
  extreme pitch. `UNUSUAL ATTITUDE`, its sense, and `ATT COMPRESSED` show,
  because the compressed ladder is not registered. The flight path marker
  and the weapon and contact cues are removed.
- **Degraded data.** A tape whose group is `Missing` paints dashes. A
  `Stale` or `Degraded` value keeps its number and a `STALE` or `CHECK`
  label, and a `Failed` group is labelled `FAILED`
  ([`AIR-OUT-004`](requirements.md#air-out-004)). The labels keep the
  meaning on a one-colour combiner.
- **World placement.** The flight path marker and contacts need a true
  heading reference. Without it, `FPM UNAVAILABLE` or `TGT NO TRUE HDG`
  shows ([`AIR-HAZ-006`](requirements.md#air-haz-006)).
- **Labels.** The surface draws its own simulation and conformality
  labels ([`AIR-FLAG-007`](requirements.md#air-flag-007)).

## Layout and reference frames

The design frame is the host frame in its own logical units. The set
accepts frames from 390 by 390 to 2560 by 1600, with an aspect from 0.45
to 2.4, so one descriptor serves a phone in either orientation, a tablet,
and a computer. The boresight is the frame centre. A text and stroke
scale follows the shorter frame side. The ladder, flight path marker,
contacts, and weapon cues are in the host camera frame. Readouts, scales,
and labels are fixed to the display.

## Verification

- Admission: `cargo test -p indicate-instrument-hud` composes both sets
  and admits every panel at each canonical frame with no frame overflow.
- Projection: `projection::tests` checks boresight, pitch, bank, heading,
  behind-the-viewer, and magnetic-heading cases.
- Surface weapons: `panel::ground_tests` checks the rocket impact point,
  terrain loss, the release states, the bomb cues, the energy weapon
  states, and the surface stake.
- Approach: `panel::approach_tests` checks the runway outline, the
  reference line against a threshold on the glide path, deviation
  movement, receiver loss, invalid geometry, the combiner, and the
  on-speed bracket.
- Profiles and cues: `panel::tests` checks the label of each profile and
  source, the one-colour combiner, tape and vertical speed polarity, the
  lead solution, the gun and missile cues, the aft locator, and recovery
  declutter.

## Known limits

- Each descriptor draws one reference context with no contacts, so
  admission does not see contact or weapon cues. The host entry point is
  `draw_hud`.
- Contacts have no state group, so range, closure, and aspect are analog
  only.
- Load factor, angle of attack, and Mach are not in the aircraft snapshot,
  so the set does not draw them.
- The dive-rung droop follows the F-14 HUD description ("angled 15° down
  from the horizon line" in a 30° dive) and the geometry of constant-pitch
  circles. No primary F-16 or F/A-18 source has confirmed it.
- The funnel assumes a steady manoeuvre and no projectile drag. The rocket
  and bomb solutions also ignore drag and wind, and they use one terrain
  elevation for the impact area.
- The runway status stands for the database evidence of
  [`AIR-IN-011`](requirements.md#air-in-011); the set does not see the
  datum, revision, or coverage behind it.
- No raster baselines and no criticality band are pinned for these sets.

## Descriptor facts

<!-- descriptor facts: begin (generated by `cargo xtask gen-set-docs`; do not edit) -->

Crate: `indicate-instrument-hud`

### Set `hud-screen`

#### Panel `hud-screen` — Screen head-up display

| Declared | Value |
|---|---|
| Render space | Planar2D |
| Required layers | Tapes, Annunciation |
| Required groups | Attitude, Kinematics, Air, Altitude, Heading, SpeedPresentation |
| Spatial bounds | 390×390 to 2560×1600, step 1×1, aspect 0.45 to 2.4 |
| Canonical sizes | 390×390, 844×390, 390×844, 1024×768, 1280×720, 2560×1600 |
| Underlay | None |
| Accepted intents | none |
| Aux channels | `HUD_CONTEXT` |
| Config keys | none |
| Group regions | none |
| Extreme states | none |
| Raster baselines | none |

### Set `hud-combiner`

#### Panel `hud-combiner` — Combiner head-up display

| Declared | Value |
|---|---|
| Render space | Planar2D |
| Required layers | Tapes, Annunciation |
| Required groups | Attitude, Kinematics, Air, Altitude, Heading, SpeedPresentation |
| Spatial bounds | 390×390 to 2560×1600, step 1×1, aspect 0.45 to 2.4 |
| Canonical sizes | 390×390, 844×390, 390×844, 1024×768, 1280×720, 2560×1600 |
| Underlay | None |
| Accepted intents | none |
| Aux channels | `HUD_CONTEXT` |
| Config keys | none |
| Group regions | none |
| Extreme states | none |
| Raster baselines | none |

#### Panel `hud-repeater` — Non-conformal flight repeater

| Declared | Value |
|---|---|
| Render space | Planar2D |
| Required layers | Tapes, Annunciation |
| Required groups | Attitude, Kinematics, Air, Altitude, Heading, SpeedPresentation |
| Spatial bounds | 390×390 to 2560×1600, step 1×1, aspect 0.45 to 2.4 |
| Canonical sizes | 390×390, 844×390, 390×844, 1024×768, 1280×720, 2560×1600 |
| Underlay | None |
| Accepted intents | none |
| Aux channels | `HUD_CONTEXT` |
| Config keys | none |
| Group regions | none |
| Extreme states | none |
| Raster baselines | none |

<!-- descriptor facts: end -->
