//! `terrain-volume`: 3D terrain relief, flight trajectory, ownship, and bugs (`VOL-01`).

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

use crate::aux_wire::TerrainPatchAux;
use crate::shared::{
    REQUIRED_VOLUME_BANDS, SELECT_WAYPOINT_3D, VOLUME_SPACE, altitude_group, clamp_inside_extent,
    emit_annunciation_band, emit_reference_grid, status_ink, usable_sig, view_point,
};

const DEFAULT_HEIGHTS: [f32; 16] = [
    -0.12, -0.10, -0.08, -0.11, //
    -0.09, -0.03, 0.02, -0.06, //
    -0.08, 0.01, 0.05, -0.04, //
    -0.11, -0.07, -0.05, -0.10,
];

/// Descriptor for the 3D terrain relief volume (`id: "terrain-volume"`, `VOL-01`).
pub const TERRAIN_VOLUME_DESCRIPTOR: InstrumentDescriptor = InstrumentDescriptor {
    id: "terrain-volume",
    title: "3D Terrain Volume",
    space: VOLUME_SPACE,
    required_layers: REQUIRED_VOLUME_BANDS,
    required_groups: GroupSet::of(&[
        GroupId::Air,
        GroupId::Kinematics,
        GroupId::Altitude,
        GroupId::Attitude,
        GroupId::Selections,
        GroupId::Trust,
    ]),
    underlay: UnderlayContract::VolumetricTerrain3D,
    config_schema: &[],
    accepted_intents: &[
        IntentClass::SELECT_ALTITUDE,
        IntentClass::DECLUTTER,
        SELECT_WAYPOINT_3D,
    ],
    aux_channels: &[AuxChannelId::TERRAIN_PATCH_3D],
    group_regions: &[],
    extreme_states: &[],
    raster_baselines: &[],
    draw: draw_terrain_entry,
};

fn draw_terrain_entry(
    ctx: &DrawContext<'_>,
    target: SceneTarget<'_, '_>,
) -> Result<(), PanelDrawError> {
    let SceneTarget::Volume3D { extent_m, writer } = target else {
        return Err(PanelDrawError::WrongTarget);
    };
    ctx.config().require_schema(&[])?;
    draw_terrain_volume(ctx, extent_m, writer)
}

/// Renders the `terrain-volume` 3D scene for `ctx` inside `extent_m`.
pub fn draw_terrain_volume(
    ctx: &DrawContext<'_>,
    extent_m: Extent3D,
    writer: &mut VolumeSceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let view = ctx.view();
    let data = ctx.data();
    let patch = match ctx.aux(AuxChannelId::TERRAIN_PATCH_3D) {
        Some(raw) => Some(TerrainPatchAux::decode(raw)?),
        None => None,
    };
    emit_reference_grid(extent_m, view.decluttered, writer)?;
    emit_terrain_surface(extent_m, patch.as_ref(), writer)?;

    if usable_sig(data.gs_kt) && usable_sig(data.vsi_fpm) {
        writer.begin_band(VolumeBandId::Trajectories3D)?;
        writer.stroke_3d(palette::MAGENTA, 0.0025)?;
        let climb = (data.vsi_fpm.value / 2000.0).clamp(-1.0, 1.0) * extent_m.height_m * 0.25;
        let p0 = view_point([0.0, 0.02, extent_m.depth_m * 0.20], view, extent_m);
        let p1 = view_point([0.0, 0.02, 0.0], view, extent_m);
        let p2 = view_point(
            [0.0, 0.02 + climb, -extent_m.depth_m * 0.30],
            view,
            extent_m,
        );
        writer.polyline_3d(&[p0, p1, p2])?;
        writer.end_band(VolumeBandId::Trajectories3D)?;
    }

    writer.begin_band(VolumeBandId::Symbols3D)?;
    let ownship = view_point([0.0, 0.02, 0.0], view, extent_m);
    let radius = (extent_m.width_m * 0.035).clamp(0.008, 0.035);
    writer.fill_color(if usable_sig(data.roll_rad) && usable_sig(data.pitch_rad) {
        safety::REFERENCE_YELLOW
    } else {
        safety::FAILURE_RED
    })?;
    writer.sphere(PaintMode::Fill, ownship, radius)?;
    writer.end_band(VolumeBandId::Symbols3D)?;

    emit_terrain_billboards(ctx, extent_m, writer)?;
    emit_terrain_handles(ctx, extent_m, writer)?;
    emit_annunciation_band(ctx, extent_m, "TERRAIN VOL", writer)
}

