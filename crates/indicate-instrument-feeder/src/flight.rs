//! Transport-independent flight report admission with source pinning and acquisition age preservation.

use crate::clock::clock_ok;
use crate::serial_is_newer;
use indicate_instrument_state::{
    Stamped,
    flight::{FlightInputs, SolutionId},
};

/// One producer publication, delivered with ages measured at host receipt.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlightReport {
    /// Source and wrapping publication identity, independent of the guidance computation.
    pub identity: SolutionId,
    /// Guidance and speed inputs with acquisition ages at receipt.
    pub inputs: FlightInputs,
}

/// Admission outcome; rejected publications never refresh sample ages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlightAdmission {
    /// A newer publication from the pinned source was stored.
    Accepted,
    /// The publication repeats or precedes the stored publication.
    Replay,
    /// The publication belongs to another producer.
    DifferentSource,
    /// The producer identity is undeclared.
    UndeclaredSource,
    /// Host time is nonfinite, negative, or moves backward.
    InvalidClock,
}

/// Pins one authenticated producer for an explicitly created transport session.
#[derive(Debug, Default)]
pub struct FlightIngress {
    last: Option<FlightReport>,
    received_ms: f64,
    observed_ms: Option<f64>,
}

impl FlightIngress {
    /// Creates an empty session. The host authenticates the producer before calling this interface.
    pub const fn new() -> Self {
        Self {
            last: None,
            received_ms: 0.0,
            observed_ms: None,
        }
    }

    /// Stores a newer publication without changing the supplied acquisition ages.
    pub fn observe(&mut self, report: FlightReport, now_ms: f64) -> FlightAdmission {
        if !clock_ok(now_ms, self.observed_ms) {
            return FlightAdmission::InvalidClock;
        }
        self.observed_ms = Some(now_ms);
        if !report.identity.is_declared() {
            return FlightAdmission::UndeclaredSource;
        }
        if let Some(last) = self.last {
            if last.identity.source != report.identity.source {
                return FlightAdmission::DifferentSource;
            }
            let newer = if last.identity.epoch == report.identity.epoch {
                serial_is_newer(report.identity.sequence, last.identity.sequence)
            } else {
                serial_is_newer(report.identity.epoch, last.identity.epoch)
            };
            if !newer {
                return FlightAdmission::Replay;
            }
        }
        self.last = Some(report);
        self.received_ms = now_ms;
        FlightAdmission::Accepted
    }

    /// Returns aged inputs; invalid clocks invalidate ages without reviving legacy inputs.
    pub fn inputs_may_be_stale(&self, now_ms: f64) -> FlightInputs {
        let Some(report) = self.last else {
            return FlightInputs::default();
        };
        let elapsed = clock_ok(now_ms, Some(self.observed_ms.unwrap_or(self.received_ms)))
            .then_some(now_ms - self.received_ms);
        FlightInputs {
            guidance: aged(report.inputs.guidance, elapsed),
            speed: aged(report.inputs.speed, elapsed),
        }
    }
}

fn aged<T>(sample: Stamped<T>, elapsed: Option<f64>) -> Stamped<T> {
    let age_ms = sample.age_ms.zip(elapsed).and_then(|(age, elapsed)| {
        let total = f64::from(age) + elapsed;
        (age.is_finite() && age >= 0.0 && total <= f64::from(f32::MAX)).then_some(total as f32)
    });
    Stamped {
        data: sample.data,
        age_ms,
    }
}

#[cfg(test)]
mod tests;
