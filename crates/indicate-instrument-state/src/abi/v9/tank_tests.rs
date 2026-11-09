//! Fuel tank group codec behavior: masks, exact length, and counts.

use std::vec;

use super::fixtures;
use super::tests::locate_payload;
use super::{
    AbiError, CAPACITY, decode_state, encode_state_samples, encode_state_with, tank_samples,
};
use crate::samples::BesideSamples;
use crate::tank::{TankGroup, TankSamples};
use crate::validate::{GroupFault, validate_state};

#[test]
fn tanks_round_trip_by_index_with_only_their_own_fields() {
    let state = fixtures::full();
    let mut buf = vec![0u8; CAPACITY];
    let len =
        encode_state_samples(&state, &fixtures::beside_samples(&state), &mut buf).expect("fits");
    let frame = &buf[..len];
    let at = locate_payload(frame, 0x18).expect("tank group present");
    let plen = usize::from(u16::from_le_bytes([frame[at - 2], frame[at - 1]]));
    // Header 8, then 2 + 4 + 4 (volume and capacity) and 2 + 4 (mass).
    assert_eq!(plen, 8 + 10 + 6);
    assert_eq!(u16::from_le_bytes([frame[at + 8], frame[at + 9]]), 0b0011);
    assert_eq!(u16::from_le_bytes([frame[at + 18], frame[at + 19]]), 0b0100);
    let report = decode_state(frame).expect("decodes");
    assert_eq!(
        report.state.fuel_tanks.data,
        Some(TankGroup {
            count: 2,
            fault: None
        })
    );
    let tanks = tank_samples(frame);
    assert!(matches!(tanks, TankSamples::Wire { .. }), "zero-copy");
    assert_eq!(tanks.iter().collect::<std::vec::Vec<_>>(), fixtures::TANKS);
    assert_eq!(tanks.get(1).and_then(|t| t.quantity_kg), Some(180.5));
    assert_eq!(tanks.get(1).and_then(|t| t.quantity_l), None);
}

#[test]
fn a_bad_tank_payload_faults_the_group_and_exposes_nothing() {
    let state = fixtures::full();
    let mut buf = vec![0u8; CAPACITY];
    let len =
        encode_state_samples(&state, &fixtures::beside_samples(&state), &mut buf).expect("fits");
    let at = locate_payload(&buf[..len], 0x18).expect("tank group present");
    for (offset, value, fault) in [
        (at + 8, 0b1_0000u8, GroupFault::MalformedPayload),
        (at + 2, 1, GroupFault::MalformedPayload),
    ] {
        let mut bad = buf[..len].to_vec();
        bad[offset] = value;
        let report = decode_state(&bad).expect("the frame still decodes");
        assert_eq!(validate_state(&report.state).fuel_tanks, Some(fault));
        assert!(tank_samples(&bad).is_empty());
    }
    let mut negative = buf[..len].to_vec();
    negative[at + 10..at + 14].copy_from_slice(&(-1.0f32).to_le_bytes());
    let report = decode_state(&negative).expect("decodes");
    assert_eq!(
        validate_state(&report.state).fuel_tanks,
        Some(GroupFault::NonFinite)
    );
}

#[test]
fn the_encoder_refuses_tanks_that_disagree_with_the_header() {
    let state = fixtures::full();
    let mut buf = vec![0u8; CAPACITY];
    let beside = BesideSamples {
        tanks: TankSamples::EMPTY,
        ..fixtures::beside_samples(&state)
    };
    assert_eq!(
        encode_state_samples(&state, &beside, &mut buf),
        Err(AbiError::TankCountMismatch {
            declared: 2,
            supplied: 0
        })
    );
    // The engine-only encoder still serves a state with no tank group.
    let mut no_tanks = state;
    no_tanks.fuel_tanks = Default::default();
    encode_state_with(&no_tanks, &fixtures::engine_samples(&no_tanks), &mut buf)
        .expect("a state without tanks encodes with engines alone");
}
