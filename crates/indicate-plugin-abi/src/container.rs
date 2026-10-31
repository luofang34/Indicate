//! Signed `.indpack` container codec and digest/signature verification (`PLUG-01`).

use indicate_sha256::Sha256Ctx;

use crate::descriptor::{
    DescriptorWireError, PluginDescriptorWire, decode_descriptor, encode_descriptor,
};

/// Magic header identifying an `.indpack` v1 container.
pub const INDPACK_MAGIC: [u8; 8] = *b"INDPACK1";

/// Container wire format version.
pub const INDPACK_FORMAT_VERSION: u8 = 1;

/// Guest/host linear-memory ABI version exported via `ind_abi_version()`.
pub const WASM_ABI_VERSION: u32 = 1;

/// Maximum Wasm module size permitted inside an `.indpack` (`256 KiB`).
pub const MAX_INDPACK_WASM_BYTES: usize = 256 * 1024;

/// Maximum serialized descriptor length inside an `.indpack`.
pub const MAX_INDPACK_DESCRIPTOR_BYTES: usize = 4096;

/// Unsigned sentinel algorithm byte (always rejected by production verifiers).
pub const SIG_ALG_NONE: u8 = 0;

/// Ed25519 detached signature algorithm identifier.
pub const SIG_ALG_ED25519: u8 = 1;

/// Keyed SHA-256 MAC signature algorithm identifier for deterministic test suites.
pub const SIG_ALG_KEYED_SHA256: u8 = 0xFE;

/// `target_kind` passed to `ind_draw` for [`PluginSpaceWire::Planar2D`](crate::PluginSpaceWire::Planar2D).
pub const TARGET_KIND_PLANAR_2D: i32 = 0;
/// `target_kind` passed to `ind_draw` for conformal angular sphere output.
pub const TARGET_KIND_ANGULAR_SPHERE: i32 = 1;
/// `target_kind` passed to `ind_draw` for 3D metric volume output.
pub const TARGET_KIND_VOLUME_3D: i32 = 2;
/// `target_kind` passed to `ind_draw` for optional 2D peripheral overlay on `AngularSphere`.
pub const TARGET_KIND_ANGULAR_OVERLAY_2D: i32 = 3;

const WASM_BINARY_HEADER: [u8; 8] = [0x00, b'a', b's', b'm', 0x01, 0x00, 0x00, 0x00];

/// Reason a `.indpack` signature failed verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SignatureError {
    /// Unsigned container (`sig_alg == 0` or empty signature) rejected fail-closed.
    #[error("unsigned .indpack container rejected")]
    UnsignedRejected,
    /// Default [`RequireSignature`] verifier has no trusted public key configured.
    #[error("no trusted signature verification key configured")]
    NoTrustedKeyConfigured,
    /// Unsupported signature algorithm byte.
    #[error("unsupported signature algorithm {0}")]
    UnsupportedAlgorithm(u8),
    /// Cryptographic signature check failed.
    #[error("signature verification failed")]
    BadSignature,
}

/// Pluggable cryptographic signature verifier for `.indpack` containers.
pub trait SignatureVerifier {
    /// Verifies `sig_bytes` under `sig_alg` over `signed_prefix` (everything from
    /// `INDPACK_MAGIC` through `wasm_sha256`).
    fn verify(
        &self,
        signed_prefix: &[u8],
        wasm_sha256: &[u8; 32],
        sig_alg: u8,
        sig_bytes: &[u8],
    ) -> Result<(), SignatureError>;
}

/// Fail-closed default verifier: rejects both unsigned packs and unconfigured keys.
#[derive(Debug, Clone, Copy, Default)]
pub struct RequireSignature;

impl SignatureVerifier for RequireSignature {
    fn verify(
        &self,
        _signed_prefix: &[u8],
        _wasm_sha256: &[u8; 32],
        sig_alg: u8,
        sig_bytes: &[u8],
    ) -> Result<(), SignatureError> {
        if sig_alg == SIG_ALG_NONE || sig_bytes.is_empty() {
            return Err(SignatureError::UnsignedRejected);
        }
        Err(SignatureError::NoTrustedKeyConfigured)
    }
}

/// Function-backed signature verifier; still rejects `SIG_ALG_NONE` or empty signatures first.
pub struct VerifyFn<F>(pub F);

