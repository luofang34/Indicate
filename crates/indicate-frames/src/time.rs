//! Clock identities and timestamps independent of connection generations.

use crate::FrameError;

/// Clock category. Equal categories do not establish a shared clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ClockDomain {
    /// A vehicle's boot-relative clock.
    VehicleBoot = 0,
    /// A simulation host's clock.
    Simulation = 1,
    /// A GNSS receiver's clock.
    Gnss = 2,
    /// A ground computer's clock.
    Ground = 3,
}

/// One clock instance with a continuous counter and established origin.
///
/// A clock reset requires a new instance, even on the same computer.
/// Cross-clock conversion requires an explicit correlation outside this crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClockId {
    domain: ClockDomain,
    instance: u128,
}

impl ClockId {
    /// Identifies one clock lifetime.
    ///
    /// # Errors
    /// Returns [`FrameError::UnspecifiedIdentity`] when `instance` is zero.
    pub const fn new(domain: ClockDomain, instance: u128) -> Result<Self, FrameError> {
        if instance == 0 {
            return Err(FrameError::UnspecifiedIdentity);
        }
        Ok(Self { domain, instance })
    }

    /// Category of the producing clock.
    pub const fn domain(self) -> ClockDomain {
        self.domain
    }

    /// Producer-assigned identity for this clock lifetime.
    pub const fn instance(self) -> u128 {
        self.instance
    }
}

/// Scale used to express a timestamp.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TimeScale {
    /// Monotonic elapsed time from the clock's declared origin.
    Monotonic = 0,
    /// GPS time.
    Gps = 1,
    /// International Atomic Time.
    Tai = 2,
    /// Coordinated Universal Time.
    Utc = 3,
}

/// The instant at which a value or transform applies.
///
/// This is not a connection generation or a receipt-time substitute for an unknown sample time.
/// A producer must align observations before composing transforms at one timestamp.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timestamp {
    /// Producing clock and its lifetime.
    pub clock: ClockId,
    /// Time scale.
    pub scale: TimeScale,
    /// Nanoseconds on the stated clock and scale.
    pub nanos: u64,
}
