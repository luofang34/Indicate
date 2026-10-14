# Shared flight guidance and speed display

This contract defines simulation display inputs shared by fixed instruments and head-worn displays (HWDs).
It grants no flight approval.

## System boundary

A flight guidance system (FGS) supplies commands, engagement states, modes, and effective targets.
An upstream aircraft model supplies speed conversions and projected envelope intervals.
Indicate validates these inputs and renders their meaning.
Indicate does not calculate control commands, aerodynamic limits, or recovery trajectories.

The producer owns sensor calibration, aircraft configuration, icing models, and atmospheric models.
The producer also owns source selection, clock synchronization, and protection activation.
A rendering backend interprets Scene IR commands without making flight decisions.

## Review findings

The review identified command gating through `shows_value()` in the fixed primary flight display (PFD).
That predicate permits stale and degraded values.
Shared command gating requires `Valid` guidance and measured attitude.
The fixed PFD, autoflight panel, and HWD use shared mode and target semantics.
Both HWD layouts consume the central alert snapshot.

## Required semantics

| Requirement | Behavior |
|---|---|
| FG-01 | AP, FD, and automatic thrust engagement remain independent. |
| FG-02 | Active and armed modes occupy distinct rows. |
| FG-03 | Commands require valid guidance and valid measured attitude. |
| FG-04 | Stale, degraded, missing, and failed commands never produce steering cues. |
| FG-05 | Unusual attitude removes ordinary steering cues and preserves relevant alerts. |
| FG-06 | Mode changes and reversions use producer-supplied transition information. |
| FG-07 | Selected speed and effective speed remain distinct. |
| FG-08 | IAS, CAS, EAS, Mach, and dynamic pressure retain distinct coordinate identities. |
| FG-09 | A target projection requires the matching guidance identity and native target. |
| FG-10 | Invalid extended inputs cannot reactivate legacy commands. |
| FG-11 | Missing envelope information never implies unrestricted flight. |
| FG-12 | Disconnected allowed intervals retain their gaps. |
| FG-13 | Limit, protection, awareness, performance, and preferred intervals retain distinct meanings. |
| FG-14 | Forward and compact HWD layouts share command, mode, target, and alert semantics. |
| FG-15 | Renderers never derive guidance from navigation deviation or ground speed. |

## Input ownership and compatibility

The tagged state frame carries optional guidance and speed-presentation groups.
Their tags extend ABI v8 without changing existing payload layouts.
Existing decoders skip these tags under the documented forward-compatibility rule.
Existing fixtures remain byte-identical.

A present guidance group is authoritative, including when invalid.
Legacy AP and FD groups apply only when the guidance group is absent.
Legacy data cannot declare automatic thrust engagement or a Mach control target.

One guidance identity contains a source identifier, source epoch, and sequence number.
The speed group references that identity when supplying an effective target projection.
Equal numeric targets do not establish identity.
Producers must reject replayed messages before updating group ages.
They must advance monotonic sequence numbers with wrapping arithmetic.

Each group carries its acquisition age.
Transport reception and rendering must not reset that age.
All displays consume one resolved aircraft snapshot and one alert snapshot per frame.

## Command and mode presentation

An attitude command contains commanded pitch and bank in aircraft coordinates.
It does not represent a flight-path vector or a navigation deviation.
The shared resolver computes signed attitude errors only from valid inputs.
Bank error follows the shortest angular difference.

The FGS supplies active and armed lateral, vertical, and thrust modes.
The FGS also supplies its speed-control channel and protection state.
A selected target does not prove that speed control is active.
Unknown modes produce an explicit unavailable indication.
Degraded mode reports do not retain normal active-mode colors.

The producer supplies a transition reason and elapsed transition time.
Indicate adds the sample age before evaluating a configured attention period.
Re-rendering a sample cannot restart that period.
Mode highlighting does not replace a required alert.

AP disconnect, automatic thrust disconnect, and speed protection use the central alert manager.
The host handles acknowledgement and plays the manager's aural output once per cockpit frame.
Individual panels never acknowledge or generate audio.

