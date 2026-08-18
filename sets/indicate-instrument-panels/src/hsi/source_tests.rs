#![allow(clippy::expect_used, clippy::panic)]

use std::string::String;
use std::vec::Vec;

use indicate_instrument_scene::{Cmd, SceneCmds, SceneWriter};
use indicate_instrument_state::{
    AircraftState, AirframeDisplayProfile, Candidate, FreshnessPolicy, HeadingMeasure,
    HeadingReference, IntegrityLevel, PanelData, SourceEpoch, SourceId, SourceInputs,
    SourceMonitors, SourcePolicies, SourceStep, UnusualAttitudeState, resolve_with_sources,
};

fn s(text: &str) -> String {
    String::from(text)
}

/// A heading candidate at `deg` degrees, magnetic.
fn hdg(src: u8, now: u64, deg: f32) -> Candidate<HeadingMeasure> {
    Candidate {
        source: SourceId(src),
        epoch: SourceEpoch(1),
        source_time_ms: now,
        receive_time_ms: now,
        sequence: now as u32,
        valid: true,
        integrity: IntegrityLevel::None,
        accuracy: 0.0,
        measurement: HeadingMeasure {
            heading_rad: deg.to_radians(),
            reference: HeadingReference::Magnetic,
        },
    }
}

fn frame(
    monitors: &mut SourceMonitors,
    unusual: &mut UnusualAttitudeState,
    policies: &SourcePolicies,
    heading: &[Candidate<HeadingMeasure>],
    now: u64,
) -> PanelData {
    let profile = AirframeDisplayProfile::simulator();
    let fresh = FreshnessPolicy::default();
    let state = AircraftState::default();
    let step = SourceStep {
        inputs: SourceInputs {
            heading,
            ..SourceInputs::default()
        },
        policies,
        now_ms: now,
    };
    resolve_with_sources(&state, &fresh, &profile, unusual, monitors, &step).0
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
fn heading_box_value_and_label_share_one_source_and_switch_together() {
    let policies = SourcePolicies::simulator();
    let mut monitors = SourceMonitors::new();
    let mut unusual = UnusualAttitudeState::default();

    // Primary heading (030) selected: the heading box reads 030 and the label
    // names SRC1; the secondary heading (090) never appears under this label.
    let up = [hdg(1, 0, 30.0), hdg(2, 0, 90.0)];
    let t = texts(&frame(&mut monitors, &mut unusual, &policies, &up, 0));
    assert!(
        t.contains(&s("030°")) && t.contains(&s("HDG1")),
        "authoritative heading and its label are the primary: {t:?}"
    );
    assert!(
        !t.contains(&s("090°")) && !t.contains(&s("HDG2")),
        "the box can never show the unselected source: {t:?}"
    );

    // Primary fails: heading box value AND label switch to the secondary.
    let down = [
        Candidate {
            valid: false,
            ..hdg(1, 100, 30.0)
        },
        hdg(2, 100, 90.0),
    ];
    let t = texts(&frame(&mut monitors, &mut unusual, &policies, &down, 100));
    assert!(
        t.contains(&s("090°")) && t.contains(&s("HDG2")),
        "heading value and label switched together: {t:?}"
    );
    assert!(
        !t.contains(&s("030°")) && !t.contains(&s("HDG1")),
        "the old heading and label must not linger: {t:?}"
    );
}

#[test]
fn heading_sustained_miscompare_is_annunciated() {
    let policies = SourcePolicies::simulator();
    let mut monitors = SourceMonitors::new();
    let mut unusual = UnusualAttitudeState::default();
    let mut last = Vec::new();
    for now in [0u64, 500, 1000] {
        let up = [hdg(1, now, 30.0), hdg(2, now, 90.0)];
        last = texts(&frame(&mut monitors, &mut unusual, &policies, &up, now));
    }
    assert!(
        last.contains(&s("HDG CMP")),
        "miscompare annunciated: {last:?}"
    );
    assert!(
        last.contains(&s("HDG1")),
        "still names the retained primary: {last:?}"
    );
}

/// A live heading rose with GPS guidance at the given scale.
fn nav_at_scale(scale: indicate_instrument_state::NavScale) -> PanelData {
    use indicate_instrument_state::{
        AircraftState, EstimateQuality, HeadingSample, NavData, NavFromTo, NavSource, Stamped,
        ValidFlags, resolve,
    };

    let mut state = AircraftState {
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
        ..Default::default()
    };
    state.nav = Stamped {
        data: Some(NavData {
            source: NavSource::Gps,
            course_rad: 0.35,
            course_reference: HeadingReference::Magnetic,
            cdi_dots: -1.2,
            fromto: NavFromTo::To,
            scale,
            ..NavData::default()
        }),
        age_ms: Some(10.0),
    };
    resolve(&state, &FreshnessPolicy::default())
}

/// The scale label names what a dot is worth, under the needle's own
/// gate. Two dots is two dots on the glass whatever the scale, so the
/// label is the only thing that stops one needle position from meaning
/// different distances in different phases.
#[test]
fn the_scale_label_draws_with_the_needle_and_names_the_scale() {
    use indicate_instrument_state::NavScale;

    for (scale, label) in [
        (NavScale::Enroute, "ENR"),
        (NavScale::Terminal, "TERM"),
        (NavScale::Approach, "APR"),
    ] {
        let t = texts(&nav_at_scale(scale));
        assert!(
            t.contains(&s(label)),
            "{scale:?} annunciates {label}: {t:?}"
        );
    }
}

/// An unknown scale takes the whole nav group with it: a deflection in
/// dots means nothing until the scale says what a dot is worth, so the
/// needle goes too rather than drawing at a guessed scale.
#[test]
fn an_unknown_scale_fails_the_group_rather_than_guessing() {
    use indicate_instrument_state::NavScale;

    let data = nav_at_scale(NavScale::Unknown);
    assert!(
        !data.nav.status.shows_value(),
        "an unknown scale fails the group: {:?}",
        data.nav.status
    );
    let t = texts(&data);
    for label in ["ENR", "TERM", "APR"] {
        assert!(!t.contains(&s(label)), "no scale is invented: {t:?}");
    }
}
