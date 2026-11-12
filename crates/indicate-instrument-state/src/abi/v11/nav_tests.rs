//! The nav group carries every flag, scale, facility, sequencing state
//! and position declaration a guidance source can make across the wire
//! unchanged, so a FROM leg, an enroute scale, a localizer, a hold or a
//! camera position reaches the displays as the source stated it. Each declared byte is part of the
//! group's minimum, so a producer that omits one is refused.

#![allow(clippy::expect_used, clippy::panic)]

use super::decode_state;
use super::tests::{encode, locate_payload};
use crate::aircraft::{
    NavFacility, NavFromTo, NavScale, NavSequencing, NavSource, PositionIntegrity, PositionSource,
};
use crate::validate::{GroupFault, validate_state};

#[test]
fn every_known_flag_and_scale_round_trips_the_nav_group() {
    let mut state = super::fixtures::full();
    for fromto in [NavFromTo::Off, NavFromTo::To, NavFromTo::From] {
        for scale in [NavScale::Enroute, NavScale::Terminal, NavScale::Approach] {
            let nav = state.nav.data.as_mut().expect("fixture has nav");
            nav.fromto = fromto;
            nav.scale = scale;
            let decoded = decode_state(&encode(&state)).expect("decodes").state;
            let got = decoded.nav.data.expect("nav present");
            assert_eq!((got.fromto, got.scale), (fromto, scale));
            assert_eq!(decoded.nav, state.nav, "{fromto:?} {scale:?}");
        }
    }
}

#[test]
fn every_facility_and_sequencing_state_round_trips_the_nav_group() {
    let mut state = super::fixtures::full();
    for (source, facility) in [
        (NavSource::Gps, NavFacility::None),
        (NavSource::Nav1, NavFacility::Vor),
        (NavSource::Nav2, NavFacility::Localizer),
    ] {
        for sequencing in [
            NavSequencing::Sequencing,
            NavSequencing::Hold,
            NavSequencing::Suspended,
        ] {
            let nav = state.nav.data.as_mut().expect("fixture has nav");
            nav.source = source;
            nav.facility = facility;
            nav.sequencing = sequencing;
            let decoded = decode_state(&encode(&state)).expect("decodes").state;
            assert_eq!(decoded.nav, state.nav, "{facility:?} {sequencing:?}");
            assert_eq!(validate_state(&decoded).nav, None, "{facility:?}");
        }
    }
}

/// The scale, facility, sequencing and position bytes each append after
/// the group's previous tail, so a producer that stamps this version and
/// stops short emits a payload below the group's minimum and the frame
/// is refused. Reusing the reserved padding byte instead would have made
/// an undeclared value decode as the zero value: the widest scale, no
/// facility, sequencing, no position sensor, or a normal position.
#[test]
fn a_nav_payload_short_of_any_declared_byte_is_refused() {
    let frame = encode(&super::fixtures::full());
    let payload = locate_payload(&frame, 0x04).expect("nav present");
    let length_at = payload - 2;
    assert_eq!(frame[length_at], 47, "the nav group is 47 bytes");
    for keep in [42u8, 43, 44, 45, 46] {
        let mut short = frame.clone();
        for _ in keep..47 {
            short.remove(payload + usize::from(keep));
        }
        short[length_at] = keep;
        assert!(
            decode_state(&short).is_err(),
            "a {keep}-byte nav payload must be refused, not decoded with a guess"
        );
    }
}

#[test]
fn a_facility_that_cannot_belong_to_the_source_fails_the_group() {
    let mut state = super::fixtures::full();
    for (source, facility, fault) in [
        (NavSource::Gps, NavFacility::Vor, GroupFault::Contradictory),
        (
            NavSource::Gps,
            NavFacility::Localizer,
            GroupFault::Contradictory,
        ),
        (
            NavSource::Nav1,
            NavFacility::None,
            GroupFault::Contradictory,
        ),
        (NavSource::None, NavFacility::Vor, GroupFault::Contradictory),
        (
            NavSource::Nav2,
            NavFacility::Unknown,
            GroupFault::UnknownEnum,
        ),
    ] {
        let nav = state.nav.data.as_mut().expect("fixture has nav");
        nav.source = source;
        nav.facility = facility;
        assert_eq!(
            validate_state(&state).nav,
            Some(fault),
            "{source:?} {facility:?}"
        );
    }
    let nav = state.nav.data.as_mut().expect("fixture has nav");
    nav.source = NavSource::Gps;
    nav.facility = NavFacility::None;
    nav.sequencing = NavSequencing::Unknown;
    assert_eq!(validate_state(&state).nav, Some(GroupFault::UnknownEnum));
}

