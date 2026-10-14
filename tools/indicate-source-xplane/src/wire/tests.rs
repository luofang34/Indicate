//! The packet shapes, byte for byte, and the refusals.

#![allow(clippy::expect_used, clippy::panic)]

use super::{
    DREF_WRITE_LEN, RREF_REQUEST_LEN, RrefRecord, decode_rref_reply, encode_dref_write,
    encode_rref_request,
};
use crate::error::WireError;

#[test]
fn an_rref_request_is_header_frequency_index_and_a_padded_path() {
    let mut out = [0xAAu8; RREF_REQUEST_LEN];
    encode_rref_request(&mut out, 20, 7, "sim/flightmodel/position/theta").expect("fits");
    assert_eq!(&out[..5], b"RREF\0");
    assert_eq!(i32::from_le_bytes([out[5], out[6], out[7], out[8]]), 20);
    assert_eq!(i32::from_le_bytes([out[9], out[10], out[11], out[12]]), 7);
    assert_eq!(&out[13..13 + 30], b"sim/flightmodel/position/theta");
    assert!(out[13 + 30..].iter().all(|byte| *byte == 0));
}

#[test]
fn a_dref_write_is_header_value_and_a_padded_path() {
    let mut out = [0xAAu8; DREF_WRITE_LEN];
    encode_dref_write(&mut out, 29.92, "sim/cockpit/misc/barometer_setting").expect("fits");
    assert_eq!(&out[..5], b"DREF\0");
    assert_eq!(f32::from_le_bytes([out[5], out[6], out[7], out[8]]), 29.92);
    assert_eq!(&out[9..9 + 34], b"sim/cockpit/misc/barometer_setting");
    assert!(out[9 + 34..].iter().all(|byte| *byte == 0));
}

#[test]
fn a_path_that_cannot_fit_or_cannot_be_carried_is_refused() {
    let mut out = [0u8; RREF_REQUEST_LEN];
    let long = "x".repeat(400);
    assert_eq!(
        encode_rref_request(&mut out, 1, 0, &long),
        Err(WireError::DatarefTooLong { len: 400, max: 399 })
    );
    assert_eq!(
        encode_rref_request(&mut out, 1, 0, "sim/\u{e9}"),
        Err(WireError::DatarefNotAscii)
    );
    assert_eq!(
        encode_rref_request(&mut out, 1, 0, "sim/a\0b"),
        Err(WireError::DatarefNotAscii)
    );
}

fn reply(version: u8, records: &[(i32, f32)]) -> Vec<u8> {
    let mut packet = b"RREF".to_vec();
    packet.push(version);
    for (index, value) in records {
        packet.extend_from_slice(&index.to_le_bytes());
        packet.extend_from_slice(&value.to_le_bytes());
    }
    packet
}

#[test]
fn a_reply_decodes_its_records_under_either_version_byte() {
    for version in *b",O" {
        let packet = reply(version, &[(0, 1.5), (21, 42.25)]);
        let records: Vec<RrefRecord> = decode_rref_reply(&packet).expect("reply").collect();
        assert_eq!(
            records,
            [
                RrefRecord {
                    index: 0,
                    value: 1.5
                },
                RrefRecord {
                    index: 21,
                    value: 42.25
                }
            ]
        );
    }
    assert_eq!(
        decode_rref_reply(&reply(b',', &[])).expect("empty").count(),
        0
    );
}

#[test]
fn a_packet_that_is_not_a_reply_or_ends_inside_a_record_is_refused_whole() {
    assert_eq!(decode_rref_reply(b"DATA,").err(), Some(WireError::NotRref));
    assert_eq!(decode_rref_reply(b"RRE").err(), Some(WireError::NotRref));
    let mut packet = reply(b',', &[(0, 1.0)]);
    packet.extend_from_slice(&[1, 2, 3]);
    assert_eq!(
        decode_rref_reply(&packet).err(),
        Some(WireError::TruncatedRecord { trailing: 3 })
    );
}
