//! Admission of operator requests: identity, order, clock, and bounds.

use core::f32::consts::TAU;

use indicate_instrument_feeder::clock::clock_ok;
use indicate_instrument_feeder::serial_is_newer;
use indicate_instrument_state::{AltitudeClass, HeadingReference, NavSource};

use crate::error::IntentFault;
use crate::namespace::{IntentNamespace, MAX_EXTENSION_PAYLOAD};
use crate::request::{
    BARO_HPA_MAX, BARO_HPA_MIN, Intent, Request, SELECT_ALTITUDE_M_MAX, SELECT_ALTITUDE_M_MIN,
};

/// A request the gate admitted, with the host clock reading it was
/// admitted at. The host routes it by intent; nothing here acts on it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Admitted {
    /// The admitted request, unchanged.
    pub request: Request,
    /// Host clock, milliseconds, at admission.
    pub admitted_at_ms: f64,
}

/// Admits requests from one pinned input device in serial order.
#[derive(Debug, Default)]
pub struct IntentGate {
    device: Option<u32>,
    sequence: Option<u32>,
    observed_ms: Option<f64>,
}

impl IntentGate {
    /// A gate that pins the first device it hears from.
    pub const fn new() -> Self {
        Self {
            device: None,
            sequence: None,
            observed_ms: None,
        }
    }

    /// A gate pinned to `device_id` before the first request.
    pub const fn pinned(device_id: u32) -> Self {
        Self {
            device: Some(device_id),
            sequence: None,
            observed_ms: None,
        }
    }

    /// Admits one request or refuses it with a typed fault. Every
    /// refusal leaves the gate as it was: a refused request never
    /// advances the sequence or the clock.
    pub fn admit(&mut self, request: &Request, now_ms: f64) -> Result<Admitted, IntentFault> {
        if !clock_ok(now_ms, self.observed_ms) {
            return Err(IntentFault::InvalidClock);
        }
        if let Some(pinned) = self.device
            && pinned != request.stamp.device_id
        {
            return Err(IntentFault::WrongDevice {
                pinned,
                found: request.stamp.device_id,
            });
        }
        if let Some(current) = self.sequence
            && !serial_is_newer(request.stamp.sequence, current)
        {
            return Err(IntentFault::Replay {
                current,
                found: request.stamp.sequence,
            });
        }
        check_bounds(&request.intent)?;
        self.device = Some(request.stamp.device_id);
        self.sequence = Some(request.stamp.sequence);
        self.observed_ms = Some(now_ms);
        Ok(Admitted {
            request: *request,
            admitted_at_ms: now_ms,
        })
    }
}

/// The value rules, in one place: finite, inside the declared bounds,
/// and against a reference the state can name.
fn check_bounds(intent: &Intent) -> Result<(), IntentFault> {
    match *intent {
        Intent::SelectBaro { hpa } => {
            bounded("barometric setting", hpa, BARO_HPA_MIN, BARO_HPA_MAX)
        }
        Intent::SetHeadingBug { rad, reference } => {
            if reference == HeadingReference::Unknown {
                return Err(IntentFault::UnknownReference {
                    what: "heading reference",
                });
            }
            if !rad.is_finite() {
                return Err(IntentFault::NonFinite {
                    what: "heading bug",
                });
            }
            if !(0.0..TAU).contains(&rad) {
                return Err(IntentFault::OutOfRange {
                    what: "heading bug",
                });
            }
            Ok(())
        }
        Intent::SelectAltitude { meters, class, .. } => {
            if class == AltitudeClass::Unknown {
                return Err(IntentFault::UnknownReference {
                    what: "altitude class",
                });
            }
            bounded(
                "selected altitude",
                meters,
                SELECT_ALTITUDE_M_MIN,
                SELECT_ALTITUDE_M_MAX,
            )
        }
        Intent::SelectNavSource(source) => {
            if source == NavSource::Unknown {
                return Err(IntentFault::UnknownReference {
                    what: "navigation source",
                });
            }
            Ok(())
        }
        Intent::AcknowledgeAlert(_)
        | Intent::AcknowledgeAllAlerts
        | Intent::SelectPage(_)
        | Intent::Declutter(_)
        | Intent::PressSoftkey { .. } => Ok(()),
        Intent::Custom {
            namespace, payload, ..
        } => {
            if namespace == IntentNamespace::CORE {
                return Err(IntentFault::ReservedCoreNamespace);
            }
            if usize::from(payload.len) > MAX_EXTENSION_PAYLOAD {
                return Err(IntentFault::OutOfRange {
                    what: "extension payload",
                });
            }
            Ok(())
        }
    }
}

fn bounded(what: &'static str, value: f32, min: f32, max: f32) -> Result<(), IntentFault> {
    if !value.is_finite() {
        return Err(IntentFault::NonFinite { what });
    }
    if !(min..=max).contains(&value) {
        return Err(IntentFault::OutOfRange { what });
    }
    Ok(())
}

#[cfg(test)]
mod tests;
