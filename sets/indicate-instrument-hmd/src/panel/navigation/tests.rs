#![allow(clippy::expect_used)]
use super::*;
use indicate_instrument_scene::{Cmd, SceneCmds};
use indicate_instrument_state::{AircraftState, FreshnessPolicy, resolve};
use std::vec::Vec;

fn data() -> PanelData {
    let mut data = resolve(&AircraftState::default(), &FreshnessPolicy::default());
    data.nav.status = SignalStatus::Valid;
    data.nav.course_rose_rad = Sig::valid(0.0);
    data.nav.data.source = NavSource::Nav1;
    data.nav.data.facility = NavFacility::Localizer;
    data.nav.data.sequencing = NavSequencing::Sequencing;
    data.nav.data.position_source = PositionSource::None;
    data.nav.data.position_integrity = PositionIntegrity::Normal;
    data.nav.data.course_reference = HeadingReference::True;
    data.nav.data.scale = NavScale::Approach;
    data.nav.data.fromto = NavFromTo::To;
    data.nav.data.cdi_dots = 1.0;
    data.nav.data.vdev_dots = Some(-1.0);
    data
}

fn render(data: &PanelData, compact: bool) -> Vec<u8> {
    let mut bytes = std::vec![0; 8192];
    let mut writer = SceneWriter::new(&mut bytes).expect("bounded writer");
    draw(data, compact, &mut writer).expect("navigation scene");
    let length = writer.finish();
    bytes.truncate(length);
    bytes
}

fn polylines(bytes: &[u8]) -> Vec<Vec<[f32; 2]>> {
    SceneCmds::new(bytes)
        .expect("decode")
        .filter_map(|cmd| match cmd.expect("command") {
            Cmd::Polyline { points } => Some(points.iter().collect()),
            _ => None,
        })
        .collect()
}

fn texts(bytes: &[u8]) -> Vec<&str> {
    SceneCmds::new(bytes)
        .expect("decode")
        .filter_map(|cmd| match cmd.expect("command") {
            Cmd::Text { text, .. } => Some(text),
            _ => None,
        })
        .collect()
}

#[test]
fn deviations_show_course_right_and_glideslope_above_in_both_layouts() {
    for compact in [false, true] {
        let bytes = render(&data(), compact);
        let lines = polylines(&bytes);
        let center = if compact {
            [200.0, 405.0]
        } else {
            [219.0, 549.0]
        };
        assert_eq!(lines[0][0], [center[0] + 30.0, center[1]]);
        let vcenter = if compact {
            [1000.0, 405.0]
        } else {
            [842.0, 286.0]
        };
        assert_eq!(lines[1][0], [vcenter[0], vcenter[1] - 46.0]);
        assert!(texts(&bytes).contains(&"NAV1 LOC APR TO"));
        assert!(texts(&bytes).contains(&"GS"));
    }
}

#[test]
fn gps_vertical_path_never_claims_ils_or_an_undeclared_approach_service() {
    let mut data = data();
    data.nav.data.source = NavSource::Gps;
    data.nav.data.facility = NavFacility::None;
    let bytes = render(&data, false);
    let text = texts(&bytes);
    assert!(text.contains(&"GPS APR TO"));
    assert!(text.contains(&"VDEV"));
    assert!(!text.contains(&"GS") && !text.contains(&"LPV"));
    assert!(
        SceneCmds::new(&bytes)
            .expect("decode")
            .any(|cmd| matches!(cmd,
        Ok(Cmd::Stroke { color, .. }) if color == palette::MAGENTA))
    );
}

#[test]
fn unavailable_guidance_removes_needles_instead_of_centering_them() {
    for status in [
        SignalStatus::Missing,
        SignalStatus::Stale,
        SignalStatus::Degraded,
        SignalStatus::Failed,
    ] {
        let mut data = data();
        data.nav.status = status;
        assert!(polylines(&render(&data, false)).is_empty());
    }
    let mut data = data();
    data.nav.data.fromto = NavFromTo::Off;
    assert!(polylines(&render(&data, false)).is_empty());
    data.nav.data.fromto = NavFromTo::To;
    data.nav.data.scale = NavScale::Unknown;
    assert!(polylines(&render(&data, false)).is_empty());
    data.nav.data.scale = NavScale::Approach;
    data.nav.course_rose_rad = Sig::missing();
    assert!(polylines(&render(&data, false)).is_empty());
}

#[test]
fn off_scale_needles_keep_their_sense_and_recovery_removes_navigation() {
    let mut data = data();
    for sign in [-1.0, 1.0] {
        data.nav.data.cdi_dots = sign * 20.0;
        data.nav.data.vdev_dots = Some(sign * 20.0);
        let lines = polylines(&render(&data, false));
        assert_eq!(lines[0].len(), 3);
        assert_eq!(lines[0][1], [219.0 + sign * 76.0, 549.0]);
        assert_eq!(lines[1][1], [842.0, 286.0 + sign * 95.0]);
    }
    data.nav.data.vdev_dots = None;
    assert_eq!(polylines(&render(&data, false)).len(), 1);
    data.presentation.unusual = true;
    for compact in [false, true] {
        assert!(polylines(&render(&data, compact)).is_empty());
    }
}

