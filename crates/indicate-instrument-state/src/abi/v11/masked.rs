//! Payload codec for the presence-masked groups: engines (0x0E) and
//! fuel tanks (0x18).
//!
//! Layout (payload-relative offsets, LE):
//!
//! | offset | field |
//! |-------:|-------|
//! | 0 | count u16: records that follow |
//! | 2 | reserved u16, zero |
//! | 4 | age f32 (NaN = never) |
//! | 8 | `count` records, packed back to back |
//!
//! Each record is a presence mask u16 followed by one f32 per set bit,
//! in bit order, from the group's append-only field registry
//! ([`crate::EngineField`], [`crate::TankField`]). A record therefore
//! sizes itself, so the payload needs no stride and no per-record length
//! and still validates exactly: its length must equal 8 plus the sum of
//! its records' sizes. A shorter or longer payload, a reserved word that
//! is not zero, or a mask bit outside the registry faults the group
//! ([`crate::GroupFault::MalformedPayload`]).
//!
//! The group grows by registering fields, not by appending after the
//! age as the fixed-layout stamped groups do: the records are the tail.
//! A new field is a new ABI version, because an older decoder cannot
//! size a record that carries a bit it does not know.
//!
//! Bound: a group's payload length is a u16, so a frame carries at most
//! (65,535 - 8) / 2 = 32,763 records with no fields. The count's u16
//! covers that.

use super::stamped::{absent, sized};
use super::{AbiError, get_f32, get_u16, put_f32, put_u16};
use crate::abi::{opt, or_nan};
use crate::aircraft::Stamped;
use crate::samples::{Sample, SampleGroup, Samples, encode_raw};
use crate::validate::GroupFault;

/// Header length: the minimum payload, for zero records.
pub(super) const MASKED_HEADER_LEN: usize = 8;

/// Decodes a masked group's header, checking its records.
pub(super) fn decode_masked<S: Sample>(p: &[u8]) -> Stamped<SampleGroup> {
    let age = opt(get_f32(p, 4));
    let count = get_u16(p, 0);
    let fault = if get_u16(p, 2) != 0 {
        Some(GroupFault::MalformedPayload)
    } else {
        Samples::<S>::parse(p.get(MASKED_HEADER_LEN..).unwrap_or(&[]), count).err()
    };
    Stamped {
        data: age.map(|_| SampleGroup { count, fault }),
        age_ms: age,
    }
}

/// The records of a payload whose header the decoder accepted; empty
/// when the payload is malformed, so a faulted group exposes nothing.
pub(super) fn samples_of<S: Sample + 'static>(p: &[u8]) -> Samples<'_, S> {
    if get_u16(p, 2) != 0 {
        return Samples::EMPTY;
    }
    Samples::parse(p.get(MASKED_HEADER_LEN..).unwrap_or(&[]), get_u16(p, 0))
        .unwrap_or(Samples::EMPTY)
}

/// Why a masked group could not encode.
pub(super) enum MaskedEncodeError {
    /// The header's count and the records disagree.
    Count { declared: u16, supplied: usize },
    /// The output buffer is too short.
    Truncated,
}

impl From<AbiError> for MaskedEncodeError {
    fn from(_: AbiError) -> Self {
        Self::Truncated
    }
}

/// Encodes a masked group from its header and the records beside it.
pub(super) fn encode_masked<S: Sample>(
    group: &Stamped<SampleGroup>,
    records: &Samples<'_, S>,
    out: &mut [u8],
) -> Result<Option<usize>, MaskedEncodeError> {
    if absent(group) {
        return if records.is_empty() {
            Ok(None)
        } else {
            Err(MaskedEncodeError::Count {
                declared: 0,
                supplied: records.len(),
            })
        };
    }
    let declared = group.data.map_or(0, |g| g.count);
    if usize::from(declared) != records.len() {
        return Err(MaskedEncodeError::Count {
            declared,
            supplied: records.len(),
        });
    }
    let header = sized(out, MASKED_HEADER_LEN)?;
    header.fill(0);
    put_u16(header, 0, declared);
    put_f32(header, 4, or_nan(group.age_ms));
    let mut at = MASKED_HEADER_LEN;
    for record in records.iter() {
        let tail = out.get_mut(at..).ok_or(MaskedEncodeError::Truncated)?;
        at += encode_raw(&record.to_raw(), tail).ok_or(MaskedEncodeError::Truncated)?;
    }
    Ok(Some(at))
}
