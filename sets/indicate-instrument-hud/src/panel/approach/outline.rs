//! The conformal runway outline, clipped in view space so edges that pass
//! behind or beneath the aircraft stay registered.
use super::*;

const VIEW_NEAR_M: f32 = 0.25;
const VIEW_PYRAMID_SLOPE: f32 = 3.0;
const MIN_ROLLOUT_EYE_M: f32 = 1.0;
const CENTRELINE_PITCH_M: f32 = 60.0;
const CENTRELINE_STRIPE_M: f32 = 36.0;

fn runway_station_and_raw_down(runway: &Runway) -> (f32, f32, f32) {
    let (along, right) = runway.axes();
    let t = runway.threshold_m;
    let station_m = -(t[0] * along[0] + t[1] * along[1]);
    let cross_m = -(t[0] * right[0] + t[1] * right[1]);
    let frac = (station_m / runway.length_m).clamp(0.0, 1.0);
    let raw_down_m = t[2] + runway.far_down_offset_m * frac;
    (station_m, cross_m, raw_down_m)
}

pub(super) fn is_on_rollout(runway: &Runway) -> bool {
    let (station_m, cross_m, raw_down_m) = runway_station_and_raw_down(runway);
    station_m > 30.0
        && station_m <= runway.length_m + 60.0
        && cross_m.abs() <= runway.width_m * 2.0
        && raw_down_m.abs() < 8.0
}

fn rollout_vertical_bias_m(runway: &Runway) -> f32 {
    let (station_m, cross_m, raw_down_m) = runway_station_and_raw_down(runway);
    let in_zone = (-90.0..=runway.length_m + 60.0).contains(&station_m)
        && cross_m.abs() <= runway.width_m * 2.0
        && (-8.0..MIN_ROLLOUT_EYE_M).contains(&raw_down_m);
    if in_zone {
        let ramp = ((station_m + 90.0) / 90.0).clamp(0.0, 1.0);
        (MIN_ROLLOUT_EYE_M - raw_down_m) * ramp
    } else {
        0.0
    }
}

fn view_point(camera: &Camera, v: [f32; 3]) -> Option<[f32; 2]> {
    camera.project_view(v).map(|p| [p.x, p.y])
}

fn in_view_pyramid(v: [f32; 3]) -> bool {
    v.iter().all(|c| c.is_finite())
        && v[0] >= VIEW_NEAR_M
        && v[1].abs() <= VIEW_PYRAMID_SLOPE * v[0]
        && v[2].abs() <= VIEW_PYRAMID_SLOPE * v[0]
}

fn clip_view_segment(a: [f32; 3], b: [f32; 3]) -> Option<([f32; 3], [f32; 3])> {
    if !a.iter().all(|c| c.is_finite()) || !b.iter().all(|c| c.is_finite()) {
        return None;
    }
    let m = VIEW_PYRAMID_SLOPE;
    let planes = [
        [a[0] - VIEW_NEAR_M, b[0] - VIEW_NEAR_M],
        [m * a[0] - a[1], m * b[0] - b[1]],
        [m * a[0] + a[1], m * b[0] + b[1]],
        [m * a[0] - a[2], m * b[0] - b[2]],
        [m * a[0] + a[2], m * b[0] + b[2]],
    ];
    let (mut t0, mut t1) = (0.0_f32, 1.0_f32);
    for [f0, f1] in planes {
        if f0 < 0.0 && f1 < 0.0 {
            return None;
        }
        let df = f1 - f0;
        if f0 < 0.0 {
            t0 = t0.max((-f0 / df).clamp(0.0, 1.0));
        } else if f1 < 0.0 {
            t1 = t1.min((-f0 / df).clamp(0.0, 1.0));
        }
        if t0 > t1 {
            return None;
        }
    }
    let at = |t: f32| {
        [
            a[0] + (b[0] - a[0]) * t,
            a[1] + (b[1] - a[1]) * t,
            a[2] + (b[2] - a[2]) * t,
        ]
    };
    Some((at(t0), at(t1)))
}

