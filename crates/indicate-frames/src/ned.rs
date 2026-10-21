//! Explicit aircraft-to-navigation selection for horizon presentation.

use crate::{Attitude, FrameError, FrameId, FrameKind, Quat, ROTATION_NORM_TOLERANCE};

/// Extracts a validated body-to-NED attitude for the requested aircraft and navigation frame.
///
/// # Errors
/// Refuses a wrong convention, another object, another navigation origin, or an invalid rotation.
pub fn ned_attitude<M>(
    attitude: &Attitude<M>,
    body: FrameId,
    navigation: FrameId,
) -> Result<Quat, FrameError> {
    for (frame, expected) in [(body, FrameKind::Body), (navigation, FrameKind::Ned)] {
        if frame.kind() != expected {
            return Err(FrameError::FrameKindMismatch {
                expected,
                found: frame.kind(),
            });
        }
    }
    for (found, expected) in [(attitude.subject, body), (attitude.frame, navigation)] {
        if found != expected {
            return Err(FrameError::FrameMismatch { expected, found });
        }
    }
    attitude
        .value
        .renormalized(ROTATION_NORM_TOLERANCE)
        .map_err(|_| FrameError::InvalidAttitude)
}

#[cfg(test)]
mod tests;
