//! Maps aircraft-relative directions to frame coordinates through the host camera.
use crate::context::HudProjection;
use indicate_instrument_descriptor::DesignFrame;
use indicate_instrument_state::{HeadingReference, PanelData, Sig, SignalStatus};

/// A position in the logical frame, y down.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenPoint {
    /// Horizontal position.
    pub x: f32,
    /// Vertical position, increasing downward.
    pub y: f32,
}

type Matrix = [[f32; 3]; 3];

/// A pinhole camera on the body x axis, with optional attitude bases.
#[derive(Debug, Clone, Copy)]
pub struct Camera {
    centre: ScreenPoint,
    focal: f32,
    roll: Option<f32>,
    pitch: Option<f32>,
    true_heading: Option<f32>,
}

/// Directions this close to the image plane are behind the viewer.
const MIN_FORWARD: f32 = 0.02;

impl Camera {
    /// Builds a camera; the attitude bases exist only for live, finite signals.
    pub fn new(data: &PanelData, projection: HudProjection, frame: DesignFrame) -> Self {
        let half = libm::tanf(projection.vertical_fov_rad * 0.5);
        let true_heading = matches!(
            data.heading.reference,
            HeadingReference::True | HeadingReference::SimLocalTrue
        )
        .then_some(data.heading.value_rad)
        .and_then(live);
        Self {
            centre: ScreenPoint {
                x: frame.width * 0.5,
                y: frame.height * 0.5,
            },
            focal: frame.height * 0.5 / half,
            roll: live(data.roll_rad),
            pitch: live(data.pitch_rad),
            true_heading,
        }
    }

    /// The boresight position, which is the frame centre.
    pub fn centre(&self) -> ScreenPoint {
        self.centre
    }

    /// Pixels per unit tangent along the image axes.
    pub fn focal(&self) -> f32 {
        self.focal
    }

    /// Whether roll and pitch are both live.
    pub fn level_basis(&self) -> bool {
        self.roll.is_some() && self.pitch.is_some()
    }

    /// Whether north/east/down vectors can be placed: attitude and true heading are live.
    pub fn world_basis(&self) -> bool {
        self.level_basis() && self.true_heading.is_some()
    }

    /// Projects a body-axis direction (forward, right, down); `None` behind the viewer.
    pub fn project_body(&self, v: [f32; 3]) -> Option<ScreenPoint> {
        let length = libm::sqrtf(v[0] * v[0] + v[1] * v[1] + v[2] * v[2]);
        if !length.is_finite() || length <= 0.0 || v[0] < MIN_FORWARD * length {
            return None;
        }
        let point = ScreenPoint {
            x: self.centre.x + self.focal * v[1] / v[0],
            y: self.centre.y + self.focal * v[2] / v[0],
        };
        (point.x.is_finite() && point.y.is_finite()).then_some(point)
    }

    /// The unit screen direction from the boresight toward a body direction, also behind the viewer.
    pub fn screen_direction(&self, v: [f32; 3]) -> Option<[f32; 2]> {
        let length = libm::sqrtf(v[1] * v[1] + v[2] * v[2]);
        let total = libm::sqrtf(v[0] * v[0] + length * length);
        (length.is_finite() && length > 1e-4 * total).then(|| [v[1] / length, v[2] / length])
    }

    /// Rotates a north/east/down vector into body axes.
    pub fn ned_to_body(&self, v: [f32; 3]) -> Option<[f32; 3]> {
        let heading = self.true_heading?;
        Some(apply(&self.body_from(heading)?, v))
    }

    /// Rotates a body vector into north/east/down; the inverse of [`Self::ned_to_body`].
    pub fn body_to_ned(&self, v: [f32; 3]) -> Option<[f32; 3]> {
        let m = self.body_from(self.true_heading?)?;
        Some([
            m[0][0] * v[0] + m[1][0] * v[1] + m[2][0] * v[2],
            m[0][1] * v[0] + m[1][1] * v[1] + m[2][1] * v[2],
            m[0][2] * v[0] + m[1][2] * v[1] + m[2][2] * v[2],
        ])
    }

    /// Rotates a vector from a level frame whose x axis points along
    /// `azimuth_from_heading` (radians right of the aircraft heading).
    pub fn level_to_body(&self, azimuth_from_heading: f32, v: [f32; 3]) -> Option<[f32; 3]> {
        Some(apply(&self.body_from(-azimuth_from_heading)?, v))
    }

    /// Projects a north/east/down vector; `None` behind the viewer or without a world basis.
    pub fn project_ned(&self, v: [f32; 3]) -> Option<ScreenPoint> {
        self.project_body(self.ned_to_body(v)?)
    }

    /// The radius in pixels of a cone of the given half-angle around the boresight.
    pub fn cone_radius(&self, half_angle_rad: f32) -> f32 {
        self.focal * libm::tanf(half_angle_rad)
    }

    /// The true heading, when known.
    pub fn true_heading(&self) -> Option<f32> {
        self.true_heading
    }

    fn body_from(&self, yaw: f32) -> Option<Matrix> {
        let (sr, cr) = (libm::sinf(self.roll?), libm::cosf(self.roll?));
        let (sp, cp) = (libm::sinf(self.pitch?), libm::cosf(self.pitch?));
        let (sy, cy) = (libm::sinf(yaw), libm::cosf(yaw));
        Some([
            [cp * cy, cp * sy, -sp],
            [sr * sp * cy - cr * sy, sr * sp * sy + cr * cy, sr * cp],
            [cr * sp * cy + sr * sy, cr * sp * sy - sr * cy, cr * cp],
        ])
    }
}

fn apply(m: &Matrix, v: [f32; 3]) -> [f32; 3] {
    let row = |r: &[f32; 3]| r[0] * v[0] + r[1] * v[1] + r[2] * v[2];
    [row(&m[0]), row(&m[1]), row(&m[2])]
}

pub(crate) fn live(signal: Sig<f32>) -> Option<f32> {
    (signal.status == SignalStatus::Valid
        && signal.value.is_finite()
        && signal.value.abs() < 1_000_000.0)
        .then_some(signal.value)
}

#[cfg(test)]
mod tests;
