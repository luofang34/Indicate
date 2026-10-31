//! `orrery-volume`: 3D celestial system, orbital rings, transfer arc, and manoeuvre nodes (`VOL-01`).

use indicate_frames::Quat;
use indicate_instrument_descriptor::{
    AuxChannelId, DrawContext, GroupSet, InstrumentDescriptor, PanelDrawError, SceneTarget,
    UnderlayContract,
};
use indicate_instrument_scene::{
    HAlign, PaintMode,
    volume::{Extent3D, HandleKind, VolumeBandId, VolumeSceneWriter, VolumeUnderlayKind},
};
use indicate_instrument_state::{GroupId, SignalStatus};
use indicate_instrument_symbology::{fmt_label, palette, safety};
use indicate_intent::IntentClass;
use libm::{cosf, sinf};

use crate::aux_wire::CelestialEphemerisAux;
use crate::shared::{
    ADJUST_MANOEUVRE_NODE, REQUIRED_VOLUME_BANDS, SELECT_WAYPOINT_3D, VOLUME_SPACE,
    clamp_inside_extent, emit_annunciation_band, emit_reference_grid, status_ink, usable_sig,
    view_point,
};

/// Descriptor for the 3D celestial system (`id: "orrery-volume"`, `VOL-01`).
pub const ORRERY_VOLUME_DESCRIPTOR: InstrumentDescriptor = InstrumentDescriptor {
    id: "orrery-volume",
    title: "3D Celestial System",
    space: VOLUME_SPACE,
    required_layers: REQUIRED_VOLUME_BANDS,
    required_groups: GroupSet::of(&[GroupId::Kinematics, GroupId::Trust]),
    underlay: UnderlayContract::CelestialSystem3D,
    config_schema: &[],
    accepted_intents: &[
        IntentClass::DECLUTTER,
        SELECT_WAYPOINT_3D,
        ADJUST_MANOEUVRE_NODE,
    ],
    aux_channels: &[AuxChannelId::CELESTIAL_EPHEMERIS_3D],
    group_regions: &[],
    extreme_states: &[],
    raster_baselines: &[],
    draw: draw_orrery_entry,
};

fn draw_orrery_entry(
    ctx: &DrawContext<'_>,
    target: SceneTarget<'_, '_>,
) -> Result<(), PanelDrawError> {
    let SceneTarget::Volume3D { extent_m, writer } = target else {
        return Err(PanelDrawError::WrongTarget);
    };
    ctx.config().require_schema(&[])?;
    draw_orrery_volume(ctx, extent_m, writer)
}

/// Renders the `orrery-volume` 3D scene for `ctx` inside `extent_m`.
pub fn draw_orrery_volume(
    ctx: &DrawContext<'_>,
    extent_m: Extent3D,
    writer: &mut VolumeSceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let view = ctx.view();
    let eph = match ctx.aux(AuxChannelId::CELESTIAL_EPHEMERIS_3D) {
        Some(raw) => Some(CelestialEphemerisAux::decode(raw)?),
        None => None,
    };
    let span = extent_m.width_m.min(extent_m.depth_m);
    let r_inner = span * eph.map_or(0.20, |e| e.inner_orbit_frac);
    let r_outer = span * eph.map_or(0.36, |e| e.outer_orbit_frac);
    let scrub_rad = ((view.time_offset_s / 600.0)
        .clamp(-core::f64::consts::TAU, core::f64::consts::TAU)) as f32;
    let phase = eph.map_or(0.45_f32, |e| e.phase_rad) + scrub_rad;

    emit_reference_grid(extent_m, view.decluttered, writer)?;
    emit_orrery_surface(extent_m, eph.as_ref(), writer)?;
    emit_orrery_orbits(extent_m, r_inner, r_outer, phase, ctx, writer)?;
    emit_orrery_bodies(extent_m, r_inner, r_outer, phase, ctx, writer)?;
    emit_orrery_billboards(ctx, extent_m, writer)?;
    emit_orrery_handles(ctx, extent_m, r_inner, r_outer, phase, writer)?;
    emit_annunciation_band(ctx, extent_m, "ORRERY VOL", writer)
}

fn emit_orrery_surface(
    extent_m: Extent3D,
    eph: Option<&CelestialEphemerisAux>,
    writer: &mut VolumeSceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let mpu = eph.map_or(1.0e9, |e| e.meters_per_unit);
    writer.begin_band(VolumeBandId::SurfaceUnderlay)?;
    writer.underlay_anchor_3d(
        VolumeUnderlayKind::CelestialSystem,
        [0.0, 0.0, 0.0],
        [
            extent_m.width_m * 0.92,
            extent_m.height_m * 0.80,
            extent_m.depth_m * 0.92,
        ],
        Quat::IDENTITY,
        mpu,
    )?;
    writer.end_band(VolumeBandId::SurfaceUnderlay)?;
    Ok(())
}

