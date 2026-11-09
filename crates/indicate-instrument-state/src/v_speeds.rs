//! Airframe V-speed band limits and reference speed bugs shared across
//! linear tapes, HUD scales, standby indicators, and circular airspeed dials.

/// Airframe structural and stall reference speeds (knots) driving airspeed
/// color arcs and limit bars (`0 <= vs0 <= vs < vfe <= vno < vne`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VSpeeds {
    /// Stall speed in landing configuration (bottom of white flap arc).
    pub vs0_kt: f32,
    /// Clean 1g stall speed (`Vs1`, bottom of green normal-operating arc).
    pub vs_kt: f32,
    /// Maximum full-flap extended speed (top of white flap arc).
    pub vfe_kt: f32,
    /// Maximum structural cruising speed (top of green arc / bottom of caution arc).
    pub vno_kt: f32,
    /// Never-exceed speed (red line).
    pub vne_kt: f32,
}

impl VSpeeds {
    /// Returns `true` when all five speeds are finite, non-negative, and strictly
    /// ordered (`vs0 <= vs < vfe <= vno < vne`).
    #[must_use]
    pub fn is_valid(&self) -> bool {
        let all = [
            self.vs0_kt,
            self.vs_kt,
            self.vfe_kt,
            self.vno_kt,
            self.vne_kt,
        ];
        all.iter().all(|v| v.is_finite() && *v >= 0.0)
            && self.vs0_kt <= self.vs_kt
            && self.vs_kt < self.vfe_kt
            && self.vfe_kt <= self.vno_kt
            && self.vno_kt < self.vne_kt
    }
}

/// One resolved V-speed bug mark (`"R"`, `"X"`, `"Y"`, `"G"`, `"A"`, `"REF"`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VSpeedBug {
    /// Short cockpit bug tag (`"R"`, `"X"`, `"Y"`, `"G"`, `"A"`, or `"REF"`).
    pub tag: &'static str,
    /// Calibrated/indicated airspeed in knots (`> 0`).
    pub speed_kt: f32,
}

/// Optional POH / AFM airframe reference speed bugs (knots) rendered alongside
/// speed tapes, HUD scales, and dial airspeed indicators.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct VSpeedBugs {
    /// Rotation speed (`Vr`).
    pub vr_kt: Option<f32>,
    /// Best angle-of-climb speed (`Vx`).
    pub vx_kt: Option<f32>,
    /// Best rate-of-climb speed (`Vy`).
    pub vy_kt: Option<f32>,
    /// Best power-off glide speed (`Vg`).
    pub vg_kt: Option<f32>,
    /// Design maneuvering speed (`Va`, optionally mass-adjusted for current weight).
    pub va_kt: Option<f32>,
    /// Final approach reference speed (`Vref`, typically `1.3 * Vs0`).
    pub vref_kt: Option<f32>,
}

impl VSpeedBugs {
    /// Maximum number of reference bugs in [`VSpeedBugs`].
    pub const MAX_BUGS: usize = 6;

    /// Returns `true` when every reference bug is `None`.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.vr_kt.is_none()
            && self.vx_kt.is_none()
            && self.vy_kt.is_none()
            && self.vg_kt.is_none()
            && self.va_kt.is_none()
            && self.vref_kt.is_none()
    }

    /// Validates that every populated bug is finite, strictly positive, ordered
    /// (`vx <= vy` when both exist), and at most `bounds.vne_kt` when `bounds` is given.
    #[must_use]
    pub fn is_valid_for(&self, bounds: Option<&VSpeeds>) -> bool {
        let cap = bounds.map_or(f32::INFINITY, |b| b.vne_kt);
        let opts = [
            self.vr_kt,
            self.vx_kt,
            self.vy_kt,
            self.vg_kt,
            self.va_kt,
            self.vref_kt,
        ];
        for v in opts.into_iter().flatten() {
            if !v.is_finite() || v <= 0.0 || v > cap {
                return false;
            }
        }
        if let (Some(vx), Some(vy)) = (self.vx_kt, self.vy_kt)
            && vx > vy
        {
            return false;
        }
        true
    }

    /// Collects all active bugs sorted by ascending `speed_kt` without heap allocation.
    #[must_use]
    pub fn sorted(&self) -> ([VSpeedBug; Self::MAX_BUGS], usize) {
        let mut out = [VSpeedBug {
            tag: "",
            speed_kt: 0.0,
        }; Self::MAX_BUGS];
        let mut count = 0usize;
        let candidates = [
            ("R", self.vr_kt),
            ("X", self.vx_kt),
            ("REF", self.vref_kt),
            ("G", self.vg_kt),
            ("Y", self.vy_kt),
            ("A", self.va_kt),
        ];
        for (tag, opt) in candidates {
            if let Some(speed_kt) = opt
                && speed_kt.is_finite()
                && speed_kt > 0.0
                && let Some(slot) = out.get_mut(count)
            {
                *slot = VSpeedBug { tag, speed_kt };
                count = count.wrapping_add(1);
            }
        }
        // Insertion sort over at most 6 items.
        let mut i = 1usize;
        while i < count {
            let mut j = i;
            while j > 0 {
                let swap = match (out.get(j - 1), out.get(j)) {
                    (Some(prev), Some(curr)) => prev.speed_kt > curr.speed_kt,
                    _ => false,
                };
                if !swap {
                    break;
                }
                out.swap(j - 1, j);
                j -= 1;
            }
            i = i.wrapping_add(1);
        }
        (out, count)
    }
}
