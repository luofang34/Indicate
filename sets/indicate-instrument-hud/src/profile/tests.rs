#![allow(clippy::expect_used)]
use super::*;
use crate::panel::tests::flight_state;
use indicate_instrument_state::{FreshnessPolicy, UnusualAttitudeState, resolve_stateful};

fn unusual(roll: f32, pitch: f32) -> bool {
    let profile = fighter_profile().expect("valid limits");
    let mut latch = UnusualAttitudeState::default();
    let data = resolve_stateful(
        &flight_state(roll, pitch, 0.0),
        &FreshnessPolicy::default(),
        &profile,
        &mut latch,
    );
    data.presentation.unusual
}

#[test]
fn ordinary_combat_attitudes_do_not_engage_recovery() {
    assert!(!unusual(0.0, 45.0 * DEG));
    assert!(!unusual(100.0 * DEG, 0.0));
    assert!(!unusual(-80.0 * DEG, -30.0 * DEG));
}

#[test]
fn near_vertical_steep_dive_and_inverted_engage_recovery() {
    assert!(unusual(0.0, 85.0 * DEG));
    assert!(unusual(0.0, -65.0 * DEG));
    assert!(unusual(150.0 * DEG, 0.0));
}
