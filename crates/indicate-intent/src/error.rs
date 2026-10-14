//! Why a request was refused. Every variant is fail-closed: a refused
//! request reaches no source, no manager, and no display.

/// The gate's typed refusal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum IntentFault {
    /// The request came from a device other than the pinned one.
    #[error("request from device {found} but device {pinned} is pinned")]
    WrongDevice {
        /// The device the gate is pinned to.
        pinned: u32,
        /// The device that asked.
        found: u32,
    },
    /// The sequence is not newer than the last admitted one: a duplicate
    /// or a reordered request.
    #[error("request sequence {found} is not newer than {current}")]
    Replay {
        /// The last admitted sequence.
        current: u32,
        /// The sequence that asked.
        found: u32,
    },
    /// The host clock reading could not be trusted: not a number,
    /// negative, or behind the last reading the gate acted on.
    #[error("host clock reading could not be trusted")]
    InvalidClock,
    /// A requested value is not a number or is infinite.
    #[error("requested {what} is not finite")]
    NonFinite {
        /// Which value.
        what: &'static str,
    },
    /// A requested value lies outside the declared bounds.
    #[error("requested {what} is outside its bounds")]
    OutOfRange {
        /// Which value.
        what: &'static str,
    },
    /// A reference, class, or source the state cannot name.
    #[error("requested {what} names an unknown reference")]
    UnknownReference {
        /// Which field.
        what: &'static str,
    },
}
