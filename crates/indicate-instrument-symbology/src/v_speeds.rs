//! Shared V-speed band and reference-bug symbology for both linear speed tapes
//! (PFD, standby tape, EFB strip) and circular/dial airspeed indicators.

use indicate_instrument_scene::{Anchor, PaintMode, Rgba8, SceneError, SceneWriter};
use indicate_instrument_state::{VSpeedBugs, VSpeeds};

use crate::{palette, safety};

/// Geometry of a vertical linear airspeed tape.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinearVSpeedScale {
    /// X coordinate of the tape's tick/index edge (`90.0` on the standard PFD).
    pub edge_x: f32,
    /// Y coordinate of the current-airspeed lubber line (`180.0` on the PFD).
    pub center_y: f32,
    /// Top clip Y of the speed strip.
    pub top_y: f32,
    /// Bottom clip Y of the speed strip.
    pub bottom_y: f32,
    /// Vertical pixels per knot (`> 0`).
    pub px_per_kt: f32,
    /// Left X of the primary (`< Vs0`, `Vs..Vno`, `Vno..Vne`, `>= Vne`) color strip.
    pub outer_band_x: f32,
    /// Left X of the inner (`Vs0..Vfe`) white flap-operating strip.
    pub inner_band_x: f32,
    /// Width of each color strip in pixels.
    pub band_width: f32,
    /// `+1.0` to place bug tabs to the right of `edge_x`, `-1.0` to the left.
    pub bug_side: f32,
}

/// Geometry of a circular / round-dial airspeed indicator.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DialVSpeedScale {
    /// Dial center X.
    pub center_x: f32,
    /// Dial center Y.
    pub center_y: f32,
    /// Outer arc radius in design-frame pixels.
    pub radius: f32,
    /// Arc stroke thickness in pixels.
    pub band_stroke_px: f32,
    /// Minimum airspeed on the dial scale (knots).
    pub min_kt: f32,
    /// Maximum airspeed on the dial scale (knots).
    pub max_kt: f32,
    /// Angle (radians, `0` = `+X`, clockwise positive) at `min_kt`.
    pub zero_angle_rad: f32,
    /// Total clockwise angular sweep (radians) from `min_kt` to `max_kt`.
    pub sweep_rad: f32,
}

impl DialVSpeedScale {
    /// Maps `speed_kt` onto a clamped dial angle in radians.
    #[must_use]
    pub fn angle_for_kt(&self, speed_kt: f32) -> f32 {
        let span = (self.max_kt - self.min_kt).max(1.0);
        let frac = ((speed_kt - self.min_kt) / span).clamp(0.0, 1.0);
        self.zero_angle_rad + frac * self.sweep_rad
    }
}

/// Draws airspeed operating-range color strips (`< Vs0` red stall bar, `Vs0..Vfe`
/// white flap strip, `Vs..Vno` green normal strip, `Vno..Vne` caution strip, and
/// `>= Vne` red overspeed bar) on a vertical speed tape.
pub fn draw_linear_bands(
    scene: &mut SceneWriter<'_>,
    ias_kt: f32,
    speeds: &VSpeeds,
    scale: &LinearVSpeedScale,
) -> Result<(), SceneError> {
    if !ias_kt.is_finite() || !speeds.is_valid() {
        return Ok(());
    }
    let primary: [(f32, f32, Rgba8); 4] = [
        (0.0, speeds.vs0_kt, safety::FAILURE_RED),
        (speeds.vs_kt, speeds.vno_kt, palette::BAND_GREEN),
        (speeds.vno_kt, speeds.vne_kt, safety::BAND_CAUTION),
        (speeds.vne_kt, speeds.vne_kt + 1000.0, safety::FAILURE_RED),
    ];
    for (lo, hi, color) in primary {
        band_rect(
            scene,
            ias_kt,
            lo,
            hi,
            scale.outer_band_x,
            scale.band_width,
            color,
            scale,
        )?;
    }
    band_rect(
        scene,
        ias_kt,
        speeds.vs0_kt,
        speeds.vfe_kt,
        scale.inner_band_x,
        scale.band_width,
        palette::WHITE,
        scale,
    )
}

#[allow(clippy::too_many_arguments)]
fn band_rect(
    scene: &mut SceneWriter<'_>,
    ias_kt: f32,
    lo_kt: f32,
    hi_kt: f32,
    x: f32,
    w: f32,
    color: Rgba8,
    scale: &LinearVSpeedScale,
) -> Result<(), SceneError> {
    if hi_kt <= lo_kt {
        return Ok(());
    }
    let y_top = (scale.center_y - (hi_kt - ias_kt) * scale.px_per_kt).max(scale.top_y);
    let y_bot = (scale.center_y - (lo_kt - ias_kt) * scale.px_per_kt).min(scale.bottom_y);
    if y_bot > y_top {
        scene.fill_color(color)?;
        scene.rect(PaintMode::Fill, x, y_top, w, y_bot - y_top)?;
    }
    Ok(())
}

