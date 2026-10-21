//! Aircraft or explicitly track-level reference for a collimated virtual instrument panel.
use super::{HeadingReference, PanelData, live};
use indicate_frames::Quat;

/// Local east/north/up axes; independent of the observer's head and either eye.
#[derive(Clone, Copy, Default)]
#[repr(C)]
pub struct ViewReference {
    /// 0: unavailable; 1: level track reference; 2: measured aircraft attitude and heading.
    pub kind: u32,
    /// Horizontal panel axis.
    pub right: [f32; 3],
    /// Vertical panel axis.
    pub up: [f32; 3],
    /// Panel forward axis.
    pub forward: [f32; 3],
}

/// Resolves a panel reference without substituting track for measured aircraft attitude.
pub fn view_reference(data: &PanelData) -> ViewReference {
    let aircraft = live(data.heading.value_rad)
        && matches!(
            data.heading.reference,
            HeadingReference::True | HeadingReference::SimLocalTrue
        )
        && live(data.pitch_rad)
        && live(data.roll_rad);
    if !aircraft && !live(data.track_rad) {
        return ViewReference::default();
    }
    let (heading, pitch, roll) = if aircraft {
        (
            data.heading.value_rad.value,
            data.pitch_rad.value,
            data.roll_rad.value,
        )
    } else {
        (data.track_rad.value, 0.0, 0.0)
    };
    if ![heading, pitch, roll].iter().all(|v| v.is_finite()) {
        return ViewReference::default();
    }
    let rotation = Quat::from_euler(roll, pitch, heading);
    let enu = |body| {
        let [north, east, down] = rotation.rotate(body);
        [east as f32, north as f32, -down as f32]
    };
    ViewReference {
        kind: if aircraft { 2 } else { 1 },
        right: enu([0.0, 1.0, 0.0]),
        up: enu([0.0, 0.0, -1.0]),
        forward: enu([1.0, 0.0, 0.0]),
    }
}

#[cfg(test)]
mod tests;
