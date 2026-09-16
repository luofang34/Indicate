//! Missing data and datum disagreements stay visible in both layouts.
use super::*;

pub(super) fn draw(
    data: &PanelData,
    alerts: Option<&AlertOutput>,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    scene.begin_layer(LayerId::Annunciation)?;
    use indicate_instrument_symbology::{annunciation, flight::modes};
    modes::draw(
        scene,
        data,
        modes::FmaLayout {
            center: [600.0, 156.0],
            column_step: 125.0,
            row_step: 22.0,
            text_size: 15.0,
            attention_ms: 5000.0,
        },
    )?;
    if let Some(alerts) = alerts {
        annunciation::draw_alert_stack_at(scene, alerts, [730.0, 540.0], 20.0, 14.0)?;
    }
    let mut attitude = data.roll_rad.status.worst(data.pitch_rad.status);
    if attitude.shows_value()
        && (!data.roll_rad.value.is_finite() || !data.pitch_rad.value.is_finite())
    {
        attitude = SignalStatus::Failed;
    }
    status(scene, "ATT", attitude, 544.0)?;
    if !live(data.track_rad) || !live(data.gs_kt) || !live(data.vsi_fpm) {
        scene.fill_color(safety::CAUTION_AMBER)?;
        scene.text(
            600.0,
            568.0,
            12.0,
            Anchor::CENTER,
            "FLIGHT PATH UNAVAILABLE",
        )?;
    }
    if data.altitude.setting_mismatch {
        scene.fill_color(safety::CAUTION_AMBER)?;
        scene.text(1000.0, 538.0, 12.0, Anchor::CENTER, "BARO CHECK")?;
    }
    scene.end_layer(LayerId::Annunciation)?;
    Ok(())
}

fn status(
    scene: &mut SceneWriter<'_>,
    label: &str,
    status: SignalStatus,
    y: f32,
) -> Result<(), PanelDrawError> {
    if status == SignalStatus::Valid {
        return Ok(());
    }
    scene.fill_color(readout::color(status))?;
    let text = fmt_label!(24, "{} {}", label, readout::status_label(status));
    scene.text(600.0, y, 13.0, Anchor::CENTER, text.as_str())?;
    Ok(())
}
