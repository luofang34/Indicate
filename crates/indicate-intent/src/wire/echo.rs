//! Wire codec for [`EchoSummary`] (`12` bytes, little-endian).

use crate::echo::EchoStatus;
use crate::wire::{IntentWireError, read_u32};

/// Wire version of the 12-byte [`EchoSummary`] layout.
pub const ECHO_SUMMARY_VERSION: u8 = 1;

/// Exact encoded byte length of [`EchoSummary`].
pub const ECHO_SUMMARY_BYTES: usize = 12;

/// Summary of the four selection echo slots and last admitted sequence carried in `DrawContext`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EchoSummary {
    /// Status per [`crate::SELECTION_KINDS`] slot.
    pub statuses: [EchoStatus; 4],
    /// Sequence number of the last admitted request, if any.
    pub last_admitted_seq: Option<u32>,
}

impl EchoSummary {
    /// All four selection slots idle and no admitted sequence recorded.
    pub const IDLE: Self = Self {
        statuses: [EchoStatus::Idle; 4],
        last_admitted_seq: None,
    };
}

impl Default for EchoSummary {
    fn default() -> Self {
        Self::IDLE
    }
}

/// Encodes `summary` into its fixed 12-byte little-endian wire layout.
pub fn encode_echo_summary(summary: &EchoSummary, out: &mut [u8; ECHO_SUMMARY_BYTES]) {
    *out = [0u8; ECHO_SUMMARY_BYTES];
    out[0] = ECHO_SUMMARY_VERSION;
    for (idx, status) in summary.statuses.iter().enumerate() {
        out[1 + idx] = match status {
            EchoStatus::Idle => 0,
            EchoStatus::Pending { .. } => 1,
            EchoStatus::Confirmed => 2,
            EchoStatus::Overdue { .. } | EchoStatus::Unmeasurable => 3,
        };
    }
    if let Some(seq) = summary.last_admitted_seq {
        out[5] = 1;
        out[8..12].copy_from_slice(&seq.to_le_bytes());
    }
}

/// Encodes `summary` at the start of `out`, returning [`ECHO_SUMMARY_BYTES`].
pub fn encode_echo_summary_slice(
    summary: &EchoSummary,
    out: &mut [u8],
) -> Result<usize, IntentWireError> {
    if out.len() < ECHO_SUMMARY_BYTES {
        return Err(IntentWireError::BufferTooSmall {
            needed: ECHO_SUMMARY_BYTES,
            available: out.len(),
        });
    }
    let mut buf = [0u8; ECHO_SUMMARY_BYTES];
    encode_echo_summary(summary, &mut buf);
    out[..ECHO_SUMMARY_BYTES].copy_from_slice(&buf);
    Ok(ECHO_SUMMARY_BYTES)
}

/// Decodes a 12-byte little-endian slice into a validated [`EchoSummary`].
pub fn decode_echo_summary(bytes: &[u8]) -> Result<EchoSummary, IntentWireError> {
    if bytes.len() != ECHO_SUMMARY_BYTES {
        return Err(IntentWireError::InvalidLength {
            expected: ECHO_SUMMARY_BYTES,
            actual: bytes.len(),
        });
    }
    if bytes[0] != ECHO_SUMMARY_VERSION {
        return Err(IntentWireError::UnsupportedVersion(bytes[0]));
    }
    if bytes[6] != 0 || bytes[7] != 0 {
        return Err(IntentWireError::NonZeroReserved);
    }
    let mut statuses = [EchoStatus::Idle; 4];
    for (idx, slot) in statuses.iter_mut().enumerate() {
        *slot = match bytes[1 + idx] {
            0 => EchoStatus::Idle,
            1 => EchoStatus::Pending { age_ms: 0.0 },
            2 => EchoStatus::Confirmed,
            3 => EchoStatus::Overdue { age_ms: 0.0 },
            value => {
                return Err(IntentWireError::InvalidDiscriminant {
                    field: "EchoStatus",
                    value,
                });
            }
        };
    }
    let seq = read_u32(&bytes[8..12]);
    let last_admitted_seq = match bytes[5] {
        0 => {
            if seq != 0 {
                return Err(IntentWireError::NonZeroReserved);
            }
            None
        }
        1 => Some(seq),
        value => {
            return Err(IntentWireError::InvalidDiscriminant {
                field: "has_last_seq",
                value,
            });
        }
    };
    Ok(EchoSummary {
        statuses,
        last_admitted_seq,
    })
}
