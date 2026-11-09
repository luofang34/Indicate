//! Engine group codec behavior: presence masks, exact length
//! validation, and engine counts with no capacity.

use std::vec;
use std::vec::Vec;

use super::fixtures;
use super::tests::locate_payload;
use super::{AbiError, CAPACITY, decode_state, encode_state, encode_state_with, engine_samples};
use crate::engine::{EngineGroup, EngineSample, EngineSamples};
use crate::signal::{FreshnessPolicy, SignalStatus};
use crate::validate::{GroupFault, validate_state};

fn encode_full() -> Vec<u8> {
    let state = fixtures::full();
    let mut buf = vec![0u8; CAPACITY];
    let len = encode_state_with(&state, &fixtures::engine_samples(&state), &mut buf)
        .expect("fixture fits");
    buf.truncate(len);
    buf
}

/// The engine payload's offset and length in `frame`.
fn engine_payload(frame: &[u8]) -> (usize, usize) {
    let at = locate_payload(frame, 0x0E).expect("engine group present");
    let len = usize::from(u16::from_le_bytes([frame[at - 2], frame[at - 1]]));
    (at, len)
}

/// `frame` with its engine payload replaced by `payload`.
fn with_engine_payload(frame: &[u8], payload: &[u8]) -> Vec<u8> {
    let (at, len) = engine_payload(frame);
    let mut out = frame[..at - 2].to_vec();
    out.extend_from_slice(&(payload.len() as u16).to_le_bytes());
    out.extend_from_slice(payload);
    out.extend_from_slice(&frame[at + len..]);
    out
}

#[test]
fn heterogeneous_engines_carry_only_their_own_fields() {
    let frame = encode_full();
    let (at, len) = engine_payload(&frame);
    let p = &frame[at..at + len];
    // Header 8, then 2 + 4 + 4, 2 + 4, and a bare 2-byte mask.
    assert_eq!(len, 8 + 10 + 6 + 2);
    assert_eq!(u16::from_le_bytes([p[0], p[1]]), 3, "count");
    assert_eq!(
        u16::from_le_bytes([p[8], p[9]]),
        0b11,
        "piston: MAP and rpm"
    );
    assert_eq!(u16::from_le_bytes([p[18], p[19]]), 0b10, "speed only");
    assert_eq!(u16::from_le_bytes([p[24], p[25]]), 0b00, "no indication");

    let report = decode_state(&frame).expect("decodes");
    assert_eq!(report.extended_groups, 0, "the samples are not a tail");
    let header = report.state.engine.data.expect("engine header");
    assert_eq!(
        header,
        EngineGroup {
            count: 3,
            fault: None
        }
    );
    let samples = engine_samples(&frame);
    assert!(matches!(samples, EngineSamples::Wire { .. }), "zero-copy");
    assert_eq!(samples.iter().collect::<Vec<_>>(), fixtures::ENGINES);
    assert_eq!(samples.get(1).and_then(|e| e.manifold_pressure_hpa), None);
    assert_eq!(samples.get(1).and_then(|e| e.rpm), Some(2310.0));
    assert_eq!(samples.get(3), None);
}

fn assert_faulted(frame: &[u8], fault: GroupFault) {
    let report = decode_state(frame).expect("the frame still decodes");
    let header = report.state.engine.data.expect("engine header");
    assert_eq!(header.fault, Some(fault));
    assert_eq!(validate_state(&report.state).engine, Some(fault));
    assert!(
        engine_samples(frame).is_empty(),
        "a faulted group exposes no readings"
    );
    let data = crate::resolve(&report.state, &FreshnessPolicy::default());
    assert_eq!(data.engine.status, SignalStatus::Failed);
}

#[test]
fn a_length_that_disagrees_with_the_masks_faults_the_group() {
    let frame = encode_full();
    let (at, len) = engine_payload(&frame);
    let p = frame[at..at + len].to_vec();
    let mut longer = p.clone();
    longer.push(0);
    assert_faulted(
        &with_engine_payload(&frame, &longer),
        GroupFault::MalformedPayload,
    );
    assert_faulted(
        &with_engine_payload(&frame, &p[..len - 1]),
        GroupFault::MalformedPayload,
    );
    let mut overcount = p.clone();
    overcount[0] = 4;
    assert_faulted(
        &with_engine_payload(&frame, &overcount),
        GroupFault::MalformedPayload,
    );
    let mut undercount = p;
    undercount[0] = 2;
    assert_faulted(
        &with_engine_payload(&frame, &undercount),
        GroupFault::MalformedPayload,
    );
}

#[test]
fn an_unregistered_bit_or_reserved_word_faults_the_group() {
    let frame = encode_full();
    let (at, len) = engine_payload(&frame);
    let mut unknown = frame[at..at + len].to_vec();
    // The bare third sample claims bit 2, which no field owns.
    unknown[24] = 0b100;
    assert_faulted(
        &with_engine_payload(&frame, &unknown),
        GroupFault::MalformedPayload,
    );
    let mut reserved = frame[at..at + len].to_vec();
    reserved[2] = 1;
    assert_faulted(
        &with_engine_payload(&frame, &reserved),
        GroupFault::MalformedPayload,
    );
}

#[test]
fn a_present_reading_that_is_not_a_number_faults_the_group() {
    let frame = encode_full();
    let (at, len) = engine_payload(&frame);
    let mut bad = frame[at..at + len].to_vec();
    bad[10..14].copy_from_slice(&f32::NAN.to_le_bytes());
    assert_faulted(&with_engine_payload(&frame, &bad), GroupFault::NonFinite);
}

#[test]
fn the_encoder_refuses_samples_that_disagree_with_the_header() {
    let state = fixtures::full();
    let mut buf = vec![0u8; CAPACITY];
    assert_eq!(
        encode_state(&state, &mut buf),
        Err(AbiError::EngineCountMismatch {
            declared: 3,
            supplied: 0
        })
    );
    let headless = crate::AircraftState::default();
    assert_eq!(
        encode_state_with(
            &headless,
            &EngineSamples::Slice(fixtures::ENGINES),
            &mut buf
        ),
        Err(AbiError::EngineCountMismatch {
            declared: 0,
            supplied: 3
        })
    );
}

/// No capacity bounds the engine count: a 33-engine stage fits the
/// ordinary buffer, and 6,000 engines fit a buffer sized for them.
#[test]
fn engine_counts_have_no_capacity() {
    for count in [33usize, 6000] {
        let samples: Vec<EngineSample> = (0..count)
            .map(|k| EngineSample {
                manifold_pressure_hpa: (k % 2 == 0).then_some(k as f32),
                rpm: Some(1000.0 + k as f32),
            })
            .collect();
        let mut state = fixtures::full();
        state.engine.data = Some(EngineGroup {
            count: count as u16,
            fault: None,
        });
        let engines = EngineSamples::Slice(&samples);
        let wire: usize = samples.iter().map(EngineSample::wire_bytes).sum();
        let mut buf = vec![0u8; CAPACITY + wire];
        let len = encode_state_with(&state, &engines, &mut buf).expect("fits its buffer");
        let frame = &buf[..len];
        let report = decode_state(frame).expect("decodes");
        assert_eq!(report.state.engine.data.map(|g| g.fault), Some(None));
        let view = engine_samples(frame);
        assert_eq!(view.len(), count);
        assert!(view.iter().eq(samples.iter().copied()));
        assert_eq!(view.get(count - 1), samples.last().copied());
    }
}
