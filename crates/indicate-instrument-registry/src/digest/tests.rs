#![allow(clippy::expect_used, clippy::panic)]

use indicate_instrument_scene::{
    MAX_SCENE_BYTES,
    volume::{Extent3D, MAX_VOLUME_SCENE_BYTES, VolumeBandId},
};
use indicate_interaction::ViewLimits;

use indicate_instrument_descriptor::{
    DesignFrame, DrawContext, GroupSet, PanelDescriptor, PanelDrawError, RenderSpace, SceneTarget,
    UnderlayContract, volume_band_bit,
};

use super::{DigestError, scene_digest};
use crate::registry::Registry;

fn draw_nothing(
    _ctx: &DrawContext<'_>,
    _target: SceneTarget<'_, '_>,
) -> Result<(), PanelDrawError> {
    Ok(())
}

const FRAME: DesignFrame = DesignFrame {
    width: 480.0,
    height: 360.0,
};

const fn panel(id: &'static str) -> PanelDescriptor {
    PanelDescriptor {
        id,
        title: "Panel",
        space: RenderSpace::Planar2D {
            frame_min: FRAME,
            frame_max: FRAME,
            frame_step: (1.0, 1.0),
            aspect_min: 1.30,
            aspect_max: 1.37,
            canonical_frames: &[FRAME],
        },
        required_layers: 0b10,
        required_groups: GroupSet::EMPTY,
        underlay: UnderlayContract::None,
        config_schema: &[],
        accepted_intents: &[],
        aux_channels: &[],
        group_regions: &[],
        extreme_states: &[],
        raster_baselines: &[],
        draw: draw_nothing,
    }
}

#[test]
fn the_digest_binds_panel_identity_and_composition() {
    static ONE: [PanelDescriptor; 1] = [panel("alpha")];
    static RENAMED: [PanelDescriptor; 1] = [panel("beta")];
    static TWO: [PanelDescriptor; 2] = [panel("alpha"), panel("beta")];
    let mut scratch = std::vec![0u8; MAX_SCENE_BYTES];
    let digest = |panels: &'static [PanelDescriptor], scratch: &mut [u8]| {
        scene_digest(&Registry::new(panels).expect("composes"), scratch).expect("digests")
    };
    let one = digest(&ONE, &mut scratch);
    assert_ne!(one, digest(&RENAMED, &mut scratch), "panel id is bound");
    assert_ne!(one, digest(&TWO, &mut scratch), "composition is bound");
}

#[test]
fn the_digest_binds_the_descriptor_contract() {
    static BASE: [PanelDescriptor; 1] = [panel("alpha")];
    static WIDER_MASK: [PanelDescriptor; 1] = [{
        let mut p = panel("alpha");
        p.required_layers = 0b110;
        p
    }];
    static WIDER_ASPECT: [PanelDescriptor; 1] = [{
        let mut p = panel("alpha");
        p.space = RenderSpace::Planar2D {
            frame_min: FRAME,
            frame_max: FRAME,
            frame_step: (1.0, 1.0),
            aspect_min: 1.30,
            aspect_max: 1.5,
            canonical_frames: &[FRAME],
        };
        p
    }];
    let mut scratch = std::vec![0u8; MAX_SCENE_BYTES];
    let digest = |panels: &'static [PanelDescriptor], scratch: &mut [u8]| {
        scene_digest(&Registry::new(panels).expect("composes"), scratch).expect("digests")
    };
    let base = digest(&BASE, &mut scratch);
    assert_ne!(base, digest(&WIDER_MASK, &mut scratch));
    assert_ne!(base, digest(&WIDER_ASPECT, &mut scratch));
}

const BIG: DesignFrame = DesignFrame {
    width: 960.0,
    height: 720.0,
};

const fn two_frame_panel() -> PanelDescriptor {
    let mut p = panel("alpha");
    p.space = RenderSpace::Planar2D {
        frame_min: FRAME,
        frame_max: BIG,
        frame_step: (480.0, 360.0),
        aspect_min: 1.30,
        aspect_max: 1.37,
        canonical_frames: &[FRAME, BIG],
    };
    p
}

fn draw_frame_rect(
    _ctx: &DrawContext<'_>,
    target: SceneTarget<'_, '_>,
) -> Result<(), PanelDrawError> {
    let SceneTarget::Planar2D { frame, writer } = target else {
        return Err(PanelDrawError::WrongTarget);
    };
    writer.begin_layer(indicate_instrument_scene::LayerId::Attitude)?;
    writer.rect(
        indicate_instrument_scene::PaintMode::Fill,
        0.0,
        0.0,
        frame.width,
        frame.height,
    )?;
    writer.end_layer(indicate_instrument_scene::LayerId::Attitude)?;
    Ok(())
}

