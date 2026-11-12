//! A flight-management source names the sensor that positions it, and
//! the source's own integrity report draws as a caution word. Both
//! words come from the nav group as declared; the panel infers neither.

#![allow(clippy::expect_used, clippy::panic)]

use std::string::String;
use std::vec::Vec;

use indicate_instrument_scene::{Cmd, Rgba8, SceneCmds, SceneWriter, nominal_text_width};
use indicate_instrument_state::{
    AircraftState, EstimateQuality, FreshnessPolicy, HeadingReference, HeadingSample, NavData,
    NavFacility, NavFromTo, NavScale, NavSequencing, NavSource, PanelData, PositionIntegrity,
    PositionSource, Stamped, ValidFlags, resolve,
};
use indicate_instrument_symbology::{palette, safety};

fn panel(source: NavSource, position: PositionSource, integrity: PositionIntegrity) -> PanelData {
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
                fromto: NavFromTo::To,
                vdev_dots: Some(1.0),
                scale: NavScale::Terminal,
                facility: NavFacility::None,
                sequencing: NavSequencing::Hold,
                position_source: position,
                position_integrity: integrity,
                ..NavData::default()
            }),
            age_ms: Some(10.0),
        },
        ..AircraftState::default()
    };
    resolve(&state, &FreshnessPolicy::default())
}

/// Every text run with the fill color it was painted in and its anchor
/// point.
fn runs(data: &PanelData) -> Vec<(String, Rgba8, [f32; 2])> {
    let mut buf = std::vec![0u8; 32 * 1024];
    let mut writer = SceneWriter::new(&mut buf).expect("buffer fits");
    super::draw_hsi(data, None, crate::BUILTIN_FRAME, &mut writer).expect("panel fits buffer");
    let len = writer.finish();
    let mut fill = palette::BLACK;
    let mut out = Vec::new();
    for cmd in SceneCmds::new(&buf[..len]).expect("valid scene") {
        match cmd.expect("valid command") {
            Cmd::FillColor { color } => fill = color,
            Cmd::Text { text, x, y, .. } => out.push((String::from(text), fill, [x, y])),
            _ => {}
        }
    }
    out
}

fn color_of(runs: &[(String, Rgba8, [f32; 2])], text: &str) -> Option<Rgba8> {
    runs.iter().find(|(t, ..)| t == text).map(|(_, c, _)| *c)
}

#[test]
fn a_flight_management_source_reads_fms_beside_its_position_sensor() {
    for (position, word, color) in [
        (PositionSource::Gps, "GPS", palette::MAGENTA),
        (PositionSource::Visual, "VIS", palette::MAGENTA),
        (PositionSource::Inertial, "INS", palette::MAGENTA),
        (PositionSource::Blended, "BLND", palette::MAGENTA),
        (PositionSource::DeadReckoning, "DR", safety::CAUTION_AMBER),
    ] {
        let data = panel(NavSource::Fms, position, PositionIntegrity::Normal);
        assert!(data.nav.status.shows_value(), "{position:?}");
        let r = runs(&data);
        assert_eq!(color_of(&r, "FMS"), Some(palette::MAGENTA), "{r:?}");
        assert_eq!(color_of(&r, word), Some(color), "{word}: {r:?}");
        for other in ["GPS", "VIS", "INS", "BLND", "DR", "DGRD", "EXCL"] {
            assert_eq!(
                color_of(&r, other).is_some(),
                other == word,
                "{position:?} draws exactly its own word: {r:?}"
            );
        }
    }
}

/// The steering source is never called GPS on the strength of its
/// color: a GPS receiver keeps its own label and names no position
/// sensor, and a flight-management lane never reads `GPS` alone.
#[test]
fn gps_and_fms_are_different_labels() {
    let gps = runs(&panel(
        NavSource::Gps,
        PositionSource::None,
        PositionIntegrity::Normal,
    ));
    assert_eq!(color_of(&gps, "GPS"), Some(palette::MAGENTA));
    assert_eq!(color_of(&gps, "FMS"), None, "{gps:?}");
    let fms = runs(&panel(
        NavSource::Fms,
        PositionSource::Visual,
        PositionIntegrity::Normal,
    ));
    assert_eq!(color_of(&fms, "GPS"), None, "{fms:?}");
    // The vertical pointer of a computed course is a computed path.
    assert!(color_of(&fms, "V").is_some() && color_of(&fms, "G").is_none());
}