## Speed and envelope presentation

The display coordinate is independent of the speed-control reference.
The reference cases retain IAS tapes.
Mach remains a contextual value and a native target when supplied.
Other coordinates are supported by the shared contract without automatic axis switching.

The producer supplies target coordinates already projected into the declared display coordinate.
It must use calibrated air data and the appropriate compressible-flow model.
The renderer never equates IAS, CAS, EAS, or dynamic pressure.

Envelope intervals describe one declared aircraft condition and one model revision.
They are conditional state limits, not proof of trajectory reachability or sustained performance.
The producer must evaluate coupled constraints before projecting them.
Separate interval kinds represent operating limits, protection, awareness, performance, and preferred operation.
Boundaries retain their limiting reasons.
Operating limits do not represent structural failure thresholds.
No interval represents guaranteed safety or trajectory reachability.

Complete coverage with no operating interval means an empty allowed set.
Partial or unavailable coverage does not make that claim.
Intervals of the same kind must be ordered and must not overlap.
The display never replaces disconnected intervals with their convex hull.
It never stretches the allowed set to fill a constant visual width.

Current speed and envelope validity remain independent.
Invalid boundaries remove boundary geometry without removing trustworthy measured speed.
Invalid effective target projections remove the bug without inventing a replacement target.
Selected target readouts remain identifiable as selections.

## Shared rendering

Shared symbology owns FMA labels, command gating, target formatting, boundary semantics, and alert ordering.
Instrument sets supply geometry and placement.
HWD steering cues use a bounded aircraft-attitude reference in both layouts.
The forward layout labels its command reference `FD ATTITUDE`.
They never follow head attitude as aircraft commands.
Compact layouts preserve active modes, effective targets, and critical alerts.

## Verification

Tests cover wire offsets, unknown codes, truncation, legacy compatibility, and independent group failures.
Tests exercise stale commands, stale attitude, reversions, protection, target identity mismatches, and disconnected intervals.
Cross-set scene tests compare meaning while allowing different geometry.
Review cases exercise forward, off-axis, and unusual-attitude presentations.

Browser previews verify layout only.
Optical alignment, latency, readability, workload, and external hazard detection require separate HWD evaluation.
No fixed IAS-to-Mach axis transition receives operational credit from these software tests.

## Producer interface

`FlightInputs` carries optional stamped guidance and speed reports.
`GuidanceSample` carries independent engagement states, active and armed modes, commands, protection state, and the effective speed target.
`SpeedSample` carries the display coordinate, measured speed, supplied derivative, contextual speed, target projections, and conditional intervals.
Speed coordinates use metres per second, dimensionless Mach, or pascals.
Display code converts velocity coordinates to knots.

`FlightIngress` pins one producer within an authenticated transport session.
The host supplies a `FlightReport` and monotonic receipt time through `observe`.
The report identity orders publications independently from the guidance computation identity.
`inputs_may_be_stale` adds elapsed host time to acquisition ages.
It returns explicit invalid ages when host time is invalid.
Duplicate or older publications never refresh ages.
The host creates a new ingress instance when it intentionally changes the authenticated producer session.

The host assigns the returned inputs to `AircraftState.flight` before resolution.
Existing quality and snapshot-coherence declarations still apply.
The host resolves once and distributes the resulting `PanelData` to every instrument.
It distributes one `AlertOutput` from the central manager with that same frame.

`AlertCondition::Autoflight` accepts producer-declared disconnect, speed-protection, and mode-reversion events.
Displays never infer a disconnect from missing telemetry.
The simulator alert policy assigns AP disconnect to warning and the other declared events to caution.
Operational installations require their own hazard assessment and aural implementation.

Rust consumers access legacy display fields through `PanelData.flight`.
`AircraftState.director`, `ap_modes`, and `ap_targets` keep their legacy wire layouts.
New complete structure initializers must provide `AircraftState.flight` or use the default structure update.

