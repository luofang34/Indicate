//! Golden-frame round trips through the state ABI, from the generator's
//! side of the boundary.

#![allow(clippy::expect_used, clippy::panic)]

use indicate_instrument_state::abi::v8::{CAPACITY, decode_state, encode_state, fixtures};

#[test]
fn a_canonical_frame_reports_no_trailing_bytes() {
    let state = fixtures::full();
    let mut buf = [0u8; CAPACITY];
    let len = encode_state(&state, &mut buf).expect("fits");
    let report = decode_state(&buf[..len]).expect("decodes");
    assert_eq!(report.trailing_bytes, 0);
    assert_eq!(report.state, state);
}

#[test]
fn bytes_past_the_last_announced_group_are_counted_and_never_read() {
    let state = fixtures::full();
    let mut buf = [0u8; CAPACITY];
    let len = encode_state(&state, &mut buf).expect("fits");
    // Three bytes a producer wrote but did not announce; a tag-shaped
    // tail must not be interpreted as a group.
    buf[len] = 0x01;
    buf[len + 1] = 0xFF;
    buf[len + 2] = 0xFF;
    let report = decode_state(&buf[..len + 3]).expect("decodes");
    assert_eq!(report.trailing_bytes, 3);
    assert_eq!(report.unknown_groups, 0);
    assert_eq!(report.state, state);
}
