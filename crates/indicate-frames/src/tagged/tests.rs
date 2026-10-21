use super::{Attitude, Position, Tagged, transform_attitude, transform_position, transform_vector};
use crate::error::FrameError;
use crate::fixtures::{clock, frame};
use crate::frame::FrameKind;
use crate::relative_attitude;
use crate::rotation::Quat;
use crate::time::{ClockDomain, TimeScale, Timestamp};
use crate::transform::FrameTransform;

fn at(nanos: u64) -> Timestamp {
    Timestamp {
        clock: clock(ClockDomain::Simulation),
        scale: TimeScale::Monotonic,
        nanos,
    }
}

fn yaw(half_turns_of_pi: f64) -> Quat {
    let h = half_turns_of_pi * core::f64::consts::PI / 2.0;
    Quat {
        w: h.cos() as f32,
        x: 0.0,
        y: 0.0,
        z: h.sin() as f32,
    }
}

/// Provenance stand-in: the frames layer must carry it bit-for-bit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Meta {
    source: u64,
    acquired_ns: u64,
    integrity: u8,
    authorized: bool,
    coherent: bool,
}

const META: Meta = Meta {
    source: 0xA1B2,
    acquired_ns: 987_654_321,
    integrity: 3,
    authorized: true,
    coherent: true,
};

fn ned_to_ecef() -> FrameTransform {
    FrameTransform::new(
        frame(FrameKind::Ned),
        frame(FrameKind::Ecef),
        at(7),
        yaw(0.5),
        [1.0e6, 2.0e6, -3.0e6],
    )
    .expect("valid")
}

#[test]
fn provenance_rides_every_accepted_transform_untouched() {
    let p: Position<Meta> = Tagged {
        frame: frame(FrameKind::Ned),
        at: at(7),
        meta: META,
        value: [100.0, -200.0, 300.0],
    };
    let out = transform_position(&p, &ned_to_ecef()).expect("transforms");
    assert_eq!(out.meta, META);
    assert_eq!(out.at, p.at);
    assert_eq!(out.frame, frame(FrameKind::Ecef));

    let v = Tagged {
        value: [1.0, 0.0, 0.0],
        ..p
    };
    assert_eq!(transform_vector(&v, &ned_to_ecef()).expect("ok").meta, META);

    let a: Attitude<Meta> = Attitude {
        subject: frame(FrameKind::Body),
        frame: frame(FrameKind::Ned),
        at: at(7),
        meta: META,
        value: yaw(0.25),
    };
    assert_eq!(
        transform_attitude(&a, &ned_to_ecef()).expect("ok").meta,
        META
    );
}

#[test]
fn wrong_frame_and_wrong_epoch_are_typed_refusals() {
    let p: Position<Meta> = Tagged {
        frame: frame(FrameKind::Eci),
        at: at(7),
        meta: META,
        value: [0.0; 3],
    };
    assert_eq!(
        transform_position(&p, &ned_to_ecef()),
        Err(FrameError::FrameMismatch {
            expected: frame(FrameKind::Ned),
            found: frame(FrameKind::Eci),
        })
    );
    let stale = Tagged {
        at: at(6),
        frame: frame(FrameKind::Ned),
        ..p
    };
    assert!(matches!(
        transform_position(&stale, &ned_to_ecef()),
        Err(FrameError::TimestampMismatch { .. })
    ));
}

#[test]
fn same_orientation_projects_distinctly_per_reference() {
    // One physical orientation, expressed against ECI; supplied ECI→LVLH
    // and ECI→target transforms give deterministic but different
    // projections — none of which replaces the canonical quaternion.
    let attitude: Attitude<Meta> = Attitude {
        subject: frame(FrameKind::Body),
        frame: frame(FrameKind::Eci),
        at: at(7),
        meta: META,
        value: yaw(0.3),
    };
    let to_lvlh = FrameTransform::new(
        frame(FrameKind::Eci),
        frame(FrameKind::Lvlh),
        at(7),
        yaw(0.5),
        [0.0; 3],
    )
    .expect("valid");
    let to_target = FrameTransform::new(
        frame(FrameKind::Eci),
        frame(FrameKind::TargetRelative),
        at(7),
        yaw(-0.5),
        [0.0; 3],
    )
    .expect("valid");
    let lvlh = transform_attitude(&attitude, &to_lvlh).expect("lvlh");
    let target = transform_attitude(&attitude, &to_target).expect("target");
    assert_eq!(lvlh.frame, frame(FrameKind::Lvlh));
    assert_eq!(target.frame, frame(FrameKind::TargetRelative));
    assert_ne!(lvlh.value, target.value, "distinct projections");
    assert_ne!(lvlh.value, attitude.value, "LVLH differs from inertial");
    // Determinism: repeating the projection is bit-identical.
    assert_eq!(
        transform_attitude(&attitude, &to_lvlh)
            .expect("again")
            .value,
        lvlh.value
    );
}

