//! Glyph geometry in output space: flattened centreline segments for a
//! stroke glyph, contour edges for an outline glyph.
//!
//! Font space is y-up with the baseline at 0 and the pen origin at
//! x = 0. Output space is the scene's: y-down, in the run's units, with
//! the glyph's pen origin where the caller puts it. Arcs flatten to a
//! chord tolerance in output units, with a segment count that is a pure
//! function of the arc and the tolerance, so every backend that asks
//! for the same glyph at the same size gets the same segments.

use super::format::{Reader, SEG_ARC, SEG_LINE};
use super::{GlyphKind, GlyphView};

/// Most segments one arc flattens to, whatever the tolerance.
const MAX_ARC_SEGMENTS: u32 = 256;

/// How a glyph's font units map into output space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlyphXform {
    /// Output units per font unit.
    pub scale: f32,
    /// Horizontal scale of the geometry ([`super::TextStyle::condense`]).
    pub condense: f32,
    /// Horizontal shear per unit of height ([`super::TextStyle::slant`]).
    pub slant: f32,
    /// The glyph's pen origin on the baseline, output units.
    pub origin: [f32; 2],
}

impl GlyphXform {
    /// A font-space point in output space.
    #[must_use]
    pub fn map(&self, p: [f32; 2]) -> [f32; 2] {
        [
            self.origin[0] + (p[0] * self.condense + self.slant * p[1]) * self.scale,
            self.origin[1] - p[1] * self.scale,
        ]
    }
}

#[derive(Debug, Clone, Copy)]
struct ArcCursor {
    center: [f32; 2],
    radii: [f32; 2],
    start: f32,
    step: f32,
    k: u32,
    n: u32,
}

impl ArcCursor {
    fn point(&self, k: u32) -> [f32; 2] {
        let a = self.start + self.step * k as f32;
        [
            self.center[0] + self.radii[0] * libm::cosf(a),
            self.center[1] + self.radii[1] * libm::sinf(a),
        ]
    }
}

/// The flattened centreline segments of a stroke glyph, in output
/// space. A dot (a stroke with no segments) is one zero-length segment,
/// which a round pen draws as a disc.
#[derive(Debug, Clone)]
pub struct StrokeSegments<'a> {
    reader: Reader<'a>,
    xf: GlyphXform,
    tolerance: f32,
    strokes_left: u16,
    segments_left: u16,
    pen: [f32; 2],
    arc: Option<ArcCursor>,
}

impl<'a> StrokeSegments<'a> {
    /// The segments of `glyph` under `xf`, flattening arcs to
    /// `tolerance` output units. Empty for an outline glyph.
    #[must_use]
    pub fn new(glyph: &GlyphView<'a>, xf: GlyphXform, tolerance: f32) -> Self {
        let mut reader = Reader::new(glyph.geometry);
        let strokes_left = if glyph.kind == GlyphKind::Stroke {
            reader.u16().unwrap_or(0)
        } else {
            0
        };
        Self {
            reader,
            xf,
            tolerance,
            strokes_left,
            segments_left: 0,
            pen: [0.0; 2],
            arc: None,
        }
    }

    fn begin_arc(&mut self) -> Option<ArcCursor> {
        let center = self.reader.point()?;
        let (rx, ry) = (self.reader.u16()?, self.reader.u16()?);
        let (start, sweep) = (self.reader.i16()?, self.reader.i16()?);
        let radii = [f32::from(rx), f32::from(ry)];
        let sweep = f32::from(sweep).to_radians() / 10.0;
        let reach = (radii[0] * self.xf.condense.abs()).max(radii[1]) * self.xf.scale.abs();
        let max_step = if self.tolerance > 0.0 && self.tolerance < reach {
            2.0 * libm::acosf(1.0 - self.tolerance / reach)
        } else {
            core::f32::consts::FRAC_PI_2
        };
        let n = (libm::ceilf(sweep.abs() / max_step) as u32).clamp(1, MAX_ARC_SEGMENTS);
        Some(ArcCursor {
            center: [f32::from(center[0]), f32::from(center[1])],
            radii,
            start: f32::from(start).to_radians() / 10.0,
            step: sweep / n as f32,
            k: 0,
            n,
        })
    }
}

impl Iterator for StrokeSegments<'_> {
    type Item = [[f32; 2]; 2];

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(arc) = self.arc.as_mut() {
                if arc.k < arc.n {
                    let a = arc.point(arc.k);
                    arc.k = arc.k.wrapping_add(1);
                    let b = arc.point(arc.k);
                    self.pen = b;
                    return Some([self.xf.map(a), self.xf.map(b)]);
                }
                self.arc = None;
            }
            if self.segments_left > 0 {
                self.segments_left = self.segments_left.saturating_sub(1);
                match self.reader.u8()? {
                    SEG_LINE => {
                        let to = self.reader.point()?;
                        let to = [f32::from(to[0]), f32::from(to[1])];
                        let from = self.pen;
                        self.pen = to;
                        return Some([self.xf.map(from), self.xf.map(to)]);
                    }
                    SEG_ARC => {
                        let arc = self.begin_arc()?;
                        let start = arc.point(0);
                        self.arc = Some(arc);
                        if start != self.pen {
                            let from = self.pen;
                            self.pen = start;
                            return Some([self.xf.map(from), self.xf.map(start)]);
                        }
                        continue;
                    }
                    _ => return None,
                }
            }
            if self.strokes_left == 0 {
                return None;
            }
            self.strokes_left = self.strokes_left.saturating_sub(1);
            self.segments_left = self.reader.u16()?;
            let start = self.reader.point()?;
            self.pen = [f32::from(start[0]), f32::from(start[1])];
            if self.segments_left == 0 {
                let dot = self.xf.map(self.pen);
                return Some([dot, dot]);
            }
        }
    }
}

/// The edges of an outline glyph's contours, each contour closed, in
/// output space. Empty for a stroke glyph.
#[derive(Debug, Clone)]
pub struct OutlineEdges<'a> {
    reader: Reader<'a>,
    xf: GlyphXform,
    contours_left: u16,
    points_left: u16,
    first: [f32; 2],
    previous: [f32; 2],
}

impl<'a> OutlineEdges<'a> {
    /// The edges of `glyph` under `xf`.
    #[must_use]
    pub fn new(glyph: &GlyphView<'a>, xf: GlyphXform) -> Self {
        let mut reader = Reader::new(glyph.geometry);
        let contours_left = if glyph.kind == GlyphKind::Outline {
            reader.u16().unwrap_or(0)
        } else {
            0
        };
        Self {
            reader,
            xf,
            contours_left,
            points_left: 0,
            first: [0.0; 2],
            previous: [0.0; 2],
        }
    }

    fn next_point(&mut self) -> Option<[f32; 2]> {
        let p = self.reader.point()?;
        Some(self.xf.map([f32::from(p[0]), f32::from(p[1])]))
    }
}

impl Iterator for OutlineEdges<'_> {
    type Item = [[f32; 2]; 2];

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.points_left > 0 {
                self.points_left = self.points_left.saturating_sub(1);
                let p = self.next_point()?;
                let edge = [self.previous, p];
                self.previous = p;
                return Some(edge);
            }
            if self.first != self.previous {
                // Close the contour just finished.
                let edge = [self.previous, self.first];
                self.previous = self.first;
                return Some(edge);
            }
            if self.contours_left == 0 {
                return None;
            }
            self.contours_left = self.contours_left.saturating_sub(1);
            let count = self.reader.u16()?;
            let p = self.next_point()?;
            self.first = p;
            self.previous = p;
            self.points_left = count.saturating_sub(1);
        }
    }
}
