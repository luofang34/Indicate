//! A face as a backend draws it: the reference 5×7 pack, or a typeface
//! with a style. Per-face metrics live here: where each glyph of a run
//! sits, how far the run advances, and where its ink can reach.

use super::geometry::{GlyphXform, OutlineEdges, StrokeSegments};
use super::{GlyphKind, GlyphView, TextStyle, Typeface, TypefaceError};
use crate::glyph::{ADVANCE, CELL_H, CELL_W};

/// A face bound to a scene's face slot.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Face<'a> {
    /// The built-in 5×7 bitmap pack ([`crate::PANEL_GLYPHS`]): the
    /// default face, with the scene crate's nominal metrics.
    Reference,
    /// A loaded typeface, lettered in a style.
    Typeface {
        /// The face.
        face: Typeface<'a>,
        /// How it is lettered.
        style: TextStyle,
    },
}

/// A rectangle relative to a run's pen origin on its baseline, in the
/// scene's y-down output units: ink above the baseline has negative y.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InkBox {
    /// Left edge.
    pub min_x: f32,
    /// Top edge.
    pub min_y: f32,
    /// Right edge.
    pub max_x: f32,
    /// Bottom edge.
    pub max_y: f32,
}

impl InkBox {
    fn union(self, other: InkBox) -> InkBox {
        InkBox {
            min_x: self.min_x.min(other.min_x),
            min_y: self.min_y.min(other.min_y),
            max_x: self.max_x.max(other.max_x),
            max_y: self.max_y.max(other.max_y),
        }
    }

    fn around(p: [f32; 2], pad: f32) -> InkBox {
        InkBox {
            min_x: p[0] - pad,
            min_y: p[1] - pad,
            max_x: p[0] + pad,
            max_y: p[1] + pad,
        }
    }
}

/// One glyph of a laid-out run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlacedGlyph<'a> {
    /// The character.
    pub ch: char,
    /// The glyph, or `None` in the reference face, whose bitmap the
    /// backend reads from [`crate::PANEL_GLYPHS`].
    pub glyph: Option<GlyphView<'a>>,
    /// Font units to output space, with the origin at this glyph's pen
    /// position on the run's baseline (the run starts at x = 0).
    pub xform: GlyphXform,
    /// Pen width for a stroke glyph, output units.
    pub pen: f32,
    /// Output units per glyph-cell pixel in the reference face.
    pub cell: f32,
}

impl<'a> Face<'a> {
    /// A typeface lettered in `style`, after checking the style.
    pub fn typeface(face: Typeface<'a>, style: TextStyle) -> Result<Self, TypefaceError> {
        style.check()?;
        Ok(Self::Typeface { face, style })
    }

