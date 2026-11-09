//! Engine indications: what each engine's gauges read, for any number
//! of engines of any kind.
//!
//! The values are the indicated ones. Mechanism lag and instrument
//! error are the source's physics, so a simulator that models a gauge
//! writes the reading its mechanism shows.
//!
//! The group is presence-masked ([`crate::samples`]): the owned
//! [`crate::AircraftState`] carries only its header, and the samples
//! travel beside the state as an [`EngineSamples`] view. Engines differ —
//! a piston engine has manifold pressure, oil and cylinder-head
//! temperatures; a turbine N1, N2 and gas temperature; a rocket chamber
//! pressure — so a sample carries only the fields its engine has, named
//! by bits of the [`EngineField`] registry. A dial bound to a field an
//! engine lacks shows its failure pose, never zero.

use crate::samples::{RawSample, Sample, SampleGroup, SampleIter, Samples, raw_wire_bytes};
use crate::validate::GroupFault;

/// One field of the engine indication registry. Each field owns the
/// presence-mask bit equal to its discriminant and is an f32 on the
/// wire. The registry is append-only: a bit never changes meaning and
/// is never reused. A new field is a new state ABI version, because a
/// decoder cannot size a sample that carries a bit it does not know.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineField {
    /// Manifold absolute pressure, hectopascals.
    ManifoldPressureHpa = 0,
    /// Shaft speed, revolutions per minute.
    Rpm = 1,
    /// Oil temperature, degrees Celsius.
    OilTemperatureC = 2,
    /// Oil pressure, kilopascals gauge.
    OilPressureKpa = 3,
    /// Fuel pressure, kilopascals gauge.
    FuelPressureKpa = 4,
    /// Coolant temperature, degrees Celsius.
    CoolantTemperatureC = 5,
    /// Cylinder-head temperature, degrees Celsius.
    CylinderHeadTemperatureC = 6,
    /// Carburettor air temperature, degrees Celsius.
    CarburettorAirTemperatureC = 7,
}

impl EngineField {
    /// Every registered field, in bit order: the order a sample's
    /// present fields follow on the wire.
    pub const ALL: [EngineField; 8] = [
        EngineField::ManifoldPressureHpa,
        EngineField::Rpm,
        EngineField::OilTemperatureC,
        EngineField::OilPressureKpa,
        EngineField::FuelPressureKpa,
        EngineField::CoolantTemperatureC,
        EngineField::CylinderHeadTemperatureC,
        EngineField::CarburettorAirTemperatureC,
    ];

    /// This field's bit in a presence mask.
    #[must_use]
    pub const fn bit(self) -> u16 {
        1 << (self as u8)
    }

    /// Bytes this field occupies on the wire.
    #[must_use]
    pub const fn wire_bytes(self) -> usize {
        4
    }

    /// Whether `v` is a value this field can hold: a temperature at or
    /// above absolute zero, any other reading not negative.
    #[must_use]
    pub fn admits(self, v: f32) -> bool {
        let floor = match self {
            Self::OilTemperatureC
            | Self::CoolantTemperatureC
            | Self::CylinderHeadTemperatureC
            | Self::CarburettorAirTemperatureC => -273.15,
            Self::ManifoldPressureHpa
            | Self::Rpm
            | Self::OilPressureKpa
            | Self::FuelPressureKpa => 0.0,
        };
        v.is_finite() && v >= floor
    }
}

/// Every bit the registry assigns; any other bit faults the group.
pub const ENGINE_FIELDS_MASK: u16 = 0x00FF;

/// Wire bytes of an engine sample whose presence mask is `mask`; `None`
/// when the mask names a field outside the registry.
#[must_use]
pub const fn sample_wire_bytes(mask: u16) -> Option<usize> {
    raw_wire_bytes(mask, ENGINE_FIELDS_MASK)
}

/// The engine group's header, carried in the owned state.
pub type EngineGroup = SampleGroup;

/// The engine samples beside a state.
pub type EngineSamples<'a> = Samples<'a, EngineSample>;

/// Iterator over an [`EngineSamples`] view.
pub type EngineIter<'a> = SampleIter<'a, EngineSample>;

