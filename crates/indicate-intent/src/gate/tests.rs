//! Admission refuses by identity, order, clock, and bounds, and a refusal
//! leaves the gate unchanged.

#![allow(clippy::expect_used, clippy::panic)]

use indicate_alerts::AlertId;
use indicate_instrument_state::{
    AltitudeClass, GeoidModelId, HeadingReference, NavSource, OriginId,
};

use super::IntentGate;
use crate::error::IntentFault;
use crate::request::{Intent, IntentStamp, PageId, Request};

fn request(device_id: u32, sequence: u32, intent: Intent) -> Request {
    Request {
        stamp: IntentStamp {
            device_id,
            sequence,
            issued_at_ms: 0,
        },
        intent,
    }
}

fn baro(hpa: f32) -> Intent {
    Intent::SelectBaro { hpa }
}

#[test]
fn the_first_device_is_pinned_and_others_are_refused() {
    let mut gate = IntentGate::new();
    let admitted = gate
        .admit(&request(7, 1, baro(1013.25)), 0.0)
        .expect("admitted");
    assert_eq!(admitted.admitted_at_ms, 0.0);
    assert_eq!(
        gate.admit(&request(8, 2, baro(1013.25)), 1.0),
        Err(IntentFault::WrongDevice {
            pinned: 7,
            found: 8
        })
    );
    let mut pinned = IntentGate::pinned(9);
    assert_eq!(
        pinned.admit(&request(7, 1, baro(1013.25)), 0.0),
        Err(IntentFault::WrongDevice {
            pinned: 9,
            found: 7
        })
    );
}

#[test]
fn sequence_must_advance_in_wrapping_serial_order() {
    let mut gate = IntentGate::new();
    gate.admit(&request(7, u32::MAX, baro(1013.0)), 0.0)
        .expect("admitted");
    // The wrap from u32::MAX to 0 is a normal single advance.
    gate.admit(&request(7, 0, baro(1013.0)), 1.0)
        .expect("wrapped");
    assert_eq!(
        gate.admit(&request(7, 0, baro(1013.0)), 2.0),
        Err(IntentFault::Replay {
            current: 0,
            found: 0
        })
    );
    assert_eq!(
        gate.admit(&request(7, u32::MAX, baro(1013.0)), 3.0),
        Err(IntentFault::Replay {
            current: 0,
            found: u32::MAX
        })
    );
    gate.admit(&request(7, 1, baro(1013.0)), 4.0).expect("next");
}

#[test]
fn an_untrusted_host_clock_refuses_without_advancing_the_gate() {
    let mut gate = IntentGate::new();
    gate.admit(&request(7, 1, baro(1013.0)), 100.0)
        .expect("admitted");
    for now_ms in [f64::NAN, -1.0, 50.0, f64::INFINITY] {
        assert_eq!(
            gate.admit(&request(7, 2, baro(1013.0)), now_ms),
            Err(IntentFault::InvalidClock)
        );
    }
    // Sequence 2 was never admitted, so it is still the next one.
    gate.admit(&request(7, 2, baro(1013.0)), 100.0)
        .expect("still next");
}

#[test]
fn values_must_be_finite_and_inside_their_bounds() {
    let mut gate = IntentGate::new();
    assert_eq!(
        gate.admit(&request(7, 1, baro(f32::NAN)), 0.0),
        Err(IntentFault::NonFinite {
            what: "barometric setting"
        })
    );
    assert_eq!(
        gate.admit(&request(7, 1, baro(700.0)), 0.0),
        Err(IntentFault::OutOfRange {
            what: "barometric setting"
        })
    );
    let bug = |rad: f32| Intent::SetHeadingBug {
        rad,
        reference: HeadingReference::Magnetic,
    };
    assert_eq!(
        gate.admit(&request(7, 1, bug(-0.1)), 0.0),
        Err(IntentFault::OutOfRange {
            what: "heading bug"
        })
    );
    assert_eq!(
        gate.admit(&request(7, 1, bug(core::f32::consts::TAU)), 0.0),
        Err(IntentFault::OutOfRange {
            what: "heading bug"
        })
    );
    let altitude = |meters: f32| Intent::SelectAltitude {
        meters,
        class: AltitudeClass::BaroIndicated,
        origin: OriginId(0),
        model: GeoidModelId(0),
    };
    assert_eq!(
        gate.admit(&request(7, 1, altitude(40_000.0)), 0.0),
        Err(IntentFault::OutOfRange {
            what: "selected altitude"
        })
    );
    // A refused request never advanced the sequence.
    gate.admit(&request(7, 1, altitude(3_048.0)), 0.0)
        .expect("in range");
}

#[test]
fn a_reference_the_state_cannot_name_is_refused() {
    let mut gate = IntentGate::new();
    assert_eq!(
        gate.admit(
            &request(
                7,
                1,
                Intent::SetHeadingBug {
                    rad: 1.0,
                    reference: HeadingReference::Unknown
                }
            ),
            0.0
        ),
        Err(IntentFault::UnknownReference {
            what: "heading reference"
        })
    );
    assert_eq!(
        gate.admit(
            &request(
                7,
                1,
                Intent::SelectAltitude {
                    meters: 100.0,
                    class: AltitudeClass::Unknown,
                    origin: OriginId(0),
                    model: GeoidModelId(0),
                }
            ),
            0.0
        ),
        Err(IntentFault::UnknownReference {
            what: "altitude class"
        })
    );
    assert_eq!(
        gate.admit(
            &request(7, 1, Intent::SelectNavSource(NavSource::Unknown)),
            0.0
        ),
        Err(IntentFault::UnknownReference {
            what: "navigation source"
        })
    );
}

#[test]
fn display_and_alert_intents_carry_no_value_to_bound() {
    let mut gate = IntentGate::new();
    for (sequence, intent) in [
        (1, Intent::AcknowledgeAlert(AlertId(0x0501))),
        (2, Intent::AcknowledgeAllAlerts),
        (3, Intent::SelectPage(PageId(2))),
        (4, Intent::Declutter(true)),
        (5, Intent::SelectNavSource(NavSource::Nav1)),
    ] {
        gate.admit(&request(7, sequence, intent), f64::from(sequence))
            .expect("admitted");
    }
}