/// Draws POH/AFM V-speed reference bug ticks and de-overlapped cyan callout tabs
/// (`R`, `X`, `REF`, `G`, `Y`, `A`) along a vertical speed tape.
pub fn draw_linear_bugs(
    scene: &mut SceneWriter<'_>,
    ias_kt: f32,
    bugs: &VSpeedBugs,
    scale: &LinearVSpeedScale,
) -> Result<(), SceneError> {
    if !ias_kt.is_finite() || bugs.is_empty() {
        return Ok(());
    }
    let (sorted, count) = bugs.sorted();
    let min_y = scale.top_y + 6.0;
    let max_y = scale.bottom_y - 6.0;
    if max_y <= min_y {
        return Ok(());
    }

    let mut tags = [""; VSpeedBugs::MAX_BUGS];
    let mut exact_y = [0.0f32; VSpeedBugs::MAX_BUGS];
    let mut box_y = [0.0f32; VSpeedBugs::MAX_BUGS];
    let mut vis = 0usize;
    for idx in 0..count {
        if let Some(bug) = sorted.get(idx) {
            let y = scale.center_y - (bug.speed_kt - ias_kt) * scale.px_per_kt;
            if y >= min_y
                && y <= max_y
                && let (Some(t_slot), Some(ey_slot), Some(by_slot)) =
                    (tags.get_mut(vis), exact_y.get_mut(vis), box_y.get_mut(vis))
            {
                *t_slot = bug.tag;
                *ey_slot = y;
                *by_slot = y;
                vis = vis.wrapping_add(1);
            }
        }
    }
    if vis == 0 {
        return Ok(());
    }
    // Note: sorted ascending in speed means descending in `y` (`box_y[0]` is lowest on screen).
    deoverlap_descending(&mut box_y, vis, min_y, max_y, 12.0);

    let side = if scale.bug_side < 0.0 { -1.0 } else { 1.0 };
    for idx in 0..vis {
        let (Some(&tag), Some(&yk), Some(&yb)) = (tags.get(idx), exact_y.get(idx), box_y.get(idx))
        else {
            continue;
        };
        let box_w = if tag.len() >= 3 { 24.0 } else { 14.0 };
        let box_h = 11.0;
        let gap = 4.5;
        let near_x = scale.edge_x + side * gap;
        let rect_x = if side > 0.0 { near_x } else { near_x - box_w };

        scene.stroke(palette::CYAN, 1.8)?;
        scene.line(scale.edge_x, yk, scale.edge_x + side * 3.0, yk)?;
        scene.line(scale.edge_x + side * 3.0, yk, near_x, yb)?;
        scene.fill_color(palette::BOX_BG)?;
        scene.stroke(palette::CYAN, 1.2)?;
        scene.rect(
            PaintMode::FillStroke,
            rect_x,
            yb - box_h * 0.5,
            box_w,
            box_h,
        )?;
        scene.fill_color(palette::CYAN)?;
        scene.text(rect_x + box_w * 0.5, yb, 9.0, Anchor::CENTER, tag)?;
    }
    Ok(())
}

fn deoverlap_descending(
    ys: &mut [f32; VSpeedBugs::MAX_BUGS],
    n: usize,
    min_y: f32,
    max_y: f32,
    sep: f32,
) {
    // Pass 1 (bottom-to-top, decreasing y): push upper boxes upward if too close.
    let mut i = 1usize;
    while i < n {
        let Some(lower) = ys.get(i - 1).copied() else {
            break;
        };
        if let Some(upper) = ys.get_mut(i)
            && *upper > lower - sep
        {
            *upper = (lower - sep).max(min_y);
        }
        i = i.wrapping_add(1);
    }
    // Pass 2 (top-to-bottom, increasing y): push lower boxes downward if clamped at top.
    let mut k = n;
    while k > 1 {
        k -= 1;
        let Some(upper) = ys.get(k).copied() else {
            continue;
        };
        if let Some(lower) = ys.get_mut(k - 1)
            && *lower < upper + sep
        {
            *lower = (upper + sep).min(max_y);
        }
    }
}

