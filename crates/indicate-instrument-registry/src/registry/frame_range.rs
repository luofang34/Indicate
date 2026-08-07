//! The declared frame range: bounds, quantization, aspect, and the
//! evidence sizes pinned inside it.
//!
//! A shell picks the frame it draws a panel at, so what a panel declares
//! here is the whole of what a shell may pick from. Every rule below is
//! refused at composition rather than at draw time: a frame a panel
//! cannot lay out against must be unaskable, not merely unhandled.

use indicate_instrument_descriptor::{DesignFrame, PanelDescriptor};

use super::error::RegistryError;

/// Validates the frame bounds, the step grid, the aspect bounds, the
/// canonical frames pinned inside them, and the baselines pinned
/// against those.
pub(super) fn validate(index: usize, panel: &PanelDescriptor) -> Result<(), RegistryError> {
    validate_bounds(index, panel)?;
    validate_canonical(index, panel)?;
    validate_baselines(index, panel)
}

fn positive(value: f32) -> bool {
    value.is_finite() && value > 0.0
}

fn validate_bounds(index: usize, panel: &PanelDescriptor) -> Result<(), RegistryError> {
    let (min, max) = (panel.frame_min, panel.frame_max);
    if !(positive(min.width) && positive(min.height) && positive(max.width) && positive(max.height))
    {
        return Err(RegistryError::BadFrameBounds { index });
    }
    if max.width < min.width || max.height < min.height {
        return Err(RegistryError::FrameRangeInverted { index });
    }
    let (step_w, step_h) = panel.frame_step;
    if !(positive(step_w) && positive(step_h)) {
        return Err(RegistryError::BadFrameStep { index });
    }
    if !(positive(panel.aspect_min)
        && positive(panel.aspect_max)
        && panel.aspect_min <= panel.aspect_max)
    {
        return Err(RegistryError::BadAspectBounds { index });
    }
    Ok(())
}

fn validate_canonical(index: usize, panel: &PanelDescriptor) -> Result<(), RegistryError> {
    if panel.canonical_frames.is_empty() {
        return Err(RegistryError::NoCanonicalFrames { index });
    }
    if !panel.canonical_frames.contains(&panel.frame_min) {
        return Err(RegistryError::CanonicalFramesMissingMin { index });
    }
    if !panel.canonical_frames.contains(&panel.frame_max) {
        return Err(RegistryError::CanonicalFramesMissingMax { index });
    }
    for (position, frame) in panel.canonical_frames.iter().enumerate() {
        // A repeated frame draws and digests the same scene twice and
        // runs the whole admission matrix again for it, inflating the
        // case count and the warning ratchet for no coverage.
        if panel.canonical_frames[..position].contains(frame) {
            return Err(RegistryError::DuplicateCanonicalFrame { index, position });
        }
        // Spelled out rather than routed through `supports`, because a
        // canonical frame that fails names *which* rule it broke, and
        // a declaration is fixed by knowing that.
        if !in_range(*frame, panel) {
            return Err(RegistryError::CanonicalFrameOutOfRange { index, position });
        }
        if !on_grid(frame.width, panel.frame_min.width, panel.frame_step.0)
            || !on_grid(frame.height, panel.frame_min.height, panel.frame_step.1)
        {
            return Err(RegistryError::CanonicalFrameOffStep { index, position });
        }
        if !aspect_ok(*frame, panel) {
            return Err(RegistryError::CanonicalFrameAspect { index, position });
        }
    }
    Ok(())
}

fn validate_baselines(index: usize, panel: &PanelDescriptor) -> Result<(), RegistryError> {
    for (position, (frame, _)) in panel.raster_baselines.iter().enumerate() {
        if !panel.canonical_frames.contains(frame) {
            return Err(RegistryError::RasterBaselineNotCanonical { index, position });
        }
        // One baseline per canonical frame: a second entry for the same
        // frame is dead, because the lookup takes the first match, and
        // two hashes for one rendering disagree about what is pinned.
        if panel.raster_baselines[..position]
            .iter()
            .any(|(earlier, _)| earlier == frame)
        {
            return Err(RegistryError::DuplicateRasterBaseline { index, position });
        }
    }
    Ok(())
}

/// Whether `panel` declared that it can lay out against `frame`: in
/// range, on the step grid, and inside the aspect bounds.
///
/// The composition layer asks this of a slot's dimensions, and a shell
/// choosing a frame may ask it before drawing — the draw path itself
/// re-checks nothing.
pub(crate) fn supports(panel: &PanelDescriptor, frame: DesignFrame) -> bool {
    in_range(frame, panel)
        && on_grid(frame.width, panel.frame_min.width, panel.frame_step.0)
        && on_grid(frame.height, panel.frame_min.height, panel.frame_step.1)
        && aspect_ok(frame, panel)
}

fn in_range(frame: DesignFrame, panel: &PanelDescriptor) -> bool {
    frame.width >= panel.frame_min.width
        && frame.width <= panel.frame_max.width
        && frame.height >= panel.frame_min.height
        && frame.height <= panel.frame_max.height
}

/// Whether `value` is `min + k * step` for a whole `k`.
///
/// Exact, with no tolerance: a step that cannot express the declared
/// frames exactly is a declaration to fix, not a rounding to absorb.
/// Admitting near-misses would put the digest and the baselines at
/// frames the arithmetic cannot reproduce.
fn on_grid(value: f32, min: f32, step: f32) -> bool {
    let offset = value - min;
    offset >= 0.0 && offset % step == 0.0
}

fn aspect_ok(frame: DesignFrame, panel: &PanelDescriptor) -> bool {
    let aspect = frame.width / frame.height;
    aspect >= panel.aspect_min && aspect <= panel.aspect_max
}

#[cfg(test)]
mod tests;
