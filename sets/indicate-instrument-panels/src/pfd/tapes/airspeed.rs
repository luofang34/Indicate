//! The left-edge airspeed tape: gradations, V-speed bands, the trend
//! bar, and the true-airspeed and groundspeed boxes at its head and
//! foot.

use indicate_instrument_scene::{Anchor, PaintMode, SceneError, SceneWriter};
use indicate_instrument_state::{GroupId, PanelData};
use indicate_instrument_symbology::v_speeds::{
    LinearVSpeedScale, draw_linear_bands, draw_linear_bugs,
};
use indicate_instrument_symbology::{fmt_label, palette, status_paint};

use super::{
    CENTER_Y, IAS_READOUT, READOUT_BOTTOM, READOUT_TOP, SPEED_TAPE_TOP, TAPE_BOTTOM,
    fitted_box_text_size, ladder_label_fits, pointed_readout,
};
use crate::pfd::{VSpeedBugs, VSpeeds};

const PX_PER_KT: f32 = 7.2;

/// Left end of the gradation ticks, which run to the tape's inner edge.
const TICK_LEFT: f32 = 78.0;

/// Right edge of the ladder labels: a gap short of the ticks, so no tick
/// strikes through a digit.
const LABEL_RIGHT: f32 = TICK_LEFT - 2.0;

const PFD_SPEED_SCALE: LinearVSpeedScale = LinearVSpeedScale {
    edge_x: 90.0,
    center_y: CENTER_Y,
    top_y: SPEED_TAPE_TOP,
    bottom_y: TAPE_BOTTOM,
    px_per_kt: PX_PER_KT,
    outer_band_x: 86.0,
    inner_band_x: 82.0,
    band_width: 4.0,
    bug_side: 1.0,
};

/// Left-edge airspeed tape with bands, reference bugs, readout, and the TAS
/// and groundspeed boxes at its head and foot.
pub fn speed_tape(
    scene: &mut SceneWriter<'_>,
    data: &PanelData,
    v: Option<&VSpeeds>,
    bugs: &VSpeedBugs,
    declutter: bool,
) -> Result<(), SceneError> {
    if data.flight.speed_present {
        extended_speed(scene, data)?;
        return gs_box(scene, data);
    }
    let ias = data.ias_kt;
    scene.fill_color(palette::TAPE_BG)?;
    scene.rect(
        PaintMode::Fill,
        0.0,
        SPEED_TAPE_TOP,
        90.0,
        TAPE_BOTTOM - SPEED_TAPE_TOP,
    )?;

    if ias.status.shows_value() {
        if let Some(v) = v {
            draw_linear_bands(scene, ias.value, v, &PFD_SPEED_SCALE)?;
        }
        scene.save()?;
        scene.clip_rect(0.0, SPEED_TAPE_TOP, 90.0, TAPE_BOTTOM - SPEED_TAPE_TOP)?;
        scene.stroke(palette::WHITE, 2.0)?;
        scene.fill_color(palette::WHITE)?;
        let lo = (((ias.value - 26.0) / 5.0) as i32).max(0);
        let hi = ((ias.value + 26.0) / 5.0) as i32;
        for step in lo..=hi {
            let kt = step * 5;
            let y = CENTER_Y - (kt as f32 - ias.value) * PX_PER_KT;
            scene.line(TICK_LEFT, y, 90.0, y)?;
            if step % 2 == 0
                && ladder_label_fits(y, 20.0, SPEED_TAPE_TOP, (READOUT_TOP, READOUT_BOTTOM))
            {
                let label = fmt_label!(8, "{kt}");
                scene.text_attributed(
                    GroupId::Air.to_u8(),
                    LABEL_RIGHT,
                    y,
                    20.0,
                    Anchor::MIDDLE_RIGHT,
                    label.as_str(),
                )?;
            }
        }
        scene.restore()?;
    } else {
        scene.fill_color(palette::GREY)?;
        scene.text(45.0, 130.0, 16.0, Anchor::CENTER, "IAS")?;
    }

    // Pointed readout box, always drawn so `Missing` shows dashes.
    let text = fmt_label!(8, "{:03}", libm::roundf(ias.value) as i32);
    pointed_readout(
        scene,
        GroupId::Air.to_u8(),
        ias,
        text.as_str(),
        &IAS_READOUT,
    )?;

    if !declutter {
        trend_bar(scene, data)?;
        if ias.status.shows_value() {
            draw_linear_bugs(scene, ias.value, bugs, &PFD_SPEED_SCALE)?;
        }
    }

    legacy_targets_or_tas(scene, data)?;
    gs_box(scene, data)?;
    Ok(())
}

