//! The lanes the round flight instruments read: the cockpit clock, a
//! shaft angle for the synchroscope, and the body yaw rate for a turn
//! needle.

use std::vec;

use super::fixtures;
use super::tests::locate_payload;
use super::{CAPACITY, decode_state, encode_state_samples};
use crate::aircraft::{AircraftState, ClockSample, Stamped};
use crate::engine::{EngineField, EngineSample};
use crate::group_id::GroupId;
use crate::signal::{FreshnessPolicy, SignalStatus};
use crate::validate::{GroupFault, validate_state};

fn encode(state: &AircraftState) -> vec::Vec<u8> {
    let mut buf = vec![0u8; CAPACITY];
    let len =
        encode_state_samples(state, &fixtures::beside_samples(state), &mut buf).expect("fits");
    buf.truncate(len);
    buf
}

#[test]
fn the_clock_round_trips_as_eight_bytes() {
    let state = fixtures::full();
    let frame = encode(&state);
    let at = locate_payload(&frame, GroupId::Clock.to_u8()).expect("clock present");
    assert_eq!(
        usize::from(u16::from_le_bytes([frame[at - 2], frame[at - 1]])),
        8
    );
    let decoded = decode_state(&frame).expect("decodes").state;
    assert_eq!(decoded.clock, state.clock);
    let data = crate::resolve(&decoded, &FreshnessPolicy::default());
    assert_eq!(data.clock_s.status, SignalStatus::Valid);
    assert_eq!(data.clock_s.value, 37_815.5);
}

#[test]
fn a_time_outside_the_day_faults_the_clock() {
    for bad in [-1.0, 86_400.0, f32::NAN, f32::INFINITY] {
        let state = AircraftState {
            clock: Stamped {
                data: Some(ClockSample { time_of_day_s: bad }),
                age_ms: Some(10.0),
            },
            ..fixtures::full()
        };
        assert_eq!(
            validate_state(&state).clock,
            Some(GroupFault::NonFinite),
            "{bad}"
        );
        let data = crate::resolve(&state, &FreshnessPolicy::default());
        assert!(!data.clock_s.status.shows_value(), "{bad}");
    }
}

#[test]
fn a_withheld_clock_is_missing() {
    let state = crate::withhold_group(&fixtures::full(), GroupId::Clock);
    let data = crate::resolve(&state, &FreshnessPolicy::default());
    assert_eq!(data.clock_s.status, SignalStatus::Missing);
}

#[test]
fn a_shaft_angle_is_a_turn_and_no_more() {
    let field = EngineField::ShaftAngleDeg;
    assert!(field.admits(0.0) && field.admits(359.9));
    assert!(!field.admits(360.0) && !field.admits(-0.1) && !field.admits(f32::NAN));
    let sample = EngineSample {
        shaft_angle_deg: Some(400.0),
        ..EngineSample::EMPTY
    };
    assert!(!sample.is_sound());
    assert_eq!(fixtures::ENGINES[0].field(field), Some(127.5));
}

#[test]
fn the_yaw_rate_shows_only_under_declared_body_rates() {
    let state = fixtures::full();
    let data = crate::resolve(&state, &FreshnessPolicy::default());
    let rates = state.attitude.data.expect("attitude").rates_rps;
    assert_eq!(data.yaw_rate_rps.value, rates[2]);
    assert_eq!(data.yaw_rate_rps.status, SignalStatus::Valid);
    let mut undeclared = state;
    undeclared.valid.rates = false;
    let data = crate::resolve(&undeclared, &FreshnessPolicy::default());
    assert!(!data.yaw_rate_rps.status.shows_value());
    // Attitude itself stays valid: the rates flag gates only the rate.
    assert_eq!(data.roll_rad.status, SignalStatus::Valid);
}

#[test]
fn a_compass_reads_true_heading_less_east_variation() {
    let state = fixtures::full();
    let data = crate::resolve(&state, &FreshnessPolicy::default());
    // 0.35 rad true with 0.15 rad east variation is 0.20 rad magnetic.
    let magnetic = data.heading.magnetic_heading_rad();
    assert_eq!(magnetic.status, SignalStatus::Valid);
    assert!((magnetic.value - 0.20).abs() < 1e-6, "{}", magnetic.value);
    let mut no_variation = state;
    no_variation.variation = Stamped::default();
    let data = crate::resolve(&no_variation, &FreshnessPolicy::default());
    assert_eq!(
        data.heading.magnetic_heading_rad().status,
        SignalStatus::Failed
    );
}
