//! The echo-back rule: a request is confirmed only when the state
//! reports the value back.

use indicate_instrument_feeder::clock::{age_ms, clock_ok};
use indicate_instrument_state::{NavSource, Selections};

use crate::gate::Admitted;
use crate::request::Intent;

/// The selection a request targets. Alert, page, and declutter intents
/// are not selections: the host routes them and nothing echoes them
/// through the state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionKind {
    /// The barometric setting.
    Baro,
    /// The heading bug.
    HeadingBug,
    /// The selected altitude.
    Altitude,
    /// The lateral navigation source.
    NavSource,
}

/// Every selection kind, in slot order.
pub const SELECTION_KINDS: [SelectionKind; 4] = [
    SelectionKind::Baro,
    SelectionKind::HeadingBug,
    SelectionKind::Altitude,
    SelectionKind::NavSource,
];

/// Where one selection request stands against the state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EchoStatus {
    /// No request outstanding.
    Idle,
    /// Requested, not yet echoed, within the confirmation window.
    Pending {
        /// Milliseconds since the request was admitted.
        age_ms: f64,
    },
    /// The state now reports the requested value. Reported once; the
    /// slot is idle afterwards.
    Confirmed,
    /// Requested, not echoed, and past the confirmation window: the
    /// shell annunciates it, the display never shows the requested
    /// value in its place.
    Overdue {
        /// Milliseconds since the request was admitted.
        age_ms: f64,
    },
    /// A request is outstanding and the host clock could not age it.
    /// Fail-closed: treated like overdue by any shell.
    Unmeasurable,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Outstanding {
    intent: Intent,
    admitted_at_ms: f64,
}

/// Holds admitted selection requests until the state echoes them.
#[derive(Debug)]
pub struct EchoMonitor {
    confirm_within_ms: f64,
    slots: [Option<Outstanding>; 4],
    observed_ms: Option<f64>,
}

impl EchoMonitor {
    /// A monitor that reports a request overdue once `confirm_within_ms`
    /// has passed without an echo.
    pub const fn new(confirm_within_ms: f64) -> Self {
        Self {
            confirm_within_ms,
            slots: [None; 4],
            observed_ms: None,
        }
    }

    /// Records an admitted request. A selection intent replaces any
    /// outstanding request of the same kind; other intents are ignored,
    /// because nothing echoes them.
    pub fn request(&mut self, admitted: &Admitted) {
        let Some(kind) = kind_of(&admitted.request.intent) else {
            return;
        };
        self.slots[slot(kind)] = Some(Outstanding {
            intent: admitted.request.intent,
            admitted_at_ms: admitted.admitted_at_ms,
        });
    }

    /// Compares every outstanding request with what the state reports
    /// now and returns one status per selection kind, in
    /// [`SELECTION_KINDS`] order. `nav_source` is the source the state
    /// reports selected.
    pub fn observe(
        &mut self,
        selections: &Selections,
        nav_source: NavSource,
        now_ms: f64,
    ) -> [EchoStatus; 4] {
        let clock_trusted = clock_ok(now_ms, self.observed_ms);
        if clock_trusted {
            self.observed_ms = Some(now_ms);
        }
        let mut out = [EchoStatus::Idle; 4];
        let observed_ms = self.observed_ms;
        let confirm_within_ms = self.confirm_within_ms;
        for (slot, status) in self.slots.iter_mut().zip(out.iter_mut()) {
            let Some(outstanding) = *slot else {
                continue;
            };
            if echoed(&outstanding.intent, selections, nav_source) {
                *slot = None;
                *status = EchoStatus::Confirmed;
                continue;
            }
            if !clock_trusted {
                *status = EchoStatus::Unmeasurable;
                continue;
            }
            let age = age_ms(now_ms, outstanding.admitted_at_ms, observed_ms);
            *status = if age > confirm_within_ms {
                EchoStatus::Overdue { age_ms: age }
            } else {
                EchoStatus::Pending { age_ms: age }
            };
        }
        out
    }
}

fn kind_of(intent: &Intent) -> Option<SelectionKind> {
    match intent {
        Intent::SelectBaro { .. } => Some(SelectionKind::Baro),
        Intent::SetHeadingBug { .. } => Some(SelectionKind::HeadingBug),
        Intent::SelectAltitude { .. } => Some(SelectionKind::Altitude),
        Intent::SelectNavSource(_) => Some(SelectionKind::NavSource),
        Intent::AcknowledgeAlert(_)
        | Intent::AcknowledgeAllAlerts
        | Intent::SelectPage(_)
        | Intent::Declutter(_)
        | Intent::PressSoftkey { .. }
        | Intent::Custom { .. } => None,
    }
}

const fn slot(kind: SelectionKind) -> usize {
    match kind {
        SelectionKind::Baro => 0,
        SelectionKind::HeadingBug => 1,
        SelectionKind::Altitude => 2,
        SelectionKind::NavSource => 3,
    }
}

/// Tolerances an echo is compared within: the wire carries `f32`, and
/// a source may round a setting to its own resolution.
const BARO_TOLERANCE_HPA: f32 = 0.05;
const HEADING_TOLERANCE_RAD: f32 = 0.001;
const ALTITUDE_TOLERANCE_M: f32 = 0.5;

/// Whether the state now reports what the request asked for. Identity
/// fields (reference, class, origin, model, source) must match exactly;
/// numbers match within tolerance.
fn echoed(intent: &Intent, selections: &Selections, nav_source: NavSource) -> bool {
    match *intent {
        Intent::SelectBaro { hpa } => selections
            .baro_sel_hpa
            .is_some_and(|echo| (echo - hpa).abs() <= BARO_TOLERANCE_HPA),
        Intent::SetHeadingBug { rad, reference } => {
            selections.heading_bug_reference == reference
                && (selections.heading_bug_rad - rad).abs() <= HEADING_TOLERANCE_RAD
        }
        Intent::SelectAltitude {
            meters,
            class,
            origin,
            model,
        } => {
            selections.altitude_sel_class == class
                && selections.altitude_sel_origin == origin
                && selections.altitude_sel_model == model
                && selections
                    .altitude_sel_m
                    .is_some_and(|echo| (echo - meters).abs() <= ALTITUDE_TOLERANCE_M)
        }
        Intent::SelectNavSource(source) => nav_source == source,
        Intent::AcknowledgeAlert(_)
        | Intent::AcknowledgeAllAlerts
        | Intent::SelectPage(_)
        | Intent::Declutter(_)
        | Intent::PressSoftkey { .. }
        | Intent::Custom { .. } => false,
    }
}

#[cfg(test)]
mod tests;
