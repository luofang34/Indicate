//! Gun and missile cues; they never command a weapon.
use super::*;
use crate::context::{WeaponEnvelope, WeaponMode};
use crate::target::Contact;

pub(super) mod funnel;
pub(super) mod ground;
mod missile;

pub(super) const GRAVITY_MPS2: f32 = 9.806_65;

/// The north/east/down direction the gun line must point for a projectile
/// to meet the contact, and the time of flight, seconds.
///
/// The projectile leaves along the gun line at the muzzle speed relative to
/// the aircraft and falls under gravity; the contact keeps its relative velocity.
pub(crate) fn lead_solution(
    contact: &Contact,
    envelope: &WeaponEnvelope,
) -> Option<([f32; 3], f32)> {
    let speed = envelope.muzzle_speed_mps;
    if !(speed.is_finite() && speed > 1.0) {
        return None;
    }
    let p = contact.relative_position_m;
    let v = contact.relative_velocity_mps;
    let mut time = contact.range_m()? / speed;
    let mut aim = p;
    for _ in 0..4 {
        aim = [
            p[0] + v[0] * time,
            p[1] + v[1] * time,
            p[2] + v[2] * time - 0.5 * GRAVITY_MPS2 * time * time,
        ];
        time = libm::sqrtf(aim[0] * aim[0] + aim[1] * aim[1] + aim[2] * aim[2]) / speed;
    }
    (time.is_finite() && aim.iter().all(|a| a.is_finite())).then_some((aim, time))
}

pub(super) type Designated<'a> = Option<(&'a Contact, super::targets::Placed)>;

pub(super) fn draw(
    data: &PanelData,
    context: &HudContext,
    camera: &Camera,
    layout: &Layout,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let designated = context
        .targets
        .designated()
        .and_then(|contact| Some((contact, super::targets::place(contact, camera, layout)?)));
    match context.weapon {
        WeaponMode::Navigation => Ok(()),
        WeaponMode::Gun => {
            gun_cross(context, camera, layout, scene)?;
            funnel::draw(data, context, camera, layout, scene)?;
            director(context, designated, camera, layout, scene)
        }
        WeaponMode::ShortRangeMissile => {
            missile::short_range(context, designated, camera, layout, scene)
        }
        WeaponMode::MediumRangeMissile => {
            missile::medium_range(context, designated, camera, layout, scene)
        }
        WeaponMode::Rockets => ground::rockets(data, context, camera, layout, scene),
        WeaponMode::GuidedBomb => ground::guided_bomb(context, data, camera, layout, scene),
        WeaponMode::DirectedEnergy => {
            ground::directed_energy(context, designated, camera, layout, scene)
        }
    }
}

/// Where the gun line meets the frame, or the boresight when it cannot project.
pub(super) fn gun_point(context: &HudContext, camera: &Camera, layout: &Layout) -> [f32; 2] {
    camera
        .project_body(context.envelope.gun_line())
        .map_or(layout.centre, |p| [p.x, p.y])
}

/// The gun cross on the gun line.
fn gun_cross(
    context: &HudContext,
    camera: &Camera,
    layout: &Layout,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let u = layout.unit;
    let [cx, gy] = gun_point(context, camera, layout);
    let cy = gy + 24.0 * u;
    halo_polyline(
        layout,
        scene,
        &[[cx, cy - 30.0 * u], [cx, cy - 18.0 * u]],
        HUD_GREEN,
        1.4 * u,
    )?;
    halo_polyline(
        layout,
        scene,
        &[[cx - 6.0 * u, cy - 24.0 * u], [cx + 6.0 * u, cy - 24.0 * u]],
        HUD_GREEN,
        1.4 * u,
    )
}

/// The lead pipper for a designated contact, with a range ring against the gun limit.
fn director(
    context: &HudContext,
    designated: Designated<'_>,
    camera: &Camera,
    layout: &Layout,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let Some((contact, placed)) = designated else {
        return Ok(());
    };
    // Beyond twice the gun range the lead is too large to be a usable aim cue.
    if placed.range_m > context.envelope.gun_max_range_m * 2.0 {
        return Ok(());
    }
    let (Some(target), Some((aim, _))) = (placed.screen, lead_solution(contact, &context.envelope))
    else {
        return Ok(());
    };
    let Some(aim) = camera.project_ned(aim) else {
        return Ok(());
    };
    // The pipper sits where the target appears when the gun line points at the aim point.
    let [cx, cy] = gun_point(context, camera, layout);
    let (x, y) = (target[0] - (aim.x - cx), target[1] - (aim.y - cy));
    if !(x.is_finite() && y.is_finite()) {
        return Ok(());
    }
    let u = layout.unit;
    let r = 14.0 * u;
    let valid = contact.status == SignalStatus::Valid;
    let tint = layout.ink(if valid {
        HUD_GREEN
    } else {
        safety::CAUTION_AMBER
    });
    scene.stroke(layout.halo(), 3.6 * u)?;
    scene.circle(PaintMode::Stroke, x, y, r)?;
    scene.stroke(tint, 1.6 * u)?;
    scene.circle(PaintMode::Stroke, x, y, r)?;
    scene.fill_color(tint)?;
    scene.circle(PaintMode::Fill, x, y, 2.0 * u)?;
    let fraction = (placed.range_m / context.envelope.gun_max_range_m.max(1.0)).clamp(0.0, 1.0);
    scene.stroke(tint, 3.0 * u)?;
    scene.arc(
        x,
        y,
        r + 4.0 * u,
        -core::f32::consts::FRAC_PI_2,
        fraction * core::f32::consts::TAU,
    )?;
    if valid && placed.range_m <= context.envelope.gun_max_range_m {
        scene.fill_color(tint)?;
        scene.text(
            x,
            y + r + 14.0 * u,
            layout.text(11.0),
            Anchor::CENTER,
            "IN RNG",
        )?;
    }
    Ok(())
}
