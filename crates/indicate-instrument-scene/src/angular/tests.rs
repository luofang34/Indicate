#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use crate::cmd::HAlign;
use crate::color::Rgba8;
use crate::encode::SceneError;
use crate::layer::LayerError;

use super::{
    AngularCmd, AngularCmds, AngularLayerId, AngularReferenceKind, AngularSceneWriter,
    validate_angular_layers,
};

#[test]
fn round_trips_all_angular_opcodes_and_validates_layers() {
    let mut buf = [0u8; 2048];
    let mut w = AngularSceneWriter::new(&mut buf).expect("writer");
    w.begin_layer(AngularLayerId::WorldCue).expect("begin");
    w.basis(
        AngularReferenceKind::LocalLevel,
        [0.0, 1.0, 0.0],
        [0.0, 0.0, -1.0],
        [1.0, 0.0, 0.0],
    )
    .expect("basis");
    w.fill_color(Rgba8::rgba(90, 255, 130, 255)).expect("fill");
    w.stroke_mrad(Rgba8::rgba(90, 255, 130, 255), 1.5)
        .expect("stroke");
    w.target_tag(7).expect("tag");
    w.ray_line([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]).expect("line");
    w.ray_polyline(&[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]])
        .expect("poly");
    w.great_circle_arc([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], 0.5)
        .expect("arc");
    w.reticle_ring([1.0, 0.0, 0.0], 0.05).expect("ring");
    w.attributed_angular_text(1, [1.0, 0.0, 0.0], 8.0, HAlign::Center, "HDG 090")
        .expect("text");
    w.end_layer(AngularLayerId::WorldCue).expect("end");
    let n = w.finish();

    let report = validate_angular_layers(&buf[..n]).expect("valid");
    assert!(report.contains(AngularLayerId::WorldCue));
    let decoded: std::vec::Vec<_> = AngularCmds::new(&buf[..n])
        .expect("cmds")
        .collect::<Result<_, _>>()
        .expect("all ok");
    assert!(decoded.iter().any(|c| matches!(
        c,
        AngularCmd::AngularText {
            text: "HDG 090",
            ..
        }
    )));
}

#[test]
fn rejects_non_unit_ray_and_out_of_order_layers() {
    let mut buf = [0u8; 512];
    let mut w = AngularSceneWriter::new(&mut buf).expect("writer");
    w.begin_layer(AngularLayerId::Guidance).expect("begin");
    assert_eq!(
        w.ray_line([2.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        Err(SceneError::InvalidGeometry)
    );
    w.end_layer(AngularLayerId::Guidance).expect("end");
    w.begin_layer(AngularLayerId::WorldCue).expect("begin2");
    w.end_layer(AngularLayerId::WorldCue).expect("end2");
    let n = w.finish();
    assert!(matches!(
        validate_angular_layers(&buf[..n]),
        Err(LayerError::OutOfOrder { .. })
    ));
}
