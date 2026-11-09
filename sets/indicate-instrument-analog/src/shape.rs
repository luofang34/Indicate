//! Drawing [`Mark`]s: the face's printed marks, a card's shapes in its
//! own transform, and the fixed parts in front.

use indicate_instrument_descriptor::PanelDrawError;
use indicate_instrument_scene::{PaintMode, SceneWriter};

use crate::spec::{Lighting, Mark, Shape};

/// The most corners one polygon mark has.
pub const MAX_SHAPE_POINTS: usize = 16;

/// Where marks land: frame point = `origin + face point × scale`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Frame {
    pub(crate) origin: [f32; 2],
    pub(crate) scale: f32,
}

impl Frame {
    fn at(&self, p: [f32; 2]) -> [f32; 2] {
        [
            self.origin[0] + p[0] * self.scale,
            self.origin[1] + p[1] * self.scale,
        ]
    }
}

/// Draws `marks` in order, mapped through `frame`.
pub(crate) fn draw_marks(
    marks: &[Mark<'_>],
    lighting: Lighting,
    frame: Frame,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    for mark in marks {
        let colour = mark.paint.under(lighting);
        match mark.shape {
            Shape::Line { from, to, width } => {
                let ([x1, y1], [x2, y2]) = (frame.at(from), frame.at(to));
                scene.stroke(colour, width * frame.scale)?;
                scene.line(x1, y1, x2, y2)?;
            }
            Shape::Arc {
                center,
                radius,
                from_deg,
                to_deg,
                width,
            } => {
                let [cx, cy] = frame.at(center);
                scene.stroke(colour, width * frame.scale)?;
                // The scene's arcs start at +x; a mark's angles start
                // at twelve.
                scene.arc(
                    cx,
                    cy,
                    radius * frame.scale,
                    (from_deg - 90.0).to_radians(),
                    (to_deg - from_deg).to_radians(),
                )?;
            }
            Shape::Disc { center, radius } => {
                let [cx, cy] = frame.at(center);
                scene.fill_color(colour)?;
                scene.circle(PaintMode::Fill, cx, cy, radius * frame.scale)?;
            }
            Shape::Rect([x, y, w, h]) => {
                let [x, y] = frame.at([x, y]);
                scene.fill_color(colour)?;
                scene.rect(PaintMode::Fill, x, y, w * frame.scale, h * frame.scale)?;
            }
            Shape::Polygon(points) => {
                let mut mapped = [[0.0; 2]; MAX_SHAPE_POINTS];
                let count = points.len().min(MAX_SHAPE_POINTS);
                for (slot, point) in mapped.iter_mut().zip(points) {
                    *slot = frame.at(*point);
                }
                scene.fill_color(colour)?;
                scene.polygon(PaintMode::Fill, mapped.get(..count).unwrap_or(&[]))?;
            }
        }
    }
    Ok(())
}

/// Whether every number in `shape` is finite and every size positive,
/// and a polygon fits the corner budget.
pub(crate) fn shape_ok(shape: &Shape<'_>) -> bool {
    let finite = |v: &[f32]| v.iter().all(|c| c.is_finite());
    match *shape {
        Shape::Line { from, to, width } => finite(&from) && finite(&to) && width > 0.0,
        Shape::Arc {
            center,
            radius,
            from_deg,
            to_deg,
            width,
        } => {
            finite(&center)
                && finite(&[radius, from_deg, to_deg, width])
                && radius > 0.0
                && width > 0.0
                && to_deg > from_deg
        }
        Shape::Disc { center, radius } => finite(&center) && radius.is_finite() && radius > 0.0,
        Shape::Rect(r) => finite(&r) && r[2] > 0.0 && r[3] > 0.0,
        Shape::Polygon(points) => {
            (3..=MAX_SHAPE_POINTS).contains(&points.len()) && points.iter().all(|p| finite(p))
        }
    }
}
