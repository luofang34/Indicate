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
//! | 0x0E | engine (planned) |
//! | 0x0F | traffic (planned) |
//! | 0x10 | projection view (synthetic vision; planned) |
//! | 0x11 | terrain bands (planned) |
//! | 0x12 | bearing pointers (stamped) — issue #53; allocated by the v8 batch |
//! | 0x13 | airframe configuration (stamped) — issue #57; allocated by the v8 batch |
//! | 0x14 | autopilot/flight-director modes (stamped) — issue #50; allocated by the v8 batch |
//! | 0x15 | autopilot targets (declared) — issue #50; allocated by the v8 batch |
//! | 0x16–0xDF | future standard groups |
//! | 0xE0–0xEF | experimentation; never in committed fixtures |
//! | 0xF0–0xFF | never assigned |
//!
//! # The v8 batch allocation contract (issue #58)
//!
//! Six issues need wire changes, and issue #58 directs one coordinated
//! ABI revision over six serial bumps. This table is the layout contract
//! the per-issue changes implement against. Version 8 itself adds none
//! of these; each lands as its own change stacked on the v8 bump.
//!
//! New groups:
//!
//! | id | group | lane | reason |
//! |----|-------|------|--------|
//! | 0x12 | BearingPointers | stamped | Two pointers. Per pointer: a source enum (None/Nav1/Nav2/Gps, 255 fail-closed Unknown), bearing rad f32, a heading reference, and a per-pointer validity. |
//! | 0x13 | AirframeConfig | stamped | Flap position ratio plus an optional selected detent (sensed and selected are never conflated), and per-axis trim ratios (elevator first). Every field is optional and NaN-absent. Configuration takes its own id: flap and trim are airframe configuration, not engine, so 0x0E keeps its engine charter (issue #57). |
//! | 0x14 | ApModes | stamped | Engagement (Off/FD/AP, Unknown fail-closed), active and armed lateral mode, active and armed vertical mode. Each mode enum has a None value distinct from Unknown. Stamped because a stale "AP engaged" annunciation is the failure the freshness discipline exists to prevent (the `fc_state.rs` precedent). |
//! | 0x15 | ApTargets | declared | Selected airspeed m/s, selected vertical speed m/s, and an altitude target with the same reference-identity trio as `altitude_sel` (class, origin, geoid model). Declared per the issue's stamped-versus-declared analysis: the targets are defensibly UI-like state, while modes and engagement are telemetry. |
//!
//! Field appends to existing groups follow the stamped lane growth
//! policy: a new field appends AFTER the trailing `age_ms`, and an older
//! decoder accepts the payload and counts the tail.
//!
//! | group | append | issue |
//! |-------|--------|-------|
//! | Air (0x03) | `tas_mps f32`, NaN-absent (12 to 16 bytes) | #52 |
//! | Dynamics (0x0B) | `ias_trend f32` (d(IAS)/dt), NaN-absent, plus Trust valid bit 9 (16 to 20 bytes) | #51 |
//! | Nav (0x04) | `scale_mode u8` (Enroute/Terminal/Approach, 255 fail-closed Unknown) and `facility_type u8`, both after `age_ms` | #54 and the #55 follow-on |

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
    /// Air data: indicated airspeed and applied altimeter setting.
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
}

impl GroupId {
    /// Number of defined groups.
    pub const COUNT: usize = 13;

    /// Every defined group in ascending id order — the canonical wire
    /// order and the index order of [`GroupStatuses`].
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
            _ => None,
        }
    }

    /// Position in [`Self::ALL`], for dense per-group tables.
    ///
    /// The mapping is a match, not arithmetic on the wire tag: assigned
    /// ids stop being contiguous at 0x0E, which the registry holds as a
    /// planned id with no variant, so `tag - 1` is no longer the
    /// position once the v8 batch allocates 0x12. The exhaustive match
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
        }
        GroupId::MonitorText => out.monitor_text = Default::default(),
        GroupId::FlightDirector => out.director = Default::default(),
    }
    out
}

#[cfg(test)]
mod tests;
