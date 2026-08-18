//! The left-edge airspeed tape: gradations, V-speed bands, the trend
//! bar, and the true-airspeed and groundspeed boxes at its head and
//! foot.

use indicate_instrument_scene::{
    Anchor, PaintMode, Rgba8, SceneError, SceneWriter, nominal_text_ink_width,
};
use indicate_instrument_state::{GroupId, PanelData};
use indicate_instrument_symbology::{fmt_label, palette, safety, status_paint};

use super::{CENTER_Y, IAS_READOUT, TAPE_BOTTOM, pointed_readout};
use crate::pfd::VSpeeds;

const PX_PER_KT: f32 = 7.2;

/// Top of the airspeed tape. The true-airspeed box owns the strip above
/// it: the box is opaque, so a tape that started at the frame edge
/// would have its topmost gradation painted over rather than covered by
/// a box beside it.
const SPEED_TAPE_TOP: f32 = 25.0;

/// Left-edge airspeed tape with bands, readout, and the TAS and
/// groundspeed boxes at its head and foot.
pub fn speed_tape(
    scene: &mut SceneWriter<'_>,
    data: &PanelData,
    v: Option<&VSpeeds>,
    declutter: bool,
) -> Result<(), SceneError> {
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
            speed_bands(scene, ias.value, v)?;
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
            scene.line(78.0, y, 90.0, y)?;
            if step % 2 == 0 {
                let label = fmt_label!(8, "{kt}");
                scene.text_attributed(
                    GroupId::Air.to_u8(),
                    70.0,
                    y,
                    20.0,
                    Anchor::CENTER,
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
    }

    tas_box(scene, data)?;
    gs_box(scene, data)?;
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
        fitted_label_size(90.0, tas_text.as_str().chars().count(), 16.0),
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
        16.0,
        gs.status,
    )
}

/// Largest size, capped at `preferred`, whose nominal ink fits `width`.
///
/// A centered label in a box of fixed width overflows on both sides once
/// its ink outstrips the box, and the frame edge is one of those sides:
/// `TAS 113kt` at 16 units carries 121 units of ink into a 90-unit box,
/// so the leading glyph paints off the panel entirely. The size follows
/// the value's width instead, which is what the pointed readouts already
/// do (DISP-02).
fn fitted_label_size(width: f32, chars: usize, preferred: f32) -> f32 {
    let ink = nominal_text_ink_width(preferred, chars);
    if ink <= width {
        return preferred;
    }
    preferred * width / ink
}

fn speed_bands(scene: &mut SceneWriter<'_>, ias: f32, v: &VSpeeds) -> Result<(), SceneError> {
    let segs: [(f32, f32, Rgba8); 3] = [
        (v.vs_kt, v.vno_kt, palette::BAND_GREEN),
        (v.vno_kt, v.vne_kt, safety::BAND_CAUTION),
        (v.vne_kt, v.vne_kt + 1000.0, safety::FAILURE_RED),
    ];
    for (lo, hi, color) in segs {
        band_rect(scene, ias, lo, hi, 86.0, 4.0, color)?;
    }
    band_rect(scene, ias, v.vs0_kt, v.vfe_kt, 82.0, 4.0, palette::WHITE)?;
    Ok(())
}

fn band_rect(
    scene: &mut SceneWriter<'_>,
    ias: f32,
    lo_kt: f32,
    hi_kt: f32,
    x: f32,
    w: f32,
    color: Rgba8,
) -> Result<(), SceneError> {
    let y_top = (CENTER_Y - (hi_kt - ias) * PX_PER_KT).max(SPEED_TAPE_TOP);
    let y_bot = (CENTER_Y - (lo_kt - ias) * PX_PER_KT).min(TAPE_BOTTOM);
    if y_bot > y_top {
        scene.fill_color(color)?;
        scene.rect(PaintMode::Fill, x, y_top, w, y_bot - y_top)?;
    }
    Ok(())
}
