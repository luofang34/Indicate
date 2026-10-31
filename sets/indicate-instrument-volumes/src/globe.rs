//! `globe-volume`: 3D planetary globe, graticule rings, track arc, and waypoint handles (`VOL-01`).

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

use crate::aux_wire::GlobeNavigationAux;
use crate::shared::{
    REQUIRED_VOLUME_BANDS, SELECT_WAYPOINT_3D, VOLUME_SPACE, clamp_inside_extent,
    emit_annunciation_band, emit_reference_grid, status_ink, usable_sig, view_point,
};

/// Descriptor for the 3D planetary globe (`id: "globe-volume"`, `VOL-01`).
pub const GLOBE_VOLUME_DESCRIPTOR: InstrumentDescriptor = InstrumentDescriptor {
    id: "globe-volume",
    title: "3D Planetary Globe",
    space: VOLUME_SPACE,
    required_layers: REQUIRED_VOLUME_BANDS,
    required_groups: GroupSet::of(&[
        GroupId::Heading,
        GroupId::Kinematics,
        GroupId::Nav,
        GroupId::Trust,
    ]),
    underlay: UnderlayContract::PlanetaryGlobe3D,
    config_schema: &[],
    accepted_intents: &[
        IntentClass::SET_HEADING_BUG,
        IntentClass::SELECT_NAV_SOURCE,
        IntentClass::DECLUTTER,
        SELECT_WAYPOINT_3D,
    ],
    aux_channels: &[AuxChannelId::GLOBE_NAVIGATION_3D],
    group_regions: &[],
    extreme_states: &[],
    raster_baselines: &[],
    draw: draw_globe_entry,
};

fn draw_globe_entry(
    ctx: &DrawContext<'_>,
    target: SceneTarget<'_, '_>,
) -> Result<(), PanelDrawError> {
    let SceneTarget::Volume3D { extent_m, writer } = target else {
        return Err(PanelDrawError::WrongTarget);
    };
    ctx.config().require_schema(&[])?;
    draw_globe_volume(ctx, extent_m, writer)
}

/// Renders the `globe-volume` 3D scene for `ctx` inside `extent_m`.
pub fn draw_globe_volume(
    ctx: &DrawContext<'_>,
    extent_m: Extent3D,
    writer: &mut VolumeSceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let view = ctx.view();
    let data = ctx.data();
    let nav_aux = match ctx.aux(AuxChannelId::GLOBE_NAVIGATION_3D) {
        Some(raw) => Some(GlobeNavigationAux::decode(raw)?),
        None => None,
    };
    let min_dim = extent_m
        .width_m
        .min(extent_m.height_m)
        .min(extent_m.depth_m);
    let globe_r = (min_dim * 0.30).max(0.05);

    emit_reference_grid(extent_m, view.decluttered, writer)?;
    emit_globe_surface(globe_r, nav_aux.as_ref(), view.orientation, writer)?;

    if usable_sig(data.heading.value_rad) {
        writer.begin_band(VolumeBandId::Trajectories3D)?;
        writer.stroke_3d(palette::MAGENTA, 0.0022)?;
        let hdg = data.heading.value_rad.value;
        let arc_r = globe_r * 1.06;
        let mut pts = [[0.0_f32; 3]; 5];
        let mut k = 0usize;
        while k < 5 {
            let lat = -0.35 + (k as f32) * 0.20;
            let p = [
                arc_r * cosf(lat) * sinf(hdg),
                arc_r * sinf(lat),
                -arc_r * cosf(lat) * cosf(hdg),
            ];
            if let Some(slot) = pts.get_mut(k) {
                *slot = view_point(p, view, extent_m);
            }
            k = k.wrapping_add(1);
        }
        writer.polyline_3d(&pts)?;
        writer.end_band(VolumeBandId::Trajectories3D)?;
    }

    writer.begin_band(VolumeBandId::Symbols3D)?;
    let ship_pos = view_point([0.0, globe_r * 0.72, -globe_r * 0.72], view, extent_m);
    writer.fill_color(if usable_sig(data.heading.value_rad) {
        palette::CYAN
    } else {
        safety::FAILURE_RED
    })?;
    writer.sphere(PaintMode::Fill, ship_pos, (globe_r * 0.08).max(0.007))?;
    writer.end_band(VolumeBandId::Symbols3D)?;

    emit_globe_billboards(ctx, extent_m, writer)?;
    emit_globe_handles(ctx, extent_m, globe_r, nav_aux.as_ref(), writer)?;
    emit_annunciation_band(ctx, extent_m, "GLOBE VOL", writer)
}