## Wire payloads

All multibyte values use little-endian encoding.
Every payload follows the existing ABI v8 tag and length header.
Reserved bytes are zero when encoded.
Unknown enum codes invalidate the affected indication.
Unknown presence bits invalidate their corresponding report.
NaN denotes an absent optional float unless an explicit presence bit requires that value.

| Guidance tag `0x16` offset | Field |
|---|---|
| 0, 4, 8 | Source, epoch, and computation sequence, each `u32`. |
| 12–14 | AP, FD, and automatic thrust engagement codes. |
| 15 | Speed-control channel. |
| 16–21 | Active and armed lateral, vertical, and thrust modes. |
| 22, 23 | Protection state and transition reason. |
| 24 | Elapsed transition milliseconds, `f32`. |
| 28 | Native target coordinate. |
| 29 | Presence bits: bit 0 target; bit 1 attitude command. |
| 30–31 | Reserved. |
| 32, 36, 40 | Native target, commanded pitch, and commanded bank, each `f32`. |
| 44 | Acquisition age in milliseconds, `f32`. |

The guidance payload contains 48 bytes.

| Speed tag `0x17` offset | Field |
|---|---|
| 0, 4, 8 | Referenced guidance source, epoch, and sequence. |
| 12, 13 | Display coordinate and envelope coverage. |
| 14–15 | Reserved. |
| 16, 20 | Current value and supplied derivative, each `f32`. |
| 24 | Contextual speed coordinate. |
| 25 | Presence bits: bit 0 contextual value; bit 1 selected target; bit 2 effective target. |
| 26, 27 | Selected and effective native coordinates. |
| 28 | Contextual speed value, `f32`. |
| 32, 36 | Selected native value and projected value, each `f32`. |
| 40, 44 | Effective native value and projected value, each `f32`. |
| 48, 52 | Model revision and condition identity, each nonzero `u32` when coverage is available. |
| 56 | Acquisition age in milliseconds, `f32`. |
| 60 | Start of eight 16-byte interval slots. |

The speed payload contains 188 bytes.
Each interval slot contains kind, lower reason, upper reason, and presence bytes at offsets 0–3.
Presence is zero for absence and one for a supplied interval.
Lower and upper `f32` values occupy offsets 4 and 8.
Offsets 12–15 are reserved.
The Rust enum definitions specify each code in this contract.

## Verification mapping

| Requirements | Executable evidence |
|---|---|
| FG-01, FG-02, FG-07, FG-14 | `hwd-bench::cross_set_tests`. |
| FG-03, FG-04, FG-05, FG-10 | State `flight::tests` and cross-set command suppression tests. |
| FG-06 | `transition_attention_uses_producer_elapsed_time_plus_age`. |
| FG-08, FG-09 | Wire offset tests and projection identity tests. |
| FG-11, FG-12, FG-13 | Envelope validation and cross-set scene tests. |
| FG-15 | Separate navigation and command modules with source-qualified input gates. |

The review gallery includes normal, off-axis, stale, mismatched-projection, partial-envelope, disconnected-envelope, reversion, and disconnect cases.
`cargo run -p hwd-bench --example flight-review` renders matching inputs through four production display surfaces.

## Sources

[FAA AC 25.1329-1C, Change 2](https://www.faa.gov/documentLibrary/media/Advisory_Circular/AC_25.1329-1C_CHG2.pdf)
supports distinct modes, transitions, reversions, guidance status, and speed protection indications.

[FAA AC 25-11B](https://www.faa.gov/documentlibrary/media/advisory_circular/ac_25-11b.pdf)
supports target markers, speed awareness cues, clear failure indications, and controlled display clutter.

[Garmin Mach selection](https://static.garmin.com/pumac/190-01717-27_a.pdf)
provides an example of a native Mach target projected onto an IAS tape.

[FAA HWD evaluation](https://www.faa.gov/data_research/research/med_humanfacs/oamtechreports/media/202602.pdf)
reports configuration-dependent workload and external-event detection effects.
