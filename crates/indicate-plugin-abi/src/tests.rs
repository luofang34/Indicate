#![allow(clippy::expect_used, clippy::panic)]

use indicate_instrument_descriptor::{
    AuxChannelId, ConfigKey, DesignFrame, GroupSet, UnderlayContract, volume_band_bit,
};
use indicate_instrument_scene::volume::{Extent3D, VolumeBandId};
use indicate_instrument_state::GroupId;
use indicate_intent::{IntentClass, core_opcode};
use indicate_interaction::ViewLimits;
use indicate_sha256::Sha256Ctx;

use crate::{
    IndpackError, MAX_PLUGIN_CANONICAL, PluginDescriptorWire, PluginSpaceWire, RequireSignature,
    SIG_ALG_KEYED_SHA256, SIG_ALG_NONE, SignatureError, VerifyFn, decode_descriptor,
    encode_descriptor, pack_indpack, unpack_indpack,
};

const TEST_KEY: &[u8] = b"indicate-test-signing-key-v1";
const MINIMAL_WASM: [u8; 11] = [
    0x00, b'a', b's', b'm', 0x01, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00,
];

fn keyed_mac(prefix: &[u8], wasm_sha256: &[u8; 32]) -> [u8; 32] {
    let mut ctx = Sha256Ctx::new();
    ctx.update(TEST_KEY);
    ctx.update(prefix);
    ctx.update(wasm_sha256);
    ctx.finalize()
}

fn sample_volume_descriptor() -> PluginDescriptorWire {
    let mut canonical_buf = [Extent3D::new(0.60, 0.45, 0.60); MAX_PLUGIN_CANONICAL];
    canonical_buf[0] = Extent3D::new(0.60, 0.45, 0.60);
    let space = PluginSpaceWire::Volume3D {
        extent_min_m: Extent3D::new(0.30, 0.30, 0.30),
        extent_max_m: Extent3D::new(1.50, 1.50, 1.50),
        canonical_buf,
        canonical_len: 1,
        view_limits: ViewLimits::VOLUMETRIC_DEFAULT,
    };
    PluginDescriptorWire::new(
        "weather-radar-3d",
        "3D Weather Volume",
        space,
        volume_band_bit(VolumeBandId::ReferenceGrid) | volume_band_bit(VolumeBandId::Failure),
        GroupSet::of(&[GroupId::Kinematics]),
        UnderlayContract::VolumetricTerrain3D,
        &[ConfigKey(0x8001)],
        &[IntentClass::core(core_opcode::DECLUTTER)],
        &[AuxChannelId::WORLD_AIRMASS],
    )
    .expect("valid descriptor")
}

#[test]
fn plugin_descriptor_round_trips_across_all_render_spaces() {
    let vol = sample_volume_descriptor();
    let mut buf = [0u8; 512];
    let n = encode_descriptor(&vol, &mut buf).expect("encode vol");
    let decoded = decode_descriptor(&buf[..n]).expect("decode vol");
    assert_eq!(decoded, vol);

    let ang = PluginDescriptorWire::new(
        "helmet-cue",
        "Helmet Cue",
        PluginSpaceWire::AngularSphere {
            overlay_frame: Some(DesignFrame {
                width: 480.0,
                height: 360.0,
            }),
        },
        0x0101,
        GroupSet::of(&[GroupId::Attitude]),
        UnderlayContract::None,
        &[],
        &[],
        &[AuxChannelId::HWD_DISPLAY],
    )
    .expect("valid angular");
    let n_ang = encode_descriptor(&ang, &mut buf).expect("encode ang");
    assert_eq!(decode_descriptor(&buf[..n_ang]).expect("decode ang"), ang);
}

#[test]
fn indpack_verifies_sha256_and_rejects_unsigned_or_bad_signatures() {
    let desc = sample_volume_descriptor();
    let mut pack = [0u8; 1024];
    let len = pack_indpack(
        &desc,
        &MINIMAL_WASM,
        SIG_ALG_KEYED_SHA256,
        |prefix, sha, sig_out| {
            let mac = keyed_mac(prefix, sha);
            sig_out[..32].copy_from_slice(&mac);
            Ok(32)
        },
        &mut pack,
    )
    .expect("pack succeeds");

    // Default RequireSignature always refuses when no trusted key is configured.
    assert_eq!(
        unpack_indpack(&pack[..len], &RequireSignature).unwrap_err(),
        IndpackError::Signature(SignatureError::NoTrustedKeyConfigured)
    );

    let verifier = VerifyFn(|prefix: &[u8], sha: &[u8; 32], alg: u8, sig: &[u8]| {
        if alg != SIG_ALG_KEYED_SHA256 || sig.len() != 32 {
            return Err(SignatureError::UnsupportedAlgorithm(alg));
        }
        let expected = keyed_mac(prefix, sha);
        if sig == expected {
            Ok(())
        } else {
            Err(SignatureError::BadSignature)
        }
    });

    let unpacked = unpack_indpack(&pack[..len], &verifier).expect("verified unpack");
    assert_eq!(unpacked.descriptor, desc);
    assert_eq!(unpacked.wasm_bytes, &MINIMAL_WASM);

    // Corrupting one byte of Wasm triggers Sha256Mismatch before signature check.
    let mut bad_wasm = pack;
    let wasm_last = len - 3 - 32 - 32 - 1;
    bad_wasm[wasm_last] ^= 0xFF;
    assert_eq!(
        unpack_indpack(&bad_wasm[..len], &verifier).unwrap_err(),
        IndpackError::Sha256Mismatch
    );

    // Corrupting one byte of signature triggers BadSignature.
    let mut bad_sig = pack;
    bad_sig[len - 1] ^= 0x01;
    assert_eq!(
        unpack_indpack(&bad_sig[..len], &verifier).unwrap_err(),
        IndpackError::Signature(SignatureError::BadSignature)
    );

    // Unsigned algorithm byte is rejected even by VerifyFn.
    let mut unsigned_pack = pack;
    unsigned_pack[len - 32 - 3] = SIG_ALG_NONE;
    assert_eq!(
        unpack_indpack(&unsigned_pack[..len], &verifier).unwrap_err(),
        IndpackError::Signature(SignatureError::UnsignedRejected)
    );
}