fn emit_orrery_orbits(
    extent_m: Extent3D,
    r_inner: f32,
    r_outer: f32,
    phase: f32,
    ctx: &DrawContext<'_>,
    writer: &mut VolumeSceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let view = ctx.view();
    writer.begin_band(VolumeBandId::Trajectories3D)?;
    writer.stroke_3d(palette::GREY, 0.0014)?;
    writer.ring_3d([0.0, 0.0, 0.0], [0.0, 1.0, 0.0], r_inner.max(0.03))?;
    writer.ring_3d([0.0, 0.0, 0.0], [0.0, 1.0, 0.0], r_outer.max(0.05))?;

    if usable_sig(ctx.data().gs_kt) {
        writer.stroke_3d(palette::MAGENTA, 0.0022)?;
        let mut arc = [[0.0_f32; 3]; 5];
        let mut idx = 0usize;
        while idx < 5 {
            let t = (idx as f32) * 0.25;
            let rad = r_inner + (r_outer - r_inner) * t;
            let ang = phase + t * 2.2;
            let p = [
                rad * cosf(ang),
                0.01 * sinf(t * core::f32::consts::PI),
                rad * sinf(ang),
            ];
            if let Some(slot) = arc.get_mut(idx) {
                *slot = view_point(p, view, extent_m);
            }
            idx = idx.wrapping_add(1);
        }
        writer.polyline_3d(&arc)?;
    }
    writer.end_band(VolumeBandId::Trajectories3D)?;
    Ok(())
}

fn emit_orrery_bodies(
    extent_m: Extent3D,
    r_inner: f32,
    r_outer: f32,
    phase: f32,
    ctx: &DrawContext<'_>,
    writer: &mut VolumeSceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let view = ctx.view();
    let star_r = (extent_m.width_m * 0.045).clamp(0.010, 0.045);
    let body_r = (extent_m.width_m * 0.025).clamp(0.006, 0.025);
    let primary = view_point([0.0, 0.0, 0.0], view, extent_m);
    let inner_body = view_point(
        [r_inner * cosf(phase), 0.0, r_inner * sinf(phase)],
        view,
        extent_m,
    );
    let outer_ang = phase * 0.5 + 1.1;
    let outer_body = view_point(
        [r_outer * cosf(outer_ang), 0.0, r_outer * sinf(outer_ang)],
        view,
        extent_m,
    );
    writer.begin_band(VolumeBandId::Symbols3D)?;
    writer.fill_color(safety::REFERENCE_YELLOW)?;
    writer.sphere(PaintMode::Fill, primary, star_r)?;
    writer.fill_color(palette::SKY)?;
    writer.sphere(PaintMode::Fill, inner_body, body_r)?;
    writer.fill_color(palette::GREEN)?;
    writer.sphere(PaintMode::Fill, outer_body, body_r * 0.85)?;
    writer.end_band(VolumeBandId::Symbols3D)?;
    Ok(())
}

fn emit_orrery_billboards(
    ctx: &DrawContext<'_>,
    extent_m: Extent3D,
    writer: &mut VolumeSceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let data = ctx.data();
    let view = ctx.view();
    let [hx, hy, _hz] = extent_m.half_extents();
    let sz = (extent_m.height_m * 0.05).clamp(0.012, 0.048);
    writer.begin_band(VolumeBandId::Billboards)?;
    let gs = data.gs_kt;
    writer.fill_color(status_ink(gs.status))?;
    if usable_sig(gs) {
        let lbl = fmt_label!(24, "VEL {:.0} kt", gs.value);
        writer.attributed_billboard_text(
            GroupId::Kinematics.to_u8(),
            [-hx * 0.65, hy * 0.55, 0.0],
            sz,
            HAlign::Left,
            lbl.as_str(),
        )?;
    } else {
        writer.billboard_text([-hx * 0.65, hy * 0.55, 0.0], sz, HAlign::Left, "VEL ----")?;
    }
    if view.time_offset_s.abs() > 0.5 {
        writer.fill_color(palette::CYAN)?;
        let tag = if view.time_offset_s > 0.0 {
            "SCRUB FWD"
        } else {
            "SCRUB AFT"
        };
        writer.billboard_text([hx * 0.65, hy * 0.55, 0.0], sz * 0.9, HAlign::Right, tag)?;
    }
    if gs.status == SignalStatus::Stale {
        writer.fill_color(safety::CAUTION_AMBER)?;
        writer.billboard_text([0.0, hy * 0.68, 0.0], sz * 0.85, HAlign::Center, "STALE")?;
    }
    writer.end_band(VolumeBandId::Billboards)?;
    Ok(())
}

fn emit_orrery_handles(
    ctx: &DrawContext<'_>,
    extent_m: Extent3D,
    r_inner: f32,
    r_outer: f32,
    phase: f32,
    writer: &mut VolumeSceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let view = ctx.view();
    let burn_a = clamp_inside_extent(
        view_point(
            [r_inner * cosf(phase), 0.0, r_inner * sinf(phase)],
            view,
            extent_m,
        ),
        extent_m,
        0.02,
    );
    let arr_ang = phase + 2.2;
    let burn_b = clamp_inside_extent(
        view_point(
            [r_outer * cosf(arr_ang), 0.0, r_outer * sinf(arr_ang)],
            view,
            extent_m,
        ),
        extent_m,
        0.02,
    );
    let hr = (extent_m.width_m * 0.04).clamp(0.01, 0.035);
    writer.begin_band(VolumeBandId::Handles)?;
    writer.target_handle(1, HandleKind::ManoeuvreNode, burn_a, hr)?;
    writer.target_handle(2, HandleKind::ManoeuvreNode, burn_b, hr)?;
    writer.end_band(VolumeBandId::Handles)?;
    Ok(())
}