/// Draws circular airspeed operating-range arcs (`Vs0..Vfe` white inner arc,
/// `Vs..Vno` green arc, `Vno..Vne` caution yellow arc, and radial `Vne` red line)
/// on a round/dial airspeed indicator.
pub fn draw_dial_arcs(
    scene: &mut SceneWriter<'_>,
    speeds: &VSpeeds,
    dial: &DialVSpeedScale,
) -> Result<(), SceneError> {
    if !speeds.is_valid() || dial.radius <= dial.band_stroke_px {
        return Ok(());
    }
    let arc_seg = |scene: &mut SceneWriter<'_>,
                   lo: f32,
                   hi: f32,
                   r: f32,
                   color: Rgba8|
     -> Result<(), SceneError> {
        let a0 = dial.angle_for_kt(lo);
        let a1 = dial.angle_for_kt(hi);
        let sweep = a1 - a0;
        if libm::fabsf(sweep) > 1e-4 {
            scene.stroke(color, dial.band_stroke_px)?;
            scene.arc(dial.center_x, dial.center_y, r, a0, sweep)?;
        }
        Ok(())
    };
    arc_seg(
        scene,
        speeds.vs_kt,
        speeds.vno_kt,
        dial.radius,
        palette::BAND_GREEN,
    )?;
    arc_seg(
        scene,
        speeds.vno_kt,
        speeds.vne_kt,
        dial.radius,
        safety::BAND_CAUTION,
    )?;
    arc_seg(
        scene,
        speeds.vs0_kt,
        speeds.vfe_kt,
        dial.radius - dial.band_stroke_px,
        palette::WHITE,
    )?;

    let a_vne = dial.angle_for_kt(speeds.vne_kt);
    let (s, c) = libm::sincosf(a_vne);
    let r_in = dial.radius - dial.band_stroke_px * 2.2;
    let r_out = dial.radius + dial.band_stroke_px * 1.2;
    scene.stroke(safety::FAILURE_RED, 2.5)?;
    scene.line(
        dial.center_x + r_in * c,
        dial.center_y + r_in * s,
        dial.center_x + r_out * c,
        dial.center_y + r_out * s,
    )
}

/// Draws radial cyan V-speed bug ticks and labels (`R`, `X`, `REF`, `G`, `Y`, `A`)
/// around the perimeter of a round/dial airspeed indicator.
pub fn draw_dial_bugs(
    scene: &mut SceneWriter<'_>,
    bugs: &VSpeedBugs,
    dial: &DialVSpeedScale,
) -> Result<(), SceneError> {
    if bugs.is_empty() || dial.radius <= 8.0 {
        return Ok(());
    }
    let (sorted, count) = bugs.sorted();
    let r_in = dial.radius + dial.band_stroke_px * 0.6;
    let r_out = r_in + 6.0;
    let r_txt = r_out + 7.0;
    for idx in 0..count {
        let Some(bug) = sorted.get(idx) else {
            continue;
        };
        if bug.speed_kt < dial.min_kt || bug.speed_kt > dial.max_kt {
            continue;
        }
        let a = dial.angle_for_kt(bug.speed_kt);
        let (s, c) = libm::sincosf(a);
        scene.stroke(palette::CYAN, 1.8)?;
        scene.line(
            dial.center_x + r_in * c,
            dial.center_y + r_in * s,
            dial.center_x + r_out * c,
            dial.center_y + r_out * s,
        )?;
        scene.fill_color(palette::CYAN)?;
        scene.text(
            dial.center_x + r_txt * c,
            dial.center_y + r_txt * s,
            8.5,
            Anchor::CENTER,
            bug.tag,
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use indicate_instrument_scene::{Cmd, SceneCmds};

    #[test]
    fn linear_and_dial_v_speeds_emit_balanced_valid_scenes() {
        let speeds = VSpeeds {
            vs0_kt: 40.0,
            vs_kt: 48.0,
            vfe_kt: 85.0,
            vno_kt: 129.0,
            vne_kt: 163.0,
        };
        let bugs = VSpeedBugs {
            vr_kt: Some(55.0),
            vx_kt: Some(62.0),
            vy_kt: Some(74.0),
            vg_kt: Some(68.0),
            va_kt: Some(103.0),
            vref_kt: Some(61.0),
        };
        let mut buf = [0u8; 8192];
        let mut w = SceneWriter::new(&mut buf).expect("scene");
        let linear = LinearVSpeedScale {
            edge_x: 90.0,
            center_y: 180.0,
            top_y: 25.0,
            bottom_y: 335.0,
            px_per_kt: 7.2,
            outer_band_x: 86.0,
            inner_band_x: 82.0,
            band_width: 4.0,
            bug_side: 1.0,
        };
        draw_linear_bands(&mut w, 68.0, &speeds, &linear).expect("linear bands");
        draw_linear_bugs(&mut w, 68.0, &bugs, &linear).expect("linear bugs");

        let dial = DialVSpeedScale {
            center_x: 120.0,
            center_y: 120.0,
            radius: 90.0,
            band_stroke_px: 5.0,
            min_kt: 30.0,
            max_kt: 180.0,
            zero_angle_rad: -1.3,
            sweep_rad: 5.2,
        };
        draw_dial_arcs(&mut w, &speeds, &dial).expect("dial arcs");
        draw_dial_bugs(&mut w, &bugs, &dial).expect("dial bugs");
        let used = w.finish();

        let mut tags = std::vec::Vec::new();
        let mut arcs = 0usize;
        for cmd in SceneCmds::new(&buf[..used]).expect("decodes") {
            match cmd.expect("cmd") {
                Cmd::Text { text, .. } => tags.push(std::string::String::from(text)),
                Cmd::Arc { .. } => arcs += 1,
                _ => {}
            }
        }
        assert_eq!(arcs, 3);
        for expected in ["R", "REF", "X", "G", "Y"] {
            assert!(
                tags.iter().any(|t| t == expected),
                "missing {expected} in {tags:?}"
            );
        }
    }
}
