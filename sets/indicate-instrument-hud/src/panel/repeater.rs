//! A boxed attitude indicator for the non-conformal repeater.
//!
//! The box, its fixed scale, and its numeric readouts keep it distinct from a
//! registered horizon, so it cannot be read as an outside-world line.
use super::*;

pub(super) fn draw(
    data: &PanelData,
    layout: &Layout,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let u = layout.unit;
    let [cx, cy] = layout.centre;
    let side = layout.field[0].min(layout.field[1]) * 0.9;
    let top = cy - side * 0.5;
    scene.stroke(HUD_GREEN, 1.4 * u)?;
    scene.rect(PaintMode::Stroke, cx - side * 0.5, top, side, side)?;
    scene.fill_color(HUD_GREEN)?;
    scene.text(
        cx,
        top - 10.0 * u,
        layout.text(11.0),
        Anchor::CENTER,
        "ATT REPEATER",
    )?;
    let roll = data.roll_rad;
    let pitch = data.pitch_rad;
    let (Some(roll_value), Some(pitch_value)) = (live(roll), live(pitch)) else {
        let status = shown_status(roll).worst(shown_status(pitch));
        scene.fill_color(status_color(layout, status))?;
        let text = fmt_label!(24, "ATT {}", status_label(status));
        return Ok(scene.text(cx, cy, layout.text(16.0), Anchor::CENTER, text.as_str())?);
    };
    scene.save()?;
    scene.clip_rect(cx - side * 0.5, top, side, side)?;
    scene.translate(cx, cy)?;
    scene.rotate(-roll_value)?;
    let per_degree = side / 90.0;
    let offset = pitch_value.to_degrees() * per_degree;
    scene.stroke(HUD_GREEN, 1.6 * u)?;
    scene.line(-side, offset, side, offset)?;
    for degrees in [-30_i32, -20, -10, 10, 20, 30] {
        let y = offset - degrees as f32 * per_degree;
        let half = if degrees % 20 == 0 { 0.16 } else { 0.1 } * side;
        scene.line(-half, y, half, y)?;
    }
    scene.restore()?;
    scene.stroke(HUD_GREEN, 2.0 * u)?;
    scene.line(cx - side * 0.22, cy, cx - side * 0.07, cy)?;
    scene.line(cx + side * 0.07, cy, cx + side * 0.22, cy)?;
    scene.circle(PaintMode::Fill, cx, cy, 2.0 * u)?;
    let bank = fmt_label!(
        16,
        "BANK {}{:.0}",
        if roll_value < 0.0 { "L" } else { "R" },
        roll_value.to_degrees().abs()
    );
    let nose = fmt_label!(16, "PITCH {:.0}", pitch_value.to_degrees());
    scene.fill_color(HUD_GREEN)?;
    let below = top + side + 14.0 * u;
    scene.text_attributed(
        GroupId::Attitude.to_u8(),
        cx - side * 0.25,
        below,
        layout.text(12.0),
        Anchor::CENTER,
        bank.as_str(),
    )?;
    scene.text_attributed(
        GroupId::Attitude.to_u8(),
        cx + side * 0.25,
        below,
        layout.text(12.0),
        Anchor::CENTER,
        nose.as_str(),
    )?;
    Ok(())
}
