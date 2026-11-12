//! The vertical pointer is phase-scaled like the needle, so it draws
//! only at a named scale and never without the scale label.

#![allow(clippy::expect_used, clippy::panic)]

use std::string::String;
use std::vec::Vec;

use indicate_instrument_scene::{Cmd, SceneCmds, SceneWriter};
use indicate_instrument_state::{
    AircraftState, EstimateQuality, FreshnessPolicy, HeadingReference, HeadingSample, NavData,
    NavFromTo, NavScale, NavSource, PanelData, Sig, Stamped, ValidFlags, resolve,
};

/// Valid GPS guidance with a vertical deviation, and a course that does
/// not convert into the rose reference, so the needle cannot draw.
fn vertical_only(scale: NavScale) -> PanelData {
    let state = AircraftState {
        heading: Stamped {
            data: Some(HeadingSample {
                heading_rad: 0.52,
                reference: HeadingReference::Magnetic,
            }),
            age_ms: Some(10.0),
        },
        quality: EstimateQuality::Good,
        valid: ValidFlags {
            heading: true,
            ..Default::default()
        },
        nav: Stamped {
            data: Some(NavData {
                source: NavSource::Gps,
                course_rad: 0.35,
                course_reference: HeadingReference::Magnetic,
                cdi_dots: -1.2,
                fromto: NavFromTo::To,
                vdev_dots: Some(0.8),
                scale,
                facility: indicate_instrument_state::NavFacility::None,
                sequencing: indicate_instrument_state::NavSequencing::Sequencing,
                ..NavData::default()
            }),
            age_ms: Some(10.0),
        },
        ..AircraftState::default()
    };
    let mut data = resolve(&state, &FreshnessPolicy::default());
    data.nav.course_rose_rad = Sig::missing();
    data
}

fn texts(data: &PanelData) -> Vec<String> {
    let mut buf = std::vec![0u8; 32 * 1024];
    let mut writer = SceneWriter::new(&mut buf).expect("buffer fits");
    super::draw_hsi(data, None, crate::BUILTIN_FRAME, &mut writer).expect("panel fits buffer");
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
fn a_vertical_pointer_without_a_needle_still_names_its_scale() {
    for (scale, label) in [
        (NavScale::Enroute, "ENR"),
        (NavScale::Terminal, "TERM"),
        (NavScale::Approach, "APR"),
    ] {
        let data = vertical_only(scale);
        assert!(data.nav.status.shows_value(), "{:?}", data.nav.status);
        assert!(
            !data.nav.course_rose_rad.status.shows_value(),
            "the point of this test is a panel with no needle"
        );
        let t = texts(&data);
        assert!(t.iter().any(|s| s == "V"), "vertical pointer drawn: {t:?}");
        assert_eq!(
            t.iter().filter(|s| *s == label).count(),
            1,
            "{scale:?} is annunciated once: {t:?}"
        );
        assert!(
            !t.iter().any(|s| s == "GPS"),
            "no needle, no receiver: {t:?}"
        );
    }
}

#[test]
fn a_valid_group_with_an_unnameable_scale_draws_no_vertical_pointer() {
    let mut data = vertical_only(NavScale::Approach);
    data.nav.data.scale = NavScale::Unknown;
    assert!(data.nav.status.shows_value());
    let t = texts(&data);
    assert!(
        !t.iter().any(|s| s == "V"),
        "no pointer without a scale: {t:?}"
    );
    for label in ["ENR", "TERM", "APR"] {
        assert!(!t.iter().any(|s| s == label), "no scale is invented: {t:?}");
    }
}
