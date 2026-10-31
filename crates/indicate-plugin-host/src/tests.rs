#![allow(clippy::expect_used, clippy::panic)]

mod wasm_builder;

use indicate_frames::Quat;
use indicate_instrument_descriptor::{
    DesignFrame, DrawContext, GroupSet, UnderlayContract, planar_layer_bit, volume_band_bit,
};
use indicate_instrument_registry::ViewLimits;
use indicate_instrument_scene::volume::{
    Extent3D, HandleKind, MAX_VOLUME_SCENE_BYTES, VolumeBandId, VolumeSceneWriter,
    VolumeUnderlayKind,
};
use indicate_instrument_scene::{
    Anchor, HAlign, LayerId, MAX_SCENE_BYTES, PaintMode, Rgba8, SceneWriter,
};
use indicate_instrument_state::{AircraftState, FreshnessPolicy, GroupId, resolve};
use indicate_plugin_abi::{
    IndpackError, PluginDescriptorWire, PluginSpaceWire, RequireSignature, SIG_ALG_KEYED_SHA256,
    SignatureError, VerifyFn, pack_indpack,
};

use crate::{LoadedPlugin, PluginDrawTarget, PluginHostError, admit_plugin};
use wasm_builder::{build_plugin_wasm, emit_static_copy_body, push_leb_i32};

const TEST_MAC: [u8; 8] = *b"INDMAC01";

fn sign_pack(desc: &PluginDescriptorWire, wasm: &[u8]) -> Vec<u8> {
    let mut buf = vec![0u8; wasm.len() + 4096];
    let n = pack_indpack(
        desc,
        wasm,
        SIG_ALG_KEYED_SHA256,
        |_prefix, sha, sig_out| {
            sig_out[0..8].copy_from_slice(&TEST_MAC);
            sig_out[8..40].copy_from_slice(sha);
            Ok(40)
        },
        &mut buf,
    )
    .expect("pack succeeds");
    buf.truncate(n);
    buf
}

fn test_verifier() -> impl indicate_plugin_abi::SignatureVerifier {
    VerifyFn(|_prefix: &[u8], sha: &[u8; 32], alg: u8, sig: &[u8]| {
        if alg != SIG_ALG_KEYED_SHA256 {
            return Err(SignatureError::UnsupportedAlgorithm(alg));
        }
        if sig.len() == 40 && sig[0..8] == TEST_MAC && sig[8..40] == sha[..] {
            Ok(())
        } else {
            Err(SignatureError::BadSignature)
        }
    })
}

fn encode_valid_volume_scene(with_air_numeral: bool, untagged: bool) -> Vec<u8> {
    let mut buf = vec![0u8; 1024];
    let mut w = VolumeSceneWriter::new(&mut buf).expect("writer");
    w.begin_band(VolumeBandId::SurfaceUnderlay).expect("band");
    w.underlay_anchor_3d(
        VolumeUnderlayKind::TerrainPatch,
        [0.0, 0.0, 0.0],
        [0.5, 0.2, 0.5],
        Quat::IDENTITY,
        1000.0,
    )
    .expect("anchor");
    w.end_band(VolumeBandId::SurfaceUnderlay).expect("end");
    w.begin_band(VolumeBandId::Billboards).expect("band");
    w.fill_color(Rgba8::rgb(255, 255, 255)).expect("color");
    if untagged {
        w.billboard_text([0.0, 0.05, 0.0], 0.02, HAlign::Center, "250")
            .expect("untagged");
    } else if with_air_numeral {
        w.attributed_billboard_text(
            GroupId::Air.to_u8(),
            [0.0, 0.05, 0.0],
            0.02,
            HAlign::Center,
            "250",
        )
        .expect("tagged");
    } else {
        w.billboard_text([0.0, 0.05, 0.0], 0.02, HAlign::Center, "---")
            .expect("dashes");
    }
    w.end_band(VolumeBandId::Billboards).expect("end");
    w.begin_band(VolumeBandId::Handles).expect("band");
    w.target_handle(1, HandleKind::Waypoint, [0.1, 0.0, -0.1], 0.02)
        .expect("handle");
    w.end_band(VolumeBandId::Handles).expect("end");
    let used = w.finish();
    buf.truncate(used);
    buf
}

