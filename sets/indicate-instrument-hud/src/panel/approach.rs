//! Approach symbology: the conformal runway, the glide path reference line,
//! and receiver deviations.
//!
//! The runway outline and the reference line come from the host's runway
//! geometry and are world cues. The deviations come from the navigation group,
//! so they keep the receiver's own validity and provenance.
use super::*;
use crate::runway::Runway;
use indicate_instrument_state::{NavFromTo, NavScale, NavSource};

const DEG: f32 = core::f32::consts::PI / 180.0;

pub(super) fn draw(
    data: &PanelData,
    runway: &Runway,
    aoa_band: Option<[f32; 2]>,
    camera: &Camera,
    layout: &Layout,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let [cx, cy] = layout.centre;
    let [hw, hh] = layout.field;
    scene.save()?;
    scene.clip_rect(cx - hw, cy - hh, hw * 2.0, hh * 2.0)?;
    outline(runway, camera, layout, scene)?;
    glide_path(runway, camera, layout, scene)?;
    scene.restore()?;
    if let Some(band) = aoa_band {
        aoa_bracket(data, band, camera, layout, scene)?;
    }
    deviations(data, layout, scene)
}

/// The on-speed bracket: an E at the band's depression below the boresight,
/// beside the flight path marker. Its top edge is the fast limit.
fn aoa_bracket(
    data: &PanelData,
    band: [f32; 2],
    camera: &Camera,
    layout: &Layout,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let [fast, slow] = band;
    if !(fast.is_finite() && slow.is_finite() && 0.0 < fast && fast < slow && slow < 0.5) {
        return Ok(());
    }
    let below = |angle: f32| camera.project_body([libm::cosf(angle), 0.0, libm::sinf(angle)]);
    let (Some(top), Some(bottom)) = (below(fast), below(slow)) else {
        return Ok(());
    };
    let u = layout.unit;
    let marker = super::ladder::velocity_ned(data).and_then(|v| camera.project_ned(v));
    let x = marker.map_or(top.x, |m| m.x) - 22.0 * u;
    let mid = (top.y + bottom.y) * 0.5;
    let tick = 8.0 * u;
    halo_polyline(
        layout,
        scene,
        &[
            [x + tick, top.y],
            [x, top.y],
            [x, bottom.y],
            [x + tick, bottom.y],
        ],
        HUD_GREEN,
        1.8 * u,
    )?;
    halo_polyline(
        layout,
        scene,
        &[[x, mid], [x + tick * 0.7, mid]],
        HUD_GREEN,
        1.8 * u,
    )
}

fn point(camera: &Camera, v: [f32; 3]) -> Option<[f32; 2]> {
    camera.project_ned(v).map(|p| [p.x, p.y])
}

/// The runway edges and threshold, and dashes along the extended centreline.
fn outline(
    runway: &Runway,
    camera: &Camera,
    layout: &Layout,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let u = layout.unit;
    let (along, right) = runway.axes();
    let t = runway.threshold_m;
    let at = |a: f32, r: f32| {
        [
            t[0] + along[0] * a + right[0] * r,
            t[1] + along[1] * a + right[1] * r,
            t[2],
        ]
    };
    let half = runway.width_m * 0.5;
    let corners = [
        at(0.0, -half),
        at(0.0, half),
        at(runway.length_m, half),
        at(runway.length_m, -half),
    ];
    if let [Some(a), Some(b), Some(c), Some(d)] = corners.map(|v| point(camera, v)) {
        halo_polyline(layout, scene, &[a, b, c, d, a], HUD_GREEN, 1.8 * u)?;
    }
    for k in 1..=6 {
        let start = -150.0 * k as f32;
        if let (Some(a), Some(b)) = (
            point(camera, at(start, 0.0)),
            point(camera, at(start + 60.0, 0.0)),
        ) {
            halo_polyline(layout, scene, &[a, b], HUD_GREEN, 1.4 * u)?;
        }
    }
    Ok(())
}

