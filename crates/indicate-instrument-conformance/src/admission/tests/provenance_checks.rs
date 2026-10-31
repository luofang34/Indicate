//! Provenance-family fixtures: totality, claim testing under
//! withholding, claim bounding, and scanner-level structure.
#![allow(clippy::expect_used, clippy::panic)]

use indicate_instrument_registry::{
    DrawContext, GroupSet, PanelDescriptor, PanelDrawError, Region, Registry, SceneTarget,
    UnderlayContract,
};
use indicate_instrument_scene::{Anchor, LayerId, Rgba8};
use indicate_instrument_state::GroupId;

use super::super::{AdmissionError, admit};
use super::{FIXTURE_SPACE, opaque_panel};

const AIR_BOX: &[(GroupId, Region)] = &[(
    GroupId::Air,
    Region {
        x: 0.0,
        y: 0.0,
        width: 100.0,
        height: 100.0,
    },
)];

const fn air_panel(
    id: &'static str,
    groups: GroupSet,
    draw: indicate_instrument_registry::DrawFn,
) -> PanelDescriptor {
    PanelDescriptor {
        id,
        title: "Probe",
        space: FIXTURE_SPACE,
        required_layers: 1 << 2,
        required_groups: groups,
        underlay: UnderlayContract::None,
        config_schema: &[],
        accepted_intents: &[],
        aux_channels: &[],
        group_regions: AIR_BOX,
        extreme_states: &[],
        raster_baselines: &[],
        draw,
    }
}

fn draw_dishonest(
    _ctx: &DrawContext<'_>,
    target: SceneTarget<'_, '_>,
) -> Result<(), PanelDrawError> {
    let SceneTarget::Planar2D { writer: scene, .. } = target else {
        return Err(PanelDrawError::WrongTarget);
    };
    scene.begin_layer(LayerId::Tapes)?;
    scene.fill_color(Rgba8::rgb(255, 255, 255))?;
    scene.text(30.0, 30.0, 14.0, Anchor::CENTER, "999")?;
    scene.end_layer(LayerId::Tapes)?;
    Ok(())
}

#[test]
fn an_unclaimed_numeral_is_refused() {
    static DISHONEST: [PanelDescriptor; 1] = [air_panel(
        "dishonest",
        GroupSet::of(&[GroupId::Air]),
        draw_dishonest,
    )];
    let registry = Registry::new(&DISHONEST).expect("structurally valid");
    assert!(matches!(
        admit(&registry),
        Err(AdmissionError::UntaggedNumeral {
            panel: "dishonest",
            ..
        })
    ));
}

fn draw_leaking(ctx: &DrawContext<'_>, target: SceneTarget<'_, '_>) -> Result<(), PanelDrawError> {
    let SceneTarget::Planar2D { writer: scene, .. } = target else {
        return Err(PanelDrawError::WrongTarget);
    };
    let data = ctx.data();
    scene.begin_layer(LayerId::Tapes)?;
    scene.fill_color(Rgba8::rgb(255, 255, 255))?;
    let air_missing = !data.ias_kt.status.shows_value();
    let kinematics_present = data.gs_kt.status.shows_value();
    if air_missing && kinematics_present {
        scene.text_attributed(
            GroupId::Air.to_u8(),
            30.0,
            30.0,
            14.0,
            Anchor::CENTER,
            "999",
        )?;
    }
    scene.end_layer(LayerId::Tapes)?;
    Ok(())
}

#[test]
fn a_fabricated_claim_is_refused_when_its_group_is_withheld() {
    static LEAKING: [PanelDescriptor; 1] = [air_panel(
        "leaking",
        GroupSet::of(&[GroupId::Air, GroupId::Kinematics]),
        draw_leaking,
    )];
    let registry = Registry::new(&LEAKING).expect("structurally valid");
    assert!(matches!(
        admit(&registry),
        Err(AdmissionError::FabricatedNumeral {
            panel: "leaking",
            group: GroupId::Air,
            ..
        })
    ));
}

fn draw_translated(
    ctx: &DrawContext<'_>,
    target: SceneTarget<'_, '_>,
) -> Result<(), PanelDrawError> {
    let SceneTarget::Planar2D { writer: scene, .. } = target else {
        return Err(PanelDrawError::WrongTarget);
    };
    let data = ctx.data();
    scene.begin_layer(LayerId::Tapes)?;
    scene.fill_color(Rgba8::rgb(255, 255, 255))?;
    let air_missing = !data.ias_kt.status.shows_value();
    let kinematics_present = data.gs_kt.status.shows_value();
    if air_missing && kinematics_present {
        scene.save()?;
        scene.translate(300.0, 300.0)?;
        scene.text_attributed(
            GroupId::Air.to_u8(),
            -270.0,
            -270.0,
            14.0,
            Anchor::CENTER,
            "999",
        )?;
        scene.restore()?;
    }
    scene.end_layer(LayerId::Tapes)?;
    Ok(())
}

