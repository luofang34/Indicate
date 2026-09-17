//! X-Plane's UDP packet shapes, encoded and decoded without a socket.
//!
//! An `RREF` request is `RREF\0`, a little-endian `i32` frequency, a
//! little-endian `i32` index the reply echoes, and the dataref path in a
//! 400-byte NUL-padded field. A reply is a four-byte `RREF` header, one
//! version byte the simulator has spelled `,` and `O`, then eight-byte
//! records of `i32` index and `f32` value. A `DREF` write is `DREF\0`,
//! a little-endian `f32`, and the path in a 500-byte NUL-padded field.

use crate::error::WireError;

/// Bytes in an `RREF` request.
pub const RREF_REQUEST_LEN: usize = 5 + 4 + 4 + RREF_PATH_LEN;
/// Bytes in a `DREF` write.
pub const DREF_WRITE_LEN: usize = 5 + 4 + DREF_PATH_LEN;
/// The path field of an `RREF` request, NUL-padded.
pub const RREF_PATH_LEN: usize = 400;
/// The path field of a `DREF` write, NUL-padded.
pub const DREF_PATH_LEN: usize = 500;
/// Bytes in one reply record.
pub const RREF_RECORD_LEN: usize = 8;
const RREF_REPLY_HEADER_LEN: usize = 5;

/// One decoded reply record.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RrefRecord {
    /// The index the request was made under.
    pub index: i32,
    /// The dataref's value.
    pub value: f32,
}

/// Writes an `RREF` request into `out`.
pub fn encode_rref_request(
    out: &mut [u8; RREF_REQUEST_LEN],
    frequency_hz: i32,
    index: i32,
    path: &str,
) -> Result<(), WireError> {
    out.fill(0);
    out[..5].copy_from_slice(b"RREF\0");
    out[5..9].copy_from_slice(&frequency_hz.to_le_bytes());
    out[9..13].copy_from_slice(&index.to_le_bytes());
    write_path(&mut out[13..], path, RREF_PATH_LEN)
}

/// Writes a `DREF` write into `out`.
pub fn encode_dref_write(
    out: &mut [u8; DREF_WRITE_LEN],
    value: f32,
    path: &str,
) -> Result<(), WireError> {
    out.fill(0);
    out[..5].copy_from_slice(b"DREF\0");
    out[5..9].copy_from_slice(&value.to_le_bytes());
    write_path(&mut out[9..], path, DREF_PATH_LEN)
}

/// Decodes an `RREF` reply into its records. A packet that is not a
/// reply, or that ends inside a record, is refused whole.
pub fn decode_rref_reply(packet: &[u8]) -> Result<RrefRecords<'_>, WireError> {
    if packet.len() < RREF_REPLY_HEADER_LEN || &packet[..4] != b"RREF" {
        return Err(WireError::NotRref);
    }
    let body = &packet[RREF_REPLY_HEADER_LEN..];
    let trailing = body.len() % RREF_RECORD_LEN;
    if trailing != 0 {
        return Err(WireError::TruncatedRecord { trailing });
    }
    Ok(RrefRecords { body })
}

/// The records of one decoded reply, read in order.
#[derive(Debug, Clone, Copy)]
pub struct RrefRecords<'a> {
    body: &'a [u8],
}

impl Iterator for RrefRecords<'_> {
    type Item = RrefRecord;

    fn next(&mut self) -> Option<RrefRecord> {
        let (record, rest) = self.body.split_first_chunk::<RREF_RECORD_LEN>()?;
        self.body = rest;
        Some(RrefRecord {
            index: i32::from_le_bytes([record[0], record[1], record[2], record[3]]),
            value: f32::from_le_bytes([record[4], record[5], record[6], record[7]]),
        })
    }
}

/// A path is printable ASCII without NUL, and shorter than its field
/// so the terminator fits.
fn write_path(field: &mut [u8], path: &str, max: usize) -> Result<(), WireError> {
    let bytes = path.as_bytes();
    if bytes.len() >= max {
        return Err(WireError::DatarefTooLong {
            len: bytes.len(),
            max: max - 1,
        });
    }
    if !bytes.iter().all(|byte| (0x20..0x7F).contains(byte)) {
        return Err(WireError::DatarefNotAscii);
    }
    field[..bytes.len()].copy_from_slice(bytes);
    Ok(())
}

#[cfg(test)]
mod tests;
