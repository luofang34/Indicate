//! Registry composition and its init-time validation.

use indicate_instrument_descriptor::{PanelDescriptor, PanelSet};

mod error;
mod frame_range;

pub use error::RegistryError;

#[derive(Debug, Clone, Copy)]
enum Composition {
    Anonymous(&'static [PanelDescriptor]),
    Sets(&'static [&'static PanelSet]),
}

/// A validated instrument composition. Construction is the gate.
#[derive(Debug, Clone, Copy)]
pub struct Registry {
    composition: Composition,
}

/// The composed instruments: set order, then order within each set.
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
                    None => {
                        self.set += 1;
                        self.panel = 0;
                    }
                }
            },
        }
    }
}

fn id_ok(id: &str) -> bool {
    !id.is_empty()
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

impl Registry {
    /// Validates and composes `panels` as a single unnamed set.
    pub fn new(panels: &'static [PanelDescriptor]) -> Result<Registry, RegistryError> {
        if panels.is_empty() {
            return Err(RegistryError::Empty);
        }
        Registry::validated(Composition::Anonymous(panels))
    }

    /// Validates and composes `sets`, in the order the shell lists them.
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

    fn validated(composition: Composition) -> Result<Registry, RegistryError> {
        let registry = Registry { composition };
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

    /// Alias for [`Self::panels`].
    pub const fn instruments(&self) -> Panels {
        self.panels()
    }

    /// The sets this registry composes, in shell order; empty for an anonymous slice.
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
    frame_range::validate(index, panel)?;
    let mut previous: Option<u16> = None;
    for key in panel.config_schema {
        if previous.is_some_and(|previous| key.0 <= previous) {
            return Err(RegistryError::SchemaKeysNotAscending { index, key: key.0 });
        }
        previous = Some(key.0);
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
