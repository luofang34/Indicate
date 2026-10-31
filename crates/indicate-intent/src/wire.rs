//! Binary wire codecs for [`Request`] and [`EchoSummary`] (INT-01).

use indicate_alerts::AlertId;
use indicate_instrument_state::{
    AltitudeClass, GeoidModelId, HeadingReference, NavSource, OriginId,
};
use thiserror::Error;

use crate::namespace::{ExtensionPayload, IntentNamespace, MAX_EXTENSION_PAYLOAD, core_opcode};
use crate::request::{Intent, IntentStamp, PageId, Request};

mod echo;
pub use echo::{
    ECHO_SUMMARY_BYTES, ECHO_SUMMARY_VERSION, EchoSummary, decode_echo_summary,
    encode_echo_summary, encode_echo_summary_slice,
};

/// Wire version of the [`Request`] envelope.
pub const INTENT_WIRE_VERSION: u8 = 1;

const REQUEST_HEADER_BYTES: usize = 30;

/// Maximum encoded byte length of one [`Request`] (`30` header + `66` headroom, `<= 96`).
pub const MAX_INTENT_WIRE_BYTES: usize = 96;

/// Fail-closed wire error returned when encoding or decoding intent envelopes or echo summaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum IntentWireError {
    /// Output slice is smaller than required.
    #[error("intent wire buffer too small: needed {needed} bytes, got {available}")]
    BufferTooSmall {
        /// Required byte capacity.
        needed: usize,
        /// Provided slice length.
        available: usize,
    },
    /// Input byte length does not match the header or declared payload length.
    #[error("invalid intent wire length: expected {expected} bytes, got {actual}")]
    InvalidLength {
        /// Expected byte length.
        expected: usize,
        /// Actual byte length.
        actual: usize,
    },
    /// Extension payload exceeds [`MAX_EXTENSION_PAYLOAD`].
    #[error("intent payload of {len} bytes exceeds 64-byte maximum")]
    PayloadTooLarge {
        /// Payload length in bytes.
        len: usize,
    },
    /// Wire format version byte does not match this build.
    #[error("unsupported intent wire version {0}")]
    UnsupportedVersion(u8),
    /// Reserved byte or flag bit is non-zero.
    #[error("non-zero reserved byte in intent wire")]
    NonZeroReserved,
    /// Unknown opcode inside [`IntentNamespace::CORE`].
    #[error("unknown core intent opcode {0:#06x}")]
    UnknownCoreOpcode(u16),
    /// [`Intent::Custom`] attempted to use [`IntentNamespace::CORE`].
    #[error("custom intent cannot encode in reserved core namespace")]
    ReservedCoreNamespace,
    /// Enumerated field byte is outside its defined set.
    #[error("invalid discriminant {value} for {field}")]
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
    /// Integer field exceeds its wire representation width.
    #[error("field {field} overflows wire representation")]
    FieldOverflow {
        /// Field name.
        field: &'static str,
    },
}

/// Encodes `request` into `out`, returning the number of bytes written.
pub fn encode_request(request: &Request, out: &mut [u8]) -> Result<usize, IntentWireError> {
    let mut payload = [0u8; MAX_EXTENSION_PAYLOAD];
    let (namespace, opcode, payload_len) = encode_intent(&request.intent, &mut payload)?;
    let total = REQUEST_HEADER_BYTES + payload_len;
    if out.len() < total {
        return Err(IntentWireError::BufferTooSmall {
            needed: total,
            available: out.len(),
        });
    }
    let len_u16 = u16::try_from(payload_len)
        .map_err(|_| IntentWireError::PayloadTooLarge { len: payload_len })?;
    out[0] = INTENT_WIRE_VERSION;
    out[1] = 0;
    out[2..6].copy_from_slice(&request.stamp.device_id.to_le_bytes());
    out[6..10].copy_from_slice(&request.stamp.sequence.to_le_bytes());
    out[10..18].copy_from_slice(&request.stamp.issued_at_ms.to_le_bytes());
    out[18..26].copy_from_slice(&namespace.0.to_le_bytes());
    out[26..28].copy_from_slice(&opcode.to_le_bytes());
    out[28..30].copy_from_slice(&len_u16.to_le_bytes());
    out[REQUEST_HEADER_BYTES..total].copy_from_slice(&payload[..payload_len]);
    Ok(total)
}