#[test]
fn a_degraded_position_or_an_excluded_sensor_draws_a_caution_word() {
    for (integrity, word) in [
        (PositionIntegrity::Degraded, "DGRD"),
        (PositionIntegrity::SensorExcluded, "EXCL"),
    ] {
        let r = runs(&panel(NavSource::Fms, PositionSource::Blended, integrity));
        assert_eq!(color_of(&r, word), Some(safety::CAUTION_AMBER), "{r:?}");
        // The position the caution is about turns with it; the source
        // word and the sequencing word keep the source color.
        assert_eq!(color_of(&r, "BLND"), Some(safety::CAUTION_AMBER));
        assert_eq!(color_of(&r, "FMS"), Some(palette::MAGENTA));
        assert_eq!(color_of(&r, "HOLD"), Some(palette::MAGENTA));
    }
    let normal = runs(&panel(
        NavSource::Fms,
        PositionSource::Blended,
        PositionIntegrity::Normal,
    ));
    assert!(color_of(&normal, "DGRD").is_none() && color_of(&normal, "EXCL").is_none());
}

#[test]
fn a_position_the_source_cannot_have_draws_no_needle_and_no_label() {
    for (source, position, integrity) in [
        (
            NavSource::Fms,
            PositionSource::None,
            PositionIntegrity::Normal,
        ),
        (
            NavSource::Fms,
            PositionSource::Unknown,
            PositionIntegrity::Normal,
        ),
        (
            NavSource::Fms,
            PositionSource::Visual,
            PositionIntegrity::Unknown,
        ),
        (
            NavSource::Gps,
            PositionSource::Visual,
            PositionIntegrity::Normal,
        ),
        (
            NavSource::Gps,
            PositionSource::None,
            PositionIntegrity::Degraded,
        ),
    ] {
        let data = panel(source, position, integrity);
        assert!(!data.nav.status.shows_value(), "{source:?} {position:?}");
        let r = runs(&data);
        for word in ["FMS", "GPS", "VIS", "DGRD", "HOLD"] {
            assert!(color_of(&r, word).is_none(), "{word}: {r:?}");
        }
    }
}

/// The two runs occupy the box a one-run label of the same length
/// occupies, so the rose clearance of the receiver label covers them.
#[test]
fn the_two_runs_are_centered_as_one_label() {
    let size = super::cdi::RECEIVER_LABEL_SIZE;
    let (cx, cy) = super::cdi::RECEIVER_LABEL_POS;
    for (position, word) in [
        (PositionSource::DeadReckoning, "DR"),
        (PositionSource::Blended, "BLND"),
    ] {
        let r = runs(&panel(NavSource::Fms, position, PositionIntegrity::Normal));
        let at = |text: &str| r.iter().find(|(t, ..)| t == text).expect("drawn").2;
        let chars = "FMS ".len() + word.len();
        let half = nominal_text_width(size, chars) / 2.0;
        let fms = at("FMS");
        let sensor = at(word);
        assert!((fms[0] - (cx - half)).abs() < 1e-3, "{fms:?}");
        let end = sensor[0] + nominal_text_width(size, word.len());
        assert!((end - (cx + half)).abs() < 1e-3, "{sensor:?}");
        assert_eq!((fms[1], sensor[1]), (cy, cy));
    }
}

/// The integrity word clears the rose rim, the vertical deviation
/// scale, and the sequencing annunciation above it.
#[test]
fn the_integrity_word_clears_the_rim_the_vertical_scale_and_the_sequencing_word() {
    let size = super::cdi::INTEGRITY_LABEL_SIZE;
    let half_w = nominal_text_width(size, 4) / 2.0;
    let (x, y) = super::cdi::INTEGRITY_LABEL_POS;
    let dx = (x - half_w) - super::CX;
    let dy = (y - size / 2.0) - super::CY;
    let gap = libm::sqrtf(dx * dx + dy * dy) - (super::ROSE_R + super::rose::REFERENCE_MARK_LEN);
    assert!(gap > 0.0, "the integrity word overlaps the rim by {gap}");
    assert!(x + half_w < 452.0 - 7.0, "it runs into the vertical scale");
    let (_, sy) = super::cdi::SEQUENCING_LABEL_POS;
    let separation = (y - size / 2.0) - (sy + super::cdi::SEQUENCING_LABEL_SIZE / 2.0);
    assert!(separation > 0.0, "it overlaps the sequencing word");
}
