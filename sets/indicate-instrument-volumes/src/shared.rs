//! Shared extents, view-transform helpers, and volumetric set constants (`VOL-01`).

use indicate_alerts::{AlertClass, ManagerHealth};
use indicate_instrument_descriptor::{
    DrawContext, InstrumentDescriptor, InstrumentSet, PanelDrawError, RenderSpace, ViewLimits,
    ViewPose3D, volume_band_bit,
};
use indicate_instrument_scene::{
    HAlign, Rgba8,
    volume::{Extent3D, VolumeBandId, VolumeSceneWriter},
};
use indicate_instrument_state::{AltitudeClass, GroupId, PanelData, Sig, SignalStatus};
use indicate_instrument_symbology::{palette, safety};
use indicate_intent::{EchoStatus, IntentClass, IntentNamespace, namespace_id};

use crate::{GLOBE_VOLUME_DESCRIPTOR, ORRERY_VOLUME_DESCRIPTOR, TERRAIN_VOLUME_DESCRIPTOR};

/// Smallest supported tabletop metric volume (`0.30 m × 0.30 m × 0.30 m`).
pub const MIN_EXTENT: Extent3D = Extent3D::new(0.30, 0.30, 0.30);

/// Default tabletop metric volume (`0.60 m × 0.45 m × 0.60 m`).
pub const CANONICAL_EXTENT: Extent3D = Extent3D::new(0.60, 0.45, 0.60);

/// Largest supported holo-table metric volume (`1.50 m × 1.50 m × 1.50 m`).
pub const MAX_EXTENT: Extent3D = Extent3D::new(1.50, 1.50, 1.50);

/// Pinned canonical extents swept by admission and scene digests.
pub const CANONICAL_EXTENTS: &[Extent3D] = &[MIN_EXTENT, CANONICAL_EXTENT, MAX_EXTENT];

/// Deterministic extension namespace for 3D spatial intents (`"com.pilotage.volumes"`).
pub const VOLUMES_NAMESPACE: IntentNamespace = namespace_id("com.pilotage.volumes");

/// Custom intent class selecting or dragging a 3D waypoint handle (`opcode = 1`).
pub const SELECT_WAYPOINT_3D: IntentClass = IntentClass::custom(VOLUMES_NAMESPACE, 1);

/// Custom intent class adjusting an orbital manoeuvre-node handle (`opcode = 2`).
pub const ADJUST_MANOEUVRE_NODE: IntentClass = IntentClass::custom(VOLUMES_NAMESPACE, 2);

/// Shared `RenderSpace::Volume3D` declaration for the reference volumetric instruments.
pub(crate) const VOLUME_SPACE: RenderSpace = RenderSpace::Volume3D {
    extent_min_m: MIN_EXTENT,
    extent_max_m: MAX_EXTENT,
    canonical_extents_m: CANONICAL_EXTENTS,
    view_limits: ViewLimits::VOLUMETRIC_DEFAULT,
};

/// Required criticality bands present in every volumetric frame regardless of degradation.
pub(crate) const REQUIRED_VOLUME_BANDS: u16 = volume_band_bit(VolumeBandId::ReferenceGrid)
    | volume_band_bit(VolumeBandId::SurfaceUnderlay)
    | volume_band_bit(VolumeBandId::Symbols3D)
    | volume_band_bit(VolumeBandId::Billboards)
    | volume_band_bit(VolumeBandId::Handles)
    | volume_band_bit(VolumeBandId::Annunciation);

/// Descriptors shipped in [`VOLUMES_SET`], in canonical composition order.
pub const VOLUMES_PANELS: &[InstrumentDescriptor] = &[
    TERRAIN_VOLUME_DESCRIPTOR,
    GLOBE_VOLUME_DESCRIPTOR,
    ORRERY_VOLUME_DESCRIPTOR,
];

/// Reference 3D volumetric instrument set (`id: "volumes"`, `VOL-01`).
pub const VOLUMES_SET: InstrumentSet = InstrumentSet {
    id: "volumes",
    panels: VOLUMES_PANELS,
};

/// Pinned SHA-256 scene digest over [`VOLUMES_PANELS`] and the canonical corpus (`VOL-01`).
pub const VOLUMES_SCENE_DIGEST: &str =
    "1a0c569bf366f0de8a0086c0c65846dcd86c1e9a89680d4e5800286d4fb23632";

pub(crate) fn usable_sig(sig: Sig<f32>) -> bool {
    sig.status.shows_value() && sig.value.is_finite() && sig.value.abs() < 1_000_000.0
}

pub(crate) fn status_ink(status: SignalStatus) -> Rgba8 {
    match status {
        SignalStatus::Valid => palette::WHITE,
        SignalStatus::Degraded | SignalStatus::Stale => safety::CAUTION_AMBER,
        SignalStatus::Missing | SignalStatus::Failed => safety::FAILURE_RED,
    }
}

pub(crate) fn altitude_group(data: &PanelData) -> GroupId {
    if data.altitude.class == AltitudeClass::LocalRelative {
        GroupId::Kinematics
    } else {
        GroupId::Air
    }
}

