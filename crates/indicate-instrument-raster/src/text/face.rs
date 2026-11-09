//! Text in a loaded face: stroke and outline glyphs, deterministically.
//!
//! Each glyph is one primitive. Its geometry is mapped through the
//! transform and snapped to Q8.8 once, like every other coordinate, and
//! then each pixel centre in the glyph's bounds is tested once: inside
//! when it lies within half the pen of any centreline segment (a stroke
//! glyph, which gives round caps and joins), or when the non-zero
//! winding of all the glyph's contours is non-zero (an outline glyph).
//! A pixel is therefore composited at most once per glyph, so a
//! translucent glyph never blends against itself where its strokes
//! cross. Arcs flatten to a quarter pixel.
//!
//! The reference 5×7 face does not come here: the bitmap path in
//! [`crate::text`] draws it exactly as before.

use indicate_instrument_glyphs::typeface::{
    Face, GlyphKind, OutlineEdges, PlacedGlyph, StrokeSegments, TypefaceError,
};
use indicate_instrument_scene::{Anchor, ArcRun, HAlign, VAlign};

use crate::error::RasterError;
use crate::surface::{PixelRect, Surface};
use crate::text::Run;
use crate::transform::Affine;

/// Arc flattening tolerance, device pixels.
pub(crate) const TOLERANCE_PX: f32 = 0.25;

/// Most mapped segments or edges one glyph may have.
const MAX_GLYPH_SEGMENTS: usize = 512;

/// The baseline of a run whose anchor is `v`, given its size (the cap
/// height): the same vertical rule the reference face follows.
fn baseline(y: f32, size: f32, v: VAlign) -> f32 {
    match v {
        VAlign::Baseline | VAlign::Bottom => y,
        VAlign::Top => y + size,
        VAlign::Middle => y + size / 2.0,
    }
}

/// Renders a straight run in `face`, anchored by its advance width.
pub(crate) fn draw_face_run(
    surface: &mut Surface<'_>,
    clip: PixelRect,
    ctm: &Affine,
    face: &Face<'_>,
    run: Run<'_>,
    color: [u8; 4],
) -> Result<(), RasterError> {
    if run.text.is_empty() || run.size <= 0.0 {
        return Ok(());
    }
    let width = face.advance_width(run.size, run.text)?;
    let left = match run.anchor.h {
        HAlign::Left => run.x,
        HAlign::Center => run.x - width / 2.0,
        HAlign::Right => run.x - width,
    };
    let mut local = *ctm;
    local.translate(left, baseline(run.y, run.size, run.anchor.v));
    let mut failure = None;
    face.layout(run.size, run.text, |placed| {
        if failure.is_none() {
            failure = draw_glyph(surface, clip, &local, face, placed, color).err();
        }
    })?;
    failure.map_or(Ok(()), Err)
}

/// Renders an arc run in `face`: each glyph at its pen position along
/// the arc, turned to the reading direction.
pub(crate) fn draw_face_arc_run(
    surface: &mut Surface<'_>,
    clip: PixelRect,
    ctm: &Affine,
    face: &Face<'_>,
    run: &ArcRun<'_>,
    color: [u8; 4],
) -> Result<(), RasterError> {
    if run.text.is_empty() || run.size <= 0.0 {
        return Ok(());
    }
    let length = face.advance_width(run.size, run.text)?;
    let mut failure = None;
    face.layout(run.size, run.text, |placed| {
        if failure.is_some() {
            return;
        }
        let (x, y, turn) = run.place(length, placed.xform.origin[0]);
        let mut local = *ctm;
        local.translate(x, y);
        local.rotate(turn);
        let mut at_pen = *placed;
        at_pen.xform.origin = [0.0, 0.0];
        failure = draw_glyph(surface, clip, &local, face, &at_pen, color).err();
    })?;
    failure.map_or(Ok(()), Err)
}

/// A reference-face glyph drawn through the bitmap path at its pen.
fn draw_reference_glyph(
    surface: &mut Surface<'_>,
    clip: PixelRect,
    ctm: &Affine,
    placed: &PlacedGlyph<'_>,
    color: [u8; 4],
) -> Result<(), RasterError> {
    let mut utf8 = [0u8; 4];
    let run = Run {
        x: placed.xform.origin[0],
        y: placed.xform.origin[1],
        size: placed.cell * 7.0,
        anchor: Anchor {
            h: HAlign::Left,
            v: VAlign::Baseline,
        },
        text: placed.ch.encode_utf8(&mut utf8),
    };
    crate::text::draw_run(surface, clip, ctm, run, color)
}

