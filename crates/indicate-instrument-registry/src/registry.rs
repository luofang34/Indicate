//! Registry composition and its init-time validation.

use indicate_instrument_scene::LAYER_COUNT;

use crate::descriptor::PanelDescriptor;
use crate::set::PanelSet;

/// How a shell named the panels it composed.
///
/// A shell with one provider crate passes a slice and never sees sets;
/// a shell composing several names the sets. Both are `Copy` and hold
/// only `'static` references, so a registry stays allocation-free —
/// the family has no allocator with which to concatenate slices.
#[derive(Debug, Clone, Copy)]
enum Composition {
    /// One unnamed set: the single-provider shell.
    Anonymous(&'static [PanelDescriptor]),
    /// Named sets, in composition order.
    Sets(&'static [&'static PanelSet]),
}

/// A validated panel composition. Construction is the gate: a shell
/// that composes nonsense fails at init, not at draw time.
#[derive(Debug, Clone, Copy)]
pub struct Registry {
    composition: Composition,
}

/// The composed panels: set order, then panel order within each set.
///
/// The flattened order is contractual — it is the order
/// [`crate::scene_digest`] streams — so a shell's set list is its
/// composition order.
///
/// `Clone` but deliberately not `Copy`, following the slice iterators:
/// a `Copy` iterator can be consumed through a copy while the original
/// silently stays where it was, and a half-read composition that looks
/// whole is the wrong thing to make easy here.
#[derive(Debug, Clone)]
pub struct Panels {
    composition: Composition,
    set: usize,
    panel: usize,
}

impl Iterator for Panels {
    type Item = &'static PanelDescriptor;

    fn next(&mut self) -> Option<Self::Item> {
        match self.composition {
            Composition::Anonymous(panels) => {
                let next = panels.get(self.panel)?;
                self.panel += 1;
                Some(next)
            }
            Composition::Sets(sets) => loop {
                let set = sets.get(self.set)?;
                match set.panels.get(self.panel) {
                    Some(next) => {
                        self.panel += 1;
                        return Some(next);
                    }
                    // An exhausted set yields to the next. Empty sets
                    // are refused at construction, so in a validated
                    // registry this advances at most once per call.
                    None => {
                        self.set += 1;
                        self.panel = 0;
                    }
                }
            },
        }
    }
}

/// Why a composition was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RegistryError {
    /// A shell with no panels has nothing to display.
    #[error("a registry must contain at least one panel")]
    Empty,
    /// A composition naming no sets has nothing to display, and would
    /// pass the per-panel checks vacuously.
    #[error("a registry must contain at least one set")]
    NoSets,
    /// A set id violates the lowercase/digits/dashes charset.
    #[error("set {set} has a malformed id")]
    BadSetId {
        /// Position in the composed set list.
        set: usize,
    },
    /// Two sets share an id, so neither can be named unambiguously.
    #[error("set {set} repeats an earlier set's id")]
    DuplicateSetId {
        /// Position of the second occurrence.
        set: usize,
    },
    /// A set contributing no panels is a provider wired up wrongly, not
    /// a shell that wanted nothing.
    #[error("set {set} contributes no panels")]
    EmptySet {
        /// Position in the composed set list.
        set: usize,
    },
    /// A panel id violates the lowercase/digits/dashes charset.
    #[error("panel {index} has a malformed id")]
    BadId {
        /// Position in the flattened composition.
        index: usize,
    },
    /// Two panels share an id.
    #[error("panel {index} repeats an earlier panel's id")]
    DuplicateId {
        /// Position of the second occurrence.
        index: usize,
    },
    /// An empty title cannot label health or layout surfaces.
    #[error("panel {index} has an empty title")]
    EmptyTitle {
        /// Position in the flattened composition.
        index: usize,
    },
    /// A panel that requires no layers would pass every completeness
    /// check vacuously.
    #[error("panel {index} declares no required layers")]
    NoRequiredLayers {
        /// Position in the flattened composition.
        index: usize,
    },
    /// Required-layer bits beyond the defined scene layers.
    #[error("panel {index} requires undefined layer bits {bits:#04x}")]
    UndefinedLayerBits {
        /// Position in the flattened composition.
        index: usize,
        /// The offending mask.
        bits: u8,
    },
    /// A non-finite or non-positive design frame.
    #[error("panel {index} has a degenerate design frame")]
    BadDesignFrame {
        /// Position in the flattened composition.
        index: usize,
    },
    /// Schema keys must be strictly ascending (unique by construction).
    #[error("panel {index} schema key {key} repeats or descends")]
    SchemaKeysNotAscending {
        /// Position in the flattened composition.
        index: usize,
        /// The out-of-order key.
        key: u16,
    },
    /// A group region for a group the panel does not consume.
    #[error("panel {index} declares a region for group {group} it does not require")]
    RegionGroupNotRequired {
        /// Position in the flattened composition.
        index: usize,
        /// The wire tag of the unrequired group.
        group: u8,
    },
    /// A group region outside the design frame (or degenerate).
    #[error("panel {index} declares a region for group {group} outside its design frame")]
    RegionOutsideFrame {
        /// Position in the flattened composition.
        index: usize,
        /// The wire tag of the group.
        group: u8,
    },
    /// Two extreme states of one panel share an id.
    #[error("panel {index} repeats the extreme-state id at position {position}")]
    DuplicateExtremeId {
        /// Position in the flattened composition.
        index: usize,
        /// Position of the second occurrence within the panel.
        position: usize,
    },
    /// An extreme-state id violates the lowercase/digits/dashes charset.
    #[error("panel {index} extreme state {position} has a malformed id")]
    BadExtremeId {
        /// Position in the flattened composition.
        index: usize,
        /// Position of the offending extreme state within the panel.
        position: usize,
    },
}

