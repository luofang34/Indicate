//! Spatial conformance fixtures for `AngularSphere` and `Volume3D` (VOL-01).
#![allow(clippy::expect_used, clippy::panic)]

use indicate_frames::Quat;
use indicate_instrument_registry::{
    DesignFrame, DrawContext, GroupSet, PanelDescriptor, PanelDrawError, Registry, RenderSpace,
    SceneTarget, UnderlayContract, angular_layer_bit, angular_overlay_layer_bit, volume_band_bit,
};
use indicate_instrument_scene::angular::AngularLayerId;
use indicate_instrument_scene::volume::{Extent3D, HandleKind, VolumeBandId, VolumeUnderlayKind};
use indicate_instrument_scene::{Anchor, HAlign, LayerId, Rgba8};
use indicate_instrument_state::GroupId;

use super::super::{AdmissionError, admit};

const EXT_MIN: Extent3D = Extent3D::new(0.30, 0.30, 0.30);
const EXT_MAX: Extent3D = Extent3D::new(0.90, 0.90, 0.90);
const OV_FRAME: DesignFrame = DesignFrame {
    width: 480.0,
    height: 360.0,
};

fn draw_valid_angular(
    ctx: &DrawContext<'_>,
    target: SceneTarget<'_, '_>,
) -> Result<(), PanelDrawError> {
    let SceneTarget::AngularSphere { angular, overlay } = target else {
        return Err(PanelDrawError::WrongTarget);
    };
    angular.begin_layer(AngularLayerId::WorldCue)?;
    angular.stroke_mrad(Rgba8::rgb(0, 255, 0), 2.0)?;
    angular.reticle_ring([0.0, 0.0, 1.0], 0.05)?;
    if ctx.data().ias_kt.status.shows_value() {
        angular.attributed_angular_text(
            GroupId::Air.to_u8(),
            [0.0, 0.0, 1.0],
            8.0,
            HAlign::Center,
            "120",
        )?;
    } else {
        angular.angular_text([0.0, 0.0, 1.0], 8.0, HAlign::Center, "---")?;
    }
    angular.end_layer(AngularLayerId::WorldCue)?;
    if let Some((_, ov)) = overlay {
        ov.begin_layer(LayerId::Annunciation)?;
        ov.fill_color(Rgba8::rgb(255, 255, 255))?;
        ov.text(24.0, 24.0, 12.0, Anchor::MIDDLE_LEFT, "SIM")?;
        ov.end_layer(LayerId::Annunciation)?;
    }
    Ok(())
}

#[test]
fn valid_angular_with_overlay_passes_admission() {
    static DESC: [PanelDescriptor; 1] = [PanelDescriptor {
        id: "ang-ok",
        title: "Angular OK",
        space: RenderSpace::AngularSphere {
            overlay_frame: Some(OV_FRAME),
        },
        required_layers: angular_layer_bit(AngularLayerId::WorldCue)
            | angular_overlay_layer_bit(LayerId::Annunciation),
        required_groups: GroupSet::of(&[GroupId::Air]),
        underlay: UnderlayContract::None,
        config_schema: &[],
        accepted_intents: &[],
        aux_channels: &[],
        group_regions: &[],
        extreme_states: &[],
        raster_baselines: &[],
        draw: draw_valid_angular,
    }];
    let registry = Registry::new(&DESC).expect("valid");
    let report = admit(&registry).expect("admitted");
    assert!(report.cases > 0);
}

