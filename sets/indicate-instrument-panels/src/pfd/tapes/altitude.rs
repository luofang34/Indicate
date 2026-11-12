//! The right-edge altitude tape: gradations, the selected-altitude bug,
//! the reference label, the baro and selection boxes, and the
//! vertical-speed bar beside it.

use indicate_instrument_scene::{
    Anchor, PaintMode, SceneError, SceneWriter, nominal_text_ink_width,
};
use indicate_instrument_state::{AltitudeClass, Sig, SignalStatus};
use indicate_instrument_state::{GroupId, PanelData};
use indicate_instrument_symbology::{fmt_label, palette, safety, status_paint};

use super::super::drum;
use super::{
    ALT_READOUT, ALTITUDE_TAPE_TOP, CENTER_Y, READOUT_TOP, TAPE_BOTTOM, fitted_box_text_size,
    fitted_text_size, ladder_label_fits, pointed_box, source_band,
};

const PX_PER_FT: f32 = 1.2;

/// Where the altitude reference label (BARO, STD, REL, ...) sits under
/// the readout box, and its run size.
const REFERENCE_LABEL_Y: f32 = 222.0;
/// Run size of the altitude reference label.
const REFERENCE_LABEL_SIZE: f32 = 12.0;

/// The band ladder labels stay clear of: the readout box and the
/// reference label under it.
const LADDER_KEEP_OUT: (f32, f32) = (READOUT_TOP, REFERENCE_LABEL_Y + REFERENCE_LABEL_SIZE / 2.0);

/// Right end of the gradation ticks, which start at the tape's inner
/// edge.
const TICK_RIGHT: f32 = 400.0;

/// Left edge of the ladder labels: a gap past the ticks.
const LABEL_LEFT: f32 = TICK_RIGHT + 4.0;

/// Left edge of the vertical-speed bar at the tape's outer edge.
const VSI_LEFT: f32 = 466.0;

/// The widest a ladder label's ink may run: a gap short of the
/// vertical-speed bar.
const LABEL_INK_WIDTH: f32 = VSI_LEFT - 2.0 - LABEL_LEFT;

/// Run size of a ladder label.
const LABEL_SIZE: f32 = 18.0;

/// Run size of the vertical-speed label.
const VSI_LABEL_SIZE: f32 = 12.0;

/// A ladder label's run size: the preferred size, shrunk so a long
/// value ("-1300", "12500") stays clear of the vertical-speed bar.
fn label_size(chars: usize) -> f32 {
    let ink = nominal_text_ink_width(LABEL_SIZE, chars);
    if ink <= LABEL_INK_WIDTH {
        LABEL_SIZE
    } else {
        LABEL_SIZE * LABEL_INK_WIDTH / ink
    }
}

/// Row and run size of the amber `BARO SEL` flag, just above the
/// barometric-setting box.
const BARO_SEL_Y: f32 = TAPE_BOTTOM - 7.0;
/// Run size of the `BARO SEL` flag.
const BARO_SEL_SIZE: f32 = 11.0;

/// The band a centered run of `size` at `y` occupies.
fn band(y: f32, size: f32) -> (f32, f32) {
    (y - size / 2.0, y + size / 2.0)
}

/// The band the `BARO SEL` flag occupies when it draws.
fn baro_sel_band(data: &PanelData) -> Option<(f32, f32)> {
    data.altitude
        .setting_mismatch
        .then(|| band(BARO_SEL_Y, BARO_SEL_SIZE))
}

/// Where the vertical-speed label sits, if it draws: at the bar's tip,
/// moved clear of the readout band, the reference label, the source
/// label and the `BARO SEL` flag, so it never lands on a box or on
/// another label. A missing rate puts its dashes just above the
/// readout.
fn vsi_label_y(data: &PanelData) -> Option<f32> {
    let v = data.vsi_fpm;
    let half = VSI_LABEL_SIZE / 2.0;
    let clear_above = LADDER_KEEP_OUT.0 - half;
    if !v.status.shows_value() {
        return Some(clear_above);
    }
    if v.value.abs() < 100.0 {
        return None;
    }
    let len = vsi_length(v.value);
    // At full scale the label parks at the strip edge, clear of the
    // selected-altitude box above and the baro box or flag below.
    let floor = baro_sel_band(data).map_or(TAPE_BOTTOM, |(top, _)| top) - half - 1.0;
    let tip_y = (CENTER_Y - len).clamp(ALTITUDE_TAPE_TOP + half + 2.0, floor);
    if len >= 0.0 {
        return Some(tip_y.min(clear_above));
    }
    let mut y = tip_y.max(LADDER_KEEP_OUT.1 + half);
    if let Some((top, bottom)) = source_band(&data.sources.altitude)
        && y + half > top
        && y - half < bottom
    {
        y = bottom + half;
    }
    Some(y.min(floor))
}

/// Bar length for a vertical speed: ±1500 fpm full scale over 180 px.
fn vsi_length(fpm: f32) -> f32 {
    (fpm / 1500.0 * 180.0).clamp(-170.0, 170.0)
}

