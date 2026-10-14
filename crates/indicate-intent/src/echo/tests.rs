//! A request is confirmed only by echo, goes overdue on silence, and is
//! unmeasurable on an untrusted clock.

#![allow(clippy::expect_used, clippy::panic)]

use indicate_instrument_state::{
    AltitudeClass, GeoidModelId, HeadingReference, NavSource, OriginId, Selections,
};

use super::{EchoMonitor, EchoStatus};
use crate::gate::Admitted;
use crate::request::{Intent, IntentStamp, Request};

fn admitted(intent: Intent, admitted_at_ms: f64) -> Admitted {
    Admitted {
        request: Request {
            stamp: IntentStamp {
                device_id: 7,
                sequence: 1,
                issued_at_ms: 0,
            },
            intent,
        },
        admitted_at_ms,
    }
}

fn selections(baro_hpa: Option<f32>) -> Selections {
    Selections {
        heading_bug_rad: 0.0,
        heading_bug_reference: HeadingReference::Magnetic,
        altitude_sel_m: None,
        altitude_sel_class: AltitudeClass::BaroIndicated,
        altitude_sel_origin: OriginId(0),
        altitude_sel_model: GeoidModelId(0),
        baro_sel_hpa: baro_hpa,
    }
}

#[test]
fn a_request_is_pending_until_the_state_echoes_it_then_confirmed_once() {
    let mut monitor = EchoMonitor::new(2000.0);
    monitor.request(&admitted(Intent::SelectBaro { hpa: 1013.2 }, 100.0));
    let status = monitor.observe(&selections(Some(1020.0)), NavSource::None, 600.0);
    assert_eq!(status[0], EchoStatus::Pending { age_ms: 500.0 });
    // Within tolerance of the source's own rounding.
    let status = monitor.observe(&selections(Some(1013.22)), NavSource::None, 700.0);
    assert_eq!(status[0], EchoStatus::Confirmed);
    let status = monitor.observe(&selections(Some(1013.22)), NavSource::None, 800.0);
    assert_eq!(status[0], EchoStatus::Idle);
}

#[test]
fn silence_past_the_window_is_overdue_and_the_request_stays_outstanding() {
    let mut monitor = EchoMonitor::new(2000.0);
    monitor.request(&admitted(Intent::SelectBaro { hpa: 1013.2 }, 100.0));
    let status = monitor.observe(&selections(None), NavSource::None, 2_500.0);
    assert_eq!(status[0], EchoStatus::Overdue { age_ms: 2400.0 });
    // A late echo still confirms.
    let status = monitor.observe(&selections(Some(1013.2)), NavSource::None, 3_000.0);
    assert_eq!(status[0], EchoStatus::Confirmed);
}

#[test]
fn an_untrusted_clock_cannot_age_a_request() {
    let mut monitor = EchoMonitor::new(2000.0);
    monitor.request(&admitted(Intent::SelectBaro { hpa: 1013.2 }, 100.0));
    monitor.observe(&selections(None), NavSource::None, 200.0);
    for now_ms in [f64::NAN, 50.0, -1.0] {
        let status = monitor.observe(&selections(None), NavSource::None, now_ms);
        assert_eq!(status[0], EchoStatus::Unmeasurable);
    }
    // An echo confirms regardless of the clock.
    let status = monitor.observe(&selections(Some(1013.2)), NavSource::None, f64::NAN);
    assert_eq!(status[0], EchoStatus::Confirmed);
}

#[test]
fn identity_fields_must_match_exactly_for_an_echo() {
    let mut monitor = EchoMonitor::new(2000.0);
    monitor.request(&admitted(
        Intent::SetHeadingBug {
            rad: 1.0,
            reference: HeadingReference::True,
        },
        0.0,
    ));
    let mut echo = selections(None);
    echo.heading_bug_rad = 1.0;
    // Right number, wrong reference: not an echo.
    let status = monitor.observe(&echo, NavSource::None, 10.0);
    assert!(matches!(status[1], EchoStatus::Pending { .. }));
    echo.heading_bug_reference = HeadingReference::True;
    let status = monitor.observe(&echo, NavSource::None, 20.0);
    assert_eq!(status[1], EchoStatus::Confirmed);

    monitor.request(&admitted(Intent::SelectNavSource(NavSource::Nav2), 20.0));
    let status = monitor.observe(&echo, NavSource::Nav1, 30.0);
    assert!(matches!(status[3], EchoStatus::Pending { .. }));
    let status = monitor.observe(&echo, NavSource::Nav2, 40.0);
    assert_eq!(status[3], EchoStatus::Confirmed);
}

#[test]
fn a_newer_request_of_the_same_kind_replaces_the_outstanding_one() {
    let mut monitor = EchoMonitor::new(2000.0);
    monitor.request(&admitted(Intent::SelectBaro { hpa: 1010.0 }, 0.0));
    monitor.request(&admitted(Intent::SelectBaro { hpa: 1020.0 }, 100.0));
    // The old value echoing back does not confirm the new request.
    let status = monitor.observe(&selections(Some(1010.0)), NavSource::None, 200.0);
    assert_eq!(status[0], EchoStatus::Pending { age_ms: 100.0 });
    let status = monitor.observe(&selections(Some(1020.0)), NavSource::None, 300.0);
    assert_eq!(status[0], EchoStatus::Confirmed);
}

#[test]
fn intents_nothing_echoes_are_not_tracked() {
    let mut monitor = EchoMonitor::new(2000.0);
    monitor.request(&admitted(Intent::Declutter(true), 0.0));
    monitor.request(&admitted(Intent::AcknowledgeAllAlerts, 0.0));
    let status = monitor.observe(&selections(None), NavSource::None, 5_000.0);
    assert_eq!(status, [EchoStatus::Idle; 4]);
}
