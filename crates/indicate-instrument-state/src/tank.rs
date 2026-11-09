//! Fuel tanks: what each tank's quantity gauge reads, for any number of
//! tanks.
//!
//! Fuel is held per tank, not per engine: one engine draws from several
//! tanks and a tank feeds several engines, so tanks are their own group,
//! indexed in the source's tank order. A dial binds a tank by index; the
//! cockpit's instrument model says which index is FRONT L, FRONT R,
//! REAR L and so on.
//!
//! The group has the engine group's shape ([`crate::samples`]): a
//! header in the owned state, and presence-masked samples beside it as a
//! [`TankSamples`] view. A tank reports volume, mass, or both, with the
//! capacity in the same terms, so a gauge in gallons reads volume and a
//! gauge in pounds reads mass without the display assuming a density.

use crate::samples::{RawSample, Sample, SampleGroup, SampleIter, Samples, raw_wire_bytes};
use crate::validate::GroupFault;

/// One field of the fuel tank registry: an f32 on the wire under the
/// presence-mask bit equal to its discriminant. Append-only.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TankField {
    /// Fuel volume in the tank, litres.
    QuantityL = 0,
    /// The tank's usable volume, litres.
    CapacityL = 1,
    /// Fuel mass in the tank, kilograms.
    QuantityKg = 2,
    /// The tank's usable mass at its rated density, kilograms.
    CapacityKg = 3,
}

impl TankField {
    /// Every registered field, in bit order.
    pub const ALL: [TankField; 4] = [
        TankField::QuantityL,
        TankField::CapacityL,
        TankField::QuantityKg,
        TankField::CapacityKg,
    ];

    /// This field's bit in a presence mask.
    #[must_use]
    pub const fn bit(self) -> u16 {
        1 << (self as u8)
    }
}

/// Every bit the registry assigns; any other bit faults the group.
pub const TANK_FIELDS_MASK: u16 = 0x000F;

/// Wire bytes of a tank sample whose presence mask is `mask`; `None`
/// when the mask names a field outside the registry.
#[must_use]
pub const fn tank_sample_wire_bytes(mask: u16) -> Option<usize> {
    raw_wire_bytes(mask, TANK_FIELDS_MASK)
}

/// The fuel tank group's header, carried in the owned state.
pub type TankGroup = SampleGroup;

/// The tank samples beside a state.
pub type TankSamples<'a> = Samples<'a, TankSample>;

/// Iterator over a [`TankSamples`] view.
pub type TankIter<'a> = SampleIter<'a, TankSample>;

/// One tank's readings. Every value is finite and not negative.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TankSample {
    /// Fuel volume, litres.
    pub quantity_l: Option<f32>,
    /// Usable volume, litres.
    pub capacity_l: Option<f32>,
    /// Fuel mass, kilograms.
    pub quantity_kg: Option<f32>,
    /// Usable mass, kilograms.
    pub capacity_kg: Option<f32>,
}

impl TankSample {
    /// A tank reporting nothing, for `..TankSample::EMPTY` in `const`
    /// literals.
    pub const EMPTY: TankSample = TankSample {
        quantity_l: None,
        capacity_l: None,
        quantity_kg: None,
        capacity_kg: None,
    };

    /// The reading of `field`, if this tank reports it.
    #[must_use]
    pub fn field(&self, field: TankField) -> Option<f32> {
        match field {
            TankField::QuantityL => self.quantity_l,
            TankField::CapacityL => self.capacity_l,
            TankField::QuantityKg => self.quantity_kg,
            TankField::CapacityKg => self.capacity_kg,
        }
    }

    /// Whether every present value is finite and not negative.
    #[must_use]
    pub fn is_sound(&self) -> bool {
        TankField::ALL
            .iter()
            .filter_map(|f| self.field(*f))
            .all(|v| v.is_finite() && v >= 0.0)
    }

    /// Wire bytes this sample encodes to.
    #[must_use]
    pub fn wire_bytes(&self) -> usize {
        Sample::wire_bytes(self)
    }
}

impl Sample for TankSample {
    const KNOWN: u16 = TANK_FIELDS_MASK;

    fn from_raw(raw: &RawSample) -> Self {
        TankSample {
            quantity_l: raw.get(TankField::QuantityL as u8),
            capacity_l: raw.get(TankField::CapacityL as u8),
            quantity_kg: raw.get(TankField::QuantityKg as u8),
            capacity_kg: raw.get(TankField::CapacityKg as u8),
        }
    }

    fn to_raw(&self) -> RawSample {
        let mut raw = RawSample::EMPTY;
        for field in TankField::ALL {
            raw.set(field as u8, self.field(field));
        }
        raw
    }

    fn is_sound(&self) -> bool {
        TankSample::is_sound(self)
    }
}

/// Checks a tank samples region against `count`; see [`Samples::parse`].
pub fn parse_tank_samples(bytes: &[u8], count: u16) -> Result<TankSamples<'_>, GroupFault> {
    Samples::parse(bytes, count)
}
