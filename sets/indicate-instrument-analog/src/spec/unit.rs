//! The units a dial is engraved in, and the conversion from each
//! quantity's native unit.
//!
//! Every input reads its quantity in one native unit: knots, feet, feet
//! per minute, hectopascals, revolutions per minute, degrees Celsius,
//! litres, or kilograms. A dial converts from it by an affine law
//! (`dial = native × scale + offset`), which is how a temperature dial in
//! Fahrenheit reads a Celsius lane.

/// The physical dimension of a quantity, for unit compatibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Dimension {
    Speed,
    VerticalSpeed,
    Height,
    Pressure,
    Rotation,
    Temperature,
    Volume,
    Mass,
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
    /// Degrees Celsius.
    DegreesCelsius,
    /// Degrees Fahrenheit.
    DegreesFahrenheit,
    /// Litres.
    Litres,
    /// US liquid gallons.
    UsGallons,
    /// Imperial gallons.
    ImperialGallons,
    /// Kilograms.
    Kilograms,
    /// Avoirdupois pounds.
    Pounds,
}

const KT_TO_MPH: f32 = 1.150_779_4;
const KT_TO_KMH: f32 = 1.852;
const FT_TO_M: f32 = 0.3048;
const FPM_TO_MPS: f32 = 0.005_08;
const HPA_PER_INHG: f32 = 33.863_89;
const HPA_TO_PSI: f32 = 0.014_503_774;
const L_PER_US_GAL: f32 = 3.785_411_8;
const L_PER_IMP_GAL: f32 = 4.546_09;
const KG_TO_LB: f32 = 2.204_622_6;

impl DialUnit {
    pub(crate) fn dimension(self) -> Dimension {
        match self {
            Self::Knots | Self::MilesPerHour | Self::KilometresPerHour => Dimension::Speed,
            Self::Feet | Self::Metres => Dimension::Height,
            Self::FeetPerMinute | Self::MetresPerSecond => Dimension::VerticalSpeed,
            Self::InchesOfMercury
            | Self::Hectopascals
            | Self::Kilopascals
            | Self::PoundsPerSquareInch => Dimension::Pressure,
            Self::RevolutionsPerMinute => Dimension::Rotation,
            Self::DegreesCelsius | Self::DegreesFahrenheit => Dimension::Temperature,
            Self::Litres | Self::UsGallons | Self::ImperialGallons => Dimension::Volume,
            Self::Kilograms | Self::Pounds => Dimension::Mass,
        }
    }

    /// The affine law from the dimension's native unit to this unit:
    /// `(scale, offset)`.
    pub(crate) fn native_law(self) -> (f32, f32) {
        let scale = match self {
            Self::Knots
            | Self::Feet
            | Self::FeetPerMinute
            | Self::Hectopascals
            | Self::RevolutionsPerMinute
            | Self::DegreesCelsius
            | Self::Litres
            | Self::Kilograms => 1.0,
            Self::MilesPerHour => KT_TO_MPH,
            Self::KilometresPerHour => KT_TO_KMH,
            Self::Metres => FT_TO_M,
            Self::MetresPerSecond => FPM_TO_MPS,
            Self::InchesOfMercury => 1.0 / HPA_PER_INHG,
            Self::Kilopascals => 0.1,
            Self::PoundsPerSquareInch => HPA_TO_PSI,
            Self::DegreesFahrenheit => 1.8,
            Self::UsGallons => 1.0 / L_PER_US_GAL,
            Self::ImperialGallons => 1.0 / L_PER_IMP_GAL,
            Self::Pounds => KG_TO_LB,
        };
        let offset = if self == Self::DegreesFahrenheit {
            32.0
        } else {
            0.0
        };
        (scale, offset)
    }

    /// `native` in this unit.
    #[must_use]
    pub fn convert(self, native: f32) -> f32 {
        let (scale, offset) = self.native_law();
        native * scale + offset
    }
}