/// Bits a required-layer mask may set: one per defined scene layer.
/// The u16 intermediate keeps the shift well-defined right up to the
/// mask's own u8 capacity; growing past eight layers must widen the
/// descriptor mask deliberately, not overflow silently.
const DEFINED_LAYER_BITS: u8 = {
    assert!(LAYER_COUNT <= 8, "layer mask is a u8");
    ((1u16 << LAYER_COUNT) - 1) as u8
};

fn id_ok(id: &str) -> bool {
    !id.is_empty()
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

impl Registry {
    /// Validates and composes `panels` as a single unnamed set — the
    /// shell that draws from one provider crate.
    pub fn new(panels: &'static [PanelDescriptor]) -> Result<Registry, RegistryError> {
        if panels.is_empty() {
            return Err(RegistryError::Empty);
        }
        Registry::validated(Composition::Anonymous(panels))
    }

    /// Validates and composes `sets`, in the order the shell lists
    /// them.
    ///
    /// Every rule [`Registry::new`] applies runs over the flattened
    /// composition, so two sets contributing the same panel id fail
    /// here rather than resolving to whichever set was listed first.
    pub fn from_sets(sets: &'static [&'static PanelSet]) -> Result<Registry, RegistryError> {
        if sets.is_empty() {
            return Err(RegistryError::NoSets);
        }
        for (index, set) in sets.iter().enumerate() {
            if !id_ok(set.id) {
                return Err(RegistryError::BadSetId { set: index });
            }
            if set.panels.is_empty() {
                return Err(RegistryError::EmptySet { set: index });
            }
            if sets[..index].iter().any(|earlier| earlier.id == set.id) {
                return Err(RegistryError::DuplicateSetId { set: index });
            }
        }
        Registry::validated(Composition::Sets(sets))
    }

    /// The per-panel rules, run over the flattened composition.
    fn validated(composition: Composition) -> Result<Registry, RegistryError> {
        let registry = Registry { composition };
        // Both constructors already refuse a composition that yields no
        // panels, so this is unreachable today. It stays because it is
        // the invariant the per-panel loop below depends on, and a
        // third `Composition` shape would otherwise inherit a loop that
        // passes by iterating nothing.
        if registry.panels().next().is_none() {
            return Err(RegistryError::Empty);
        }
        for (index, panel) in registry.panels().enumerate() {
            validate_panel(index, panel)?;
            if registry
                .panels()
                .take(index)
                .any(|earlier| earlier.id == panel.id)
            {
                return Err(RegistryError::DuplicateId { index });
            }
        }
        Ok(registry)
    }

    /// The composed descriptors, in shell order.
    pub const fn panels(&self) -> Panels {
        Panels {
            composition: self.composition,
            set: 0,
            panel: 0,
        }
    }

    /// The sets this registry composes, in shell order; empty for a
    /// registry built from a bare slice, which named none.
    pub const fn sets(&self) -> &'static [&'static PanelSet] {
        match self.composition {
            Composition::Anonymous(_) => &[],
            Composition::Sets(sets) => sets,
        }
    }

    /// The descriptor with `id`, if composed.
    pub fn by_id(&self, id: &str) -> Option<&'static PanelDescriptor> {
        self.panels().find(|panel| panel.id == id)
    }
}

fn validate_panel(index: usize, panel: &PanelDescriptor) -> Result<(), RegistryError> {
    if !id_ok(panel.id) {
        return Err(RegistryError::BadId { index });
    }
    if panel.title.is_empty() {
        return Err(RegistryError::EmptyTitle { index });
    }
    if panel.required_layers == 0 {
        return Err(RegistryError::NoRequiredLayers { index });
    }
    if panel.required_layers & !DEFINED_LAYER_BITS != 0 {
        return Err(RegistryError::UndefinedLayerBits {
            index,
            bits: panel.required_layers,
        });
    }
    let frame = panel.design_frame;
    if !(frame.width.is_finite()
        && frame.height.is_finite()
        && frame.width > 0.0
        && frame.height > 0.0)
    {
        return Err(RegistryError::BadDesignFrame { index });
    }
    let mut previous: Option<u16> = None;
    for key in panel.config_schema {
        if previous.is_some_and(|previous| key.0 <= previous) {
            return Err(RegistryError::SchemaKeysNotAscending { index, key: key.0 });
        }
        previous = Some(key.0);
    }
    for (group, region) in panel.group_regions {
        if !panel.required_groups.contains(*group) {
            return Err(RegistryError::RegionGroupNotRequired {
                index,
                group: *group as u8,
            });
        }
        let inside = region.x >= 0.0
            && region.y >= 0.0
            && region.width > 0.0
            && region.height > 0.0
            && region.x + region.width <= frame.width
            && region.y + region.height <= frame.height;
        if !inside {
            return Err(RegistryError::RegionOutsideFrame {
                index,
                group: *group as u8,
            });
        }
    }
    for (position, extreme) in panel.extreme_states.iter().enumerate() {
        if !id_ok(extreme.id) {
            return Err(RegistryError::BadExtremeId { index, position });
        }
        if panel.extreme_states[..position]
            .iter()
            .any(|earlier| earlier.id == extreme.id)
        {
            return Err(RegistryError::DuplicateExtremeId { index, position });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
