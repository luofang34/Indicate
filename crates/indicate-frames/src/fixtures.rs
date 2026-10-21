//! Explicit identities shared by transform test fixtures.
#![cfg(test)]

use crate::{ClockDomain, ClockId, FrameId, FrameKind, TimeScale, Timestamp};

pub(crate) fn frame(kind: FrameKind) -> FrameId {
    FrameId::new(kind, u128::from(kind.to_u8()) + 1).expect("fixture frame identity")
}

pub(crate) fn clock(domain: ClockDomain) -> ClockId {
    ClockId::new(domain, u128::from(domain as u8) + 1).expect("fixture clock identity")
}

pub(crate) fn at(nanos: u64) -> Timestamp {
    Timestamp {
        clock: clock(ClockDomain::Simulation),
        scale: TimeScale::Monotonic,
        nanos,
    }
}
