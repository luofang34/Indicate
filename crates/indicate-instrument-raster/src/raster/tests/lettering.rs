//! Text along an arc, and the face slot as graphics state.

use indicate_instrument_scene::{ArcAlign, ArcRun, ArcSense, FaceSlot};

use super::{WHITE, render_scene, scene};

fn run(sense: ArcSense, angle: f32) -> ArcRun<'static> {
    ArcRun {
        cx: 60.0,
        cy: 60.0,
        radius: 40.0,
        angle,
        sense,
        align: ArcAlign::Center,
        size: 8.0,
        text: "TOP",
    }
}

fn lit(fb: &[u8], x: usize, y: usize, w: usize) -> bool {
    fb[(y * w + x) * 4 + 3] != 0
}

#[test]
fn an_arc_run_paints_on_its_side_of_the_circle_and_nowhere_else() {
    let top = scene(|w| {
        w.fill_color(WHITE).expect("fill");
        w.text_arc(&run(ArcSense::Clockwise, -core::f32::consts::FRAC_PI_2))
            .expect("arc");
    });
    let (res, fb) = render_scene(&top, 120, 120);
    res.expect("renders");
    let ink: std::vec::Vec<(usize, usize)> = (0..120)
        .flat_map(|y| (0..120).map(move |x| (x, y)))
        .filter(|(x, y)| lit(&fb, *x, *y, 120))
        .collect();
    assert!(!ink.is_empty(), "the run paints");
    // A clockwise run at twelve o'clock sits above its baseline circle:
    // between radius 40 and 48 above the centre, near x = 60.
    assert!(ink.iter().all(|(_, y)| (10..=21).contains(y)), "{ink:?}");
    assert!(ink.iter().all(|(x, _)| (40..=80).contains(x)));
}

#[test]
fn a_counter_clockwise_run_at_six_o_clock_hangs_inside_the_circle() {
    let bottom = scene(|w| {
        w.fill_color(WHITE).expect("fill");
        w.text_arc(&run(
            ArcSense::CounterClockwise,
            core::f32::consts::FRAC_PI_2,
        ))
        .expect("arc");
    });
    let (res, fb) = render_scene(&bottom, 120, 120);
    res.expect("renders");
    let rows: std::vec::Vec<usize> = (0..120)
        .filter(|y| (0..120).any(|x| lit(&fb, x, *y, 120)))
        .collect();
    assert!(!rows.is_empty());
    assert!(rows.iter().all(|y| (91..=101).contains(y)), "{rows:?}");
}

#[test]
fn the_face_slot_is_restored_with_the_graphics_state() {
    let mut state = crate::state::RenderState::new(crate::surface::PixelRect {
        left: 0,
        top: 0,
        right: 1,
        bottom: 1,
    });
    state.save().expect("save");
    state.set_font(FaceSlot(3));
    assert_eq!(state.current().font, FaceSlot(3));
    state.restore().expect("restore");
    assert_eq!(state.current().font, FaceSlot::DEFAULT);
}
