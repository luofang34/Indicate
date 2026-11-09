//! Engine group codec behavior: slot order, absent readings, and the
//! capacity fault.

use super::fixtures;
use super::tests::{locate_payload, one_group_frame};
use super::{CAPACITY, decode_state, encode_state};
use crate::validate::{GroupFault, validate_state};

#[test]
fn engine_slots_decode_in_order_and_an_absent_reading_stays_absent() {
    let mut payload = [0u8; 40];
    payload[0] = 2;
    payload[4..8].copy_from_slice(&812.5f32.to_le_bytes());
    payload[8..12].copy_from_slice(&2300.0f32.to_le_bytes());
    payload[12..16].copy_from_slice(&795.0f32.to_le_bytes());
    payload[16..20].copy_from_slice(&f32::NAN.to_le_bytes());
    for slot in [20, 24, 28, 32] {
        payload[slot..slot + 4].copy_from_slice(&f32::NAN.to_le_bytes());
    }
    payload[36..40].copy_from_slice(&30.0f32.to_le_bytes());
    let report = decode_state(&one_group_frame(0x0E, &payload)).expect("decodes");
    let engines = report.state.engine.data.expect("engine decoded");
    assert_eq!(engines.count, 2);
    let first = engines.engine(0).expect("engine 1");
    assert_eq!(first.manifold_pressure_hpa, Some(812.5));
    assert_eq!(first.rpm, Some(2300.0));
    let second = engines.engine(1).expect("engine 2");
    assert_eq!(second.manifold_pressure_hpa, Some(795.0));
    assert_eq!(second.rpm, None, "a NaN reading is no reading");
    assert!(engines.engine(2).is_none(), "past the count is unreported");
    assert_eq!(report.state.engine.age_ms, Some(30.0));
    let mut frame = std::vec![0u8; CAPACITY];
    let len = encode_state(&report.state, &mut frame).expect("re-encodes");
    assert_eq!(
        locate_payload(&frame[..len], 0x0E).map(|at| &frame[at..at + 40]),
        Some(&payload[..])
    );
}

#[test]
fn an_engine_count_past_capacity_faults_the_group() {
    let mut state = fixtures::full();
    if let Some(engines) = state.engine.data.as_mut() {
        engines.count = 5;
    }
    assert_eq!(validate_state(&state).engine, Some(GroupFault::UnknownEnum));
    let data = crate::resolve(&state, &crate::FreshnessPolicy::default());
    assert_eq!(data.engine.status, crate::SignalStatus::Failed);
}