fn draw_valid_volume(
    ctx: &DrawContext<'_>,
    target: SceneTarget<'_, '_>,
) -> Result<(), PanelDrawError> {
    let SceneTarget::Volume3D { extent_m, writer } = target else {
        return Err(PanelDrawError::WrongTarget);
    };
    writer.begin_band(VolumeBandId::SurfaceUnderlay)?;
    writer.underlay_anchor_3d(
        VolumeUnderlayKind::TerrainPatch,
        [0.0, 0.0, 0.0],
        [extent_m.width_m, extent_m.height_m * 0.5, extent_m.depth_m],
        Quat::IDENTITY,
        1000.0,
    )?;
    writer.end_band(VolumeBandId::SurfaceUnderlay)?;
    writer.begin_band(VolumeBandId::Billboards)?;
    writer.fill_color(Rgba8::rgb(255, 255, 255))?;
    if ctx.data().ias_kt.status.shows_value() {
        writer.attributed_billboard_text(
            GroupId::Air.to_u8(),
            [0.0, 0.05, 0.0],
            0.02,
            HAlign::Center,
            "1500",
        )?;
    } else {
        writer.billboard_text([0.0, 0.05, 0.0], 0.02, HAlign::Center, "----")?;
    }
    writer.end_band(VolumeBandId::Billboards)?;
    writer.begin_band(VolumeBandId::Handles)?;
    writer.target_handle(
        1,
        HandleKind::Waypoint,
        [extent_m.width_m * 0.25, 0.0, 0.0],
        0.02,
    )?;
    writer.end_band(VolumeBandId::Handles)?;
    Ok(())
}

fn draw_oob_handle_volume(
    _ctx: &DrawContext<'_>,
    target: SceneTarget<'_, '_>,
) -> Result<(), PanelDrawError> {
    let SceneTarget::Volume3D { extent_m, writer } = target else {
        return Err(PanelDrawError::WrongTarget);
    };
    writer.begin_band(VolumeBandId::Handles)?;
    writer.target_handle(7, HandleKind::Point, [extent_m.width_m, 0.0, 0.0], 0.02)?;
    writer.end_band(VolumeBandId::Handles)?;
    Ok(())
}

#[test]
fn volume_admission_accepts_honest_volume_and_rejects_oob_handle() {
    static OK_VOL: [PanelDescriptor; 1] = [PanelDescriptor {
        id: "vol-ok",
        title: "Volume OK",
        space: RenderSpace::Volume3D {
            extent_min_m: EXT_MIN,
            extent_max_m: EXT_MAX,
            canonical_extents_m: &[EXT_MIN, EXT_MAX],
            view_limits: indicate_instrument_registry::ViewLimits::VOLUMETRIC_DEFAULT,
        },
        required_layers: volume_band_bit(VolumeBandId::SurfaceUnderlay)
            | volume_band_bit(VolumeBandId::Billboards)
            | volume_band_bit(VolumeBandId::Handles),
        required_groups: GroupSet::of(&[GroupId::Air]),
        underlay: UnderlayContract::VolumetricTerrain3D,
        config_schema: &[],
        accepted_intents: &[],
        aux_channels: &[],
        group_regions: &[],
        extreme_states: &[],
        raster_baselines: &[],
        draw: draw_valid_volume,
    }];
    let reg = Registry::new(&OK_VOL).expect("valid");
    admit(&reg).expect("admitted");

    static BAD_VOL: [PanelDescriptor; 1] = [PanelDescriptor {
        id: "vol-oob",
        title: "Volume OOB",
        space: RenderSpace::Volume3D {
            extent_min_m: EXT_MIN,
            extent_max_m: EXT_MIN,
            canonical_extents_m: &[EXT_MIN],
            view_limits: indicate_instrument_registry::ViewLimits::LOCKED,
        },
        required_layers: volume_band_bit(VolumeBandId::Handles),
        required_groups: GroupSet::EMPTY,
        underlay: UnderlayContract::None,
        config_schema: &[],
        accepted_intents: &[],
        aux_channels: &[],
        group_regions: &[],
        extreme_states: &[],
        raster_baselines: &[],
        draw: draw_oob_handle_volume,
    }];
    let bad_reg = Registry::new(&BAD_VOL).expect("valid");
    assert!(matches!(
        admit(&bad_reg),
        Err(AdmissionError::HandleOutOfBounds {
            panel: "vol-oob",
            target_id: 7,
            ..
        })
    ));
}
