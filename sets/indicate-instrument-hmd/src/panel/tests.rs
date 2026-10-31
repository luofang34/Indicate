#![allow(clippy::expect_used, clippy::panic)]
use super::{HMD_DESCRIPTOR, HMD_FRAME, HMD_SET};
use indicate_instrument_conformance::admit;
use indicate_instrument_descriptor::{
    AuxChannelId, AuxEntry, DrawContext, PanelDrawError, SceneTarget,
};
use indicate_instrument_registry::{PanelSet, Registry};
use indicate_instrument_scene::{
    Cmd, LayerId, SceneCmds, SceneWriter,
    angular::{AngularCmds, AngularSceneWriter, MAX_ANGULAR_SCENE_BYTES},
};
use indicate_instrument_state::{AircraftState, FreshnessPolicy, resolve};
use std::vec::Vec;

#[test]
fn independent_overlay_passes_admission_without_background_or_overflow() {
    static SETS: [&PanelSet; 1] = [&HMD_SET];
    let registry = Registry::from_sets(&SETS).expect("independent set composes");
    let report = admit(&registry).expect("transparent readouts pass source withholding");
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
}

#[test]
fn missing_air_data_draws_missing_not_a_numeral() {
    let data = resolve(&AircraftState::default(), &FreshnessPolicy::default());
    let mut ang_buf = std::vec![0u8; MAX_ANGULAR_SCENE_BYTES];
    let mut ang_writer = AngularSceneWriter::new(&mut ang_buf).expect("angular buffer");
    let mut storage = [0; 8192];
    let mut writer = SceneWriter::new(&mut storage).expect("scene buffer");
    let ctx = DrawContext::new(&data);
    (HMD_DESCRIPTOR.draw)(
        &ctx,
        SceneTarget::AngularSphere {
            angular: &mut ang_writer,
            overlay: Some((HMD_FRAME, &mut writer)),
        },
    )
    .expect("missing-data scene");
    let ang_size = ang_writer.finish();
    let size = writer.finish();
    assert!(
        AngularCmds::new(&ang_buf[..ang_size])
            .expect("angular decode")
            .count()
            >= 4
    );
    let commands: Vec<_> = SceneCmds::new(&storage[..size])
        .expect("decode")
        .map(|cmd| cmd.expect("valid command"))
        .collect();
    assert!(!commands.iter().any(|cmd| matches!(
        cmd,
        Cmd::BeginLayer {
            layer: LayerId::Background
        }
    )));
    let texts: Vec<_> = commands
        .iter()
        .filter_map(|cmd| match cmd {
            Cmd::Text { text, .. } => Some(*text),
            _ => None,
        })
        .collect();
    assert!(texts.contains(&"IAS KT"));
    assert!(texts.contains(&"MISSING"));
    assert!(texts.contains(&"ATT MISSING"));
    assert!(texts.contains(&"---"));
    assert!(
        !texts
            .iter()
            .any(|text| text.chars().any(|c| c.is_ascii_digit()))
    );
}

#[test]
fn hwd_display_aux_channel_round_trips_and_rejects_malformed_bytes() {
    let data = resolve(&AircraftState::default(), &FreshnessPolicy::default());
    let display = crate::DisplayContext {
        detail: crate::DetailLevel::Reduced,
        ..crate::DisplayContext::default()
    };
    let wire = display.encode_aux();
    let entries = [AuxEntry {
        id: AuxChannelId::HWD_DISPLAY,
        payload: &wire,
    }];
    let ctx = DrawContext::new(&data).with_aux(&entries);
    let mut ang_buf = std::vec![0u8; MAX_ANGULAR_SCENE_BYTES];
    let mut ang_writer = AngularSceneWriter::new(&mut ang_buf).expect("angular buffer");
    let mut storage = [0; 8192];
    let mut writer = SceneWriter::new(&mut storage).expect("scene buffer");
    (HMD_DESCRIPTOR.draw)(
        &ctx,
        SceneTarget::AngularSphere {
            angular: &mut ang_writer,
            overlay: Some((HMD_FRAME, &mut writer)),
        },
    )
    .expect("valid aux");

    let bad = [AuxEntry {
        id: AuxChannelId::HWD_DISPLAY,
        payload: &[0xFF],
    }];
    let bad_ctx = DrawContext::new(&data).with_aux(&bad);
    let mut ang_writer2 = AngularSceneWriter::new(&mut ang_buf).expect("angular buffer");
    let mut writer2 = SceneWriter::new(&mut storage).expect("scene buffer");
    assert_eq!(
        (HMD_DESCRIPTOR.draw)(
            &bad_ctx,
            SceneTarget::AngularSphere {
                angular: &mut ang_writer2,
                overlay: Some((HMD_FRAME, &mut writer2)),
            },
        ),
        Err(PanelDrawError::InvalidAux(AuxChannelId::HWD_DISPLAY.0))
    );
}
