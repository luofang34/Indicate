//! Glyphs to triangles, for a 3D builder or an atlas baker.
//!
//! Output is in the run's units, so a run laid out at a size in
//! millimetres tessellates in millimetres, in the scene's y-down
//! orientation with the run's pen origin at (0, 0). Nothing allocates:
//! triangles go to the caller's sink as they are made.
//!
//! - A stroke glyph becomes a quad along each centreline segment and a
//!   disc at each segment end, which together make the round caps and
//!   joins a pen draws. The pieces overlap; a renderer that blends a
//!   translucent glyph draws the triangles into a coverage mask first.
//! - An outline glyph becomes trapezoids between consecutive vertex
//!   heights, spanning the intervals where the non-zero winding of all
//!   its contours is inside. The trapezoids tile the glyph exactly.
//! - The reference 5×7 face becomes one square per set pixel.

use super::face::{Face, PlacedGlyph};
use super::geometry::{OutlineEdges, StrokeSegments};
use super::{GlyphKind, GlyphView, TypefaceError};
use crate::glyph::{CELL_H, CELL_W};

/// A triangle in output space.
pub type Triangle = [[f32; 2]; 3];

/// Most edge crossings one horizontal band of an outline may have.
const MAX_CROSSINGS: usize = 64;

/// Disc segments bounds, whatever the tolerance.
const MIN_DISC: u32 = 8;
const MAX_DISC: u32 = 64;

/// Tessellates every glyph of `text` at `size` in `face`, flattening
/// curves to `tolerance` output units; returns the run's advance width.
pub fn tessellate_run(
    face: &Face<'_>,
    size: f32,
    text: &str,
    tolerance: f32,
    mut sink: impl FnMut(Triangle),
) -> Result<f32, TypefaceError> {
    let mut failure = None;
    let advance = face.layout(size, text, |placed| {
        if failure.is_none()
            && let Err(e) = tessellate_glyph(placed, tolerance, &mut sink)
        {
            failure = Some(e);
        }
    })?;
    failure.map_or(Ok(advance), Err)
}

/// Tessellates one laid-out glyph.
pub fn tessellate_glyph(
    placed: &PlacedGlyph<'_>,
    tolerance: f32,
    sink: &mut impl FnMut(Triangle),
) -> Result<(), TypefaceError> {
    match placed.glyph {
        None => reference_cells(placed, sink),
        Some(glyph) if glyph.kind == GlyphKind::Stroke => {
            stroke(&glyph, placed, tolerance, sink);
            Ok(())
        }
        Some(glyph) => outline(&glyph, placed, sink),
    }
}

fn reference_cells(
    placed: &PlacedGlyph<'_>,
    sink: &mut impl FnMut(Triangle),
) -> Result<(), TypefaceError> {
    let bitmap = crate::PANEL_GLYPHS
        .glyph(placed.ch)
        .map_err(|_| TypefaceError::MissingGlyph { ch: placed.ch })?
        .glyph;
    let c = placed.cell;
    let x0 = placed.xform.origin[0];
    let top = -(CELL_H as f32) * c;
    for row in 0..CELL_H {
        for col in 0..CELL_W {
            if bitmap.pixel(col, row) {
                let (x, y) = (x0 + col as f32 * c, top + row as f32 * c);
                quad([[x, y], [x + c, y], [x + c, y + c], [x, y + c]], sink);
            }
        }
    }
    Ok(())
}

fn quad(q: [[f32; 2]; 4], sink: &mut impl FnMut(Triangle)) {
    let [a, b, c, d] = q;
    sink([a, b, c]);
    sink([a, c, d]);
}

fn disc(center: [f32; 2], radius: f32, tolerance: f32, sink: &mut impl FnMut(Triangle)) {
    if radius <= 0.0 {
        return;
    }
    let step = if tolerance > 0.0 && tolerance < radius {
        2.0 * libm::acosf(1.0 - tolerance / radius)
    } else {
        core::f32::consts::FRAC_PI_2
    };
    let n = (libm::ceilf(core::f32::consts::TAU / step) as u32).clamp(MIN_DISC, MAX_DISC);
    let at = |k: u32| {
        let a = core::f32::consts::TAU * k as f32 / n as f32;
        [
            center[0] + radius * libm::cosf(a),
            center[1] + radius * libm::sinf(a),
        ]
    };
    for k in 0..n {
        sink([center, at(k), at(k + 1)]);
    }
}

fn stroke(
    glyph: &GlyphView<'_>,
    placed: &PlacedGlyph<'_>,
    tolerance: f32,
    sink: &mut impl FnMut(Triangle),
) {
    let half = placed.pen * 0.5;
    for [a, b] in StrokeSegments::new(glyph, placed.xform, tolerance) {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let len = libm::sqrtf(dx * dx + dy * dy);
        if len > 0.0 {
            let (nx, ny) = (-dy / len * half, dx / len * half);
            quad(
                [
                    [a[0] + nx, a[1] + ny],
                    [b[0] + nx, b[1] + ny],
                    [b[0] - nx, b[1] - ny],
                    [a[0] - nx, a[1] - ny],
                ],
                sink,
            );
        }
        disc(a, half, tolerance, sink);
        disc(b, half, tolerance, sink);
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct Crossing {
    top: f32,
    bottom: f32,
    winding: i32,
}

fn outline(
    glyph: &GlyphView<'_>,
    placed: &PlacedGlyph<'_>,
    sink: &mut impl FnMut(Triangle),
) -> Result<(), TypefaceError> {
    let edges = || OutlineEdges::new(glyph, placed.xform);
    let next_y = |above: f32| {
        edges()
            .flat_map(|[a, b]| [a[1], b[1]])
            .filter(|y| *y > above)
            .fold(f32::INFINITY, f32::min)
    };
    let mut y0 = next_y(f32::NEG_INFINITY);
    while y0.is_finite() {
        let y1 = next_y(y0);
        if !y1.is_finite() {
            break;
        }
        let mut crossings = [Crossing::default(); MAX_CROSSINGS];
        let mut count = 0usize;
        for [a, b] in edges() {
            let (lo, hi) = if a[1] < b[1] { (a, b) } else { (b, a) };
            if lo[1] > y0 || hi[1] < y1 || lo[1] == hi[1] {
                continue;
            }
            let x_at = |y: f32| lo[0] + (hi[0] - lo[0]) * (y - lo[1]) / (hi[1] - lo[1]);
            let slot = crossings
                .get_mut(count)
                .ok_or(TypefaceError::InvalidGlyph {
                    codepoint: u32::from(glyph.ch),
                })?;
            *slot = Crossing {
                top: x_at(y0),
                bottom: x_at(y1),
                winding: if a[1] < b[1] { 1 } else { -1 },
            };
            count = count.wrapping_add(1);
        }
        let band = crossings.get_mut(..count).unwrap_or(&mut []);
        band.sort_unstable_by(|p, q| (p.top + p.bottom).total_cmp(&(q.top + q.bottom)));
        let mut winding = 0;
        let mut left: Option<Crossing> = None;
        for c in band.iter() {
            let before = winding;
            winding += c.winding;
            match (before == 0, winding == 0, left) {
                (true, false, _) => left = Some(*c),
                (false, true, Some(l)) => {
                    quad(
                        [[l.top, y0], [c.top, y0], [c.bottom, y1], [l.bottom, y1]],
                        sink,
                    );
                    left = None;
                }
                _ => {}
            }
        }
        y0 = y1;
    }
    Ok(())
}