fn legacy_targets_or_tas(scene: &mut SceneWriter<'_>, data: &PanelData) -> Result<(), SceneError> {
    use indicate_instrument_state::flight::presentation;
    use indicate_instrument_symbology::flight::speed;
    if presentation::selected_speed(data).is_none() && presentation::effective_speed(data).is_none()
    {
        return tas_box(scene, data);
    }
    speed::target_markers(
        scene,
        data,
        speed::SpeedLayout {
            center: [90.0, CENTER_Y],
            half_height: 155.0,
            readout_x: 45.0,
            text_size: 24.0,
            label_size: 8.0,
            readout_width: 86.0,
            readout_background: None,
            tick_step_pixels: PX_PER_KT * 10.0,
            color: palette::WHITE,
        },
    )?;
    speed::target_readouts(scene, data, [45.0, 8.0], 8.0, 86.0)
}

fn extended_speed(scene: &mut SceneWriter<'_>, data: &PanelData) -> Result<(), SceneError> {
    use indicate_instrument_symbology::flight::speed;
    scene.fill_color(palette::TAPE_BG)?;
    scene.rect(
        PaintMode::Fill,
        0.0,
        SPEED_TAPE_TOP,
        90.0,
        TAPE_BOTTOM - SPEED_TAPE_TOP,
    )?;
    speed::draw(
        scene,
        data,
        speed::SpeedLayout {
            center: [90.0, CENTER_Y],
            half_height: 105.0,
            readout_x: 45.0,
            text_size: 24.0,
            label_size: 8.0,
            readout_width: 86.0,
            readout_background: Some(palette::BOX_BG),
            tick_step_pixels: 55.0,
            color: palette::WHITE,
        },
    )?;
    Ok(())
}

/// How far ahead the trend cue reads, in seconds. The bar marks where
/// the airspeed will be if the current rate holds, so the look-ahead is
/// the whole meaning of its length and belongs beside it.
const TREND_LOOK_AHEAD_S: f32 = 6.0;

/// The airspeed trend bar, just outside the tape's inner edge: from the
/// pointer line to where the airspeed will be after
/// [`TREND_LOOK_AHEAD_S`] at the current rate.
///
/// Drawn only when the tape itself is showing a value — a trend beside
/// dashes marks a change in a number the pilot cannot read. An absent
/// rate draws nothing at all, because a zero-length bar would claim the
/// airspeed is steady, which is a different statement from not knowing.
fn trend_bar(scene: &mut SceneWriter<'_>, data: &PanelData) -> Result<(), SceneError> {
    let trend = data.ias_trend_kt_s;
    if !trend.status.shows_value() || !data.ias_kt.status.shows_value() {
        return Ok(());
    }
    let reach = trend.value * TREND_LOOK_AHEAD_S * PX_PER_KT;
    // The tip stops at the tape's own ends — which start below the
    // true-airspeed box, not at the frame edge. Past them the bar would
    // point at a speed the tape is not showing.
    let tip = (CENTER_Y - reach).clamp(SPEED_TAPE_TOP, TAPE_BOTTOM);
    let (top, height) = if tip < CENTER_Y {
        (tip, CENTER_Y - tip)
    } else {
        (CENTER_Y, tip - CENTER_Y)
    };
    // A not-a-number rate fails every ordering comparison, so a bare
    // length test passes it straight through to a rect no backend can
    // paint. Finiteness is the first question, length the second.
    if !height.is_finite() || height <= 0.0 {
        return Ok(());
    }
    scene.fill_color(palette::MAGENTA)?;
    scene.rect(PaintMode::Fill, 90.0, top, 4.0, height)?;
    Ok(())
}

/// True-airspeed box at the head of the tape, mirroring the groundspeed
/// box at its foot. TAS is air data, so the box wears primary white
/// where the kinematic-derived GS box wears magenta; an absent TAS (a
/// source may supply IAS alone) shows this box's dashes and leaves the
/// tape itself untouched.
fn tas_box(scene: &mut SceneWriter<'_>, data: &PanelData) -> Result<(), SceneError> {
    let tas = data.tas_kt;
    let tas_text = fmt_label!(12, "TAS {:.0}kt", tas.value);
    status_paint::readout_box(
        scene,
        GroupId::Air.to_u8(),
        0.0,
        0.0,
        90.0,
        SPEED_TAPE_TOP,
        tas_text.as_str(),
        palette::WHITE,
        fitted_box_text_size(90.0, tas_text.as_str(), 16.0, tas.status),
        tas.status,
    )
}

/// Groundspeed box at the foot of the tape. Ground speed is derived
/// from the kinematic solution rather than from air data, so it wears
/// magenta and carries the kinematics group's claim.
fn gs_box(scene: &mut SceneWriter<'_>, data: &PanelData) -> Result<(), SceneError> {
    let gs = data.gs_kt;
    let gs_text = fmt_label!(12, "GS {:.0}kt", gs.value);
    status_paint::readout_box(
        scene,
        GroupId::Kinematics.to_u8(),
        0.0,
        TAPE_BOTTOM,
        90.0,
        25.0,
        gs_text.as_str(),
        palette::MAGENTA,
        fitted_box_text_size(90.0, gs_text.as_str(), 16.0, gs.status),
        gs.status,
    )
}