fn draw_glyph(
    surface: &mut Surface<'_>,
    clip: PixelRect,
    ctm: &Affine,
    face: &Face<'_>,
    placed: &PlacedGlyph<'_>,
    color: [u8; 4],
) -> Result<(), RasterError> {
    let Some(glyph) = placed.glyph else {
        return draw_reference_glyph(surface, clip, ctm, placed, color);
    };
    let Some(ink) = face.glyph_ink(placed, TOLERANCE_PX) else {
        return Ok(());
    };
    let mut segments = [[0.0f32; 4]; MAX_GLYPH_SEGMENTS];
    let mut count = 0usize;
    let too_complex = || {
        RasterError::Typeface(TypefaceError::InvalidGlyph {
            codepoint: u32::from(placed.ch),
        })
    };
    let mut push = |a: [f32; 2], b: [f32; 2]| -> Result<(), RasterError> {
        let (pa, pb) = (ctm.map(a[0], a[1])?, ctm.map(b[0], b[1])?);
        let slot = segments.get_mut(count).ok_or_else(too_complex)?;
        *slot = [
            pa[0].to_f32(),
            pa[1].to_f32(),
            pb[0].to_f32(),
            pb[1].to_f32(),
        ];
        count = count.wrapping_add(1);
        Ok(())
    };
    match glyph.kind {
        GlyphKind::Stroke => {
            for [a, b] in StrokeSegments::new(&glyph, placed.xform, TOLERANCE_PX) {
                push(a, b)?;
            }
        }
        GlyphKind::Outline => {
            for [a, b] in OutlineEdges::new(&glyph, placed.xform) {
                push(a, b)?;
            }
        }
    }
    let segments = segments.get(..count).unwrap_or(&[]);
    let region = device_bounds(ctm, [ink.min_x, ink.min_y, ink.max_x, ink.max_y])?.intersect(clip);
    let half_pen = placed.pen * 0.5;
    for py in region.top..region.bottom {
        let cy = py as f32 + 0.5;
        for px in region.left..region.right {
            surface.count_sample();
            let cx = px as f32 + 0.5;
            let inside = match glyph.kind {
                GlyphKind::Stroke => {
                    surface.count_stroke_segment_tests(segments.len() as u64);
                    segments
                        .iter()
                        .any(|s| segment_distance(s, cx, cy) <= half_pen)
                }
                GlyphKind::Outline => {
                    surface.count_polygon_edge_tests(segments.len() as u64);
                    winding(segments, cx, cy) != 0
                }
            };
            if inside {
                surface.composite(px, py, color);
            }
        }
    }
    Ok(())
}

/// The pixel bounds of a logical box under `ctm`: the box's four mapped
/// corners, widened to whole pixels.
fn device_bounds(ctm: &Affine, b: [f32; 4]) -> Result<PixelRect, RasterError> {
    let corners = [[b[0], b[1]], [b[2], b[1]], [b[2], b[3]], [b[0], b[3]]];
    let mut lo = [f32::INFINITY; 2];
    let mut hi = [f32::NEG_INFINITY; 2];
    for c in corners {
        let p = ctm.map(c[0], c[1])?;
        let (x, y) = (p[0].to_f32(), p[1].to_f32());
        lo = [lo[0].min(x), lo[1].min(y)];
        hi = [hi[0].max(x), hi[1].max(y)];
    }
    Ok(PixelRect {
        left: libm::floorf(lo[0]) as i32,
        top: libm::floorf(lo[1]) as i32,
        right: libm::ceilf(hi[0]) as i32 + 1,
        bottom: libm::ceilf(hi[1]) as i32 + 1,
    })
}

/// Distance from `(cx, cy)` to the segment `[ax, ay, bx, by]`.
fn segment_distance(s: &[f32; 4], cx: f32, cy: f32) -> f32 {
    let [ax, ay, bx, by] = *s;
    let (ex, ey) = (bx - ax, by - ay);
    let len2 = ex * ex + ey * ey;
    let t = if len2 <= 0.0 {
        0.0
    } else {
        (((cx - ax) * ex + (cy - ay) * ey) / len2).clamp(0.0, 1.0)
    };
    let (qx, qy) = (ax + t * ex - cx, ay + t * ey - cy);
    libm::sqrtf(qx * qx + qy * qy)
}

/// Non-zero winding number of `(cx, cy)` against closed-contour edges.
fn winding(edges: &[[f32; 4]], cx: f32, cy: f32) -> i32 {
    let mut wn = 0i32;
    for &[ax, ay, bx, by] in edges {
        let left = (bx - ax) * (cy - ay) - (cx - ax) * (by - ay);
        if ay <= cy {
            if by > cy && left > 0.0 {
                wn = wn.wrapping_add(1);
            }
        } else if by <= cy && left < 0.0 {
            wn = wn.wrapping_sub(1);
        }
    }
    wn
}
