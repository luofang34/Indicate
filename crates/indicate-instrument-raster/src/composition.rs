//! Screen composition on the reference rasterizer: several validated
//! panel scenes into one framebuffer (AIR-OUT-011).
//!
//! This is the backend contract's composition rule executed rather than
//! described. Slots paint in index order; each is clipped to its rect,
//! translated to its origin, and replayed from that panel's own scene.
//! There is one framebuffer, cleared once, and one global mapping from
//! screen-logical units to device pixels — the same 1:1 identity every
//! single-panel render uses, so a composed frame is comparable to the
//! panel frames beneath it pixel for pixel.
//!
//! Placement reuses [`crate::FramebufferDims`]'s sub-window idea: a slot
//! is a window onto a shared surface, differing from a strided
//! sub-buffer only in that slots may overlap. Nothing here rescales a
//! scene — the IR has no scale op and a compositor never rewrites
//! commands, so a slot's dimensions *are* the frame its panel is asked
//! to emit.
//!
//! **One snapshot, one alert state, every slot.** The inputs are fanned
//! to every panel unchanged, which is what makes two overlapping panels
//! showing the same quantity unable to disagree
//! (`AIR-BAS-007`). A caller that resolved twice would have to work at
//! it.
//!
//! Scope, stated plainly: this is a determinism instrument, not a
//! flight-worthy compositor. A slot that fails here spoils the whole
//! frame, as every reference render does, because a bit-exact harness
//! that painted a partial frame would pin a hash of undefined content.
//! The per-slot in-rect failure presentation the contract requires of a
//! real backend is a runtime obligation of the shell, above this.
//!
//! A composition is validated at init by
//! [`indicate_instrument_registry::validate_composition`]; this paints
//! what that admitted, and deliberately does not re-run it — validation
//! needs the criticality bands, and painting must not.

use indicate_alerts::AlertOutput;
use indicate_instrument_registry::{
    CompositionDescriptor, DesignFrame, EMPTY_CONFIG, Registry, Slot,
};
use indicate_instrument_scene::SceneWriter;
use indicate_instrument_state::PanelData;

use crate::error::RasterError;
use crate::raster::{Placement, paint_at};
use crate::report::{FrameId, FramebufferDims, RenderReport, RenderStatus};
use crate::surface::Surface;

/// What every slot of one composed frame is drawn from.
///
/// The scratch buffer is the caller's, reused slot by slot: a slot's
/// scene is encoded, painted, and finished with before the next one is
/// encoded, so a composed frame needs one scene buffer rather than one
/// per slot. Size it [`indicate_instrument_scene::MAX_SCENE_BYTES`].
pub struct CompositionInputs<'a> {
    /// The single resolved snapshot every slot draws from.
    pub data: &'a PanelData,
    /// The single alert state every slot draws from.
    pub alerts: Option<&'a AlertOutput>,
    /// Scene encoding scratch, reused across slots.
    pub scratch: &'a mut [u8],
}

/// Paints `composition` into one RGBA8 framebuffer.
///
/// The framebuffer is the logical screen at 1:1. On success it holds the
/// composed frame; on any failure after the framebuffer geometry is
/// accepted it holds the spoil pattern and the error is returned, so a
/// caller can never mistake a partial composition for a whole one.
///
/// The report describes the composed frame: `layers_present` is the
/// union over slots, `unknown_opcodes` their sum, and `work` the whole
/// frame's. Composed work is deliberately not gated against
/// [`crate::RenderWork::BUDGET`], which prices one panel — a composed
/// budget is the sum of its slots' and belongs to whoever declares the
/// screen.
pub fn render_composition(
    registry: &Registry,
    composition: &CompositionDescriptor,
    inputs: &mut CompositionInputs<'_>,
    pixels: &mut [u8],
    dims: FramebufferDims,
    frame: FrameId,
) -> Result<RenderReport, RasterError> {
    let mut surface = Surface::new(pixels, dims)?;
    surface.clear();
    let mut painted = Painted::default();
    for slot in composition.slots {
        match paint_slot(registry, slot, inputs, &mut surface, &mut painted) {
            Ok(()) => {}
            Err(error) => {
                surface.spoil();
                return Err(error);
            }
        }
    }
    Ok(RenderReport {
        scene_version: painted.scene_version,
        status: RenderStatus::Painted,
        frame,
        unknown_opcodes: painted.unknown_opcodes,
        layers_present: painted.layers_present,
        work: surface.work(),
    })
}

/// What the slots painted so far contribute to the composed report.
#[derive(Default)]
struct Painted {
    scene_version: u8,
    unknown_opcodes: u32,
    layers_present: u8,
}

fn paint_slot(
    registry: &Registry,
    slot: &Slot,
    inputs: &mut CompositionInputs<'_>,
    surface: &mut Surface<'_>,
    painted: &mut Painted,
) -> Result<(), RasterError> {
    let panel = registry
        .by_id(slot.panel)
        .ok_or(RasterError::SlotPanelMissing { panel: slot.panel })?;
    let mut writer = SceneWriter::new(inputs.scratch)
        .map_err(|_| RasterError::SlotSceneBuffer { panel: panel.id })?;
    // The frame asked for is the slot's own size: composition is
    // placement, so the rect and the emission frame are one decision.
    (panel.draw)(
        inputs.data,
        &EMPTY_CONFIG,
        inputs.alerts,
        slot_frame(slot),
        &mut writer,
    )
    .map_err(|source| RasterError::SlotDraw {
        panel: panel.id,
        source,
    })?;
    let used = writer.finish();
    let scene = inputs
        .scratch
        .get(..used)
        .ok_or(RasterError::SlotSceneBuffer { panel: panel.id })?;
    painted.scene_version = scene.first().copied().unwrap_or(0);
    let (unknown, layers) = paint_at(
        scene,
        surface,
        Placement {
            origin: (slot.rect.x, slot.rect.y),
            extent: Some((slot.rect.width, slot.rect.height)),
        },
    )?;
    painted.unknown_opcodes = painted.unknown_opcodes.wrapping_add(unknown);
    painted.layers_present |= layers;
    Ok(())
}

/// The frame a slot asks its panel for.
fn slot_frame(slot: &Slot) -> DesignFrame {
    DesignFrame {
        width: slot.rect.width,
        height: slot.rect.height,
    }
}

#[cfg(test)]
mod tests;
