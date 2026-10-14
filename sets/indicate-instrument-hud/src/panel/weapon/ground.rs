//! Air-to-surface and directed-energy cues: the rocket impact point, the
//! guided-bomb steering line and release cue, and the energy weapon's reticle.
use super::*;
use crate::context::EnergyState;
use crate::target::Domain;

const KNOT: f32 = 0.514_444;
const FOOT: f32 = 0.3048;

fn norm(v: [f32; 3]) -> f32 {
    libm::sqrtf(v[0] * v[0] + v[1] * v[1] + v[2] * v[2])
}

/// Time to fall `height` metres from a downward speed under gravity, seconds.
fn fall_time(down_speed: f32, height: f32) -> Option<f32> {
    let disc = down_speed * down_speed + 2.0 * GRAVITY_MPS2 * height;
    let t = (-down_speed + libm::sqrtf(disc)) / GRAVITY_MPS2;
    (height > 0.0 && disc >= 0.0 && t.is_finite() && t > 0.0).then_some(t)
}

fn velocity(data: &PanelData) -> Option<[f32; 3]> {
    super::super::ladder::velocity_ned(data).map(|v| [v[0] * KNOT, v[1] * KNOT, v[2] * KNOT])
}

/// The rocket impact point relative to the aircraft, north/east/down metres:
/// launched along the gun line at the rocket speed, falling under gravity onto
/// the terrain elevation the host supplies.
pub(crate) fn rocket_impact(
    data: &PanelData,
    context: &HudContext,
    camera: &Camera,
) -> Option<[f32; 3]> {
    let elevation = context.terrain_elevation_m;
    if elevation.status != SignalStatus::Valid || !elevation.value.is_finite() {
        return None;
    }
    let altitude = super::super::live(data.altitude.value_ft)? * FOOT;
    let line = camera.body_to_ned(context.envelope.gun_line())?;
    let own = velocity(data)?;
    let speed = context.envelope.rocket_speed_mps;
    let launch = [
        own[0] + line[0] * speed,
        own[1] + line[1] * speed,
        own[2] + line[2] * speed,
    ];
    let t = fall_time(launch[2], altitude - elevation.value)?;
    Some([
        launch[0] * t,
        launch[1] * t,
        launch[2] * t + 0.5 * GRAVITY_MPS2 * t * t,
    ])
}

pub(super) fn rockets(
    data: &PanelData,
    context: &HudContext,
    camera: &Camera,
    layout: &Layout,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let u = layout.unit;
    let [cx, cy] = layout.centre;
    let [hw, hh] = layout.field;
    let Some(impact) = rocket_impact(data, context, camera) else {
        scene.fill_color(layout.ink(safety::CAUTION_AMBER))?;
        return Ok(scene.text(
            cx,
            cy + 44.0 * u,
            layout.text(11.0),
            Anchor::CENTER,
            "CCIP INVALID",
        )?);
    };
    let in_range = norm(impact) <= context.envelope.rocket_max_range_m;
    let point = camera.project_ned(impact).map(|p| [p.x, p.y]);
    let [x, y] = point.unwrap_or([cx, cy + hh]);
    let (px, py) = (x.clamp(cx - hw, cx + hw), y.clamp(cy - hh, cy + hh));
    let limited = point.is_none() || px != x || py != y;
    let tint = layout.ink(if in_range && !limited {
        HUD_GREEN
    } else {
        safety::CAUTION_AMBER
    });
    let r = 10.0 * u;
    scene.stroke(layout.halo(), 3.6 * u)?;
    scene.circle(PaintMode::Stroke, px, py, r)?;
    scene.stroke(tint, 1.6 * u)?;
    scene.circle(PaintMode::Stroke, px, py, r)?;
    scene.fill_color(tint)?;
    scene.circle(PaintMode::Fill, px, py, 2.0 * u)?;
    let label = if limited {
        "CCIP LIMIT"
    } else if in_range {
        "IN RNG"
    } else {
        "RNG"
    };
    Ok(scene.text(
        px,
        py + r + 13.0 * u,
        layout.text(11.0),
        Anchor::CENTER,
        label,
    )?)
}

/// The designated surface target, when there is one in the picture.
fn surface_target(context: &HudContext) -> Option<&crate::target::Contact> {
    context
        .targets
        .designated()
        .filter(|c| c.domain == Domain::Surface && c.drawable())
}

/// Release state of a guided bomb from horizontal distance and reach, metres.
pub(crate) fn release_state(
    distance: f32,
    ballistic: f32,
    glide_ratio: f32,
) -> (&'static str, f32) {
    let reach = ballistic * glide_ratio;
    let cue = ((distance - reach) / reach.max(1.0)).max(0.0);
    let state = if distance > reach {
        "STEER"
    } else if distance >= ballistic * 0.7 {
        "REL"
    } else {
        "MIN RNG"
    };
    (state, cue)
}

