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

    /// The frame this panel offers for a space, or why it offers none.
    ///
    /// [`Self::accepts`] is a veto: it can refuse a frame a shell
    /// already holds, but it cannot produce one. Without this, a shell
    /// that must pick a frame walks the step grid itself, which is the
    /// arithmetic two shells write differently — and the reason
    /// `accepts` exists at all. So the panel answers this question too.
    ///
    /// `space` is in the same logical units as a [`DesignFrame`], never
    /// device pixels. A shell holding a surface in physical pixels
    /// divides by its own scale factor before asking, because the panel
    /// knows nothing of that factor; two shells that pass different
    /// units for one window get different frames, which is the outcome
    /// a shared rule exists to prevent. A shell under no constraint
    /// passes `frame_max` and receives it back.
    ///
    /// The frame returned is the largest by area that fits inside
    /// `space` on both axes and that `accepts` admits. It is found in
    /// closed form, never by enumeration: each axis is clamped into the
    /// declared range and floored onto the step grid, and if that frame
    /// is outside the aspect bounds the two frames that reach an aspect
    /// bound exactly — one limited by width, one by height — are floored
    /// the same way and the larger admissible one wins, width first when
    /// their areas tie. A panel whose step cannot express its own grid
    /// exactly refuses here through `accepts` rather than rounding onto
    /// a line the arithmetic cannot reproduce.
    pub fn choose_frame(&self, space: DesignFrame) -> Result<DesignFrame, FrameRefusal> {
        if !(finite_positive(space.width) && finite_positive(space.height)) {
            return Err(FrameRefusal::Degenerate);
        }
        let clamped = self.floor_into_range(space)?;
        if self.accepts(clamped).is_ok() {
            return Ok(clamped);
        }
        // Only the aspect bounds can still refuse a frame that is inside
        // the range and on the grid, and the largest frame under an
        // aspect bound touches that bound on one axis.
        let by_width = DesignFrame {
            width: clamped.height * self.aspect_max,
            height: clamped.height,
        };
        let by_height = DesignFrame {
            width: clamped.width,
            height: clamped.width / self.aspect_min,
        };
        let mut best: Option<DesignFrame> = None;
        for candidate in [by_width, by_height] {
            // Reaching an aspect bound can ask for more of an axis than
            // the space has; the space is still the ceiling.
            let bounded = DesignFrame {
                width: candidate.width.min(space.width),
                height: candidate.height.min(space.height),
            };
            let Ok(floored) = self.floor_into_range(bounded) else {
                continue;
            };
            if self.accepts(floored).is_err() {
                continue;
            }
            let better = match best {
                None => true,
                Some(b) => floored.width * floored.height > b.width * b.height,
            };
            if better {
                best = Some(floored);
            }
        }
        best.ok_or(FrameRefusal::Aspect)
    }

    /// Each axis clamped to the declared range and floored onto the step
    /// grid. Refuses a space that cannot hold `frame_min`: a panel is
    /// not served by a frame smaller than the one it declared it needs,
    /// and shrinking one is the shell's business, not the panel's.
    fn floor_into_range(&self, space: DesignFrame) -> Result<DesignFrame, FrameRefusal> {
        let width = floor_on_grid(
            space.width.min(self.frame_max.width),
            self.frame_min.width,
            self.frame_step.0,
        )?;
        let height = floor_on_grid(
            space.height.min(self.frame_max.height),
            self.frame_min.height,
            self.frame_step.1,
        )?;
        Ok(DesignFrame { width, height })
    }
}

/// The largest `min + k * step` that is not above `value`, for a whole
/// `k`. A `value` below `min` has no such line and refuses.
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
    // `offset` is not negative, so truncating toward zero is the floor.
    let k = (steps as i64) as f32;
    let line = min + k * step;
    // Binary rounding can put the computed line a hair above the value
    // it was floored from; the line below is then the real answer.
    if line > value && k >= 1.0 {
        return Ok(min + (k - 1.0) * step);
    }
    Ok(line)
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
