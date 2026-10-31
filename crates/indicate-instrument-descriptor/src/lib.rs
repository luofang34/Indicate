//! The instrument descriptor vocabulary: identity, render spaces, required
//! layers and groups, underlay contracts, draw contexts, and canonical states
//! (ADR-0029, ADR-0033).

#![no_std]

#[cfg(test)]
extern crate std;

mod aux;
mod config;
mod context;
mod criticality;
mod descriptor;
mod frame;
mod group_set;
mod set;
mod space;
pub mod states;

pub use aux::{AuxChannelId, AuxEntry, MAX_AUX_ENTRIES, custom_aux_id};
pub use config::{CONFIG_BLOB_MAX, ConfigBlob, ConfigError, ConfigKey, EMPTY_CONFIG, keys};
pub use context::{
    DRAW_CONTEXT_HEADER_BYTES, DRAW_CONTEXT_WIRE_VERSION, DecodedDrawContext, DrawContext,
    DrawContextWireError, decode_draw_context, encode_draw_context,
};
pub use criticality::{CriticalityBands, PanelCriticality};
pub use descriptor::{
    DesignFrame, DrawFn, ExtremeState, InstrumentDescriptor, InstrumentDrawError, PanelDescriptor,
    PanelDrawError, Region,
};
pub use frame::{ExtentRefusal, FRAME_STEP_TOLERANCE, FrameRefusal};
pub use group_set::GroupSet;
pub use indicate_interaction::{ViewLimits, ViewPose3D};
pub use set::{InstrumentSet, PanelSet};
pub use space::{
    RenderSpace, SceneTarget, UnderlayContract, UnderlayStatus, angular_layer_bit,
    angular_overlay_layer_bit, planar_layer_bit, volume_band_bit,
};
pub use states::{CANONICAL_STATES, CanonicalState};