/// Applies `view` (`focus_local`, `orientation`, `zoom_scale`) and clamps inside `extent`.
pub(crate) fn view_point(local: [f32; 3], view: &ViewPose3D, extent: Extent3D) -> [f32; 3] {
    let zoom = view.zoom_scale.clamp(0.25, 4.0);
    let shifted = [
        (local[0] - view.focus_local[0] * 0.15) * zoom,
        (local[1] - view.focus_local[1] * 0.15) * zoom,
        (local[2] - view.focus_local[2] * 0.15) * zoom,
    ];
    let rot = view.orientation.rotate([
        f64::from(shifted[0]),
        f64::from(shifted[1]),
        f64::from(shifted[2]),
    ]);
    clamp_inside_extent([rot[0] as f32, rot[1] as f32, rot[2] as f32], extent, 0.015)
}

/// Clamps `pos` inside `extent` with `margin_m` clearance on all six faces.
pub(crate) fn clamp_inside_extent(pos: [f32; 3], extent: Extent3D, margin_m: f32) -> [f32; 3] {
    let half = extent.half_extents();
    let lim = |h: f32| (h - margin_m.min(h * 0.25)).max(0.005);
    let lx = lim(half[0]);
    let ly = lim(half[1]);
    let lz = lim(half[2]);
    let clean = |v: f32, b: f32| if v.is_finite() { v.clamp(-b, b) } else { 0.0 };
    [clean(pos[0], lx), clean(pos[1], ly), clean(pos[2], lz)]
}

/// Emits the metric `ReferenceGrid` band scaled to `extent`.
pub(crate) fn emit_reference_grid(
    extent: Extent3D,
    decluttered: bool,
    writer: &mut VolumeSceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let [hx, hy, hz] = extent.half_extents();
    let floor_y = -hy * 0.85;
    writer.begin_band(VolumeBandId::ReferenceGrid)?;
    writer.stroke_3d(palette::GREY, 0.0015)?;
    writer.line_3d([-hx * 0.9, floor_y, 0.0], [hx * 0.9, floor_y, 0.0])?;
    writer.line_3d([0.0, floor_y, -hz * 0.9], [0.0, floor_y, hz * 0.9])?;
    if !decluttered {
        writer.line_3d([0.0, -hy * 0.85, 0.0], [0.0, hy * 0.85, 0.0])?;
        for frac in [-0.5_f32, 0.5_f32] {
            writer.line_3d(
                [-hx * 0.85, floor_y, hz * frac],
                [hx * 0.85, floor_y, hz * frac],
            )?;
            writer.line_3d(
                [hx * frac, floor_y, -hz * 0.85],
                [hx * frac, floor_y, hz * 0.85],
            )?;
        }
    }
    writer.end_band(VolumeBandId::ReferenceGrid)?;
    Ok(())
}

/// Emits the mandatory `Annunciation` band (`SIM / NOT FOR FLIGHT`, underlay, echo, and alerts).
pub(crate) fn emit_annunciation_band(
    ctx: &DrawContext<'_>,
    extent: Extent3D,
    mode_tag: &str,
    writer: &mut VolumeSceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let [hx, hy, hz] = extent.half_extents();
    let text_h = (extent.height_m * 0.045).clamp(0.012, 0.045);
    writer.begin_band(VolumeBandId::Annunciation)?;
    writer.fill_color(palette::MODE_ACTIVE)?;
    writer.billboard_text(
        [-hx * 0.72, hy * 0.82, -hz * 0.72],
        text_h,
        HAlign::Left,
        mode_tag,
    )?;
    writer.fill_color(safety::CAUTION_AMBER)?;
    writer.billboard_text(
        [0.0, -hy * 0.88, hz * 0.75],
        text_h * 0.9,
        HAlign::Center,
        "SIM / NOT FOR FLIGHT",
    )?;
    let underlay = ctx.underlay();
    if underlay.available && !underlay.calibrated {
        writer.fill_color(safety::CAUTION_AMBER)?;
        writer.billboard_text(
            [hx * 0.70, hy * 0.82, -hz * 0.72],
            text_h * 0.85,
            HAlign::Right,
            "UNDERLAY UNCAL",
        )?;
    }
    let has_overdue_echo = ctx
        .echo()
        .statuses
        .iter()
        .any(|s| matches!(s, EchoStatus::Overdue { .. } | EchoStatus::Unmeasurable));
    if has_overdue_echo {
        writer.fill_color(safety::CAUTION_AMBER)?;
        writer.billboard_text(
            [hx * 0.70, hy * 0.68, -hz * 0.72],
            text_h * 0.85,
            HAlign::Right,
            "CMD NO ECHO",
        )?;
    }
    if let Some(alerts) = ctx.alerts() {
        let urgent = alerts.health() != ManagerHealth::Nominal
            || alerts
                .active()
                .iter()
                .any(|a| !a.inhibited && a.class >= AlertClass::Caution);
        if urgent {
            let is_warn = alerts
                .active()
                .iter()
                .any(|a| !a.inhibited && a.class >= AlertClass::Warning)
                || alerts.health() != ManagerHealth::Nominal;
            writer.fill_color(if is_warn {
                safety::FAILURE_RED
            } else {
                safety::CAUTION_AMBER
            })?;
            writer.billboard_text(
                [0.0, hy * 0.84, 0.0],
                text_h,
                HAlign::Center,
                if is_warn { "ALERT WARN" } else { "ALERT CAUT" },
            )?;
        }
    }
    writer.end_band(VolumeBandId::Annunciation)?;
    Ok(())
}
