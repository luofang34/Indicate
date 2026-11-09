//! The static face: everything that does not move.
//!
//! The face is a pure function of the spec and the lighting. It never
//! reads state, which is what lets a 3D backend rasterise it once into
//! a texture, and what lets admission prove its numerals are printed
//! legends rather than values (`ATTR_CONFIG` runs identical in every
//! case).

use indicate_instrument_descriptor::{DesignFrame, PanelDrawError};
use indicate_instrument_scene::{
    ATTR_CONFIG, Anchor, ArcAlign, ArcRun, FaceSlot, PaintMode, SceneWriter,
};

use crate::spec::{AnalogDialSpec, Lighting, Paint, ScaleRing};

/// Where a dial sits in a design frame: its centre and how many frame
/// units one face unit spans.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    /// Face centre, frame units.
    pub center: [f32; 2],
    /// Frame units per face unit.
    pub scale: f32,
}

impl Placement {
    /// The dial centred in `frame`, its case filling the shorter side.
    #[must_use]
    pub fn centred(spec: &AnalogDialSpec<'_>, frame: DesignFrame) -> Self {
        let case_radius = frame.width.min(frame.height) * 0.5;
        let face_share = spec.face_radius_mm / (spec.case_diameter_mm * 0.5);
        Self {
            center: [frame.width * 0.5, frame.height * 0.5],
            scale: case_radius * face_share,
        }
    }

    /// A face-unit point in frame units.
    #[must_use]
    pub fn at(&self, point: [f32; 2]) -> [f32; 2] {
        [
            self.center[0] + point[0] * self.scale,
            self.center[1] + point[1] * self.scale,
        ]
    }
}

/// The point at `angle_deg` (clockwise from twelve) and `radius` about
/// `center`, in face units.
fn polar(center: [f32; 2], radius: f32, angle_deg: f32) -> [f32; 2] {
    let a = angle_deg.to_radians();
    [
        center[0] + radius * libm::sinf(a),
        center[1] - radius * libm::cosf(a),
    ]
}

/// Draws the face of `spec` into the band the caller has open: case,
/// bezel, face, printed marks, range markings, graduations, numerals,
/// and legends.
pub fn draw_face(
    spec: &AnalogDialSpec<'_>,
    lighting: Lighting,
    place: &Placement,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let [cx, cy] = place.center;
    let case_r = place.scale * spec.case_diameter_mm * 0.5 / spec.face_radius_mm;
    scene.fill_color(Paint::plain(spec.bezel).under(lighting))?;
    scene.circle(PaintMode::Fill, cx, cy, case_r)?;
    scene.fill_color(spec.face.under(lighting))?;
    scene.circle(PaintMode::Fill, cx, cy, place.scale)?;
    let frame = crate::shape::Frame {
        origin: place.center,
        scale: place.scale,
    };
    crate::shape::draw_marks(spec.marks, lighting, frame, scene)?;
    for ring in spec.rings {
        draw_markings(ring, lighting, place, scene)?;
        draw_ticks(ring, lighting, place, scene)?;
        in_face(scene, spec.lettering.numerals, |scene| {
            draw_numerals(ring, lighting, place, scene)
        })?;
    }
    in_face(scene, spec.lettering.legends, |scene| {
        draw_legends(spec, lighting, place, scene)
    })
}

