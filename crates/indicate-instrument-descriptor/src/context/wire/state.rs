//! The state section of the draw-context wire: the host's raw state,
//! or one synthesized from resolved data, with the record groups beside it.

use indicate_instrument_state::{
    AircraftState, BesideSamples,
    abi::v9::{self, AbiError},
};

use super::super::DrawContext;
use super::super::synth::synthesize_aircraft_state;
use super::DrawContextWireError;

/// Encodes the context's state section at `*off`, advancing it, and
/// returns the section length.
pub(super) fn encode_state_section(
    ctx: &DrawContext<'_>,
    out: &mut [u8],
    off: &mut usize,
) -> Result<u16, DrawContextWireError> {
    let beside = BesideSamples {
        engines: *ctx.engines(),
        tanks: *ctx.tanks(),
    };
    if let Some(state) = ctx.raw_state() {
        return encode_frame(state, &beside, out, off);
    }
    let mut synthesized = synthesize_aircraft_state(ctx.data());
    // Resolved data carries each group's status, not its records: a
    // header counts the records the context actually carries.
    let count = |n: usize| u16::try_from(n).map_err(|_| DrawContextWireError::LengthOverflow);
    if let Some(group) = synthesized.engine.data.as_mut() {
        group.count = count(ctx.engines().len())?;
    }
    if let Some(group) = synthesized.fuel_tanks.data.as_mut() {
        group.count = count(ctx.tanks().len())?;
    }
    encode_frame(&synthesized, &beside, out, off)
}

fn encode_frame(
    state: &AircraftState,
    beside: &BesideSamples<'_>,
    out: &mut [u8],
    off: &mut usize,
) -> Result<u16, DrawContextWireError> {
    let avail = out.len();
    let tail = out
        .get_mut(*off..)
        .ok_or(DrawContextWireError::BufferTooSmall {
            needed: *off + 2,
            available: avail,
        })?;
    let written = v9::encode_state_samples(state, beside, tail).map_err(|err| match err {
        AbiError::Truncated => DrawContextWireError::BufferTooSmall {
            needed: avail.saturating_add(64),
            available: avail,
        },
        other => DrawContextWireError::State(other),
    })?;
    *off += written;
    u16::try_from(written).map_err(|_| DrawContextWireError::LengthOverflow)
}
