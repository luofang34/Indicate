//! The smallest panel set that passes admission, written to be read.
//!
//! A panel is a plugin over three stable contracts — the state-group
//! vocabulary, the scene-command IR, and the glyph vocabulary — and a
//! set is the unit a shell names when it composes panels. This crate is
//! one of each: a single [`TEMPLATE_DESCRIPTOR`] inside a single
//! [`TEMPLATE_SET`], drawing one label and one readout.
//!
//! It exists to be copied, so the comments say why each value is what it
//! is rather than what it contains. Four properties are what a copy has
//! to keep:
//!
//! - **The layer envelope.** Every band is opened, saved, drawn,
//!   restored and closed, in strictly ascending order and never nested.
//!   `SceneWriter::begin_layer`/`end_layer` emit the mandatory
//!   save/restore, so using them is the whole obligation.
//! - **Honest status.** A run showing a number claims the state group
//!   the number came from; a run showing dashes claims nothing. Getting
//!   this backwards is the failure a first set hits.
//! - **A background declaration the scene keeps.** Declaring
//!   [`BackgroundCapability::Opaque`](indicate_instrument_descriptor::BackgroundCapability)
//!   obliges the panel to cover the frame in the `Background` band, in
//!   every corpus case.
//! - **Kernel-only dependencies.** A set draws against the kernel and
//!   never against the tier that judges it.
//!
//! The crate's own test composes this set into a registry and runs the
//! `indicate-instrument-conformance` admission harness over it, so "the
//! template is admissible" is something the build establishes rather
//! than something this comment asserts.

#![no_std]

#[cfg(test)]
extern crate std;

mod template;

pub use template::{TEMPLATE_DESCRIPTOR, TEMPLATE_SET};