/// Runs `draw` in face slot `slot`. The default slot emits nothing, so
/// a dial lettered in the default face draws exactly as before; any
/// other slot is set inside its own save and restore, so it cannot leak
/// into what follows.
pub(crate) fn in_face(
    scene: &mut SceneWriter<'_>,
    slot: FaceSlot,
    draw: impl FnOnce(&mut SceneWriter<'_>) -> Result<(), PanelDrawError>,
) -> Result<(), PanelDrawError> {
    if slot == FaceSlot::DEFAULT {
        return draw(scene);
    }
    scene.save()?;
    scene.font(slot)?;
    draw(scene)?;
    scene.restore()?;
    Ok(())
}

fn draw_legends(
    spec: &AnalogDialSpec<'_>,
    lighting: Lighting,
    place: &Placement,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    for legend in spec.legends {
        let [x, y] = place.at(legend.at);
        scene.fill_color(legend.paint.under(lighting))?;
        scene.text_attributed(
            ATTR_CONFIG,
            x,
            y,
            legend.size * place.scale,
            Anchor::CENTER,
            legend.text,
        )?;
    }
    for legend in spec.arc_legends {
        scene.fill_color(legend.paint.under(lighting))?;
        // The dial counts angles from twelve; the scene from +x.
        let run = ArcRun {
            cx: place.center[0],
            cy: place.center[1],
            radius: legend.radius * place.scale,
            angle: (legend.angle_deg - 90.0).to_radians(),
            sense: legend.sense,
            align: ArcAlign::Center,
            size: legend.size * place.scale,
            text: legend.text,
        };
        scene.text_arc_attributed(ATTR_CONFIG, &run)?;
    }
    Ok(())
}

fn draw_markings(
    ring: &ScaleRing<'_>,
    lighting: Lighting,
    place: &Placement,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let [cx, cy] = place.at(ring.center);
    let r = ring.radius * place.scale;
    for arc in ring.arcs {
        let start = ring.law.angle_deg(arc.from);
        let end = ring.law.angle_deg(arc.to);
        let mid = (arc.inner + arc.outer) * 0.5 * r;
        let colour = Paint::plain(arc.colour.rgba()).under(lighting);
        scene.stroke(colour, (arc.outer - arc.inner) * r)?;
        // The scene's arcs start at +x; the dial's angles start at twelve.
        scene.arc(
            cx,
            cy,
            mid,
            (start - 90.0).to_radians(),
            (end - start).to_radians(),
        )?;
    }
    for radial in ring.radials {
        let angle = ring.law.angle_deg(radial.at);
        let a = place.at(polar(ring.center, radial.inner * ring.radius, angle));
        let b = place.at(polar(ring.center, radial.outer * ring.radius, angle));
        let colour = Paint::plain(radial.colour.rgba()).under(lighting);
        scene.stroke(colour, radial.width * place.scale)?;
        scene.line(a[0], a[1], b[0], b[1])?;
    }
    Ok(())
}

fn draw_ticks(
    ring: &ScaleRing<'_>,
    lighting: Lighting,
    place: &Placement,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let colour = ring.paint.under(lighting);
    for set in ring.ticks {
        scene.stroke(colour, set.width * place.scale)?;
        // Counted in steps rather than accumulated, so a long run of
        // fine graduations does not drift by rounding.
        let steps = libm::floorf((set.to - set.from) / set.every + 1e-3);
        let count = if steps.is_finite() && steps >= 0.0 {
            (steps as u32).min(1000)
        } else {
            0
        };
        for k in 0..=count {
            let value = set.from + set.every * k as f32;
            let angle = ring.law.angle_deg(value);
            let a = place.at(polar(ring.center, set.inner * ring.radius, angle));
            let b = place.at(polar(ring.center, set.outer * ring.radius, angle));
            scene.line(a[0], a[1], b[0], b[1])?;
        }
    }
    Ok(())
}

fn draw_numerals(
    ring: &ScaleRing<'_>,
    lighting: Lighting,
    place: &Placement,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    scene.fill_color(ring.paint.under(lighting))?;
    for numeral in ring.numerals {
        let angle = ring.law.angle_deg(numeral.value);
        let [x, y] = place.at(polar(ring.center, ring.numeral_radius * ring.radius, angle));
        scene.text_attributed(
            ATTR_CONFIG,
            x,
            y,
            ring.numeral_size * place.scale,
            Anchor::CENTER,
            numeral.text,
        )?;
    }
    Ok(())
}
