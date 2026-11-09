//! The physical build: what a 3D backend makes a real gauge from.
//!
//! A 3D backend does not draw the dial's scene each frame. It
//! rasterises the face once per lighting ([`crate::draw_face_scene`])
//! into a texture, extrudes each pointer outline into a thin needle at
//! its stack height, and builds each drum from its aperture and digit
//! strip. Per frame it calls [`crate::pose`] and applies one rotation
//! per needle and one offset per wheel. This module states, in
//! millimetres in the dial frame, everything that requires.

use indicate_instrument_scene::LayerId;

use crate::moving::{DIGIT_PITCH_SHARE, SEPARATOR_SHARE};
use crate::outline::{PointerOutline, pointer_outline};
use crate::spec::{AnalogDialSpec, Channel, MAX_POINTERS, MAX_WINDOWS, Paint, ScaleLaw, SpecError};

/// One pointer, ready to extrude.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointerBuild<'a> {
    /// Silhouette in millimetres, pivot at the origin, pointing at
    /// twelve o'clock (`-y`) at zero rotation.
    pub outline_mm: PointerOutline,
    /// Pivot in the dial frame, millimetres from the face centre.
    pub pivot_mm: [f32; 2],
    /// Hub cap radius, millimetres; zero for none.
    pub hub_radius_mm: f32,
    /// Height of the needle's plane above the face, millimetres.
    pub stack_height_mm: f32,
    /// Its paint, with its luminous glow if it has one.
    pub paint: Paint,
    /// The channel that drives it.
    pub channel: Channel,
    /// Its value-to-angle law, the same one [`crate::pose`] evaluates.
    pub law: ScaleLaw<'a>,
}

/// One drum window, ready to build.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowBuild {
    /// Aperture `[x, y, width, height]` in the dial frame, millimetres.
    pub aperture_mm: [f32; 4],
    /// Digit wheels, rightmost first.
    pub wheels: u8,
    /// Width of one wheel, millimetres.
    pub wheel_width_mm: f32,
    /// Distance between adjacent digits on a wheel's strip, millimetres.
    /// A wheel at position `p` shows digit `floor(p)` offset by
    /// `(p - floor(p)) * digit_pitch_mm` toward the next.
    pub digit_pitch_mm: f32,
    /// Digit height, millimetres.
    pub digit_height_mm: f32,
    /// The channel that drives it.
    pub channel: Channel,
    /// Digit paint.
    pub paint: Paint,
}

/// Everything a 3D backend needs beyond the face texture.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhysicalBuild<'a> {
    /// Case outside diameter, millimetres.
    pub case_diameter_mm: f32,
    /// Face radius, millimetres.
    pub face_radius_mm: f32,
    /// The scene bands [`crate::draw_face_scene`] emits, as
    /// [`LayerId`] bits. The full dial scene opens the same bands, but
    /// its `Tapes` band also holds the moving parts, so a backend takes
    /// the face from the face-only scene, not by filtering the full one.
    pub face_layers: u16,
    /// Pointers, in [`AnalogDialSpec::pointers`] order.
    pub pointers: [Option<PointerBuild<'a>>; MAX_POINTERS],
    /// Drum windows, in [`AnalogDialSpec::windows`] order.
    pub windows: [Option<WindowBuild>; MAX_WINDOWS],
}

/// Bits of the bands the face-only scene emits.
pub const FACE_LAYERS: u16 = (1 << LayerId::Background.to_u8()) | (1 << LayerId::Tapes.to_u8());

/// The physical build of `spec`, after checking it.
pub fn physical_build<'a>(spec: &AnalogDialSpec<'a>) -> Result<PhysicalBuild<'a>, SpecError> {
    spec.check()?;
    let mm = spec.face_radius_mm;
    let mut pointers = [None; MAX_POINTERS];
    for (slot, pointer) in pointers.iter_mut().zip(spec.pointers) {
        *slot = Some(PointerBuild {
            outline_mm: pointer_outline(&pointer.style).scaled(mm),
            pivot_mm: [pointer.pivot[0] * mm, pointer.pivot[1] * mm],
            hub_radius_mm: pointer.style.hub_radius * mm,
            stack_height_mm: pointer.stack_height_mm,
            paint: pointer.style.paint,
            channel: pointer.channel,
            law: pointer.law,
        });
    }
    let mut windows = [None; MAX_WINDOWS];
    for (slot, window) in windows.iter_mut().zip(spec.windows) {
        let [x, y, w, h] = window.aperture;
        let gap = if window.separator.is_some() {
            SEPARATOR_SHARE
        } else {
            0.0
        };
        let wheel_w = w / (f32::from(window.wheels.max(1)) + gap);
        *slot = Some(WindowBuild {
            aperture_mm: [x * mm, y * mm, w * mm, h * mm],
            wheels: window.wheels,
            wheel_width_mm: wheel_w * mm,
            digit_pitch_mm: h * DIGIT_PITCH_SHARE * mm,
            digit_height_mm: window.digit_size * mm,
            channel: window.channel,
            paint: window.paint,
        });
    }
    Ok(PhysicalBuild {
        case_diameter_mm: spec.case_diameter_mm,
        face_radius_mm: mm,
        face_layers: FACE_LAYERS,
        pointers,
        windows,
    })
}
