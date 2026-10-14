//! One pitch-ladder rung, shared by the conformal and the compressed ladder.
//!
//! Climb rungs are solid with end legs toward the horizon. Dive rungs are
//! dashed, and each half droops its outer end away from the horizon by half
//! the dive angle. The two halves then form a caret whose apex points to the
//! horizon, and they follow the curve of constant-pitch circles about the
//! nadir. The two hemispheres differ in shape as well as in pattern.
use super::*;

const DEG: f32 = core::f32::consts::PI / 180.0;

pub(super) struct Rung {
    pub centre: [f32; 2],
    /// Unit direction along the rung, toward the right half.
    pub along: [f32; 2],
    /// Unit direction from the rung toward the horizon.
    pub toward: [f32; 2],
    /// Distance from the centre to each outer end.
    pub half: f32,
    /// Distance from the centre to each inner end.
    pub gap: f32,
}

impl Rung {
    /// The point `distance` from the inner end of one half; `side` is -1 or +1.
    fn on_half(&self, side: f32, distance: f32, bend: f32) -> [f32; 2] {
        let inner = [
            self.centre[0] + self.along[0] * side * self.gap,
            self.centre[1] + self.along[1] * side * self.gap,
        ];
        let (s, c) = (libm::sinf(bend), libm::cosf(bend));
        [
            inner[0] + (self.along[0] * side * c - self.toward[0] * s) * distance,
            inner[1] + (self.along[1] * side * c - self.toward[1] * s) * distance,
        ]
    }

    pub(super) fn draw(
        &self,
        degrees: i32,
        layout: &Layout,
        scene: &mut SceneWriter<'_>,
    ) -> Result<(), PanelDrawError> {
        let u = layout.unit;
        let length = self.half - self.gap;
        let bend = if degrees < 0 {
            degrees.unsigned_abs() as f32 * DEG * 0.5
        } else {
            0.0
        };
        for side in [-1.0_f32, 1.0] {
            let at = |d: f32| self.on_half(side, d, bend);
            if degrees < 0 {
                for k in 0..3 {
                    let start = length * k as f32 / 3.0;
                    halo_polyline(
                        layout,
                        scene,
                        &[at(start), at(start + length * 0.2)],
                        HUD_GREEN,
                        1.4 * u,
                    )?;
                }
            } else {
                halo_polyline(layout, scene, &[at(0.0), at(length)], HUD_GREEN, 1.4 * u)?;
            }
            let tip = at(length);
            if degrees != 0 {
                let leg = 7.0 * u;
                let foot = [tip[0] + self.toward[0] * leg, tip[1] + self.toward[1] * leg];
                halo_polyline(layout, scene, &[tip, foot], HUD_GREEN, 1.4 * u)?;
            }
            if degrees != 0 && degrees % 10 == 0 {
                let label = fmt_label!(8, "{}", degrees.abs());
                let [x, y] = at(length + 16.0 * u);
                scene.fill_color(HUD_GREEN)?;
                scene.text_attributed(
                    GroupId::Attitude.to_u8(),
                    x,
                    y,
                    layout.text(13.0),
                    Anchor::CENTER,
                    label.as_str(),
                )?;
            }
        }
        Ok(())
    }
}

/// The zenith (circle) and nadir (circle with a cross) symbols.
pub(super) fn pole(
    at: [f32; 2],
    zenith: bool,
    layout: &Layout,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let u = layout.unit;
    scene.stroke(HUD_GREEN, 1.4 * u)?;
    scene.circle(PaintMode::Stroke, at[0], at[1], 9.0 * u)?;
    if !zenith {
        let r = 6.0 * u;
        scene.line(at[0] - r, at[1] - r, at[0] + r, at[1] + r)?;
        scene.line(at[0] - r, at[1] + r, at[0] + r, at[1] - r)?;
    }
    Ok(())
}
