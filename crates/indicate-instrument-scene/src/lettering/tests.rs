#![allow(clippy::expect_used, clippy::panic)]

use std::vec;

use crate::{
    ArcAlign, ArcRun, ArcSense, Cmd, DecodeError, FaceSlot, LayerId, SceneCmds, SceneWriter,
    validate_layers,
};

const RUN: ArcRun<'static> = ArcRun {
    cx: 120.0,
    cy: 120.0,
    radius: 90.0,
    angle: -core::f32::consts::FRAC_PI_2,
    sense: ArcSense::CounterClockwise,
    align: ArcAlign::Center,
    size: 12.0,
    text: "LEFT ENGINE RIGHT",
};

#[test]
fn font_and_text_arc_round_trip_inside_a_layer() {
    let mut buf = vec![0u8; 512];
    let mut w = SceneWriter::new(&mut buf).expect("writer");
    w.begin_layer(LayerId::Tapes).expect("layer");
    w.font(FaceSlot(2)).expect("font");
    w.text_arc_attributed(0, &RUN).expect("arc");
    w.end_layer(LayerId::Tapes).expect("layer");
    let len = w.finish();
    let scene = &buf[..len];
    validate_layers(scene).expect("a well-formed layer");
    let cmds: std::vec::Vec<_> = SceneCmds::new(scene)
        .expect("decodes")
        .map(|c| c.expect("command"))
        .collect();
    assert!(cmds.contains(&Cmd::Font { slot: FaceSlot(2) }));
    assert!(cmds.contains(&Cmd::Attribute { group: 0 }));
    assert!(cmds.contains(&Cmd::TextArc(RUN)));
}

#[test]
fn an_unassigned_sense_or_alignment_byte_is_malformed() {
    for (at, value) in [(20usize, 2u8), (21, 3)] {
        let mut buf = vec![0u8; 256];
        let mut w = SceneWriter::new(&mut buf).expect("writer");
        w.text_arc(&RUN).expect("arc");
        let len = w.finish();
        // Version byte, opcode, two length bytes, then the payload.
        buf[1 + 3 + at] = value;
        let first = SceneCmds::new(&buf[..len])
            .expect("framing")
            .next()
            .expect("one command");
        assert!(
            matches!(first, Err(DecodeError::BadPayload { .. })),
            "{first:?}"
        );
    }
}

#[test]
fn a_clockwise_run_faces_outward_and_a_counter_clockwise_run_inward() {
    let top = ArcRun {
        sense: ArcSense::Clockwise,
        align: ArcAlign::Start,
        ..RUN
    };
    // At twelve o'clock a clockwise run reads left to right, its glyph
    // +x along +x and its glyph up (-y) away from the centre.
    let (x, y, turn) = top.place(10.0, 0.0);
    assert!((x - 120.0).abs() < 1e-4 && (y - 30.0).abs() < 1e-4);
    assert!(turn.abs() < 1e-6);
    let bottom = ArcRun {
        angle: core::f32::consts::FRAC_PI_2,
        align: ArcAlign::Start,
        ..RUN
    };
    let (x, y, turn) = bottom.place(10.0, 0.0);
    assert!((x - 120.0).abs() < 1e-4 && (y - 210.0).abs() < 1e-4);
    assert!(turn.abs() < 1e-6, "reads left to right at six o'clock");
    // Advancing a counter-clockwise run at six o'clock moves it right.
    let (x2, _, _) = bottom.place(10.0, 5.0);
    assert!(x2 > x);
}

#[test]
fn an_over_long_arc_run_is_refused_by_the_writer() {
    let long = "X".repeat(crate::MAX_TEXT_BYTES + 1);
    let run = ArcRun { text: &long, ..RUN };
    let mut buf = vec![0u8; 1024];
    let mut w = SceneWriter::new(&mut buf).expect("writer");
    assert!(w.text_arc(&run).is_err());
    assert!(w.text_arc_attributed(3, &run).is_err());
    assert_eq!(w.finish(), 1, "a refused pair leaves nothing behind");
}
