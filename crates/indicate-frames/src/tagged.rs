//! Frame-tagged kinematic state with provenance carried through
//! transforms.

use crate::error::FrameError;
use crate::frame::FrameId;
use crate::rotation::Quat;
use crate::time::Timestamp;
use crate::transform::{FrameTransform, ROTATION_NORM_TOLERANCE, check_timestamps};

/// A value with a physical reference frame, timestamp, and producer metadata.
/// Transformations preserve metadata; they do not establish source quality or freshness.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tagged<T, M> {
    /// Coordinates in which the value is expressed.
    pub frame: FrameId,
    /// The instant the value is valid at.
    pub at: Timestamp,
    /// Producer-stamped provenance, preserved verbatim by transforms.
    pub meta: M,
    /// The value.
    pub value: T,
}

/// Position, meters, in the tagged frame.
pub type Position<M> = Tagged<[f64; 3], M>;
/// Velocity, meters/second, in the tagged frame.
pub type Velocity<M> = Tagged<[f64; 3], M>;
/// Acceleration, meters/second², in the tagged frame.
pub type Acceleration<M> = Tagged<[f64; 3], M>;
/// Body angular velocity, radians/second, about body axes; the tag
/// names the reference frame the rotation is measured against.
pub type AngularVelocity<M> = Tagged<[f32; 3], M>;
/// Orientation of one explicitly identified object in another reference frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Attitude<M> {
    /// The object's own coordinates: vehicle, camera, or observer.
    pub subject: FrameId,
    /// Reference coordinates into which `value` rotates the subject.
    pub frame: FrameId,
    /// Instant at which the orientation applies.
    pub at: Timestamp,
    /// Producer provenance, preserved by transformations.
    pub meta: M,
    /// Rotation from `subject` into `frame`.
    pub value: Quat,
}

/// The vehicle-neutral pose boundary: where the vehicle is and how it
/// is oriented, each component carrying its own frame/timestamp/provenance
/// tag (an aircraft may pair NED position with NED attitude; a
/// spacecraft may pair ECI position with LVLH attitude — the pairing is
/// explicit, never assumed).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pose<M> {
    /// Orientation of the body in its tagged reference frame.
    pub attitude: Attitude<M>,
    /// Location in its tagged frame, meters.
    pub position: Position<M>,
}

/// The vehicle-neutral twist boundary: how the pose is changing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Twist<M> {
    /// Body-axis rotation rate against its tagged reference.
    pub angular_velocity: AngularVelocity<M>,
    /// Translational rate in its tagged frame.
    pub velocity: Velocity<M>,
}

fn check_input<T, M>(tagged: &Tagged<T, M>, transform: &FrameTransform) -> Result<(), FrameError> {
    if tagged.frame != transform.from_frame() {
        return Err(FrameError::FrameMismatch {
            expected: transform.from_frame(),
            found: tagged.frame,
        });
    }
    check_timestamps(tagged.at, transform.at())
}

/// Transforms a position (rotation and translation apply).
///
/// # Errors
///
/// Typed refusals for frame or timestamp disagreement ([`FrameError`]).
pub fn transform_position<M: Copy>(
    position: &Position<M>,
    transform: &FrameTransform,
) -> Result<Position<M>, FrameError> {
    check_input(position, transform)?;
    let rotated = transform.rotation().rotate(position.value);
    let t = transform.translation_m();
    Ok(Tagged {
        frame: transform.to_frame(),
        at: position.at,
        meta: position.meta,
        value: [rotated[0] + t[0], rotated[1] + t[1], rotated[2] + t[2]],
    })
}

/// Transforms a free vector (velocity, acceleration): rotation only —
/// a snapshot transform carries no rotating-frame kinematic terms.
///
/// # Errors
///
/// Typed refusals for frame or timestamp disagreement ([`FrameError`]).
pub fn transform_vector<M: Copy>(
    vector: &Tagged<[f64; 3], M>,
    transform: &FrameTransform,
) -> Result<Tagged<[f64; 3], M>, FrameError> {
    check_input(vector, transform)?;
    Ok(Tagged {
        frame: transform.to_frame(),
        at: vector.at,
        meta: vector.meta,
        value: transform.rotation().rotate(vector.value),
    })
}

/// Re-expresses the same object's orientation in another reference frame.
/// The transform must begin at the attitude's reference frame and share its timestamp.
///
/// # Errors
/// Refuses mismatched frames, timestamps, or an invalid attitude rotation.
pub fn transform_attitude<M: Copy>(
    attitude: &Attitude<M>,
    transform: &FrameTransform,
) -> Result<Attitude<M>, FrameError> {
    if attitude.frame != transform.from_frame() {
        return Err(FrameError::FrameMismatch {
            expected: transform.from_frame(),
            found: attitude.frame,
        });
    }
    check_timestamps(attitude.at, transform.at())?;
    let value = attitude
        .value
        .renormalized(ROTATION_NORM_TOLERANCE)
        .map_err(|_| FrameError::InvalidAttitude)?;
    Ok(Attitude {
        subject: attitude.subject,
        frame: transform.to_frame(),
        at: attitude.at,
        meta: attitude.meta,
        value: transform.rotation().compose(value),
    })
}

/// Expresses one observed object's attitude relative to another observed object.
/// Both observations must share a reference frame and an aligned timestamp.
/// The result preserves both provenance records in subject, reference order.
///
/// # Errors
/// Refuses mismatched references, timestamps, or invalid rotations.
pub fn relative_attitude<S: Copy, R: Copy>(
    subject: &Attitude<S>,
    reference: &Attitude<R>,
) -> Result<Attitude<(S, R)>, FrameError> {
    let reference_transform = FrameTransform::new(
        reference.subject,
        reference.frame,
        reference.at,
        reference.value,
        [0.0; 3],
    )?;
    let relative = transform_attitude(subject, &reference_transform.inverse())?;
    Ok(Attitude {
        subject: relative.subject,
        frame: relative.frame,
        at: relative.at,
        meta: (subject.meta, reference.meta),
        value: relative.value,
    })
}

#[cfg(test)]
mod tests;
