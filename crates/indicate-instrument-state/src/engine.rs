//! Engine indications: what each engine's gauges read.
//!
//! The values are the indicated ones. Mechanism lag and instrument
//! error are the source's physics, so a simulator that models a gauge
//! writes the reading its mechanism shows, and the display paints that
//! reading without filtering it again.

use crate::validate::GroupFault;

/// The most engines one engine group carries.
pub const MAX_ENGINES: usize = 4;

/// One engine's indicated readings. Each field is optional because an
/// engine without the sensor must display `Missing`, not a substitute.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct EngineSample {
    /// Manifold absolute pressure, hectopascals.
    pub manifold_pressure_hpa: Option<f32>,
    /// Crankshaft speed, revolutions per minute.
    pub rpm: Option<f32>,
}

/// The engine group: `count` engines in panel order, engine 1 first.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct EngineIndication {
    /// How many entries of `engines` the source reports. A count above
    /// [`MAX_ENGINES`] faults the group rather than truncating it.
    pub count: u8,
    /// Per-engine readings; entries at or past `count` are not reported.
    pub engines: [EngineSample; MAX_ENGINES],
}

impl EngineIndication {
    /// Engine `index` (zero-based), or `None` when the source does not
    /// report that engine.
    #[must_use]
    pub fn engine(&self, index: usize) -> Option<&EngineSample> {
        if index < usize::from(self.count) {
            self.engines.get(index)
        } else {
            None
        }
    }
}

/// The engine group's own fault, if it has one: a count past the
/// capacity, or a reading that is not a finite, non-negative number.
pub(crate) fn engine_fault(indication: &EngineIndication) -> Option<GroupFault> {
    if usize::from(indication.count) > MAX_ENGINES {
        return Some(GroupFault::UnknownEnum);
    }
    let sound = |value: Option<f32>| value.is_none_or(|v| v.is_finite() && v >= 0.0);
    let all_sound = indication
        .engines
        .iter()
        .all(|e| sound(e.manifold_pressure_hpa) && sound(e.rpm));
    if all_sound {
        None
    } else {
        Some(GroupFault::NonFinite)
    }
}