pub(super) fn guided_bomb(
    context: &HudContext,
    data: &PanelData,
    camera: &Camera,
    layout: &Layout,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let u = layout.unit;
    let [cx, cy] = layout.centre;
    let Some(target) = surface_target(context) else {
        scene.fill_color(layout.ink(safety::CAUTION_AMBER))?;
        return Ok(scene.text(
            cx,
            cy + 44.0 * u,
            layout.text(11.0),
            Anchor::CENTER,
            "NO DESIG",
        )?);
    };
    let p = target.relative_position_m;
    let own = velocity(data);
    let fall = own.and_then(|v| fall_time(v[2], p[2]).map(|t| (t, libm::hypotf(v[0], v[1]) * t)));
    let bearing = libm::atan2f(p[1], p[0]);
    let along = |elevation: f32| {
        let (s, c) = (libm::sinf(elevation), libm::cosf(elevation));
        camera
            .project_ned([c * libm::cosf(bearing), c * libm::sinf(bearing), -s])
            .map(|q| [q.x, q.y])
    };
    // The steering line: the vertical through the target's bearing.
    let (Some(top), Some(bottom)) = (along(0.35), along(-0.35)) else {
        return Ok(());
    };
    let [hw, hh] = layout.field;
    scene.save()?;
    // Below the heading readout, as the ladder is.
    let clip_top = cy - hh + 34.0 * u;
    scene.clip_rect(cx - hw, clip_top, hw * 2.0, cy + hh - clip_top)?;
    halo_polyline(layout, scene, &[top, bottom], HUD_GREEN, 1.6 * u)?;
    let Some((_, ballistic)) = fall else {
        scene.restore()?;
        return Ok(());
    };
    let (state, cue) = release_state(
        libm::hypotf(p[0], p[1]),
        ballistic,
        context.envelope.bomb_glide_ratio,
    );
    let marker = velocity(data)
        .and_then(|v| camera.project_ned(v))
        .map_or(cy, |m| m.y);
    let y = marker - (cue * 120.0 * u).min(hh * 2.0);
    let x = top[0] + (bottom[0] - top[0]) * ((y - top[1]) / (bottom[1] - top[1]).max(1e-3));
    halo_polyline(
        layout,
        scene,
        &[[x - 14.0 * u, y], [x + 14.0 * u, y]],
        HUD_GREEN,
        2.4 * u,
    )?;
    scene.restore()?;
    release_labels(state, p, camera, layout, scene)
}

pub(super) fn directed_energy(
    context: &HudContext,
    designated: Designated<'_>,
    camera: &Camera,
    layout: &Layout,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let u = layout.unit;
    let [cx, cy] = layout.centre;
    let r = camera
        .cone_radius(context.envelope.energy_half_angle_rad)
        .max(8.0 * u);
    scene.stroke(layout.halo(), 3.4 * u)?;
    scene.circle(PaintMode::Stroke, cx, cy, r)?;
    scene.stroke(HUD_GREEN, 1.4 * u)?;
    scene.circle(PaintMode::Stroke, cx, cy, r)?;
    for [dx, dy] in [[1.0, 0.0], [-1.0, 0.0], [0.0, 1.0], [0.0, -1.0]] {
        let (a, b) = (r + 3.0 * u, r + 10.0 * u);
        halo_polyline(
            layout,
            scene,
            &[[cx + dx * a, cy + dy * a], [cx + dx * b, cy + dy * b]],
            HUD_GREEN,
            1.4 * u,
        )?;
    }
    let Some(energy) = context.energy else {
        return Ok(());
    };
    heat_bar(energy, [cx + r + 22.0 * u, cy], layout, scene)?;
    let on_target = designated.is_some_and(|(contact, placed)| {
        contact.status == SignalStatus::Valid
            && placed.range_m <= context.envelope.energy_max_range_m
            && placed
                .screen
                .is_some_and(|[x, y]| libm::hypotf(x - cx, y - cy) <= r)
    });
    let (text, color) = if energy.locked_out {
        ("HOT", safety::CAUTION_AMBER)
    } else if energy.firing {
        ("LASE", HUD_GREEN)
    } else if on_target {
        ("ON TGT", HUD_GREEN)
    } else {
        return Ok(());
    };
    scene.fill_color(layout.ink(color))?;
    Ok(scene.text(
        cx,
        cy + layout.field[1] * 0.55,
        layout.text(13.0),
        Anchor::CENTER,
        text,
    )?)
}

/// A vertical bar that fills with heat; amber near the cut-out, red after it.
fn heat_bar(
    energy: EnergyState,
    at: [f32; 2],
    layout: &Layout,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let u = layout.unit;
    let (w, h) = (6.0 * u, 54.0 * u);
    let [x, y] = [at[0] - w * 0.5, at[1] - h * 0.5];
    let heat = if energy.heat.is_finite() {
        energy.heat.clamp(0.0, 1.0)
    } else {
        1.0
    };
    let color = layout.ink(if energy.locked_out {
        safety::FAILURE_RED
    } else if heat > 0.7 {
        safety::CAUTION_AMBER
    } else {
        HUD_GREEN
    });
    scene.stroke(HUD_GREEN, 1.2 * u)?;
    scene.rect(PaintMode::Stroke, x, y, w, h)?;
    scene.fill_color(color)?;
    scene.rect(PaintMode::Fill, x, y + h * (1.0 - heat), w, h * heat)?;
    Ok(())
}

/// The release state under the field centre, and the laser mark beside the target.
fn release_labels(
    state: &str,
    p: [f32; 3],
    camera: &Camera,
    layout: &Layout,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let u = layout.unit;
    let [cx, cy] = layout.centre;
    let hh = layout.field[1];
    let tint = layout.ink(if state == "MIN RNG" {
        safety::CAUTION_AMBER
    } else {
        HUD_GREEN
    });
    scene.fill_color(tint)?;
    let size = if state == "REL" { 20.0 } else { 13.0 };
    scene.text(cx, cy + hh * 0.55, layout.text(size), Anchor::CENTER, state)?;
    if let Some(at) = camera.project_ned(p) {
        scene.fill_color(layout.ink(HUD_GREEN))?;
        scene.text(
            at.x + 22.0 * u,
            at.y - 18.0 * u,
            layout.text(11.0),
            Anchor::MIDDLE_LEFT,
            "L",
        )?;
    }
    Ok(())
}
