//! The host keeps the five obligations: one generation per admission,
//! a paint clock that runs on stale data, one resolve and one alert
//! step per frame, a watchdog on its own progress, and no frame from a
//! refused paint.

#![allow(clippy::expect_used, clippy::panic)]

use indicate_alerts::{
    AlertClass, AlertCondition, AlertEvent, AlertProfile, AlertState, DisplayFault,
};
use indicate_instrument_descriptor::{
    DesignFrame, DrawContext, EMPTY_CONFIG, PanelDescriptor, PanelDrawError, SceneTarget,
};
use indicate_instrument_scene::{LayerId, SceneError};
use indicate_instrument_state::{AircraftState, abi::v8::fixtures};
use indicate_instrument_template::TEMPLATE_DESCRIPTOR;

use crate::{
    EchoStatus, Host, HostConfig, HostError, Intent, IntentStamp, Request, Routed, failure_page,
};

const FRAME: DesignFrame = DesignFrame {
    width: 240.0,
    height: 120.0,
};

fn host() -> Host {
    Host::new(HostConfig::default(), AlertProfile::simulator())
}

fn request(sequence: u32, intent: Intent) -> Request {
    Request {
        stamp: IntentStamp {
            device_id: 1,
            sequence,
            issued_at_ms: 0,
        },
        intent,
    }
}

fn failing_draw(_: &DrawContext<'_>, _: SceneTarget<'_, '_>) -> Result<(), PanelDrawError> {
    Err(PanelDrawError::Scene(SceneError::BufferFull))
}

#[test]
fn each_admission_advances_the_snapshot_generation_once() {
    let mut host = host();
    assert_eq!(
        host.admit_snapshot(fixtures::full(), 0.0).expect("clock"),
        1
    );
    assert_eq!(
        host.admit_snapshot(fixtures::full(), 1.0).expect("clock"),
        2
    );
    assert_eq!(host.state().snapshot.generation, 2);
    assert_eq!(
        host.admit_snapshot(fixtures::full(), 0.5),
        Err(HostError::InvalidClock)
    );
    assert_eq!(host.state().snapshot.generation, 2);
}

#[test]
fn a_frame_is_made_on_the_paint_clock_and_carries_the_snapshot_generation() {
    let mut host = host();
    host.admit_snapshot(fixtures::full(), 0.0).expect("clock");
    let mut buf = [0u8; 4096];
    let painted = host
        .paint(
            &TEMPLATE_DESCRIPTOR,
            &EMPTY_CONFIG,
            FRAME,
            &[],
            &mut buf,
            10.0,
        )
        .expect("painted");
    assert_eq!(painted.frame_generation, 1);
    assert_eq!(painted.render_generation, 1);
    assert!(painted.scene_len > 0);
    assert_ne!(painted.layers.present & (1 << LayerId::Tapes.to_u8()), 0);
    // No new data: the clock still makes a frame, and only the render
    // generation moves.
    let painted = host
        .paint(
            &TEMPLATE_DESCRIPTOR,
            &EMPTY_CONFIG,
            FRAME,
            &[],
            &mut buf,
            20.0,
        )
        .expect("painted");
    assert_eq!(painted.frame_generation, 1);
    assert_eq!(painted.render_generation, 2);
}

#[test]
fn a_refused_paint_makes_no_frame_and_the_failure_page_stands_in() {
    let mut host = host();
    host.admit_snapshot(fixtures::full(), 0.0).expect("clock");
    let failing = PanelDescriptor {
        draw: failing_draw,
        ..TEMPLATE_DESCRIPTOR
    };
    let mut buf = [0u8; 4096];
    let refused = host.paint(&failing, &EMPTY_CONFIG, FRAME, &[], &mut buf, 10.0);
    assert!(matches!(refused, Err(HostError::Draw(_))));
    assert_eq!(host.render_generation(), 0);
    let len = failure_page(&mut buf, FRAME, "DISPLAY FAIL").expect("page");
    let report = indicate_instrument_scene::validate_layers(&buf[..len]).expect("valid");
    assert_eq!(report.present, 1 << LayerId::Failure.to_u8());
}

#[test]
fn a_renderer_that_stops_making_frames_is_annunciated_then_cleared() {
    let mut host = host();
    host.admit_snapshot(fixtures::full(), 0.0).expect("clock");
    let mut buf = [0u8; 4096];
    host.paint(
        &TEMPLATE_DESCRIPTOR,
        &EMPTY_CONFIG,
        FRAME,
        &[],
        &mut buf,
        0.0,
    )
    .expect("painted");
    // Past the deadline with no frame between: the tick asserts a stall.
    let painted = host
        .paint(
            &TEMPLATE_DESCRIPTOR,
            &EMPTY_CONFIG,
            FRAME,
            &[],
            &mut buf,
            1500.0,
        )
        .expect("painted");
    let stalled = painted
        .alerts
        .active()
        .iter()
        .find(|alert| alert.class == AlertClass::Warning)
        .expect("stall warning");
    assert_eq!(
        stalled.id,
        indicate_alerts::AlertCondition::Display(DisplayFault::RendererStalled).id()
    );
    // The frame just made clears the stall on the next step.
    let painted = host
        .paint(
            &TEMPLATE_DESCRIPTOR,
            &EMPTY_CONFIG,
            FRAME,
            &[],
            &mut buf,
            1600.0,
        )
        .expect("painted");
    assert!(!host.stalled());
    let cleared = painted
        .alerts
        .active()
        .iter()
        .find(|alert| alert.id == AlertCondition::Display(DisplayFault::RendererStalled).id())
        .expect("latched warning stays until acknowledged");
    assert_eq!(cleared.state, AlertState::LatchedCleared);
}

