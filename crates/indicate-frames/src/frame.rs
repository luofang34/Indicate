//! Coordinate conventions and explicit identities for physical reference frames.

use crate::FrameError;

/// Coordinate convention, independent of the physical frame instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FrameKind {
    /// Vehicle forward, right, down axes.
    Body = 0,
    /// Fixed sensor or mounting coordinates declared by the producer.
    Installation = 1,
    /// North, east, down at a declared origin.
    Ned = 2,
    /// Earth-centered, Earth-fixed coordinates.
    Ecef = 3,
    /// Earth-centered inertial coordinates in a declared realization.
    Eci = 4,
    /// Local-vertical, local-horizontal orbital coordinates.
    Lvlh = 5,
    /// Radial, transverse, normal orbital coordinates.
    Rtn = 6,
    /// Coordinates relative to a declared object.
    TargetRelative = 7,
    /// Camera forward, right, down axes; forward follows the optical axis.
    CameraFrd = 8,
    /// Camera optical axes: right, down, forward.
    CameraOptical = 9,
    /// Observer forward, right, down axes.
    ObserverFrd = 10,
    /// East, north, up at a declared origin.
    Enu = 11,
    /// Horizon-level axes with yaw following the vehicle heading.
    HorizonVehicleYaw = 12,
}

impl FrameKind {
    /// Encodes the convention. This byte does not identify a physical frame.
    pub const fn to_u8(self) -> u8 {
        self as u8
    }

    /// Decodes a coordinate convention.
    ///
    /// # Errors
    /// Returns [`FrameError::UnknownFrameKind`] for an unknown code.
    pub const fn from_u8(code: u8) -> Result<Self, FrameError> {
        match code {
            0 => Ok(Self::Body),
            1 => Ok(Self::Installation),
            2 => Ok(Self::Ned),
            3 => Ok(Self::Ecef),
            4 => Ok(Self::Eci),
            5 => Ok(Self::Lvlh),
            6 => Ok(Self::Rtn),
            7 => Ok(Self::TargetRelative),
            8 => Ok(Self::CameraFrd),
            9 => Ok(Self::CameraOptical),
            10 => Ok(Self::ObserverFrd),
            11 => Ok(Self::Enu),
            12 => Ok(Self::HorizonVehicleYaw),
            _ => Err(FrameError::UnknownFrameKind { code }),
        }
    }
}

/// One reference frame within a producer-assigned identity namespace.
///
/// Producers must share identifiers only for the same physical axes and origin.
/// A reset of the origin or reference alignment requires a new identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameId {
    kind: FrameKind,
    instance: u128,
}

impl FrameId {
    /// Binds a coordinate convention to a nonzero instance identifier.
    ///
    /// # Errors
    /// Returns [`FrameError::UnspecifiedIdentity`] when `instance` is zero.
    pub const fn new(kind: FrameKind, instance: u128) -> Result<Self, FrameError> {
        if instance == 0 {
            return Err(FrameError::UnspecifiedIdentity);
        }
        Ok(Self { kind, instance })
    }

    /// Coordinate convention used by this frame.
    pub const fn kind(self) -> FrameKind {
        self.kind
    }

    /// Producer-assigned identity, including the alignment lifetime.
    pub const fn instance(self) -> u128 {
        self.instance
    }
}

#[cfg(test)]
mod tests;