/// Right-edge altitude tape with selected-altitude bug and baro box.
/// The tape carries its reference label (REL amber, BARO/STD/MSL/AGL
/// white, RED for an unknown reference) so a local-relative height can
/// never read as barometric altitude; the bug and selection readout
/// render only when the selection's reference class matches.
pub fn altitude_tape(scene: &mut SceneWriter<'_>, data: &PanelData) -> Result<(), SceneError> {
    let alt = data.altitude.value_ft;
    scene.fill_color(palette::TAPE_BG)?;
    scene.rect(
        PaintMode::Fill,
        390.0,
        ALTITUDE_TAPE_TOP,
        90.0,
        TAPE_BOTTOM - ALTITUDE_TAPE_TOP,
    )?;

    if alt.status.shows_value() {
        scene.save()?;
        scene.clip_rect(
            390.0,
            ALTITUDE_TAPE_TOP,
            90.0,
            TAPE_BOTTOM - ALTITUDE_TAPE_TOP,
        )?;
        scene.stroke(palette::WHITE, 2.0)?;
        scene.fill_color(palette::WHITE)?;
        let keep_out = [
            Some(LADDER_KEEP_OUT),
            source_band(&data.sources.altitude),
            vsi_label_y(data).map(|y| band(y, VSI_LABEL_SIZE)),
            baro_sel_band(data),
        ];
        let lo = ((alt.value - 155.0) / 20.0) as i32;
        let hi = ((alt.value + 155.0) / 20.0) as i32;
        for step in lo..=hi {
            let ft = step * 20;
            let y = CENTER_Y - (ft as f32 - alt.value) * PX_PER_FT;
            scene.line(390.0, y, TICK_RIGHT, y)?;
            if step.rem_euclid(5) == 0
                && ladder_label_fits(y, LABEL_SIZE, ALTITUDE_TAPE_TOP, &keep_out)
            {
                let label = fmt_label!(12, "{ft}");
                scene.text_attributed(
                    altitude_claim(data),
                    LABEL_LEFT,
                    y,
                    label_size(label.as_str().chars().count()),
                    Anchor::MIDDLE_LEFT,
                    label.as_str(),
                )?;
            }
        }
        if let (true, Some(sel_m)) = (data.altitude.bug_compatible, data.selections.altitude_sel_m)
        {
            let sel_ft = sel_m * indicate_instrument_state::units::M_TO_FT;
            let y = (CENTER_Y - (sel_ft - alt.value) * PX_PER_FT)
                .clamp(ALTITUDE_TAPE_TOP + 4.0, TAPE_BOTTOM - 4.0);
            scene.fill_color(palette::CYAN)?;
            scene.polygon(
                PaintMode::Fill,
                &[
                    [390.0, y - 8.0],
                    [398.0, y - 8.0],
                    [398.0, y - 3.0],
                    [393.0, y],
                    [398.0, y + 3.0],
                    [398.0, y + 8.0],
                    [390.0, y + 8.0],
                ],
            )?;
        }
        scene.restore()?;
    } else {
        scene.fill_color(palette::GREY)?;
        scene.text(435.0, 130.0, 16.0, Anchor::CENTER, "ALT")?;
    }

    altitude_readout(scene, data, alt)?;
    reference_label(scene, data)?;
    baro_and_sel_boxes(scene, data)?;
    Ok(())
}

/// The group an altitude value derives from under the declared class:
/// local-relative altitude is kinematic; every other class rides the
/// air-data group's stamp (the altitude group only qualifies the
/// datum), and its claim must say so or a barometric altitude would be
/// refused as fabricated under kinematics withholding.
fn altitude_claim(data: &PanelData) -> u8 {
    match data.altitude.class {
        AltitudeClass::LocalRelative => GroupId::Kinematics.to_u8(),
        _ => GroupId::Air.to_u8(),
    }
}

/// The altitude reference label under the value box. REL is amber —
/// simulator-relative height demands attention — and an unknown wire
/// reference is red beside its failed tape.
fn reference_label(scene: &mut SceneWriter<'_>, data: &PanelData) -> Result<(), SceneError> {
    let class = data.altitude.class;
    scene.fill_color(match class {
        AltitudeClass::LocalRelative => safety::CAUTION_AMBER,
        AltitudeClass::Unknown => safety::FAILURE_RED,
        _ => palette::WHITE,
    })?;
    scene.text(
        442.0,
        REFERENCE_LABEL_Y,
        REFERENCE_LABEL_SIZE,
        Anchor::CENTER,
        class.label(),
    )?;
    Ok(())
}

