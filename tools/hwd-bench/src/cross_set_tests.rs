//! Fixed and head-worn displays preserve the same automation meaning.
#![allow(clippy::expect_used, clippy::panic)]

use indicate_alerts::AlertOutput;
use indicate_instrument_descriptor::{
    DrawContext, EMPTY_CONFIG, PanelDescriptor, RenderSpace, SceneTarget,
};
use indicate_instrument_hmd::{HMD_DESCRIPTOR, HMD_GLANCE_DESCRIPTOR};
use indicate_instrument_panels::{AUTOFLIGHT_DESCRIPTOR, PFD_DESCRIPTOR};
use indicate_instrument_scene::{
    Cmd, LayerId, PaintMode, SceneCmds, SceneWriter,
    angular::{AngularSceneWriter, MAX_ANGULAR_SCENE_BYTES},
};
use indicate_instrument_state::{
    FreshnessPolicy, PanelData, SignalStatus, abi::v8::fixtures, resolve,
};

fn render(panel: &PanelDescriptor, data: &PanelData, alerts: Option<&AlertOutput>) -> Vec<u8> {
    let ctx = DrawContext::new(data)
        .with_config(EMPTY_CONFIG)
        .with_alerts(alerts);
    let mut storage = vec![0; 65536];
    let mut ang_buf = vec![0u8; MAX_ANGULAR_SCENE_BYTES];
    let mut writer = SceneWriter::new(&mut storage).expect("buffer");
    match panel.space {
        RenderSpace::Planar2D { frame_min, .. } => {
            (panel.draw)(
                &ctx,
                SceneTarget::Planar2D {
                    frame: frame_min,
                    writer: &mut writer,
                },
            )
            .expect("planar panel");
        }
        RenderSpace::AngularSphere {
            overlay_frame: Some(frame),
        } => {
            let mut ang_writer = AngularSceneWriter::new(&mut ang_buf).expect("angular buffer");
            (panel.draw)(
                &ctx,
                SceneTarget::AngularSphere {
                    angular: &mut ang_writer,
                    overlay: Some((frame, &mut writer)),
                },
            )
            .expect("angular panel");
            let _ang_used = ang_writer.finish();
        }
        _ => panic!("unsupported space in cross_set_tests: {}", panel.id),
    }
    let size = writer.finish();
    storage.truncate(size);
    storage
}

fn texts(bytes: &[u8]) -> Vec<&str> {
    SceneCmds::new(bytes)
        .expect("scene")
        .filter_map(|cmd| match cmd.expect("command") {
            Cmd::Text { text, .. } => Some(text),
            _ => None,
        })
        .collect()
}

fn bars(bytes: &[u8]) -> usize {
    let mut guidance = false;
    SceneCmds::new(bytes)
        .expect("scene")
        .filter(|cmd| match cmd.as_ref().expect("command") {
            Cmd::BeginLayer {
                layer: LayerId::Guidance,
            } => {
                guidance = true;
                false
            }
            Cmd::EndLayer {
                layer: LayerId::Guidance,
            } => {
                guidance = false;
                false
            }
            Cmd::Rect {
                mode: PaintMode::Fill,
                ..
            } => guidance,
            _ => false,
        })
        .count()
}

#[test]
fn all_mode_surfaces_preserve_engagement_and_native_target_identity() {
    let data = resolve(&fixtures::extended(), &FreshnessPolicy::default());
    for panel in [
        &PFD_DESCRIPTOR,
        &AUTOFLIGHT_DESCRIPTOR,
        &HMD_DESCRIPTOR,
        &HMD_GLANCE_DESCRIPTOR,
    ] {
        let scene = render(panel, &data, None);
        let labels = texts(&scene);
        for required in [
            "AP ON",
            "FD ON",
            "A/THR ON",
            "NAV",
            "ALT",
            "SPD",
            "APR",
            "GS",
            "TGT M 0.780",
            "SEL IAS 301",
        ] {
            assert!(
                labels.contains(&required),
                "{} lacks {required}: {labels:?}",
                panel.id
            );
        }
    }
}

#[test]
fn legacy_ap_failure_preserves_independently_valid_director_annunciation() {
    let mut state = fixtures::extended();
    state.flight = Default::default();
    let mut data = resolve(&state, &FreshnessPolicy::default());
    data.flight.ap_modes.status = SignalStatus::Stale;
    data.flight.director.engagement = indicate_instrument_state::FdEngagement::Engaged;
    data.flight.director.mode = indicate_instrument_state::FdMode::Nav;
    for panel in [&PFD_DESCRIPTOR, &HMD_DESCRIPTOR, &HMD_GLANCE_DESCRIPTOR] {
        let scene = render(panel, &data, None);
        assert_eq!(bars(&scene), 2, "{}", panel.id);
        let labels = texts(&scene);
        assert!(labels.contains(&"AP STALE"), "{}", panel.id);
        assert!(labels.contains(&"FD ON NAV"), "{}", panel.id);
        assert!(!labels.contains(&"AP ON"), "{}", panel.id);
    }
}

