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
            && self.course_rad.is_finite()
            && (100.0..=6000.0).contains(&self.length_m)
            && (10.0..=150.0).contains(&self.width_m)
            && (0.01..=0.2).contains(&self.glide_path_rad)
    }

    /// Unit vectors along the landing direction and to its right, north/east/down.
    pub(crate) fn axes(&self) -> ([f32; 3], [f32; 3]) {
        let (s, c) = (libm::sinf(self.course_rad), libm::cosf(self.course_rad));
        ([c, s, 0.0], [-s, c, 0.0])
    }
}
