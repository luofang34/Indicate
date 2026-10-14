//! The one clock rule every lane shares.

#![allow(clippy::expect_used, clippy::panic)]

use super::{age_ms, clock_ok};

#[test]
fn a_finite_forward_clock_is_accepted() {
    assert!(clock_ok(0.0, None));
    assert!(clock_ok(10.0, Some(10.0)));
    assert!(clock_ok(11.0, Some(10.0)));
    assert!((age_ms(15.0, 10.0, Some(10.0)) - 5.0).abs() < f64::EPSILON);
}

#[test]
fn a_clock_that_is_not_a_number_is_refused() {
    assert!(!clock_ok(f64::NAN, None));
    assert!(!clock_ok(f64::INFINITY, None));
    assert!(!clock_ok(f64::NEG_INFINITY, Some(0.0)));
    assert!(age_ms(f64::NAN, 0.0, None).is_nan());
}

#[test]
fn a_clock_that_runs_backwards_or_before_zero_is_refused() {
    assert!(!clock_ok(-1.0, None));
    assert!(!clock_ok(9.0, Some(10.0)));
    assert!(age_ms(9.0, 5.0, Some(10.0)).is_nan());
}

#[test]
fn an_age_never_goes_negative_on_an_accepted_clock() {
    // An acceptance recorded on a later reading than the one being
    // aged against is clamped, never negative.
    assert!(age_ms(10.0, 12.0, Some(10.0)).abs() < f64::EPSILON);
}
