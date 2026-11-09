//! Where every moving part of a dial stands, from resolved state or
//! from raw values.
//!
//! This is the whole per-frame cost of a 3D gauge: [`reading`] picks
//! the dial's two values out of [`PanelData`], and [`pose`] turns them
//! into one angle per pointer and one offset per drum wheel. Nothing
//! here draws or decodes a scene.

use indicate_instrument_state::{EngineSamples, PanelData};

use crate::law::wheel_positions;
use crate::spec::{
    AnalogDialSpec, Channel, FailurePose, MAX_POINTERS, MAX_WHEELS, MAX_WINDOWS, WindowSpec,
};

/// The dial's two values in dial units; `None` where the state shows
/// no value.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DialReading {
    /// The primary channel.
    pub primary: Option<f32>,
    /// The secondary channel.
    pub secondary: Option<f32>,
}

impl DialReading {
    /// The value `channel` reads.
    #[must_use]
    pub fn of(&self, channel: Channel) -> Option<f32> {
        match channel {
            Channel::Primary => self.primary,
            Channel::Secondary => self.secondary,
        }
    }
}

/// Reads `spec`'s channels from resolved state and the engine samples
/// beside it, in dial units.
#[must_use]
pub fn reading(
    spec: &AnalogDialSpec<'_>,
    data: &PanelData,
    engines: &EngineSamples<'_>,
) -> DialReading {
    DialReading {
        primary: spec.primary.read(data, engines),
        secondary: spec
            .secondary
            .and_then(|channel| channel.read(data, engines)),
    }
}

/// One pointer's pose.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PointerPose {
    /// Rotation about the pivot, radians clockwise from twelve o'clock:
    /// the one number a 3D backend applies to the extruded needle.
    pub angle_rad: f32,
    /// Whether the pointer is drawn at all.
    pub visible: bool,
    /// Whether the pointer shows a value, rather than resting at its
    /// failure pose.
    pub live: bool,
}

/// One drum window's pose.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct WindowPose {
    /// Each wheel's position in digits, rightmost first; see
    /// [`crate::wheel_positions`].
    pub wheels: [f32; MAX_WHEELS],
    /// Whether the drum shows a value. A drum without one shows no
    /// digits, because a digit would claim a value the state lacks.
    pub live: bool,
}

/// Every moving part of one dial.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DialPose {
    /// Pointer poses, in [`AnalogDialSpec::pointers`] order.
    pub pointers: [PointerPose; MAX_POINTERS],
    /// Window poses, in [`AnalogDialSpec::windows`] order.
    pub windows: [WindowPose; MAX_WINDOWS],
    /// Whether the failure flag shows: some moving part has no value.
    pub flag: bool,
}

/// Poses every moving part of `spec` for `reading`.
///
/// `spec` is assumed to have passed [`AnalogDialSpec::check`]; parts
/// past the fixed capacities are not posed.
#[must_use]
pub fn pose(spec: &AnalogDialSpec<'_>, reading: &DialReading) -> DialPose {
    let mut out = DialPose::default();
    let primary = reading
        .primary
        .map(|v| clamp_to(v, spec.input_range))
        .filter(|v| v.is_finite());
    let value = |channel: Channel| match channel {
        Channel::Primary => primary,
        Channel::Secondary => reading.secondary.filter(|v| v.is_finite()),
    };
    for (slot, pointer) in out.pointers.iter_mut().zip(spec.pointers) {
        *slot = match (value(pointer.channel), pointer.on_failure) {
            (Some(v), _) => PointerPose {
                angle_rad: pointer.law.angle_rad(v),
                visible: true,
                live: true,
            },
            (None, FailurePose::Park { angle_deg }) => PointerPose {
                angle_rad: angle_deg.to_radians(),
                visible: true,
                live: false,
            },
            (None, FailurePose::Hide) => PointerPose::default(),
        };
        out.flag |= !slot.live;
    }
    for (slot, window) in out.windows.iter_mut().zip(spec.windows) {
        *slot = window_pose(window, value(window.channel));
        out.flag |= !slot.live;
    }
    out
}

fn clamp_to(value: f32, range: Option<[f32; 2]>) -> f32 {
    match range {
        Some([lo, hi]) if lo < hi => value.clamp(lo, hi),
        _ => value,
    }
}

fn window_pose(window: &WindowSpec<'_>, value: Option<f32>) -> WindowPose {
    let Some(v) = value else {
        return WindowPose::default();
    };
    // Rounded to a thousandth of a digit, so 29.92 does not show its
    // rightmost wheel a hair short of 2 because 29.92 has no exact f32.
    let shown =
        libm::roundf(clamp_to(v, Some(window.range)) * window.value_scale * 1000.0) / 1000.0;
    WindowPose {
        wheels: wheel_positions::<MAX_WHEELS>(shown, usize::from(window.wheels)),
        live: true,
    }
}
