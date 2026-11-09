//! Which faces a dial is lettered in, and legends that run along an arc.
//!
//! A dial names its faces by scene face slot, never by typeface. The
//! host binds each slot to a typeface and a style, the same binding for
//! the 2D scene ([`crate::draw_dial`]) and the 3D build
//! ([`crate::physical_build`]), so the two cannot letter one dial
//! differently. A cockpit that letters its airspeed indicator in the
//! narrow 1940 Kollsman style and its oil gauge in round MS33558 forms
//! binds two slots to the same face with two styles.

use indicate_instrument_scene::{ArcSense, FaceSlot};

use super::Paint;

/// The face slots a dial's lettering uses. Slot 0, the default face,
/// emits no face change, so a dial lettered in it draws exactly as an
/// unlettered one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Lettering {
    /// Scale numerals and drum digits.
    pub numerals: FaceSlot,
    /// Printed legends, straight or along an arc.
    pub legends: FaceSlot,
}

impl Lettering {
    /// Everything in the default face.
    pub const DEFAULT: Lettering = Lettering {
        numerals: FaceSlot::DEFAULT,
        legends: FaceSlot::DEFAULT,
    };
}

/// A printed legend that follows the face's curve, such as "LEFT ENGINE
/// RIGHT" round the bottom of a dual gauge. The backend places its
/// glyphs from the bound face's metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArcLegend<'a> {
    /// What is printed, from the panel glyph vocabulary.
    pub text: &'a str,
    /// Radius of its baseline, face units.
    pub radius: f32,
    /// Where its middle sits, degrees clockwise from twelve o'clock.
    pub angle_deg: f32,
    /// Reading direction: clockwise along the top of a dial, counter-
    /// clockwise along the bottom so it still reads left to right.
    pub sense: ArcSense,
    /// Its size (cap height), face units.
    pub size: f32,
    /// Its paint.
    pub paint: Paint,
}
