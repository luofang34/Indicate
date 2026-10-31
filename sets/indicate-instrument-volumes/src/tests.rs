//! Conformance, determinism, view-pose, and fail-closed tests (`VOL-01`, `IXN-01`).

#![allow(clippy::expect_used, clippy::panic)]

use indicate_frames::Quat;
use indicate_instrument_conformance::admit;
use indicate_instrument_descriptor::{
    AuxChannelId, AuxEntry, DrawContext, InstrumentDescriptor, PanelDrawError, SceneTarget,
    ViewPose3D,
};
use indicate_instrument_registry::{InstrumentSet, Registry, scene_digest};
use indicate_instrument_scene::volume::{
    Extent3D, MAX_VOLUME_SCENE_BYTES, VolumeCmd, VolumeCmds, VolumeSceneWriter,
};
use indicate_instrument_state::{AircraftState, FreshnessPolicy, SignalStatus, resolve};
use std::vec::Vec;

use crate::{
    CANONICAL_EXTENT, CelestialEphemerisAux, GLOBE_VOLUME_DESCRIPTOR, GlobeNavigationAux,
    MAX_EXTENT, MIN_EXTENT, ORRERY_VOLUME_DESCRIPTOR, TERRAIN_VOLUME_DESCRIPTOR, TerrainPatchAux,
    VOLUMES_SCENE_DIGEST, VOLUMES_SET,
};

fn render_vol(panel: &InstrumentDescriptor, ctx: &DrawContext<'_>, extent_m: Extent3D) -> Vec<u8> {
    let mut buf = std::vec![0u8; MAX_VOLUME_SCENE_BYTES];
    let mut writer = VolumeSceneWriter::new(&mut buf).expect("volume writer");
    (panel.draw)(
        ctx,
        SceneTarget::Volume3D {
            extent_m,
            writer: &mut writer,
        },
    )
    .expect("volume draw");
    let used = writer.finish();
    buf.truncate(used);
    buf
}

#[test]
fn volumes_set_passes_full_conformance_admission_without_warnings() {
    static SETS: [&InstrumentSet; 1] = [&VOLUMES_SET];
    let registry = Registry::from_sets(&SETS).expect("volumes set composes");
    let report = admit(&registry).expect("volumes set passes admission");
    assert!(report.cases > 0);
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
}

#[test]
fn volumes_scene_digest_is_reproducible_and_pinned() {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    static SETS: [&InstrumentSet; 1] = [&VOLUMES_SET];
    let registry = Registry::from_sets(&SETS).expect("volumes set composes");
    let mut scratch = std::vec![0u8; MAX_VOLUME_SCENE_BYTES];
    let raw = scene_digest(&registry, &mut scratch).expect("volumes digest");
    let mut digest = std::string::String::with_capacity(64);
    for byte in raw {
        digest.push(HEX[(byte >> 4) as usize] as char);
        digest.push(HEX[(byte & 0x0f) as usize] as char);
    }
    assert_eq!(digest, VOLUMES_SCENE_DIGEST);
}

#[test]
fn view_pose_and_extent_deterministically_transform_scenes_and_keep_handles_in_bounds() {
    let state = indicate_instrument_registry::states::typical();
    let data = resolve(&state, &FreshnessPolicy::default());
    let base_ctx = DrawContext::new(&data);

    let custom_pose = ViewPose3D {
        focus_local: [0.25, -0.15, 0.20],
        orientation: Quat::from_euler(0.15, -0.25, 0.40),
        zoom_scale: 1.8,
        active_target: indicate_interaction::TargetId(1),
        time_offset_s: 240.0,
        decluttered: true,
    };
    let moved_ctx = DrawContext::new(&data).with_view(custom_pose);

    for desc in [
        &TERRAIN_VOLUME_DESCRIPTOR,
        &GLOBE_VOLUME_DESCRIPTOR,
        &ORRERY_VOLUME_DESCRIPTOR,
    ] {
        let base_bytes = render_vol(desc, &base_ctx, CANONICAL_EXTENT);
        let moved_bytes = render_vol(desc, &moved_ctx, CANONICAL_EXTENT);
        assert_ne!(
            base_bytes, moved_bytes,
            "{} must respond to ViewPose3D",
            desc.id
        );

        for ext in [MIN_EXTENT, CANONICAL_EXTENT, MAX_EXTENT] {
            let bytes = render_vol(desc, &moved_ctx, ext);
            for cmd in VolumeCmds::new(&bytes).expect("decode") {
                if let VolumeCmd::TargetHandle { center, .. } = cmd.expect("cmd") {
                    assert!(
                        ext.contains_point(center),
                        "{} handle {center:?} escaped {ext:?}",
                        desc.id
                    );
                }
            }
        }
    }
}

