#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use indicate_frames::Quat;

use crate::{
    GesturePhase, INTERACTION_FORMAT_VERSION, InteractionError, InteractionFrame,
    InteractionOutcome, InteractionStamp, KeyState, MAX_INTERACTION_BYTES, SpatialEvent, TargetId,
    VIEW_POSE_BYTES, VIEW_POSE_VERSION, ViewLimits, ViewPose3D, apply_event, decode_interaction,
    decode_view_pose, encode_interaction, encode_view_pose, encode_view_pose_slice,
};

fn stamp() -> InteractionStamp {
    InteractionStamp {
        device_id: 7,
        sequence: 42,
        timestamp_us: 1_500_000,
    }
}

#[test]
fn round_trips_every_known_spatial_event() {
    let q = Quat::from_euler(0.1, -0.2, 0.3);
    let events = [
        SpatialEvent::Activate {
            target: TargetId(11),
            point_local: [0.1, -0.05, 0.25],
        },
        SpatialEvent::Drag3D {
            phase: GesturePhase::Changed,
            target: TargetId(3),
            origin_local: [0.0, 0.1, 0.0],
            delta_local: [0.02, -0.01, 0.04],
        },
        SpatialEvent::Transform3D {
            phase: GesturePhase::Began,
            pivot_local: [0.0, 0.0, 0.0],
            translation_local: [0.05, 0.0, -0.05],
            rotation: q,
            scale: 1.25,
        },
        SpatialEvent::RayHover {
            target: TargetId(9),
            origin_local: [0.0, 0.4, 0.8],
            dir_local: [0.0, 0.0, -1.0],
        },
        SpatialEvent::DiscreteKey {
            key_id: 0x0102,
            state: KeyState::Pressed,
        },
        SpatialEvent::ScrubTime { offset_s: -120.5 },
        SpatialEvent::ResetView,
    ];
    let mut buf = [0u8; MAX_INTERACTION_BYTES];
    for event in events {
        let frame = InteractionFrame {
            stamp: stamp(),
            event,
        };
        let n = encode_interaction(&frame, &mut buf).expect("encode");
        let decoded = decode_interaction(&buf[..n]).expect("decode");
        assert_eq!(decoded, frame);
    }
}

#[test]
fn unknown_opcode_skips_payload_cleanly() {
    let mut raw = [0u8; 26];
    raw[0] = INTERACTION_FORMAT_VERSION;
    raw[1] = 0xAA;
    raw[2..4].copy_from_slice(&6u16.to_le_bytes());
    raw[4..8].copy_from_slice(&9u32.to_le_bytes());
    raw[8..12].copy_from_slice(&12u32.to_le_bytes());
    raw[12..20].copy_from_slice(&999u64.to_le_bytes());
    raw[20..26].copy_from_slice(&[1, 2, 3, 4, 5, 6]);
    let decoded = decode_interaction(&raw).expect("forward-compatible unknown");
    assert_eq!(decoded.stamp.device_id, 9);
    assert_eq!(decoded.event, SpatialEvent::Unknown { opcode: 0xAA });
}

#[test]
fn rejects_non_finite_non_unit_and_bad_scale_interactions() {
    let mut buf = [0u8; MAX_INTERACTION_BYTES];
    let bad_nan = InteractionFrame {
        stamp: stamp(),
        event: SpatialEvent::Activate {
            target: TargetId::NONE,
            point_local: [f32::NAN, 0.0, 0.0],
        },
    };
    assert_eq!(
        encode_interaction(&bad_nan, &mut buf),
        Err(InteractionError::NonFinite {
            field: "point_local"
        })
    );

    let bad_ray = InteractionFrame {
        stamp: stamp(),
        event: SpatialEvent::RayHover {
            target: TargetId::NONE,
            origin_local: [0.0; 3],
            dir_local: [0.0, 2.0, 0.0],
        },
    };
    assert_eq!(
        encode_interaction(&bad_ray, &mut buf),
        Err(InteractionError::NonUnit { field: "dir_local" })
    );

    let bad_scale = InteractionFrame {
        stamp: stamp(),
        event: SpatialEvent::Transform3D {
            phase: GesturePhase::Changed,
            pivot_local: [0.0; 3],
            translation_local: [0.0; 3],
            rotation: Quat::IDENTITY,
            scale: 0.0,
        },
    };
    assert_eq!(
        encode_interaction(&bad_scale, &mut buf),
        Err(InteractionError::NonPositiveScale { field: "scale" })
    );
}

