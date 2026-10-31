//! Fail-closed binary wire codec for [`AlertOutput`] snapshots.

use core::cmp::Ordering;

use thiserror::Error;

use crate::class::{AlertClass, AlertState, AuralToken};
use crate::condition::AlertId;
use crate::output::{ActiveAlert, AlertOutput, MAX_ACTIVE_ALERTS, ManagerHealth};

/// Wire version of the [`AlertOutput`] snapshot encoding.
pub const ALERT_WIRE_VERSION: u8 = 1;

const HEADER_BYTES: usize = 12;
const ENTRY_BYTES: usize = 10;

/// Maximum encoded byte length of an [`AlertOutput`] snapshot (`12 + 24 * 10`).
pub const MAX_ALERT_WIRE_BYTES: usize = HEADER_BYTES + MAX_ACTIVE_ALERTS * ENTRY_BYTES;

/// Fail-closed error returned when encoding or decoding [`AlertOutput`] wire bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum AlertWireError {
    /// Output buffer cannot hold the encoded snapshot.
    #[error("alert wire buffer too small: needed {needed} bytes, got {available}")]
    BufferTooSmall {
        /// Required byte capacity.
        needed: usize,
        /// Provided slice length.
        available: usize,
    },
    /// Input byte length does not match the header or declared entry count.
    #[error("invalid alert wire length: expected {expected} bytes, got {actual}")]
    InvalidLength {
        /// Expected byte length.
        expected: usize,
        /// Actual byte length.
        actual: usize,
    },
    /// Wire version byte is unsupported.
    #[error("unsupported alert wire version {0}")]
    UnsupportedVersion(u8),
    /// Reserved flag bits are non-zero.
    #[error("non-zero reserved alert wire flags")]
    NonZeroReserved,
    /// Active alert count exceeds [`MAX_ACTIVE_ALERTS`].
    #[error("active alert count {0} exceeds maximum")]
    TooManyAlerts(u8),
    /// Enumeration discriminant byte is unknown.
    #[error("invalid alert wire discriminant {value} for {field}")]
    InvalidDiscriminant {
        /// Field name.
        field: &'static str,
        /// Raw byte value.
        value: u8,
    },
    /// Active alert flags violate class inhibition or declutter invariants.
    #[error("illegal inhibition or declutter flag on alert")]
    IllegalAlertFlags,
    /// Decoded alerts are not in strict priority order.
    #[error("alert entries are not strictly priority-ordered")]
    UnsortedEntries,
}

/// Encodes `alerts` into `out`, returning the number of bytes written.
pub fn encode_alerts(alerts: &AlertOutput, out: &mut [u8]) -> Result<usize, AlertWireError> {
    let active = alerts.active();
    let needed = HEADER_BYTES + active.len() * ENTRY_BYTES;
    if out.len() < needed {
        return Err(AlertWireError::BufferTooSmall {
            needed,
            available: out.len(),
        });
    }
    let count = u8::try_from(active.len()).map_err(|_| AlertWireError::TooManyAlerts(u8::MAX))?;
    let mut flags = 0u8;
    if alerts.overflow() {
        flags |= 0x01;
    }
    if alerts.health() == ManagerHealth::Faulted {
        flags |= 0x02;
    }
    out[0] = ALERT_WIRE_VERSION;
    out[1] = flags;
    out[2] = encode_aural(alerts.aural());
    out[3] = count;
    out[4..8].copy_from_slice(&alerts.overflow_dropped().to_le_bytes());
    out[8..12].copy_from_slice(&alerts.generation().to_le_bytes());
    for (idx, item) in active.iter().enumerate() {
        let base = HEADER_BYTES + idx * ENTRY_BYTES;
        encode_entry(item, &mut out[base..base + ENTRY_BYTES]);
    }
    Ok(needed)
}

/// Decodes a wire byte slice into a validated [`AlertOutput`].
pub fn decode_alerts(bytes: &[u8]) -> Result<AlertOutput, AlertWireError> {
    if bytes.len() < HEADER_BYTES {
        return Err(AlertWireError::InvalidLength {
            expected: HEADER_BYTES,
            actual: bytes.len(),
        });
    }
    if bytes[0] != ALERT_WIRE_VERSION {
        return Err(AlertWireError::UnsupportedVersion(bytes[0]));
    }
    let flags = bytes[1];
    if (flags & !0x03) != 0 {
        return Err(AlertWireError::NonZeroReserved);
    }
    let aural = decode_aural(bytes[2])?;
    let count = usize::from(bytes[3]);
    if count > MAX_ACTIVE_ALERTS {
        return Err(AlertWireError::TooManyAlerts(bytes[3]));
    }
    let expected = HEADER_BYTES + count * ENTRY_BYTES;
    if bytes.len() != expected {
        return Err(AlertWireError::InvalidLength {
            expected,
            actual: bytes.len(),
        });
    }
    let overflow_dropped = read_u32(&bytes[4..8]);
    let generation = read_u32(&bytes[8..12]);
    let mut output = AlertOutput::empty(generation, overflow_dropped);
    output.set_aural(aural);
    output.set_overflow((flags & 0x01) != 0);
    output.set_health(if (flags & 0x02) != 0 {
        ManagerHealth::Faulted
    } else {
        ManagerHealth::Nominal
    });
    let mut prev: Option<ActiveAlert> = None;
    for idx in 0..count {
        let base = HEADER_BYTES + idx * ENTRY_BYTES;
        let entry = decode_entry(&bytes[base..base + ENTRY_BYTES])?;
        if let Some(p) = prev {
            let ord = entry.class.cmp(&p.class).then(p.id.cmp(&entry.id));
            if ord != Ordering::Less {
                return Err(AlertWireError::UnsortedEntries);
            }
        }
        prev = Some(entry);
        output.push(entry);
    }
    Ok(output)
}

