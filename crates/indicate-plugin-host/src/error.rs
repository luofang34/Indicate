//! Typed errors returned by [`crate::LoadedPlugin`] and [`crate::admit_plugin`].

use indicate_instrument_descriptor::DrawContextWireError;
use indicate_instrument_scene::LayerError;
use indicate_instrument_state::GroupId;
use indicate_plugin_abi::IndpackError;

/// Fail-closed error returned when loading, executing, or admitting a Wasm plugin.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum PluginHostError {
    /// `.indpack` container, digest, or signature verification failed.
    #[error("indpack error: {0}")]
    Indpack(#[from] IndpackError),
    /// Wasm module declares host imports (zero ambient imports permitted).
    #[error("wasm module declares forbidden host imports")]
    HostImportsForbidden,
    /// Wasm module failed validation, compilation, or instantiation.
    #[error("wasm module error: {0}")]
    WasmModule(String),
    /// Wasm linear memory exceeds [`crate::MAX_WASM_MEMORY_PAGES`] (`256 KiB`) or uses memory64.
    #[error("wasm linear memory exceeds the 4-page (256 KiB) sandbox limit")]
    MemoryLimitExceeded,
    /// Required guest export (`memory`, `ind_abi_version`, `ind_scratch_ptr`, `ind_scratch_cap`, `ind_draw`) missing or mistyped.
    #[error("missing or mistyped guest export `{0}`")]
    MissingExport(&'static str),
    /// Guest `ind_abi_version()` did not return [`indicate_plugin_abi::WASM_ABI_VERSION`].
    #[error("unsupported guest ABI version {0}")]
    UnsupportedAbiVersion(i32),
    /// Guest scratch window `[ptr .. ptr + cap]` is out of bounds or too small.
    #[error("invalid guest scratch window ptr={ptr} cap={cap}")]
    InvalidScratchWindow {
        /// Guest scratch pointer.
        ptr: i32,
        /// Guest scratch capacity.
        cap: i32,
    },
    /// Draw context wire encoding failed or exceeded guest scratch window.
    #[error("draw context wire error: {0}")]
    ContextWire(#[from] DrawContextWireError),
    /// Requested [`crate::PluginDrawTarget`] does not match the plugin's declared render space or bounds.
    #[error("draw target incompatible with plugin descriptor space or bounds")]
    TargetMismatch,
    /// Deterministic fuel budget exhausted during guest execution.
    #[error("guest execution exhausted its deterministic fuel budget")]
    OutOfFuel,
    /// Guest Wasm trapped during execution.
    #[error("guest trap: {0}")]
    GuestTrap(String),
    /// Guest `ind_draw` returned a negative error code.
    #[error("guest ind_draw returned error code {0}")]
    GuestDrawError(i32),
    /// Guest returned a byte count exceeding `out_cap` or caller output buffer.
    #[error("guest scene length {written} exceeds capacity {cap}")]
    SceneOverflow {
        /// Bytes reported by guest.
        written: usize,
        /// Permitted output capacity.
        cap: usize,
    },
    /// Emitted scene violated the 2D, angular, or 3D band/layer isolation contract.
    #[error("emitted scene violated layer/band contract: {0}")]
    SceneLayer(#[from] LayerError),
    /// Emitted scene omitted one or more declared `required_layers` / `VolumeBandId` bits.
    #[error("emitted scene missing required layer/band bits {0:#06x}")]
    MissingRequiredLayers(u16),
    /// Emitted text contains a character outside `PANEL_VOCABULARY`.
    #[error("plugin draws uncovered glyph {ch:?} in state {state}")]
    GlyphCoverage {
        /// Corpus state ID.
        state: &'static str,
        /// Uncovered character.
        ch: char,
    },
    /// Emitted scene has a dangling or misplaced `ATTRIBUTE` provenance tag.
    #[error("plugin emitted misplaced ATTRIBUTE provenance tag in state {state}")]
    MisplacedClaim {
        /// Corpus state ID.
        state: &'static str,
    },
    /// Emitted numeric text lacks a preceding `ATTRIBUTE` provenance tag.
    #[error("plugin emitted untagged numeral {text:?} in state {state}")]
    UntaggedNumeral {
        /// Corpus state ID.
        state: &'static str,
        /// Offending numeric run.
        text: String,
    },
    /// Emitted text claims configuration provenance or a group outside `required_groups`.
    #[error("plugin emitted foreign or config provenance tag {tag:#04x} for {text:?} in {state}")]
    InvalidProvenanceClaim {
        /// Corpus state ID.
        state: &'static str,
        /// Tag byte.
        tag: u8,
        /// Claiming text run.
        text: String,
    },
    /// Emitted visible text claims a group that is withheld in the current case.
    #[error("plugin fabricated {text:?} for withheld group {group:?} in state {state}")]
    FabricatedNumeral {
        /// Corpus state ID.
        state: &'static str,
        /// Withheld group.
        group: GroupId,
        /// Offending text run.
        text: String,
    },
    /// Emitted scene violated the declared 2D or 3D underlay contract.
    #[error("plugin violated underlay contract in state {state}: {detail}")]
    UnderlayViolation {
        /// Corpus state ID.
        state: &'static str,
        /// Defect description.
        detail: &'static str,
    },
    /// Emitted 3D `TARGET_HANDLE` lies outside the volume extent bounds.
    #[error("plugin handle {target_id} at {center:?} outside extent bounds in state {state}")]
    HandleOutOfBounds {
        /// Corpus state ID.
        state: &'static str,
        /// Handle ID.
        target_id: u32,
        /// Volume-space handle center.
        center: [f32; 3],
    },
    /// Plugin declares a non-degenerate 2D frame or 3D extent range but ignores the size argument.
    #[error("plugin ignores declared frame or extent range across canonical sweep")]
    RangeIgnored,
}
