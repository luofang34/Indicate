//! Review profiles exercise production policy with explicit visibility evidence.
use crate::{Error, cases::Case, scene};
use indicate_instrument_hmd::*;
use indicate_instrument_state::{PanelData, Sig, SignalStatus, flight::presentation};
use std::collections::{HashMap, HashSet};

const NAMES: [&str; 6] = [
    "Primary / flight",
    "Primary / reduced",
    "Supplemental / mission reservation",
    "Supplemental / confirmed HUD",
    "Supplemental / confirmed cockpit",
    "Supplemental / stale visibility",
];

pub(super) fn profile_names() -> String {
    NAMES
        .iter()
        .map(|name| format!("{name:?}"))
        .collect::<Vec<_>>()
        .join(",")
}

pub(super) fn contexts() -> [DisplayContext; 6] {
    let normal = DisplayContext::default();
    let supplemental = DisplayContext {
        role: DisplayRole::Supplemental,
        ..normal
    };
    let hud = DisplayContext {
        region: ViewRegion::Hud,
        alternate_flight_display: Sig::valid(true),
        ..supplemental
    };
    [
        normal,
        DisplayContext {
            detail: DetailLevel::Reduced,
            ..normal
        },
        DisplayContext {
            task: DisplayTask::Mission,
            ..supplemental
        },
        hud,
        DisplayContext {
            region: ViewRegion::Cockpit,
            ..hud
        },
        DisplayContext {
            alternate_flight_display: Sig::with_status(true, SignalStatus::Stale),
            ..hud
        },
    ]
}

#[derive(Default)]
pub(super) struct Gallery {
    pub shapes: Vec<String>,
    indices: HashMap<String, usize>,
}

impl Gallery {
    fn intern(&mut self, shape: String) -> usize {
        if let Some(index) = self.indices.get(&shape) {
            return *index;
        }
        let index = self.shapes.len();
        self.shapes.push(shape.clone());
        self.indices.insert(shape, index);
        index
    }

    pub(super) fn encode_case(&mut self, case: &Case) -> Result<String, Error> {
        let frame = view_reference(&case.data);
        let angular = directions(&case.data);
        let lines = angular.strokes[..angular.length as usize]
            .iter()
            .map(|line| format!("[{:?},{:?}]", line.a, line.b))
            .collect::<Vec<_>>()
            .join(",");
        let profiles = contexts()
            .into_iter()
            .map(|context| self.profile(case, context))
            .collect::<Result<Vec<_>, _>>()?
            .join(",");
        Ok(format!(
            "{{\"name\":{:?},\"note\":{:?},\"head\":{:?},\"background\":{:?},\"profiles\":[{profiles}],\"basis\":[{:?},{:?},{:?}],\"kind\":{},\"recovery\":{},\"world\":[{lines}],\"attentionMs\":{}}}",
            case.name,
            case.note,
            case.head,
            case.background,
            frame.right,
            frame.up,
            frame.forward,
            frame.kind,
            case.data.presentation.unusual,
            attention_remaining(&case.data),
        ))
    }

    fn profile(&mut self, case: &Case, context: DisplayContext) -> Result<String, Error> {
        let plan = presentation_plan(&case.data, context, case.alerts.as_ref());
        let forward = self.view(case, context, false)?;
        let compact = self.view(case, context, true)?;
        Ok(format!(
            "{{\"forward\":{forward},\"compact\":{compact},\"blank\":{},\"mission\":{}}}",
            plan.blank_flight, plan.mission
        ))
    }

    fn view(
        &mut self,
        case: &Case,
        context: DisplayContext,
        compact: bool,
    ) -> Result<String, Error> {
        let shapes = scene::encode(&case.data, case.alerts.as_ref(), context, compact)?;
        let mut expired = case.data;
        expired.flight.guidance.value.transition.elapsed_ms = Some(5000.0);
        let settled: HashSet<_> = scene::encode(&expired, case.alerts.as_ref(), context, compact)?
            .into_iter()
            .collect();
        let mut steady = Vec::new();
        let mut attention = Vec::new();
        for shape in shapes {
            let stable = settled.contains(&shape);
            let index = self.intern(shape);
            if stable {
                steady.push(index);
            } else {
                attention.push(index);
            }
        }
        Ok(format!(
            "{{\"steady\":{steady:?},\"attention\":{attention:?}}}"
        ))
    }
}

fn attention_remaining(data: &PanelData) -> f32 {
    if !presentation::transition_visible(data, 5000.0) {
        return 0.0;
    }
    let elapsed = data
        .flight
        .guidance
        .value
        .transition
        .elapsed_ms
        .unwrap_or(5000.0);
    let age = data.flight.guidance_age_ms.unwrap_or(5000.0);
    (5000.0 - elapsed - age).max(0.0)
}

pub(super) fn zones() -> String {
    [
        ("AUTOMATION", AUTOMATION_ZONE),
        ("ALERTS", ALERT_ZONE),
        ("EXTERNAL VIEW / CONFORMAL CUES", CENTRAL_VIEW_ZONE),
        ("MISSION STATUS RESERVATION", MISSION_ZONE),
        ("HOST CONTEXT", HOST_CONTEXT_ZONE),
    ]
    .into_iter()
    .map(|(label, zone)| {
        format!(
            "{{\"label\":{label:?},\"bounds\":[{},{},{},{}]}}",
            zone.x, zone.y, zone.width, zone.height
        )
    })
    .collect::<Vec<_>>()
    .join(",")
}