fn encode_entry(item: &ActiveAlert, out: &mut [u8]) {
    let mut flags = 0u8;
    if item.inhibited {
        flags |= 0x01;
    }
    if item.decluttered {
        flags |= 0x02;
    }
    out[0..2].copy_from_slice(&item.id.0.to_le_bytes());
    out[2] = encode_class(item.class);
    out[3] = encode_state(item.state);
    out[4] = encode_aural(item.aural);
    out[5] = flags;
    out[6..10].copy_from_slice(&item.generation.to_le_bytes());
}

fn decode_entry(bytes: &[u8]) -> Result<ActiveAlert, AlertWireError> {
    let mut id_bytes = [0u8; 2];
    id_bytes.copy_from_slice(&bytes[0..2]);
    let id = AlertId(u16::from_le_bytes(id_bytes));
    let class = decode_class(bytes[2])?;
    let state = decode_state(bytes[3])?;
    let aural = decode_aural(bytes[4])?;
    let flags = bytes[5];
    if (flags & !0x03) != 0 {
        return Err(AlertWireError::NonZeroReserved);
    }
    let inhibited = (flags & 0x01) != 0;
    let decluttered = (flags & 0x02) != 0;
    if (inhibited && class == AlertClass::Warning)
        || (decluttered && !class.declutters_under_unusual())
    {
        return Err(AlertWireError::IllegalAlertFlags);
    }
    Ok(ActiveAlert {
        id,
        class,
        state,
        aural,
        inhibited,
        decluttered,
        generation: read_u32(&bytes[6..10]),
    })
}

const fn encode_class(class: AlertClass) -> u8 {
    match class {
        AlertClass::Maintenance => 0,
        AlertClass::Status => 1,
        AlertClass::Advisory => 2,
        AlertClass::Caution => 3,
        AlertClass::Warning => 4,
    }
}

const fn decode_class(raw: u8) -> Result<AlertClass, AlertWireError> {
    match raw {
        0 => Ok(AlertClass::Maintenance),
        1 => Ok(AlertClass::Status),
        2 => Ok(AlertClass::Advisory),
        3 => Ok(AlertClass::Caution),
        4 => Ok(AlertClass::Warning),
        value => Err(AlertWireError::InvalidDiscriminant {
            field: "AlertClass",
            value,
        }),
    }
}

const fn encode_state(state: AlertState) -> u8 {
    match state {
        AlertState::Active => 0,
        AlertState::Acknowledged => 1,
        AlertState::LatchedCleared => 2,
    }
}

const fn decode_state(raw: u8) -> Result<AlertState, AlertWireError> {
    match raw {
        0 => Ok(AlertState::Active),
        1 => Ok(AlertState::Acknowledged),
        2 => Ok(AlertState::LatchedCleared),
        value => Err(AlertWireError::InvalidDiscriminant {
            field: "AlertState",
            value,
        }),
    }
}

const fn encode_aural(aural: AuralToken) -> u8 {
    match aural {
        AuralToken::Silent => 0,
        AuralToken::SingleChime => 1,
        AuralToken::TripleChime => 2,
        AuralToken::ContinuousTone => 3,
        AuralToken::AutopilotDisconnect => 4,
    }
}

const fn decode_aural(raw: u8) -> Result<AuralToken, AlertWireError> {
    match raw {
        0 => Ok(AuralToken::Silent),
        1 => Ok(AuralToken::SingleChime),
        2 => Ok(AuralToken::TripleChime),
        3 => Ok(AuralToken::ContinuousTone),
        4 => Ok(AuralToken::AutopilotDisconnect),
        value => Err(AlertWireError::InvalidDiscriminant {
            field: "AuralToken",
            value,
        }),
    }
}

fn read_u32(slice: &[u8]) -> u32 {
    let mut b = [0u8; 4];
    b.copy_from_slice(&slice[..4]);
    u32::from_le_bytes(b)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
    use super::*;
    use crate::{
        AlertCondition, AlertContext, AlertEvent, AlertManager, AlertProfile, AltFault, NavFault,
    };

    #[test]
    fn round_trips_alert_manager_output() {
        let mut mgr = AlertManager::new();
        let profile = AlertProfile::simulator();
        let events = [
            AlertEvent::Assert(AlertCondition::Altitude(AltFault::ReferenceLost)),
            AlertEvent::Assert(AlertCondition::Heading(NavFault::HeadingReferenceLost)),
        ];
        let out = mgr.step(&profile, &events, AlertContext::default(), 100);
        let mut buf = [0u8; MAX_ALERT_WIRE_BYTES];
        let n = encode_alerts(&out, &mut buf).expect("encode");
        let decoded = decode_alerts(&buf[..n]).expect("decode");
        assert_eq!(decoded, out);
    }
}
