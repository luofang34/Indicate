//! Speed and altitude tapes, vertical speed, groundspeed, and the heading scale.
use super::tape::{self, Tape};
use super::*;
use indicate_instrument_state::{
    AltitudeClass, HeadingReference, VSpeedBugs,
    flight::{presentation, speed::SpeedCoordinate},
};
use indicate_instrument_symbology::palette;

pub(super) fn draw(
    data: &PanelData,
    context: &HudContext,
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
    if !data.presentation.unusual
        && matches!(coordinate, SpeedCoordinate::Ias | SpeedCoordinate::Cas)
        && shown_status(speed).shows_value()
    {
        draw_hud_v_speeds(speed.value, speed_index, context, layout, scene)?;
    }
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

/// Draws airframe stall/never-exceed ticks and reference speed bugs (`R`, `X`,
/// `REF`, `G`, `Y`, `A`) along the inboard edge of the HUD airspeed tape.
fn draw_hud_v_speeds(
    ias_kt: f32,
    speed_index: f32,
    context: &HudContext,
    layout: &Layout,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let u = layout.unit;
    let cy = layout.centre[1];
    let half = tape::half_height(layout);
    let per_kt = half / 60.0;
    let min_y = cy - half + 5.0 * u;
    let max_y = cy + half - 5.0 * u;
    let on_tape = |kt: f32| {
        let y = cy - (kt - ias_kt) * per_kt;
        (y >= min_y && y <= max_y).then_some(y)
    };

    if let Some(v) = context.v_speeds.as_ref().filter(|v| v.is_valid()) {
        for limit_kt in [v.vs0_kt, v.vne_kt] {
            if let Some(y) = on_tape(limit_kt) {
                halo_polyline(
                    layout,
                    scene,
                    &[[speed_index, y], [speed_index + 6.0 * u, y]],
                    safety::FAILURE_RED,
                    1.8 * u,
                )?;
            }
        }
    }
    if context.v_speed_bugs.is_empty() {
        return Ok(());
    }
    let (sorted, count) = context.v_speed_bugs.sorted();
    let mut tags = [""; VSpeedBugs::MAX_BUGS];
    let mut exact_y = [0.0f32; VSpeedBugs::MAX_BUGS];
    let mut label_y = [0.0f32; VSpeedBugs::MAX_BUGS];
    let mut vis = 0usize;
    for bug in sorted.iter().take(count) {
        if let Some(y) = on_tape(bug.speed_kt)
            && let (Some(ts), Some(ey), Some(ly)) = (
                tags.get_mut(vis),
                exact_y.get_mut(vis),
                label_y.get_mut(vis),
            )
        {
            (*ts, *ey, *ly) = (bug.tag, y, y);
            vis = vis.wrapping_add(1);
        }
    }
    separate_labels(label_y.get_mut(..vis).unwrap_or_default(), 10.5 * u, min_y);
    let bug_color = layout.ink(palette::CYAN);
    for idx in 0..vis {
        if let (Some(&tag), Some(&yk), Some(&yl)) =
            (tags.get(idx), exact_y.get(idx), label_y.get(idx))
        {
            halo_polyline(
                layout,
                scene,
                &[
                    [speed_index, yk],
                    [speed_index + 3.5 * u, yk],
                    [speed_index + 5.5 * u, yl],
                ],
                palette::CYAN,
                1.3 * u,
            )?;
            scene.fill_color(bug_color)?;
            scene.text(
                speed_index + 6.5 * u,
                yl,
                layout.text(9.0),
                Anchor::MIDDLE_LEFT,
                tag,
            )?;
        }
    }
    Ok(())
}

/// Moves each label up until it clears the one below it by `sep`, so bugs
/// at close speeds keep legible tags. The tick stays at the exact speed.
fn separate_labels(label_y: &mut [f32], sep: f32, min_y: f32) {
    let mut below: Option<f32> = None;
    for y in label_y.iter_mut() {
        if let Some(lo) = below
            && *y > lo - sep
        {
            *y = (lo - sep).max(min_y);
        }
        below = Some(*y);
    }
}
