//! The checks a spec passes before anything draws or builds from it.
//!
//! A spec read from data at run time can say anything, and a draw path
//! has no way to report a meaningless scale except by drawing a wrong
//! one. So the checks run once, up front, and every later step may
//! assume their results.

use super::{
    AnalogDialSpec, Channel, MAX_EXTRA_CHANNELS, MAX_POINTERS, MAX_WHEELS, MAX_WINDOWS, ScaleLaw,
    ScaleRing,
};

/// Why a spec was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SpecError {
    /// More pointers, windows, or wheels than the fixed capacities.
    #[error("the dial declares {found} {what}, more than the {limit} it can carry")]
    Capacity {
        /// What overflowed.
        what: &'static str,
        /// How many the spec declares.
        found: usize,
        /// The capacity.
        limit: usize,
    },
    /// A piecewise law with fewer than two points, values not strictly
    /// ascending, or a non-finite entry; or a turns law with no turn.
    #[error("a scale law cannot place values (ring or pointer {index})")]
    BadLaw {
        /// Index of the ring, or of the pointer counted after the rings.
        index: usize,
    },
    /// The engraved unit does not measure what the input reports.
    #[error("the {channel:?} channel's unit does not measure its input")]
    UnitMismatch {
        /// The channel.
        channel: Channel,
    },
    /// A moving part follows a channel the dial does not have.
    #[error("a moving part follows the {channel:?} channel, which the dial lacks")]
    MissingChannel {
        /// The channel.
        channel: Channel,
    },
    /// A size, radius, range, or position that is not a finite number
    /// in its allowed range.
    #[error("the dial's {what} is not a usable number")]
    BadGeometry {
        /// What was wrong.
        what: &'static str,
    },
}

fn law_ok(law: &ScaleLaw<'_>) -> bool {
    match *law {
        ScaleLaw::Piecewise {
            reference_deg,
            points,
        } => {
            reference_deg.is_finite()
                && points.len() >= 2
                && points
                    .iter()
                    .all(|p| p.value.is_finite() && p.angle_deg.is_finite())
                && points.windows(2).all(|pair| match pair {
                    [lo, hi] => lo.value < hi.value,
                    _ => false,
                })
        }
        ScaleLaw::Turns {
            reference_deg,
            value_per_turn,
        } => reference_deg.is_finite() && value_per_turn.is_finite() && value_per_turn != 0.0,
    }
}

fn ring_ok(ring: &ScaleRing<'_>) -> bool {
    let finite_pair = |p: [f32; 2]| p.iter().all(|c| c.is_finite());
    finite_pair(ring.center)
        && ring.radius.is_finite()
        && ring.radius > 0.0
        && ring
            .ticks
            .iter()
            .all(|t| t.every.is_finite() && t.every > 0.0 && t.from.is_finite() && t.to.is_finite())
}

fn capacity(what: &'static str, found: usize, limit: usize) -> Result<(), SpecError> {
    if found > limit {
        Err(SpecError::Capacity { what, found, limit })
    } else {
        Ok(())
    }
}

impl AnalogDialSpec<'_> {
    /// Checks the spec once, before any draw or build reads it.
    pub fn check(&self) -> Result<(), SpecError> {
        capacity("pointers", self.pointers.len(), MAX_POINTERS)?;
        capacity("windows", self.windows.len(), MAX_WINDOWS)?;
        for window in self.windows {
            capacity("wheels", usize::from(window.wheels), MAX_WHEELS)?;
        }
        let sizes_ok = self.case_diameter_mm.is_finite()
            && self.face_radius_mm.is_finite()
            && self.face_radius_mm > 0.0
            && self.case_diameter_mm >= 2.0 * self.face_radius_mm;
        if !sizes_ok {
            return Err(SpecError::BadGeometry {
                what: "face radius or case diameter",
            });
        }
        capacity("extra channels", self.extra.len(), MAX_EXTRA_CHANNELS)?;
        let channels = [
            (Channel::Primary, Some(self.primary)),
            (Channel::Secondary, self.secondary),
        ]
        .into_iter()
        .chain(
            self.extra
                .iter()
                .enumerate()
                .map(|(n, c)| (Channel::Extra(n as u8), Some(*c))),
        );
        for (which, channel) in channels {
            if channel.is_some_and(|c| !c.units_agree()) {
                return Err(SpecError::UnitMismatch { channel: which });
            }
        }
        for (index, ring) in self.rings.iter().enumerate() {
            if !law_ok(&ring.law) || !ring_ok(ring) {
                return Err(SpecError::BadLaw { index });
            }
        }
        for (offset, pointer) in self.pointers.iter().enumerate() {
            if !law_ok(&pointer.law) {
                let index = self.rings.len().saturating_add(offset);
                return Err(SpecError::BadLaw { index });
            }
        }
        self.check_moving_parts()
    }

    fn check_moving_parts(&self) -> Result<(), SpecError> {
        let channels = self
            .pointers
            .iter()
            .map(|p| p.channel)
            .chain(self.windows.iter().map(|w| w.channel));
        for channel in channels {
            if self.channel(channel).is_none() {
                return Err(SpecError::MissingChannel { channel });
            }
        }
        for window in self.windows {
            let [lo, hi] = window.range;
            let ok = window.aperture.iter().all(|c| c.is_finite())
                && window.value_scale.is_finite()
                && window.value_scale > 0.0
                && lo.is_finite()
                && hi.is_finite()
                && lo < hi;
            if !ok {
                return Err(SpecError::BadGeometry {
                    what: "drum window",
                });
            }
        }
        if let Some([lo, hi]) = self.input_range
            && !(lo.is_finite() && hi.is_finite() && lo < hi)
        {
            return Err(SpecError::BadGeometry {
                what: "input range",
            });
        }
        Ok(())
    }
}