fn emit_globe_surface(
    globe_r: f32,
    nav_aux: Option<&GlobeNavigationAux>,
    orientation: Quat,
    writer: &mut VolumeSceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let mpu = nav_aux.map_or(20_000_000.0, |a| a.meters_per_unit);
    writer.begin_band(VolumeBandId::SurfaceUnderlay)?;
    let d = globe_r * 2.0;
    writer.underlay_anchor_3d(
        VolumeUnderlayKind::PlanetaryGlobe,
        [0.0, 0.0, 0.0],
        [d, d, d],
        orientation,
        mpu,
    )?;
    writer.stroke_3d(palette::SKY, 0.0018)?;
    writer.ring_3d([0.0, 0.0, 0.0], [0.0, 1.0, 0.0], globe_r)?;
    writer.ring_3d([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], globe_r)?;
    writer.ring_3d([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], globe_r)?;
    writer.end_band(VolumeBandId::SurfaceUnderlay)?;
    Ok(())
}

fn emit_globe_billboards(
    ctx: &DrawContext<'_>,
    extent_m: Extent3D,
    writer: &mut VolumeSceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let data = ctx.data();
    let [hx, hy, _hz] = extent_m.half_extents();
    let sz = (extent_m.height_m * 0.05).clamp(0.012, 0.048);
    writer.begin_band(VolumeBandId::Billboards)?;
    let hdg = data.heading.value_rad;
    writer.fill_color(status_ink(hdg.status))?;
    if usable_sig(hdg) {
        let deg = ((hdg.value.to_degrees() % 360.0) + 360.0) % 360.0;
        let lbl = fmt_label!(24, "HDG {:03.0}°", deg);
        writer.attributed_billboard_text(
            GroupId::Heading.to_u8(),
            [-hx * 0.65, hy * 0.55, 0.0],
            sz,
            HAlign::Left,
            lbl.as_str(),
        )?;
    } else {
        writer.billboard_text([-hx * 0.65, hy * 0.55, 0.0], sz, HAlign::Left, "HDG ---")?;
    }

    let nav_dist =
        data.nav.data.dist_nm.filter(|d| {
            data.nav.status.shows_value() && d.is_finite() && (0.0..100_000.0).contains(d)
        });
    writer.fill_color(status_ink(data.nav.status))?;
    if let Some(nm) = nav_dist {
        let lbl = fmt_label!(24, "DIST {:.1} NM", nm);
        writer.attributed_billboard_text(
            GroupId::Nav.to_u8(),
            [hx * 0.65, hy * 0.55, 0.0],
            sz,
            HAlign::Right,
            lbl.as_str(),
        )?;
    } else {
        writer.billboard_text([hx * 0.65, hy * 0.55, 0.0], sz, HAlign::Right, "DIST ----")?;
    }
    if hdg.status == SignalStatus::Stale || data.nav.status == SignalStatus::Stale {
        writer.fill_color(safety::CAUTION_AMBER)?;
        writer.billboard_text([0.0, hy * 0.68, 0.0], sz * 0.85, HAlign::Center, "STALE")?;
    }
    writer.end_band(VolumeBandId::Billboards)?;
    Ok(())
}

fn emit_globe_handles(
    ctx: &DrawContext<'_>,
    extent_m: Extent3D,
    globe_r: f32,
    nav_aux: Option<&GlobeNavigationAux>,
    writer: &mut VolumeSceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let view = ctx.view();
    let brg = nav_aux.map_or(0.65_f32, |a| a.waypoint_bearing_rad);
    let r = globe_r * 1.08;
    let wp = clamp_inside_extent(
        view_point(
            [r * sinf(brg), globe_r * 0.25, -r * cosf(brg)],
            view,
            extent_m,
        ),
        extent_m,
        0.02,
    );
    let pole = clamp_inside_extent(view_point([0.0, r, 0.0], view, extent_m), extent_m, 0.02);
    let hr = (globe_r * 0.12).clamp(0.01, 0.035);
    writer.begin_band(VolumeBandId::Handles)?;
    writer.target_handle(1, HandleKind::Waypoint, wp, hr)?;
    writer.target_handle(2, HandleKind::Point, pole, hr)?;
    writer.end_band(VolumeBandId::Handles)?;
    Ok(())
}
