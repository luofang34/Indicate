//! Arc runs under every honesty rule a straight run meets.
#![allow(clippy::expect_used, clippy::panic)]

use indicate_instrument_registry::{
    DrawContext, GroupSet, PanelDrawError, Registry, SceneTarget, UnderlayContract,
};
use indicate_instrument_scene::{
    ATTR_CONFIG, ArcAlign, ArcRun, ArcSense, FaceSlot, LayerId, Rgba8, SceneWriter,
};
use indicate_instrument_state::GroupId;

use super::super::{AdmissionError, admit};
use super::opaque_panel;

fn air_panel(draw: indicate_instrument_registry::DrawFn) -> Registry {
    let panels = std::boxed::Box::leak(std::boxed::Box::new(opaque_panel(draw)));
    panels[0].underlay = UnderlayContract::None;
    panels[0].required_groups = GroupSet::of(&[GroupId::Air]);
    Registry::new(panels).expect("structurally valid")
}

fn arc(text: &str, angle: f32) -> ArcRun<'_> {
    ArcRun {
        cx: 60.0,
        cy: 60.0,
        radius: 40.0,
        angle,
        sense: ArcSense::Clockwise,
        align: ArcAlign::Center,
        size: 10.0,
        text,
    }
}

fn open<'s, 'b>(target: SceneTarget<'s, 'b>) -> Result<&'s mut SceneWriter<'b>, PanelDrawError> {
    match target {
        SceneTarget::Planar2D { writer, .. } => {
            writer.begin_layer(LayerId::Tapes)?;
            writer.fill_color(Rgba8::rgb(255, 255, 255))?;
            Ok(writer)
        }
        _ => Err(PanelDrawError::WrongTarget),
    }
}

fn draw_arc_legend(_: &DrawContext<'_>, t: SceneTarget<'_, '_>) -> Result<(), PanelDrawError> {
    let w = open(t)?;
    w.font(FaceSlot(1))?;
    w.text_arc_attributed(ATTR_CONFIG, &arc("LEFT ENGINE RIGHT 100", -1.5))?;
    w.end_layer(LayerId::Tapes)?;
    Ok(())
}

#[test]
fn a_printed_arc_legend_in_a_bound_face_is_admitted() {
    admit(&air_panel(draw_arc_legend)).expect("a fixed arc legend is admissible");
}

fn draw_unclaimed(_: &DrawContext<'_>, t: SceneTarget<'_, '_>) -> Result<(), PanelDrawError> {
    let w = open(t)?;
    w.text_arc(&arc("120", -1.5))?;
    w.end_layer(LayerId::Tapes)?;
    Ok(())
}

#[test]
fn an_unclaimed_arc_numeral_is_refused() {
    assert!(matches!(
        admit(&air_panel(draw_unclaimed)),
        Err(AdmissionError::UntaggedNumeral { .. })
    ));
}

fn draw_claimed(_: &DrawContext<'_>, t: SceneTarget<'_, '_>) -> Result<(), PanelDrawError> {
    let w = open(t)?;
    w.text_arc_attributed(GroupId::Air.to_u8(), &arc("120", -1.5))?;
    w.end_layer(LayerId::Tapes)?;
    Ok(())
}

#[test]
fn an_arc_numeral_claiming_a_withheld_group_is_refused() {
    assert!(matches!(
        admit(&air_panel(draw_claimed)),
        Err(AdmissionError::FabricatedNumeral { .. })
    ));
}

fn draw_moving(ctx: &DrawContext<'_>, t: SceneTarget<'_, '_>) -> Result<(), PanelDrawError> {
    let ias = ctx.data().ias_kt;
    let angle = if ias.status.shows_value() {
        ias.value * 0.01
    } else {
        0.0
    };
    let w = open(t)?;
    w.text_arc_attributed(ATTR_CONFIG, &arc("100", angle))?;
    w.end_layer(LayerId::Tapes)?;
    Ok(())
}

#[test]
fn an_arc_legend_that_moves_with_state_is_refused() {
    assert!(matches!(
        admit(&air_panel(draw_moving)),
        Err(AdmissionError::ConfigClaim { .. })
    ));
}
