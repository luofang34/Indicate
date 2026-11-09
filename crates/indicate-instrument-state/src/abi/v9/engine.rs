//! Payload codec for the engine group.
//!
//! Layout (payload-relative offsets, LE):
//!
//! | group | layout | len |
//! |-------|--------|----:|
//! | engine | count u8; 0×3; per engine ×4: manifold pressure hPa f32, rpm f32; age f32 | 40 |
//!
//! Every engine slot is on the wire whatever the count, so the layout
//! is fixed and a future field appends after the trailing age like the
//! other stamped groups.

use super::stamped::{absent, sized};
use super::{AbiError, get_f32, get_u8, put_f32, put_u8};
use crate::abi::{opt, or_nan};
use crate::aircraft::{AircraftState, Stamped};
use crate::engine::{EngineIndication, EngineSample, MAX_ENGINES};

/// Minimum engine payload length.
pub(super) const ENGINE_LEN: usize = 4 + MAX_ENGINES * 8 + 4;
const AGE_AT: usize = 4 + MAX_ENGINES * 8;

pub(super) fn decode_engine(state: &mut AircraftState, p: &[u8]) {
    let age = opt(get_f32(p, AGE_AT));
    state.engine = Stamped {
        data: age.map(|_| {
            let mut engines = [EngineSample::default(); MAX_ENGINES];
            for (i, slot) in engines.iter_mut().enumerate() {
                let at = 4 + i * 8;
                *slot = EngineSample {
                    manifold_pressure_hpa: opt(get_f32(p, at)),
                    rpm: opt(get_f32(p, at + 4)),
                };
            }
            EngineIndication {
                count: get_u8(p, 0),
                engines,
            }
        }),
        age_ms: age,
    };
}

pub(super) fn encode_engine(
    state: &AircraftState,
    out: &mut [u8],
) -> Result<Option<usize>, AbiError> {
    if absent(&state.engine) {
        return Ok(None);
    }
    let p = sized(out, ENGINE_LEN)?;
    let indication = state.engine.data.unwrap_or_default();
    p.fill(0);
    put_u8(p, 0, indication.count);
    for (i, engine) in indication.engines.iter().enumerate() {
        let at = 4 + i * 8;
        put_f32(p, at, or_nan(engine.manifold_pressure_hpa));
        put_f32(p, at + 4, or_nan(engine.rpm));
    }
    put_f32(p, AGE_AT, or_nan(state.engine.age_ms));
    Ok(Some(ENGINE_LEN))
}
