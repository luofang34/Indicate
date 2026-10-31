//! Validation of declared render spaces, bounds, and canonical sizes.

use indicate_instrument_descriptor::{
    DesignFrame, FrameRefusal, PanelDescriptor, Region, RenderSpace,
};
use indicate_instrument_scene::{
    LAYER_COUNT, angular::ANGULAR_LAYER_COUNT, volume::VOLUME_BAND_COUNT,
};

use super::error::RegistryError;

const PLANAR_LAYER_MASK: u16 = (1u16 << LAYER_COUNT) - 1;
const ANGULAR_LAYER_MASK: u16 = (1u16 << ANGULAR_LAYER_COUNT) - 1;
const VOLUME_BAND_MASK: u16 = (1u16 << VOLUME_BAND_COUNT) - 1;

pub(super) fn validate(index: usize, panel: &PanelDescriptor) -> Result<(), RegistryError> {
    if panel.required_layers == 0 {
        return Err(RegistryError::NoRequiredLayers { index });
    }
    match panel.space {
        RenderSpace::Planar2D {
            frame_min,
            frame_max,
            frame_step,
            aspect_min,
            aspect_max,
            canonical_frames,
        } => {
            if !panel.underlay.valid_for_planar_2d() {
                return Err(RegistryError::IncompatibleUnderlay { index });
            }
            if (panel.required_layers & !PLANAR_LAYER_MASK) != 0 {
                return Err(RegistryError::UndefinedLayerBits {
                    index,
                    bits: panel.required_layers,
                });
            }
            validate_bounds(
                index, frame_min, frame_max, frame_step, aspect_min, aspect_max,
            )?;
            validate_canonical(index, panel, frame_min, frame_max, canonical_frames)?;
            validate_baselines(index, panel, canonical_frames)?;
            validate_regions(index, panel, frame_min)
        }
        RenderSpace::AngularSphere { overlay_frame } => {
            validate_angular(index, panel, overlay_frame)
        }
        RenderSpace::Volume3D {
            extent_min_m,
            extent_max_m,
            canonical_extents_m,
            view_limits,
        } => validate_volume(
            index,
            panel,
            extent_min_m,
            extent_max_m,
            canonical_extents_m,
            &view_limits,
        ),
    }
}

fn positive(value: f32) -> bool {
    value.is_finite() && value > 0.0
}

fn validate_angular(
    index: usize,
    panel: &PanelDescriptor,
    overlay_frame: Option<DesignFrame>,
) -> Result<(), RegistryError> {
    if !panel.underlay.valid_for_angular_sphere() {
        return Err(RegistryError::IncompatibleUnderlay { index });
    }
    if !panel.group_regions.is_empty() || !panel.raster_baselines.is_empty() {
        return Err(RegistryError::NonPlanar2DMetadata { index });
    }
    let allowed = if let Some(frame) = overlay_frame {
        if !(positive(frame.width) && positive(frame.height)) {
            return Err(RegistryError::BadOverlayFrame { index });
        }
        ANGULAR_LAYER_MASK | (PLANAR_LAYER_MASK << 8)
    } else {
        ANGULAR_LAYER_MASK
    };
    if (panel.required_layers & !allowed) != 0 {
        return Err(RegistryError::UndefinedLayerBits {
            index,
            bits: panel.required_layers,
        });
    }
    if (panel.required_layers & ANGULAR_LAYER_MASK) == 0 {
        return Err(RegistryError::NoRequiredLayers { index });
    }
    Ok(())
}