/// Setting and selection boxes. The setting readout states whether the
/// shown setting is applied: a barometric tape shows the applied value
/// in cyan, a pressure tape shows STD, and every other reference shows
/// the setting prefixed SET in grey — visibly not applied to the tape.
/// A selected/applied disagreement adds the amber BARO SEL flag.
fn baro_and_sel_boxes(scene: &mut SceneWriter<'_>, data: &PanelData) -> Result<(), SceneError> {
    let baro = data.baro_hpa;
    let (text, color) = match data.altitude.class {
        AltitudeClass::BaroIndicated => (fmt_label!(12, "{:.0}", baro.value), palette::CYAN),
        AltitudeClass::Pressure => (fmt_label!(12, "STD"), palette::CYAN),
        _ => (fmt_label!(12, "SET {:.0}", baro.value), palette::GREY),
    };
    status_paint::readout_box(
        scene,
        GroupId::Air.to_u8(),
        390.0,
        TAPE_BOTTOM,
        90.0,
        25.0,
        text.as_str(),
        color,
        fitted_box_text_size(90.0, text.as_str(), 16.0, baro.status),
        baro.status,
    )?;
    if data.altitude.setting_mismatch {
        status_paint::draw_flag(scene, 435.0, BARO_SEL_Y, "BARO SEL")?;
    }
    match (data.altitude.bug_compatible, data.selections.altitude_sel_m) {
        (true, Some(sel_m)) => {
            let sel_ft = sel_m * indicate_instrument_state::units::M_TO_FT;
            let text = fmt_label!(12, "{}", libm::roundf(sel_ft) as i64);
            scene.fill_color(palette::BOX_BG)?;
            scene.stroke(palette::GREY, 1.5)?;
            scene.rect(PaintMode::FillStroke, 390.0, 0.0, 90.0, ALTITUDE_TAPE_TOP)?;
            scene.fill_color(palette::CYAN)?;
            scene.text_attributed(
                GroupId::Selections.to_u8(),
                435.0,
                12.0,
                fitted_box_text_size(90.0, text.as_str(), 18.0, SignalStatus::Valid),
                Anchor::CENTER,
                text.as_str(),
            )?;
        }
        (false, Some(_)) => {
            // A selection in an incompatible reference never renders as
            // a plausible number; the amber marker says why it is gone.
            scene.fill_color(safety::CAUTION_AMBER)?;
            scene.text(435.0, 12.0, 14.0, Anchor::CENTER, "SEL REF")?;
        }
        (_, None) => {}
    }
    Ok(())
}

/// Vertical-speed bar at the right edge of the altitude tape.
pub fn vsi(scene: &mut SceneWriter<'_>, data: &PanelData) -> Result<(), SceneError> {
    let v = data.vsi_fpm;
    scene.stroke(palette::GREY, 1.0)?;
    for dy in [-120.0f32, -60.0, 60.0, 120.0] {
        scene.line(VSI_LEFT, CENTER_Y + dy, VSI_LEFT + 8.0, CENTER_Y + dy)?;
    }
    if !v.status.shows_value() {
        // Dashes rather than an empty strip. Every other readout on this
        // panel says "I have no value" out loud, and a blank scale reads
        // as zero vertical speed to anyone scanning it — which is the
        // fabrication this signal's own validity exists to prevent. The
        // dash path stays unclaimed, like the others: it is the honest
        // degraded display, not a value derived from a withheld group.
        scene.fill_color(palette::WHITE)?;
        let y = vsi_label_y(data).unwrap_or(CENTER_Y);
        scene.text(452.0, y, VSI_LABEL_SIZE, Anchor::CENTER, "---")?;
        return Ok(());
    }
    let len = vsi_length(v.value);
    scene.fill_color(palette::MAGENTA)?;
    if len >= 0.0 {
        scene.rect(PaintMode::Fill, VSI_LEFT, CENTER_Y - len, 8.0, len.max(1.0))?;
    } else {
        scene.rect(PaintMode::Fill, VSI_LEFT, CENTER_Y, 8.0, -len)?;
    }
    if let Some(y) = vsi_label_y(data) {
        let label = fmt_label!(12, "{}", libm::roundf(v.value / 50.0) as i64 * 50);
        scene.fill_color(palette::WHITE)?;
        scene.text_attributed(
            GroupId::Kinematics.to_u8(),
            452.0,
            y,
            VSI_LABEL_SIZE,
            Anchor::CENTER,
            label.as_str(),
        )?;
    }
    Ok(())
}

/// The altitude readout: the pointed box every tape value gets, with a
/// rolling-digit drum for its interior. Missing/Failed/Stale paint the
/// same unclaimed dashes as the airspeed readout — the honesty paths do
/// not move with the interior.
fn altitude_readout(
    scene: &mut SceneWriter<'_>,
    data: &PanelData,
    alt: Sig<f32>,
) -> Result<(), SceneError> {
    pointed_box(scene, alt, &ALT_READOUT)?;
    if alt.status.shows_value() {
        drum::draw(scene, altitude_claim(data), alt.value, &ALT_READOUT)
    } else {
        // Unclaimed, like every dash path: dashes are the honest
        // degraded display, not a value derived from a withheld group.
        let size = fitted_text_size(&ALT_READOUT, 3);
        scene.text(ALT_READOUT.text_x, 180.0, size, Anchor::CENTER, "---")
    }
}
