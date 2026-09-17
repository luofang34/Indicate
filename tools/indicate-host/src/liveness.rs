//! The progress watchdog: a frame must be made within the deadline,
//! whatever the data did (AIR-IN-013, REN-04).

use indicate_alerts::{AlertCondition, AlertEvent, DisplayFault};
use indicate_instrument_feeder::clock::age_ms;

/// Watches how often a frame is made. Data freshness is a different
/// question, answered per signal by the freshness policy; this only
/// asks whether the renderer keeps producing.
#[derive(Debug)]
pub struct Liveness {
    deadline_ms: f64,
    last_painted_ms: Option<f64>,
    stalled: bool,
}

impl Liveness {
    /// A watchdog that reports a stall once `deadline_ms` pass without
    /// a completed frame.
    pub const fn new(deadline_ms: f64) -> Self {
        Self {
            deadline_ms,
            last_painted_ms: None,
            stalled: false,
        }
    }

    /// Called on every paint-clock tick, before the frame is attempted.
    /// Returns the alert event to feed the manager when the stall state
    /// changes: an assertion when the deadline has passed since the last
    /// completed frame, nothing while the state is unchanged.
    pub fn tick(&mut self, now_ms: f64) -> Option<AlertEvent> {
        let last = self.last_painted_ms?;
        let age = age_ms(now_ms, last, Some(last));
        let overdue = age.is_nan() || age > self.deadline_ms;
        if overdue && !self.stalled {
            self.stalled = true;
            return Some(AlertEvent::Assert(AlertCondition::Display(
                DisplayFault::RendererStalled,
            )));
        }
        None
    }

    /// Called when a frame was completed. Returns the clearing event if
    /// a stall was standing.
    pub fn painted(&mut self, now_ms: f64) -> Option<AlertEvent> {
        self.last_painted_ms = Some(now_ms);
        if self.stalled {
            self.stalled = false;
            return Some(AlertEvent::Clear(AlertCondition::Display(
                DisplayFault::RendererStalled,
            )));
        }
        None
    }

    /// Whether a stall is currently standing.
    pub const fn stalled(&self) -> bool {
        self.stalled
    }
}