fn build_honest_volume_wasm() -> Vec<u8> {
    let scene_fed = encode_valid_volume_scene(true, false);
    let scene_dash = encode_valid_volume_scene(false, false);
    let mut data_seg = scene_fed.clone();
    let dash_src = data_seg.len() as i32;
    data_seg.extend_from_slice(&scene_dash);

    // Locals: 0..6 = params; 7 = rem_groups, 8 = pos, 9 = has_air.
    // State v8 starts at scratch_ptr (4096) + 72: [72]=ver(8), [73]=group_count, [74..]=TLVs.
    let mut body = Vec::new();
    // local 7 = i32.load8_u(4096 + 73)
    body.push(0x41);
    push_leb_i32(&mut body, 4096 + 73);
    body.extend_from_slice(&[0x2D, 0x00, 0x00, 0x21, 0x07]);
    // local 8 = 4096 + 74
    body.push(0x41);
    push_leb_i32(&mut body, 4096 + 74);
    body.extend_from_slice(&[0x21, 0x08]);
    // block $done (0x02 0x40), loop $next (0x03 0x40)
    body.extend_from_slice(&[0x02, 0x40, 0x03, 0x40]);
    // br_if $done(1) (i32.eqz (local.get 7))
    body.extend_from_slice(&[0x20, 0x07, 0x45, 0x0D, 0x01]);
    // if (i32.eq (i32.load8_u (local.get 8)) Air) { local.set 9 = 1; br $done(2) }
    body.extend_from_slice(&[0x20, 0x08, 0x2D, 0x00, 0x00, 0x41]);
    push_leb_i32(&mut body, i32::from(GroupId::Air.to_u8()));
    body.extend_from_slice(&[0x46, 0x04, 0x40, 0x41, 0x01, 0x21, 0x09, 0x0C, 0x02, 0x0B]);
    // pos = pos + 3 + i32.load16_u offset=1 (local.get 8)
    body.extend_from_slice(&[
        0x20, 0x08, 0x41, 0x03, 0x6A, 0x20, 0x08, 0x2F, 0x00, 0x01, 0x6A, 0x21, 0x08,
    ]);
    // rem_groups -= 1; br $next(0); end loop; end block
    body.extend_from_slice(&[
        0x20, 0x07, 0x41, 0x01, 0x6B, 0x21, 0x07, 0x0C, 0x00, 0x0B, 0x0B,
    ]);
    // if (result i32) (local.get 9) { copy fed } else { copy dash }
    body.extend_from_slice(&[0x20, 0x09, 0x04, 0x7F]);
    body.extend_from_slice(&emit_static_copy_body(0, scene_fed.len()));
    body.push(0x05);
    body.extend_from_slice(&emit_static_copy_body(dash_src, scene_dash.len()));
    body.push(0x0B);
    build_plugin_wasm(2, 3, &body, &data_seg)
}

