//! At its design size the PFD paints no text that is cut, struck
//! through, or overlapping: every tape label sits whole inside its
//! strip and clear of the ticks and boxes, and every readout, box
//! label, and annunciation keeps clear of the others and of its box
//! border.
//!
//! Text ink is measured with the nominal metrics of the scene contract
//! (`nominal_text_ink_width`, run size as ink height), which every
//! backend honors. The sweep covers the whole speed and altitude ranges
//! with a light-aircraft V-speed configuration, so a label at any value
//! is checked, not only the ones a fixture happens to show.

#![allow(clippy::expect_used, clippy::panic)]

use std::string::String;
use std::vec::Vec;

use indicate_instrument_scene::{
    Cmd, HAlign, PaintMode, SceneCmds, VAlign, nominal_text_ink_width, nominal_text_width,
};
use indicate_instrument_state::{AltitudeClass, PanelData, Sig, SignalStatus, VSpeedBugs, VSpeeds};

use super::tests::{PfdConfig, flying, render};
use crate::{PANEL_H, PANEL_W};

type Rect = [f32; 4];

#[derive(Debug, Clone)]
struct Run {
    text: String,
    ink: Rect,
    clip: Option<Rect>,
}

#[derive(Debug, Default)]
struct Layout {
    runs: Vec<Run>,
    /// Each box with the width of the border stroked on its edge.
    boxes: Vec<(Rect, f32)>,
    ticks: Vec<(Rect, Option<Rect>)>,
}

#[derive(Clone, Copy, Default)]
struct Context {
    transformed: bool,
    clip: Option<Rect>,
    stroke: f32,
}

fn ink(x: f32, y: f32, size: f32, h: HAlign, v: VAlign, text: &str) -> Rect {
    let chars = text.chars().count();
    let advance = nominal_text_width(size, chars);
    let left = match h {
        HAlign::Left => x,
        HAlign::Center => x - advance / 2.0,
        HAlign::Right => x - advance,
    };
    let top = match v {
        VAlign::Top => y,
        VAlign::Middle => y - size / 2.0,
        VAlign::Bottom | VAlign::Baseline => y - size,
    };
    [
        left,
        top,
        left + nominal_text_ink_width(size, chars),
        top + size,
    ]
}

fn bounds(points: impl Iterator<Item = [f32; 2]>) -> Rect {
    points.fold(
        [f32::MAX, f32::MAX, f32::MIN, f32::MIN],
        |[l, t, r, b], [x, y]| [l.min(x), t.min(y), r.max(x), b.max(y)],
    )
}

fn intersects(a: Rect, b: Rect) -> bool {
    a[0] < b[2] && b[0] < a[2] && a[1] < b[3] && b[1] < a[3]
}

fn inside(a: Rect, b: Rect) -> bool {
    a[0] >= b[0] && a[1] >= b[1] && a[2] <= b[2] && a[3] <= b[3]
}

fn intersect(a: Option<Rect>, b: Rect) -> Rect {
    a.map_or(b, |a| {
        [
            a[0].max(b[0]),
            a[1].max(b[1]),
            a[2].min(b[2]),
            a[3].min(b[3]),
        ]
    })
}

/// The untransformed text runs, boxes, and tick lines of one scene. The
/// attitude symbology rotates with the horizon and is not part of the
/// tape layout, so anything drawn under a transform is left out.
fn layout(scene: &[u8]) -> Layout {
    let mut out = Layout::default();
    let mut now = Context::default();
    let mut saved = Vec::new();
    for command in SceneCmds::new(scene).expect("valid scene") {
        match command.expect("valid command") {
            Cmd::Save => saved.push(now),
            Cmd::Restore => now = saved.pop().expect("balanced"),
            Cmd::Translate { .. } | Cmd::Rotate { .. } => now.transformed = true,
            Cmd::ClipRect { x, y, w, h } => {
                now.clip = Some(intersect(now.clip, [x, y, x + w, y + h]));
            }
            Cmd::Stroke { width, .. } => now.stroke = width,
            _ if now.transformed => {}
            Cmd::Text {
                x,
                y,
                size,
                anchor,
                text,
            } => out.runs.push(Run {
                text: String::from(text),
                ink: ink(x, y, size, anchor.h, anchor.v, text),
                clip: now.clip,
            }),
            Cmd::Rect {
                mode: PaintMode::FillStroke,
                x,
                y,
                w,
                h,
            } => out.boxes.push(([x, y, x + w, y + h], now.stroke)),
            Cmd::Polygon {
                mode: PaintMode::FillStroke,
                points,
            } => out.boxes.push((bounds(points.iter()), now.stroke)),
            Cmd::Line { x1, y1, x2, y2 } => {
                let half = now.stroke / 2.0;
                let line = [x1.min(x2) - half, y1.min(y2) - half, x1.max(x2) + half];
                out.ticks
                    .push(([line[0], line[1], line[2], y1.max(y2) + half], now.clip));
            }
            _ => {}
        }
    }
    out
}

/// A tape strip clip spans most of the panel height; any other clip is
/// a drum window, whose digits roll through it partly visible by design.
fn is_tape_strip(clip: Rect) -> bool {
    clip[3] - clip[1] > 300.0
}