/// Decodes a wire byte slice into a [`Request`].
pub fn decode_request(bytes: &[u8]) -> Result<Request, IntentWireError> {
    if bytes.len() < REQUEST_HEADER_BYTES {
        return Err(IntentWireError::InvalidLength {
            expected: REQUEST_HEADER_BYTES,
            actual: bytes.len(),
        });
    }
    if bytes[0] != INTENT_WIRE_VERSION {
        return Err(IntentWireError::UnsupportedVersion(bytes[0]));
    }
    if bytes[1] != 0 {
        return Err(IntentWireError::NonZeroReserved);
    }
    let payload_len = usize::from(read_u16(&bytes[28..30]));
    if payload_len > MAX_EXTENSION_PAYLOAD {
        return Err(IntentWireError::PayloadTooLarge { len: payload_len });
    }
    let expected = REQUEST_HEADER_BYTES + payload_len;
    if bytes.len() != expected {
        return Err(IntentWireError::InvalidLength {
            expected,
            actual: bytes.len(),
        });
    }
    let stamp = IntentStamp {
        device_id: read_u32(&bytes[2..6]),
        sequence: read_u32(&bytes[6..10]),
        issued_at_ms: read_u64(&bytes[10..18]),
    };
    let namespace = IntentNamespace(read_u64(&bytes[18..26]));
    let opcode = read_u16(&bytes[26..28]);
    let payload = &bytes[REQUEST_HEADER_BYTES..expected];
    let intent = if namespace.is_core() {
        decode_core_intent(opcode, payload)?
    } else {
        Intent::Custom {
            namespace,
            opcode,
            payload: ExtensionPayload::new(payload)?,
        }
    };
    Ok(Request { stamp, intent })
}

fn encode_intent(
    intent: &Intent,
    out: &mut [u8; MAX_EXTENSION_PAYLOAD],
) -> Result<(IntentNamespace, u16, usize), IntentWireError> {
    match *intent {
        Intent::SelectBaro { hpa } => {
            check_finite(hpa, "hpa")?;
            out[0..4].copy_from_slice(&hpa.to_le_bytes());
            Ok((IntentNamespace::CORE, core_opcode::SELECT_BARO, 4))
        }
        Intent::SetHeadingBug { rad, reference } => {
            check_finite(rad, "rad")?;
            if reference == HeadingReference::Unknown {
                return Err(IntentWireError::InvalidDiscriminant {
                    field: "HeadingReference",
                    value: reference.to_u8(),
                });
            }
            out[0..4].copy_from_slice(&rad.to_le_bytes());
            out[4] = reference.to_u8();
            Ok((IntentNamespace::CORE, core_opcode::SET_HEADING_BUG, 5))
        }
        Intent::SelectAltitude {
            meters,
            class,
            origin,
            model,
        } => encode_altitude(out, meters, class, origin, model),
        Intent::SelectNavSource(source) => {
            out[0] = encode_nav_source(source)?;
            Ok((IntentNamespace::CORE, core_opcode::SELECT_NAV_SOURCE, 1))
        }
        Intent::AcknowledgeAlert(id) => {
            out[0..2].copy_from_slice(&id.0.to_le_bytes());
            Ok((IntentNamespace::CORE, core_opcode::ACKNOWLEDGE_ALERT, 2))
        }
        Intent::AcknowledgeAllAlerts => Ok((
            IntentNamespace::CORE,
            core_opcode::ACKNOWLEDGE_ALL_ALERTS,
            0,
        )),
        Intent::SelectPage(page) => {
            out[0] = page.0;
            Ok((IntentNamespace::CORE, core_opcode::SELECT_PAGE, 1))
        }
        Intent::Declutter(enabled) => {
            out[0] = u8::from(enabled);
            Ok((IntentNamespace::CORE, core_opcode::DECLUTTER, 1))
        }
        Intent::PressSoftkey { key_id } => {
            out[0..2].copy_from_slice(&key_id.to_le_bytes());
            Ok((IntentNamespace::CORE, core_opcode::PRESS_SOFTKEY, 2))
        }
        Intent::Custom {
            namespace,
            opcode,
            payload,
        } => {
            if namespace.is_core() {
                return Err(IntentWireError::ReservedCoreNamespace);
            }
            let len = usize::from(payload.len);
            if len > MAX_EXTENSION_PAYLOAD {
                return Err(IntentWireError::PayloadTooLarge { len });
            }
            out[..len].copy_from_slice(&payload.bytes[..len]);
            Ok((namespace, opcode, len))
        }
    }
}

fn encode_altitude(
    out: &mut [u8; MAX_EXTENSION_PAYLOAD],
    meters: f32,
    class: AltitudeClass,
    origin: OriginId,
    model: GeoidModelId,
) -> Result<(IntentNamespace, u16, usize), IntentWireError> {
    check_finite(meters, "meters")?;
    if class == AltitudeClass::Unknown {
        return Err(IntentWireError::InvalidDiscriminant {
            field: "AltitudeClass",
            value: class.to_u8(),
        });
    }
    let origin_u8 =
        u8::try_from(origin.0).map_err(|_| IntentWireError::FieldOverflow { field: "origin" })?;
    out[0..4].copy_from_slice(&meters.to_le_bytes());
    out[4] = class.to_u8();
    out[5] = origin_u8;
    out[6] = model.0;
    Ok((IntentNamespace::CORE, core_opcode::SELECT_ALTITUDE, 7))
}

