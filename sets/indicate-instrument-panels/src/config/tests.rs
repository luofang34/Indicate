//! The configuration panel's honesty: each scale answers for its own
//! sensor, and an absent one draws dashes rather than a centred pointer.

#![allow(clippy::expect_used, clippy::panic)]

use std::string::String;
use std::vec::Vec;

use indicate_instrument_scene::{Cmd, MAX_SCENE_BYTES, SceneCmds, SceneWriter};
use indicate_instrument_state::{
    AircraftState, AirframeConfig, EstimateQuality, FreshnessPolicy, PanelData, Stamped, resolve,
};

use super::draw_config;
use crate::BUILTIN_FRAME;

fn panel(config: Option<AirframeConfig>, age_ms: Option<f32>) -> PanelData {
    let state = AircraftState {
        airframe: Stamped {
            data: config,
            age_ms,
        },
        quality: EstimateQuality::Good,
        ..AircraftState::default()
    };
    resolve(&state, &FreshnessPolicy::default())
}

fn texts(data: &PanelData) -> Vec<String> {
    let mut buf = std::vec![0u8; MAX_SCENE_BYTES];
    let mut writer = SceneWriter::new(&mut buf).expect("writer");
    draw_config(data, None, BUILTIN_FRAME, &mut writer).expect("panel fits buffer");
    let len = writer.finish();
    SceneCmds::new(&buf[..len])
        .expect("valid scene")
        .map(|c| c.expect("valid command"))
        .filter_map(|c| match c {
            Cmd::Text { text, .. } => Some(String::from(text)),
            _ => None,
        })
        .collect()
}

#[test]
fn each_scale_answers_for_its_own_sensor() {
    // A vehicle with a flap sensor and no trim sensor shows one reading
    // and one set of dashes — not a trim pointer at neutral, which would
    // claim a trim setting nobody measured.
    let data = panel(
        Some(AirframeConfig {
            flap_ratio: Some(0.5),
            flap_selected_ratio: None,
            elevator_trim_ratio: None,
            aileron_trim_ratio: None,
            rudder_trim_ratio: None,
        }),
        Some(40.0),
    );
    let t = texts(&data);
    assert!(t.iter().any(|s| s == "50"), "the flap reading: {t:?}");
    assert!(t.iter().any(|s| s == "---"), "the trim dashes: {t:?}");
}

#[test]
fn a_missing_group_dashes_both_scales() {
    let t = texts(&panel(None, None));
    assert_eq!(
        t.iter().filter(|s| *s == "---").count(),
        2,
        "both scales dash: {t:?}"
    );
}

/// The detent is a mark beside the sensed pointer, never a substitute
/// for it. A flap in transit therefore reads as two marks apart, and one
/// that never reaches its detent reads as two marks that stay apart.
#[test]
fn a_selected_detent_does_not_move_the_sensed_pointer() {
    let mut buf = std::vec![0u8; MAX_SCENE_BYTES];
    let mut writer = SceneWriter::new(&mut buf).expect("writer");
    let data = panel(
        Some(AirframeConfig {
            flap_ratio: Some(0.25),
            flap_selected_ratio: Some(1.0),
            elevator_trim_ratio: Some(0.0),
            aileron_trim_ratio: None,
            rudder_trim_ratio: None,
        }),
        Some(40.0),
    );
    draw_config(&data, None, BUILTIN_FRAME, &mut writer).expect("panel fits buffer");
    let len = writer.finish();

    let mut marks: Vec<f32> = Vec::new();
    for command in SceneCmds::new(&buf[..len]).expect("valid scene") {
        if let Cmd::Polygon { points, .. } = command.expect("valid command") {
            let ys: Vec<f32> = points.iter().map(|p| p[1]).collect();
            if ys.len() == 3 {
                marks.push(ys.iter().copied().fold(0.0, |a, b| a + b) / 3.0);
            }
        }
    }
    // The sensed flap mark, the selected detent, and the trim pointer.
    assert_eq!(marks.len(), 3, "three marks: {marks:?}");
    let (sensed, detent) = (marks[1], marks[0]);
    assert!(
        (detent - sensed).abs() > 100.0,
        "the detent sits well away from the sensed position: {marks:?}"
    );
}

