//! What a dial reads, and which of its channels a moving part follows.

use indicate_instrument_state::{
    AltitudeClass, BesideSamples, EngineField, GroupId, PanelData, Sig, TankField,
};

use super::unit::{DialUnit, Dimension};

/// A state lane a dial can read. Each reads the indicated value: lag
/// and instrument error are the source's physics. Engines and tanks are
/// addressed by zero-based index in the source's order; the cockpit's
/// instrument model says which index is engine 1 or FRONT L.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialInput {
    /// Indicated airspeed (Air group), knots.
    IndicatedAirspeed,
    /// The rate-of-climb instrument's reading (Air group), feet per minute.
    IndicatedVerticalSpeed,
    /// Barometric indicated altitude (Altitude group), feet. Any other
    /// declared altitude class shows no value: an altimeter is barometric.
    BaroAltitude,
    /// The applied altimeter setting (Air group), hectopascals.
    BaroSetting,
    /// Manifold pressure of one engine (Engine group), hectopascals.
    ManifoldPressure {
        /// Engine index.
        engine: u16,
    },
    /// Speed of one engine (Engine group), revolutions per minute.
    EngineRpm {
        /// Engine index.
        engine: u16,
    },
    /// Oil temperature of one engine (Engine group), degrees Celsius.
    OilTemperature {
        /// Engine index.
        engine: u16,
    },
    /// Oil pressure of one engine (Engine group), hectopascals.
    OilPressure {
        /// Engine index.
        engine: u16,
    },
    /// Fuel pressure of one engine (Engine group), hectopascals.
    FuelPressure {
        /// Engine index.
        engine: u16,
    },
    /// Coolant temperature of one engine (Engine group), degrees Celsius.
    CoolantTemperature {
        /// Engine index.
        engine: u16,
    },
    /// Cylinder-head temperature of one engine (Engine group), degrees
    /// Celsius.
    CylinderHeadTemperature {
        /// Engine index.
        engine: u16,
    },
    /// Carburettor air temperature of one engine (Engine group), degrees
    /// Celsius.
    CarburettorAirTemperature {
        /// Engine index.
        engine: u16,
    },
    /// Bank angle (Attitude group), degrees, positive right.
    Roll,
    /// Pitch angle (Attitude group), degrees, positive nose up.
    Pitch,
    /// Heading in the source's declared reference (Heading group),
    /// degrees: what a directional gyro shows.
    GyroHeading,
    /// Heading converted to magnetic north (Heading group), degrees:
    /// what a magnetic compass card shows.
    MagneticHeading,
    /// Body yaw rate (Attitude group, under its declared body-rate
    /// validity), degrees per second, positive nose right: what a turn
    /// needle's rate gyro senses.
    YawRate,
    /// Lateral specific force (Dynamics group), metres per second
    /// squared, positive right: what moves a turn indicator's ball.
    LateralForce,
    /// The time of day the cockpit clock shows (Clock group), seconds
    /// after midnight.
    ClockTime,
    /// How far one engine's shaft leads another's (Engine group),
    /// degrees in `[0, 360)`: what a synchroscope shows. It turns
    /// steadily while the engines run at different speeds.
    ShaftPhase {
        /// The engine whose lead is shown.
        engine: u16,
        /// The engine it is compared with.
        reference: u16,
    },
    /// Fuel in one tank (FuelTanks group): litres on a dial engraved in
    /// volume, kilograms on one engraved in mass. A tank that does not
    /// report the dial's quantity shows no value; the display never
    /// assumes a density.
    FuelQuantity {
        /// Tank index.
        tank: u16,
    },
}

impl DialInput {
    /// Whether a dial engraved in `unit` can show this input.
    fn admits(self, unit: Dimension) -> bool {
        let native = match self {
            Self::IndicatedAirspeed => Dimension::Speed,
            Self::IndicatedVerticalSpeed => Dimension::VerticalSpeed,
            Self::BaroAltitude => Dimension::Height,
            Self::BaroSetting
            | Self::ManifoldPressure { .. }
            | Self::OilPressure { .. }
            | Self::FuelPressure { .. } => Dimension::Pressure,
            Self::EngineRpm { .. } => Dimension::Rotation,
            Self::OilTemperature { .. }
            | Self::CoolantTemperature { .. }
            | Self::CylinderHeadTemperature { .. }
            | Self::CarburettorAirTemperature { .. } => Dimension::Temperature,
            Self::FuelQuantity { .. } => {
                return matches!(unit, Dimension::Volume | Dimension::Mass);
            }
            Self::Roll
            | Self::Pitch
            | Self::GyroHeading
            | Self::MagneticHeading
            | Self::ShaftPhase { .. } => Dimension::Angle,
            Self::YawRate => Dimension::AngularRate,
            Self::LateralForce => Dimension::Acceleration,
            Self::ClockTime => Dimension::Time,
        };
        native == unit
    }

    /// The state group the value comes from, which a numeral derived
    /// from it must claim.
    #[must_use]
    pub const fn group(self) -> GroupId {
        match self {
            Self::IndicatedAirspeed | Self::IndicatedVerticalSpeed | Self::BaroSetting => {
                GroupId::Air
            }
            Self::BaroAltitude => GroupId::Altitude,
            Self::FuelQuantity { .. } => GroupId::FuelTanks,
            Self::Roll | Self::Pitch | Self::YawRate => GroupId::Attitude,
            Self::GyroHeading | Self::MagneticHeading => GroupId::Heading,
            Self::LateralForce => GroupId::Dynamics,
            Self::ClockTime => GroupId::Clock,
            Self::ManifoldPressure { .. }
            | Self::EngineRpm { .. }
            | Self::OilTemperature { .. }
            | Self::OilPressure { .. }
            | Self::FuelPressure { .. }
            | Self::CoolantTemperature { .. }
            | Self::CylinderHeadTemperature { .. }
            | Self::CarburettorAirTemperature { .. }
            | Self::ShaftPhase { .. } => GroupId::Engine,
        }
    }

