//! Mode, simulation, weapon, failure, and alert annunciations.
use super::*;
use crate::context::{HudProfile, WeaponMode, WorldSource};
use indicate_instrument_symbology::annunciation::draw_alert_stack_at;

pub(super) fn draw(
    data: &PanelData,
    context: &HudContext,
    mode: HudMode,
    alerts: Option<&AlertOutput>,
    layout: &Layout,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    scene.begin_layer(LayerId::Annunciation)?;
    let u = layout.unit;
    let [cx, _] = layout.centre;
    let bottom = layout.height - 12.0 * u;
    scene.fill_color(HUD_GREEN)?;
    if let Some(identity) = identity(context, mode) {
        scene.text(
            cx,
            bottom - 15.0 * u,
            layout.text(11.0),
            Anchor::CENTER,
            identity,
        )?;
    }
    scene.text(
        cx,
        bottom,
        layout.text(10.0),
        Anchor::CENTER,
        "SIM / NOT FOR FLIGHT",
    )?;
    if context.approach.as_ref().is_some_and(|r| r.usable()) {
        scene.text(
            70.0 * u,
            18.0 * u,
            layout.text(15.0),
            Anchor::MIDDLE_LEFT,
            "APP",
        )?;
    }
    if context.profile == HudProfile::Screen {
        let weapon = match context.weapon {
            WeaponMode::Navigation => "NAV",
            WeaponMode::Gun => "GUN",
            WeaponMode::ShortRangeMissile => "SRM",
            WeaponMode::MediumRangeMissile => "MRM",
            WeaponMode::Rockets => "RKT",
            WeaponMode::GuidedBomb => "LGB",
            WeaponMode::DirectedEnergy => "DEW",
        };
        scene.text(
            14.0 * u,
            18.0 * u,
            layout.text(15.0),
            Anchor::MIDDLE_LEFT,
            weapon,
        )?;
    }
    flags(data, context, mode, layout, scene)?;
    if let Some(alerts) = alerts {
        let origin = [layout.width - 170.0 * u, 86.0 * u];
        // The stack grows upward from its origin by up to four rows.
        draw_alert_stack_at(scene, alerts, origin, 17.0 * u, layout.text(12.0))?;
    }
    scene.end_layer(LayerId::Annunciation)?;
    Ok(())
}

/// The conformality label. A screen over its own synthetic world makes no HUD
/// claim to qualify; a combiner or camera video always carries one.
fn identity(context: &HudContext, mode: HudMode) -> Option<&'static str> {
    let video = context.projection.value.source == WorldSource::Video;
    match (context.profile, mode) {
        (HudProfile::Combiner, HudMode::Conformal) => Some("HUD-SIM"),
        // The outside view itself is synthetic vision; the label says so.
        (HudProfile::Screen, HudMode::Conformal) if !video => Some("SVS"),
        (HudProfile::Screen, HudMode::Conformal) => None,
        (HudProfile::Screen, HudMode::Repeater) if !video => Some("CAMERA INVALID"),
        (_, HudMode::Repeater) => Some("NON-CONFORMAL / NOT A HUD"),
    }
}

fn flags(
    data: &PanelData,
    context: &HudContext,
    mode: HudMode,
    layout: &Layout,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let u = layout.unit;
    let [cx, cy] = layout.centre;
    let attitude = shown_status(data.roll_rad).worst(shown_status(data.pitch_rad));
    if mode == HudMode::Conformal && attitude != SignalStatus::Valid {
        scene.fill_color(status_color(layout, attitude))?;
        let text = fmt_label!(24, "ATT {}", status_label(attitude));
        scene.text(
            cx,
            cy - 40.0 * u,
            layout.text(18.0),
            Anchor::CENTER,
            text.as_str(),
        )?;
    }
    if data.presentation.unusual && attitude.shows_value() {
        unusual_attitude(data, mode == HudMode::Conformal, layout, scene)?;
    }
    let status_y = layout.height - 44.0 * u;
    scene.fill_color(layout.ink(safety::CAUTION_AMBER))?;
    let combat = !context.targets.contacts().is_empty() || context.weapon != WeaponMode::Navigation;
    let combat_shown = mode == HudMode::Conformal && context.profile == HudProfile::Screen;
    if combat && !combat_shown {
        scene.text(
            cx,
            status_y - 15.0 * u,
            layout.text(11.0),
            Anchor::CENTER,
            "TGT CUES OFF",
        )?;
    }
    if mode == HudMode::Conformal {
        if attitude.shows_value() && ladder::velocity_ned(data).is_none() {
            scene.text(
                cx,
                status_y,
                layout.text(11.0),
                Anchor::CENTER,
                "FPM UNAVAILABLE",
            )?;
        }
        let frame = DesignFrame {
            width: layout.width,
            height: layout.height,
        };
        let world = Camera::new(data, context.projection.value, frame);
        if combat_shown && !context.targets.contacts().is_empty() && !world.world_basis() {
            scene.text(
                cx,
                status_y - 15.0 * u,
                layout.text(11.0),
                Anchor::CENTER,
                "TGT NO TRUE HDG",
            )?;
        }
    }
    Ok(())
}

/// The unusual-attitude label, its sense, and the non-conformal ladder state.
fn unusual_attitude(
    data: &PanelData,
    compressed: bool,
    layout: &Layout,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let u = layout.unit;
    let [cx, _] = layout.centre;
    scene.fill_color(layout.ink(safety::CAUTION_AMBER))?;
    scene.text(
        cx,
        recovery::label_band(layout),
        layout.text(16.0),
        Anchor::CENTER,
        "UNUSUAL ATTITUDE",
    )?;
    let sense = if data.presentation.inverted {
        "INVERTED"
    } else if data.presentation.nose_high {
        "NOSE HIGH"
    } else if data.presentation.nose_low {
        "NOSE LOW"
    } else {
        "HIGH BANK"
    };
    scene.text(
        cx,
        recovery::label_band(layout) + 18.0 * u,
        layout.text(13.0),
        Anchor::CENTER,
        sense,
    )?;
    if compressed {
        scene.text(
            cx,
            recovery::label_band(layout) + 33.0 * u,
            layout.text(11.0),
            Anchor::CENTER,
            "ATT COMPRESSED",
        )?;
    }
    Ok(())
}