#[test]
fn attitude_is_meaningful_without_any_horizon() {
    // A pure inertial attitude never touches NED or gravity; the
    // canonical state stays a quaternion and q / -q describe the same
    // physical orientation through the rotation kernel.
    let q = yaw(0.37);
    let neg = Quat {
        w: -q.w,
        x: -q.x,
        y: -q.y,
        z: -q.z,
    };
    let v = [1.0, 2.0, 3.0];
    let a = q.rotate(v);
    let b = neg.rotate(v);
    for (x, y) in a.iter().zip(b) {
        assert!((x - y).abs() < 1e-9, "q and -q rotate identically");
    }
}

#[test]
fn fixed_point_boundary_fixtures_stay_exact() {
    // Exact-norm quaternions (squared component sums of 1.0 in f32)
    // survive validation without renormalization drift.
    for q in [
        Quat {
            w: 1.0,
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
        Quat {
            w: 0.0,
            x: 1.0,
            y: 0.0,
            z: 0.0,
        },
        Quat {
            w: 0.5,
            x: 0.5,
            y: 0.5,
            z: 0.5,
        },
        Quat {
            w: 0.0,
            x: 0.0,
            y: 0.0,
            z: -1.0,
        },
    ] {
        let t = FrameTransform::new(
            frame(FrameKind::Body),
            frame(FrameKind::Ned),
            at(1),
            q,
            [0.0; 3],
        )
        .expect("exact-norm accepted");
        assert_eq!(t.rotation(), q, "no drift for exact-norm input");
    }
}

#[test]
fn an_attitude_that_is_not_a_rotation_is_refused_rather_than_composed() {
    let transform = FrameTransform::new(
        frame(FrameKind::Body),
        frame(FrameKind::Ned),
        at(1),
        yaw(0.0),
        [0.0; 3],
    )
    .expect("identity transform");
    for value in [
        Quat {
            w: f32::NAN,
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
        Quat {
            w: 0.0,
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
        Quat {
            w: 2.0,
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
    ] {
        let attitude: Attitude<()> = Attitude {
            subject: frame(FrameKind::Body),
            frame: frame(FrameKind::Body),
            at: at(1),
            meta: (),
            value,
        };
        assert_eq!(
            transform_attitude(&attitude, &transform),
            Err(FrameError::InvalidAttitude)
        );
    }
}

#[test]
fn an_attitude_within_norm_tolerance_is_renormalized_on_the_way_through() {
    let transform = FrameTransform::new(
        frame(FrameKind::Body),
        frame(FrameKind::Ned),
        at(1),
        yaw(0.0),
        [0.0; 3],
    )
    .expect("identity transform");
    let attitude: Attitude<()> = Attitude {
        subject: frame(FrameKind::Body),
        frame: frame(FrameKind::Body),
        at: at(1),
        meta: (),
        value: Quat {
            w: 1.01,
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
    };
    let out = transform_attitude(&attitude, &transform).expect("drift within tolerance");
    assert!((out.value.w - 1.0).abs() < 1e-6);
    assert_eq!(out.frame, frame(FrameKind::Ned));
}

#[test]
fn camera_rereferencing_preserves_its_subject_and_provenance() {
    let camera = Attitude {
        subject: frame(FrameKind::CameraFrd),
        frame: frame(FrameKind::Ned),
        at: at(7),
        meta: META,
        value: yaw(0.2),
    };
    let out = transform_attitude(&camera, &ned_to_ecef()).expect("camera transform");
    assert_eq!(out.subject, camera.subject);
    assert_eq!(out.frame, frame(FrameKind::Ecef));
    assert_eq!(out.meta, camera.meta);
    assert_eq!(out.at, camera.at);
}

#[test]
fn camera_relative_to_aircraft_preserves_both_sources_and_checks_alignment() {
    let aircraft = Attitude {
        subject: frame(FrameKind::Body),
        frame: frame(FrameKind::Ned),
        at: at(50),
        meta: "vehicle",
        value: Quat::from_euler(0.0, 0.0, 0.5),
    };
    let mut camera = Attitude {
        subject: frame(FrameKind::CameraFrd),
        frame: aircraft.frame,
        at: aircraft.at,
        meta: "camera",
        value: Quat::from_euler(0.0, 0.0, 0.8),
    };
    let relative = relative_attitude(&camera, &aircraft).expect("aligned observations");
    assert_eq!(relative.subject, camera.subject);
    assert_eq!(relative.frame, aircraft.subject);
    assert_eq!(relative.meta, ("camera", "vehicle"));
    assert!((relative.value.to_euler().2 - 0.3).abs() < 1e-6);
    camera.at.nanos = 51;
    assert!(matches!(
        relative_attitude(&camera, &aircraft),
        Err(FrameError::TimestampMismatch { .. })
    ));
    camera.at = aircraft.at;
    camera.frame = crate::FrameId::new(FrameKind::Ned, 99).expect("other navigation origin");
    assert!(matches!(
        relative_attitude(&camera, &aircraft),
        Err(FrameError::FrameMismatch { .. })
    ));
}
