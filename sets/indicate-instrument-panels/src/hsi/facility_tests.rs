//! The receiver label names what the receiver is tuned to, and the
//! sequencing annunciation says when the source holds or has suspended
//! sequencing. Both draw under the needle's gate and keep clear of the
//! rose and of the vertical deviation scale.

#![allow(clippy::expect_used, clippy::panic)]

use std::string::String;
use std::vec::Vec;

use indicate_instrument_scene::{Cmd, SceneCmds, SceneWriter, nominal_text_width};
use indicate_instrument_state::{
    AircraftState, EstimateQuality, FreshnessPolicy, HeadingReference, HeadingSample, NavData,
    NavFacility, NavFromTo, NavScale, NavSequencing, NavSource, PanelData, Stamped, ValidFlags,
    resolve,
};

fn panel(source: NavSource, facility: NavFacility, sequencing: NavSequencing) -> PanelData {
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
                source,
                course_rad: 0.35,
                course_reference: HeadingReference::Magnetic,
                cdi_dots: -1.2,
                fromto: NavFromTo::From,
                vdev_dots: Some(2.5),
                scale: NavScale::Terminal,
                facility,
                sequencing,
                ..NavData::default()
            }),
            age_ms: Some(10.0),
        },
        ..AircraftState::default()
    };
    resolve(&state, &FreshnessPolicy::default())
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
fn the_receiver_label_names_the_facility() {
    for (source, facility, label) in [
        (NavSource::Gps, NavFacility::None, "GPS"),
        (NavSource::Nav1, NavFacility::Vor, "NAV1 VOR"),
        (NavSource::Nav2, NavFacility::Localizer, "NAV2 LOC"),
    ] {
        let data = panel(source, facility, NavSequencing::Sequencing);
        assert!(data.nav.status.shows_value(), "{facility:?}");
        let t = texts(&data);
        assert!(t.iter().any(|s| s == label), "{label}: {t:?}");
    }
}

#[test]
fn a_facility_the_source_denies_draws_no_needle_and_no_label() {
    let data = panel(
        NavSource::Gps,
        NavFacility::Localizer,
        NavSequencing::Sequencing,
    );
    assert!(!data.nav.status.shows_value(), "{:?}", data.nav.status);
    let t = texts(&data);
    assert!(!t.iter().any(|s| s.starts_with("GPS")), "{t:?}");
}

#[test]
fn hold_and_suspension_are_annunciated_with_the_flag() {
    for (sequencing, label) in [
        (NavSequencing::Hold, Some("HOLD")),
        (NavSequencing::Suspended, Some("SUSP")),
        (NavSequencing::Sequencing, None),
    ] {
        let t = texts(&panel(NavSource::Gps, NavFacility::None, sequencing));
        for shown in ["HOLD", "SUSP"] {
            assert_eq!(
                t.iter().any(|s| s == shown),
                label == Some(shown),
                "{sequencing:?}: {t:?}"
            );
        }
    }
}

#[test]
fn the_annunciation_goes_with_the_needle() {
    let mut data = panel(NavSource::Gps, NavFacility::None, NavSequencing::Hold);
    data.nav.course_rose_rad = indicate_instrument_state::Sig::missing();
    let t = texts(&data);
    assert!(!t.iter().any(|s| s == "HOLD"), "no needle, no HOLD: {t:?}");
}

/// The annunciation clears the rose rim, and the vertical deviation
/// pointer at full scale. Stated as a test so that growing either
/// fails here rather than overlapping the label.
#[test]
fn the_sequencing_label_clears_the_rim_and_the_vertical_scale() {
    let half_w = nominal_text_width(super::cdi::SEQUENCING_LABEL_SIZE, 4) / 2.0;
    let half_h = super::cdi::SEQUENCING_LABEL_SIZE / 2.0;
    let (x, y) = super::cdi::SEQUENCING_LABEL_POS;
    // The label sits below and right of the rose center, so its
    // top-left corner is nearest.
    let dx = (x - half_w) - super::CX;
    let dy = (y - half_h) - super::CY;
    let gap = libm::sqrtf(dx * dx + dy * dy) - (super::ROSE_R + super::rose::REFERENCE_MARK_LEN);
    assert!(gap > 0.0, "the sequencing label overlaps the rim by {gap}");
    // The vertical pointer at x=452 pegs 2.5 dots (96 units) below
    // center and is 7 units wide either side of its axis.
    assert!(
        x + half_w < 452.0 - 7.0,
        "the sequencing label runs into the vertical deviation scale"
    );
}
