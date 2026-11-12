//! The declared vocabularies of the nav group: the source coding, the
//! deflection scale, the facility a receiver is tuned to, and the leg
//! sequencing state.
//!
//! Each is declared by the guidance source and never inferred by a
//! display. Each decodes fail-closed: a wire value this build does not
//! know is `Unknown`, and `Unknown` fails the nav group. Values are
//! append-only, so a later kind is a new value, never a new meaning
//! for an old one.

use super::NavSource;

impl NavSource {
    /// Fail-closed wire decoding. The nav group and each bearing
    /// pointer share this coding.
    #[must_use]
    pub const fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::None,
            1 => Self::Gps,
            2 => Self::Nav1,
            3 => Self::Nav2,
            4 => Self::Fms,
            _ => Self::Unknown,
        }
    }

    /// Wire encoding; `Unknown` round-trips as unknown.
    #[must_use]
    pub const fn to_u8(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Gps => 1,
            Self::Nav1 => 2,
            Self::Nav2 => 3,
            Self::Fms => 4,
            Self::Unknown => 255,
        }
    }

    /// The word that names the source to the pilot; empty when there is
    /// no source to name.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Gps => "GPS",
            Self::Nav1 => "NAV1",
            Self::Nav2 => "NAV2",
            Self::Fms => "FMS",
            Self::None | Self::Unknown => "",
        }
    }

    /// Whether the source computes its own course, as opposed to a
    /// receiver tuned to a ground facility. The displays draw a
    /// computed course in magenta and its vertical deviation as a
    /// computed path.
    #[must_use]
    pub const fn is_computed(self) -> bool {
        matches!(self, Self::Gps | Self::Fms)
    }
}

/// The deflection scale the guidance source is flying to.
///
/// Two dots is two dots on the glass whatever the phase, so the same
/// needle position means a different distance in each. A source that
/// changes scale without saying so changes the meaning of the picture
/// and nothing on the panel could tell. The scale is therefore
/// declared, never inferred from a distance the display was handed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NavScale {
    /// Enroute.
    Enroute,
    /// Terminal.
    Terminal,
    /// Approach.
    Approach,
    /// The wire carried a scale this build does not know, or none was
    /// declared. Guidance whose scale is unknown means nothing, so the
    /// nav group fails rather than drawing at a guessed scale.
    #[default]
    Unknown,
}

impl NavScale {
    /// Fail-closed wire decoding.
    #[must_use]
    pub const fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::Enroute,
            1 => Self::Terminal,
            2 => Self::Approach,
            _ => Self::Unknown,
        }
    }

    /// Wire encoding; `Unknown` round-trips as unknown.
    #[must_use]
    pub const fn to_u8(self) -> u8 {
        match self {
            Self::Enroute => 0,
            Self::Terminal => 1,
            Self::Approach => 2,
            Self::Unknown => 255,
        }
    }

    /// The label that names this scale to the pilot. Every character is
    /// in the panel glyph vocabulary, which has no `+` and no `/`, so
    /// the richer per-approach names cannot be spelled here yet.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Enroute => "ENR",
            Self::Terminal => "TERM",
            Self::Approach => "APR",
            Self::Unknown => "",
        }
    }
}

/// What a navigation receiver is tuned to.
///
/// A GPS course has no ground facility. A NAV receiver is tuned to a
/// VOR or a localizer, and the two read differently: a localizer has
/// no TO/FROM and pairs with a glideslope. A future facility kind,
/// such as a GLS approach, is a new appended value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NavFacility {
    /// No ground facility: a GPS or flight-management course, or no
    /// source selected.
    None,
    /// A VOR.
    Vor,
    /// A localizer.
    Localizer,
    /// The wire carried a facility this build does not know, or none
    /// was declared. The nav group fails.
    #[default]
    Unknown,
}

impl NavFacility {
    /// Fail-closed wire decoding.
    #[must_use]
    pub const fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::None,
            1 => Self::Vor,
            2 => Self::Localizer,
            _ => Self::Unknown,
        }
    }

    /// Wire encoding; `Unknown` round-trips as unknown.
    #[must_use]
    pub const fn to_u8(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Vor => 1,
            Self::Localizer => 2,
            Self::Unknown => 255,
        }
    }

    /// The label that names the facility to the pilot; empty when there
    /// is no facility to name.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Vor => "VOR",
            Self::Localizer => "LOC",
            Self::None | Self::Unknown => "",
        }
    }

    /// Whether this facility can belong to `source`. A GPS course, a
    /// flight-management course and no source have no facility. A NAV
    /// receiver is tuned to a VOR or a localizer. Any other pair is a
    /// source that contradicts itself.
    #[must_use]
    pub const fn fits(self, source: NavSource) -> bool {
        match source {
            NavSource::None | NavSource::Gps | NavSource::Fms => matches!(self, Self::None),
            NavSource::Nav1 | NavSource::Nav2 => matches!(self, Self::Vor | Self::Localizer),
            NavSource::Unknown => false,
        }
    }
}

/// Whether the guidance source is sequencing through its legs.
///
/// A holding pattern flips the TO/FROM flag on every pass over the fix.
/// The state says why, so the flip does not read as a fault.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NavSequencing {
    /// The source sequences from leg to leg.
    Sequencing,
    /// The source is flying a holding pattern.
    Hold,
    /// The source has suspended automatic leg sequencing.
    Suspended,
    /// The wire carried a state this build does not know, or none was
    /// declared. The nav group fails.
    #[default]
    Unknown,
}

impl NavSequencing {
    /// Fail-closed wire decoding.
    #[must_use]
    pub const fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::Sequencing,
            1 => Self::Hold,
            2 => Self::Suspended,
            _ => Self::Unknown,
        }
    }

    /// Wire encoding; `Unknown` round-trips as unknown.
    #[must_use]
    pub const fn to_u8(self) -> u8 {
        match self {
            Self::Sequencing => 0,
            Self::Hold => 1,
            Self::Suspended => 2,
            Self::Unknown => 255,
        }
    }

    /// The annunciation for this state; empty while sequencing.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Hold => "HOLD",
            Self::Suspended => "SUSP",
            Self::Sequencing | Self::Unknown => "",
        }
    }
}