fn draw_clipped_edge(
    camera: &Camera,
    layout: &Layout,
    scene: &mut SceneWriter<'_>,
    a_ned: [f32; 3],
    b_ned: [f32; 3],
    width: f32,
) -> Result<(), PanelDrawError> {
    let (Some(va), Some(vb)) = (camera.ned_to_view(a_ned), camera.ned_to_view(b_ned)) else {
        return Ok(());
    };
    let Some((ca, cb)) = clip_view_segment(va, vb) else {
        return Ok(());
    };
    if let (Some(pa), Some(pb)) = (view_point(camera, ca), view_point(camera, cb)) {
        halo_polyline(layout, scene, &[pa, pb], HUD_GREEN, width)?;
    }
    Ok(())
}

/// The runway edges and threshold, and dashes along the extended centreline.
///
/// When all four corners sit inside the forward view pyramid, the closed
/// rectangle is emitted as a single five-point polyline; once any corner passes
/// behind or steeply beneath the aircraft (flare, rollout, or overflight), each
/// edge is clipped in 3D view space so sidelines remain registered without
/// projecting extreme off-boresight coordinates.
pub(super) fn outline(
    runway: &Runway,
    camera: &Camera,
    layout: &Layout,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let u = layout.unit;
    let (along, right) = runway.axes();
    let t = runway.threshold_m;
    let z_bias = rollout_vertical_bias_m(runway);
    let slope_per_m = runway.far_down_offset_m / runway.length_m.max(1.0);
    let at = |a: f32, r: f32| {
        let clamped_a = a.clamp(-runway.pre_threshold_m, runway.length_m);
        [
            t[0] + along[0] * a + right[0] * r,
            t[1] + along[1] * a + right[1] * r,
            t[2] + z_bias + slope_per_m * clamped_a,
        ]
    };
    let half = runway.width_m * 0.5;
    let primary_len = runway.primary_length_m();
    primary_surface(runway, camera, layout, scene, &at)?;
    if runway.pre_threshold_m > 10.0 {
        let start = -runway.pre_threshold_m;
        unusable_zone(camera, layout, scene, &at, [start, 0.0], start, half)?;
    }
    if primary_len + 10.0 < runway.length_m {
        let span = [primary_len, runway.length_m];
        unusable_zone(camera, layout, scene, &at, span, runway.length_m, half)?;
    }
    let lead_origin = -runway.pre_threshold_m.max(0.0);
    for k in 1..=6 {
        let start = lead_origin - 150.0 * k as f32;
        let (a, b) = (at(start, 0.0), at(start + 60.0, 0.0));
        draw_clipped_edge(camera, layout, scene, a, b, 1.4 * u)?;
    }
    if !runway.unpaved {
        draw_interior_centreline(runway, primary_len, camera, layout, scene, &at)?;
    }
    Ok(())
}

