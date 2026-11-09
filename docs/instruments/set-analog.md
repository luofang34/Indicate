# Set document: analog instruments

This document describes the `indicate-instrument-analog` set. The
[panel contract](panel-contract.md) states what every set owes. This
document states what the set is for, what it draws, how it fails, and
which requirements it answers to. The descriptor facts at the end are
generated from the code and checked in CI.

## Intended function

The set draws round analog instruments for a **SIM / NOT FOR FLIGHT**
cockpit ([`AIR-BAS-001`](requirements.md#air-bas-001)). It is a
conventional-instrument surface under
[`AIR-OUT-003`](requirements.md#air-out-003): airspeed, sensitive
altimeter, vertical speed, gyro horizon, directional gyro, turn and
bank, and magnetic compass, each identifiable on its own. It also draws
a panel clock and three engine gauges: manifold pressure, tachometer,
and synchroscope.

One description, `AnalogDialSpec`, serves two backends:

- A 2D backend draws the panel scene like any other panel.
- A 3D backend builds a physical gauge from `physical_build`. It
  rasterises the static face once per lighting, extrudes each pointer
  outline into a thin needle at its stack height, and builds each drum
  window from its aperture and digit strip. It extrudes each card and
  each fixed overlay into a thin plate at its stack height, and builds
  each tape as a drum turning about a vertical axis behind its window.
  Each frame it calls `pose` and applies one rotation per needle, one
  offset per drum wheel, one turn and one slide per card, and one turn
  per tape drum. It draws and decodes no scene per frame.

A dial's parts, back to front: the face with its printed marks, drum
windows, cards, tapes, pointers by stack height, then fixed overlays by
stack height. A card is a gyro horizon's bar, which turns against the
bank and slides with pitch. A tape is a heading card read through a
window; the 2D scene slides it as a flat strip, and the drum turns by
the same arc length. An overlay is a part fixed in front of the moving
ones, such as a miniature aircraft, a lubber line, or a turn
indicator's tube.

A spec holds borrowed slices and fixed capacities, so it needs no
allocation. The registered panels use `static` specs. A simulator that
reads its instrument packs at run time builds its own specs and calls
`draw_dial`, `draw_face_scene`, `physical_build`, and `pose` directly.
`AnalogDialSpec::check` refuses a spec that cannot place values before
anything draws from it.

## Inputs

Each needle shows the indicated value that the state carries. Mechanism
lag and instrument error are the source's physics. A source that models
them writes the reading the mechanism shows, and the set filters
nothing.

- Indicated airspeed and the applied altimeter setting come from the
  air-data group under [`AIR-IN-004`](requirements.md#air-in-004).
- Indicated vertical speed comes from the same group. It is the
  rate-of-climb instrument's own reading (`vsi_mps`), never the kinematic
  vertical velocity.
- Altitude comes from the datum-qualified altitude under
  [`AIR-IN-002`](requirements.md#air-in-002). Only a barometric
  indicated altitude shows; any other declared class flags the
  altimeter.
- Manifold pressure, engine speed, oil and fuel pressure, and oil,
  coolant, cylinder-head and carburettor air temperatures come from the
  engine group (`0x0E`). Fuel quantity comes from the fuel tank group
  (`0x18`), by tank index: in volume on a dial engraved in volume, in
  mass on one engraved in mass. The display never assumes a density.
  A dial can follow more than two quantities through its extra channels,
  for example the three pointers of an engine gauge unit.
- The engine pressures arrive in kilopascals and the temperatures in
  degrees Celsius. A dial converts them to its engraved unit, such as
  pounds per square inch or degrees Fahrenheit.
- Engine and tank samples arrive beside the state as `EngineSamples`
  and `TankSamples` views, which `DrawContext::with_engines` and
  `DrawContext::with_tanks` attach. A dial reads engine or tank N,
  numbered from zero, for any N that the source reports. An engine or
  tank that lacks the dial's field shows the failure pose, not zero. No
  `AIR-IN` requirement covers engine or fuel indications yet, so this
  document cites none for them.
- Bank and pitch come from the attitude group under
  [`AIR-IN-001`](requirements.md#air-in-001). The turn needle reads the
  body yaw rate, shown only under the source's declared body-rate
  validity, as a rate gyro senses it. The ball reads the lateral
  specific force from the dynamics group.
- The directional gyro shows the heading in the reference the source
  declares. The compass shows it converted to magnetic north, which
  needs a usable variation sample; both are under
  [`AIR-IN-005`](requirements.md#air-in-005).
- The clock reads the clock group (`0x19`): the time of day the cockpit
  clock shows, which the source writes as the clock's own reading.
- The synchroscope reads how far engine 0's shaft leads engine 1's,
  from each engine's shaft angle. The pointer turns while the engines
  run at different speeds. A pose holds no memory between frames, so it
  cannot integrate a speed difference; the source integrates its speed
  into the angle.
- The set has no ammeter, suction gauge or hydraulic pressure gauge:
  the state carries no electrical, vacuum or hydraulic lane.
- Every reading folds source trust under
  [`AIR-IN-009`](requirements.md#air-in-009).

## Failure presentation

When a needle's channel shows no value, the needle goes to the failure
pose its spec declares: it parks at its stop, or it is hidden. A red
`OFF` flag then shows in the face, in the `Annunciation` band
([`AIR-OUT-004`](requirements.md#air-out-004),
[`AIR-FLAG-004`](requirements.md#air-flag-004)). A drum window without
its value shows no digits, because a digit would claim a value that the
state does not have. For the same reason a heading card without its
value shows a blank card, and a card, such as a horizon bar, takes its
declared failure pose. A turn needle without its rate rests upright, as
a run-down gyro's does, with the flag raised.

The printed graduations and legends stay. They are printed legends under
the panel contract: they claim configuration provenance, and admission
proves that each case draws them identically. The flat face and the
physical face are therefore the same in every state.

## Verification

`cargo test -p indicate-instrument-analog` does these checks:

- It admits the set and pins the scene digest over the corpus and the
  extreme states: the altimeter's barometric states and the compass's
  magnetic cruise state.
- It checks which way each gyro, heading, and ball part moves, the
  clock hands and the synchroscope phase against the corpus, and that a
  tape drum turns by the card's arc length.
- It checks that every value-to-angle law lands on its scale points.
- It checks that every withheld required group parks the needle and
  raises the flag.
- It checks that the face scene holds no moving part and claims no
  state.
- It renders the face alone and finds no pointer pixels.

With `INDICATE_ANALOG_PNG_DIR` set, the render test writes a picture of
each gauge at several readings, failed, and under night lighting.

## Descriptor facts

<!-- descriptor facts: begin (generated by `cargo xtask gen-set-docs`; do not edit) -->

Crate: `indicate-instrument-analog`

### Set `analog`

#### Panel `analog-airspeed-kt` — Airspeed (kt)

| Declared | Value |
|---|---|
| Render space | Planar2D |
| Required layers | Tapes, Annunciation |
| Required groups | Air, Trust |
| Spatial bounds | 240×240 to 240×240, step 1×1, aspect 0.95 to 1.05 |
| Canonical sizes | 240×240 |
| Underlay | Opaque |
| Accepted intents | none |
| Aux channels | none |
| Config keys | none |
| Group regions | none |
| Extreme states | none |
| Raster baselines | none |

#### Panel `analog-airspeed-mph` — Airspeed (mph)

| Declared | Value |
|---|---|
| Render space | Planar2D |
| Required layers | Tapes, Annunciation |
| Required groups | Air, Trust |
| Spatial bounds | 240×240 to 240×240, step 1×1, aspect 0.95 to 1.05 |
| Canonical sizes | 240×240 |
| Underlay | Opaque |
| Accepted intents | none |
| Aux channels | none |
| Config keys | none |
| Group regions | none |
| Extreme states | none |
| Raster baselines | none |

#### Panel `analog-altimeter` — Altimeter

| Declared | Value |
|---|---|
| Render space | Planar2D |
| Required layers | Tapes, Annunciation |
| Required groups | Air, Trust, Altitude |
| Spatial bounds | 240×240 to 240×240, step 1×1, aspect 0.95 to 1.05 |
| Canonical sizes | 240×240 |
| Underlay | Opaque |
| Accepted intents | none |
| Aux channels | none |
| Config keys | none |
| Group regions | Air at (148, 106) size 54×28 |
| Extreme states | `baro-cruise`, `baro-high` |
| Raster baselines | none |

#### Panel `analog-vertical-speed` — Vertical speed

| Declared | Value |
|---|---|
| Render space | Planar2D |
| Required layers | Tapes, Annunciation |
| Required groups | Air, Trust |
| Spatial bounds | 240×240 to 240×240, step 1×1, aspect 0.95 to 1.05 |
| Canonical sizes | 240×240 |
| Underlay | Opaque |
| Accepted intents | none |
| Aux channels | none |
| Config keys | none |
| Group regions | none |
| Extreme states | none |
| Raster baselines | none |

#### Panel `analog-manifold-pressure` — Manifold pressure

| Declared | Value |
|---|---|
| Render space | Planar2D |
| Required layers | Tapes, Annunciation |
| Required groups | Trust, Engine |
| Spatial bounds | 240×240 to 240×240, step 1×1, aspect 0.95 to 1.05 |
| Canonical sizes | 240×240 |
| Underlay | Opaque |
| Accepted intents | none |
| Aux channels | none |
| Config keys | none |
| Group regions | none |
| Extreme states | none |
| Raster baselines | none |

#### Panel `analog-tachometer` — Tachometer

| Declared | Value |
|---|---|
| Render space | Planar2D |
| Required layers | Tapes, Annunciation |
| Required groups | Trust, Engine |
| Spatial bounds | 240×240 to 240×240, step 1×1, aspect 0.95 to 1.05 |
| Canonical sizes | 240×240 |
| Underlay | Opaque |
| Accepted intents | none |
| Aux channels | none |
| Config keys | none |
| Group regions | none |
| Extreme states | none |
| Raster baselines | none |

#### Panel `analog-gyro-horizon` — Gyro horizon

| Declared | Value |
|---|---|
| Render space | Planar2D |
| Required layers | Tapes, Annunciation |
| Required groups | Attitude, Trust |
| Spatial bounds | 240×240 to 240×240, step 1×1, aspect 0.95 to 1.05 |
| Canonical sizes | 240×240 |
| Underlay | Opaque |
| Accepted intents | none |
| Aux channels | none |
| Config keys | none |
| Group regions | none |
| Extreme states | none |
| Raster baselines | none |

#### Panel `analog-directional-gyro` — Directional gyro

| Declared | Value |
|---|---|
| Render space | Planar2D |
| Required layers | Tapes, Annunciation |
| Required groups | Trust, Heading |
| Spatial bounds | 240×240 to 240×240, step 1×1, aspect 0.95 to 1.05 |
| Canonical sizes | 240×240 |
| Underlay | Opaque |
| Accepted intents | none |
| Aux channels | none |
| Config keys | none |
| Group regions | none |
| Extreme states | none |
| Raster baselines | none |

#### Panel `analog-turn-and-bank` — Turn and bank

| Declared | Value |
|---|---|
| Render space | Planar2D |
| Required layers | Tapes, Annunciation |
| Required groups | Attitude, Trust, Dynamics |
| Spatial bounds | 240×240 to 240×240, step 1×1, aspect 0.95 to 1.05 |
| Canonical sizes | 240×240 |
| Underlay | Opaque |
| Accepted intents | none |
| Aux channels | none |
| Config keys | none |
| Group regions | none |
| Extreme states | none |
| Raster baselines | none |

#### Panel `analog-magnetic-compass` — Magnetic compass

| Declared | Value |
|---|---|
| Render space | Planar2D |
| Required layers | Tapes, Annunciation |
| Required groups | Trust, Heading, Variation |
| Spatial bounds | 240×240 to 240×240, step 1×1, aspect 0.95 to 1.05 |
| Canonical sizes | 240×240 |
| Underlay | Opaque |
| Accepted intents | none |
| Aux channels | none |
| Config keys | none |
| Group regions | none |
| Extreme states | `magnetic-cruise` |
| Raster baselines | none |

#### Panel `analog-clock` — Clock

| Declared | Value |
|---|---|
| Render space | Planar2D |
| Required layers | Tapes, Annunciation |
| Required groups | Trust, Clock |
| Spatial bounds | 240×240 to 240×240, step 1×1, aspect 0.95 to 1.05 |
| Canonical sizes | 240×240 |
| Underlay | Opaque |
| Accepted intents | none |
| Aux channels | none |
| Config keys | none |
| Group regions | none |
| Extreme states | none |
| Raster baselines | none |

#### Panel `analog-synchroscope` — Synchroscope

| Declared | Value |
|---|---|
| Render space | Planar2D |
| Required layers | Tapes, Annunciation |
| Required groups | Trust, Engine |
| Spatial bounds | 240×240 to 240×240, step 1×1, aspect 0.95 to 1.05 |
| Canonical sizes | 240×240 |
| Underlay | Opaque |
| Accepted intents | none |
| Aux channels | none |
| Config keys | none |
| Group regions | none |
| Extreme states | none |
| Raster baselines | none |

<!-- descriptor facts: end -->