fn draw_fixed_rect(
    _ctx: &DrawContext<'_>,
    target: SceneTarget<'_, '_>,
) -> Result<(), PanelDrawError> {
    let SceneTarget::Planar2D { writer, .. } = target else {
        return Err(PanelDrawError::WrongTarget);
    };
    writer.begin_layer(indicate_instrument_scene::LayerId::Attitude)?;
    writer.rect(
        indicate_instrument_scene::PaintMode::Fill,
        0.0,
        0.0,
        FRAME.width,
        FRAME.height,
    )?;
    writer.end_layer(indicate_instrument_scene::LayerId::Attitude)?;
    Ok(())
}

#[test]
fn the_digest_draws_every_canonical_frame_and_the_panel_sees_it() {
    static ONE_SIZED: [PanelDescriptor; 1] = [{
        let mut p = panel("alpha");
        p.draw = draw_frame_rect;
        p
    }];
    static ONE_FIXED: [PanelDescriptor; 1] = [{
        let mut p = panel("alpha");
        p.draw = draw_fixed_rect;
        p
    }];
    static TWO_SIZED: [PanelDescriptor; 1] = [{
        let mut p = two_frame_panel();
        p.draw = draw_frame_rect;
        p
    }];
    static TWO_FIXED: [PanelDescriptor; 1] = [{
        let mut p = two_frame_panel();
        p.draw = draw_fixed_rect;
        p
    }];
    let mut scratch = std::vec![0u8; MAX_SCENE_BYTES];
    let digest = |panels: &'static [PanelDescriptor], scratch: &mut [u8]| {
        scene_digest(&Registry::new(panels).expect("composes"), scratch).expect("digests")
    };
    assert_eq!(
        digest(&ONE_SIZED, &mut scratch),
        digest(&ONE_FIXED, &mut scratch)
    );
    assert_ne!(
        digest(&TWO_SIZED, &mut scratch),
        digest(&TWO_FIXED, &mut scratch)
    );
}

fn draw_one_layer(
    _ctx: &DrawContext<'_>,
    target: SceneTarget<'_, '_>,
) -> Result<(), PanelDrawError> {
    let SceneTarget::Planar2D { writer, .. } = target else {
        return Err(PanelDrawError::WrongTarget);
    };
    writer.begin_layer(indicate_instrument_scene::LayerId::Attitude)?;
    writer.end_layer(indicate_instrument_scene::LayerId::Attitude)?;
    Ok(())
}

#[test]
fn an_undersized_scratch_fails_typed_not_truncated() {
    static ONE: [PanelDescriptor; 1] = [{
        let mut p = panel("alpha");
        p.draw = draw_one_layer;
        p
    }];
    let registry = Registry::new(&ONE).expect("composes");
    let mut none = [0u8; 0];
    assert!(matches!(
        scene_digest(&registry, &mut none),
        Err(DigestError::Scratch { len: 0 })
    ));
    let mut tiny = [0u8; 2];
    assert!(matches!(
        scene_digest(&registry, &mut tiny),
        Err(DigestError::Draw { panel: "alpha", .. })
    ));
}

fn draw_volume_box(
    _ctx: &DrawContext<'_>,
    target: SceneTarget<'_, '_>,
) -> Result<(), PanelDrawError> {
    let SceneTarget::Volume3D { extent_m, writer } = target else {
        return Err(PanelDrawError::WrongTarget);
    };
    writer.begin_band(VolumeBandId::ReferenceGrid)?;
    writer.save()?;
    writer.line_3d([0.0, 0.0, 0.0], [extent_m.width_m, 0.0, 0.0])?;
    writer.restore()?;
    writer.end_band(VolumeBandId::ReferenceGrid)?;
    Ok(())
}

#[test]
fn digest_streams_volume_3d_instruments_deterministically() {
    static VOL: [PanelDescriptor; 1] = [PanelDescriptor {
        id: "vol-test",
        title: "Volume Test",
        space: RenderSpace::Volume3D {
            extent_min_m: Extent3D::new(0.3, 0.3, 0.3),
            extent_max_m: Extent3D::new(1.5, 1.5, 1.5),
            canonical_extents_m: &[Extent3D::new(0.6, 0.45, 0.6)],
            view_limits: ViewLimits::VOLUMETRIC_DEFAULT,
        },
        required_layers: volume_band_bit(VolumeBandId::ReferenceGrid),
        required_groups: GroupSet::EMPTY,
        underlay: UnderlayContract::None,
        config_schema: &[],
        accepted_intents: &[],
        aux_channels: &[],
        group_regions: &[],
        extreme_states: &[],
        raster_baselines: &[],
        draw: draw_volume_box,
    }];
    let registry = Registry::new(&VOL).expect("volume registry");
    let mut scratch = std::vec![0u8; MAX_VOLUME_SCENE_BYTES];
    let d1 = scene_digest(&registry, &mut scratch).expect("digest 1");
    let d2 = scene_digest(&registry, &mut scratch).expect("digest 2");
    assert_eq!(d1, d2);
}
