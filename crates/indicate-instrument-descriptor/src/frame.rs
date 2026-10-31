//! Predicates over an instrument's declared 2D frame and 3D volume bounds.

use indicate_instrument_scene::volume::Extent3D;

use crate::descriptor::{DesignFrame, InstrumentDescriptor};
use crate::space::RenderSpace;

/// Which declared bound a 2D [`DesignFrame`] violated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FrameRefusal {
    /// Instrument does not render in [`RenderSpace::Planar2D`].
    #[error("instrument does not declare a Planar2D render space")]
    NotPlanar2D,
    /// A non-finite or non-positive dimension: not a frame at all.
    #[error("frame dimensions must be finite and positive")]
    Degenerate,
    /// Outside `frame_min`..=`frame_max` on at least one axis.
    #[error("frame is outside the declared range")]
    OutOfRange,
    /// Not `frame_min + k * frame_step` for a whole `k` on some axis.
    #[error("frame is off the declared step grid")]
    OffStep,
    /// Width/height outside `aspect_min`..=`aspect_max`.
    #[error("frame aspect is outside the declared bounds")]
    Aspect,
}

/// Which declared bound a 3D [`Extent3D`] violated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ExtentRefusal {
    /// Instrument does not render in [`RenderSpace::Volume3D`].
    #[error("instrument does not declare a Volume3D render space")]
    NotVolume3D,
    /// A non-finite or non-positive metric dimension.
    #[error("volume extent dimensions must be finite and positive")]
    Degenerate,
    /// Outside `extent_min_m`..=`extent_max_m` on at least one axis.
    #[error("volume extent is outside the declared metric range")]
    OutOfRange,
}

const MAX_GRID_STEPS: usize = 4096;

/// Exact zero tolerance applied when testing a dimension against the step grid.
pub const FRAME_STEP_TOLERANCE: f32 = 0.0;

impl InstrumentDescriptor {
    /// Whether this `Planar2D` instrument declared that it can lay out against `frame`.
    pub fn accepts_2d(&self, frame: DesignFrame) -> Result<(), FrameRefusal> {
        let RenderSpace::Planar2D {
            frame_min,
            frame_max,
            frame_step,
            aspect_min,
            aspect_max,
            ..
        } = self.space
        else {
            return Err(FrameRefusal::NotPlanar2D);
        };
        if !(finite_positive(frame.width) && finite_positive(frame.height)) {
            return Err(FrameRefusal::Degenerate);
        }
        let inside = frame.width >= frame_min.width
            && frame.width <= frame_max.width
            && frame.height >= frame_min.height
            && frame.height <= frame_max.height;
        if !inside {
            return Err(FrameRefusal::OutOfRange);
        }
        if !on_grid(frame.width, frame_min.width, frame_step.0)
            || !on_grid(frame.height, frame_min.height, frame_step.1)
        {
            return Err(FrameRefusal::OffStep);
        }
        let aspect = frame.width / frame.height;
        if !(aspect >= aspect_min && aspect <= aspect_max) {
            return Err(FrameRefusal::Aspect);
        }
        Ok(())
    }

    /// Alias for [`Self::accepts_2d`].
    pub fn accepts(&self, frame: DesignFrame) -> Result<(), FrameRefusal> {
        self.accepts_2d(frame)
    }

    /// Largest admissible 2D [`DesignFrame`] by area that fits inside `space`.
    pub fn choose_frame_2d(&self, space: DesignFrame) -> Result<DesignFrame, FrameRefusal> {
        let RenderSpace::Planar2D {
            frame_min,
            frame_max,
            frame_step,
            aspect_min,
            aspect_max,
            ..
        } = self.space
        else {
            return Err(FrameRefusal::NotPlanar2D);
        };
        if !(finite_positive(space.width) && finite_positive(space.height)) {
            return Err(FrameRefusal::Degenerate);
        }
        let widest_by_aspect = space.height * aspect_max * (1.0 + 4.0 * f32::EPSILON);
        let mut width = floor_on_grid(
            space
                .width
                .min(frame_max.width)
                .min(widest_by_aspect.max(frame_min.width)),
            frame_min.width,
            frame_step.0,
        )?;
        let mut refusal = FrameRefusal::Aspect;
        for _ in 0..MAX_GRID_STEPS {
            if width < frame_min.width {
                break;
            }
            match self.tallest_admissible(
                width,
                space,
                (frame_min, frame_max, frame_step, aspect_min),
            ) {
                Ok(frame) => return Ok(frame),
                Err(FrameRefusal::OutOfRange) => refusal = FrameRefusal::OutOfRange,
                Err(_) => {}
            }
            width -= frame_step.0;
        }
        Err(refusal)
    }

    /// Alias for [`Self::choose_frame_2d`].
    pub fn choose_frame(&self, space: DesignFrame) -> Result<DesignFrame, FrameRefusal> {
        self.choose_frame_2d(space)
    }

    /// Whether this `Volume3D` instrument accepts `extent_m`.
    pub fn accepts_volume_3d(&self, extent_m: Extent3D) -> Result<(), ExtentRefusal> {
        let RenderSpace::Volume3D {
            extent_min_m,
            extent_max_m,
            ..
        } = self.space
        else {
            return Err(ExtentRefusal::NotVolume3D);
        };
        if !extent_m.is_valid() {
            return Err(ExtentRefusal::Degenerate);
        }
        let inside = extent_m.width_m >= extent_min_m.width_m
            && extent_m.width_m <= extent_max_m.width_m
            && extent_m.height_m >= extent_min_m.height_m
            && extent_m.height_m <= extent_max_m.height_m
            && extent_m.depth_m >= extent_min_m.depth_m
            && extent_m.depth_m <= extent_max_m.depth_m;
        if !inside {
            return Err(ExtentRefusal::OutOfRange);
        }
        Ok(())
    }

    fn tallest_admissible(
        &self,
        width: f32,
        space: DesignFrame,
        bounds: (DesignFrame, DesignFrame, (f32, f32), f32),
    ) -> Result<DesignFrame, FrameRefusal> {
        let (frame_min, frame_max, frame_step, aspect_min) = bounds;
        let by_aspect = width / aspect_min * (1.0 + 4.0 * f32::EPSILON);
        let ceiling = space.height.min(frame_max.height).min(by_aspect);
        let height = floor_on_grid(ceiling, frame_min.height, frame_step.1)?;
        let frame = DesignFrame { width, height };
        self.accepts_2d(frame)?;
        Ok(frame)
    }
}

fn floor_on_grid(value: f32, min: f32, step: f32) -> Result<f32, FrameRefusal> {
    if !(step.is_finite() && step > 0.0) {
        return Err(FrameRefusal::Degenerate);
    }
    let offset = value - min;
    if offset < 0.0 {
        return Err(FrameRefusal::OutOfRange);
    }
    let steps = offset / step;
    if !steps.is_finite() {
        return Err(FrameRefusal::Degenerate);
    }
    let k = (steps as i64) as f32;
    let line = min + k * step;
    if line > value && k >= 1.0 {
        return Ok(min + (k - 1.0) * step);
    }
    Ok(line)
}

fn finite_positive(value: f32) -> bool {
    value.is_finite() && value > 0.0
}

fn on_grid(value: f32, min: f32, step: f32) -> bool {
    let offset = value - min;
    if offset < 0.0 {
        return false;
    }
    let remainder = offset % step;
    remainder <= FRAME_STEP_TOLERANCE || remainder >= step - FRAME_STEP_TOLERANCE
}

#[cfg(test)]
mod optimality_tests;
#[cfg(test)]
mod tests;