fn validate_volume(
    index: usize,
    panel: &PanelDescriptor,
    min: indicate_instrument_scene::volume::Extent3D,
    max: indicate_instrument_scene::volume::Extent3D,
    canonical: &[indicate_instrument_scene::volume::Extent3D],
    view_limits: &indicate_interaction::ViewLimits,
) -> Result<(), RegistryError> {
    if !panel.underlay.valid_for_volume_3d() {
        return Err(RegistryError::IncompatibleUnderlay { index });
    }
    if !panel.group_regions.is_empty() || !panel.raster_baselines.is_empty() {
        return Err(RegistryError::NonPlanar2DMetadata { index });
    }
    if (panel.required_layers & !VOLUME_BAND_MASK) != 0 {
        return Err(RegistryError::UndefinedLayerBits {
            index,
            bits: panel.required_layers,
        });
    }
    if !(min.is_valid() && max.is_valid()) {
        return Err(RegistryError::BadExtentBounds { index });
    }
    if max.width_m < min.width_m || max.height_m < min.height_m || max.depth_m < min.depth_m {
        return Err(RegistryError::ExtentRangeInverted { index });
    }
    if !view_limits.is_valid() {
        return Err(RegistryError::BadViewLimits { index });
    }
    if canonical.is_empty() {
        return Err(RegistryError::NoCanonicalExtents { index });
    }
    for (position, ext) in canonical.iter().enumerate() {
        if canonical[..position].contains(ext) {
            return Err(RegistryError::DuplicateCanonicalExtent { index, position });
        }
        if panel.accepts_volume_3d(*ext).is_err() {
            return Err(RegistryError::CanonicalExtentOutOfRange { index, position });
        }
    }
    Ok(())
}

fn validate_bounds(
    index: usize,
    min: DesignFrame,
    max: DesignFrame,
    step: (f32, f32),
    aspect_min: f32,
    aspect_max: f32,
) -> Result<(), RegistryError> {
    if !(positive(min.width) && positive(min.height) && positive(max.width) && positive(max.height))
    {
        return Err(RegistryError::BadFrameBounds { index });
    }
    if max.width < min.width || max.height < min.height {
        return Err(RegistryError::FrameRangeInverted { index });
    }
    if !(positive(step.0) && positive(step.1)) {
        return Err(RegistryError::BadFrameStep { index });
    }
    if !(positive(aspect_min) && positive(aspect_max) && aspect_min <= aspect_max) {
        return Err(RegistryError::BadAspectBounds { index });
    }
    Ok(())
}

fn validate_canonical(
    index: usize,
    panel: &PanelDescriptor,
    frame_min: DesignFrame,
    frame_max: DesignFrame,
    canonical_frames: &[DesignFrame],
) -> Result<(), RegistryError> {
    if canonical_frames.is_empty() {
        return Err(RegistryError::NoCanonicalFrames { index });
    }
    if !canonical_frames.contains(&frame_min) {
        return Err(RegistryError::CanonicalFramesMissingMin { index });
    }
    if !canonical_frames.contains(&frame_max) {
        return Err(RegistryError::CanonicalFramesMissingMax { index });
    }
    for (position, frame) in canonical_frames.iter().enumerate() {
        if canonical_frames[..position].contains(frame) {
            return Err(RegistryError::DuplicateCanonicalFrame { index, position });
        }
        match panel.accepts_2d(*frame) {
            Ok(()) => {}
            Err(
                FrameRefusal::Degenerate | FrameRefusal::OutOfRange | FrameRefusal::NotPlanar2D,
            ) => {
                return Err(RegistryError::CanonicalFrameOutOfRange { index, position });
            }
            Err(FrameRefusal::OffStep) => {
                return Err(RegistryError::CanonicalFrameOffStep { index, position });
            }
            Err(FrameRefusal::Aspect) => {
                return Err(RegistryError::CanonicalFrameAspect { index, position });
            }
        }
    }
    Ok(())
}

fn validate_baselines(
    index: usize,
    panel: &PanelDescriptor,
    canonical_frames: &[DesignFrame],
) -> Result<(), RegistryError> {
    for (position, (frame, _)) in panel.raster_baselines.iter().enumerate() {
        if !canonical_frames.contains(frame) {
            return Err(RegistryError::RasterBaselineNotCanonical { index, position });
        }
        if panel.raster_baselines[..position]
            .iter()
            .any(|(earlier, _)| earlier == frame)
        {
            return Err(RegistryError::DuplicateRasterBaseline { index, position });
        }
    }
    Ok(())
}

fn validate_regions(
    index: usize,
    panel: &PanelDescriptor,
    frame_min: DesignFrame,
) -> Result<(), RegistryError> {
    for (group, region) in panel.group_regions {
        if !panel.required_groups.contains(*group) {
            return Err(RegistryError::RegionGroupNotRequired {
                index,
                group: *group as u8,
            });
        }
        let inside = region.is_sound() && Region::of(frame_min).contains(region);
        if !inside {
            return Err(RegistryError::RegionOutsideFrame {
                index,
                group: *group as u8,
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
