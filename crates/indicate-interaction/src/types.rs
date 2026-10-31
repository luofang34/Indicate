//! Core spatial interaction types and fail-closed wire errors.

use indicate_frames::Quat;
use libm::sqrtf;
use thiserror::Error;

/// Tolerance for unit-norm validation of quaternions and ray directions on the wire.
pub(crate) const UNIT_TOLERANCE: f32 = 1.0e-3;

/// Stable identifier of an interactive scene handle or target within an instrument.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TargetId(pub u32);

impl TargetId {
    /// Sentinel value indicating no active or hit target.
    pub const NONE: Self = Self(0);

    /// Whether this target identifier is [`TargetId::NONE`].
    #[must_use]
    pub const fn is_none(self) -> bool {
        self.0 == 0
    }
}

/// Lifecycle phase of a continuous 3D drag or transform gesture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum GesturePhase {
    /// Initial contact or pinch start.
    Began = 1,
    /// Incremental update while active.
    Changed = 2,
    /// Clean release completing the gesture.
    Ended = 3,
    /// System interruption aborting the gesture.
    Cancelled = 4,
}

impl GesturePhase {
    /// Decodes a wire byte into a [`GesturePhase`].
    #[must_use]
    pub const fn from_u8(raw: u8) -> Option<Self> {
        match raw {
            1 => Some(Self::Began),
            2 => Some(Self::Changed),
            3 => Some(Self::Ended),
            4 => Some(Self::Cancelled),
            _ => None,
        }
    }

    /// Encodes this [`GesturePhase`] as its canonical wire byte.
    #[must_use]
    pub const fn to_u8(self) -> u8 {
        self as u8
    }
}

/// Transition state of a discrete hardware or bezel key input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum KeyState {
    /// Initial key press.
    Pressed = 1,
    /// Key release.
    Released = 2,
    /// Held key auto-repeat.
    Repeat = 3,
}

impl KeyState {
    /// Decodes a wire byte into a [`KeyState`].
    #[must_use]
    pub const fn from_u8(raw: u8) -> Option<Self> {
        match raw {
            1 => Some(Self::Pressed),
            2 => Some(Self::Released),
            3 => Some(Self::Repeat),
            _ => None,
        }
    }

    /// Encodes this [`KeyState`] as its canonical wire byte.
    #[must_use]
    pub const fn to_u8(self) -> u8 {
        self as u8
    }
}

/// Monotonic device and timing stamp attached to one [`InteractionFrame`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InteractionStamp {
    /// Stable host input-device identifier.
    pub device_id: u32,
    /// Per-device sequence number.
    pub sequence: u32,
    /// Capture timestamp in microseconds on the host interaction clock.
    pub timestamp_us: u64,
}

/// One typed spatial or discrete input event in local instrument meters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpatialEvent {
    /// Primary tap, pinch-select, or click on a scene point (`0x01`).
    Activate {
        /// Hit scene target, or [`TargetId::NONE`] for background activation.
        target: TargetId,
        /// Contact point in local volume coordinates (meters).
        point_local: [f32; 3],
    },
    /// Single-hand or pointer 3D translation gesture (`0x02`).
    Drag3D {
        /// Gesture lifecycle phase.
        phase: GesturePhase,
        /// Hit target at gesture start, or [`TargetId::NONE`].
        target: TargetId,
        /// Gesture origin in local volume coordinates (meters).
        origin_local: [f32; 3],
        /// Incremental translation delta in local volume coordinates (meters).
        delta_local: [f32; 3],
    },
    /// Two-hand or trackball simultaneous pan, orbit, and scale gesture (`0x03`).
    Transform3D {
        /// Gesture lifecycle phase.
        phase: GesturePhase,
        /// Pivot center in local volume coordinates (meters).
        pivot_local: [f32; 3],
        /// Incremental translation in local volume coordinates (meters).
        translation_local: [f32; 3],
        /// Incremental unit rotation quaternion.
        rotation: Quat,
        /// Multiplicative scale factor (`> 0.0`).
        scale: f32,
    },
    /// Gaze or controller ray hover update (`0x04`).
    RayHover {
        /// Currently hovered target, or [`TargetId::NONE`].
        target: TargetId,
        /// Ray origin in local volume coordinates (meters).
        origin_local: [f32; 3],
        /// Unit ray direction in local volume coordinates.
        dir_local: [f32; 3],
    },
    /// Discrete bezel, keyboard, or controller button transition (`0x05`).
    DiscreteKey {
        /// Logical key identifier.
        key_id: u16,
        /// Transition state.
        state: KeyState,
    },
    /// Ephemeris or replay preview time scrub offset (`0x06`).
    ScrubTime {
        /// Preview offset from current time in seconds.
        offset_s: f64,
    },
    /// Reset camera pose, active target, and scrub offset to defaults (`0x07`).
    ResetView,
    /// Unrecognized future opcode skipped cleanly on decode.
    Unknown {
        /// Raw wire opcode byte.
        opcode: u8,
    },
}