#[test]
fn an_acknowledgement_request_reaches_the_alert_manager() {
    let mut host = host();
    host.admit_snapshot(fixtures::full(), 0.0).expect("clock");
    let mut buf = [0u8; 4096];
    let condition = AlertCondition::Display(DisplayFault::BackendLost);
    host.paint(
        &TEMPLATE_DESCRIPTOR,
        &EMPTY_CONFIG,
        FRAME,
        &[AlertEvent::Assert(condition)],
        &mut buf,
        0.0,
    )
    .expect("painted");
    assert_eq!(
        host.request(&request(1, Intent::AcknowledgeAlert(condition.id())), 1.0)
            .expect("admitted"),
        Routed::ToAlerts
    );
    let painted = host
        .paint(
            &TEMPLATE_DESCRIPTOR,
            &EMPTY_CONFIG,
            FRAME,
            &[],
            &mut buf,
            2.0,
        )
        .expect("painted");
    let alert = painted
        .alerts
        .active()
        .iter()
        .find(|alert| alert.id == condition.id())
        .expect("still asserted");
    assert_eq!(alert.state, AlertState::Acknowledged);
}

#[test]
fn a_selection_is_pending_until_the_admitted_snapshot_echoes_it() {
    let mut host = host();
    host.admit_snapshot(fixtures::full(), 0.0).expect("clock");
    let routed = host
        .request(&request(1, Intent::SelectBaro { hpa: 1020.0 }), 1.0)
        .expect("admitted");
    assert_eq!(routed, Routed::ToSource(Intent::SelectBaro { hpa: 1020.0 }));
    let mut buf = [0u8; 4096];
    let painted = host
        .paint(
            &TEMPLATE_DESCRIPTOR,
            &EMPTY_CONFIG,
            FRAME,
            &[],
            &mut buf,
            2.0,
        )
        .expect("painted");
    assert!(matches!(painted.echo[0], EchoStatus::Pending { .. }));
    // The source reports the value back in the next snapshot.
    let mut echoed: AircraftState = fixtures::full();
    echoed.selections.baro_sel_hpa = Some(1020.0);
    host.admit_snapshot(echoed, 3.0).expect("clock");
    let painted = host
        .paint(
            &TEMPLATE_DESCRIPTOR,
            &EMPTY_CONFIG,
            FRAME,
            &[],
            &mut buf,
            4.0,
        )
        .expect("painted");
    assert_eq!(painted.echo[0], EchoStatus::Confirmed);
}

#[test]
fn declutter_and_page_requests_return_to_the_display() {
    let mut host = host();
    assert_eq!(
        host.request(&request(1, Intent::Declutter(true)), 0.0)
            .expect("admitted"),
        Routed::ToDisplay(Intent::Declutter(true))
    );
    assert!(matches!(
        host.request(
            &request(2, Intent::SelectPage(indicate_intent::PageId(3))),
            1.0
        ),
        Ok(Routed::ToDisplay(Intent::SelectPage(_)))
    ));
}

#[test]
fn interaction_frames_evolve_view_pose_and_volume_paint_forwards_echo_and_pose() {
    use indicate_instrument_state::Quat;
    use indicate_instrument_volumes::{CANONICAL_EXTENT, TERRAIN_VOLUME_DESCRIPTOR};
    use indicate_interaction::{
        GesturePhase, InteractionFrame, InteractionStamp, SpatialEvent, ViewLimits,
    };

    let mut host = host();
    host.admit_snapshot(fixtures::full(), 0.0).expect("clock");
    host.request(&request(1, Intent::SelectBaro { hpa: 1018.0 }), 1.0)
        .expect("admitted");

    let mut buf = [0u8; 8192];
    let first = host
        .paint_volume(
            &TERRAIN_VOLUME_DESCRIPTOR,
            &EMPTY_CONFIG,
            CANONICAL_EXTENT,
            &[],
            &mut buf,
            10.0,
        )
        .expect("painted volume");
    let first_bytes = buf[..first.scene_len].to_vec();
    assert_eq!(host.echo_summary().last_admitted_seq, Some(1));
    assert!(matches!(first.echo[0], EchoStatus::Pending { .. }));

    let ixn1 = InteractionFrame {
        stamp: InteractionStamp {
            device_id: 1,
            sequence: 1,
            timestamp_us: 20_000,
        },
        event: SpatialEvent::Transform3D {
            phase: GesturePhase::Changed,
            pivot_local: [0.0, 0.0, 0.0],
            translation_local: [0.05, 0.0, 0.0],
            rotation: Quat::IDENTITY,
            scale: 1.5,
        },
    };
    let ixn2 = InteractionFrame {
        stamp: InteractionStamp {
            device_id: 1,
            sequence: 2,
            timestamp_us: 21_000,
        },
        event: SpatialEvent::ScrubTime { offset_s: 120.0 },
    };
    let _o1 = host
        .interact(&ixn1, &ViewLimits::VOLUMETRIC_DEFAULT)
        .expect("interact 1");
    let _o2 = host
        .interact(&ixn2, &ViewLimits::VOLUMETRIC_DEFAULT)
        .expect("interact 2");
    let pose = host.view_pose();
    assert!((pose.zoom_scale - 1.5).abs() < 1e-5);
    assert!((pose.time_offset_s - 120.0).abs() < 1e-5);

    let second = host
        .paint_volume(
            &TERRAIN_VOLUME_DESCRIPTOR,
            &EMPTY_CONFIG,
            CANONICAL_EXTENT,
            &[],
            &mut buf,
            20.0,
        )
        .expect("painted volume");
    assert_ne!(first_bytes, &buf[..second.scene_len]);
}
