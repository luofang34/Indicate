//! Cards, tapes, and the fixed overlays in front of the moving parts.

use indicate_instrument_descriptor::PanelDrawError;
use indicate_instrument_scene::{Anchor, PaintMode, SceneWriter};

use crate::face::Placement;
use crate::pose::{DialPose, TapePose};
use crate::shape::{Frame, draw_marks};
use crate::spec::{AnalogDialSpec, Lighting, TapeSense, TapeSpec};

/// Draws every visible card: its shapes turned about its pivot and slid
/// along its own vertical axis.
pub(super) fn draw_cards(
    spec: &AnalogDialSpec<'_>,
    pose: &DialPose,
    lighting: Lighting,
    place: &Placement,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    for (card, card_pose) in spec.cards.iter().zip(&pose.cards) {
        if !card_pose.visible {
            continue;
        }
        let [x, y] = place.at(card.pivot);
        scene.save()?;
        scene.translate(x, y)?;
        scene.rotate(card_pose.angle_rad)?;
        scene.translate(0.0, card_pose.offset * place.scale)?;
        let frame = Frame {
            origin: [0.0, 0.0],
            scale: place.scale,
        };
        draw_marks(card.shapes, lighting, frame, scene)?;
        scene.restore()?;
    }
    Ok(())
}

/// Draws every tape: the card behind its window, and, when the tape
/// shows a value, the graduations and numerals in view.
pub(super) fn draw_tapes(
    spec: &AnalogDialSpec<'_>,
    pose: &DialPose,
    lighting: Lighting,
    place: &Placement,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    for (tape, tape_pose) in spec.tapes.iter().zip(&pose.tapes) {
        let group = spec
            .channel(tape.channel)
            .map_or(0, |channel| channel.input.group().to_u8());
        draw_tape(tape, tape_pose, group, lighting, place, scene)?;
    }
    Ok(())
}

fn draw_tape(
    tape: &TapeSpec<'_>,
    pose: &TapePose,
    group: u8,
    lighting: Lighting,
    place: &Placement,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let [ax, ay, aw, ah] = tape.aperture;
    let [x, y] = place.at([ax, ay]);
    let (w, h) = (aw * place.scale, ah * place.scale);
    scene.fill_color(tape.drum)?;
    scene.rect(PaintMode::Fill, x, y, w, h)?;
    if !pose.live {
        // A blank card: a printed heading would claim a value.
        return Ok(());
    }
    let sign = match tape.sense {
        TapeSense::RightIncreasing => 1.0,
        TapeSense::LeftIncreasing => -1.0,
    };
    let per_unit = tape.pitch * place.scale;
    let size = tape.numeral_size * place.scale;
    // Values further than this from the reading point are out of view;
    // a numeral is let in half a numeral early so it slides in whole.
    let reach = (w * 0.5 + size * 2.0) / per_unit;
    let along = |value: f32| sign * (pose.value + nearest(value - pose.value, tape.period));
    scene.save()?;
    scene.clip_rect(x, y, w, h)?;
    // The strip moves by one translate: the reading point at the
    // window's centre.
    scene.translate(x + w * 0.5 - sign * pose.value * per_unit, y)?;
    for ticks in tape.ticks {
        scene.stroke(tape.paint.under(lighting), ticks.width * place.scale)?;
        let count = libm::floorf(tape.period / ticks.every + 1e-3) as u32;
        for k in 0..count {
            let value = ticks.every * k as f32;
            if nearest(value - pose.value, tape.period).abs() > reach {
                continue;
            }
            let at = along(value) * per_unit;
            scene.line(at, ticks.top * h, at, ticks.bottom * h)?;
        }
    }
    scene.fill_color(tape.paint.under(lighting))?;
    for numeral in tape.numerals {
        if nearest(numeral.value - pose.value, tape.period).abs() > reach {
            continue;
        }
        let at = along(numeral.value) * per_unit;
        scene.text_attributed(
            group,
            at,
            tape.numeral_at * h,
            size,
            Anchor::CENTER,
            numeral.text,
        )?;
    }
    scene.restore()?;
    Ok(())
}

/// `difference` wrapped into `[-period / 2, period / 2)`: how far a
/// value printed round the drum lies from the reading point.
fn nearest(difference: f32, period: f32) -> f32 {
    let half = period * 0.5;
    let shifted = difference + half;
    shifted - libm::floorf(shifted / period) * period - half
}

/// Draws the fixed overlays, in the ascending stack height
/// [`AnalogDialSpec::check`] requires of them.
pub(super) fn draw_overlays(
    spec: &AnalogDialSpec<'_>,
    lighting: Lighting,
    place: &Placement,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let frame = Frame {
        origin: place.center,
        scale: place.scale,
    };
    for overlay in spec.overlays {
        draw_marks(overlay.shapes, lighting, frame, scene)?;
    }
    Ok(())
}