#[test]
fn every_primary_display_suppresses_degraded_commands_and_keeps_disconnect_warning() {
    let data = resolve(&fixtures::extended(), &FreshnessPolicy::default());
    let alerts = super::cases::flight::disconnect_alert();
    for panel in [&PFD_DESCRIPTOR, &HMD_DESCRIPTOR, &HMD_GLANCE_DESCRIPTOR] {
        assert_eq!(bars(&render(panel, &data, None)), 2, "{}", panel.id);
        for status in [
            SignalStatus::Stale,
            SignalStatus::Degraded,
            SignalStatus::Missing,
            SignalStatus::Failed,
        ] {
            let mut bad = data;
            bad.flight.guidance.status = status;
            let scene = render(panel, &bad, Some(&alerts));
            assert_eq!(bars(&scene), 0, "{} {status:?}", panel.id);
            assert!(texts(&scene).contains(&"AP DISC"));
            assert!(!texts(&scene).contains(&"AP ON"));
        }
        let mut recovery = data;
        recovery.presentation.unusual = true;
        let scene = render(panel, &recovery, Some(&alerts));
        assert_eq!(bars(&scene), 0);
        assert!(texts(&scene).contains(&"AP DISC"));
    }
}

#[test]
fn envelope_gaps_survive_tape_projection_and_compact_summary() {
    use indicate_instrument_state::flight::envelope::*;
    let mut state = fixtures::extended();
    let envelope = &mut state.flight.speed.data.as_mut().expect("speed").envelope;
    envelope.intervals = [None; MAX_ENVELOPE_INTERVALS];
    for (index, (lower, upper)) in [(130.0, 140.0), (150.0, 161.0)].into_iter().enumerate() {
        envelope.intervals[index] = Some(EnvelopeInterval {
            kind: EnvelopeKind::Operating,
            lower,
            upper,
            lower_reason: BoundaryReason::Control,
            upper_reason: BoundaryReason::Mmo,
        });
    }
    let data = resolve(&state, &FreshnessPolicy::default());
    for (panel, lane_x, current_y) in [
        (&PFD_DESCRIPTOR, 124.0, 180.0),
        (&HMD_DESCRIPTOR, 314.0, 286.0),
    ] {
        let scene = render(panel, &data, None);
        let segments: Vec<_> = SceneCmds::new(&scene)
            .expect("scene")
            .filter_map(|cmd| match cmd.expect("command") {
                Cmd::Line { x1, y1, x2, y2 } if x1 == lane_x && x2 == lane_x => Some([y1, y2]),
                _ => None,
            })
            .collect();
        assert_eq!(segments.len(), 2, "{}", panel.id);
        assert!(
            segments
                .iter()
                .all(|[a, b]| !(a.min(*b) < current_y && current_y < a.max(*b)))
        );
        assert!(texts(&scene).contains(&"OUTSIDE LIM"));
    }
    assert!(texts(&render(&HMD_GLANCE_DESCRIPTOR, &data, None)).contains(&"OUTSIDE LIM"));
}

#[test]
fn missing_partial_and_empty_envelopes_make_different_claims() {
    use indicate_instrument_state::flight::envelope::*;
    for (coverage, expected) in [
        (EnvelopeCoverage::Unavailable, "ENV ---"),
        (EnvelopeCoverage::Partial, "ENV PARTIAL"),
        (EnvelopeCoverage::Complete, "ENV EMPTY"),
    ] {
        let mut state = fixtures::extended();
        let envelope = &mut state.flight.speed.data.as_mut().expect("speed").envelope;
        envelope.coverage = coverage;
        envelope.intervals = [None; MAX_ENVELOPE_INTERVALS];
        let data = resolve(&state, &FreshnessPolicy::default());
        for panel in [&PFD_DESCRIPTOR, &HMD_DESCRIPTOR, &HMD_GLANCE_DESCRIPTOR] {
            let scene = render(panel, &data, None);
            assert!(
                texts(&scene).contains(&expected),
                "{}: {expected}",
                panel.id
            );
            assert!(
                texts(&scene).contains(&"282"),
                "measured speed remains independent"
            );
        }
    }
}

#[test]
fn a_mach_pitch_target_annunciates_mach_instead_of_ias() {
    use indicate_instrument_state::{VerticalMode, flight::guidance::SpeedControl};
    let mut state = fixtures::extended();
    let guidance = state.flight.guidance.data.as_mut().expect("guidance");
    guidance.speed_control = SpeedControl::Pitch;
    guidance.vertical.active = VerticalMode::Airspeed;
    let data = resolve(&state, &FreshnessPolicy::default());
    for panel in [
        &PFD_DESCRIPTOR,
        &AUTOFLIGHT_DESCRIPTOR,
        &HMD_DESCRIPTOR,
        &HMD_GLANCE_DESCRIPTOR,
    ] {
        assert!(texts(&render(panel, &data, None)).contains(&"MACH"));
    }
}