    /// Lays out `text` at `size` (the cap height, output units), calling
    /// `place` for each glyph in order; returns the run's advance width.
    /// A character the face lacks fails the whole run.
    pub fn layout(
        &self,
        size: f32,
        text: &str,
        mut place: impl FnMut(&PlacedGlyph<'a>),
    ) -> Result<f32, TypefaceError> {
        match self {
            Face::Reference => {
                let cell = size / CELL_H as f32;
                let advance = f32::from(ADVANCE) * cell;
                let mut pen_x = 0.0;
                for ch in text.chars() {
                    crate::PANEL_GLYPHS
                        .glyph(ch)
                        .map_err(|_| TypefaceError::MissingGlyph { ch })?;
                    place(&PlacedGlyph {
                        ch,
                        glyph: None,
                        xform: reference_xform(cell, pen_x),
                        pen: 0.0,
                        cell,
                    });
                    pen_x += advance;
                }
                Ok(pen_x)
            }
            Face::Typeface { face, style } => layout_typeface(face, style, size, text, place),
        }
    }

    /// The run's advance width: where a following run would start.
    pub fn advance_width(&self, size: f32, text: &str) -> Result<f32, TypefaceError> {
        self.layout(size, text, |_| {})
    }

    /// Where `placed` can paint, relative to the run's origin; `None`
    /// for a glyph with no ink. Stroke ink includes half the pen and
    /// the arc flattening `tolerance`.
    #[must_use]
    pub fn glyph_ink(&self, placed: &PlacedGlyph<'a>, tolerance: f32) -> Option<InkBox> {
        let Some(glyph) = placed.glyph else {
            // A reference cell paints within its 5×7 pixel box.
            let x0 = placed.xform.origin[0];
            return Some(InkBox {
                min_x: x0,
                min_y: -(CELL_H as f32) * placed.cell,
                max_x: x0 + CELL_W as f32 * placed.cell,
                max_y: 0.0,
            });
        };
        let pad = placed.pen * 0.5 + tolerance.max(0.0);
        let mut ink: Option<InkBox> = None;
        let mut add = |p: [f32; 2], pad: f32| {
            let b = InkBox::around(p, pad);
            ink = Some(ink.map_or(b, |i| i.union(b)));
        };
        match glyph.kind {
            GlyphKind::Stroke => {
                for [a, b] in StrokeSegments::new(&glyph, placed.xform, tolerance) {
                    add(a, pad);
                    add(b, pad);
                }
            }
            GlyphKind::Outline => {
                for [a, _] in OutlineEdges::new(&glyph, placed.xform) {
                    add(a, 0.0);
                }
            }
        }
        ink
    }

    /// Where the whole run can paint, relative to its origin; `None`
    /// for a run with no ink.
    pub fn ink_box(
        &self,
        size: f32,
        text: &str,
        tolerance: f32,
    ) -> Result<Option<InkBox>, TypefaceError> {
        let mut ink: Option<InkBox> = None;
        self.layout(size, text, |placed| {
            if let Some(b) = self.glyph_ink(placed, tolerance) {
                ink = Some(ink.map_or(b, |i| i.union(b)));
            }
        })?;
        Ok(ink)
    }
}

fn reference_xform(cell: f32, pen_x: f32) -> GlyphXform {
    GlyphXform {
        scale: cell,
        condense: 1.0,
        slant: 0.0,
        origin: [pen_x, 0.0],
    }
}

fn layout_typeface<'a>(
    face: &Typeface<'a>,
    style: &TextStyle,
    size: f32,
    text: &str,
    mut place: impl FnMut(&PlacedGlyph<'a>),
) -> Result<f32, TypefaceError> {
    let scale = size / f32::from(face.cap_height());
    let pen = style
        .pen
        .map_or(f32::from(face.default_pen()) * scale, |p| p * size);
    // A heavier pen widens each glyph by the extra ink on both sides,
    // so the side bearings keep their clear space.
    let pen_growth = pen - f32::from(face.default_pen()) * scale;
    let mut pen_x = 0.0;
    let mut previous: Option<char> = None;
    for ch in text.chars() {
        let glyph = face.glyph(ch, style.alternates)?;
        if let Some(left) = previous {
            pen_x += f32::from(face.kerning(left, ch)) * style.condense * scale;
        }
        place(&PlacedGlyph {
            ch,
            glyph: Some(glyph),
            xform: GlyphXform {
                scale,
                condense: style.condense,
                slant: style.slant,
                origin: [pen_x + pen_growth * 0.5, 0.0],
            },
            pen,
            cell: 0.0,
        });
        pen_x += f32::from(glyph.advance) * style.condense * scale + pen_growth;
        previous = Some(ch);
    }
    Ok(pen_x)
}

/// The host's binding of face slots to faces: slot `k` is `slots[k]`.
/// A slot past the table, and every slot of an empty table, draws in
/// slot 0's binding, or in [`Face::Reference`] when slot 0 is unbound
/// too. A host that binds nothing therefore letters every scene in the
/// default face.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FaceTable<'a> {
    slots: &'a [Face<'a>],
}

impl<'a> FaceTable<'a> {
    /// No bindings: every slot draws in the reference face.
    pub const DEFAULT: FaceTable<'static> = FaceTable { slots: &[] };

    /// Binds `slots[k]` to slot `k`.
    #[must_use]
    pub const fn new(slots: &'a [Face<'a>]) -> Self {
        Self { slots }
    }

    /// The face slot `slot` draws in.
    #[must_use]
    pub fn face(&self, slot: u8) -> Face<'a> {
        self.slots
            .get(usize::from(slot))
            .or_else(|| self.slots.first())
            .copied()
            .unwrap_or(Face::Reference)
    }
}
