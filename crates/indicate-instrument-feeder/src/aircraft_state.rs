//! The one place admitted feeder lanes become instrument state (VAL-01,
//! ADR-0029).
//!
//! Each tracker hands back what it admitted in its own vocabulary: raw
//! quaternions and NED vectors, a guidance sample, aged flight reports.
//! A shell that wrote the mapping into [`AircraftState`] itself would
//! write it differently from the next shell, and the discipline the
//! trackers enforce (ages that are NaN on an untrusted clock, trust
//! that is declared rather than assumed) would be lost at the seam. The
//! functions here are that seam, once.
//!
//! What they deliberately leave alone: [`AircraftState::snapshot`]'s
//! generation, which the shell's gate advances per admitted snapshot
//! and never copies from a tracker (the backend contract's generation
//! rule); and every valid flag no lane here authorizes (heading,
//! variation, turn, slip, airspeed trend), which stay false so their
//! groups resolve `Failed` until a lane vouches for them.

use indicate_instrument_state::{
    AircraftState, Attitude, EstimateQuality, Kinematics, Quat, SnapshotCoherence, Stamped,
    ValidFlags, flight::FlightInputs,
};

use crate::avionics::{Coherence, IngressSnapshot};
use crate::nav_display::nav_display_state;
use crate::nav_guidance::NavSnapshot;

/// Trust valid-flag bits the avionics gate authorizes, in the state
/// ABI's bit order: attitude, rates, position, horizontal velocity.
const BIT_ATTITUDE: u32 = 1 << 0;
const BIT_RATES: u32 = 1 << 1;
const BIT_POSITION: u32 = 1 << 2;
const BIT_VELOCITY_HORIZONTAL: u32 = 1 << 3;

/// Writes the admitted avionics lanes into `state`: attitude,
/// kinematics, the declared estimate quality, the valid flags the gate
/// authorized, and snapshot coherence.
///
/// A lane the gate has not admitted leaves its group at the default,
/// which resolves `Missing`. An age the gate could not measure (NaN, on
/// an untrusted clock) becomes `None`, which also resolves `Missing`: a
/// broken clock never reads as fresh data.
pub fn apply_avionics(snapshot: &IngressSnapshot, state: &mut AircraftState) {
    state.attitude = snapshot
        .attitude
        .map_or_else(Stamped::default, |group| Stamped {
            data: Some(Attitude {
                quat: Quat {
                    w: group.data.quat[0],
                    x: group.data.quat[1],
                    y: group.data.quat[2],
                    z: group.data.quat[3],
                },
                rates_rps: group.data.rates,
            }),
            age_ms: age(group.age_ms),
        });
    state.kinematics = snapshot
        .kinematics
        .map_or_else(Stamped::default, |group| Stamped {
            data: Some(Kinematics {
                pos_ned_m: group.data.pos_ned,
                vel_ned_mps: group.data.vel_ned,
            }),
            age_ms: age(group.age_ms),
        });
    state.quality = quality(snapshot.quality);
    state.valid = ValidFlags {
        attitude: snapshot.valid_flags & BIT_ATTITUDE != 0,
        rates: snapshot.valid_flags & BIT_RATES != 0,
        position: snapshot.valid_flags & BIT_POSITION != 0,
        velocity_horizontal: snapshot.valid_flags & BIT_VELOCITY_HORIZONTAL != 0,
        ..state.valid
    };
    state.snapshot.coherence = match snapshot.coherence.status {
        Coherence::Insufficient => SnapshotCoherence::Insufficient,
        Coherence::Coherent => SnapshotCoherence::Coherent,
        Coherence::ExcessiveSkew => SnapshotCoherence::ExcessiveSkew,
    };
}

/// Writes the navigation lane into `state` through the display profile,
/// so absent guidance is absent and a NaN age is `Missing`.
pub fn apply_nav(snapshot: Option<&NavSnapshot>, state: &mut AircraftState) {
    state.nav = nav_display_state(snapshot).unwrap_or_default();
}

/// Writes aged flight reports into `state`; the ages inside are the
/// producer's plus host elapsed time, already judged by
/// [`crate::flight::FlightIngress`].
pub fn apply_flight(inputs: FlightInputs, state: &mut AircraftState) {
    state.flight = inputs;
}

/// A tracker age into a state age: finite, non-negative, and within
/// `f32`, or `None`. `None` resolves `Missing`.
fn age(age_ms: f64) -> Option<f32> {
    (age_ms.is_finite() && age_ms >= 0.0 && age_ms <= f64::from(f32::MAX)).then_some(age_ms as f32)
}

/// The gate's quality code in the state's vocabulary; a code outside
/// the declared set is `Unknown`, which resolves `Failed`.
fn quality(code: u32) -> EstimateQuality {
    match code {
        0 => EstimateQuality::Good,
        1 => EstimateQuality::Degraded,
        2 => EstimateQuality::Unusable,
        _ => EstimateQuality::Unknown,
    }
}

#[cfg(test)]
mod tests;
