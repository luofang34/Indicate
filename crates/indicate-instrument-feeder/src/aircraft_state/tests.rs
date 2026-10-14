//! The seam from admitted lanes to instrument state keeps the trackers'
//! discipline: nothing fresh from an untrusted clock, nothing trusted
//! that was not authorized, and the shell's generation untouched.

#![allow(clippy::expect_used, clippy::panic)]

use indicate_instrument_state::{
    AircraftState, EstimateQuality, FreshnessPolicy, SignalStatus, SnapshotCoherence, resolve,
};

use super::{apply_avionics, apply_flight, apply_nav};
use crate::avionics::{
    AttitudeGroup, AvionicsIngress, AvionicsSample, IncarnationPolicy, IngressConfig,
    KinematicsGroup,
};
use crate::stamp::{CLOCK_SIMULATION, ROLE_OPERATIONAL_ESTIMATE, RawStamp};

const VEHICLE: u64 = 42;

fn ingress() -> AvionicsIngress {
    AvionicsIngress::new(IngressConfig {
        vehicle_id: VEHICLE,
        role: ROLE_OPERATIONAL_ESTIMATE,
        source_id: None,
        incarnation: None,
        incarnation_policy: IncarnationPolicy::PinFirst,
        maximum_seen_incarnations: 8,
        maximum_skew_nanos: 5_000_000,
    })
}

fn sample(sequence: u32) -> AvionicsSample {
    let stamp = RawStamp {
        role: ROLE_OPERATIONAL_ESTIMATE,
        integrity: 2,
        source_id: 7,
        incarnation: [1; 16],
        epoch: 1,
        sequence,
        acquired_at_ns: 1_000_000,
        clock: CLOCK_SIMULATION,
    };
    AvionicsSample {
        vehicle_id: VEHICLE,
        attitude: AttitudeGroup {
            quat: [1.0, 0.0, 0.0, 0.0],
            rates: [0.1, 0.2, 0.3],
            arm_state: 0,
        },
        kinematics: KinematicsGroup {
            pos_ned: [10.0, 20.0, -30.0],
            vel_ned: [40.0, 0.0, -1.0],
            arm_state: 0,
        },
        valid_flags: 0b1111,
        quality: 0,
        attitude_stamp: Some(stamp),
        kinematics_stamp: Some(stamp),
        estimator_status_stamp: Some(stamp),
    }
}

#[test]
fn an_admitted_publication_becomes_fed_valid_groups() {
    let mut ingress = ingress();
    assert!(ingress.ingest(&sample(1), 100.0));
    let mut state = AircraftState::default();
    state.snapshot.generation = 9;
    apply_avionics(&ingress.snapshot(110.0), &mut state);

    let attitude = state.attitude.data.expect("fed");
    assert_eq!(attitude.quat.w, 1.0);
    assert_eq!(attitude.rates_rps, [0.1, 0.2, 0.3]);
    assert_eq!(state.attitude.age_ms, Some(10.0));
    let kinematics = state.kinematics.data.expect("fed");
    assert_eq!(kinematics.pos_ned_m, [10.0, 20.0, -30.0]);
    assert_eq!(kinematics.vel_ned_mps, [40.0, 0.0, -1.0]);
    assert_eq!(state.quality, EstimateQuality::Good);
    assert!(state.valid.attitude && state.valid.rates);
    assert!(state.valid.position && state.valid.velocity_horizontal);
    // No lane here vouches for these; they stay fail-closed.
    assert!(!state.valid.velocity_vertical && !state.valid.heading);
    assert_eq!(state.snapshot.coherence, SnapshotCoherence::Coherent);
    // The shell owns the snapshot generation; a tracker never writes it.
    assert_eq!(state.snapshot.generation, 9);

    let data = resolve(&state, &FreshnessPolicy::default());
    assert_eq!(data.pitch_rad.status, SignalStatus::Valid);
}

#[test]
fn nothing_admitted_maps_to_missing_groups() {
    let ingress = ingress();
    let mut state = AircraftState::default();
    apply_avionics(&ingress.snapshot(0.0), &mut state);
    assert!(state.attitude.data.is_none());
    assert!(state.kinematics.data.is_none());
    assert_eq!(state.quality, EstimateQuality::Unusable);
    let data = resolve(&state, &FreshnessPolicy::default());
    assert_eq!(data.pitch_rad.status, SignalStatus::Missing);
}

#[test]
fn an_age_the_gate_could_not_measure_resolves_missing_not_fresh() {
    let mut ingress = ingress();
    assert!(ingress.ingest(&sample(1), 100.0));
    let mut state = AircraftState::default();
    // A clock behind the one the gate acted on: the snapshot ages NaN.
    apply_avionics(&ingress.snapshot(50.0), &mut state);
    assert!(state.attitude.data.is_some());
    assert_eq!(state.attitude.age_ms, None);
    let data = resolve(&state, &FreshnessPolicy::default());
    assert_eq!(data.pitch_rad.status, SignalStatus::Missing);
}

#[test]
fn quality_outside_the_declared_codes_is_unknown() {
    assert_eq!(super::quality(0), EstimateQuality::Good);
    assert_eq!(super::quality(1), EstimateQuality::Degraded);
    assert_eq!(super::quality(2), EstimateQuality::Unusable);
    assert_eq!(super::quality(3), EstimateQuality::Unknown);
    assert_eq!(super::quality(u32::MAX), EstimateQuality::Unknown);
}

#[test]
fn absent_navigation_and_flight_lanes_leave_defaults() {
    let mut state = AircraftState::default();
    apply_nav(None, &mut state);
    assert!(state.nav.data.is_none());
    apply_flight(Default::default(), &mut state);
    assert_eq!(state.flight, Default::default());
}
