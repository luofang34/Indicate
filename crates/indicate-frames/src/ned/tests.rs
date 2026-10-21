use crate::fixtures::{at, frame};
use crate::{Attitude, FrameError, FrameKind, Quat, ned_attitude};

#[test]
fn ned_reference_passes_through_bit_identical() {
    let attitude = Attitude {
        subject: frame(FrameKind::Body),
        frame: frame(FrameKind::Ned),
        at: at(1),
        meta: 42,
        value: Quat {
            w: 0.5,
            x: 0.5,
            y: 0.5,
            z: 0.5,
        },
    };
    assert_eq!(
        ned_attitude(&attitude, attitude.subject, attitude.frame),
        Ok(attitude.value)
    );
}

#[test]
fn camera_attitude_cannot_be_used_as_aircraft_attitude() {
    let attitude = Attitude {
        subject: frame(FrameKind::CameraFrd),
        frame: frame(FrameKind::Ned),
        at: at(1),
        meta: (),
        value: Quat::IDENTITY,
    };
    assert!(matches!(
        ned_attitude(&attitude, frame(FrameKind::Body), attitude.frame),
        Err(FrameError::FrameMismatch { .. })
    ));
}

#[test]
fn another_navigation_origin_cannot_be_relabeled() {
    let attitude = Attitude {
        subject: frame(FrameKind::Body),
        frame: frame(FrameKind::Ned),
        at: at(1),
        meta: (),
        value: Quat::IDENTITY,
    };
    let other = crate::FrameId::new(FrameKind::Ned, 999).expect("another origin");
    assert!(matches!(
        ned_attitude(&attitude, attitude.subject, other),
        Err(FrameError::FrameMismatch { .. })
    ));
}
