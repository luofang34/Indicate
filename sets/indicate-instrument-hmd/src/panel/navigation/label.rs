//! The CDI label line: the source, what positions it, the scale, the
//! flag, and the sequencing state.
use super::*;
use indicate_instrument_scene::nominal_text_width;

/// The run size of the label line.
const SIZE: f32 = 12.0;

/// Draws the label line centered on `x`.
///
/// "NAV1 LOC APR TO", "GPS TERM FROM HOLD", "FMS VIS TERM TO". A
/// flight-management source names its position sensor beside the
/// source word. That word is its own run, because it takes the caution
/// color while the words around it keep the source color.
pub(super) fn draw(
    scene: &mut SceneWriter<'_>,
    nav: &NavData,
    at: [f32; 2],
    color: Rgba8,
) -> Result<(), PanelDrawError> {
    let facility = nav.facility.label();
    let head = fmt_label!(
        12,
        "{}{}{}",
        nav.source.label(),
        if facility.is_empty() { "" } else { " " },
        facility
    );
    let sequencing = nav.sequencing.label();
    let tail = fmt_label!(
        16,
        "{} {}{}{}",
        nav.scale.label(),
        if nav.fromto == NavFromTo::To {
            "TO"
        } else {
            "FROM"
        },
        if sequencing.is_empty() { "" } else { " " },
        sequencing
    );
    let position = nav.position_source.label();
    scene.fill_color(color)?;
    if position.is_empty() {
        let label = fmt_label!(32, "{} {}", head.as_str(), tail.as_str());
        scene.text_attributed(
            GroupId::Nav.to_u8(),
            at[0],
            at[1],
            SIZE,
            Anchor::CENTER,
            label.as_str(),
        )?;
        return Ok(());
    }
    let position_color = if nav.position_is_caution() {
        safety::CAUTION_AMBER
    } else {
        color
    };
    let runs = [
        (head.as_str(), color),
        (position, position_color),
        (tail.as_str(), color),
    ];
    let chars = |text: &str| text.chars().count();
    let total = runs.iter().map(|(text, _)| chars(text) + 1).sum::<usize>() - 1;
    let mut x = at[0] - nominal_text_width(SIZE, total) / 2.0;
    for (text, run_color) in runs {
        scene.fill_color(run_color)?;
        scene.text_attributed(
            GroupId::Nav.to_u8(),
            x,
            at[1],
            SIZE,
            Anchor::MIDDLE_LEFT,
            text,
        )?;
        x += nominal_text_width(SIZE, chars(text) + 1);
    }
    Ok(())
}

/// Draws `DGRD` or `EXCL` in the caution color beside the `CDI` title.
/// A normal position draws nothing.
pub(super) fn draw_integrity(
    scene: &mut SceneWriter<'_>,
    nav: &NavData,
    center: [f32; 2],
) -> Result<(), PanelDrawError> {
    let label = nav.position_integrity.label();
    if label.is_empty() {
        return Ok(());
    }
    scene.fill_color(safety::CAUTION_AMBER)?;
    scene.text_attributed(
        GroupId::Nav.to_u8(),
        center[0] + 30.0,
        center[1] - 25.0,
        SIZE,
        Anchor::MIDDLE_LEFT,
        label,
    )?;
    Ok(())
}