impl<F> SignatureVerifier for VerifyFn<F>
where
    F: Fn(&[u8], &[u8; 32], u8, &[u8]) -> Result<(), SignatureError>,
{
    fn verify(
        &self,
        signed_prefix: &[u8],
        wasm_sha256: &[u8; 32],
        sig_alg: u8,
        sig_bytes: &[u8],
    ) -> Result<(), SignatureError> {
        if sig_alg == SIG_ALG_NONE || sig_bytes.is_empty() {
            return Err(SignatureError::UnsignedRejected);
        }
        (self.0)(signed_prefix, wasm_sha256, sig_alg, sig_bytes)
    }
}

/// Verified contents unpacked from an `.indpack` container.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UnpackedIndpack<'a> {
    /// Validated static plugin metadata.
    pub descriptor: PluginDescriptorWire,
    /// Verified Wasm module bytecode.
    pub wasm_bytes: &'a [u8],
    /// Verified SHA-256 digest of `wasm_bytes`.
    pub wasm_sha256: [u8; 32],
    /// Signature algorithm identifier.
    pub sig_alg: u8,
    /// Detached signature bytes.
    pub sig_bytes: &'a [u8],
}

/// Fail-closed error returned when packing or unpacking an `.indpack` container.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum IndpackError {
    /// Output buffer too small when packing.
    #[error("indpack buffer too small: needed {needed}, got {available}")]
    BufferTooSmall {
        /// Needed bytes.
        needed: usize,
        /// Available bytes.
        available: usize,
    },
    /// Container truncated or carries trailing bytes.
    #[error("indpack container truncated or has trailing bytes")]
    InvalidLength,
    /// Container magic header is not `INDPACK1`.
    #[error("invalid indpack magic header")]
    BadMagic,
    /// Unsupported container format version.
    #[error("unsupported indpack version {0}")]
    UnsupportedVersion(u8),
    /// Descriptor section exceeds size limit or failed decoding.
    #[error("descriptor error: {0}")]
    Descriptor(#[from] DescriptorWireError),
    /// Wasm bytecode section is empty, oversized, or lacks `\0asm\x01\0\0\0`.
    #[error("invalid or oversized Wasm module payload")]
    InvalidWasmModule,
    /// Embedded `wasm_sha256` does not match `Sha256(wasm_bytes)`.
    #[error("wasm SHA-256 digest mismatch")]
    Sha256Mismatch,
    /// Container signature check failed.
    #[error("signature error: {0}")]
    Signature(#[from] SignatureError),
}

/// Packs a descriptor, Wasm module, and caller-supplied signer into an `.indpack` buffer.
pub fn pack_indpack<S>(
    descriptor: &PluginDescriptorWire,
    wasm_bytes: &[u8],
    sig_alg: u8,
    sign: S,
    out: &mut [u8],
) -> Result<usize, IndpackError>
where
    S: FnOnce(&[u8], &[u8; 32], &mut [u8]) -> Result<usize, SignatureError>,
{
    if wasm_bytes.len() < WASM_BINARY_HEADER.len()
        || wasm_bytes.len() > MAX_INDPACK_WASM_BYTES
        || wasm_bytes[..8] != WASM_BINARY_HEADER
    {
        return Err(IndpackError::InvalidWasmModule);
    }
    let mut desc_buf = [0u8; MAX_INDPACK_DESCRIPTOR_BYTES];
    let desc_len = encode_descriptor(descriptor, &mut desc_buf)?;
    let prefix_len = 8 + 1 + 4 + desc_len + 4 + wasm_bytes.len() + 32;
    if out.len() < prefix_len + 3 {
        return Err(IndpackError::BufferTooSmall {
            needed: prefix_len + 3,
            available: out.len(),
        });
    }
    out[0..8].copy_from_slice(&INDPACK_MAGIC);
    out[8] = INDPACK_FORMAT_VERSION;
    out[9..13].copy_from_slice(&(desc_len as u32).to_le_bytes());
    let d_end = 13 + desc_len;
    out[13..d_end].copy_from_slice(&desc_buf[..desc_len]);
    out[d_end..d_end + 4].copy_from_slice(&(wasm_bytes.len() as u32).to_le_bytes());
    let w_start = d_end + 4;
    let w_end = w_start + wasm_bytes.len();
    out[w_start..w_end].copy_from_slice(wasm_bytes);

    let wasm_sha256 = sha256_bytes(wasm_bytes);
    out[w_end..prefix_len].copy_from_slice(&wasm_sha256);

    let mut sig_scratch = [0u8; 512];
    let sig_len = sign(&out[..prefix_len], &wasm_sha256, &mut sig_scratch)?;
    let sig_len_u16 = u16::try_from(sig_len).map_err(|_| IndpackError::InvalidLength)?;
    let total = prefix_len + 3 + sig_len;
    if out.len() < total {
        return Err(IndpackError::BufferTooSmall {
            needed: total,
            available: out.len(),
        });
    }
    out[prefix_len] = sig_alg;
    out[prefix_len + 1..prefix_len + 3].copy_from_slice(&sig_len_u16.to_le_bytes());
    out[prefix_len + 3..total].copy_from_slice(&sig_scratch[..sig_len]);
    Ok(total)
}

/// Unpacks and verifies an `.indpack` container fail-closed.
pub fn unpack_indpack<'a>(
    bytes: &'a [u8],
    verifier: &dyn SignatureVerifier,
) -> Result<UnpackedIndpack<'a>, IndpackError> {
    if bytes.len() < 52 {
        return Err(IndpackError::InvalidLength);
    }
    if bytes[0..8] != INDPACK_MAGIC {
        return Err(IndpackError::BadMagic);
    }
    if bytes[8] != INDPACK_FORMAT_VERSION {
        return Err(IndpackError::UnsupportedVersion(bytes[8]));
    }
    let desc_len =
        usize::try_from(read_u32(&bytes[9..13])).map_err(|_| IndpackError::InvalidLength)?;
    if desc_len == 0 || desc_len > MAX_INDPACK_DESCRIPTOR_BYTES {
        return Err(IndpackError::InvalidLength);
    }
    let d_end = 13usize
        .checked_add(desc_len)
        .ok_or(IndpackError::InvalidLength)?;
    let w_hdr_end = d_end.checked_add(4).ok_or(IndpackError::InvalidLength)?;
    if bytes.len() < w_hdr_end {
        return Err(IndpackError::InvalidLength);
    }
    let wasm_len = usize::try_from(read_u32(&bytes[d_end..w_hdr_end]))
        .map_err(|_| IndpackError::InvalidLength)?;
    if wasm_len < WASM_BINARY_HEADER.len() || wasm_len > MAX_INDPACK_WASM_BYTES {
        return Err(IndpackError::InvalidWasmModule);
    }
    let w_end = w_hdr_end
        .checked_add(wasm_len)
        .ok_or(IndpackError::InvalidLength)?;
    let prefix_len = w_end.checked_add(32).ok_or(IndpackError::InvalidLength)?;
    let sig_hdr_end = prefix_len
        .checked_add(3)
        .ok_or(IndpackError::InvalidLength)?;
    if bytes.len() < sig_hdr_end {
        return Err(IndpackError::InvalidLength);
    }

    let wasm_bytes = &bytes[w_hdr_end..w_end];
    if wasm_bytes[..8] != WASM_BINARY_HEADER {
        return Err(IndpackError::InvalidWasmModule);
    }
    let mut wasm_sha256 = [0u8; 32];
    wasm_sha256.copy_from_slice(&bytes[w_end..prefix_len]);
    let actual_sha = sha256_bytes(wasm_bytes);
    if !constant_time_eq_32(&wasm_sha256, &actual_sha) {
        return Err(IndpackError::Sha256Mismatch);
    }

    let sig_alg = bytes[prefix_len];
    let sig_len = usize::from(u16::from_le_bytes([
        bytes[prefix_len + 1],
        bytes[prefix_len + 2],
    ]));
    let total = sig_hdr_end
        .checked_add(sig_len)
        .ok_or(IndpackError::InvalidLength)?;
    if bytes.len() != total {
        return Err(IndpackError::InvalidLength);
    }
    let sig_bytes = &bytes[sig_hdr_end..total];
    verifier.verify(&bytes[..prefix_len], &wasm_sha256, sig_alg, sig_bytes)?;

    let descriptor = decode_descriptor(&bytes[13..d_end])?;
    Ok(UnpackedIndpack {
        descriptor,
        wasm_bytes,
        wasm_sha256,
        sig_alg,
        sig_bytes,
    })
}

fn sha256_bytes(data: &[u8]) -> [u8; 32] {
    let mut ctx = Sha256Ctx::new();
    ctx.update(data);
    ctx.finalize()
}

fn constant_time_eq_32(a: &[u8; 32], b: &[u8; 32]) -> bool {
    let mut diff = 0u8;
    let mut i = 0;
    while i < 32 {
        diff |= a[i] ^ b[i];
        i = i.wrapping_add(1);
    }
    diff == 0
}

fn read_u32(s: &[u8]) -> u32 {
    u32::from_le_bytes([s[0], s[1], s[2], s[3]])
}
