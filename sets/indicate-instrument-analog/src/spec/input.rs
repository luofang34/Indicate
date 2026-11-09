//! What a dial reads and in which unit it is engraved.

use indicate_instrument_state::{AltitudeClass, EngineField, EngineSamples, GroupId, PanelData};

/// A state lane a dial can read. Each reads the indicated value: lag
/// and instrument error are the source's physics.
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
        /// Zero-based engine index, in the source's panel order.
        engine: u16,
    },
    /// Speed of one engine (Engine group), revolutions per minute.
    EngineRpm {
        /// Zero-based engine index, in the source's panel order.
        engine: u16,
    },
}

/// The physical dimension of a quantity, for unit compatibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dimension {
    Speed,
    VerticalSpeed,
    Height,
    Pressure,
    Rotation,
}

impl DialInput {
    fn dimension(self) -> Dimension {
        match self {
            Self::IndicatedAirspeed => Dimension::Speed,
            Self::IndicatedVerticalSpeed => Dimension::VerticalSpeed,
            Self::BaroAltitude => Dimension::Height,
            Self::BaroSetting | Self::ManifoldPressure { .. } => Dimension::Pressure,
            Self::EngineRpm { .. } => Dimension::Rotation,
        }
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
            Self::ManifoldPressure { .. } | Self::EngineRpm { .. } => GroupId::Engine,
        }
    }

    /// The value in this input's native unit, or `None` when the state
    /// shows no value for it. Engine inputs read `engines`, the samples
    /// beside the state; an engine the source does not report, or one
    /// that lacks the field, shows no value.
    #[must_use]
    pub fn native(self, data: &PanelData, engines: &EngineSamples<'_>) -> Option<f32> {
        let shown = |sig: indicate_instrument_state::Sig<f32>| {
            sig.status.shows_value().then_some(sig.value)
        };
        match self {
            Self::IndicatedAirspeed => shown(data.ias_kt),
            Self::IndicatedVerticalSpeed => shown(data.vsi_indicated_fpm),
            Self::BaroAltitude => (data.altitude.class == AltitudeClass::BaroIndicated)
                .then_some(data.altitude.value_ft)
                .and_then(shown),
            Self::BaroSetting => shown(data.baro_hpa),
            Self::ManifoldPressure { engine } => {
                engine_value(data, engines, engine, EngineField::ManifoldPressureHpa)
            }
            Self::EngineRpm { engine } => engine_value(data, engines, engine, EngineField::Rpm),
        }
    }
}

fn engine_value(
    data: &PanelData,
    engines: &EngineSamples<'_>,
    engine: u16,
    field: EngineField,
) -> Option<f32> {
    if !data.engine.status.shows_value() {
        return None;
    }
    engines
        .get(usize::from(engine))
        .and_then(|sample| sample.field(field))
        .filter(|v| v.is_finite())
}

/// The unit a dial is engraved in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialUnit {
    /// International knots.
    Knots,
    /// Statute miles per hour.
    MilesPerHour,
    /// Kilometres per hour.
    KilometresPerHour,
    /// International feet.
    Feet,
    /// Metres.
    Metres,
    /// Feet per minute.
    FeetPerMinute,
    /// Metres per second.
    MetresPerSecond,
    /// Inches of mercury at 0 °C.
    InchesOfMercury,
    /// Hectopascals (millibars).
    Hectopascals,
    /// Kilopascals.
    Kilopascals,
    /// Pounds-force per square inch.
    PoundsPerSquareInch,
    /// Revolutions per minute.
    RevolutionsPerMinute,
}

const KT_TO_MPH: f32 = 1.150_779_4;
const KT_TO_KMH: f32 = 1.852;
const FT_TO_M: f32 = 0.3048;
const FPM_TO_MPS: f32 = 0.005_08;
const HPA_PER_INHG: f32 = 33.863_89;
const HPA_TO_PSI: f32 = 0.014_503_774;

impl DialUnit {
    fn dimension(self) -> Dimension {
        match self {
            Self::Knots | Self::MilesPerHour | Self::KilometresPerHour => Dimension::Speed,
            Self::Feet | Self::Metres => Dimension::Height,
            Self::FeetPerMinute | Self::MetresPerSecond => Dimension::VerticalSpeed,
            Self::InchesOfMercury
            | Self::Hectopascals
            | Self::Kilopascals
            | Self::PoundsPerSquareInch => Dimension::Pressure,
            Self::RevolutionsPerMinute => Dimension::Rotation,
        }
    }

    /// Dial units per native unit of the input's dimension: knots,
    /// feet, feet per minute, hectopascals, or revolutions per minute.
    fn per_native(self) -> f32 {
        match self {
            Self::Knots
            | Self::Feet
            | Self::FeetPerMinute
            | Self::Hectopascals
            | Self::RevolutionsPerMinute => 1.0,
            Self::MilesPerHour => KT_TO_MPH,
            Self::KilometresPerHour => KT_TO_KMH,
            Self::Metres => FT_TO_M,
            Self::MetresPerSecond => FPM_TO_MPS,
            Self::InchesOfMercury => 1.0 / HPA_PER_INHG,
            Self::Kilopascals => 0.1,
            Self::PoundsPerSquareInch => HPA_TO_PSI,
        }
    }
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
        self.input.dimension() == self.unit.dimension()
    }

    /// The value in dial units, or `None` when the state shows none.
    #[must_use]
    pub fn read(&self, data: &PanelData, engines: &EngineSamples<'_>) -> Option<f32> {
        self.input
            .native(data, engines)
            .map(|v| v * self.unit.per_native())
    }
}

/// Which of a dial's two channels a moving part follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    /// [`crate::AnalogDialSpec::primary`].
    Primary,
    /// [`crate::AnalogDialSpec::secondary`].
    Secondary,
}