/// A dashed line at the glide path angle below the horizon, along the runway course.
///
/// Holding the flight path marker on it flies the glide path angle.
fn glide_path(
    runway: &Runway,
    camera: &Camera,
    layout: &Layout,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let u = layout.unit;
    let depression = runway.glide_path_rad;
    let direction = |offset: f32| {
        let azimuth = runway.course_rad + offset;
        [
            libm::cosf(depression) * libm::cosf(azimuth),
            libm::cosf(depression) * libm::sinf(azimuth),
            libm::sinf(depression),
        ]
    };
    for k in -6..=6 {
        if k == 0 {
            continue;
        }
        let a = k as f32 * 2.0 * DEG;
        if let (Some(p), Some(q)) = (
            point(camera, direction(a - 0.7 * DEG)),
            point(camera, direction(a + 0.7 * DEG)),
        ) {
            halo_polyline(layout, scene, &[p, q], HUD_GREEN, 1.4 * u)?;
        }
    }
    if let Some([x, y]) = point(camera, direction(13.5 * DEG)) {
        scene.fill_color(HUD_GREEN)?;
        scene.text(x + 6.0 * u, y, layout.text(10.0), Anchor::MIDDLE_LEFT, "GP")?;
    }
    Ok(())
}

fn available(data: &PanelData) -> bool {
    let nav = &data.nav.data;
    data.nav.status == SignalStatus::Valid
        && matches!(
            nav.source,
            NavSource::Nav1 | NavSource::Nav2 | NavSource::Gps
        )
        && matches!(nav.fromto, NavFromTo::To | NavFromTo::From)
        && nav.scale == NavScale::Approach
        && nav.cdi_dots.is_finite()
}

/// Lateral and vertical deviation scales; the diamonds sit where the course and path are.
fn deviations(
    data: &PanelData,
    layout: &Layout,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let u = layout.unit;
    let [cx, cy] = layout.centre;
    let [hw, hh] = layout.field;
    let dot = 18.0 * u;
    let lateral = [cx, cy + hh * 0.62];
    let vertical = [cx + hw - 44.0 * u, cy];
    if !available(data) {
        return unavailable(data, lateral, layout, scene);
    }
    let nav = &data.nav.data;
    for k in [-2.0_f32, -1.0, 1.0, 2.0] {
        scene.stroke(HUD_GREEN, 1.3 * u)?;
        scene.circle(PaintMode::Stroke, lateral[0] + k * dot, lateral[1], 3.0 * u)?;
        if nav.vdev_dots.is_some() {
            scene.circle(
                PaintMode::Stroke,
                vertical[0],
                vertical[1] + k * dot,
                3.0 * u,
            )?;
        }
    }
    diamond(
        [lateral[0] + nav.cdi_dots.clamp(-2.4, 2.4) * dot, lateral[1]],
        layout,
        scene,
    )?;
    let label = if nav.source == NavSource::Gps {
        "LNAV"
    } else {
        "LOC"
    };
    scene.fill_color(HUD_GREEN)?;
    scene.text_attributed(
        GroupId::Nav.to_u8(),
        lateral[0],
        lateral[1] + 14.0 * u,
        layout.text(10.0),
        Anchor::CENTER,
        label,
    )?;
    if let Some(vdev) = nav.vdev_dots.filter(|v| v.is_finite()) {
        diamond(
            [vertical[0], vertical[1] + vdev.clamp(-2.4, 2.4) * dot],
            layout,
            scene,
        )?;
        let label = if nav.source == NavSource::Gps {
            "VNAV"
        } else {
            "GS"
        };
        scene.fill_color(HUD_GREEN)?;
        scene.text_attributed(
            GroupId::Nav.to_u8(),
            vertical[0],
            vertical[1] - 3.0 * dot,
            layout.text(10.0),
            Anchor::CENTER,
            label,
        )?;
    }
    Ok(())
}

fn diamond(
    at: [f32; 2],
    layout: &Layout,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let r = 6.0 * layout.unit;
    let [x, y] = at;
    halo_polyline(
        layout,
        scene,
        &[[x, y - r], [x + r, y], [x, y + r], [x - r, y], [x, y - r]],
        HUD_GREEN,
        1.8 * layout.unit,
    )
}

/// The receiver status where the lateral scale would be.
fn unavailable(
    data: &PanelData,
    lateral: [f32; 2],
    layout: &Layout,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let status = if data.nav.status == SignalStatus::Valid {
        SignalStatus::Degraded
    } else {
        data.nav.status
    };
    scene.fill_color(status_color(layout, status))?;
    let text = fmt_label!(
        24,
        "LOC GS {}",
        if status == SignalStatus::Degraded {
            "UNAVAILABLE"
        } else {
            status_label(status)
        }
    );
    Ok(scene.text(
        lateral[0],
        lateral[1],
        layout.text(11.0),
        Anchor::CENTER,
        text.as_str(),
    )?)
}
