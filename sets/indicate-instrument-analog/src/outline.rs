//! Pointer silhouettes as closed polygons.
//!
//! The same outline is filled in the 2D scene and extruded into a thin
//! needle by a 3D backend, so the two cannot disagree about a pointer's
//! shape. It is drawn pointing at twelve o'clock with the pivot at the
//! origin; a pose is one rotation about that origin.

use crate::spec::{MAX_OUTLINE_POINTS, PointerShape, PointerStyle};

/// A pointer silhouette: up to [`MAX_OUTLINE_POINTS`] corners in face
/// units, pivot at the origin, tip toward `-y`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointerOutline {
    points: [[f32; 2]; MAX_OUTLINE_POINTS],
    len: usize,
}

impl PointerOutline {
    fn from_slice(corners: &[[f32; 2]]) -> Self {
        let mut points = [[0.0; 2]; MAX_OUTLINE_POINTS];
        for (slot, corner) in points.iter_mut().zip(corners) {
            *slot = *corner;
        }
        Self {
            points,
            len: corners.len().min(MAX_OUTLINE_POINTS),
        }
    }

    /// The corners, in order.
    #[must_use]
    pub fn points(&self) -> &[[f32; 2]] {
        self.points.get(..self.len).unwrap_or(&[])
    }

    /// The outline scaled by `factor`: face units to pixels or millimetres.
    #[must_use]
    pub fn scaled(&self, factor: f32) -> Self {
        let mut out = *self;
        for point in &mut out.points {
            *point = [point[0] * factor, point[1] * factor];
        }
        out
    }
}

/// The silhouette `style` describes.
#[must_use]
pub fn pointer_outline(style: &PointerStyle) -> PointerOutline {
    let length = style.length;
    let half = style.width * 0.5;
    let tail = style.tail;
    match style.shape {
        PointerShape::Lance => {
            let shoulder = -length + style.width * 3.0;
            PointerOutline::from_slice(&[
                [0.0, -length],
                [half, shoulder],
                [half, tail],
                [-half, tail],
                [-half, shoulder],
            ])
        }
        PointerShape::Sword => {
            let shoulder = -length * 0.55;
            let waist = half * 0.45;
            PointerOutline::from_slice(&[
                [0.0, -length],
                [half, shoulder],
                [waist, tail],
                [-waist, tail],
                [-half, shoulder],
            ])
        }
        PointerShape::Baton => PointerOutline::from_slice(&[
            [half, -length],
            [half, tail],
            [-half, tail],
            [-half, -length],
        ]),
        PointerShape::Triangle => {
            let base = -length * 0.82;
            let shaft = half * 0.3;
            PointerOutline::from_slice(&[
                [0.0, -length],
                [half * 2.0, base],
                [shaft, base],
                [shaft, tail],
                [-shaft, tail],
                [-shaft, base],
                [-half * 2.0, base],
            ])
        }
    }
}
