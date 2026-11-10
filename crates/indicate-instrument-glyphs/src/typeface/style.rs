//! How one face is lettered for one use: narrowing, pen, slant and
//! glyph alternates.
//!
//! A style is bound with its face, out of band, to a scene's face slot.
//! It changes how glyphs are drawn and advanced, never which glyphs a
//! scene names, so one stroke skeleton serves a condensed 1940 dial, the
//! round MS33558 forms and a maker's heavier variant.

use super::TypefaceError;

/// One use of a face.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextStyle {
    /// Horizontal scale of the glyph geometry and advances, at constant
    /// pen: below 1 narrows the lettering without thinning its strokes.
    pub condense: f32,
    /// Pen width as a fraction of the cap height, for stroke glyphs;
    /// `None` keeps the face's default pen. Outline glyphs ignore it.
    pub pen: Option<f32>,
    /// Horizontal shear per unit of height: positive leans right.
    pub slant: f32,
    /// Enabled glyph alternates: bit `tag - 1` enables the alternate
    /// with that tag (see [`super::Typeface::alternate_tag`]).
    pub alternates: u32,
}

impl TextStyle {
    /// The face as drawn: no narrowing, the default pen, upright, no
    /// alternates.
    pub const REGULAR: TextStyle = TextStyle {
        condense: 1.0,
        pen: None,
        slant: 0.0,
        alternates: 0,
    };

    /// Checks every parameter is a finite number in its range: condense
    /// in `[0.25, 4]`, pen in `(0, 0.5]`, slant in `[-1, 1]`.
    pub fn check(&self) -> Result<(), TypefaceError> {
        let bad = |field| Err(TypefaceError::InvalidStyle { field });
        if !(self.condense.is_finite() && (0.25..=4.0).contains(&self.condense)) {
            return bad("condense");
        }
        if let Some(pen) = self.pen
            && !(pen.is_finite() && pen > 0.0 && pen <= 0.5)
        {
            return bad("pen");
        }
        if !(self.slant.is_finite() && (-1.0..=1.0).contains(&self.slant)) {
            return bad("slant");
        }
        Ok(())
    }
}

impl Default for TextStyle {
    fn default() -> Self {
        Self::REGULAR
    }
}
