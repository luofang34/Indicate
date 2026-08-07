//! Panel registry: the descriptor contract shells compose (ADR-0029,
//! ADR-0033).
//!
//! A panel is a plugin over three stable contracts — the state-group
//! vocabulary, the scene-command IR, and the glyph vocabulary. This
//! crate holds the descriptor a shell consumes instead of hard-coded
//! panel enumeration: identity, required layers and state groups, the
//! design frame, background capability, the bounded key-TLV
//! configuration schema, and the draw entry point. A registry is plain
//! data composed by each shell, validated at init; an out-of-repo panel
//! registers by being named in the shell's composition, never by
//! link-time magic.
//!
//! A shell drawing from one provider crate passes a descriptor slice to
//! [`Registry::new`]. A shell drawing from several names
//! [`PanelSet`]s and calls [`Registry::from_sets`], so adding an
//! instrument family is one declaration naming the set rather than one
//! line per panel — the by-hand panel list ADR-0029 removed does not
//! reappear one layer up. Both compositions are `Copy` and
//! allocation-free, and the same per-panel rules run over the flattened
//! traversal either way, so a panel id claimed by two sets is refused
//! at init.

#![no_std]

#[cfg(test)]
extern crate std;

mod config;
mod descriptor;
mod digest;
mod group_set;
mod registry;
mod set;
pub mod states;

pub use config::{CONFIG_BLOB_MAX, ConfigBlob, ConfigError, ConfigKey, EMPTY_CONFIG, keys};
pub use descriptor::{
    BackgroundCapability, DesignFrame, DrawFn, ExtremeState, PanelDescriptor, PanelDrawError,
    Region,
};
pub use digest::{DigestError, SCENE_DIGEST_DOMAIN, scene_digest};
pub use group_set::GroupSet;
pub use registry::{Panels, Registry, RegistryError};
pub use set::PanelSet;
pub use states::{CANONICAL_STATES, CanonicalState};
