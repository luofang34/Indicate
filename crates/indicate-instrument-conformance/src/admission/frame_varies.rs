//! Proof that a panel offering more than one frame actually uses the
//! one it is handed.
//!
//! `DrawFn` takes a `DesignFrame`, and nothing obliged a panel to read
//! it. A panel that ignored the argument and emitted fixed geometry
//! would satisfy every other check here, because the rest of the matrix
//! asks each canonical frame the same questions separately and never
//! compares the answers.
//!
//! A panel is free to ignore it — that is what a degenerate range
//! declares, and every shipped panel declares one today. What is
//! refused is declaring a range and then not varying across it: a shell
//! that asks for the larger frame is owed more instrument, not the same
//! picture stretched by the backend.

use indicate_instrument_registry::{DesignFrame, PanelDescriptor};
use indicate_instrument_scene::MAX_SCENE_BYTES;
use indicate_instrument_state::{FreshnessPolicy, resolve};

use super::error::AdmissionError;
use super::{CANONICAL_STATES, draw_scene};

/// Whether the panel declared more than one frame it can be asked for.
fn range_is_degenerate(panel: &PanelDescriptor) -> bool {
    panel.frame_min == panel.frame_max
}

/// Requires the emitted bytes to differ between the smallest and
/// largest frames a panel accepts.
///
/// Differing bytes are a weak proof of a good layout and a sufficient
/// proof of the thing that was unprovable before: that the argument
/// reached the geometry at all. A panel whose two frames encode
/// identically either ignored the parameter or drew something that does
/// not depend on it, and neither is what declaring a range claims.
pub(super) fn check_frame_varies(panel: &'static PanelDescriptor) -> Result<(), AdmissionError> {
    if range_is_degenerate(panel) {
        return Ok(());
    }
    let mut small = [0u8; MAX_SCENE_BYTES];
    let mut large = [0u8; MAX_SCENE_BYTES];
    for state in CANONICAL_STATES {
        let data = resolve(&(state.build)(), &FreshnessPolicy::default());
        let at_min = draw_bytes(panel, &data, panel.frame_min, &mut small)?;
        let at_max = draw_bytes(panel, &data, panel.frame_max, &mut large)?;
        if at_min != at_max {
            return Ok(());
        }
    }
    Err(AdmissionError::FrameIgnored {
        panel: panel.id,
        min: panel.frame_min,
        max: panel.frame_max,
    })
}

fn draw_bytes<'b>(
    panel: &PanelDescriptor,
    data: &indicate_instrument_state::PanelData,
    frame: DesignFrame,
    buf: &'b mut [u8],
) -> Result<&'b [u8], AdmissionError> {
    draw_scene(panel, data, None, frame, buf).map_err(|source| AdmissionError::Draw {
        panel: panel.id,
        state: "frame-variation",
        withheld: None,
        alerted: false,
        source,
    })
}

#[cfg(test)]
mod tests;