fn check(scene: &[u8], case: &str) {
    let layout = layout(scene);
    let frame = [0.0, 0.0, PANEL_W as f32, PANEL_H as f32];
    let (ladder, free): (Vec<&Run>, Vec<&Run>) = layout
        .runs
        .iter()
        .filter(|run| run.clip.is_none_or(is_tape_strip))
        .partition(|run| run.clip.is_some());
    for label in &ladder {
        let strip = label.clip.expect("ladder labels are clipped");
        assert!(
            inside(label.ink, strip),
            "{case}: tape label {label:?} is cut by its strip"
        );
        for (tick, clip) in &layout.ticks {
            assert!(
                *clip != label.clip || !intersects(label.ink, *tick),
                "{case}: a tick {tick:?} strikes through {label:?}"
            );
        }
        for (boxed, _) in &layout.boxes {
            assert!(
                !intersects(label.ink, *boxed),
                "{case}: tape label {label:?} runs under box {boxed:?}"
            );
        }
    }
    for (i, run) in free.iter().enumerate() {
        assert!(inside(run.ink, frame), "{case}: {run:?} leaves the frame");
        for other in free.iter().skip(i + 1).chain(ladder.iter()) {
            assert!(
                !intersects(run.ink, other.ink),
                "{case}: {run:?} overlaps {other:?}"
            );
        }
        for (boxed, stroke) in &layout.boxes {
            // The border straddles the box edge; text stays inside its
            // inner half.
            let half = stroke / 2.0 - 1e-3;
            let border = [
                boxed[0] + half,
                boxed[1] + half,
                boxed[2] - half,
                boxed[3] - half,
            ];
            let anchored_inside = intersects(run.ink, *boxed) && {
                let [l, t, r, b] = run.ink;
                inside(
                    [(l + r) / 2.0, (t + b) / 2.0, (l + r) / 2.0, (t + b) / 2.0],
                    *boxed,
                )
            };
            if anchored_inside {
                assert!(
                    inside(run.ink, border),
                    "{case}: {run:?} touches the border of its box {boxed:?}"
                );
            } else {
                assert!(
                    !intersects(run.ink, *boxed),
                    "{case}: {run:?} runs into box {boxed:?}"
                );
            }
        }
    }
}

/// A light single's V-speeds and bugs: bands and bugs both draw.
fn light_single() -> PfdConfig {
    PfdConfig {
        v_speeds: Some(VSpeeds {
            vs0_kt: 40.0,
            vs_kt: 48.0,
            vfe_kt: 85.0,
            vno_kt: 129.0,
            vne_kt: 163.0,
        }),
        v_speed_bugs: VSpeedBugs {
            vr_kt: Some(55.0),
            vx_kt: Some(62.0),
            vy_kt: Some(74.0),
            ..VSpeedBugs::default()
        },
        ..PfdConfig::default()
    }
}

const CLASSES: [AltitudeClass; 5] = [
    AltitudeClass::Pressure,
    AltitudeClass::BaroIndicated,
    AltitudeClass::LocalRelative,
    AltitudeClass::GeometricMsl,
    AltitudeClass::Agl,
];

fn state(i: u16) -> PanelData {
    let step = f32::from(i);
    let mut data = flying();
    let ias = step * 0.37 % 210.0;
    data.ias_kt = Sig::with_status(ias, SignalStatus::Valid);
    data.tas_kt = Sig::with_status(ias * 1.1, SignalStatus::Valid);
    data.gs_kt = Sig::with_status(ias * 1.05, SignalStatus::Valid);
    data.altitude.value_ft = Sig::with_status(step * 4.3 - 1200.0, SignalStatus::Valid);
    data.altitude.class = CLASSES[usize::from(i) % CLASSES.len()];
    data.baro_hpa = Sig::with_status(1013.0, SignalStatus::Valid);
    data.altitude.bug_compatible = i % 2 == 0;
    data.selections.altitude_sel_m = (i % 3 != 0).then_some(f32::from(i % 70) * 152.4);
    data
}

#[test]
fn no_pfd_text_is_cut_struck_or_overlapping_across_speed_and_altitude() {
    let cfg = light_single();
    for i in 0..5000u16 {
        check(&render(&state(i), &cfg), &std::format!("state {i}"));
    }
}

#[test]
fn the_reported_frame_reads_clean() {
    // 107 kt, 6543 ft on a standard-pressure tape: the frame where
    // "120" read as "12(" and "STD" sat on "6500".
    let mut data = flying();
    data.ias_kt = Sig::with_status(107.0, SignalStatus::Valid);
    data.tas_kt = Sig::with_status(118.0, SignalStatus::Valid);
    data.gs_kt = Sig::with_status(117.0, SignalStatus::Valid);
    data.altitude.value_ft = Sig::with_status(6543.0, SignalStatus::Valid);
    data.altitude.class = AltitudeClass::Pressure;
    check(&render(&data, &light_single()), "107 kt, 6543 ft STD");
}

#[test]
fn degraded_readouts_keep_their_layout() {
    for status in [
        SignalStatus::Missing,
        SignalStatus::Stale,
        SignalStatus::Failed,
    ] {
        let mut data = state(1234);
        data.ias_kt.status = status;
        data.tas_kt.status = status;
        data.gs_kt.status = status;
        data.altitude.value_ft.status = status;
        data.baro_hpa.status = status;
        check(&render(&data, &light_single()), &std::format!("{status:?}"));
    }
}
