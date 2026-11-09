//! Printed-legend fixtures: a configuration-claimed run is admitted
//! only when it is the same in every case of the matrix.
#![allow(clippy::expect_used, clippy::panic)]

use indicate_instrument_registry::{
    DrawContext, GroupSet, PanelDrawError, Registry, SceneTarget, UnderlayContract,
};
use indicate_instrument_scene::{ATTR_CONFIG, Anchor, LayerId, Rgba8, SceneWriter};
use indicate_instrument_state::GroupId;

use super::super::{AdmissionError, admit};
use super::opaque_panel;

fn legend_panel(draw: indicate_instrument_registry::DrawFn) -> Registry {
    let panels = std::boxed::Box::leak(std::boxed::Box::new(opaque_panel(draw)));
    panels[0].underlay = UnderlayContract::None;
    panels[0].required_groups = GroupSet::of(&[GroupId::Air]);
    Registry::new(panels).expect("structurally valid")
}

fn planar<'s, 'b>(target: SceneTarget<'s, 'b>) -> Result<&'s mut SceneWriter<'b>, PanelDrawError> {
    match target {
        SceneTarget::Planar2D { writer, .. } => Ok(writer),
        _ => Err(PanelDrawError::WrongTarget),
    }
}

/// A graduation printed on a dial face: the same digits at the same
/// place whatever the state.
fn draw_printed_legend(
    _ctx: &DrawContext<'_>,
    target: SceneTarget<'_, '_>,
) -> Result<(), PanelDrawError> {
    let scene = planar(target)?;
    scene.begin_layer(LayerId::Tapes)?;
    scene.fill_color(Rgba8::rgb(255, 255, 255))?;
    scene.save()?;
    scene.translate(60.0, 60.0)?;
    scene.rotate(0.5)?;
    scene.text_attributed(ATTR_CONFIG, 0.0, -40.0, 12.0, Anchor::CENTER, "120")?;
    scene.restore()?;
    scene.text_attributed(ATTR_CONFIG, 60.0, 90.0, 10.0, Anchor::CENTER, "KNOTS")?;
    scene.end_layer(LayerId::Tapes)?;
    Ok(())
}

#[test]
fn a_printed_legend_is_admitted_in_every_case() {
    let report = admit(&legend_panel(draw_printed_legend)).expect("a fixed legend is admissible");
    assert!(report.cases > 0);
}

/// Digits that follow the airspeed, claimed as configuration.
fn draw_state_numeral(
    ctx: &DrawContext<'_>,
    target: SceneTarget<'_, '_>,
) -> Result<(), PanelDrawError> {
    let scene = planar(target)?;
    let text = if ctx.data().ias_kt.status.shows_value() {
        "103"
    } else {
        "100"
    };
    scene.begin_layer(LayerId::Tapes)?;
    scene.fill_color(Rgba8::rgb(255, 255, 255))?;
    scene.text_attributed(ATTR_CONFIG, 30.0, 30.0, 14.0, Anchor::CENTER, text)?;
    scene.end_layer(LayerId::Tapes)?;
    Ok(())
}

#[test]
fn a_state_dependent_numeral_claimed_as_configuration_is_refused() {
    assert!(matches!(
        admit(&legend_panel(draw_state_numeral)),
        Err(AdmissionError::ConfigClaim { .. })
    ));
}

/// Fixed digits that ride on the needle: the text never changes, the
/// placement carries the value.
fn draw_moving_legend(
    ctx: &DrawContext<'_>,
    target: SceneTarget<'_, '_>,
) -> Result<(), PanelDrawError> {
    let scene = planar(target)?;
    let ias = ctx.data().ias_kt;
    let angle = if ias.status.shows_value() {
        ias.value * 0.01
    } else {
        0.0
    };
    scene.begin_layer(LayerId::Tapes)?;
    scene.fill_color(Rgba8::rgb(255, 255, 255))?;
    scene.save()?;
    scene.translate(60.0, 60.0)?;
    scene.rotate(angle)?;
    scene.text_attributed(ATTR_CONFIG, 0.0, -40.0, 12.0, Anchor::CENTER, "100")?;
    scene.restore()?;
    scene.end_layer(LayerId::Tapes)?;
    Ok(())
}

#[test]
fn a_legend_that_moves_with_state_is_refused() {
    assert!(matches!(
        admit(&legend_panel(draw_moving_legend)),
        Err(AdmissionError::ConfigClaim { .. })
    ));
}

/// A legend drawn only while airspeed is shown: its absence is itself
/// a reading.
fn draw_vanishing_legend(
    ctx: &DrawContext<'_>,
    target: SceneTarget<'_, '_>,
) -> Result<(), PanelDrawError> {
    let scene = planar(target)?;
    scene.begin_layer(LayerId::Tapes)?;
    if ctx.data().ias_kt.status.shows_value() {
        scene.fill_color(Rgba8::rgb(255, 255, 255))?;
        scene.text_attributed(ATTR_CONFIG, 30.0, 30.0, 14.0, Anchor::CENTER, "100")?;
    }
    scene.end_layer(LayerId::Tapes)?;
    Ok(())
}

#[test]
fn a_legend_that_appears_only_with_data_is_refused() {
    assert!(matches!(
        admit(&legend_panel(draw_vanishing_legend)),
        Err(AdmissionError::ConfigClaim { .. })
    ));
}
