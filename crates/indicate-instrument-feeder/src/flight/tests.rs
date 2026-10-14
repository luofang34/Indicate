#![allow(clippy::expect_used, clippy::panic)]
use super::*;
use indicate_instrument_state::{FreshnessPolicy, SignalStatus, abi::v8::fixtures, resolve};

fn report(sequence: u32) -> FlightReport {
    FlightReport {
        identity: SolutionId {
            source: 5,
            epoch: 0,
            sequence,
        },
        inputs: fixtures::extended().flight,
    }
}

#[test]
fn replay_never_refreshes_acquisition_age() {
    let mut ingress = FlightIngress::new();
    assert_eq!(ingress.observe(report(8), 100.0), FlightAdmission::Accepted);
    assert_eq!(ingress.observe(report(8), 700.0), FlightAdmission::Replay);
    assert_eq!(
        ingress.inputs_may_be_stale(700.0).guidance.age_ms,
        Some(640.0)
    );
    assert_eq!(ingress.observe(report(7), 800.0), FlightAdmission::Replay);
    assert_eq!(ingress.inputs_may_be_stale(800.0).speed.age_ms, Some(745.0));
}

#[test]
fn wrapping_sequence_and_epoch_ordering_pin_the_producer() {
    let mut ingress = FlightIngress::new();
    assert_eq!(
        ingress.observe(report(u32::MAX), 0.0),
        FlightAdmission::Accepted
    );
    assert_eq!(ingress.observe(report(0), 1.0), FlightAdmission::Accepted);
    let mut next = report(0);
    next.identity.epoch = 1;
    assert_eq!(ingress.observe(next, 2.0), FlightAdmission::Accepted);
    assert_eq!(ingress.observe(report(100), 3.0), FlightAdmission::Replay);
    next.identity.source = 6;
    assert_eq!(ingress.observe(next, 4.0), FlightAdmission::DifferentSource);
}

#[test]
fn clock_failure_preserves_authority_and_suppresses_guidance() {
    let mut ingress = FlightIngress::new();
    assert_eq!(ingress.observe(report(1), 100.0), FlightAdmission::Accepted);
    assert_eq!(
        ingress.observe(report(2), 99.0),
        FlightAdmission::InvalidClock
    );
    let mut state = fixtures::extended();
    state.flight = ingress.inputs_may_be_stale(f64::NAN);
    assert!(state.flight.guidance.data.is_some());
    let data = resolve(&state, &FreshnessPolicy::default());
    assert_ne!(data.flight.guidance.status, SignalStatus::Valid);
    assert!(indicate_instrument_state::flight::presentation::attitude_error(&data).is_none());
}

#[test]
fn fresh_publication_does_not_make_an_old_measurement_current() {
    let mut report = report(1);
    report.inputs.guidance.age_ms = Some(9000.0);
    let mut ingress = FlightIngress::new();
    assert_eq!(ingress.observe(report, 10.0), FlightAdmission::Accepted);
    assert_eq!(
        ingress.inputs_may_be_stale(20.0).guidance.age_ms,
        Some(9010.0)
    );
}
