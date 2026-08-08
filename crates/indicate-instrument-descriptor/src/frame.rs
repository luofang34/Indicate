//! The predicate over a panel's declared frame bounds.
//!
//! A descriptor publishes `frame_min`, `frame_max`, `frame_step`, and
//! the aspect bounds. Publishing those without the rule that reads them
//! leaves every shell to write the rule, and two shells that write it
//! separately disagree — on the tolerance, on whether a bound is
//! inclusive, on whether the step is measured from the minimum or from
//! zero — while each stays locally green, because each only ever tests
//! its own. The panel is the one thing that knows, so the panel answers.

use crate::descriptor::{DesignFrame, PanelDescriptor};

/// Which declared bound a frame violated.
///
/// Typed rather than a bool because a shell that asked for a frame
/// wants to know what to ask for instead, and the diagnostic then reads
/// the same on every shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FrameRefusal {
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

/// The tolerance applied when testing a dimension against the step
/// grid: exactly none.
///
/// Stated as a constant because it is the parameter two shells would
/// otherwise each choose. Zero is the choice: a step that cannot
/// express a frame exactly is a declaration to fix, not a rounding to
/// absorb, and admitting near-misses would pin the digest and the
/// raster baselines at frames the arithmetic cannot reproduce.
pub const FRAME_STEP_TOLERANCE: f32 = 0.0;

impl PanelDescriptor {
    /// Whether this panel declared that it can lay out against `frame`.
    ///
    /// This is the whole rule, and the only copy of it. A shell asks
    /// before drawing; the draw path re-checks nothing.
    ///
    /// Safe on a descriptor this crate never validated: a bound that is
    /// not a number refuses every frame rather than accepting one.
    pub fn accepts(&self, frame: DesignFrame) -> Result<(), FrameRefusal> {
        if !(finite_positive(frame.width) && finite_positive(frame.height)) {
            return Err(FrameRefusal::Degenerate);
        }
        // Phrased as "must be inside" rather than "reject if outside",
        // so a bound that is itself not a number refuses every frame
        // instead of accepting every frame. A shell may hold a
        // descriptor this crate never validated.
        let inside = frame.width >= self.frame_min.width
            && frame.width <= self.frame_max.width
            && frame.height >= self.frame_min.height
            && frame.height <= self.frame_max.height;
        if !inside {
            return Err(FrameRefusal::OutOfRange);
        }
        if !on_grid(frame.width, self.frame_min.width, self.frame_step.0)
            || !on_grid(frame.height, self.frame_min.height, self.frame_step.1)
        {
            return Err(FrameRefusal::OffStep);
        }
        let aspect = frame.width / frame.height;
        if !(aspect >= self.aspect_min && aspect <= self.aspect_max) {
            return Err(FrameRefusal::Aspect);
        }
        Ok(())
    }
}

fn finite_positive(value: f32) -> bool {
    value.is_finite() && value > 0.0
}

/// Whether `value` is `min + k * step` for a whole `k`.
///
/// Both sides of a grid line are tested. At a tolerance of zero the two
/// tests collapse into one, but a one-sided test would silently admit
/// near-misses above a line and refuse the identical miss below it,
/// which is not what a tolerance means.
fn on_grid(value: f32, min: f32, step: f32) -> bool {
    let offset = value - min;
    if offset < 0.0 {
        return false;
    }
    let remainder = offset % step;
    remainder <= FRAME_STEP_TOLERANCE || remainder >= step - FRAME_STEP_TOLERANCE
}

#[cfg(test)]
mod tests;