#[test]
fn every_source_code_round_trips_the_nav_group_and_a_pointer() {
    let mut state = super::fixtures::full();
    for (code, source) in [
        (0u8, NavSource::None),
        (1, NavSource::Gps),
        (2, NavSource::Nav1),
        (3, NavSource::Nav2),
        (4, NavSource::Fms),
        (255, NavSource::Unknown),
    ] {
        assert_eq!(source.to_u8(), code);
        assert_eq!(NavSource::from_u8(code), source);
        state.nav.data.as_mut().expect("fixture has nav").source = source;
        state
            .bearings
            .data
            .as_mut()
            .expect("fixture has pointers")
            .second
            .source = source;
        let frame = encode(&state);
        let nav = locate_payload(&frame, 0x04).expect("nav present");
        let pointers = locate_payload(&frame, 0x12).expect("pointers present");
        assert_eq!(
            (frame[nav], frame[pointers + 8]),
            (code, code),
            "{source:?}"
        );
        let decoded = decode_state(&frame).expect("decodes").state;
        assert_eq!(decoded.nav, state.nav, "{source:?}");
        assert_eq!(decoded.bearings, state.bearings, "{source:?}");
    }
    assert_eq!(NavSource::from_u8(5), NavSource::Unknown);
}

#[test]
fn every_position_source_and_integrity_round_trips_at_its_own_offset() {
    let mut state = super::fixtures::full();
    for (code, position) in [
        (1u8, PositionSource::Gps),
        (2, PositionSource::Visual),
        (3, PositionSource::Inertial),
        (4, PositionSource::DeadReckoning),
        (5, PositionSource::Blended),
    ] {
        for (integrity_code, integrity) in [
            (0u8, PositionIntegrity::Normal),
            (1, PositionIntegrity::Degraded),
            (2, PositionIntegrity::SensorExcluded),
        ] {
            let nav = state.nav.data.as_mut().expect("fixture has nav");
            nav.source = NavSource::Fms;
            nav.facility = NavFacility::None;
            nav.position_source = position;
            nav.position_integrity = integrity;
            let frame = encode(&state);
            let payload = locate_payload(&frame, 0x04).expect("nav present");
            assert_eq!(
                (frame[payload + 45], frame[payload + 46]),
                (code, integrity_code),
                "{position:?} {integrity:?}"
            );
            let decoded = decode_state(&frame).expect("decodes").state;
            assert_eq!(decoded.nav, state.nav, "{position:?} {integrity:?}");
            assert_eq!(validate_state(&decoded).nav, None, "{position:?}");
        }
    }
    assert_eq!(PositionSource::from_u8(6), PositionSource::Unknown);
    assert_eq!(PositionIntegrity::from_u8(3), PositionIntegrity::Unknown);
}

fn fault_of(
    source: NavSource,
    facility: NavFacility,
    position: PositionSource,
    integrity: PositionIntegrity,
) -> Option<GroupFault> {
    let mut state = super::fixtures::full();
    let nav = state.nav.data.as_mut().expect("fixture has nav");
    nav.source = source;
    nav.facility = facility;
    nav.position_source = position;
    nav.position_integrity = integrity;
    validate_state(&state).nav
}

/// A flight-management lane names its position sensor, and takes no
/// ground facility.
#[test]
fn a_flight_management_lane_must_name_its_position_sensor() {
    use NavFacility::{None as NoFacility, Vor};
    use NavSource::Fms;
    use PositionIntegrity::Normal;
    let contradictory = Some(GroupFault::Contradictory);
    let unknown = Some(GroupFault::UnknownEnum);
    assert_eq!(
        fault_of(Fms, NoFacility, PositionSource::Visual, Normal),
        None
    );
    assert_eq!(
        fault_of(Fms, NoFacility, PositionSource::None, Normal),
        contradictory
    );
    assert_eq!(
        fault_of(Fms, Vor, PositionSource::Gps, Normal),
        contradictory
    );
    assert_eq!(
        fault_of(Fms, NoFacility, PositionSource::Unknown, Normal),
        unknown
    );
    let integrity = PositionIntegrity::Unknown;
    assert_eq!(
        fault_of(Fms, NoFacility, PositionSource::Gps, integrity),
        unknown
    );
}

/// A receiver is its own sensor, so it names none, and only a named
/// sensor has an integrity to report.
#[test]
fn a_receiver_names_no_position_sensor_and_reports_no_integrity() {
    use NavFacility::{None as NoFacility, Vor};
    use NavSource::{Gps, Nav1};
    use PositionIntegrity::{Degraded, Normal, SensorExcluded};
    let contradictory = Some(GroupFault::Contradictory);
    assert_eq!(
        fault_of(Gps, NoFacility, PositionSource::None, Normal),
        None
    );
    assert_eq!(
        fault_of(Gps, NoFacility, PositionSource::Gps, Normal),
        contradictory
    );
    assert_eq!(
        fault_of(Gps, NoFacility, PositionSource::None, Degraded),
        contradictory
    );
    assert_eq!(
        fault_of(Nav1, Vor, PositionSource::Visual, Normal),
        contradictory
    );
    assert_eq!(
        fault_of(Nav1, Vor, PositionSource::None, SensorExcluded),
        contradictory
    );
    let none = NavSource::None;
    assert_eq!(
        fault_of(none, NoFacility, PositionSource::Inertial, Normal),
        contradictory
    );
}
