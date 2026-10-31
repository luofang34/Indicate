//! Signed `.indpack` plugin container codec and Wasm guest/host ABI constants (`PLUG-01`).
//!
//! Third-party instruments ship as single-file `.indpack` archives containing a
//! validated static [`PluginDescriptorWire`], a pure-computation Wasm module, an
//! embedded SHA-256 module digest, and a detached cryptographic signature. This
//! crate is `#![no_std]` and zero-allocation so both embedded shells and Wasm
//! build tooling share identical wire validation.

#![no_std]

#[cfg(test)]
extern crate std;

mod container;
mod descriptor;

#[cfg(test)]
mod tests;

pub use container::{
    INDPACK_FORMAT_VERSION, INDPACK_MAGIC, IndpackError, MAX_INDPACK_DESCRIPTOR_BYTES,
    MAX_INDPACK_WASM_BYTES, RequireSignature, SIG_ALG_ED25519, SIG_ALG_KEYED_SHA256, SIG_ALG_NONE,
    SignatureError, SignatureVerifier, TARGET_KIND_ANGULAR_OVERLAY_2D, TARGET_KIND_ANGULAR_SPHERE,
    TARGET_KIND_PLANAR_2D, TARGET_KIND_VOLUME_3D, UnpackedIndpack, VerifyFn, WASM_ABI_VERSION,
    pack_indpack, unpack_indpack,
};
pub use descriptor::{
    DescriptorWireError, MAX_PLUGIN_AUX_CHANNELS, MAX_PLUGIN_CANONICAL, MAX_PLUGIN_CONFIG_KEYS,
    MAX_PLUGIN_ID_LEN, MAX_PLUGIN_INTENTS, MAX_PLUGIN_TITLE_LEN, PLUGIN_DESCRIPTOR_VERSION,
    PluginDescriptorWire, PluginSpaceWire, decode_descriptor, encode_descriptor,
};