#[test]
fn valid_volume_3d_and_planar_2d_plugins_load_admit_and_draw() {
    let ext = Extent3D::new(0.60, 0.45, 0.60);
    let vol_desc = PluginDescriptorWire::new(
        "plugin-terrain-3d",
        "Plugin Terrain 3D",
        PluginSpaceWire::volume_3d(ext, ext, &[ext], ViewLimits::VOLUMETRIC_DEFAULT)
            .expect("space"),
        volume_band_bit(VolumeBandId::SurfaceUnderlay)
            | volume_band_bit(VolumeBandId::Billboards)
            | volume_band_bit(VolumeBandId::Handles),
        GroupSet::of(&[GroupId::Air]),
        UnderlayContract::VolumetricTerrain3D,
        &[],
        &[],
        &[],
    )
    .expect("desc");
    let vol_pack = sign_pack(&vol_desc, &build_honest_volume_wasm());
    let mut vol_plugin = LoadedPlugin::load(&vol_pack, &test_verifier()).expect("loads");
    let adm = admit_plugin(&mut vol_plugin).expect("volume plugin passes admission");
    assert_eq!(adm.cases_checked, 24);
    assert!(adm.max_case_fuel > 0);

    // Also verify a valid 2D Planar plugin!
    let frame = DesignFrame {
        width: 480.0,
        height: 360.0,
    };
    let mut scene_2d = vec![0u8; 512];
    let mut sw = SceneWriter::new(&mut scene_2d).expect("sw");
    sw.begin_layer(LayerId::Background).expect("bg");
    sw.fill_color(Rgba8::rgb(10, 14, 24)).expect("fill");
    sw.rect(PaintMode::Fill, 0.0, 0.0, 480.0, 360.0)
        .expect("rect");
    sw.end_layer(LayerId::Background).expect("end");
    sw.begin_layer(LayerId::Annunciation).expect("ann");
    sw.fill_color(Rgba8::rgb(255, 255, 255)).expect("fill");
    sw.text(24.0, 24.0, 14.0, Anchor::MIDDLE_LEFT, "SIM")
        .expect("text");
    sw.end_layer(LayerId::Annunciation).expect("end");
    let used_2d = sw.finish();
    scene_2d.truncate(used_2d);

    let wasm_2d = build_plugin_wasm(2, 0, &emit_static_copy_body(0, scene_2d.len()), &scene_2d);
    let desc_2d = PluginDescriptorWire::new(
        "plugin-card-2d",
        "Plugin Card 2D",
        PluginSpaceWire::planar_2d(frame, frame, (1.0, 1.0), 1.30, 1.37, &[frame]).expect("space"),
        planar_layer_bit(LayerId::Background) | planar_layer_bit(LayerId::Annunciation),
        GroupSet::EMPTY,
        UnderlayContract::Opaque,
        &[],
        &[],
        &[],
    )
    .expect("desc2d");
    let pack_2d = sign_pack(&desc_2d, &wasm_2d);
    let mut plug_2d = LoadedPlugin::load(&pack_2d, &test_verifier()).expect("loads 2d");
    let adm_2d = admit_plugin(&mut plug_2d).expect("2d plugin passes admission");
    assert_eq!(adm_2d.cases_checked, 12);

    let data = resolve(&AircraftState::default(), &FreshnessPolicy::default());
    let mut out_2d = vec![0u8; MAX_SCENE_BYTES];
    let rep = plug_2d
        .draw(
            &DrawContext::new(&data),
            PluginDrawTarget::Planar2D {
                frame,
                out: &mut out_2d,
            },
        )
        .expect("draws 2d");
    assert_eq!(&out_2d[..rep.primary_bytes], scene_2d.as_slice());
}

#[test]
fn corrupted_sha256_and_bad_signature_are_rejected_fail_closed() {
    let ext = Extent3D::new(0.60, 0.45, 0.60);
    let desc = PluginDescriptorWire::new(
        "plugin-tamper",
        "Tamper",
        PluginSpaceWire::volume_3d(ext, ext, &[ext], ViewLimits::LOCKED).expect("space"),
        volume_band_bit(VolumeBandId::Handles),
        GroupSet::EMPTY,
        UnderlayContract::None,
        &[],
        &[],
        &[],
    )
    .expect("desc");
    let wasm = build_plugin_wasm(1, 0, &[0x41, 0x00], &[1, 2, 3, 4]);
    let pack = sign_pack(&desc, &wasm);

    // Default RequireSignature rejects without configured key.
    assert!(matches!(
        LoadedPlugin::load(&pack, &RequireSignature),
        Err(PluginHostError::Indpack(IndpackError::Signature(
            SignatureError::NoTrustedKeyConfigured
        )))
    ));

    // Corrupt last byte of signature -> BadSignature.
    let mut bad_sig = pack.clone();
    let last = bad_sig.len() - 1;
    bad_sig[last] ^= 0xFF;
    assert!(matches!(
        LoadedPlugin::load(&bad_sig, &test_verifier()),
        Err(PluginHostError::Indpack(IndpackError::Signature(
            SignatureError::BadSignature
        )))
    ));

    // Corrupt last byte of Wasm data segment -> Sha256Mismatch (checked before signature!).
    let mut bad_wasm = pack;
    let wasm_tail = bad_wasm.len() - 43 - 32 - 1;
    bad_wasm[wasm_tail] ^= 0x55;
    assert!(matches!(
        LoadedPlugin::load(&bad_wasm, &test_verifier()),
        Err(PluginHostError::Indpack(IndpackError::Sha256Mismatch))
    ));
}

