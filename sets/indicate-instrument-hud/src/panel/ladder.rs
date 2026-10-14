//! Conformal pitch ladder, ghost horizon, flight path marker, and bank scale.
use super::rung::{self, Rung};
use super::*;

const DEG: f32 = core::f32::consts::PI / 180.0;

/// The level-frame azimuth of the ladder centre: the ground track when the
/// flight path is known, so the ladder and the flight path marker share a column.
fn ladder_azimuth(data: &PanelData, camera: &Camera) -> f32 {
    let (Some(heading), Some(track), Some(speed)) = (
        camera.true_heading(),
        live(data.track_rad),
        live(data.gs_kt),
    ) else {
        return 0.0;
    };
    if speed < 20.0 {
        return 0.0;
    }
    let drift = libm::remainderf(track - heading, 2.0 * core::f32::consts::PI);
    drift.clamp(-0.35, 0.35)
}

fn elevation(azimuth: f32, angle: f32) -> [f32; 3] {
    [
        libm::cosf(angle) * libm::cosf(azimuth),
        libm::cosf(angle) * libm::sinf(azimuth),
        -libm::sinf(angle),
    ]
}

pub(super) fn draw(
    data: &PanelData,
    camera: &Camera,
    layout: &Layout,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    waterline(layout, scene)?;
    if !camera.level_basis() {
        return Ok(());
    }
    let [cx, cy] = layout.centre;
    let [hw, hh] = layout.field;
    scene.save()?;
    // The top band of the field holds the heading readout.
    let top = cy - hh + 34.0 * layout.unit;
    scene.clip_rect(cx - hw, top, hw * 2.0, cy + hh - top)?;
    let azimuth = ladder_azimuth(data, camera);
    for degrees in (-90..=90).step_by(5) {
        rung(camera, layout, azimuth, degrees, scene)?;
    }
    ghost_horizon(
        data,
        camera,
        layout,
        azimuth,
        [cx - hw, top, cx + hw, cy + hh],
        scene,
    )?;
    scene.restore()?;
    flight_path(data, camera, layout, scene)?;
    bank_scale(data, layout, scene)
}

/// The fixed aircraft reference at the boresight.
pub(super) fn waterline(
    layout: &Layout,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let [cx, cy] = layout.centre;
    let u = layout.unit;
    let points = [
        [cx - 22.0 * u, cy],
        [cx - 10.0 * u, cy],
        [cx - 5.0 * u, cy + 7.0 * u],
        [cx, cy],
        [cx + 5.0 * u, cy + 7.0 * u],
        [cx + 10.0 * u, cy],
        [cx + 22.0 * u, cy],
    ];
    halo_polyline(layout, scene, &points, HUD_GREEN, 1.6 * u)
}

fn rung(
    camera: &Camera,
    layout: &Layout,
    azimuth: f32,
    degrees: i32,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let angle = degrees as f32 * DEG;
    let at = |a: f32, e: f32| {
        camera
            .level_to_body(azimuth, elevation(a, e))
            .and_then(|v| camera.project_body(v))
    };
    if degrees.abs() == 90 {
        return match at(0.0, angle) {
            Some(p) => rung::pole([p.x, p.y], degrees > 0, layout, scene),
            None => Ok(()),
        };
    }
    let (Some(centre), Some(left), Some(right)) = (at(0.0, angle), at(-DEG, angle), at(DEG, angle))
    else {
        return Ok(());
    };
    let Some(along) = unit([right.x - left.x, right.y - left.y]) else {
        return Ok(());
    };
    let reach = layout.field[0].max(layout.field[1]) * 1.6;
    if (centre.x - layout.centre[0]).abs() > reach || (centre.y - layout.centre[1]).abs() > reach {
        return Ok(());
    }
    let toward = at(0.0, angle - DEG * degrees.signum() as f32)
        .and_then(|p| unit([p.x - centre.x, p.y - centre.y]))
        .unwrap_or([0.0, 0.0]);
    let u = layout.unit;
    Rung {
        centre: [centre.x, centre.y],
        along,
        toward,
        half: if degrees == 0 {
            layout.field[0] * 1.4
        } else {
            layout.field[0] * 0.42
        },
        gap: if degrees == 0 { 34.0 * u } else { 22.0 * u },
    }
    .draw(degrees, layout, scene)
}