fn emit_terrain_surface(
    extent_m: Extent3D,
    patch: Option<&TerrainPatchAux<'_>>,
    writer: &mut VolumeSceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let [hx, hy, hz] = extent_m.half_extents();
    let mpu = patch.map_or(10_000.0, |p| p.meters_per_unit);
    writer.begin_band(VolumeBandId::SurfaceUnderlay)?;
    writer.underlay_anchor_3d(
        VolumeUnderlayKind::TerrainPatch,
        [0.0, -hy * 0.45, 0.0],
        [
            extent_m.width_m * 0.9,
            extent_m.height_m * 0.4,
            extent_m.depth_m * 0.9,
        ],
        Quat::IDENTITY,
        mpu,
    )?;
    writer.stroke_3d(palette::GROUND, 0.0018)?;
    let cols = patch.map_or(4, |p| p.cols);
    let rows = patch.map_or(4, |p| p.rows);
    let count = usize::from(cols).wrapping_mul(usize::from(rows));
    let mut scaled = [0.0_f32; 256];
    let mut idx = 0usize;
    while idx < count {
        let base = patch
            .and_then(|p| p.sample(idx))
            .or_else(|| DEFAULT_HEIGHTS.get(idx).copied())
            .unwrap_or(-0.08);
        if let Some(slot) = scaled.get_mut(idx) {
            *slot = (base * extent_m.height_m).clamp(-hy * 0.85, hy * 0.25);
        }
        idx = idx.wrapping_add(1);
    }
    let step_x = (hx * 1.5) / f32::from(cols.saturating_sub(1).max(1));
    let step_z = (hz * 1.5) / f32::from(rows.saturating_sub(1).max(1));
    if let Some(samples) = scaled.get(..count) {
        writer.heightfield_grid(
            cols,
            rows,
            [-hx * 0.75, 0.0, -hz * 0.75],
            step_x,
            step_z,
            samples,
        )?;
    }
    writer.end_band(VolumeBandId::SurfaceUnderlay)?;
    Ok(())
}

fn emit_terrain_billboards(
    ctx: &DrawContext<'_>,
    extent_m: Extent3D,
    writer: &mut VolumeSceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let data = ctx.data();
    let [hx, hy, _hz] = extent_m.half_extents();
    let sz = (extent_m.height_m * 0.05).clamp(0.012, 0.048);
    writer.begin_band(VolumeBandId::Billboards)?;
    let alt = data.altitude.value_ft;
    writer.fill_color(status_ink(alt.status))?;
    if usable_sig(alt) {
        let lbl = fmt_label!(24, "ALT {:.0} FT", alt.value);
        writer.attributed_billboard_text(
            altitude_group(data).to_u8(),
            [hx * 0.65, hy * 0.55, 0.0],
            sz,
            HAlign::Right,
            lbl.as_str(),
        )?;
    } else {
        writer.billboard_text([hx * 0.65, hy * 0.55, 0.0], sz, HAlign::Right, "ALT ----")?;
    }
    let gs = data.gs_kt;
    writer.fill_color(status_ink(gs.status))?;
    if usable_sig(gs) {
        let lbl = fmt_label!(24, "GS {:.0} kt", gs.value);
        writer.attributed_billboard_text(
            GroupId::Kinematics.to_u8(),
            [-hx * 0.65, hy * 0.55, 0.0],
            sz,
            HAlign::Left,
            lbl.as_str(),
        )?;
    } else {
        writer.billboard_text([-hx * 0.65, hy * 0.55, 0.0], sz, HAlign::Left, "GS ----")?;
    }
    if alt.status == SignalStatus::Stale || gs.status == SignalStatus::Stale {
        writer.fill_color(safety::CAUTION_AMBER)?;
        writer.billboard_text([0.0, hy * 0.68, 0.0], sz * 0.85, HAlign::Center, "STALE")?;
    }
    writer.end_band(VolumeBandId::Billboards)?;
    Ok(())
}

fn emit_terrain_handles(
    ctx: &DrawContext<'_>,
    extent_m: Extent3D,
    writer: &mut VolumeSceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let view = ctx.view();
    let data = ctx.data();
    let [hx, hy, hz] = extent_m.half_extents();
    let bug_y = data
        .selections
        .altitude_sel_m
        .filter(|v| v.is_finite())
        .map_or(hy * 0.25, |m| ((m / 4000.0) - 0.25) * hy);
    let alt_bug = clamp_inside_extent(
        view_point([hx * 0.45, bug_y, 0.0], view, extent_m),
        extent_m,
        0.02,
    );
    let wp = clamp_inside_extent(
        view_point([0.0, 0.0, -hz * 0.40], view, extent_m),
        extent_m,
        0.02,
    );
    let hr = (extent_m.width_m * 0.04).clamp(0.01, 0.035);
    writer.begin_band(VolumeBandId::Handles)?;
    writer.target_handle(1, HandleKind::AltitudeBug, alt_bug, hr)?;
    writer.target_handle(2, HandleKind::Waypoint, wp, hr)?;
    writer.end_band(VolumeBandId::Handles)?;
    Ok(())
}
