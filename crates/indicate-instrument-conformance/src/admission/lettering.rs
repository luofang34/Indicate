//! Where a text run inks in the face the host binds to its slot.
//!
//! Admission judges a run where a backend paints it. A run in the
//! reference face keeps the scene's nominal metrics ([`text_rect`],
//! [`arc_run_rect`]), which every pinned band and legend witness is
//! measured with. A run in a bound typeface is placed by that face's
//! own advance widths and ink boxes, with the anchor and baseline rule
//! the reference rasterizer applies, so a wide face that overruns its
//! frame, a legend that moves, or a readout that leaves its region is
//! seen in the face the cockpit letters it in.
//!
//! The slot resolves through [`FaceTable::face`] and nothing else, so
//! the harness and the rasterizer agree on which face an unbound slot
//! falls back to. A character the bound face lacks fails the run: the
//! rasterizer refuses it at draw time, and the harness refuses it here
//! rather than measuring a substitute.

use indicate_instrument_glyphs::typeface::{Face, FaceTable, InkBox, TypefaceError};
use indicate_instrument_scene::{ArcRun, Cmd, HAlign, VAlign};

use super::geometry::{Ctm, Rect, arc_run_rect, text_rect};

/// Arc flattening tolerance for stroke ink, design units. The reference
/// rasterizer flattens at a quarter device pixel, and admission frames
/// are drawn at one design unit per pixel, so the same quarter keeps
/// the harness's ink at least as wide as the painted ink.
const INK_TOLERANCE: f32 = 0.25;

/// A text run's design-space ink rectangle and its text, in the face
/// bound to `slot`; `Ok(None)` for a command that is not a text run.
pub(super) fn run_ink<'a>(
    ctm: &Ctm,
    cmd: &Cmd<'a>,
    slot: u8,
    faces: &FaceTable<'_>,
) -> Result<Option<(Rect, &'a str)>, TypefaceError> {
    let face = faces.face(slot);
    match *cmd {
        Cmd::Text {
            x,
            y,
            size,
            anchor,
            text,
        } => {
            let local = if face == Face::Reference {
                text_rect(x, y, size, anchor.h, anchor.v, text.chars().count())
            } else {
                face_text_rect(&face, [x, y, size], anchor.h, anchor.v, text)?
            };
            Ok(Some((ctm.map_rect(&local), text)))
        }
        Cmd::TextArc(ref arc) => {
            let rect = if face == Face::Reference {
                arc_run_rect(ctm, arc)
            } else {
                arc_ink(ctm, arc, &face)?
            };
            Ok(Some((rect, arc.text)))
        }
        _ => Ok(None),
    }
}

/// A straight run's local ink in a loaded face, anchored by its advance
/// width; `v` is `[x, y, size]`.
fn face_text_rect(
    face: &Face<'_>,
    v: [f32; 3],
    h: HAlign,
    va: VAlign,
    text: &str,
) -> Result<Rect, TypefaceError> {
    let [x, y, size] = v;
    let width = face.advance_width(size, text)?;
    let left = match h {
        HAlign::Left => x,
        HAlign::Center => x - width / 2.0,
        HAlign::Right => x - width,
    };
    let base = baseline(y, size, va);
    let ink = face.ink_box(size, text, INK_TOLERANCE)?;
    Ok(ink.map_or_else(|| point(left, base), |b| offset(b, left, base)))
}

/// The baseline of a run anchored at `y` by `v`, given its size (the
/// cap height): the rule the rasterizer letters every face by.
fn baseline(y: f32, size: f32, v: VAlign) -> f32 {
    match v {
        VAlign::Baseline | VAlign::Bottom => y,
        VAlign::Top => y + size,
        VAlign::Middle => y + size / 2.0,
    }
}

/// The union of every glyph's ink, each placed and turned along the
/// arc as the rasterizer places it.
fn arc_ink(ctm: &Ctm, run: &ArcRun<'_>, face: &Face<'_>) -> Result<Rect, TypefaceError> {
    let length = face.advance_width(run.size, run.text)?;
    let mut bound: Option<Rect> = None;
    face.layout(run.size, run.text, |placed| {
        let (x, y, turn) = run.place(length, placed.xform.origin[0]);
        let mut at_pen = *placed;
        at_pen.xform.origin = [0.0, 0.0];
        let Some(ink) = face.glyph_ink(&at_pen, INK_TOLERANCE) else {
            return;
        };
        let mut local = *ctm;
        local.translate(x, y);
        local.rotate(turn);
        let glyph = local.map_rect(&offset(ink, 0.0, 0.0));
        bound = Some(bound.map_or(glyph, |b| b.union(&glyph)));
    })?;
    Ok(bound.unwrap_or_else(|| {
        let (x, y) = ctm.apply(run.cx, run.cy);
        point(x, y)
    }))
}

fn offset(ink: InkBox, dx: f32, dy: f32) -> Rect {
    Rect {
        min_x: ink.min_x + dx,
        min_y: ink.min_y + dy,
        max_x: ink.max_x + dx,
        max_y: ink.max_y + dy,
    }
}

fn point(x: f32, y: f32) -> Rect {
    Rect {
        min_x: x,
        min_y: y,
        max_x: x,
        max_y: y,
    }
}
