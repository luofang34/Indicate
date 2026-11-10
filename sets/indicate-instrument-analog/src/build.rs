//! The physical build: what a 3D backend makes a real gauge from.
//!
//! A 3D backend does not draw the dial's scene each frame. It
//! rasterises the face once per lighting ([`crate::draw_face_scene`])
//! into a texture, extrudes each pointer outline into a thin needle at
//! its stack height, builds each drum from its aperture and digit
//! strip, extrudes each card's and overlay's shapes into a thin plate at
//! its stack height, and builds each tape as a drum turning about a
//! vertical axis behind its window. Per frame it calls [`crate::pose`]
//! and applies one rotation per needle, one offset per wheel, one turn
//! and one slide per card, and one turn per tape drum. This module
//! states, in millimetres in the dial frame, everything that requires.
//! Card and overlay shapes are borrowed from the spec in face units;
//! multiply by [`PhysicalBuild::face_radius_mm`] for millimetres.

use indicate_instrument_scene::LayerId;

use crate::moving::{DIGIT_PITCH_SHARE, SEPARATOR_SHARE};
use crate::outline::{PointerOutline, pointer_outline};
use crate::spec::{
    AnalogDialSpec, CardTurn, Channel, MAX_CARDS, MAX_POINTERS, MAX_TAPES, MAX_WINDOWS, Mark,
    OverlaySpec, Paint, ScaleLaw, SpecError, TapeSense, TapeSpec,
};

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
    /// The face slot the digit strip is lettered in.
    pub face: indicate_instrument_scene::FaceSlot,
}

/// One card, ready to build.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CardBuild<'a> {
    /// Its shapes in face units about the pivot, at no turn and no
    /// travel.
    pub shapes: &'a [Mark<'a>],
    /// Pivot in the dial frame, millimetres from the face centre.
    pub pivot_mm: [f32; 2],
    /// How it turns, the same law [`crate::pose`] evaluates.
    pub turn: Option<CardTurn<'a>>,
    /// Travel stops, millimetres along the turned card's vertical axis;
    /// [`crate::CardPose::offset`] times the face radius is the travel.
    pub travel_limit_mm: Option<[f32; 2]>,
    /// Height of its plane above the face, millimetres.
    pub stack_height_mm: f32,
}

/// One tape as a drum behind its window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TapeBuild<'a> {
    /// The spec it is built from, for its marks and numerals.
    pub tape: &'a TapeSpec<'a>,
    /// Window `[x, y, width, height]` in the dial frame, millimetres.
    pub aperture_mm: [f32; 4],
    /// Drum radius, millimetres: one period of card round it, so a
    /// value `v` stands `v × pitch` of arc from the reading point.
    pub drum_radius_mm: f32,
    /// Which way the drum's values run past the window. A drum turns
    /// by `TapePose::value / period` of a turn.
    pub sense: TapeSense,
    /// The face slot its numerals are lettered in: the dial's numeral
    /// slot.
    pub face: indicate_instrument_scene::FaceSlot,
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
    /// Cards, in [`AnalogDialSpec::cards`] order.
    pub cards: [Option<CardBuild<'a>>; MAX_CARDS],
    /// Tapes, in [`AnalogDialSpec::tapes`] order.
    pub tapes: [Option<TapeBuild<'a>>; MAX_TAPES],
    /// Fixed parts in front of the moving parts, in face units.
    pub overlays: &'a [OverlaySpec<'a>],
    /// The face slots the dial is lettered in; the host binds them to
    /// faces and styles for both the face texture and the drum strips.
    pub lettering: crate::spec::Lettering,
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
            face: spec.lettering.numerals,
        });
    }
    let mut cards = [None; MAX_CARDS];
    for (slot, card) in cards.iter_mut().zip(spec.cards) {
        *slot = Some(CardBuild {
            shapes: card.shapes,
            pivot_mm: [card.pivot[0] * mm, card.pivot[1] * mm],
            turn: card.turn,
            travel_limit_mm: card.travel.map(|t| [t.limit[0] * mm, t.limit[1] * mm]),
            stack_height_mm: card.stack_height_mm,
        });
    }
    let mut tapes = [None; MAX_TAPES];
    for (slot, tape) in tapes.iter_mut().zip(spec.tapes) {
        let [x, y, w, h] = tape.aperture;
        *slot = Some(TapeBuild {
            tape,
            aperture_mm: [x * mm, y * mm, w * mm, h * mm],
            drum_radius_mm: tape.pitch * tape.period / core::f32::consts::TAU * mm,
            sense: tape.sense,
            face: spec.lettering.numerals,
        });
    }
    Ok(PhysicalBuild {
        case_diameter_mm: spec.case_diameter_mm,
        face_radius_mm: mm,
        face_layers: FACE_LAYERS,
        pointers,
        windows,
        cards,
        tapes,
        overlays: spec.overlays,
        lettering: spec.lettering,
    })
}
