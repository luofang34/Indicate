//! The course deviation indicator: course arrow, deviation bar, scale
//! dots, and TO/FROM triangle.

use indicate_instrument_scene::{
    Anchor, PaintMode, Rgba8, SceneError, SceneWriter, nominal_text_width,
};
use indicate_instrument_state::{GroupId, NavData, NavFromTo, NavResolved, NavSource};

use indicate_instrument_symbology::fixed_str::FixedStr;
use indicate_instrument_symbology::{fmt_label, palette, safety};

/// Two dots of lateral deviation = 75 px (37.5 px/dot, G5 proportions).
const PX_PER_DOT: f32 = 37.5;

pub(crate) fn source_color(source: NavSource) -> Rgba8 {
    // A computed course is magenta and a ground facility is green, so
    // the color answers "whose course" before the label is read.
    if source.is_computed() {
        palette::MAGENTA
    } else {
        palette::GREEN
    }
}

/// Which source drives the needle, and what it is tuned to: `GPS`,
/// `NAV1 VOR`, `NAV2 LOC`, `FMS`. Nav1 and Nav2 wear the same green, so
/// the text is the only thing that tells them apart, and a VOR and a
/// localizer read differently (a localizer has no TO/FROM and pairs
/// with a glideslope). The numerals make this a claimed run — it
/// derives from the nav group, so it claims `GroupId::Nav`, and the
/// claim is honest exactly where the CDI gate is: a withheld or failed
/// nav group never reaches this draw.
///
/// Distinct from [`indicate_instrument_symbology::source_label`], which
/// names the SENSOR feeding a function and colors it by health. This
/// names the source a selection points at, and colors it by source
/// class. One panel paints both, so they never share a name.
pub(crate) fn receiver_text(nav: &NavData) -> Option<FixedStr<12>> {
    let receiver = nav.source.label();
    // No source is gated out by the caller; an unknown one fails the
    // nav group in resolve. Neither has a label to draw.
    if receiver.is_empty() {
        return None;
    }
    let facility = nav.facility.label();
    Some(if facility.is_empty() {
        fmt_label!(12, "{receiver}")
    } else {
        fmt_label!(12, "{receiver} {facility}")
    })
}

/// The label position, lower left beside the rose: clear of the rose's
/// outermost ink and directly above the course box, whose value wears
/// the same source color. The clearance is a test, not a comment, so
/// that growing the rose fails rather than overlapping the label.
pub(crate) const RECEIVER_LABEL_POS: (f32, f32) = (54.0, super::CY + 110.0);

/// The run size the label paints at. Named so the clearance test
/// measures the box the label really occupies.
pub(crate) const RECEIVER_LABEL_SIZE: f32 = 14.0;

/// Draws the receiver label in the source color, claimed from the nav
/// group. Draws nothing for `None`/`Unknown` — the caller's gate already
/// excludes both, and this arm keeps that property local.
///
/// A flight-management source also names the sensor that positions it:
/// `FMS GPS`, `FMS VIS`, `FMS DR`. The position word is a second run,
/// because it takes the caution color while the source word keeps the
/// source color. The pair is centered as one label, so it occupies the
/// box a one-run label of the same length occupies.
pub fn draw_receiver_label(scene: &mut SceneWriter<'_>, nav: &NavData) -> Result<(), SceneError> {
    let Some(text) = receiver_text(nav) else {
        return Ok(());
    };
    let (x, y) = RECEIVER_LABEL_POS;
    let position = nav.position_source.label();
    scene.fill_color(source_color(nav.source))?;
    if position.is_empty() {
        scene.text_attributed(
            GroupId::Nav.to_u8(),
            x,
            y,
            RECEIVER_LABEL_SIZE,
            Anchor::CENTER,
            text.as_str(),
        )?;
        return Ok(());
    }
    let source_chars = text.as_str().chars().count();
    let all_chars = source_chars + 1 + position.chars().count();
    let left = x - nominal_text_width(RECEIVER_LABEL_SIZE, all_chars) / 2.0;
    scene.text_attributed(
        GroupId::Nav.to_u8(),
        left,
        y,
        RECEIVER_LABEL_SIZE,
        Anchor::MIDDLE_LEFT,
        text.as_str(),
    )?;
    if nav.position_is_caution() {
        scene.fill_color(safety::CAUTION_AMBER)?;
    }
    scene.text_attributed(
        GroupId::Nav.to_u8(),
        left + nominal_text_width(RECEIVER_LABEL_SIZE, source_chars + 1),
        y,
        RECEIVER_LABEL_SIZE,
        Anchor::MIDDLE_LEFT,
        position,
    )?;
    Ok(())
}

/// Where the position integrity word sits: lower right under the
/// sequencing annunciation, mirroring the scale label under the
/// receiver label.
pub(crate) const INTEGRITY_LABEL_POS: (f32, f32) = (410.0, super::CY + 128.0);

/// The run size the position integrity word paints at.
pub(crate) const INTEGRITY_LABEL_SIZE: f32 = 14.0;

/// Draws `DGRD` or `EXCL` in the caution color, under the needle's
/// gate.
///
/// The needle is only as good as the position it is computed from. The
/// source declares a degraded position or an excluded sensor, and the
/// word says so beside the needle. A normal position draws nothing.
pub fn draw_integrity_label(scene: &mut SceneWriter<'_>, nav: &NavData) -> Result<(), SceneError> {
    let label = nav.position_integrity.label();
    if label.is_empty() {
        return Ok(());
    }
    scene.fill_color(safety::CAUTION_AMBER)?;
    scene.text_attributed(
        GroupId::Nav.to_u8(),
        INTEGRITY_LABEL_POS.0,
        INTEGRITY_LABEL_POS.1,
        INTEGRITY_LABEL_SIZE,
        Anchor::CENTER,
        label,
    )?;
    Ok(())
}