#[test]
fn missing_and_stale_signals_withhold_or_flag_honestly() {
    let empty_data = resolve(&AircraftState::default(), &FreshnessPolicy::default());
    let empty_ctx = DrawContext::new(&empty_data);

    for desc in [
        &TERRAIN_VOLUME_DESCRIPTOR,
        &GLOBE_VOLUME_DESCRIPTOR,
        &ORRERY_VOLUME_DESCRIPTOR,
    ] {
        let bytes = render_vol(desc, &empty_ctx, CANONICAL_EXTENT);
        for cmd in VolumeCmds::new(&bytes).expect("decode") {
            if let VolumeCmd::BillboardText { text, .. } = cmd.expect("cmd") {
                assert!(
                    !text.chars().any(|c| c.is_ascii_digit()),
                    "{} emitted numeral on empty state: {text}",
                    desc.id
                );
            }
        }
    }

    let mut stale_data = resolve(
        &indicate_instrument_registry::states::typical(),
        &FreshnessPolicy::default(),
    );
    stale_data.gs_kt.status = SignalStatus::Stale;
    stale_data.heading.value_rad.status = SignalStatus::Stale;
    let stale_ctx = DrawContext::new(&stale_data);
    for desc in [
        &TERRAIN_VOLUME_DESCRIPTOR,
        &GLOBE_VOLUME_DESCRIPTOR,
        &ORRERY_VOLUME_DESCRIPTOR,
    ] {
        let bytes = render_vol(desc, &stale_ctx, CANONICAL_EXTENT);
        let has_stale = VolumeCmds::new(&bytes)
            .expect("decode")
            .any(|c| matches!(c, Ok(VolumeCmd::BillboardText { text: "STALE", .. })));
        assert!(has_stale, "{} must annunciate STALE", desc.id);
    }
}

#[test]
fn volumetric_aux_channels_round_trip_and_reject_malformed_payloads() {
    let data = resolve(
        &indicate_instrument_registry::states::typical(),
        &FreshnessPolicy::default(),
    );
    let terrain_wire = TerrainPatchAux::encode_4x4(5_000.0, &[0.02; 16]);
    let globe_wire = (GlobeNavigationAux {
        waypoint_bearing_rad: 1.1,
        waypoint_range_nm: 42.5,
        meters_per_unit: 1.5e7,
    })
    .encode();
    let orrery_wire = (CelestialEphemerisAux {
        inner_orbit_frac: 0.18,
        outer_orbit_frac: 0.40,
        phase_rad: 1.25,
        meters_per_unit: 2.0e9,
    })
    .encode();

    for (desc, ch, wire) in [
        (
            &TERRAIN_VOLUME_DESCRIPTOR,
            AuxChannelId::TERRAIN_PATCH_3D,
            terrain_wire.as_slice(),
        ),
        (
            &GLOBE_VOLUME_DESCRIPTOR,
            AuxChannelId::GLOBE_NAVIGATION_3D,
            globe_wire.as_slice(),
        ),
        (
            &ORRERY_VOLUME_DESCRIPTOR,
            AuxChannelId::CELESTIAL_EPHEMERIS_3D,
            orrery_wire.as_slice(),
        ),
    ] {
        let ok_entries = [AuxEntry {
            id: ch,
            payload: wire,
        }];
        let ok_ctx = DrawContext::new(&data).with_aux(&ok_entries);
        let default_bytes = render_vol(desc, &DrawContext::new(&data), CANONICAL_EXTENT);
        let aux_bytes = render_vol(desc, &ok_ctx, CANONICAL_EXTENT);
        assert_ne!(default_bytes, aux_bytes);

        let bad_entries = [AuxEntry {
            id: ch,
            payload: &[0xFF, 0x00, 0x01],
        }];
        let bad_ctx = DrawContext::new(&data).with_aux(&bad_entries);
        let mut buf = std::vec![0u8; MAX_VOLUME_SCENE_BYTES];
        let mut writer = VolumeSceneWriter::new(&mut buf).expect("writer");
        assert_eq!(
            (desc.draw)(
                &bad_ctx,
                SceneTarget::Volume3D {
                    extent_m: CANONICAL_EXTENT,
                    writer: &mut writer,
                },
            ),
            Err(PanelDrawError::InvalidAux(ch.0))
        );
    }
}
