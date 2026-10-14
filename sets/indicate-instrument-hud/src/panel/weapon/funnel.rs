//! The gun funnel: where the rounds fired up to this moment are now.
//!
//! Each sample is a round fired `t` seconds ago. Under a steady manoeuvre the
//! aircraft held the current body rates over that interval, so the round left
//! along the gun line and velocity of `t` seconds ago, then fell under gravity.
//! The two edges are a target wingspan apart at each round's range: a target
//! whose wingtips touch both edges is at the range the rounds are passing.
use super::*;

/// Times of flight sampled along the stream, seconds.
const TIMES: [f32; 8] = [0.25, 0.4, 0.55, 0.7, 0.85, 1.0, 1.15, 1.3];
const KNOT: f32 = 0.514_444;

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn scale(a: [f32; 3], k: f32) -> [f32; 3] {
    [a[0] * k, a[1] * k, a[2] * k]
}

fn norm(a: [f32; 3]) -> f32 {
    libm::sqrtf(a[0] * a[0] + a[1] * a[1] + a[2] * a[2])
}

/// Rotates `v` back by the rotation the angular velocity `w` makes in `t` seconds.
fn back(v: [f32; 3], w: [f32; 3], t: f32) -> [f32; 3] {
    let rate = norm(w);
    if rate < 1e-6 {
        return v;
    }
    let k = scale(w, 1.0 / rate);
    let angle = -rate * t;
    let (s, c) = (libm::sinf(angle), libm::cosf(angle));
    let cross = [
        k[1] * v[2] - k[2] * v[1],
        k[2] * v[0] - k[0] * v[2],
        k[0] * v[1] - k[1] * v[0],
    ];
    let dot = k[0] * v[0] + k[1] * v[1] + k[2] * v[2];
    add(add(scale(v, c), scale(cross, s)), scale(k, dot * (1.0 - c)))
}

/// Position, relative to the aircraft now, of a round fired `t` seconds ago.
pub(crate) fn round(
    velocity: [f32; 3],
    nose: [f32; 3],
    rates: [f32; 3],
    muzzle: f32,
    t: f32,
) -> [f32; 3] {
    const STEPS: usize = 8;
    let dt = t / STEPS as f32;
    let mut origin = [0.0; 3];
    for step in 0..STEPS {
        let s = (step as f32 + 0.5) * dt;
        origin = add(origin, scale(back(velocity, rates, s), -dt));
    }
    let launch = add(
        back(velocity, rates, t),
        scale(back(nose, rates, t), muzzle),
    );
    add(
        add(origin, scale(launch, t)),
        [0.0, 0.0, 0.5 * GRAVITY_MPS2 * t * t],
    )
}

/// The stream in frame coordinates with each round's range, nearest first.
pub(crate) fn stream(
    data: &PanelData,
    context: &HudContext,
    camera: &Camera,
) -> Option<[([f32; 2], f32); TIMES.len()]> {
    let rates = context.body_rates_rps;
    if rates.status != SignalStatus::Valid || !rates.value.iter().all(|r| r.is_finite()) {
        return None;
    }
    let velocity = scale(super::super::ladder::velocity_ned(data)?, KNOT);
    let nose = camera.body_to_ned(context.envelope.gun_line())?;
    let rates = camera.body_to_ned(rates.value)?;
    let mut points = [([0.0; 2], 0.0); TIMES.len()];
    for (slot, t) in points.iter_mut().zip(TIMES) {
        let position = round(velocity, nose, rates, context.envelope.muzzle_speed_mps, t);
        let at = camera.project_ned(position)?;
        *slot = ([at.x, at.y], norm(position));
    }
    Some(points)
}

pub(super) fn draw(
    data: &PanelData,
    context: &HudContext,
    camera: &Camera,
    layout: &Layout,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let u = layout.unit;
    let Some(points) = stream(data, context, camera) else {
        let [cx, cy] = layout.centre;
        scene.fill_color(layout.ink(safety::CAUTION_AMBER))?;
        return Ok(scene.text(
            cx,
            cy + 44.0 * u,
            layout.text(11.0),
            Anchor::CENTER,
            "FUNNEL OFF",
        )?);
    };
    let wingspan = context.envelope.target_wingspan_m;
    let mut edges = [[[0.0_f32; 2]; TIMES.len()]; 2];
    for index in 0..TIMES.len() {
        let before = points[index.saturating_sub(1)].0;
        let after = points[(index + 1).min(TIMES.len() - 1)].0;
        let (tx, ty) = (after[0] - before[0], after[1] - before[1]);
        let length = libm::sqrtf(tx * tx + ty * ty).max(1e-3);
        let normal = [-ty / length, tx / length];
        let ([x, y], range) = points[index];
        let half = camera.focal() * wingspan * 0.5 / range.max(1.0);
        edges[0][index] = [x - normal[0] * half, y - normal[1] * half];
        edges[1][index] = [x + normal[0] * half, y + normal[1] * half];
    }
    // Like the combiner it imitates, the funnel ends at the edge of the field.
    let [cx, cy] = layout.centre;
    let [hw, hh] = layout.field;
    scene.save()?;
    scene.clip_rect(cx - hw, cy - hh, hw * 2.0, hh * 2.0)?;
    for edge in &edges {
        halo_polyline(layout, scene, edge, HUD_GREEN, 1.6 * u)?;
    }
    scene.restore()?;
    Ok(())
}
