//! Typed failures for frame composition and decoding.

use crate::{FrameId, FrameKind, Timestamp};

/// Why a frame operation was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FrameError {
    /// An unknown coordinate convention was supplied.
    #[error("unknown frame convention {code}")]
    UnknownFrameKind {
        /// Unrecognized wire value.
        code: u8,
    },
    /// A frame identity was supplied with an incompatible convention.
    #[error("expected frame convention {expected:?}, found {found:?}")]
    FrameKindMismatch {
        /// Required convention.
        expected: FrameKind,
        /// Supplied convention.
        found: FrameKind,
    },
    /// An operation received a different physical frame.
    #[error("expected frame {expected:?}, found {found:?}")]
    FrameMismatch {
        /// Required frame.
        expected: FrameId,
        /// Supplied frame.
        found: FrameId,
    },
    /// Two inputs apply at different instants.
    #[error("expected timestamp {expected:?}, found {found:?}")]
    TimestampMismatch {
        /// Required instant.
        expected: Timestamp,
        /// Supplied instant.
        found: Timestamp,
    },
    /// Clock categories or instances differ.
    #[error("clock identities differ")]
    ClockMismatch,
    /// Time scales differ.
    #[error("time scales differ")]
    TimeScaleMismatch,
    /// A zero identifier cannot establish physical identity.
    #[error("frame or clock identity is unspecified")]
    UnspecifiedIdentity,
    /// Rotation or translation is invalid.
    #[error("invalid rigid transform")]
    InvalidTransform,
    /// The supplied attitude is not a rotation.
    #[error("invalid attitude rotation")]
    InvalidAttitude,
}