/// A dashed line at the field edge nearest the horizon, drawn while the
/// horizon is outside the field. It is a direction cue, not a registered line.
fn ghost_horizon(
    data: &PanelData,
    camera: &Camera,
    layout: &Layout,
    azimuth: f32,
    field: [f32; 4],
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let [left, top, right, bottom] = field;
    let [cx, cy] = layout.centre;
    let horizon = camera
        .level_to_body(azimuth, elevation(0.0, 0.0))
        .and_then(|v| camera.project_body(v));
    if horizon.is_some_and(|p| p.x > left && p.x < right && p.y > top && p.y < bottom) {
        return Ok(());
    }
    let (Some(roll), Some(pitch)) = (live(data.roll_rad), live(data.pitch_rad)) else {
        return Ok(());
    };
    let below = [libm::sinf(roll), libm::cosf(roll)];
    let sign = if pitch >= 0.0 { 1.0 } else { -1.0 };
    let direction = horizon
        .and_then(|p| unit([p.x - cx, p.y - cy]))
        .unwrap_or([below[0] * sign, below[1] * sign]);
    let reach_x = (right - cx) / direction[0].abs().max(1e-6);
    let reach_y = if direction[1] >= 0.0 {
        (bottom - cy) / direction[1].max(1e-6)
    } else {
        (cy - top) / (-direction[1]).max(1e-6)
    };
    let u = layout.unit;
    let distance = reach_x.min(reach_y) - 10.0 * u;
    let mid = [cx + direction[0] * distance, cy + direction[1] * distance];
    let along = [-direction[1], direction[0]];
    let half = layout.field[0] * 1.2;
    let mut start = -half;
    while start < half {
        let a = [mid[0] + along[0] * start, mid[1] + along[1] * start];
        let b = [
            mid[0] + along[0] * (start + 18.0 * u),
            mid[1] + along[1] * (start + 18.0 * u),
        ];
        halo_polyline(layout, scene, &[a, b], HUD_GREEN, 1.6 * u)?;
        start += 30.0 * u;
    }
    Ok(())
}

fn unit(v: [f32; 2]) -> Option<[f32; 2]> {
    let length = libm::sqrtf(v[0] * v[0] + v[1] * v[1]);
    (length.is_finite() && length > 1e-6).then(|| [v[0] / length, v[1] / length])
}

/// The flight path vector; it needs the true heading to share a frame with the track.
pub(super) fn velocity_ned(data: &PanelData) -> Option<[f32; 3]> {
    let (speed, track, climb) = (
        live(data.gs_kt)?,
        live(data.track_rad)?,
        live(data.vsi_fpm)?,
    );
    let climb_kt = climb / 101.268_6;
    if libm::sqrtf(speed * speed + climb_kt * climb_kt) < 5.0 {
        return None;
    }
    Some([
        speed * libm::cosf(track),
        speed * libm::sinf(track),
        -climb_kt,
    ])
}

fn flight_path(
    data: &PanelData,
    camera: &Camera,
    layout: &Layout,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let Some(point) = velocity_ned(data).and_then(|v| camera.project_ned(v)) else {
        return Ok(());
    };
    let u = layout.unit;
    let [cx, cy] = layout.centre;
    let [hw, hh] = layout.field;
    let x = point.x.clamp(cx - hw, cx + hw);
    let y = point.y.clamp(cy - hh, cy + hh);
    let limited = x != point.x || y != point.y;
    let r = 7.0 * u;
    scene.stroke(layout.halo(), 3.6 * u)?;
    scene.circle(PaintMode::Stroke, x, y, r)?;
    scene.stroke(HUD_GREEN, 1.6 * u)?;
    scene.circle(PaintMode::Stroke, x, y, r)?;
    halo_polyline(
        layout,
        scene,
        &[[x - r - 12.0 * u, y], [x - r, y]],
        HUD_GREEN,
        1.6 * u,
    )?;
    halo_polyline(
        layout,
        scene,
        &[[x + r, y], [x + r + 12.0 * u, y]],
        HUD_GREEN,
        1.6 * u,
    )?;
    halo_polyline(
        layout,
        scene,
        &[[x, y - r], [x, y - r - 8.0 * u]],
        HUD_GREEN,
        1.6 * u,
    )?;
    if limited {
        halo_polyline(
            layout,
            scene,
            &[[x - r, y - r], [x + r, y + r]],
            safety::CAUTION_AMBER,
            1.6 * u,
        )?;
        halo_polyline(
            layout,
            scene,
            &[[x - r, y + r], [x + r, y - r]],
            safety::CAUTION_AMBER,
            1.6 * u,
        )?;
    }
    Ok(())
}

/// A fixed bank scale under the boresight with a pointer toward the ground.
pub(super) fn bank_scale(
    data: &PanelData,
    layout: &Layout,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let Some(roll) = live(data.roll_rad) else {
        return Ok(());
    };
    let u = layout.unit;
    let [cx, cy] = layout.centre;
    let radius = layout.field[1] * 0.92;
    let at = |angle: f32, r: f32| [cx + r * libm::sinf(angle), cy + r * libm::cosf(angle)];
    for degrees in [-60, -45, -30, -20, -10, 0, 10, 20, 30, 45, 60] {
        let angle = degrees as f32 * DEG;
        let length = if degrees % 30 == 0 { 12.0 } else { 7.0 } * u;
        halo_polyline(
            layout,
            scene,
            &[at(angle, radius), at(angle, radius + length)],
            HUD_GREEN,
            1.4 * u,
        )?;
    }
    let tip = at(roll, radius - 2.0 * u);
    let left = at(roll - 3.0 * DEG, radius - 14.0 * u);
    let right = at(roll + 3.0 * DEG, radius - 14.0 * u);
    halo_polyline(layout, scene, &[left, tip, right, left], HUD_GREEN, 1.6 * u)
}