/// The usable surface: a closed rectangle when paved, or threshold and end
/// bars joined by dashed sidelines when unpaved.
fn primary_surface(
    runway: &Runway,
    camera: &Camera,
    layout: &Layout,
    scene: &mut SceneWriter<'_>,
    at: &impl Fn(f32, f32) -> [f32; 3],
) -> Result<(), PanelDrawError> {
    let u = layout.unit;
    let half = runway.width_m * 0.5;
    let primary_len = runway.primary_length_m();
    if runway.unpaved {
        let (a, b) = (at(0.0, -half), at(0.0, half));
        draw_clipped_edge(camera, layout, scene, a, b, 2.1 * u)?;
        let (a, b) = (at(primary_len, -half), at(primary_len, half));
        draw_clipped_edge(camera, layout, scene, a, b, 1.8 * u)?;
        return draw_dashed_sidelines(camera, layout, scene, at, 0.0, primary_len, half, 1.6 * u);
    }
    let corners = [
        at(0.0, -half),
        at(0.0, half),
        at(primary_len, half),
        at(primary_len, -half),
    ];
    let in_pyramid = corners
        .iter()
        .all(|&c| camera.ned_to_view(c).is_some_and(in_view_pyramid));
    if in_pyramid {
        if let [Some(a), Some(b), Some(c), Some(d)] = corners.map(|v| point(camera, v)) {
            halo_polyline(layout, scene, &[a, b, c, d, a], HUD_GREEN, 1.8 * u)?;
        }
        return Ok(());
    }
    for [c0, c1] in [
        [corners[0], corners[1]],
        [corners[1], corners[2]],
        [corners[2], corners[3]],
        [corners[3], corners[0]],
    ] {
        draw_clipped_edge(camera, layout, scene, c0, c1, 1.8 * u)?;
    }
    Ok(())
}

/// A displaced threshold or an overrun: dashed sidelines over `span` and a
/// short bar across the runway at `bar_m`.
fn unusable_zone(
    camera: &Camera,
    layout: &Layout,
    scene: &mut SceneWriter<'_>,
    at: &impl Fn(f32, f32) -> [f32; 3],
    span: [f32; 2],
    bar_m: f32,
    half: f32,
) -> Result<(), PanelDrawError> {
    let w = 1.3 * layout.unit;
    let [start, end] = span;
    draw_dashed_sidelines(camera, layout, scene, at, start, end, half, w)?;
    let (a, b) = (at(bar_m, -half * 0.6), at(bar_m, half * 0.6));
    draw_clipped_edge(camera, layout, scene, a, b, w)
}

#[allow(clippy::too_many_arguments)]
fn draw_dashed_sidelines(
    camera: &Camera,
    layout: &Layout,
    scene: &mut SceneWriter<'_>,
    at: &impl Fn(f32, f32) -> [f32; 3],
    start_m: f32,
    end_m: f32,
    half_width_m: f32,
    stroke_w: f32,
) -> Result<(), PanelDrawError> {
    let span = end_m - start_m;
    if span <= 4.0 {
        return Ok(());
    }
    let pitch = (span / 6.0).clamp(55.0, 140.0);
    let dash = pitch * 0.40;
    let steps = (libm::ceilf(span / pitch) as u32).clamp(1, 64);
    for idx in 0..steps {
        let s0 = start_m + idx as f32 * pitch;
        let s1 = (s0 + dash).min(end_m);
        if s0 >= s1 {
            break;
        }
        for side in [-half_width_m, half_width_m] {
            draw_clipped_edge(camera, layout, scene, at(s0, side), at(s1, side), stroke_w)?;
        }
    }
    Ok(())
}

fn draw_interior_centreline(
    runway: &Runway,
    primary_len: f32,
    camera: &Camera,
    layout: &Layout,
    scene: &mut SceneWriter<'_>,
    at: &impl Fn(f32, f32) -> [f32; 3],
) -> Result<(), PanelDrawError> {
    let (station_m, _, _) = runway_station_and_raw_down(runway);
    if !(-900.0..=runway.length_m).contains(&station_m) {
        return Ok(());
    }
    let max_end = primary_len - 30.0;
    let first_idx = if station_m > 30.0 {
        libm::floorf((station_m - 30.0) / CENTRELINE_PITCH_M).max(0.0) as u32
    } else {
        0
    };
    for step in 0..24_u32 {
        let start = 30.0 + (first_idx.saturating_add(step) as f32) * CENTRELINE_PITCH_M;
        let end = (start + CENTRELINE_STRIPE_M).min(max_end);
        if start >= end {
            break;
        }
        draw_clipped_edge(
            camera,
            layout,
            scene,
            at(start, 0.0),
            at(end, 0.0),
            1.4 * layout.unit,
        )?;
    }
    Ok(())
}
