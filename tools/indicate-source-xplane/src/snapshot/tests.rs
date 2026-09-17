//! Reply packets become admitted state with the simulator's units
//! converted, its frame mapped, and its clock judged.

#![allow(clippy::expect_used, clippy::panic)]

use indicate_instrument_state::{
    AltitudeClass, FreshnessPolicy, HeadingReference, Quat, SignalStatus, resolve,
};

use super::{BARO_ORIGIN, XPlaneSource};
use crate::datarefs::Dataref;
use crate::units::{HPA_PER_INHG, M_PER_FT, MPS_PER_KT, deg_to_rad};

fn reply(values: &[(Dataref, f32)]) -> Vec<u8> {
    let mut packet = b"RREF,".to_vec();
    for (dataref, value) in values {
        packet.extend_from_slice(&dataref.index().to_le_bytes());
        packet.extend_from_slice(&value.to_le_bytes());
    }
    packet
}

fn everything(sim_time_s: f32) -> Vec<u8> {
    reply(&[
        (Dataref::PitchDeg, 5.0),
        (Dataref::RollDeg, -10.0),
        (Dataref::HeadingTrueDeg, 90.0),
        (Dataref::HeadingMagDeg, 80.0),
        (Dataref::RollRateDps, 1.0),
        (Dataref::PitchRateDps, 2.0),
        (Dataref::YawRateDps, 3.0),
        (Dataref::LocalX, 100.0),
        (Dataref::LocalY, 300.0),
        (Dataref::LocalZ, -200.0),
        (Dataref::LocalVx, 10.0),
        (Dataref::LocalVy, -1.0),
        (Dataref::LocalVz, -50.0),
        (Dataref::IasKt, 120.0),
        (Dataref::TasMps, 65.0),
        (Dataref::AltitudeFt, 5000.0),
        (Dataref::BaroInHg, 29.92),
        (Dataref::HeadingBugMagDeg, 45.0),
        (Dataref::AltitudeSelFt, 6000.0),
        (Dataref::WindFromMagDeg, 270.0),
        (Dataref::WindSpeedKt, 15.0),
        (Dataref::SimTimeS, sim_time_s),
    ])
}

#[test]
fn a_full_reply_becomes_valid_state_in_si_and_ned() {
    let mut source = XPlaneSource::new();
    assert_eq!(
        source.observe(&everything(100.0), 1000.0).expect("applied"),
        22
    );
    let state = source.snapshot(1010.0);

    let attitude = state.attitude.data.expect("attitude");
    let expected = Quat::from_euler(deg_to_rad(-10.0), deg_to_rad(5.0), deg_to_rad(90.0));
    assert!((attitude.quat.w - expected.w).abs() < 1e-6);
    assert!((attitude.quat.z - expected.z).abs() < 1e-6);
    assert!((attitude.rates_rps[0] - deg_to_rad(1.0)).abs() < 1e-6);
    assert_eq!(state.attitude.age_ms, Some(10.0));

    let kinematics = state.kinematics.data.expect("kinematics");
    // OpenGL (100 east, 300 up, -200 south) is (200 north, 100 east, -300 down).
    assert_eq!(kinematics.pos_ned_m, [200.0, 100.0, -300.0]);
    assert_eq!(kinematics.vel_ned_mps, [50.0, 10.0, 1.0]);

    let air = state.air.data.expect("air");
    assert!((air.ias_mps.expect("ias") - 120.0 * MPS_PER_KT).abs() < 1e-4);
    assert!((air.baro_setting_hpa.expect("baro") - 29.92 * HPA_PER_INHG).abs() < 1e-3);
    assert_eq!(air.tas_mps, Some(65.0));

    assert_eq!(state.altitude.reference_class, AltitudeClass::BaroIndicated);
    assert!((state.altitude.sample_m.expect("alt") - 5000.0 * M_PER_FT).abs() < 1e-3);
    assert_eq!(state.altitude.origin, BARO_ORIGIN);

    let heading = state.heading.data.expect("heading");
    assert_eq!(heading.reference, HeadingReference::Magnetic);
    assert!((heading.heading_rad - deg_to_rad(80.0)).abs() < 1e-6);
    let variation = state.variation.data.expect("variation");
    assert!((variation.east_positive_rad - deg_to_rad(10.0)).abs() < 1e-6);
    let wind = state.wind.data.expect("wind");
    assert!((wind.from_rad - deg_to_rad(280.0)).abs() < 1e-5);
    assert!((wind.speed_mps - 15.0 * MPS_PER_KT).abs() < 1e-4);

    assert_eq!(
        state.selections.heading_bug_reference,
        HeadingReference::Magnetic
    );
    assert!((state.selections.altitude_sel_m.expect("sel") - 6000.0 * M_PER_FT).abs() < 1e-3);
    assert_eq!(
        state.selections.altitude_sel_class,
        AltitudeClass::BaroIndicated
    );
    assert!(state.valid.attitude && state.valid.position && state.valid.heading);
    assert!(source.provenance().is_simulation());

    let data = resolve(&state, &FreshnessPolicy::default());
    assert_eq!(data.pitch_rad.status, SignalStatus::Valid);
    assert_eq!(data.ias_kt.status, SignalStatus::Valid);
}