/// One engine's indicated readings. A field is `Some` exactly when the
/// engine has that indication; its presence bit is set on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct EngineSample {
    /// Manifold absolute pressure, hectopascals.
    pub manifold_pressure_hpa: Option<f32>,
    /// Shaft speed, revolutions per minute.
    pub rpm: Option<f32>,
    /// Oil temperature, degrees Celsius.
    pub oil_temperature_c: Option<f32>,
    /// Oil pressure, kilopascals gauge.
    pub oil_pressure_kpa: Option<f32>,
    /// Fuel pressure, kilopascals gauge.
    pub fuel_pressure_kpa: Option<f32>,
    /// Coolant temperature, degrees Celsius.
    pub coolant_temperature_c: Option<f32>,
    /// Cylinder-head temperature, degrees Celsius.
    pub cylinder_head_temperature_c: Option<f32>,
    /// Carburettor air temperature, degrees Celsius.
    pub carburettor_air_temperature_c: Option<f32>,
}

impl EngineSample {
    /// An engine with no indication, for `..EngineSample::EMPTY` in
    /// `const` literals.
    pub const EMPTY: EngineSample = EngineSample {
        manifold_pressure_hpa: None,
        rpm: None,
        oil_temperature_c: None,
        oil_pressure_kpa: None,
        fuel_pressure_kpa: None,
        coolant_temperature_c: None,
        cylinder_head_temperature_c: None,
        carburettor_air_temperature_c: None,
    };

    /// The reading of `field`, if this engine has it.
    #[must_use]
    pub fn field(&self, field: EngineField) -> Option<f32> {
        match field {
            EngineField::ManifoldPressureHpa => self.manifold_pressure_hpa,
            EngineField::Rpm => self.rpm,
            EngineField::OilTemperatureC => self.oil_temperature_c,
            EngineField::OilPressureKpa => self.oil_pressure_kpa,
            EngineField::FuelPressureKpa => self.fuel_pressure_kpa,
            EngineField::CoolantTemperatureC => self.coolant_temperature_c,
            EngineField::CylinderHeadTemperatureC => self.cylinder_head_temperature_c,
            EngineField::CarburettorAirTemperatureC => self.carburettor_air_temperature_c,
        }
    }

    fn slot(&mut self, field: EngineField) -> &mut Option<f32> {
        match field {
            EngineField::ManifoldPressureHpa => &mut self.manifold_pressure_hpa,
            EngineField::Rpm => &mut self.rpm,
            EngineField::OilTemperatureC => &mut self.oil_temperature_c,
            EngineField::OilPressureKpa => &mut self.oil_pressure_kpa,
            EngineField::FuelPressureKpa => &mut self.fuel_pressure_kpa,
            EngineField::CoolantTemperatureC => &mut self.coolant_temperature_c,
            EngineField::CylinderHeadTemperatureC => &mut self.cylinder_head_temperature_c,
            EngineField::CarburettorAirTemperatureC => &mut self.carburettor_air_temperature_c,
        }
    }

    /// The sample's presence mask.
    #[must_use]
    pub fn presence(&self) -> u16 {
        self.to_raw().mask
    }

    /// Whether every present reading is a value its field can hold.
    #[must_use]
    pub fn is_sound(&self) -> bool {
        EngineField::ALL
            .iter()
            .all(|f| self.field(*f).is_none_or(|v| f.admits(v)))
    }

    /// Wire bytes this sample encodes to.
    #[must_use]
    pub fn wire_bytes(&self) -> usize {
        Sample::wire_bytes(self)
    }
}

impl Sample for EngineSample {
    const KNOWN: u16 = ENGINE_FIELDS_MASK;

    fn from_raw(raw: &RawSample) -> Self {
        let mut sample = EngineSample::default();
        for field in EngineField::ALL {
            *sample.slot(field) = raw.get(field as u8);
        }
        sample
    }

    fn to_raw(&self) -> RawSample {
        let mut raw = RawSample::EMPTY;
        for field in EngineField::ALL {
            raw.set(field as u8, self.field(field));
        }
        raw
    }

    fn is_sound(&self) -> bool {
        EngineSample::is_sound(self)
    }
}

/// Checks an engine samples region against `count`; see
/// [`Samples::parse`].
pub fn parse_samples(bytes: &[u8], count: u16) -> Result<EngineSamples<'_>, GroupFault> {
    Samples::parse(bytes, count)
}

/// The engine group's own fault: why the decoder refused its samples.
pub(crate) fn engine_fault(group: &EngineGroup) -> Option<GroupFault> {
    group.fault
}