/// Stamped spatial interaction frame carried between a display shell and host.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InteractionFrame {
    /// Device and sequence metadata.
    pub stamp: InteractionStamp,
    /// Decoded spatial event.
    pub event: SpatialEvent,
}

/// Fail-closed error returned when encoding, decoding, or applying spatial interaction data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum InteractionError {
    /// Output slice is smaller than the encoded frame or pose requires.
    #[error("output buffer too small: needed {needed} bytes, got {available}")]
    BufferTooSmall {
        /// Required byte capacity.
        needed: usize,
        /// Provided slice length.
        available: usize,
    },
    /// Input slice is truncated or does not match the declared wire length.
    #[error("unexpected wire length: expected {expected} bytes, got {actual}")]
    InvalidLength {
        /// Expected byte length.
        expected: usize,
        /// Actual byte length.
        actual: usize,
    },
    /// Interaction frame exceeds [`crate::MAX_INTERACTION_BYTES`].
    #[error("interaction frame of {len} bytes exceeds limit")]
    FrameTooLarge {
        /// Total wire length in bytes.
        len: usize,
    },
    /// Wire format version byte does not match this build.
    #[error("unsupported wire format version {0}")]
    UnsupportedVersion(u8),
    /// Reserved flags or padding bytes are non-zero.
    #[error("non-zero reserved flags or bytes")]
    NonZeroReserved,
    /// Enumeration discriminant byte is outside its defined set.
    #[error("invalid enum discriminant {value} for {field}")]
    InvalidDiscriminant {
        /// Field name.
        field: &'static str,
        /// Raw byte value.
        value: u8,
    },
    /// Floating-point field is NaN or infinite.
    #[error("non-finite floating-point value in {field}")]
    NonFinite {
        /// Field name.
        field: &'static str,
    },
    /// Quaternion or direction vector norm deviates from unity beyond tolerance.
    #[error("non-unit vector or quaternion in {field}")]
    NonUnit {
        /// Field name.
        field: &'static str,
    },
    /// Scale or zoom factor is not strictly positive.
    #[error("scale factor must be positive in {field}")]
    NonPositiveScale {
        /// Field name.
        field: &'static str,
    },
    /// [`crate::ViewLimits`] bounds are inverted, negative, or non-finite.
    #[error("invalid view limits")]
    InvalidLimits,
    /// Attempted to encode [`SpatialEvent::Unknown`] with a known core opcode.
    #[error("unknown event carries reserved core opcode {0:#04x}")]
    ReservedUnknownOpcode(u8),
}

pub(crate) fn check_vec3(v: [f32; 3], field: &'static str) -> Result<(), InteractionError> {
    if v[0].is_finite() && v[1].is_finite() && v[2].is_finite() {
        Ok(())
    } else {
        Err(InteractionError::NonFinite { field })
    }
}

pub(crate) fn check_unit_vec3(v: [f32; 3], field: &'static str) -> Result<(), InteractionError> {
    check_vec3(v, field)?;
    let norm = sqrtf(v[0] * v[0] + v[1] * v[1] + v[2] * v[2]);
    if norm.is_finite() && (norm - 1.0).abs() <= UNIT_TOLERANCE {
        Ok(())
    } else {
        Err(InteractionError::NonUnit { field })
    }
}

pub(crate) fn check_unit_quat(q: Quat, field: &'static str) -> Result<(), InteractionError> {
    if !(q.x.is_finite() && q.y.is_finite() && q.z.is_finite() && q.w.is_finite()) {
        return Err(InteractionError::NonFinite { field });
    }
    let norm = sqrtf(q.x * q.x + q.y * q.y + q.z * q.z + q.w * q.w);
    if norm.is_finite() && (norm - 1.0).abs() <= UNIT_TOLERANCE {
        Ok(())
    } else {
        Err(InteractionError::NonUnit { field })
    }
}