#[test]
fn infinite_loop_wasm_is_aborted_deterministically_by_fuel_metering() {
    let ext = Extent3D::new(0.60, 0.45, 0.60);
    let desc = PluginDescriptorWire::new(
        "plugin-runaway",
        "Runaway",
        PluginSpaceWire::volume_3d(ext, ext, &[ext], ViewLimits::LOCKED).expect("space"),
        volume_band_bit(VolumeBandId::Handles),
        GroupSet::EMPTY,
        UnderlayContract::None,
        &[],
        &[],
        &[],
    )
    .expect("desc");
    // (loop $l (br $l)) unreachable -> 0x03 0x40 0x0C 0x00 0x0B 0x00
    let loop_body = [0x03u8, 0x40, 0x0C, 0x00, 0x0B, 0x00];
    let wasm = build_plugin_wasm(1, 0, &loop_body, &[]);
    let pack = sign_pack(&desc, &wasm);
    let mut plugin = LoadedPlugin::load(&pack, &test_verifier())
        .expect("loads")
        .with_frame_fuel(25_000);

    let data = resolve(&AircraftState::default(), &FreshnessPolicy::default());
    let mut out = vec![0u8; MAX_VOLUME_SCENE_BYTES];
    let err = plugin
        .draw(
            &DrawContext::new(&data),
            PluginDrawTarget::Volume3D {
                extent_m: ext,
                out: &mut out,
            },
        )
        .expect_err("infinite loop must trap on fuel");
    assert_eq!(err, PluginHostError::OutOfFuel);
}

#[test]
fn unattributed_numeral_and_oversized_memory_are_rejected() {
    let ext = Extent3D::new(0.60, 0.45, 0.60);
    let bad_scene = encode_valid_volume_scene(true, true);
    let bad_wasm = build_plugin_wasm(2, 0, &emit_static_copy_body(0, bad_scene.len()), &bad_scene);
    let desc = PluginDescriptorWire::new(
        "plugin-untagged",
        "Untagged",
        PluginSpaceWire::volume_3d(ext, ext, &[ext], ViewLimits::LOCKED).expect("space"),
        volume_band_bit(VolumeBandId::SurfaceUnderlay)
            | volume_band_bit(VolumeBandId::Billboards)
            | volume_band_bit(VolumeBandId::Handles),
        GroupSet::of(&[GroupId::Air]),
        UnderlayContract::VolumetricTerrain3D,
        &[],
        &[],
        &[],
    )
    .expect("desc");
    let pack = sign_pack(&desc, &bad_wasm);
    let mut plugin = LoadedPlugin::load(&pack, &test_verifier()).expect("loads");
    let err = admit_plugin(&mut plugin).expect_err("untagged numeral must fail admission");
    assert!(matches!(err, PluginHostError::UntaggedNumeral { .. }));

    // Also verify memory > 4 pages (5 pages max) is rejected at load time!
    let big_mem_wasm = build_plugin_wasm(5, 0, &[0x41, 0x00], &[]);
    let big_pack = sign_pack(&desc, &big_mem_wasm);
    assert_eq!(
        LoadedPlugin::load(&big_pack, &test_verifier()).expect_err("5 pages must fail"),
        PluginHostError::MemoryLimitExceeded
    );
}
