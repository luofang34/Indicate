//! Safe interpreted Wasm host for `.indpack` instrument plugins (`PLUG-01`).
//!
//! Third-party instrument plugins run inside [`wasmi`] with zero WASI or
//! ambient host imports, a strict `<= 4` page (`256 KiB`) linear-memory ceiling,
//! and deterministic per-frame fuel metering ([`DEFAULT_FRAME_FUEL`]). Every
//! emitted 2D, angular, or 3D volumetric scene buffer is validated fail-closed
//! before leaving [`LoadedPlugin::draw`], and [`admit_plugin`] runs the full
//! canonical-state and withholding provenance sweep at load/integration time.

mod admit;
mod error;
mod host;

#[cfg(test)]
mod tests;

pub use admit::{PluginAdmissionReport, admit_plugin};
pub use error::PluginHostError;
pub use host::{
    DEFAULT_FRAME_FUEL, LoadedPlugin, MAX_WASM_MEMORY_PAGES, PluginDrawReport, PluginDrawTarget,
};
