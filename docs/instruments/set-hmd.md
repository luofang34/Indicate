# Set document: head-worn instruments

This document describes the `indicate-instrument-hmd` set. The
[panel contract](panel-contract.md) states what every set owes; this
document states what this set is for, what it draws, how it fails, and
which requirements it answers to. The descriptor facts at the end are
generated from the code and checked in CI.

## Intended function

The set is a **SIM / NOT FOR FLIGHT** head-worn simulator display
([`AIR-BAS-001`](requirements.md#air-bas-001)). It presents flight
measurements, attitude, guidance targets, automation modes, and alerts
near the observer's eyes. It supplies supplemental awareness in the
simulator and no airborne HUD or HWD credit
([`AIR-OUT-008`](requirements.md#air-out-008)).

The set has two presentation layers. Instrument text and scales are
nonconformal and sit in one display-fixed frame. World-referenced cues
(true-bearing compass, velocity vector, aircraft attitude reference)
leave the set as earth-referenced directions that the host projects.
The set declares no camera, design eye, or boresight calibration
([`AIR-IN-010`](requirements.md#air-in-010)), so a host that shows the
world cues as conformal must supply and validate that evidence itself;
without it the surface is a non-conformal repeater
([`AIR-OUT-007`](requirements.md#air-out-007),
[`AIR-UNAV-006`](requirements.md#air-unav-006)). The
[presentation policy](hwd-presentation-policy.md) records the display
roles, tasks, content reservations, and declutter precedence.

## Inputs

The set consumes the resolved aircraft snapshot and the central alert
snapshot, one of each per frame. The groups it requires are listed in
the descriptor facts below. Attitude, air data, altitude, heading, and
velocity arrive under [`AIR-IN-001`](requirements.md#air-in-001) through
[`AIR-IN-005`](requirements.md#air-in-005); time and validity evidence
under [`AIR-IN-008`](requirements.md#air-in-008) and
[`AIR-IN-009`](requirements.md#air-in-009); guidance, modes, and targets
under [`AIR-IN-015`](requirements.md#air-in-015); alerts under
[`AIR-IN-012`](requirements.md#air-in-012). The host also supplies a
display context: the display role, the task, the detail level, and the
visibility evidence for another flight display. Missing, stale,
degraded, or failed visibility evidence cannot authorize blanking.

## Modes and failure presentation

- **Unusual attitude.** The set applies the shared unusual-attitude
  policy ([`AIR-MODE-003`](requirements.md#air-mode-003)): a prominent
  `UNUSUAL ATTITUDE` indication, a nose-high, nose-low, high-bank, or
  inverted label, steering cues removed, and receiver detail suppressed.
  Recovery overrides duplicate-display blanking and the mission task.
- **Degraded and reversionary.** A readout whose group is `Missing`
  paints dashes with no provenance claim. `Stale` and `Degraded` values
  keep their number and wear a `STALE` or `CHECK` label. A `Failed`
  group paints its label in failure red
  ([`AIR-OUT-004`](requirements.md#air-out-004),
  [`AIR-FLAG-001`](requirements.md#air-flag-001) through
  [`AIR-FLAG-005`](requirements.md#air-flag-005)). Steering cues need
  valid guidance and valid measured attitude; a stale, degraded,
  missing, or failed command produces no cue
  ([`AIR-MODE-004`](requirements.md#air-mode-004)).
- **Alerts.** The ordered alert stack, its overflow, and the manager's
  health are drawn in the alert reservation; a caution or warning
  restores flight geometry and suspends the mission reservation.
- **Display failure.** The set draws its own failure content inside its
  frame; a renderer or output failure is the host's to present under
  [`AIR-UNAV-007`](requirements.md#air-unav-007).

## Layout and reference frames

Coordinates use one 1200 by 600 logical frame. The
[presentation policy](hwd-presentation-policy.md) names the
reservations: automation, orientation, alerts, central external view,
flight measurements, aircraft guidance, mission status, system status,
and host context. A head turn changes the projection of the world cues
and never the measured attitude or the flight-director errors.

## Verification

- Admission: `cargo test -p indicate-instrument-hmd` composes the set,
  runs the admission harness, and pins the out-of-frame ink count.
- Presentation: `presentation::tests` and
  `presentation::recovery_tests` cover visibility qualification,
  recovery precedence, urgent alerts, mission reservations, and
  transition expiry.
- Cross-set meaning: `hwd-bench::cross_set_tests` compares mode, target,
  command, envelope, and alert semantics with the fixed panels.
- Review: `cargo run --release -p hwd-bench -- target/hwd-gallery.html`
  renders the review cases in a browser with head sweeps.

## Known limits

- Each descriptor reads `HWD_DISPLAY` from the auxiliary channel or uses
  the default display context when the channel is absent.
- World-referenced cues emit into the angular-sphere scene target, while
  display-fixed text and scales emit into the overlay frame.
- The set declares no group regions and no raster baselines, and no
  criticality band is pinned for it, so a screen composition that names
  it is refused.

## Descriptor facts

<!-- descriptor facts: begin (generated by `cargo xtask gen-set-docs`; do not edit) -->

Crate: `indicate-instrument-hmd`

### Set `hmd-replay`

#### Panel `hmd-replay` — Head-worn flight instruments

| Declared | Value |
|---|---|
| Render space | AngularSphere |
| Required layers | WorldCue, Annunciation, Overlay(Tapes), Overlay(Annunciation) |
| Required groups | Attitude, Kinematics, Air, Nav, Trust, Altitude, Heading, Dynamics, FlightDirector, ApModes, ApTargets, Guidance, SpeedPresentation |
| Spatial bounds | unit sphere + overlay 1200×600 |
| Canonical sizes | 1200×600 |
| Underlay | None |
| Accepted intents | none |
| Aux channels | `HWD_DISPLAY` |
| Config keys | none |
| Group regions | none |
| Extreme states | `extended-flight-guidance` |
| Raster baselines | none |

#### Panel `hmd-glance` — Head-worn off-axis instruments

| Declared | Value |
|---|---|
| Render space | AngularSphere |
| Required layers | WorldCue, Annunciation, Overlay(Tapes), Overlay(Annunciation) |
| Required groups | Attitude, Kinematics, Air, Nav, Trust, Altitude, Heading, Dynamics, FlightDirector, ApModes, ApTargets, Guidance, SpeedPresentation |
| Spatial bounds | unit sphere + overlay 1200×600 |
| Canonical sizes | 1200×600 |
| Underlay | None |
| Accepted intents | none |
| Aux channels | `HWD_DISPLAY` |
| Config keys | none |
| Group regions | none |
| Extreme states | `extended-flight-guidance` |
| Raster baselines | none |

<!-- descriptor facts: end -->
