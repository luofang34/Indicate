//! The state-group vocabulary: stable identities for every group a
//! source can supply (ADR-0029 extensible state groups).
//!
//! A group id is a wire tag, a descriptor requirement, and a status key
//! all at once, so the enum is the single registration point: adding a
//! group adds one variant, and the exhaustive matches over it (wire
//! codec, minimum length, status reporting, withholding) each become a
//! compile error until the new group is handled there.
//!
//! # Id registry (append-only)
//!
//! Assigned ids never change meaning and are never reused. Reserved
//! ranges, recorded here so an allocation is a doc edit before it is a
//! variant:
//!
//! | id | group |
//! |----|-------|
//! | 0x00 | never assigned (guards zeroed memory) |
//! | 0x01–0x0D | the variants below |
//! | 0x0E | engine indications (stamped) |
//! | 0x0F | traffic (planned) |
//! | 0x10 | projection view (synthetic vision; planned) |
//! | 0x11 | terrain bands (planned) |
//! | 0x12 | bearing pointers (stamped) |
//! | 0x13 | airframe configuration (stamped) |
//! | 0x14 | autopilot/flight-director modes (stamped) |
//! | 0x15 | autopilot targets (declared) |
//! | 0x16 | coherent flight guidance (stamped) |
//! | 0x17 | speed presentation and conditional envelope (stamped) |
//! | 0x18 | fuel tanks (stamped, variable) |
//! | 0x19 | cockpit clock (stamped) |
//! | 0x1A–0xDF | future standard groups |
//! | 0xE0–0xEF | experimentation; never in committed fixtures |
//! | 0xF0–0xFF | never assigned |
//!
//! # Allocated ids and their layouts
//!
//! An id is allocated before its group exists, so that one coordinated
//! ABI revision carries every group that needs one rather than a bump
//! per group. This table is the layout contract each group implements
//! against.
//!
//! An allocated id with no variant is not on the wire: `from_u8`
//! answers `None`, and a decoder counts its tag and skips the payload.
//! `reserved_and_allocated_tags_have_no_status_slot_yet` holds that
//! until the variant lands.
//!
//! New groups:
//!
//! | id | group | lane | reason |
//! |----|-------|------|--------|
//! | 0x12 | BearingPointers | stamped | Two pointers. Per pointer: a source enum in the shipped `NavSource` coding (0 None, 1 Gps, 2 Nav1, 3 Nav2, 255 fail-closed Unknown), bearing rad f32, a heading reference, and a per-pointer validity. |
//! | 0x13 | AirframeConfig | stamped | Flap position ratio plus an optional selected detent (sensed and selected are never conflated), and per-axis trim ratios (elevator first). Every field is optional and NaN-absent. Configuration takes its own id: flap and trim are airframe configuration, not engine, so 0x0E keeps its engine charter. |
//! | 0x14 | ApModes | stamped | Engagement (Off/FD/AP, Unknown fail-closed), active and armed lateral mode, active and armed vertical mode. Each mode enum has a None value distinct from Unknown. Stamped because a stale "AP engaged" annunciation is the failure the freshness discipline exists to prevent (the `fc_state.rs` precedent). |
//! | 0x15 | ApTargets | declared | Selected airspeed m/s, selected vertical speed m/s, and an altitude target with the same reference-identity trio as `altitude_sel` (class, origin, geoid model). Declared per the issue's stamped-versus-declared analysis: the targets are defensibly UI-like state, while modes and engagement are telemetry. |
//! | 0x0E | Engine | stamped, variable | count u16, reserved u16 (zero), age f32, then `count` samples packed back to back. Each sample is a presence mask u16 over the append-only `EngineField` registry, then its present fields in bit order, each an f32: bit 0 manifold pressure hPa, 1 rpm, 2 oil temperature °C, 3 oil pressure kPa, 4 fuel pressure kPa, 5 coolant temperature °C, 6 cylinder-head temperature °C, 7 carburettor air temperature °C, 8 shaft angle degrees in `[0, 360)`. The length must equal the header plus every sample's size; an unknown bit or a disagreeing length faults the group. There is no engine capacity: the samples travel beside the owned state as an `EngineSamples` view. Readings are indicated values: gauge lag and error are the source's physics. Allocated in ABI v9. |
//! | 0x18 | FuelTanks | stamped, variable | The engine group's layout over the `TankField` registry, one sample per tank in the source's tank order: bit 0 quantity L, 1 capacity L, 2 quantity kg, 3 capacity kg. Fuel is per tank, not per engine, so tanks are their own group; the instrument model maps an index to FRONT L, REAR R and so on. Allocated in ABI v9. |
//! | 0x19 | Clock | stamped | The time of day the cockpit clock shows, seconds after midnight in `[0, 86400)`, then the age: 8 bytes. A clock is an instrument with its own error and the time it was set to, so the source writes what its clock reads, not a time the display derives. Allocated in ABI v9. |
//!
//! Field appends to existing groups follow the stamped lane growth
//! policy: a new field appends AFTER the trailing `age_ms`, and an older
//! decoder accepts the payload and counts the tail.
//!
//! | group | append |
//! |-------|--------|
//! | Air (0x03) | `tas_mps f32`, NaN-absent (12 to 16 bytes) |
//! | Air (0x03) | `vsi_mps f32`, the rate-of-climb instrument's reading, NaN-absent (16 to 20 bytes; ABI v9) |
//! | Dynamics (0x0B) | `ias_trend f32` (d(IAS)/dt), NaN-absent, plus Trust valid bit 9 (16 to 20 bytes) |
//! | Nav (0x04) | `scale_mode u8` (Enroute/Terminal/Approach, 255 fail-closed Unknown; 42 to 43 bytes) |
//! | Nav (0x04) | `facility_type u8` (None/VOR/Localizer, any other value fail-closed Unknown, and a facility that cannot belong to the source fails the group) then `sequencing u8` (Sequencing/HOLD/SUSP, any other value fail-closed Unknown; 43 to 45 bytes; ABI v10). A later facility kind such as GLS is an appended value. |

