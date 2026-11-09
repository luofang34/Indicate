//! A runway that the host supplies from its scenery for approach symbology.
use indicate_instrument_state::SignalStatus;

/// A landing runway relative to the aircraft, from the host's scenery.
///
/// The geometry places the conformal runway outline and the glide path
/// reference line. Receiver deviations come from the navigation group, not
/// from this geometry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Runway {
    /// Landing threshold centre minus aircraft position, north/east/down metres.
    pub threshold_m: [f32; 3],
    /// Far-end down coordinate minus `threshold_m[2]` in north/east/down metres
    /// (`0.0` on a flat tangent plane; positive when downhill slope or Earth curvature
    /// places the far end lower than the threshold).
    pub far_down_offset_m: f32,
    /// Displaced pre-threshold overrun distance before `threshold_m` (`>= 0`, `0.0` when none),
    /// drawn with dashed sidelines leading up to the solid landing threshold bar.
    pub pre_threshold_m: f32,
    /// Opposite-end displaced threshold distance inward from the far end (`>= 0`, `0.0` when none),
    /// separating the between-thresholds landing zone from the reciprocal overrun.
    pub reciprocal_displaced_m: f32,
    /// Whether the landing surface is unpaved (`Turf`, `Gravel`, `Dirt`), drawn with dashed
    /// sidelines and without painted centreline stripes.
    pub unpaved: bool,
    /// True direction of landing along the centreline, radians.
    pub course_rad: f32,
    /// Length from the threshold to the far end, metres.
    pub length_m: f32,
    /// Width, metres.
    pub width_m: f32,
    /// Glide path angle below the horizontal, radians.
    pub glide_path_rad: f32,
    /// Trust in the geometry; only `Valid` geometry is drawn.
    pub status: SignalStatus,
}

impl Runway {
    pub(crate) fn usable(&self) -> bool {
        self.status == SignalStatus::Valid
            && self.threshold_m.iter().all(|v| v.is_finite())
            && self.far_down_offset_m.is_finite()
            && self.far_down_offset_m.abs() <= 500.0
            && self.pre_threshold_m.is_finite()
            && (0.0..=3000.0).contains(&self.pre_threshold_m)
            && self.reciprocal_displaced_m.is_finite()
            && (0.0..=3000.0).contains(&self.reciprocal_displaced_m)
            && self.course_rad.is_finite()
            && (100.0..=6000.0).contains(&self.length_m)
            && (7.0..=150.0).contains(&self.width_m)
            && (0.01..=0.2).contains(&self.glide_path_rad)
    }

    pub(crate) fn primary_length_m(&self) -> f32 {
        let recip = self.reciprocal_displaced_m.max(0.0);
        (self.length_m - recip).clamp(60.0_f32.min(self.length_m), self.length_m)
    }

    /// Unit vectors along the landing direction and to its right, north/east/down.
    pub(crate) fn axes(&self) -> ([f32; 3], [f32; 3]) {
        let (s, c) = (libm::sinf(self.course_rad), libm::cosf(self.course_rad));
        ([c, s, 0.0], [-s, c, 0.0])
    }
}
