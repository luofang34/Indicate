//! Contacts that a simulator supplies for display; the set never tracks or classifies.
use indicate_instrument_state::SignalStatus;

/// Number of contacts one frame can carry.
pub const MAX_CONTACTS: usize = 16;

/// Identity that the host source declares for a contact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Identity {
    /// No identity is declared.
    #[default]
    Unknown,
    /// Declared friendly.
    Friendly,
    /// Declared hostile.
    Hostile,
}

/// Whether a contact flies or sits on the surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Domain {
    /// An aircraft.
    #[default]
    Air,
    /// A surface target: a vehicle, a site, or a ship.
    Surface,
}

/// One contact relative to the aircraft, in local north/east/down metres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Contact {
    /// Contact position minus aircraft position, north/east/down metres.
    pub relative_position_m: [f32; 3],
    /// Contact velocity minus aircraft velocity, north/east/down metres per second.
    pub relative_velocity_mps: [f32; 3],
    /// Declared identity.
    pub identity: Identity,
    /// Air or surface.
    pub domain: Domain,
    /// Trust in this contact; only `Valid` and `Stale` contacts are drawn.
    pub status: SignalStatus,
}

impl Default for Contact {
    fn default() -> Self {
        Self {
            relative_position_m: [0.0; 3],
            relative_velocity_mps: [0.0; 3],
            identity: Identity::Unknown,
            domain: Domain::Air,
            status: SignalStatus::Missing,
        }
    }
}

impl Contact {
    /// Range to the contact, metres; `None` when the position is not finite.
    pub fn range_m(&self) -> Option<f32> {
        let [n, e, d] = self.relative_position_m;
        let range = libm::sqrtf(n * n + e * e + d * d);
        range.is_finite().then_some(range)
    }

    pub(crate) fn drawable(&self) -> bool {
        matches!(self.status, SignalStatus::Valid | SignalStatus::Stale)
            && self.range_m().is_some_and(|range| range > 1.0)
            && self.relative_velocity_mps.iter().all(|v| v.is_finite())
    }
}

/// A bounded set of contacts with an optional designated contact.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TargetPicture {
    contacts: [Contact; MAX_CONTACTS],
    count: usize,
    designated: Option<usize>,
}

impl TargetPicture {
    /// Adds a contact and returns its index; `None` when the picture is full.
    pub fn push(&mut self, contact: Contact) -> Option<usize> {
        let index = self.count;
        let slot = self.contacts.get_mut(index)?;
        *slot = contact;
        self.count = index.wrapping_add(1);
        Some(index)
    }

    /// Designates a contact by index; an index without a contact clears the designation.
    pub fn designate(&mut self, index: Option<usize>) {
        self.designated = index.filter(|i| *i < self.count);
    }

    /// The contacts in insertion order.
    pub fn contacts(&self) -> &[Contact] {
        self.contacts.get(..self.count).unwrap_or(&[])
    }

    /// The designated contact index, if any.
    pub fn designated_index(&self) -> Option<usize> {
        self.designated
    }

    /// The designated contact, if any.
    pub fn designated(&self) -> Option<&Contact> {
        self.designated.and_then(|i| self.contacts().get(i))
    }
}
