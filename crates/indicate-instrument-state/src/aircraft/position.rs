//! The declared position vocabularies of the nav group: the sensor that
//! positions a flight-management lane, and the integrity of that
//! position.
//!
//! A flight-management lane steers on whatever position solution the
//! vehicle has, and a pilot flies a camera fix differently from a
//! satellite fix. The guidance source states both values and a display
//! never infers either. Each decodes fail-closed, and each is
//! append-only, like the vocabularies in the sibling `navigation`
//! module.

use super::{NavData, NavSource};

/// The sensor that positions a flight-management lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PositionSource {
    /// No separate position sensor: the source is a receiver, which is
    /// its own sensor, or no source is selected.
    None,
    /// Satellite navigation.
    Gps,
    /// A camera that fixes position against terrain.
    Visual,
    /// Inertial navigation without external aiding.
    Inertial,
    /// Dead reckoning from heading, airspeed and wind.
    DeadReckoning,
    /// Several aiding sensors blended into one solution.
    Blended,
    /// The wire carried a sensor this build does not know, or none was
    /// declared. The nav group fails.
    #[default]
    Unknown,
}

impl PositionSource {
    /// Fail-closed wire decoding.
    #[must_use]
    pub const fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::None,
            1 => Self::Gps,
            2 => Self::Visual,
            3 => Self::Inertial,
            4 => Self::DeadReckoning,
            5 => Self::Blended,
            _ => Self::Unknown,
        }
    }

    /// Wire encoding; `Unknown` round-trips as unknown.
    #[must_use]
    pub const fn to_u8(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Gps => 1,
            Self::Visual => 2,
            Self::Inertial => 3,
            Self::DeadReckoning => 4,
            Self::Blended => 5,
            Self::Unknown => 255,
        }
    }

    /// The word that names the sensor to the pilot; empty when there is
    /// no sensor to name.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Gps => "GPS",
            Self::Visual => "VIS",
            Self::Inertial => "INS",
            Self::DeadReckoning => "DR",
            Self::Blended => "BLND",
            Self::None | Self::Unknown => "",
        }
    }

    /// Whether this sensor can belong to `source`. A flight-management
    /// lane always steers on a position, so it names one. A receiver
    /// and no source have no separate sensor. Any other pair is a
    /// source that contradicts itself.
    #[must_use]
    pub const fn fits(self, source: NavSource) -> bool {
        match source {
            NavSource::Fms => !matches!(self, Self::None | Self::Unknown),
            NavSource::None | NavSource::Gps | NavSource::Nav1 | NavSource::Nav2 => {
                matches!(self, Self::None)
            }
            NavSource::Unknown => false,
        }
    }
}

/// What the guidance source reports about the integrity of its
/// position.
///
/// This is the source's own statement about the position it steers on.
/// It is not [`crate::IntegrityLevel`], which is the evidence a display
/// uses to choose between two sources of one function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PositionIntegrity {
    /// The source reports nothing wrong with its position.
    Normal,
    /// The position is usable with reduced accuracy or reduced
    /// assurance.
    Degraded,
    /// A monitor removed a sensor from the solution, for example a
    /// satellite receiver that disagrees with the other sensors.
    SensorExcluded,
    /// The wire carried a state this build does not know, or none was
    /// declared. The nav group fails.
    #[default]
    Unknown,
}

impl PositionIntegrity {
    /// Fail-closed wire decoding.
    #[must_use]
    pub const fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::Normal,
            1 => Self::Degraded,
            2 => Self::SensorExcluded,
            _ => Self::Unknown,
        }
    }

    /// Wire encoding; `Unknown` round-trips as unknown.
    #[must_use]
    pub const fn to_u8(self) -> u8 {
        match self {
            Self::Normal => 0,
            Self::Degraded => 1,
            Self::SensorExcluded => 2,
            Self::Unknown => 255,
        }
    }

    /// The caution word for this state; empty while normal.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Degraded => "DGRD",
            Self::SensorExcluded => "EXCL",
            Self::Normal | Self::Unknown => "",
        }
    }

    /// Whether this state can accompany `position`. Only a named
    /// position sensor has an integrity to report, so a source without
    /// one reports `Normal`.
    #[must_use]
    pub const fn fits(self, position: PositionSource) -> bool {
        match self {
            Self::Normal => true,
            Self::Degraded | Self::SensorExcluded => {
                !matches!(position, PositionSource::None | PositionSource::Unknown)
            }
            Self::Unknown => false,
        }
    }
}

impl NavData {
    /// Whether the position word is a caution. Dead reckoning drifts
    /// without bound, and a degraded or excluded sensor reduces what
    /// the position is worth, so every display draws the word in its
    /// caution color.
    #[must_use]
    pub const fn position_is_caution(&self) -> bool {
        matches!(self.position_source, PositionSource::DeadReckoning)
            || !matches!(self.position_integrity, PositionIntegrity::Normal)
    }
}
