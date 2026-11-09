//! The state section of the draw-context wire: the host's raw state,
//! or one synthesized from resolved data, with its engine samples.

use indicate_instrument_state::{
    AircraftState, EngineSamples,
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
    if let Some(state) = ctx.raw_state() {
        return encode_frame(state, ctx.engines(), out, off);
    }
    let mut synthesized = synthesize_aircraft_state(ctx.data());
    // Resolved data carries the engine status, not the samples: the
    // header counts the samples the context actually carries.
    if let Some(group) = synthesized.engine.data.as_mut() {
        group.count =
            u16::try_from(ctx.engines().len()).map_err(|_| DrawContextWireError::LengthOverflow)?;
    }
    encode_frame(&synthesized, ctx.engines(), out, off)
}

fn encode_frame(
    state: &AircraftState,
    engines: &EngineSamples<'_>,
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
    let written = v9::encode_state_with(state, engines, tail).map_err(|err| match err {
        AbiError::Truncated => DrawContextWireError::BufferTooSmall {
            needed: avail.saturating_add(64),
            available: avail,
        },
        other => DrawContextWireError::State(other),
    })?;
    *off += written;
    u16::try_from(written).map_err(|_| DrawContextWireError::LengthOverflow)
}
