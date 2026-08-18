#![allow(clippy::expect_used, clippy::panic)]

use std::string::String;
use std::vec::Vec;

use indicate_instrument_scene::{Cmd, LayerId, Rgba8, SceneCmds, SceneWriter};
use indicate_instrument_state::{
    AircraftState, AirframeDisplayProfile, Candidate, FreshnessPolicy, GroupId, HeadingMeasure,
    HeadingReference, IntegrityLevel, NavData, NavFromTo, NavResolved, NavSource, PanelData, Sig,
    SignalStatus, SourceEpoch, SourceId, SourceInputs, SourceMonitors, SourcePolicies, SourceStep,
    UnusualAttitudeState, resolve_with_sources,
};
use indicate_instrument_symbology::palette;

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

// ---- nav source identity (#55) ----------------------------------------------

/// A live heading rose resolved from a real heading sample, so the
/// rose basis is resolve's own selection; nav guidance is unset.
fn heading_only_panel() -> PanelData {
    let state = AircraftState {
        heading: indicate_instrument_state::Stamped {
            data: Some(indicate_instrument_state::HeadingSample {
                heading_rad: 0.52,
                reference: HeadingReference::Magnetic,
            }),
            age_ms: Some(10.0),
        },
        quality: indicate_instrument_state::EstimateQuality::Good,
        valid: indicate_instrument_state::ValidFlags {
            heading: true,
            ..Default::default()
        },
        ..Default::default()
    };
    indicate_instrument_state::resolve(&state, &FreshnessPolicy::default())
}

/// `heading_only_panel` plus nav guidance from `source`.
fn with_nav(source: NavSource) -> PanelData {
    let mut data = heading_only_panel();
    data.nav = NavResolved {
        data: NavData {
            source,
            course_rad: 0.35,
            course_reference: HeadingReference::Magnetic,
            cdi_dots: -1.2,
            fromto: NavFromTo::To,
            ..NavData::default()
        },
        status: SignalStatus::Valid,
        course_rose_rad: Sig::with_status(0.35, SignalStatus::Valid),
    };
    data
}

/// What one frame shows about the selected nav source: every text run
/// with the fill color active at its emission and its provenance claim,
/// plus the fill colors of the filled polygons in the Guidance band
/// (the CDI's arrow, bar, and TO/FROM triangle wear the source color).
struct SourceInk {
    texts: Vec<(String, Rgba8, Option<u8>)>,
    guidance_fills: Vec<Rgba8>,
}

fn source_ink(data: &PanelData) -> SourceInk {
    let mut buf = std::vec![0u8; 32 * 1024];
    let mut writer = SceneWriter::new(&mut buf).expect("buffer fits");
    super::draw_hsi(data, None, crate::BUILTIN_FRAME, &mut writer).expect("panel fits buffer");
    let len = writer.finish();
    let mut layer = None;
    let mut fill = None;
    let mut claim = None;
    let mut ink = SourceInk {
        texts: Vec::new(),
        guidance_fills: Vec::new(),
    };
    for command in SceneCmds::new(&buf[..len]).expect("valid scene") {
        match command.expect("valid command") {
            Cmd::BeginLayer { layer: id } => layer = Some(id),
            Cmd::EndLayer { .. } => layer = None,
            Cmd::FillColor { color } => fill = Some(color),
            Cmd::Attribute { group } => claim = Some(group),
            Cmd::Text { text, .. } => {
                ink.texts.push((
                    String::from(text),
                    fill.expect("text has a fill"),
                    claim.take(),
                ));
            }
            Cmd::Polygon { mode, .. }
                if layer == Some(LayerId::Guidance)
                    && mode != indicate_instrument_scene::PaintMode::Stroke =>
            {
                ink.guidance_fills.push(fill.expect("polygon has a fill"));
            }
            _ => {}
        }
    }
    ink
}

#[test]
fn nav_source_label_cdi_and_course_box_switch_together() {
    let cases = [
        (NavSource::Gps, "GPS", palette::MAGENTA),
        (NavSource::Nav1, "NAV1", palette::GREEN),
        (NavSource::Nav2, "NAV2", palette::GREEN),
    ];
    let nav = GroupId::Nav.to_u8();
    for (source, label, color) in cases {
        let ink = source_ink(&with_nav(source));
        // The label names the receiver, in the source color, claiming
        // the nav group its identity derives from.
        assert!(
            ink.texts.contains(&(s(label), color, Some(nav))),
            "{label} must paint in its source color with the nav claim: {:?}",
            ink.texts
        );
        // The CRS-box value wears the same color.
        assert!(
            ink.texts.iter().any(|(t, c, _)| t == "020°" && *c == color),
            "course box value follows the source: {:?}",
            ink.texts
        );
        // The CDI ink wears the same color.
        assert!(
            ink.guidance_fills.contains(&color),
            "CDI polygon ink follows the source: {:?}",
            ink.guidance_fills
        );
        // No other receiver's name lingers in the same frame.
        for (_, other, _) in cases.iter().filter(|(s_, _, _)| *s_ != source) {
            assert!(
                !ink.texts.iter().any(|(t, _, _)| t == other),
                "{other} must not paint while {label} is selected: {:?}",
                ink.texts
            );
        }
    }

    // The switch itself: GPS to Nav1 moves all three in the same frame —
    // no magenta needle, no GPS label, no magenta course value survives.
    let after = source_ink(&with_nav(NavSource::Nav1));
    assert!(
        !after.guidance_fills.contains(&palette::MAGENTA),
        "the old source's needle color must not linger: {:?}",
        after.guidance_fills
    );
    assert!(
        !after
            .texts
            .iter()
            .any(|(t, c, _)| t == "GPS" || (t == "020°" && *c == palette::MAGENTA)),
        "the old source's label and course color must not linger: {:?}",
        after.texts
    );
}

#[test]
fn the_source_label_shares_the_cdi_gate() {
    // No source selected: no needle, no label.
    let t = texts(&heading_only_panel());
    for label in ["GPS", "NAV1", "NAV2"] {
        assert!(
            !t.contains(&s(label)),
            "{label} without a selected source: {t:?}"
        );
    }

    // The course cannot convert into the rose reference: the CDI gate
    // closes and the label goes with the needle.
    let mut data = with_nav(NavSource::Gps);
    data.nav.course_rose_rad = Sig::with_status(0.0, SignalStatus::Failed);
    let t = texts(&data);
    assert!(
        !t.contains(&s("GPS")),
        "an unconvertible course shows neither needle nor label: {t:?}"
    );

    // The nav group fails: same gate, same outcome.
    let mut data = with_nav(NavSource::Nav1);
    data.nav.status = SignalStatus::Failed;
    let t = texts(&data);
    assert!(
        !t.contains(&s("NAV1")),
        "a failed nav group shows neither needle nor label: {t:?}"
    );
}