use crate::aircraft::AircraftState;
use crate::signal::SignalStatus;

/// Stable identity of one state group.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum GroupId {
    /// Attitude quaternion and body rates.
    Attitude = 0x01,
    /// NED position and velocity.
    Kinematics = 0x02,
    /// Air data: indicated airspeed, applied altimeter setting, and
    /// true airspeed.
    Air = 0x03,
    /// Lateral/vertical navigation guidance, including waypoint idents.
    Nav = 0x04,
    /// Wind estimate.
    Wind = 0x05,
    /// Pilot selections and bugs.
    Selections = 0x06,
    /// Source trust: quality, validity flags, snapshot coherence and
    /// generation.
    Trust = 0x07,
    /// Datum-qualified altitude declaration.
    Altitude = 0x08,
    /// Independent, reference-typed heading sample.
    Heading = 0x09,
    /// Magnetic-variation sample.
    Variation = 0x0A,
    /// Typed turn and slip/skid estimates.
    Dynamics = 0x0B,
    /// Machine-monitoring text channel.
    MonitorText = 0x0C,
    /// Flight-director commanded attitude, mode, and engagement.
    FlightDirector = 0x0D,
    /// Engine indications: manifold pressure and speed per engine.
    Engine = 0x0E,
    /// Bearing pointers, independent of the selected nav source.
    BearingPointers = 0x12,
    /// Airframe configuration: flap position and trim.
    AirframeConfig = 0x13,
    /// Autoflight engagement and the active and armed modes.
    ApModes = 0x14,
    /// The values the automation is flying toward.
    ApTargets = 0x15,
    /// Independent engagements, modes, and effective guidance.
    Guidance = 0x16,
    /// Typed speed values and source-supplied target and envelope projections.
    SpeedPresentation = 0x17,
    /// Fuel quantity per tank, presence-masked and variable in length.
    FuelTanks = 0x18,
    /// The time of day the cockpit clock shows.
    Clock = 0x19,
}

impl GroupId {
    /// Number of defined groups.
    pub const COUNT: usize = 22;

    /// Every defined group in ascending id order: the canonical wire
    /// order. Table slots come from [`Self::index`], not from here.
    pub const ALL: [GroupId; Self::COUNT] = [
        GroupId::Attitude,
        GroupId::Kinematics,
        GroupId::Air,
        GroupId::Nav,
        GroupId::Wind,
        GroupId::Selections,
        GroupId::Trust,
        GroupId::Altitude,
        GroupId::Heading,
        GroupId::Variation,
        GroupId::Dynamics,
        GroupId::MonitorText,
        GroupId::FlightDirector,
        GroupId::Engine,
        GroupId::BearingPointers,
        GroupId::AirframeConfig,
        GroupId::ApModes,
        GroupId::ApTargets,
        GroupId::Guidance,
        GroupId::SpeedPresentation,
        GroupId::FuelTanks,
        GroupId::Clock,
    ];

    /// The wire tag.
    pub const fn to_u8(self) -> u8 {
        self as u8
    }