/// A group whose sample is too old draws no numerals. `Missing` and
/// `Stale` are different situations and the existing dash tests cannot
/// tell them apart, because withholding a group zeroes its data: only
/// a state that carries a value AND a status that refuses it proves the
/// gate is on the status rather than on the value.
#[test]
fn a_stale_group_draws_no_numerals_even_though_it_carries_values() {
    let stale = panel(
        Some(AirframeConfig {
            flap_ratio: Some(0.5),
            flap_selected_ratio: None,
            elevator_trim_ratio: Some(-0.2),
            aileron_trim_ratio: None,
            rudder_trim_ratio: None,
        }),
        Some(5_000.0),
    );
    assert!(
        !stale.airframe.status.shows_value(),
        "the fixture must be refused by the policy, or this proves nothing"
    );
    let t = texts(&stale);
    assert!(
        !t.iter().any(|s| s == "50" || s == "-20"),
        "no numeral survives a status that refuses the value: {t:?}"
    );
    assert_eq!(
        t.iter().filter(|s| *s == "---").count(),
        2,
        "both scales dash: {t:?}"
    );
}

/// A source that declares itself unusable takes the panel with it. The
/// shared `source-unusable` state exists to say so: values present,
/// quality unusable, and every panel must fail visibly rather than
/// render the numbers.
#[test]
fn a_source_that_declares_itself_unusable_draws_no_numerals() {
    let state = AircraftState {
        airframe: Stamped {
            data: Some(AirframeConfig {
                flap_ratio: Some(0.25),
                flap_selected_ratio: None,
                elevator_trim_ratio: Some(-0.15),
                aileron_trim_ratio: None,
                rudder_trim_ratio: None,
            }),
            age_ms: Some(40.0),
        },
        quality: EstimateQuality::Unusable,
        ..AircraftState::default()
    };
    let data = resolve(&state, &FreshnessPolicy::default());
    let t = texts(&data);
    assert!(
        !t.iter().any(|s| s == "25" || s == "-15"),
        "an untrusted source's readings are not drawn: {t:?}"
    );
}

/// Every range the group's fault checks, per field and per bound. A
/// check that only ever fired on the flap ratio would let a trim
/// pointer off the end of its scale.
#[test]
fn out_of_range_configuration_faults_the_group_per_field() {
    use indicate_instrument_state::validate_state;
    let good = AirframeConfig {
        flap_ratio: Some(0.5),
        flap_selected_ratio: Some(0.5),
        elevator_trim_ratio: Some(-0.2),
        aileron_trim_ratio: Some(0.05),
        rudder_trim_ratio: Some(0.0),
    };
    let base = |config: AirframeConfig| AircraftState {
        airframe: Stamped {
            data: Some(config),
            age_ms: Some(40.0),
        },
        ..AircraftState::default()
    };
    assert!(validate_state(&base(good)).airframe.is_none());
    for bad in [
        AirframeConfig {
            flap_ratio: Some(1.8),
            ..good
        },
        AirframeConfig {
            flap_ratio: Some(-0.1),
            ..good
        },
        AirframeConfig {
            flap_selected_ratio: Some(1.8),
            ..good
        },
        AirframeConfig {
            elevator_trim_ratio: Some(-1.5),
            ..good
        },
        AirframeConfig {
            aileron_trim_ratio: Some(1.5),
            ..good
        },
        AirframeConfig {
            rudder_trim_ratio: Some(f32::INFINITY),
            ..good
        },
    ] {
        assert!(
            validate_state(&base(bad)).airframe.is_some(),
            "must fault: {bad:?}"
        );
    }
}
