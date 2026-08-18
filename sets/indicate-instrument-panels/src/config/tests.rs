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
