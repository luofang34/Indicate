//! Unit and frame conversions from X-Plane's vocabulary into the
//! state's SI, radians, and NED.

use core::f32::consts::{PI, TAU};

/// Knots to meters per second.
pub const MPS_PER_KT: f32 = 0.514_444_4;
/// Feet to meters.
pub const M_PER_FT: f32 = 0.304_8;
/// Inches of mercury to hectopascals.
pub const HPA_PER_INHG: f32 = 33.863_9;
/// Feet per minute to meters per second.
pub const MPS_PER_FPM: f32 = 0.005_08;

/// Degrees to radians.
pub fn deg_to_rad(deg: f32) -> f32 {
    deg.to_radians()
}

/// Radians to degrees.
pub fn rad_to_deg(rad: f32) -> f32 {
    rad.to_degrees()
}

/// Wraps an angle into `[0, 2π)`.
pub fn wrap_heading(rad: f32) -> f32 {
    let wrapped = rad % TAU;
    if wrapped < 0.0 {
        wrapped + TAU
    } else {
        wrapped
    }
}

/// Wraps a signed angle into `(-π, π]`.
pub fn wrap_signed(rad: f32) -> f32 {
    let mut wrapped = rad % TAU;
    if wrapped > PI {
        wrapped -= TAU;
    } else if wrapped <= -PI {
        wrapped += TAU;
    }
    wrapped
}

/// X-Plane's local OpenGL frame (x east, y up, z south) into NED.
pub fn opengl_to_ned(xyz: [f32; 3]) -> [f32; 3] {
    [-xyz[2], xyz[0], -xyz[1]]
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn headings_wrap_into_a_full_turn_and_signed_angles_into_a_half_turn() {
        assert!((wrap_heading(-0.5) - (TAU - 0.5)).abs() < 1e-6);
        assert!((wrap_heading(TAU + 0.25) - 0.25).abs() < 1e-6);
        assert!((wrap_signed(PI + 0.5) - (-PI + 0.5)).abs() < 1e-6);
        assert!((wrap_signed(-PI - 0.5) - (PI - 0.5)).abs() < 1e-6);
    }

    #[test]
    fn the_opengl_frame_maps_east_up_south_onto_north_east_down() {
        // One meter east, two up, three south.
        assert_eq!(opengl_to_ned([1.0, 2.0, 3.0]), [-3.0, 1.0, -2.0]);
    }
}