    /// The value in its native unit for a dial engraved in `unit` (which
    /// picks volume or mass for fuel), or `None` when the state shows no
    /// value for it.
    #[must_use]
    pub fn native(
        self,
        data: &PanelData,
        beside: &BesideSamples<'_>,
        unit: DialUnit,
    ) -> Option<f32> {
        let shown = |sig: Sig<f32>| sig.status.shows_value().then_some(sig.value);
        let engine = |engine: u16, field: EngineField| engine_value(data, beside, engine, field);
        match self {
            Self::IndicatedAirspeed => shown(data.ias_kt),
            Self::IndicatedVerticalSpeed => shown(data.vsi_indicated_fpm),
            Self::BaroAltitude => (data.altitude.class == AltitudeClass::BaroIndicated)
                .then_some(data.altitude.value_ft)
                .and_then(shown),
            Self::BaroSetting => shown(data.baro_hpa),
            Self::ManifoldPressure { engine: e } => engine(e, EngineField::ManifoldPressureHpa),
            Self::EngineRpm { engine: e } => engine(e, EngineField::Rpm),
            Self::OilTemperature { engine: e } => engine(e, EngineField::OilTemperatureC),
            // Engine pressures arrive in kilopascals; the pressure
            // dimension's native unit is the hectopascal.
            Self::OilPressure { engine: e } => {
                engine(e, EngineField::OilPressureKpa).map(|v| v * 10.0)
            }
            Self::FuelPressure { engine: e } => {
                engine(e, EngineField::FuelPressureKpa).map(|v| v * 10.0)
            }
            Self::CoolantTemperature { engine: e } => engine(e, EngineField::CoolantTemperatureC),
            Self::CylinderHeadTemperature { engine: e } => {
                engine(e, EngineField::CylinderHeadTemperatureC)
            }
            Self::CarburettorAirTemperature { engine: e } => {
                engine(e, EngineField::CarburettorAirTemperatureC)
            }
            Self::FuelQuantity { tank } => tank_value(data, beside, tank, unit),
            Self::Roll => shown(data.roll_rad).map(f32::to_degrees),
            Self::Pitch => shown(data.pitch_rad).map(f32::to_degrees),
            Self::GyroHeading => shown(data.heading.value_rad).map(f32::to_degrees),
            Self::MagneticHeading => {
                shown(data.heading.magnetic_heading_rad()).map(f32::to_degrees)
            }
            Self::YawRate => shown(data.yaw_rate_rps).map(f32::to_degrees),
            Self::LateralForce => shown(data.slip_lat_mps2),
            Self::ClockTime => shown(data.clock_s),
            Self::ShaftPhase {
                engine: e,
                reference,
            } => {
                let lead = engine(e, EngineField::ShaftAngleDeg)?;
                let behind = engine(reference, EngineField::ShaftAngleDeg)?;
                let phase = lead - behind;
                Some(if phase < 0.0 { phase + 360.0 } else { phase })
            }
        }
    }
}

fn engine_value(
    data: &PanelData,
    beside: &BesideSamples<'_>,
    engine: u16,
    field: EngineField,
) -> Option<f32> {
    if !data.engine.status.shows_value() {
        return None;
    }
    beside
        .engines
        .get(usize::from(engine))
        .and_then(|sample| sample.field(field))
        .filter(|v| v.is_finite())
}

fn tank_value(
    data: &PanelData,
    beside: &BesideSamples<'_>,
    tank: u16,
    unit: DialUnit,
) -> Option<f32> {
    if !data.fuel_tanks.status.shows_value() {
        return None;
    }
    let field = match unit.dimension() {
        Dimension::Mass => TankField::QuantityKg,
        _ => TankField::QuantityL,
    };
    beside
        .tanks
        .get(usize::from(tank))
        .and_then(|sample| sample.field(field))
        .filter(|v| v.is_finite())
}

/// A dial input and the unit its scale is engraved in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DialChannel {
    /// The state lane.
    pub input: DialInput,
    /// The engraved unit.
    pub unit: DialUnit,
}

impl DialChannel {
    /// Whether the engraved unit measures what the input reports.
    #[must_use]
    pub fn units_agree(&self) -> bool {
        self.input.admits(self.unit.dimension())
    }

    /// The value in dial units, or `None` when the state shows none.
    #[must_use]
    pub fn read(&self, data: &PanelData, beside: &BesideSamples<'_>) -> Option<f32> {
        self.input
            .native(data, beside, self.unit)
            .map(|v| self.unit.convert(v))
    }
}

/// Which of a dial's channels a moving part follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    /// [`crate::AnalogDialSpec::primary`].
    Primary,
    /// [`crate::AnalogDialSpec::secondary`].
    Secondary,
    /// [`crate::AnalogDialSpec::extra`]`[n]`: one more independently
    /// sourced quantity, such as each pointer of a three-pointer engine
    /// gauge unit.
    Extra(u8),
}
