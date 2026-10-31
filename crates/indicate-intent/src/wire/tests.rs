#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use indicate_alerts::AlertId;
use indicate_instrument_state::{
    AltitudeClass, GeoidModelId, HeadingReference, NavSource, OriginId,
};

use crate::{
    ECHO_SUMMARY_BYTES, ECHO_SUMMARY_VERSION, EchoStatus, EchoSummary, ExtensionPayload,
    INTENT_WIRE_VERSION, Intent, IntentClass, IntentFault, IntentGate, IntentNamespace,
    IntentStamp, IntentWireError, MAX_INTENT_WIRE_BYTES, PageId, Request, decode_echo_summary,
    decode_request, encode_echo_summary, encode_request, namespace_id,
};

fn stamp(seq: u32) -> IntentStamp {
    IntentStamp {
        device_id: 3,
        sequence: seq,
        issued_at_ms: 9_876,
    }
}

#[test]
fn namespace_id_is_deterministic_fnv1a_and_non_zero() {
    let ns = namespace_id("com.pilotage.orrery");
    assert!(!ns.is_core());
    assert_eq!(ns, namespace_id("com.pilotage.orrery"));
    assert_ne!(ns, namespace_id("com.pilotage.terrain"));
}

#[test]
fn round_trips_all_core_and_custom_requests() {
    let ns = namespace_id("com.pilotage.volumes");
    let custom_payload = ExtensionPayload::new(&[0x10, 0x20, 0x30, 0x40]).expect("payload");
    let intents = [
        Intent::SelectBaro { hpa: 1013.25 },
        Intent::SetHeadingBug {
            rad: 1.5,
            reference: HeadingReference::Magnetic,
        },
        Intent::SetHeadingBug {
            rad: 2.0,
            reference: HeadingReference::True,
        },
        Intent::SelectAltitude {
            meters: 3048.0,
            class: AltitudeClass::BaroIndicated,
            origin: OriginId(1),
            model: GeoidModelId(2),
        },
        Intent::SelectNavSource(NavSource::Nav1),
        Intent::SelectNavSource(NavSource::Nav2),
        Intent::SelectNavSource(NavSource::Gps),
        Intent::AcknowledgeAlert(AlertId(0x0101)),
        Intent::AcknowledgeAllAlerts,
        Intent::SelectPage(PageId(4)),
        Intent::Declutter(true),
        Intent::PressSoftkey { key_id: 12 },
        Intent::Custom {
            namespace: ns,
            opcode: 0x0042,
            payload: custom_payload,
        },
    ];
    let mut buf = [0u8; MAX_INTENT_WIRE_BYTES];
    for (idx, intent) in intents.into_iter().enumerate() {
        let req = Request {
            stamp: stamp(u32::try_from(idx + 1).expect("seq")),
            intent,
        };
        let n = encode_request(&req, &mut buf).expect("encode");
        assert_eq!(buf[0], INTENT_WIRE_VERSION);
        assert_eq!(buf[1], 0);
        let decoded = decode_request(&buf[..n]).expect("decode");
        assert_eq!(decoded, req);
        assert!(intent.class().matches(decoded.intent.class()));
    }
}

#[test]
fn gate_and_wire_reject_core_namespace_in_custom_intent() {
    let bad = Request {
        stamp: stamp(1),
        intent: Intent::Custom {
            namespace: IntentNamespace::CORE,
            opcode: 1,
            payload: ExtensionPayload::EMPTY,
        },
    };
    let mut gate = IntentGate::new();
    assert_eq!(
        gate.admit(&bad, 10.0),
        Err(IntentFault::ReservedCoreNamespace)
    );
    let mut buf = [0u8; MAX_INTENT_WIRE_BYTES];
    assert_eq!(
        encode_request(&bad, &mut buf),
        Err(IntentWireError::ReservedCoreNamespace)
    );

    let wildcard = IntentClass::custom(namespace_id("ext"), IntentClass::ANY_OPCODE);
    assert!(wildcard.matches(IntentClass::custom(namespace_id("ext"), 99)));
}

#[test]
fn echo_summary_wire_layout_and_fail_closed_checks() {
    let summary = EchoSummary {
        statuses: [
            EchoStatus::Idle,
            EchoStatus::Pending { age_ms: 45.0 },
            EchoStatus::Confirmed,
            EchoStatus::Unmeasurable,
        ],
        last_admitted_seq: Some(77),
    };
    let mut wire = [0u8; ECHO_SUMMARY_BYTES];
    encode_echo_summary(&summary, &mut wire);
    assert_eq!(wire[0], ECHO_SUMMARY_VERSION);
    assert_eq!(&wire[1..5], &[0, 1, 2, 3]);
    assert_eq!(wire[5], 1);
    assert_eq!(&wire[6..8], &[0, 0]);
    assert_eq!(&wire[8..12], &77u32.to_le_bytes());

    let decoded = decode_echo_summary(&wire).expect("decode");
    assert_eq!(decoded.last_admitted_seq, Some(77));
    assert_eq!(decoded.statuses[2], EchoStatus::Confirmed);
    assert_eq!(decoded.statuses[3], EchoStatus::Overdue { age_ms: 0.0 });

    let mut bad = wire;
    bad[6] = 1;
    assert_eq!(
        decode_echo_summary(&bad),
        Err(IntentWireError::NonZeroReserved)
    );
}