#[test]
fn view_pose_exact_wire_layout_and_reserved_checks() {
    let pose = ViewPose3D {
        focus_local: [0.25, -0.5, 0.125],
        orientation: Quat::IDENTITY,
        zoom_scale: 1.5,
        active_target: TargetId(5),
        time_offset_s: 60.0,
        decluttered: true,
    };
    let mut wire = [0u8; VIEW_POSE_BYTES];
    encode_view_pose(&pose, &mut wire).expect("encode");
    assert_eq!(wire[0], VIEW_POSE_VERSION);
    assert_eq!(wire[1], 1);
    assert_eq!(&wire[2..4], &[0, 0]);
    assert_eq!(&wire[28..32], &1.0_f32.to_le_bytes());
    let decoded = decode_view_pose(&wire).expect("decode");
    assert_eq!(decoded, pose);

    let mut slice_out = [0u8; 64];
    assert_eq!(
        encode_view_pose_slice(&pose, &mut slice_out).expect("slice"),
        VIEW_POSE_BYTES
    );

    let mut bad_flags = wire;
    bad_flags[1] = 0x02;
    assert_eq!(
        decode_view_pose(&bad_flags),
        Err(InteractionError::NonZeroReserved)
    );

    let mut bad_reserved = wire;
    bad_reserved[2] = 1;
    assert_eq!(
        decode_view_pose(&bad_reserved),
        Err(InteractionError::NonZeroReserved)
    );
}

#[test]
fn apply_event_clamps_to_view_limits_and_locks_cleanly() {
    let mut pose = ViewPose3D::default();
    let limits = ViewLimits::VOLUMETRIC_DEFAULT;

    let out = apply_event(
        &mut pose,
        &SpatialEvent::Drag3D {
            phase: GesturePhase::Changed,
            target: TargetId(4),
            origin_local: [0.0; 3],
            delta_local: [5.0, -3.0, 0.25],
        },
        &limits,
    )
    .expect("drag");
    assert_eq!(out, InteractionOutcome::ViewUpdated);
    assert_eq!(pose.focus_local, [1.0, -1.0, 0.25]);
    assert_eq!(pose.active_target, TargetId(4));

    let out = apply_event(
        &mut pose,
        &SpatialEvent::Transform3D {
            phase: GesturePhase::Changed,
            pivot_local: [0.0; 3],
            translation_local: [0.0; 3],
            rotation: Quat::from_euler(0.0, 0.0, 0.5),
            scale: 100.0,
        },
        &limits,
    )
    .expect("transform");
    assert_eq!(out, InteractionOutcome::ViewUpdated);
    assert_eq!(pose.zoom_scale, limits.zoom_max);

    let out = apply_event(
        &mut pose,
        &SpatialEvent::ScrubTime { offset_s: -9999.0 },
        &limits,
    )
    .expect("scrub");
    assert_eq!(out, InteractionOutcome::ViewUpdated);
    assert_eq!(pose.time_offset_s, -limits.time_scrub_limit_s);

    let mut locked = ViewPose3D::default();
    assert_eq!(
        apply_event(
            &mut locked,
            &SpatialEvent::ScrubTime { offset_s: 10.0 },
            &ViewLimits::LOCKED,
        )
        .expect("locked scrub"),
        InteractionOutcome::Ignored
    );
    assert_eq!(locked.time_offset_s, 0.0);
}
