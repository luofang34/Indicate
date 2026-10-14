//! Speed and altitude tapes, vertical speed, groundspeed, and the heading scale.
use super::tape::{self, Tape};
use super::*;
use indicate_instrument_state::{AltitudeClass, HeadingReference, flight::presentation};

pub(super) fn draw(
    data: &PanelData,
    layout: &Layout,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let u = layout.unit;
    let [cx, cy] = layout.centre;
    let [hw, _] = layout.field;
    let (coordinate, speed, group) = presentation::current_speed(data);
    let speed_index = cx - hw - 12.0 * u;
    Tape {
        label: coordinate.label(),
        signal: speed,
        group,
        index: speed_index,
        side: -1.0,
        minor: 10.0,
        major: 50.0,
        span: 60.0,
        floor: Some(0.0),
    }
    .draw(layout, scene)?;
    let altitude_group = if data.altitude.class == AltitudeClass::LocalRelative {
        GroupId::Kinematics
    } else {
        GroupId::Air
    };
    let label = fmt_label!(16, "{} FT", data.altitude.class.label());
    Tape {
        label: label.as_str(),
        signal: data.altitude.value_ft,
        group: altitude_group,
        index: cx + hw + 18.0 * u,
        side: 1.0,
        minor: 100.0,
        major: 500.0,
        span: 1000.0,
        floor: None,
    }
    .draw(layout, scene)?;
    tape::vertical_speed(data, cx + hw + 6.0 * u, layout, scene)?;
    let below = cy + tape::half_height(layout) + 30.0 * u;
    line(
        scene,
        layout,
        "GS",
        data.gs_kt,
        GroupId::Kinematics,
        [speed_index, below],
        Anchor::MIDDLE_RIGHT,
    )?;
    heading_tape(data, layout, scene)
}

/// A labelled value on one line; failure never removes the label.
fn line(
    scene: &mut SceneWriter<'_>,
    layout: &Layout,
    label: &str,
    signal: Sig<f32>,
    group: GroupId,
    at: [f32; 2],
    anchor: Anchor,
) -> Result<(), PanelDrawError> {
    let status = shown_status(signal);
    scene.fill_color(status_color(layout, status))?;
    let size = layout.text(13.0);
    if status.shows_value() {
        let text = fmt_label!(24, "{} {:.0}", label, signal.value);
        scene.text_attributed(group.to_u8(), at[0], at[1], size, anchor, text.as_str())?;
    } else {
        let text = fmt_label!(24, "{} ---", label);
        scene.text(at[0], at[1], size, anchor, text.as_str())?;
    }
    if status != SignalStatus::Valid {
        scene.text(
            at[0],
            at[1] + 15.0 * layout.unit,
            layout.text(10.0),
            anchor,
            status_label(status),
        )?;
    }
    Ok(())
}

/// A heading scale above the conformal field; it is a scale, not a world cue.
fn heading_tape(
    data: &PanelData,
    layout: &Layout,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let u = layout.unit;
    let [cx, cy] = layout.centre;
    let half = layout.field[0] * 0.62;
    let base = cy - layout.field[1] - 10.0 * u;
    let status = shown_status(data.heading.value_rad);
    let reference = match data.heading.reference {
        HeadingReference::True => "HDG T",
        HeadingReference::SimLocalTrue => "HDG SIM T",
        HeadingReference::Magnetic => "HDG M",
        HeadingReference::Unknown => "HDG REF",
    };
    scene.fill_color(HUD_GREEN)?;
    scene.text(
        cx - half - 8.0 * u,
        base - 8.0 * u,
        layout.text(11.0),
        Anchor::MIDDLE_RIGHT,
        reference,
    )?;
    if !status.shows_value() {
        scene.fill_color(status_color(layout, status))?;
        let text = fmt_label!(24, "HDG --- {}", status_label(status));
        return Ok(scene.text(
            cx,
            base - 8.0 * u,
            layout.text(14.0),
            Anchor::CENTER,
            text.as_str(),
        )?);
    }
    let heading = (data.heading.value_rad.value.to_degrees() % 360.0 + 360.0) % 360.0;
    let color = status_color(layout, status);
    heading_ticks(layout, heading, [cx, base], half, color, scene)?;
    halo_polyline(
        layout,
        scene,
        &[
            [cx - 5.0 * u, base + 8.0 * u],
            [cx, base + 1.0 * u],
            [cx + 5.0 * u, base + 8.0 * u],
        ],
        color,
        1.4 * u,
    )?;
    let value = fmt_label!(8, "{:03.0}", libm::roundf(heading) % 360.0);
    scene.fill_color(color)?;
    scene.text_attributed(
        GroupId::Heading.to_u8(),
        cx,
        base + 20.0 * u,
        layout.text(13.0),
        Anchor::CENTER,
        value.as_str(),
    )?;
    if status != SignalStatus::Valid {
        scene.text(
            cx + 34.0 * u,
            base + 20.0 * u,
            layout.text(10.0),
            Anchor::MIDDLE_LEFT,
            status_label(status),
        )?;
    }
    Ok(())
}

/// The clipped ticks and two-digit labels of the heading scale.
fn heading_ticks(
    layout: &Layout,
    heading: f32,
    [cx, base]: [f32; 2],
    half: f32,
    color: Rgba8,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let u = layout.unit;
    let per_degree = half / 15.0;
    scene.save()?;
    scene.clip_rect(cx - half, base - 34.0 * u, half * 2.0, 36.0 * u)?;
    let first = libm::floorf((heading - 20.0) / 5.0) as i32;
    for index in first..first + 9 {
        let mark = index * 5;
        let x = cx + (mark as f32 - heading) * per_degree;
        let length = if mark % 10 == 0 { 9.0 } else { 5.0 } * u;
        halo_polyline(
            layout,
            scene,
            &[[x, base], [x, base - length]],
            color,
            1.3 * u,
        )?;
        if mark % 10 == 0 {
            let label = fmt_label!(4, "{:02}", mark.rem_euclid(360) / 10);
            scene.fill_color(color)?;
            scene.text_attributed(
                GroupId::Heading.to_u8(),
                x,
                base - 20.0 * u,
                layout.text(12.0),
                Anchor::CENTER,
                label.as_str(),
            )?;
        }
    }
    scene.restore()?;
    Ok(())
}
