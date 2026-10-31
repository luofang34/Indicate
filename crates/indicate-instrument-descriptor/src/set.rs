//! Instrument sets: the packaging unit a shell names when composing a registry.

use crate::descriptor::InstrumentDescriptor;

/// A named group of instruments contributed by one provider crate.
#[derive(Debug, Clone, Copy)]
pub struct InstrumentSet {
    /// Set identity, under the same charset rule as instrument ids.
    pub id: &'static str,
    /// The set's instruments, in the order they compose.
    pub panels: &'static [InstrumentDescriptor],
}

impl InstrumentSet {
    /// Returns the instruments contained in this set.
    #[must_use]
    pub const fn instruments(&self) -> &'static [InstrumentDescriptor] {
        self.panels
    }
}

/// Alias preserving compatibility with existing planar panel consumers.
pub type PanelSet = InstrumentSet;