#[test]
fn a_transform_cannot_hide_a_dishonest_numeral() {
    static TRANSLATED: [PanelDescriptor; 1] = [air_panel(
        "translated",
        GroupSet::of(&[GroupId::Air, GroupId::Kinematics]),
        draw_translated,
    )];
    let registry = Registry::new(&TRANSLATED).expect("structurally valid");
    assert!(matches!(
        admit(&registry),
        Err(AdmissionError::FabricatedNumeral {
            panel: "translated",
            group: GroupId::Air,
            ..
        })
    ));
}

fn draw_foreign_claim(
    _ctx: &DrawContext<'_>,
    target: SceneTarget<'_, '_>,
) -> Result<(), PanelDrawError> {
    let SceneTarget::Planar2D { writer: scene, .. } = target else {
        return Err(PanelDrawError::WrongTarget);
    };
    scene.begin_layer(LayerId::Tapes)?;
    scene.fill_color(Rgba8::rgb(255, 255, 255))?;
    scene.text_attributed(
        GroupId::Wind.to_u8(),
        30.0,
        30.0,
        14.0,
        Anchor::CENTER,
        "999",
    )?;
    scene.end_layer(LayerId::Tapes)?;
    Ok(())
}

#[test]
fn a_claim_outside_the_required_groups_is_refused() {
    let panels = std::boxed::Box::leak(std::boxed::Box::new(opaque_panel(draw_foreign_claim)));
    panels[0].underlay = UnderlayContract::None;
    let registry = Registry::new(panels).expect("structurally valid");
    assert!(matches!(
        admit(&registry),
        Err(AdmissionError::ForeignClaim { tag, .. }) if tag == GroupId::Wind.to_u8()
    ));
}

fn draw_config_claim(
    _ctx: &DrawContext<'_>,
    target: SceneTarget<'_, '_>,
) -> Result<(), PanelDrawError> {
    let SceneTarget::Planar2D { writer: scene, .. } = target else {
        return Err(PanelDrawError::WrongTarget);
    };
    scene.begin_layer(LayerId::Tapes)?;
    scene.fill_color(Rgba8::rgb(255, 255, 255))?;
    scene.text_attributed(
        indicate_instrument_scene::ATTR_CONFIG,
        30.0,
        30.0,
        14.0,
        Anchor::CENTER,
        "V1 65",
    )?;
    scene.end_layer(LayerId::Tapes)?;
    Ok(())
}

#[test]
fn a_config_claim_is_refused_under_the_empty_config() {
    let panels = std::boxed::Box::leak(std::boxed::Box::new(opaque_panel(draw_config_claim)));
    panels[0].underlay = UnderlayContract::None;
    let registry = Registry::new(panels).expect("structurally valid");
    assert!(matches!(
        admit(&registry),
        Err(AdmissionError::ConfigClaim { .. })
    ));
}

fn draw_clipped_away(
    _ctx: &DrawContext<'_>,
    target: SceneTarget<'_, '_>,
) -> Result<(), PanelDrawError> {
    let SceneTarget::Planar2D { writer: scene, .. } = target else {
        return Err(PanelDrawError::WrongTarget);
    };
    scene.begin_layer(LayerId::Tapes)?;
    scene.save()?;
    scene.clip_rect(0.0, 0.0, 10.0, 10.0)?;
    scene.fill_color(Rgba8::rgb(255, 255, 255))?;
    scene.text_attributed(
        GroupId::Air.to_u8(),
        200.0,
        200.0,
        14.0,
        Anchor::CENTER,
        "999",
    )?;
    scene.restore()?;
    scene.end_layer(LayerId::Tapes)?;
    Ok(())
}

#[test]
fn a_claimed_numeral_clipped_fully_away_is_tolerated() {
    let panels = std::boxed::Box::leak(std::boxed::Box::new(opaque_panel(draw_clipped_away)));
    panels[0].underlay = UnderlayContract::None;
    panels[0].required_groups = GroupSet::of(&[GroupId::Air]);
    let registry = Registry::new(panels).expect("structurally valid");
    admit(&registry).expect("an unshown claim is not a fabrication");
}

#[test]
fn a_dangling_claim_is_refused_by_the_scanner() {
    let bytes = [1u8, 0x31, 1, 0, 2, 0x31, 1, 0, 3];
    assert!(matches!(
        super::super::collect_runs(&bytes),
        Err(super::super::RunsDefect::MisplacedClaim)
    ));
    let bytes = [
        1u8, 0x31, 1, 0, 2, 0x23, 17, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 128, 63, 0, 0, 128, 63,
    ];
    assert!(matches!(
        super::super::collect_runs(&bytes),
        Err(super::super::RunsDefect::MisplacedClaim)
    ));
}