#[test]
fn a_group_is_missing_until_every_constituent_has_arrived() {
    let mut source = XPlaneSource::new();
    source
        .observe(&reply(&[(Dataref::WindSpeedKt, 10.0)]), 0.0)
        .expect("applied");
    let state = source.snapshot(1.0);
    assert!(state.wind.data.is_none());
    assert!(state.attitude.data.is_none());
    let data = resolve(&state, &FreshnessPolicy::default());
    assert_eq!(data.pitch_rad.status, SignalStatus::Missing);
}

#[test]
fn a_paused_simulator_repeats_its_clock_and_the_truth_lane_ages_out() {
    let mut source = XPlaneSource::new();
    source.observe(&everything(100.0), 0.0).expect("applied");
    // The same simulation time again: a time regression to the gate,
    // refused and counted; the admitted attitude keeps its age.
    source.observe(&everything(100.0), 500.0).expect("applied");
    assert_eq!(source.counters().refused_truth, 1);
    let state = source.snapshot(4000.0);
    assert_eq!(state.attitude.age_ms, Some(4000.0));
    let data = resolve(&state, &FreshnessPolicy::default());
    assert_eq!(data.pitch_rad.status, SignalStatus::Failed);
    // The clock advances: admitted again, fresh again.
    source.observe(&everything(100.5), 4100.0).expect("applied");
    let state = source.snapshot(4110.0);
    assert_eq!(state.attitude.age_ms, Some(10.0));
}

#[test]
fn an_untrusted_host_clock_applies_nothing_and_ages_nothing() {
    let mut source = XPlaneSource::new();
    source.observe(&everything(100.0), 100.0).expect("applied");
    assert!(source.observe(&everything(101.0), 50.0).is_err());
    assert!(source.observe(&everything(101.0), f64::NAN).is_err());
    let state = source.snapshot(f64::NAN);
    assert_eq!(state.air.age_ms, None);
    assert_eq!(state.attitude.age_ms, None);
}

#[test]
fn unknown_indices_and_non_finite_values_are_skipped_and_counted() {
    let mut source = XPlaneSource::new();
    let mut packet = reply(&[(Dataref::IasKt, f32::NAN)]);
    packet.extend_from_slice(&99i32.to_le_bytes());
    packet.extend_from_slice(&1.0f32.to_le_bytes());
    assert_eq!(source.observe(&packet, 0.0).expect("applied"), 0);
    assert_eq!(source.counters().unknown_indices, 1);
    assert!(source.snapshot(1.0).air.data.is_none());
}
