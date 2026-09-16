//! Shared flight guidance, projection, and envelope review cases.

use super::*;
use indicate_instrument_state::flight::{envelope::*, guidance::*};

pub(super) fn append(out: &mut Vec<Case>) {
    let extended = resolve(
        &indicate_instrument_state::abi::v8::fixtures::extended(),
        &FreshnessPolicy::default(),
    );
    let mut add = |name, note, change: fn(&mut PanelData), head| {
        let mut data = nominal();
        data.flight = extended.flight;
        change(&mut data);
        out.push(Case {
            name,
            note,
            data,
            head,
            background: "terrain",
            alerts: None,
        });
    };
    add(
        "43 Mach guidance",
        "Mach remains the active target. IAS carries the projected target and model-supplied boundaries.",
        |_| {},
        [90.0, 0.0, 0.0],
    );
    add(
        "44 Guidance off-axis",
        "Independent engagement states and the effective target remain visible beside aircraft-referenced command bars.",
        |_| {},
        [145.0, 0.0, 0.0],
    );
    add(
        "45 Stale guidance",
        "Stale guidance removes command bars, effective targets, and normal mode annunciations.",
        |d| d.flight.guidance.status = SignalStatus::Stale,
        [90.0, 0.0, 0.0],
    );
    add(
        "46 Projection mismatch",
        "A projection from another guidance computation cannot place the effective target marker.",
        |d| d.flight.speed.value.guidance_identity.sequence = 41,
        [90.0, 0.0, 0.0],
    );
    add(
        "47 Partial envelope",
        "Partial model coverage remains explicit beside the supplied boundaries.",
        |d| d.flight.speed.value.envelope.coverage = EnvelopeCoverage::Partial,
        [90.0, 0.0, 0.0],
    );
    add(
        "48 Envelope gap",
        "Two operating intervals retain their gap. The display cannot join them into one allowed band.",
        gap,
        [90.0, 0.0, 0.0],
    );
    add(
        "49 Mode reversion",
        "Producer-timed amber attention identifies a mode reversion; it does not replace an alert.",
        |d| d.flight.guidance.value.transition.reason = TransitionReason::Reversion,
        [90.0, 0.0, 0.0],
    );
    add(
        "50 Empty envelope",
        "Complete model coverage with no operating interval reports an empty allowed set.",
        |d| d.flight.speed.value.envelope.intervals = [None; MAX_ENVELOPE_INTERVALS],
        [90.0, 0.0, 0.0],
    );
    add(
        "51 AP disconnect",
        "The shared warning survives unusual-attitude declutter and remains independent of guidance availability.",
        |d| {
            d.presentation.unusual = true;
            d.flight.guidance.value.autopilot = indicate_instrument_state::FdEngagement::Off;
        },
        [145.0, 0.0, 0.0],
    );
    if let Some(last) = out.last_mut() {
        last.alerts = Some(disconnect_alert());
    }
}

fn gap(data: &mut PanelData) {
    let envelope = &mut data.flight.speed.value.envelope;
    envelope.intervals[0] = Some(EnvelopeInterval {
        kind: EnvelopeKind::Operating,
        lower: 130.0,
        upper: 140.0,
        lower_reason: BoundaryReason::Maneuver,
        upper_reason: BoundaryReason::Control,
    });
    envelope.intervals[1] = Some(EnvelopeInterval {
        kind: EnvelopeKind::Operating,
        lower: 150.0,
        upper: 161.0,
        lower_reason: BoundaryReason::Control,
        upper_reason: BoundaryReason::Mmo,
    });
}

pub(crate) fn disconnect_alert() -> indicate_alerts::AlertOutput {
    use indicate_alerts::*;
    AlertManager::new().step(
        &AlertProfile::simulator(),
        &[AlertEvent::Assert(AlertCondition::Autoflight(
            AutoflightFault::AutopilotDisconnect,
        ))],
        AlertContext {
            declutter: true,
            ..AlertContext::default()
        },
        0,
    )
}
