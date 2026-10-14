//! Missile cues: an infrared seeker circle, and a radar steering circle and dot.
use super::*;
use crate::context::MissileEnvelope;

/// Angle between the nose and a body direction, radians.
fn off_nose(body: [f32; 3]) -> f32 {
    let length = libm::sqrtf(body[0] * body[0] + body[1] * body[1] + body[2] * body[2]);
    libm::acosf((body[0] / length.max(1e-6)).clamp(-1.0, 1.0))
}

/// The launch cue for one missile from lock and range.
fn cue(locked: bool, range: f32, envelope: &MissileEnvelope) -> (&'static str, Rgba8) {
    if !locked {
        ("NO LOCK", safety::CAUTION_AMBER)
    } else if range < envelope.min_range_m {
        ("MIN RNG", safety::CAUTION_AMBER)
    } else if range > envelope.max_range_m {
        ("LOCK", HUD_GREEN)
    } else {
        ("SHOOT", HUD_GREEN)
    }
}

fn label(
    text: &str,
    color: Rgba8,
    layout: &Layout,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let [cx, cy] = layout.centre;
    let size = if text == "SHOOT" { 20.0 } else { 13.0 };
    scene.fill_color(layout.ink(color))?;
    Ok(scene.text(
        cx,
        cy + layout.field[1] * 0.55,
        layout.text(size),
        Anchor::CENTER,
        text,
    )?)
}

fn ring(
    at: [f32; 2],
    radius: f32,
    width: f32,
    layout: &Layout,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    if !(radius.is_finite() && radius > 0.0) {
        return Ok(());
    }
    scene.stroke(layout.halo(), width + 2.0)?;
    scene.circle(PaintMode::Stroke, at[0], at[1], radius)?;
    scene.stroke(HUD_GREEN, width)?;
    scene.circle(PaintMode::Stroke, at[0], at[1], radius)?;
    Ok(())
}

/// The infrared seeker: a small circle at the nose that moves onto a locked contact.
pub(super) fn short_range(
    context: &HudContext,
    designated: Designated<'_>,
    camera: &Camera,
    layout: &Layout,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let envelope = &context.envelope.short_range;
    let u = layout.unit;
    let radius = camera.cone_radius(envelope.circle_half_angle_rad);
    let lock = designated.filter(|(contact, placed)| {
        contact.status == SignalStatus::Valid
            && placed.screen.is_some()
            && off_nose(placed.body) <= envelope.lock_limit_rad
            && placed.range_m <= envelope.max_range_m * 1.5
    });
    let at = lock
        .and_then(|(_, placed)| placed.screen)
        .unwrap_or(layout.centre);
    ring(
        at,
        radius,
        if lock.is_some() { 2.4 * u } else { 1.4 * u },
        layout,
        scene,
    )?;
    let Some((_, placed)) = designated else {
        return Ok(());
    };
    let (text, color) = cue(lock.is_some(), placed.range_m, envelope);
    label(text, color, layout, scene)
}

/// The radar missile: a steering circle at the nose and a dot toward the contact.
pub(super) fn medium_range(
    context: &HudContext,
    designated: Designated<'_>,
    camera: &Camera,
    layout: &Layout,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let envelope = &context.envelope.medium_range;
    let u = layout.unit;
    let radius = camera.cone_radius(envelope.circle_half_angle_rad);
    ring(layout.centre, radius, 1.4 * u, layout, scene)?;
    let Some((contact, placed)) = designated else {
        return Ok(());
    };
    let angle = off_nose(placed.body);
    let tracked = contact.status == SignalStatus::Valid && angle <= envelope.lock_limit_rad;
    if let (true, Some(direction)) = (tracked, camera.screen_direction(placed.body)) {
        // The dot sits off the circle centre in proportion to the steering error.
        let offset = (angle / envelope.circle_half_angle_rad).min(2.0) * radius;
        let [cx, cy] = layout.centre;
        let dot = [cx + direction[0] * offset, cy + direction[1] * offset];
        scene.fill_color(layout.halo())?;
        scene.circle(PaintMode::Fill, dot[0], dot[1], 5.5 * u)?;
        scene.fill_color(HUD_GREEN)?;
        scene.circle(PaintMode::Fill, dot[0], dot[1], 4.0 * u)?;
    }
    let steered = tracked && angle <= envelope.circle_half_angle_rad;
    let (text, color) = cue(steered, placed.range_m, envelope);
    let text = if tracked && !steered { "STEER" } else { text };
    label(text, color, layout, scene)
}