fn decode_core_intent(opcode: u16, payload: &[u8]) -> Result<Intent, IntentWireError> {
    match opcode {
        core_opcode::SELECT_BARO => {
            require_payload(payload, 4)?;
            let hpa = read_f32(&payload[0..4]);
            check_finite(hpa, "hpa")?;
            Ok(Intent::SelectBaro { hpa })
        }
        core_opcode::SET_HEADING_BUG => {
            require_payload(payload, 5)?;
            let rad = read_f32(&payload[0..4]);
            check_finite(rad, "rad")?;
            let reference = HeadingReference::from_u8(payload[4]);
            if reference == HeadingReference::Unknown {
                return Err(IntentWireError::InvalidDiscriminant {
                    field: "HeadingReference",
                    value: payload[4],
                });
            }
            Ok(Intent::SetHeadingBug { rad, reference })
        }
        core_opcode::SELECT_ALTITUDE => decode_core_altitude(payload),
        core_opcode::SELECT_NAV_SOURCE => {
            require_payload(payload, 1)?;
            Ok(Intent::SelectNavSource(decode_nav_source(payload[0])?))
        }
        core_opcode::ACKNOWLEDGE_ALERT => {
            require_payload(payload, 2)?;
            Ok(Intent::AcknowledgeAlert(AlertId(read_u16(&payload[0..2]))))
        }
        core_opcode::ACKNOWLEDGE_ALL_ALERTS => {
            require_payload(payload, 0)?;
            Ok(Intent::AcknowledgeAllAlerts)
        }
        core_opcode::SELECT_PAGE => {
            require_payload(payload, 1)?;
            Ok(Intent::SelectPage(PageId(payload[0])))
        }
        core_opcode::DECLUTTER => {
            require_payload(payload, 1)?;
            match payload[0] {
                0 => Ok(Intent::Declutter(false)),
                1 => Ok(Intent::Declutter(true)),
                value => Err(IntentWireError::InvalidDiscriminant {
                    field: "Declutter",
                    value,
                }),
            }
        }
        core_opcode::PRESS_SOFTKEY => {
            require_payload(payload, 2)?;
            Ok(Intent::PressSoftkey {
                key_id: read_u16(&payload[0..2]),
            })
        }
        other => Err(IntentWireError::UnknownCoreOpcode(other)),
    }
}

fn decode_core_altitude(payload: &[u8]) -> Result<Intent, IntentWireError> {
    require_payload(payload, 7)?;
    let meters = read_f32(&payload[0..4]);
    check_finite(meters, "meters")?;
    let class = AltitudeClass::from_u8(payload[4]);
    if class == AltitudeClass::Unknown {
        return Err(IntentWireError::InvalidDiscriminant {
            field: "AltitudeClass",
            value: payload[4],
        });
    }
    Ok(Intent::SelectAltitude {
        meters,
        class,
        origin: OriginId(u32::from(payload[5])),
        model: GeoidModelId(payload[6]),
    })
}

const fn encode_nav_source(source: NavSource) -> Result<u8, IntentWireError> {
    match source {
        NavSource::Nav1 => Ok(0),
        NavSource::Nav2 => Ok(1),
        NavSource::Gps => Ok(2),
        NavSource::None => Ok(3),
        NavSource::Unknown => Err(IntentWireError::InvalidDiscriminant {
            field: "NavSource",
            value: 255,
        }),
    }
}

const fn decode_nav_source(raw: u8) -> Result<NavSource, IntentWireError> {
    match raw {
        0 => Ok(NavSource::Nav1),
        1 => Ok(NavSource::Nav2),
        2 => Ok(NavSource::Gps),
        3 => Ok(NavSource::None),
        value => Err(IntentWireError::InvalidDiscriminant {
            field: "NavSource",
            value,
        }),
    }
}

fn check_finite(val: f32, field: &'static str) -> Result<(), IntentWireError> {
    if val.is_finite() {
        Ok(())
    } else {
        Err(IntentWireError::NonFinite { field })
    }
}

fn require_payload(payload: &[u8], expected: usize) -> Result<(), IntentWireError> {
    if payload.len() == expected {
        Ok(())
    } else {
        Err(IntentWireError::InvalidLength {
            expected: REQUEST_HEADER_BYTES + expected,
            actual: REQUEST_HEADER_BYTES + payload.len(),
        })
    }
}

fn read_u16(slice: &[u8]) -> u16 {
    let mut b = [0u8; 2];
    b.copy_from_slice(&slice[..2]);
    u16::from_le_bytes(b)
}

pub(super) fn read_u32(slice: &[u8]) -> u32 {
    let mut b = [0u8; 4];
    b.copy_from_slice(&slice[..4]);
    u32::from_le_bytes(b)
}

fn read_u64(slice: &[u8]) -> u64 {
    let mut b = [0u8; 8];
    b.copy_from_slice(&slice[..8]);
    u64::from_le_bytes(b)
}

fn read_f32(slice: &[u8]) -> f32 {
    let mut b = [0u8; 4];
    b.copy_from_slice(&slice[..4]);
    f32::from_le_bytes(b)
}

#[cfg(test)]
mod tests;