/// Where the sequencing annunciation sits: lower right, mirroring the
/// receiver label, clear of the rose rim and of the vertical deviation
/// scale above it.
pub(crate) const SEQUENCING_LABEL_POS: (f32, f32) = (410.0, super::CY + 110.0);

/// The run size the sequencing annunciation paints at.
pub(crate) const SEQUENCING_LABEL_SIZE: f32 = 14.0;

/// Draws `HOLD` or `SUSP` in the source color, under the needle's gate.
///
/// A holding pattern flips the TO/FROM flag on every pass over the
/// fix, and a suspension stops the TO fix from advancing. The source
/// declares which, so the flip and the frozen fix do not read as a
/// fault. Sequencing normally draws nothing.
pub fn draw_sequencing_label(scene: &mut SceneWriter<'_>, nav: &NavData) -> Result<(), SceneError> {
    let label = nav.sequencing.label();
    if label.is_empty() {
        return Ok(());
    }
    scene.fill_color(source_color(nav.source))?;
    scene.text_attributed(
        GroupId::Nav.to_u8(),
        SEQUENCING_LABEL_POS.0,
        SEQUENCING_LABEL_POS.1,
        SEQUENCING_LABEL_SIZE,
        Anchor::CENTER,
        label,
    )?;
    Ok(())
}

/// Where the scale label sits: under the receiver label, so the two
/// facts about the needle — which receiver drives it, and what a dot is
/// worth — read together.
pub(crate) const SCALE_LABEL_POS: (f32, f32) = (54.0, super::CY + 128.0);
/// The run size the scale label paints at. Named so the clearance
/// tests measure the box the label really occupies.
pub(crate) const SCALE_LABEL_SIZE: f32 = 14.0;

/// Draws the scale the deflection is on, in the source color, under the
/// same gate as the needle.
///
/// Two dots is two dots on the glass whatever the scale, so the label is
/// what stops one needle position from meaning different distances in
/// different phases. An unknown scale never reaches here: the nav group
/// fails first, and the needle goes with it.
pub fn draw_scale_label(scene: &mut SceneWriter<'_>, nav: &NavResolved) -> Result<(), SceneError> {
    let label = nav.data.scale.label();
    if label.is_empty() {
        return Ok(());
    }
    scene.fill_color(source_color(nav.data.source))?;
    scene.text_attributed(
        GroupId::Nav.to_u8(),
        SCALE_LABEL_POS.0,
        SCALE_LABEL_POS.1,
        SCALE_LABEL_SIZE,
        Anchor::CENTER,
        label,
    )?;
    Ok(())
}

/// Draws the CDI in the rose frame, rotated to the selected course.
pub fn draw_cdi(
    scene: &mut SceneWriter<'_>,
    nav: &NavResolved,
    heading_rad: f32,
) -> Result<(), SceneError> {
    let color = source_color(nav.data.source);
    scene.save()?;
    scene.translate(super::CX, super::CY)?;
    scene.rotate(nav.course_rose_rad.value - heading_rad)?;

    // Course arrow: head, fore shaft, aft shaft.
    scene.fill_color(color)?;
    scene.stroke(color, 4.0)?;
    scene.polygon(
        PaintMode::Fill,
        &[[0.0, -90.0], [-10.0, -70.0], [10.0, -70.0]],
    )?;
    scene.line(0.0, -70.0, 0.0, -38.0)?;
    scene.line(0.0, 38.0, 0.0, 90.0)?;

    // Scale dots on the perpendicular.
    scene.stroke(palette::GREY, 2.0)?;
    for dx in [-2.0f32, -1.0, 1.0, 2.0] {
        scene.circle(PaintMode::Stroke, dx * PX_PER_DOT, 0.0, 4.0)?;
    }

    // Deviation bar: where the course line *is*, relative to the aircraft.
    // Only a TO or FROM resolution has a deviation to show. Without one
    // the bar is removed, as the HUD and HMD remove theirs: a centered
    // bar would read as on course. The course arrow stays, because the
    // course is still valid.
    scene.fill_color(color)?;
    if matches!(nav.data.fromto, NavFromTo::To | NavFromTo::From) {
        let dx = (nav.data.cdi_dots.clamp(-2.4, 2.4)) * PX_PER_DOT;
        scene.rect(PaintMode::Fill, dx - 2.5, -36.0, 5.0, 72.0)?;
    }

    // TO/FROM triangle beside the fore shaft.
    match nav.data.fromto {
        NavFromTo::To => {
            scene.polygon(
                PaintMode::Fill,
                &[[0.0, -34.0], [-8.0, -18.0], [8.0, -18.0]],
            )?;
        }
        NavFromTo::From => {
            scene.polygon(PaintMode::Fill, &[[0.0, 34.0], [-8.0, 18.0], [8.0, 18.0]])?;
        }
        // Unknown resolution never reaches here (the nav group fails
        // before drawing); the arm is the exhaustiveness fail-safe and
        // draws no flag rather than inventing one.
        NavFromTo::Off | NavFromTo::Unknown => {}
    }
    scene.restore()?;
    Ok(())
}

#[cfg(test)]
mod tests;
