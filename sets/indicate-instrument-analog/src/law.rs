//! Value-to-angle laws and drum-wheel offsets: the pure arithmetic a
//! 3D backend runs per frame instead of drawing or decoding a scene.

use crate::spec::{ScaleLaw, ScalePoint};

impl ScaleLaw<'_> {
    /// Where `value` sits, degrees clockwise from twelve o'clock.
    ///
    /// A piecewise law holds at its end points, so a value past the
    /// last graduation rests on the stop rather than wrapping round the
    /// face. A turns law is unbounded, as an altimeter hand is. A law
    /// whose data cannot place anything answers its reference angle;
    /// [`crate::AnalogDialSpec::check`] refuses such a spec up front.
    #[must_use]
    pub fn angle_deg(&self, value: f32) -> f32 {
        match *self {
            Self::Piecewise {
                reference_deg,
                points,
            } => reference_deg + piecewise(points, value),
            Self::Turns {
                reference_deg,
                value_per_turn,
            } => {
                if value_per_turn.is_finite() && value_per_turn != 0.0 {
                    reference_deg + 360.0 * (value / value_per_turn)
                } else {
                    reference_deg
                }
            }
        }
    }

    /// [`Self::angle_deg`] in radians, the unit a scene `ROTATE` and a
    /// 3D transform take.
    #[must_use]
    pub fn angle_rad(&self, value: f32) -> f32 {
        self.angle_deg(value).to_radians()
    }
}

fn piecewise(points: &[ScalePoint], value: f32) -> f32 {
    let (Some(first), Some(last)) = (points.first(), points.last()) else {
        return 0.0;
    };
    if !value.is_finite() || value <= first.value {
        return first.angle_deg;
    }
    if value >= last.value {
        return last.angle_deg;
    }
    for pair in points.windows(2) {
        if let [lo, hi] = pair
            && value <= hi.value
        {
            let span = hi.value - lo.value;
            let t = if span > 0.0 {
                (value - lo.value) / span
            } else {
                0.0
            };
            return lo.angle_deg + t * (hi.angle_deg - lo.angle_deg);
        }
    }
    last.angle_deg
}

/// Each wheel's position, in digits, for a drum showing `shown`
/// (already scaled to the integer the wheels display), rightmost wheel
/// first.
///
/// The rightmost wheel turns continuously. Every other wheel stands on
/// a whole digit and turns only while every wheel to its right rolls
/// from 9 to 0, as an odometer's carry pins drive it. A position of
/// `2.5` means the wheel is half way from 2 to 3.
#[must_use]
pub fn wheel_positions<const N: usize>(shown: f32, wheels: usize) -> [f32; N] {
    let mut out = [0.0; N];
    if !shown.is_finite() || shown < 0.0 {
        return out;
    }
    let mut place = 1.0_f32;
    for (k, slot) in out.iter_mut().enumerate().take(wheels) {
        let position = shown / place;
        let digit = libm::floorf(position) % 10.0;
        let below = shown % place;
        let carry = if k == 0 {
            position - libm::floorf(position)
        } else {
            (below - (place - 1.0)).clamp(0.0, 1.0)
        };
        *slot = digit + carry;
        place *= 10.0;
    }
    out
}

#[cfg(test)]
mod tests;
