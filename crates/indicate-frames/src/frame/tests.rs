use super::{FrameId, FrameKind};
use crate::FrameError;

#[test]
fn conventions_round_trip_without_claiming_instance_identity() {
    for code in 0..=12 {
        let kind = FrameKind::from_u8(code).expect("known convention");
        assert_eq!(kind.to_u8(), code);
    }
    for code in 13..=255 {
        assert_eq!(
            FrameKind::from_u8(code),
            Err(FrameError::UnknownFrameKind { code })
        );
    }
}

#[test]
fn physical_identity_is_required_and_distinguishes_equal_conventions() {
    assert_eq!(
        FrameId::new(FrameKind::CameraFrd, 0),
        Err(FrameError::UnspecifiedIdentity)
    );
    let visible = FrameId::new(FrameKind::CameraFrd, 1).expect("visible");
    let thermal = FrameId::new(FrameKind::CameraFrd, 2).expect("thermal");
    assert_ne!(visible, thermal);
    assert_eq!(visible.kind(), thermal.kind());
    assert_eq!(visible.instance(), 1);
}
