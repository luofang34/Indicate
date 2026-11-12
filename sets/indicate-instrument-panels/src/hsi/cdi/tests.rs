//! The deviation bar and the TO/FROM triangle follow the stated flag:
//! a flag of `Off` removes the bar rather than centering it, and FROM
//! moves the triangle aft of the rose center.

#![allow(clippy::expect_used, clippy::panic)]

use std::vec::Vec;

use indicate_instrument_scene::{Cmd, PaintMode, SceneCmds, SceneWriter};
use indicate_instrument_state::{
    NavData, NavFromTo, NavResolved, NavScale, NavSource, Sig, SignalStatus,
};

use super::draw_cdi;

/// What one CDI draw fills, in the rotated rose frame: the bar
/// rectangles as `[x, y, w, h]`, and each filled triangle's apex `y`.
struct Fills {
    bars: Vec<[f32; 4]>,
    triangle_apex_y: Vec<f32>,
}

fn draw(fromto: NavFromTo, cdi_dots: f32) -> Fills {
    let nav = NavResolved {
        data: NavData {
            source: NavSource::Gps,
            scale: NavScale::Terminal,
            fromto,
            cdi_dots,
            ..NavData::default()
        },
        status: SignalStatus::Valid,
        course_rose_rad: Sig::with_status(0.0, SignalStatus::Valid),
    };
    let mut buf = std::vec![0u8; 8 * 1024];
    let mut writer = SceneWriter::new(&mut buf).expect("buffer fits");
    draw_cdi(&mut writer, &nav, 0.0).expect("cdi fits buffer");
    let len = writer.finish();
    let mut fills = Fills {
        bars: Vec::new(),
        triangle_apex_y: Vec::new(),
    };
    for command in SceneCmds::new(&buf[..len]).expect("valid scene") {
        match command.expect("valid command") {
            Cmd::Rect {
                mode: PaintMode::Fill,
                x,
                y,
                w,
                h,
            } => fills.bars.push([x, y, w, h]),
            Cmd::Polygon {
                mode: PaintMode::Fill,
                points,
            } => fills
                .triangle_apex_y
                .push(points.get(0).expect("a vertex")[1]),
            _ => {}
        }
    }
    fills
}

#[test]
fn an_off_flag_removes_the_bar_and_keeps_the_course_arrow() {
    let off = draw(NavFromTo::Off, 0.0);
    assert!(off.bars.is_empty(), "no bar without a flag: {:?}", off.bars);
    // The course arrow head is the only fill left: no flag triangle.
    assert_eq!(off.triangle_apex_y, [-90.0]);
}

#[test]
fn to_and_from_draw_the_bar_where_the_course_is() {
    for flag in [NavFromTo::To, NavFromTo::From] {
        let fills = draw(flag, -1.0);
        assert_eq!(fills.bars, [[-37.5 - 2.5, -36.0, 5.0, 72.0]], "{flag:?}");
    }
}

#[test]
fn the_flag_triangle_flips_fore_and_aft_with_the_flag() {
    // The arrow head is at -90; the TO triangle points fore of center
    // and the FROM triangle aft of it.
    assert_eq!(draw(NavFromTo::To, 0.0).triangle_apex_y, [-90.0, -34.0]);
    assert_eq!(draw(NavFromTo::From, 0.0).triangle_apex_y, [-90.0, 34.0]);
}

#[test]
fn a_deviation_past_full_scale_pegs_the_bar_past_the_outer_dot() {
    for (dots, x) in [(9.0, 2.4 * 37.5), (-9.0, -2.4 * 37.5)] {
        let fills = draw(NavFromTo::To, dots);
        assert_eq!(fills.bars.len(), 1);
        assert!((fills.bars[0][0] + 2.5 - x).abs() < 1e-4, "{dots}");
    }
}
