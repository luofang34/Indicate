#![allow(clippy::expect_used, clippy::panic)]

use indicate_alerts::{
    AlertCondition, AlertContext, AlertEvent, AlertManager, AlertProfile, NavFault,
};
use indicate_frames::Quat;
use indicate_instrument_state::{FreshnessPolicy, GroupId, resolve};
use indicate_intent::{EchoStatus, EchoSummary};
use indicate_interaction::{TargetId, ViewPose3D};

use crate::aux::{AuxChannelId, AuxEntry, custom_aux_id};
use crate::config::ConfigBlob;
use crate::context::{
    DRAW_CONTEXT_HEADER_BYTES, DrawContext, DrawContextWireError, decode_draw_context,
    encode_draw_context,
};
use crate::space::{UnderlayContract, UnderlayStatus};
use crate::states::CANONICAL_STATES;

fn sample_echo_and_view() -> (EchoSummary, ViewPose3D) {
    let echo = EchoSummary {
        statuses: [
            EchoStatus::Pending { age_ms: 40.0 },
            EchoStatus::Confirmed,
            EchoStatus::Idle,
            EchoStatus::Overdue { age_ms: 600.0 },
        ],
        last_admitted_seq: Some(7),
    };
    let view = ViewPose3D {
        focus_local: [0.05, -0.02, 0.10],
        orientation: Quat::from_euler(0.1, -0.05, 0.2),
        zoom_scale: 1.5,
        active_target: TargetId(42),
        time_offset_s: 120.0,
        decluttered: true,
    };
    (echo, view)
}

#[test]
fn draw_context_round_trips_full_envelope_in_ascending_aux_order() {
    let state = (CANONICAL_STATES[0].build)();
    let data = resolve(&state, &FreshnessPolicy::default());
    let cfg_raw = [0x01, 0x00, 0x01, 0x00, 0x02];
    let cfg = ConfigBlob::parse(&cfg_raw).expect("valid config");

    let mut mgr = AlertManager::new();
    let profile = AlertProfile::simulator();
    let events = [AlertEvent::Assert(AlertCondition::Heading(
        NavFault::HeadingReferenceLost,
    ))];
    let alerts = mgr.step(&profile, &events, AlertContext::default(), 1000);
    let (echo, view) = sample_echo_and_view();
    let underlay = UnderlayStatus {
        available: true,
        calibrated: true,
        quality_pct: 95,
    };
    // Intentionally pass out-of-order distinct IDs to verify canonical ascending emission.
    let aux = [
        AuxEntry {
            id: AuxChannelId::TERRAIN_PATCH_3D,
            payload: &[1, 2, 3, 4],
        },
        AuxEntry {
            id: AuxChannelId::HWD_DISPLAY,
            payload: &[9, 8],
        },
    ];

    let ctx = DrawContext::new(&data)
        .with_raw_state(&state)
        .with_config(cfg)
        .with_alerts(Some(&alerts))
        .with_echo(echo)
        .with_view(view)
        .with_underlay(underlay)
        .with_aux(&aux);

    let mut wire = [0u8; 1024];
    let written = encode_draw_context(&ctx, &mut wire).expect("encode succeeds");
    assert!(written > DRAW_CONTEXT_HEADER_BYTES);
    assert_eq!(&wire[..4], &[1, 0x03, 95, 1]);

    let decoded = decode_draw_context(&wire[..written]).expect("decode succeeds");
    let rebound = decoded.as_draw_context();
    assert_eq!(rebound.underlay(), underlay);
    assert_eq!(rebound.view().active_target, TargetId(42));
    assert!(rebound.view().decluttered);
    assert_eq!(rebound.echo().last_admitted_seq, Some(7));
    assert_eq!(rebound.config().as_bytes(), &cfg_raw);
    assert_eq!(rebound.alerts(), Some(&alerts));
    assert_eq!(rebound.aux(AuxChannelId::HWD_DISPLAY), Some(&[9u8, 8][..]));
    assert_eq!(
        rebound.aux(AuxChannelId::TERRAIN_PATCH_3D),
        Some(&[1u8, 2, 3, 4][..])
    );
    assert_eq!(rebound.aux_entries()[0].id, AuxChannelId::HWD_DISPLAY);
    assert_eq!(rebound.aux_entries()[1].id, AuxChannelId::TERRAIN_PATCH_3D);
}

#[test]
fn draw_context_synthesizes_state_when_raw_state_not_supplied() {
    for canonical in CANONICAL_STATES {
        let state = (canonical.build)();
        let data = resolve(&state, &FreshnessPolicy::default());
        let ctx = DrawContext::new(&data);
        let mut wire = [0u8; 1024];
        let n = encode_draw_context(&ctx, &mut wire).expect("synthesized encode");
        let decoded = decode_draw_context(&wire[..n]).expect("synthesized decode");
        for g in [
            GroupId::Attitude,
            GroupId::Kinematics,
            GroupId::Air,
            GroupId::Heading,
        ] {
            assert_eq!(
                decoded.data().groups.status(g),
                data.groups.status(g),
                "group {g:?} status mismatch on {}",
                canonical.id
            );
        }
    }
}

#[test]
fn draw_context_wire_rejects_corrupt_headers_and_unsorted_aux() {
    let state = (CANONICAL_STATES[0].build)();
    let data = resolve(&state, &FreshnessPolicy::default());
    let aux = [
        AuxEntry {
            id: AuxChannelId::HWD_DISPLAY,
            payload: &[1],
        },
        AuxEntry {
            id: AuxChannelId::HUD_CONTEXT,
            payload: &[2],
        },
    ];
    let ctx = DrawContext::new(&data)
        .with_raw_state(&state)
        .with_aux(&aux);
    let mut wire = [0u8; 1024];
    let n = encode_draw_context(&ctx, &mut wire).expect("encode");

    // Corrupt underlay quality > 100
    let mut bad_qual = wire;
    bad_qual[2] = 101;
    assert_eq!(
        decode_draw_context(&bad_qual[..n]).unwrap_err(),
        DrawContextWireError::InvalidUnderlayQuality(101)
    );

    // Corrupt underlay reserved flag bit
    let mut bad_flags = wire;
    bad_flags[1] = 0x04;
    assert_eq!(
        decode_draw_context(&bad_flags[..n]).unwrap_err(),
        DrawContextWireError::InvalidUnderlayFlags(0x04)
    );

    // Swap aux IDs on wire so second ID <= first ID
    let mut bad_aux = wire;
    let first_aux_off = n - (9 + 9);
    bad_aux[first_aux_off..first_aux_off + 4].copy_from_slice(&0x0010u32.to_le_bytes());
    assert_eq!(
        decode_draw_context(&bad_aux[..n]).unwrap_err(),
        DrawContextWireError::AuxOrder(0x0002)
    );
}

#[test]
fn custom_aux_and_underlay_contracts_hold_invariants() {
    let id = custom_aux_id("org.pilotage.weather.radar");
    assert_ne!(id.0 & 0x8000_0000, 0);
    assert_eq!(id.well_known_name(), None);
    assert_eq!(
        AuxChannelId::TERRAIN_PATCH_3D.well_known_name(),
        Some("TERRAIN_PATCH_3D")
    );
    for raw in 0u8..=8 {
        let contract = UnderlayContract::from_u8(raw).expect("valid discriminant");
        assert_eq!(contract.to_u8(), raw);
    }
    assert_eq!(UnderlayContract::from_u8(9), None);
}