#[test]
fn the_label_names_a_vor_and_the_sequencing_state_and_only_a_localizer_claims_gs() {
    let mut data = data();
    data.nav.data.facility = NavFacility::Vor;
    data.nav.data.fromto = NavFromTo::From;
    data.nav.data.sequencing = NavSequencing::Hold;
    let bytes = render(&data, false);
    let text = texts(&bytes);
    assert!(text.contains(&"NAV1 VOR APR FROM HOLD"), "{text:?}");
    assert!(text.contains(&"VDEV") && !text.contains(&"GS"), "{text:?}");
    data.nav.data.sequencing = NavSequencing::Suspended;
    assert!(texts(&render(&data, false)).contains(&"NAV1 VOR APR FROM SUSP"));
}

#[test]
fn a_facility_the_source_denies_or_an_unknown_sequencing_removes_the_needles() {
    for (facility, sequencing) in [
        (NavFacility::None, NavSequencing::Sequencing),
        (NavFacility::Unknown, NavSequencing::Sequencing),
        (NavFacility::Localizer, NavSequencing::Unknown),
    ] {
        let mut data = data();
        data.nav.data.facility = facility;
        data.nav.data.sequencing = sequencing;
        assert!(
            polylines(&render(&data, false)).is_empty(),
            "{facility:?} {sequencing:?}"
        );
    }
}

/// Every text run with the fill color it was painted in.
fn colored_texts(bytes: &[u8]) -> Vec<(&str, Rgba8)> {
    let mut fill = palette::BLACK;
    let mut out = Vec::new();
    for cmd in SceneCmds::new(bytes).expect("decode") {
        match cmd.expect("command") {
            Cmd::FillColor { color } => fill = color,
            Cmd::Text { text, .. } => out.push((text, fill)),
            _ => {}
        }
    }
    out
}

fn fms(position: PositionSource, integrity: PositionIntegrity) -> PanelData {
    let mut data = data();
    data.nav.data.source = NavSource::Fms;
    data.nav.data.facility = NavFacility::None;
    data.nav.data.position_source = position;
    data.nav.data.position_integrity = integrity;
    data
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
        for compact in [false, true] {
            let bytes = render(&fms(position, PositionIntegrity::Normal), compact);
            let text = colored_texts(&bytes);
            assert!(text.contains(&("FMS", palette::MAGENTA)), "{text:?}");
            assert!(text.contains(&(word, color)), "{text:?}");
            assert!(text.contains(&("APR TO", palette::MAGENTA)), "{text:?}");
            assert!(text.iter().any(|(t, _)| *t == "VDEV"), "{text:?}");
            assert_eq!(polylines(&bytes).len(), 2, "both needles draw");
        }
    }
}

#[test]
fn the_fms_runs_are_centered_as_one_label() {
    let bytes = render(
        &fms(PositionSource::Blended, PositionIntegrity::Normal),
        false,
    );
    let runs: Vec<(&str, f32)> = SceneCmds::new(&bytes)
        .expect("decode")
        .filter_map(|cmd| match cmd.expect("command") {
            Cmd::Text { text, x, y, .. } if y == 549.0 + 28.0 => Some((text, x)),
            _ => None,
        })
        .collect();
    let advance = indicate_instrument_scene::nominal_text_width(12.0, 1);
    let half = advance * "FMS BLND APR TO".len() as f32 / 2.0;
    assert_eq!(runs.len(), 3, "{runs:?}");
    assert!((runs[0].1 - (219.0 - half)).abs() < 1e-3, "{runs:?}");
    assert!((runs[1].1 - (219.0 - half + 4.0 * advance)).abs() < 1e-3);
    assert!((runs[2].1 - (219.0 - half + 9.0 * advance)).abs() < 1e-3);
    let end = runs[2].1 + advance * "APR TO".len() as f32;
    assert!((end - (219.0 + half)).abs() < 1e-3, "{runs:?}");
}

#[test]
fn a_degraded_position_or_an_excluded_sensor_draws_a_caution_word() {
    for (integrity, word) in [
        (PositionIntegrity::Degraded, "DGRD"),
        (PositionIntegrity::SensorExcluded, "EXCL"),
    ] {
        let bytes = render(&fms(PositionSource::Visual, integrity), false);
        let text = colored_texts(&bytes);
        assert!(text.contains(&(word, safety::CAUTION_AMBER)), "{text:?}");
        assert!(text.contains(&("VIS", safety::CAUTION_AMBER)), "{text:?}");
        assert!(text.contains(&("FMS", palette::MAGENTA)), "{text:?}");
    }
    let bytes = render(
        &fms(PositionSource::Visual, PositionIntegrity::Normal),
        false,
    );
    let normal = colored_texts(&bytes);
    assert!(
        !normal.iter().any(|(t, _)| *t == "DGRD" || *t == "EXCL"),
        "{normal:?}"
    );
}

#[test]
fn a_position_the_source_cannot_have_removes_the_needles() {
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
        let mut data = fms(position, integrity);
        data.nav.data.source = source;
        assert!(
            polylines(&render(&data, false)).is_empty(),
            "{source:?} {position:?} {integrity:?}"
        );
    }
}
