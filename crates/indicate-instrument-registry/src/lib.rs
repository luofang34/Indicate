//! Instrument composition and cross-shell identity (ADR-0029, ADR-0033).
//!
//! A registry is plain data composed by each shell from instrument
//! descriptors and validated at init; an out-of-repo instrument registers by
//! being named in the shell's composition, never by link-time magic.

#![no_std]

#[cfg(test)]
extern crate std;

mod composition;
mod digest;
mod registry;

pub use composition::{
    COMPOSITION_DIGEST_DOMAIN, CompositionDescriptor, CompositionError, MAX_COMPOSITION_SLOTS,
    Slot, composition_digest, validate_composition,
};
pub use digest::{DigestError, SCENE_DIGEST_DOMAIN, scene_digest};
pub use indicate_instrument_descriptor::states;
pub use indicate_instrument_descriptor::{
    AuxChannelId, AuxEntry, CANONICAL_STATES, CONFIG_BLOB_MAX, CanonicalState, ConfigBlob,
    ConfigError, ConfigKey, CriticalityBands, DRAW_CONTEXT_HEADER_BYTES, DRAW_CONTEXT_WIRE_VERSION,
    DecodedDrawContext, DesignFrame, DrawContext, DrawContextWireError, DrawFn, EMPTY_CONFIG,
    ExtentRefusal, ExtremeState, FRAME_STEP_TOLERANCE, FrameRefusal, GroupSet,
    InstrumentDescriptor, InstrumentDrawError, InstrumentSet, MAX_AUX_ENTRIES, PanelCriticality,
    PanelDescriptor, PanelDrawError, PanelSet, Region, RenderSpace, SceneTarget, UnderlayContract,
    UnderlayStatus, ViewLimits, ViewPose3D, angular_layer_bit, angular_overlay_layer_bit,
    custom_aux_id, decode_draw_context, encode_draw_context, keys, planar_layer_bit,
    volume_band_bit,
};
pub use registry::{Panels, Registry, RegistryError};
