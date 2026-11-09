//! Payload codec for the engine group: any number of engines, each
//! carrying only the fields it has.
//!
//! Layout (payload-relative offsets, LE):
//!
//! | offset | field |
//! |-------:|-------|
//! | 0 | count u16: engines that follow |
//! | 2 | reserved u16, zero |
//! | 4 | age f32 (NaN = never) |
//! | 8 | `count` samples, packed back to back |
//!
//! Each sample is a presence mask u16 followed by the fields its mask
//! names, in bit order, each at the width the
//! [`crate::EngineField`] registry gives it (every field registered
//! today is an f32). A sample therefore sizes itself, so the payload
//! needs no stride and no per-sample length, and it validates exactly:
//! its length must equal 8 plus the sum of its samples' sizes. A
//! shorter or longer payload, a reserved word that is not zero, or a
//! mask bit outside the registry faults the group
//! ([`crate::GroupFault::MalformedPayload`]).
//!
//! The group grows by registering fields, not by appending after the
//! age as the fixed-layout stamped groups do: the samples are the tail.
//! A new field is a new ABI version, because an older decoder cannot
//! size a sample that carries a bit it does not know.
//!
//! Bound: a group's payload length is a u16, so a frame carries at
//! most (65,535 - 8) / 2 = 32,763 engines with no fields, or 6,552
//! with both of today's fields. The count's u16 covers both.

use super::stamped::{absent, sized};
use super::{AbiError, get_f32, get_u16, put_f32, put_u16};
use crate::abi::{opt, or_nan};
use crate::aircraft::{AircraftState, Stamped};
use crate::engine::{EngineGroup, EngineSamples, parse_samples};
use crate::validate::GroupFault;

/// Engine header length: the minimum payload, for zero engines.
pub(super) const ENGINE_HEADER_LEN: usize = 8;

pub(super) fn decode_engine(state: &mut AircraftState, p: &[u8]) {
    let age = opt(get_f32(p, 4));
    let count = get_u16(p, 0);
    let fault = if get_u16(p, 2) != 0 {
        Some(GroupFault::MalformedPayload)
    } else {
        parse_samples(p.get(ENGINE_HEADER_LEN..).unwrap_or(&[]), count).err()
    };
    state.engine = Stamped {
        data: age.map(|_| EngineGroup { count, fault }),
        age_ms: age,
    };
}

/// The samples of an engine payload whose header the decoder accepted;
/// empty when the payload is malformed, so a faulted group exposes no
/// readings.
pub(super) fn samples_of(p: &[u8]) -> EngineSamples<'_> {
    if get_u16(p, 2) != 0 {
        return EngineSamples::EMPTY;
    }
    parse_samples(p.get(ENGINE_HEADER_LEN..).unwrap_or(&[]), get_u16(p, 0))
        .unwrap_or(EngineSamples::EMPTY)
}

pub(super) fn encode_engine(
    state: &AircraftState,
    engines: &EngineSamples<'_>,
    out: &mut [u8],
) -> Result<Option<usize>, AbiError> {
    if absent(&state.engine) {
        return if engines.is_empty() {
            Ok(None)
        } else {
            Err(AbiError::EngineCountMismatch {
                declared: 0,
                supplied: engines.len(),
            })
        };
    }
    let declared = state.engine.data.map_or(0, |group| group.count);
    if usize::from(declared) != engines.len() {
        return Err(AbiError::EngineCountMismatch {
            declared,
            supplied: engines.len(),
        });
    }
    let header = sized(out, ENGINE_HEADER_LEN)?;
    header.fill(0);
    put_u16(header, 0, declared);
    put_f32(header, 4, or_nan(state.engine.age_ms));
    let mut at = ENGINE_HEADER_LEN;
    for sample in engines.iter() {
        let tail = out.get_mut(at..).ok_or(AbiError::Truncated)?;
        at += sample.encode(tail).ok_or(AbiError::Truncated)?;
    }
    Ok(Some(at))
}
