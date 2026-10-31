#![allow(clippy::expect_used, clippy::panic)]

use indicate_instrument_registry::{
    DesignFrame, DrawContext, GroupSet, PanelDescriptor, PanelDrawError, Registry, RenderSpace,
    SceneTarget, UnderlayContract,
};
use indicate_instrument_scene::{LayerId, PaintMode};

use super::check_frame_varies;
use crate::admission::error::AdmissionError;

const MIN: DesignFrame = DesignFrame {
    width: 480.0,
    height: 360.0,
};
const MAX: DesignFrame = DesignFrame {
    width: 600.0,
    height: 450.0,
};

fn draw_varying(_ctx: &DrawContext<'_>, target: SceneTarget<'_, '_>) -> Result<(), PanelDrawError> {
    let SceneTarget::Planar2D {
        frame,
        writer: scene,
    } = target
    else {
        return Err(PanelDrawError::WrongTarget);
    };
    scene.begin_layer(LayerId::Tapes)?;
    scene.rect(PaintMode::Fill, 0.0, 0.0, frame.width, frame.height)?;
    scene.end_layer(LayerId::Tapes)?;
    Ok(())
}

fn draw_varying_only_when_alerted(
    ctx: &DrawContext<'_>,
    target: SceneTarget<'_, '_>,
) -> Result<(), PanelDrawError> {
    let SceneTarget::Planar2D {
        frame,
        writer: scene,
    } = target
    else {
        return Err(PanelDrawError::WrongTarget);
    };
    let width = if ctx.alerts().is_some() {
        frame.width
    } else {
        480.0
    };
    scene.begin_layer(LayerId::Tapes)?;
    scene.rect(PaintMode::Fill, 0.0, 0.0, width, 360.0)?;
    scene.end_layer(LayerId::Tapes)?;
    Ok(())
}

fn draw_fixed(_ctx: &DrawContext<'_>, target: SceneTarget<'_, '_>) -> Result<(), PanelDrawError> {
    let SceneTarget::Planar2D { writer: scene, .. } = target else {
        return Err(PanelDrawError::WrongTarget);
    };
    scene.begin_layer(LayerId::Tapes)?;
    scene.rect(PaintMode::Fill, 0.0, 0.0, 480.0, 360.0)?;
    scene.end_layer(LayerId::Tapes)?;
    Ok(())
}

const fn ranged(id: &'static str, draw: indicate_instrument_registry::DrawFn) -> PanelDescriptor {
    PanelDescriptor {
        id,
        title: "Probe",
        space: RenderSpace::Planar2D {
            frame_min: MIN,
            frame_max: MAX,
            frame_step: (120.0, 90.0),
            aspect_min: 1.30,
            aspect_max: 1.37,
            canonical_frames: &[MIN, MAX],
        },
        required_layers: 0b0000_0100,
        required_groups: GroupSet::EMPTY,
        underlay: UnderlayContract::None,
        config_schema: &[],
        accepted_intents: &[],
        aux_channels: &[],
        group_regions: &[],
        extreme_states: &[],
        raster_baselines: &[],
        draw,
    }
}

static VARYING: PanelDescriptor = ranged("varying", draw_varying);
static FIXED: PanelDescriptor = ranged("fixed", draw_fixed);

#[test]
fn a_panel_that_ignores_its_frame_is_refused() {
    assert_eq!(
        check_frame_varies(&FIXED),
        Err(AdmissionError::FrameIgnored {
            panel: "fixed",
            min: MIN,
            max: MAX,
        })
    );
}

#[test]
fn a_panel_that_uses_its_frame_is_admitted() {
    assert_eq!(check_frame_varies(&VARYING), Ok(()));
}

#[test]
fn a_panel_that_varies_only_under_alerts_is_admitted() {
    static ALERTED: PanelDescriptor = ranged("alerted-only", draw_varying_only_when_alerted);
    assert_eq!(check_frame_varies(&ALERTED), Ok(()));
}

#[test]
fn a_degenerate_range_is_not_asked_to_vary() {
    static FIXED_RANGE: PanelDescriptor = PanelDescriptor {
        space: RenderSpace::Planar2D {
            frame_min: MIN,
            frame_max: MIN,
            frame_step: (120.0, 90.0),
            aspect_min: 1.30,
            aspect_max: 1.37,
            canonical_frames: &[MIN],
        },
        ..ranged("degenerate", draw_fixed)
    };
    assert_eq!(check_frame_varies(&FIXED_RANGE), Ok(()));
}

#[test]
fn the_shipped_panels_are_unaffected() {
    let registry = Registry::new(indicate_instrument_panels::BUILTIN_PANELS).expect("composes");
    for panel in registry.panels() {
        assert_eq!(check_frame_varies(panel), Ok(()), "{}", panel.id);
    }
}