    /// The group for a wire tag; `None` for ids this build cannot place
    /// (the codec counts and skips them).
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0x01 => Some(GroupId::Attitude),
            0x02 => Some(GroupId::Kinematics),
            0x03 => Some(GroupId::Air),
            0x04 => Some(GroupId::Nav),
            0x05 => Some(GroupId::Wind),
            0x06 => Some(GroupId::Selections),
            0x07 => Some(GroupId::Trust),
            0x08 => Some(GroupId::Altitude),
            0x09 => Some(GroupId::Heading),
            0x0A => Some(GroupId::Variation),
            0x0B => Some(GroupId::Dynamics),
            0x0C => Some(GroupId::MonitorText),
            0x0D => Some(GroupId::FlightDirector),
            0x0E => Some(GroupId::Engine),
            0x12 => Some(GroupId::BearingPointers),
            0x13 => Some(GroupId::AirframeConfig),
            0x14 => Some(GroupId::ApModes),
            0x15 => Some(GroupId::ApTargets),
            0x16 => Some(GroupId::Guidance),
            0x17 => Some(GroupId::SpeedPresentation),
            0x18 => Some(GroupId::FuelTanks),
            0x19 => Some(GroupId::Clock),
            _ => None,
        }
    }

    /// The group's slot in dense per-group tables: [`GroupStatuses`]
    /// and the descriptor `GroupSet` bitset.
    ///
    /// Slots are append-only, like wire tags. A slot is assigned when
    /// its group lands and never moves, because a `GroupSet` is written
    /// into signed plugin descriptors as raw bits: renumbering a slot
    /// would change what an already-signed package declares. So a slot
    /// is neither the wire tag minus one nor the position in
    /// [`Self::ALL`]; [`Self::ALL`] keeps wire order, and the slots are
    /// a dense permutation of `0..COUNT`. The exhaustive match also
    /// fails to compile when a new variant has no slot.
    pub const fn index(self) -> usize {
        match self {
            GroupId::Attitude => 0,
            GroupId::Kinematics => 1,
            GroupId::Air => 2,
            GroupId::Nav => 3,
            GroupId::Wind => 4,
            GroupId::Selections => 5,
            GroupId::Trust => 6,
            GroupId::Altitude => 7,
            GroupId::Heading => 8,
            GroupId::Variation => 9,
            GroupId::Dynamics => 10,
            GroupId::MonitorText => 11,
            GroupId::FlightDirector => 12,
            GroupId::BearingPointers => 13,
            GroupId::AirframeConfig => 14,
            GroupId::ApModes => 15,
            GroupId::ApTargets => 16,
            GroupId::Guidance => 17,
            GroupId::SpeedPresentation => 18,
            GroupId::Engine => 19,
            GroupId::FuelTanks => 20,
            GroupId::Clock => 21,
        }
    }
}

/// Per-group status, keyed by [`GroupId`] — the generic surface a
/// registry or harness asks instead of a method per group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GroupStatuses([SignalStatus; GroupId::COUNT]);

impl GroupStatuses {
    /// The status of one group.
    pub fn status(&self, id: GroupId) -> SignalStatus {
        self.0[id.index()]
    }

    /// Sets the status of one group (resolution-internal).
    pub(crate) fn set(&mut self, id: GroupId, status: SignalStatus) {
        self.0[id.index()] = status;
    }
}

/// `state` with one group withheld, exactly as if the source had never
/// fed it: stamped groups lose data and age, declared groups return to
/// their fail-closed defaults, and the validity flags covering the group
/// are cleared. The admission harness drives panels with this to prove
/// a withheld required group renders `Missing`, never a value.
pub fn withhold_group(state: &AircraftState, group: GroupId) -> AircraftState {
    let mut out = *state;
    match group {
        GroupId::Attitude => {
            out.attitude = Default::default();
            out.valid.attitude = false;
            out.valid.rates = false;
        }
        GroupId::Kinematics => {
            out.kinematics = Default::default();
            out.valid.position = false;
            out.valid.velocity_horizontal = false;
            out.valid.velocity_vertical = false;
        }
        GroupId::Air => out.air = Default::default(),
        GroupId::Nav => out.nav = Default::default(),
        GroupId::Wind => out.wind = Default::default(),
        GroupId::Selections => out.selections = Default::default(),
        GroupId::Trust => {
            out.quality = Default::default();
            out.valid = Default::default();
            out.snapshot = Default::default();
        }
        GroupId::BearingPointers => out.bearings = Default::default(),
        GroupId::AirframeConfig => out.airframe = Default::default(),
        GroupId::Engine => out.engine = Default::default(),
        GroupId::FuelTanks => out.fuel_tanks = Default::default(),
        GroupId::Clock => out.clock = Default::default(),
        GroupId::Altitude => out.altitude = Default::default(),
        GroupId::Heading => {
            out.heading = Default::default();
            out.valid.heading = false;
        }
        GroupId::Variation => {
            out.variation = Default::default();
            out.valid.variation = false;
        }
        GroupId::Dynamics => {
            out.dynamics = Default::default();
            out.valid.turn = false;
            out.valid.slip = false;
            out.valid.ias_trend = false;
        }
        GroupId::MonitorText => out.monitor_text = Default::default(),
        GroupId::FlightDirector => out.director = Default::default(),
        GroupId::ApModes => out.ap_modes = Default::default(),
        GroupId::ApTargets => out.ap_targets = Default::default(),
        GroupId::Guidance => out.flight.guidance = Default::default(),
        GroupId::SpeedPresentation => out.flight.speed = Default::default(),
    }
    out
}

#[cfg(test)]
mod tests;
