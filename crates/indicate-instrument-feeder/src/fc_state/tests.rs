//! FC-state pinning, wrap-safe advancement, and staleness.

#![allow(clippy::expect_used, clippy::panic)]

use super::{FcCommand, FcReport, FcStateTracker};
use crate::stamp::{CLOCK_HOST_MONOTONIC, ROLE_FC_STATE, RawStamp};

fn report(epoch: u32, sequence: u32, arm_state: u32) -> FcReport {
    FcReport {
        stamp: RawStamp {
            role: ROLE_FC_STATE,
            integrity: 2,
            source_id: 3,
            incarnation: [4; 16],
            epoch,
            sequence,
            acquired_at_ns: 0,
            clock: CLOCK_HOST_MONOTONIC,
        },
        arm_state,
        last_command: Some(FcCommand {
            arm: true,
            result: 0,
        }),
    }
}

#[test]
fn only_new_reports_restart_the_age_clock() {
    let mut tracker = FcStateTracker::new(3000.0);
    assert!(tracker.observe(None, 0.0).is_none());
    let view = tracker
        .observe(Some(&report(1, 1, 1)), 0.0)
        .expect("accepted");
    assert_eq!(view.arm_state, 1);
    // Duplicate sequence: the age keeps running.
    let view = tracker
        .observe(Some(&report(1, 1, 2)), 1000.0)
        .expect("view stands");
    assert_eq!(view.arm_state, 1);
    assert_eq!(view.age_ms, 1000.0);
    // A newer sequence restarts it.
    let view = tracker
        .observe(Some(&report(1, 2, 2)), 2000.0)
        .expect("accepted");
    assert_eq!(view.arm_state, 2);
    assert_eq!(view.age_ms, 0.0);
}

#[test]
fn identity_is_pinned_and_epochs_replay_safe() {
    let mut tracker = FcStateTracker::new(3000.0);
    tracker
        .observe(Some(&report(5, 5, 1)), 0.0)
        .expect("accepted");
    // A different source is not this FC's stream.
    let mut foreign = report(6, 6, 2);
    foreign.stamp.source_id = 99;
    let view = tracker.observe(Some(&foreign), 1.0).expect("view stands");
    assert_eq!(view.arm_state, 1);
    // An older epoch is a replay.
    let view = tracker
        .observe(Some(&report(4, 9, 2)), 2.0)
        .expect("view stands");
    assert_eq!(view.arm_state, 1);
    // A newer epoch restarts the numbering.
    let view = tracker
        .observe(Some(&report(6, 1, 0)), 3.0)
        .expect("accepted");
    assert_eq!(view.arm_state, 0);
}

#[test]
fn staleness_and_out_of_range_arm_values_fail_closed() {
    let mut tracker = FcStateTracker::new(3000.0);
    tracker
        .observe(Some(&report(1, 1, 1)), 0.0)
        .expect("accepted");
    let view = tracker.view(3001.0).expect("view");
    assert!(view.stale);
    // An impossible arm value never replaces the view.
    let view = tracker
        .observe(Some(&report(1, 2, 3)), 3002.0)
        .expect("view stands");
    assert_eq!(view.arm_state, 1);
}

#[test]
fn an_untrusted_clock_accepts_nothing_and_views_stale() {
    let mut tracker = FcStateTracker::new(3000.0);
    // Nothing accepted under a clock that is not a number.
    assert!(tracker.observe(Some(&report(1, 1, 1)), f64::NAN).is_none());
    let view = tracker
        .observe(Some(&report(1, 1, 1)), 100.0)
        .expect("accepted");
    assert!(!view.stale);
    // A reading behind the last one the tracker acted on is refused:
    // the newer report is not accepted and the view is stale with a
    // NaN age, never a negative one.
    let view = tracker
        .observe(Some(&report(1, 2, 2)), 50.0)
        .expect("view stands");
    assert_eq!(view.arm_state, 1);
    assert!(view.age_ms.is_nan());
    assert!(view.stale);
    for now_ms in [f64::NAN, -1.0, f64::INFINITY] {
        let view = tracker.view(now_ms).expect("view stands");
        assert!(view.age_ms.is_nan());
        assert!(view.stale);
    }
    let view = tracker.view(200.0).expect("view stands");
    assert_eq!(view.age_ms, 100.0);
    assert!(!view.stale);
}
