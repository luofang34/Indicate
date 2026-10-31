//! Background-contract fixtures: both defect directions and the
//! coverage evasions the graphics-state scanner refuses.
#![allow(clippy::expect_used, clippy::panic)]

use indicate_instrument_registry::{
    DrawContext, GroupSet, PanelDescriptor, PanelDrawError, Registry, SceneTarget, UnderlayContract,
};
use indicate_instrument_scene::{LayerId, PaintMode, Rgba8};

use super::super::{AdmissionError, admit};
use super::{FIXTURE_SPACE, opaque_panel};

const fn notused_panel(
    id: &'static str,
    required_layers: u16,
    draw: indicate_instrument_registry::DrawFn,
) -> PanelDescriptor {
    PanelDescriptor {
        id,
        title: "Probe",
        space: FIXTURE_SPACE,
        required_layers,
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

fn draw_notused_painter(
    _ctx: &DrawContext<'_>,
    target: SceneTarget<'_, '_>,
) -> Result<(), PanelDrawError> {
    let SceneTarget::Planar2D { writer: scene, .. } = target else {
        return Err(PanelDrawError::WrongTarget);
    };
    scene.begin_layer(LayerId::Background)?;
    scene.fill_color(Rgba8::rgb(0, 0, 0))?;
    scene.rect(PaintMode::Fill, 0.0, 0.0, 480.0, 360.0)?;
    scene.end_layer(LayerId::Background)?;
    scene.begin_layer(LayerId::Tapes)?;
    scene.end_layer(LayerId::Tapes)?;
    Ok(())
}

#[test]
fn a_notused_panel_that_paints_the_band_is_refused() {
    static DEFECT: [PanelDescriptor; 1] =
        [notused_panel("shy-painter", 1 << 2, draw_notused_painter)];
    let registry = Registry::new(&DEFECT).expect("structurally valid");
    assert!(matches!(
        admit(&registry),
        Err(AdmissionError::BackgroundContract {
            panel: "shy-painter",
            declared: "None",
            ..
        })
    ));
}

fn draw_optimist(
    _ctx: &DrawContext<'_>,
    target: SceneTarget<'_, '_>,
) -> Result<(), PanelDrawError> {
    let SceneTarget::Planar2D { writer: scene, .. } = target else {
        return Err(PanelDrawError::WrongTarget);
    };
    scene.begin_layer(LayerId::Tapes)?;
    scene.end_layer(LayerId::Tapes)?;
    Ok(())
}

#[test]
fn an_opaque_panel_that_covers_nothing_is_refused() {
    let panels = std::boxed::Box::leak(std::boxed::Box::new(opaque_panel(draw_optimist)));
    panels[0].id = "optimist";
    let registry = Registry::new(panels).expect("structurally valid");
    assert!(matches!(
        admit(&registry),
        Err(AdmissionError::BackgroundContract {
            panel: "optimist",
            declared: "Opaque",
            ..
        })
    ));
}

fn draw_clip_evasion(
    _ctx: &DrawContext<'_>,
    target: SceneTarget<'_, '_>,
) -> Result<(), PanelDrawError> {
    let SceneTarget::Planar2D { writer: scene, .. } = target else {
        return Err(PanelDrawError::WrongTarget);
    };
    scene.begin_layer(LayerId::Background)?;
    scene.save()?;
    scene.clip_rect(0.0, 0.0, 4.0, 4.0)?;
    scene.fill_color(Rgba8::rgb(10, 20, 30))?;
    scene.rect(PaintMode::Fill, 0.0, 0.0, 480.0, 360.0)?;
    scene.restore()?;
    scene.end_layer(LayerId::Background)?;
    scene.begin_layer(LayerId::Tapes)?;
    scene.end_layer(LayerId::Tapes)?;
    Ok(())
}

fn draw_rotate_evasion(
    _ctx: &DrawContext<'_>,
    target: SceneTarget<'_, '_>,
) -> Result<(), PanelDrawError> {
    let SceneTarget::Planar2D { writer: scene, .. } = target else {
        return Err(PanelDrawError::WrongTarget);
    };
    scene.begin_layer(LayerId::Background)?;
    scene.save()?;
    scene.translate(240.0, 180.0)?;
    scene.rotate(core::f32::consts::FRAC_PI_4)?;
    scene.fill_color(Rgba8::rgb(10, 20, 30))?;
    scene.rect(PaintMode::Fill, -280.0, -280.0, 560.0, 560.0)?;
    scene.restore()?;
    scene.end_layer(LayerId::Background)?;
    scene.begin_layer(LayerId::Tapes)?;
    scene.end_layer(LayerId::Tapes)?;
    Ok(())
}

fn draw_alpha_evasion(
    _ctx: &DrawContext<'_>,
    target: SceneTarget<'_, '_>,
) -> Result<(), PanelDrawError> {
    let SceneTarget::Planar2D { writer: scene, .. } = target else {
        return Err(PanelDrawError::WrongTarget);
    };
    scene.begin_layer(LayerId::Background)?;
    scene.fill_color(Rgba8::rgba(10, 20, 30, 8))?;
    scene.save()?;
    scene.fill_color(Rgba8::rgb(10, 20, 30))?;
    scene.restore()?;
    scene.rect(PaintMode::Fill, 0.0, 0.0, 480.0, 360.0)?;
    scene.end_layer(LayerId::Background)?;
    scene.begin_layer(LayerId::Tapes)?;
    scene.end_layer(LayerId::Tapes)?;
    Ok(())
}

fn draw_empty_band_notused(
    _ctx: &DrawContext<'_>,
    target: SceneTarget<'_, '_>,
) -> Result<(), PanelDrawError> {
    let SceneTarget::Planar2D { writer: scene, .. } = target else {
        return Err(PanelDrawError::WrongTarget);
    };
    scene.begin_layer(LayerId::Background)?;
    scene.end_layer(LayerId::Background)?;
    scene.begin_layer(LayerId::Tapes)?;
    scene.end_layer(LayerId::Tapes)?;
    Ok(())
}

#[test]
fn coverage_evasions_are_refused() {
    for (name, draw) in [
        (
            "clip",
            draw_clip_evasion as indicate_instrument_registry::DrawFn,
        ),
        ("rotate", draw_rotate_evasion),
        ("alpha", draw_alpha_evasion),
    ] {
        let panels = std::boxed::Box::leak(std::boxed::Box::new(opaque_panel(draw)));
        let registry = Registry::new(panels).expect("structurally valid");
        assert!(
            matches!(
                admit(&registry),
                Err(AdmissionError::BackgroundContract {
                    declared: "Opaque",
                    ..
                })
            ),
            "{name} evasion must be refused"
        );
    }
}

#[test]
fn an_empty_band_under_notused_is_tolerated() {
    static SHY: [PanelDescriptor; 1] = [notused_panel("shy", 1 << 2, draw_empty_band_notused)];
    let registry = Registry::new(&SHY).expect("structurally valid");
    admit(&registry).expect("an opened-empty band paints nothing");
}

fn draw_empty(_ctx: &DrawContext<'_>, _target: SceneTarget<'_, '_>) -> Result<(), PanelDrawError> {
    Ok(())
}

#[test]
fn a_panel_missing_its_required_band_is_refused() {
    static HOLLOW: [PanelDescriptor; 1] = [notused_panel("hollow", 1 << 4, draw_empty)];
    let registry = Registry::new(&HOLLOW).expect("structurally valid");
    assert!(matches!(
        admit(&registry),
        Err(AdmissionError::MissingRequiredLayers {
            panel: "hollow",
            ..
        })
    ));
}
