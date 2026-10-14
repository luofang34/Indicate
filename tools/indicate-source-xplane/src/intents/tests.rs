//! Selections map to the simulator's own units and references, and
//! nothing else is written.

#![allow(clippy::expect_used, clippy::panic)]

use indicate_instrument_state::{AltitudeClass, GeoidModelId, HeadingReference, OriginId};
use indicate_intent::Intent;

use super::dref_for;
use crate::datarefs::Dataref;

#[test]
fn a_barometric_setting_is_written_in_inches_of_mercury() {
    let (dataref, value) = dref_for(&Intent::SelectBaro { hpa: 1013.25 }).expect("written");
    assert_eq!(dataref, Dataref::BaroInHg);
    assert!((value - 29.92).abs() < 0.01);
}

#[test]
fn a_magnetic_heading_bug_is_written_in_degrees_and_a_true_one_is_not_converted() {
    let (dataref, value) = dref_for(&Intent::SetHeadingBug {
        rad: core::f32::consts::FRAC_PI_2,
        reference: HeadingReference::Magnetic,
    })
    .expect("written");
    assert_eq!(dataref, Dataref::HeadingBugMagDeg);
    assert!((value - 90.0).abs() < 1e-4);
    assert_eq!(
        dref_for(&Intent::SetHeadingBug {
            rad: 1.0,
            reference: HeadingReference::True,
        }),
        None
    );
}

#[test]
fn a_barometric_altitude_selection_is_written_in_feet() {
    let (dataref, value) = dref_for(&Intent::SelectAltitude {
        meters: 3048.0,
        class: AltitudeClass::BaroIndicated,
        origin: OriginId(1),
        model: GeoidModelId::UNDECLARED,
    })
    .expect("written");
    assert_eq!(dataref, Dataref::AltitudeSelFt);
    assert!((value - 10_000.0).abs() < 1e-2);
}

#[test]
fn display_and_alert_intents_write_nothing() {
    assert_eq!(dref_for(&Intent::AcknowledgeAllAlerts), None);
    assert_eq!(dref_for(&Intent::Declutter(true)), None);
}
