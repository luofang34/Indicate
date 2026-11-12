//! The nav group carries every flag, scale, facility and sequencing
//! state a guidance source can declare across the wire unchanged, so a
//! FROM leg, an enroute scale, a localizer or a hold reaches the
//! displays as the source stated it. Each declared byte is part of the
//! group's minimum, so a producer that omits one is refused.

#![allow(clippy::expect_used, clippy::panic)]

use super::decode_state;
use super::tests::{encode, locate_payload};
use crate::aircraft::{NavFacility, NavFromTo, NavScale, NavSequencing, NavSource};
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

/// The scale, facility and sequencing bytes each append after the
/// group's previous tail, so a producer that stamps this version and
/// stops short emits a payload below the group's minimum and the frame
/// is refused. Reusing the reserved padding byte instead would have made
/// an undeclared value decode as the zero value: the widest scale, no
/// facility, or sequencing.
#[test]
fn a_nav_payload_short_of_any_declared_byte_is_refused() {
    let frame = encode(&super::fixtures::full());
    let payload = locate_payload(&frame, 0x04).expect("nav present");
    let length_at = payload - 2;
    assert_eq!(frame[length_at], 45, "the nav group is 45 bytes");
    for keep in [42u8, 43, 44] {
        let mut short = frame.clone();
        for _ in keep..45 {
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
