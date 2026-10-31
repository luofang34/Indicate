#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use indicate_frames::Quat;

use crate::cmd::{HAlign, PaintMode};
use crate::color::Rgba8;
use crate::decode::DecodeError;
use crate::encode::SceneError;
use crate::layer::LayerError;

use super::{
    Extent3D, HandleKind, VOLUME_SCENE_VERSION, VolumeBandId, VolumeCmd, VolumeCmds,
    VolumeSceneWriter, VolumeUnderlayKind, validate_volume_bands,
};

fn write_upper_bands(w: &mut VolumeSceneWriter<'_>) {
    w.begin_band(VolumeBandId::Trajectories3D).expect("traj");
    w.polyline_3d(&[[-0.1, 0.0, -0.1], [0.0, 0.05, 0.0], [0.1, 0.08, 0.1]])
        .expect("poly");
    w.end_band(VolumeBandId::Trajectories3D).expect("end");

    w.begin_band(VolumeBandId::Symbols3D).expect("sym");
    w.sphere(PaintMode::Fill, [0.0, 0.05, 0.0], 0.015)
        .expect("sphere");
    w.end_band(VolumeBandId::Symbols3D).expect("end");

    w.begin_band(VolumeBandId::Billboards).expect("bill");
    w.attributed_billboard_text(1, [0.0, 0.12, 0.0], 0.018, HAlign::Center, "ALT 4500")
        .expect("text");
    w.end_band(VolumeBandId::Billboards).expect("end");

    w.begin_band(VolumeBandId::Handles).expect("handles");
    w.target_handle(1, HandleKind::Waypoint, [0.1, 0.08, 0.1], 0.025)
        .expect("handle");
    w.end_band(VolumeBandId::Handles).expect("end");

    w.begin_band(VolumeBandId::Annunciation).expect("ann");
    w.billboard_text(
        [0.0, 0.2, 0.0],
        0.016,
        HAlign::Center,
        "SIM / NOT FOR FLIGHT",
    )
    .expect("sim");
    w.end_band(VolumeBandId::Annunciation).expect("end");

    w.begin_band(VolumeBandId::Failure).expect("fail");
    w.billboard_text([0.0, 0.0, 0.0], 0.02, HAlign::Center, "ATT FAIL")
        .expect("fail");
    w.end_band(VolumeBandId::Failure).expect("end");
}

#[test]
fn round_trips_every_volume_opcode_and_validates_all_nine_bands() {
    let mut buf = [0u8; 4096];
    let mut w = VolumeSceneWriter::new(&mut buf).expect("writer");

    w.begin_band(VolumeBandId::ReferenceGrid).expect("grid");
    w.stroke_3d(Rgba8::rgba(120, 140, 160, 255), 0.002)
        .expect("stroke");
    w.line_3d([-0.2, 0.0, 0.0], [0.2, 0.0, 0.0]).expect("line");
    w.ring_3d([0.0, 0.0, 0.0], [0.0, 1.0, 0.0], 0.2)
        .expect("ring");
    w.end_band(VolumeBandId::ReferenceGrid).expect("end");

    w.begin_band(VolumeBandId::SurfaceUnderlay)
        .expect("underlay");
    w.underlay_anchor_3d(
        VolumeUnderlayKind::TerrainPatch,
        [0.0, -0.1, 0.0],
        [0.5, 0.2, 0.5],
        Quat::IDENTITY,
        10_000.0,
    )
    .expect("anchor");
    w.heightfield_grid(2, 2, [-0.1, -0.1, -0.1], 0.2, 0.2, &[0.0, 0.02, 0.01, 0.03])
        .expect("grid");
    w.end_band(VolumeBandId::SurfaceUnderlay).expect("end");

    w.begin_band(VolumeBandId::Volumes3D).expect("vol");
    w.save().expect("save");
    w.translate_3d([0.05, 0.0, 0.0]).expect("trans");
    w.rotate_quat(Quat::from_euler(0.0, 0.0, 0.2)).expect("rot");
    w.clip_box_3d([-0.25, -0.2, -0.25], [0.25, 0.2, 0.25])
        .expect("clip");
    w.fill_color(Rgba8::rgba(60, 180, 255, 128)).expect("fill");
    w.box_3d(PaintMode::FillStroke, [-0.05; 3], [0.05; 3])
        .expect("box");
    w.cylinder_3d(PaintMode::Fill, [0.0, -0.05, 0.0], [0.0, 0.05, 0.0], 0.02)
        .expect("cyl");
    w.restore().expect("restore");
    w.end_band(VolumeBandId::Volumes3D).expect("end");

    write_upper_bands(&mut w);

    let n = w.finish();
    let report = validate_volume_bands(&buf[..n]).expect("valid bands");
    assert_eq!(report.present, 0x01FF);

    let ext = Extent3D::new(0.6, 0.45, 0.6);
    assert!(ext.is_valid());
    assert!(ext.contains_point([0.1, 0.08, 0.1]));
    assert!(!ext.contains_point([0.5, 0.0, 0.0]));
}

#[test]
fn rejects_malformed_volume_commands_and_forward_skips_unknown() {
    let mut buf = [0u8; 256];
    let mut w = VolumeSceneWriter::new(&mut buf).expect("writer");
    assert_eq!(
        w.target_handle(0, HandleKind::Point, [0.0; 3], 0.02),
        Err(SceneError::InvalidGeometry)
    );
    assert_eq!(
        w.box_3d(PaintMode::Fill, [1.0, 0.0, 0.0], [0.0, 0.0, 0.0]),
        Err(SceneError::InvalidGeometry)
    );

    let mut raw = [0u8; 16];
    raw[0] = VOLUME_SCENE_VERSION;
    raw[1] = 0x50;
    raw[2..4].copy_from_slice(&1u16.to_le_bytes());
    raw[4] = 0;
    raw[5] = 0x01;
    raw[6..8].copy_from_slice(&0u16.to_le_bytes());
    raw[8] = 0x4F;
    raw[9..11].copy_from_slice(&0u16.to_le_bytes());
    raw[11] = 0x02;
    raw[12..14].copy_from_slice(&0u16.to_le_bytes());
    raw[14] = 0x51;
    raw[15] = 0; // truncated EndBand length!
    assert_eq!(
        VolumeCmds::new(&raw)
            .expect("ver")
            .collect::<Result<std::vec::Vec<_>, _>>(),
        Err(DecodeError::Truncated)
    );

    let mut ok_unknown = [0u8; 17];
    ok_unknown[..14].copy_from_slice(&raw[..14]);
    ok_unknown[14] = 0x51;
    ok_unknown[15..17].copy_from_slice(&1u16.to_le_bytes());
    let mut with_band = [0u8; 18];
    with_band[..17].copy_from_slice(&ok_unknown);
    with_band[17] = 0;
    let report = validate_volume_bands(&with_band).expect("unknown counted");
    assert_eq!(report.unknown, 1);

    let without_band = [VOLUME_SCENE_VERSION, 0x10, 4, 0, 1, 2, 3, 4];
    assert_eq!(
        validate_volume_bands(&without_band),
        Err(LayerError::CommandOutsideLayer)
    );
    let _ = VolumeCmd::Save;
}
