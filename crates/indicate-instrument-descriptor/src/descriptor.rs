//! The instrument descriptor: everything a shell needs to wire one instrument.

use indicate_instrument_scene::SceneError;
use indicate_instrument_state::{AircraftState, GroupId};
use indicate_intent::IntentClass;

use crate::aux::AuxChannelId;
use crate::config::{ConfigError, ConfigKey};
use crate::context::DrawContext;
use crate::group_set::GroupSet;
use crate::space::{RenderSpace, SceneTarget, UnderlayContract};

/// An instrument's pure draw entry point: `DrawContext` + `SceneTarget` → scene bytes.
pub type DrawFn = fn(&DrawContext<'_>, SceneTarget<'_, '_>) -> Result<(), InstrumentDrawError>;

/// Why an instrument draw failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum InstrumentDrawError {
    /// The scene writer refused a command.
    #[error("scene writer refused a command")]
    Scene(SceneError),
    /// The configuration blob does not decode for this instrument.
    #[error("config error: {0}")]
    Config(#[from] ConfigError),
    /// The caller passed a [`SceneTarget`] variant incompatible with `space`.
    #[error("scene target variant does not match the instrument's render space")]
    WrongTarget,
    /// An auxiliary channel payload failed fail-closed validation.
    #[error("malformed auxiliary channel payload ({0:#010x})")]
    InvalidAux(u32),
}

impl From<SceneError> for InstrumentDrawError {
    fn from(error: SceneError) -> Self {
        Self::Scene(error)
    }
}

/// Alias preserving ergonomic compatibility with planar panel consumers.
pub type PanelDrawError = InstrumentDrawError;

/// The 2D logical space a planar instrument draws against; backends scale.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DesignFrame {
    /// Logical width.
    pub width: f32,
    /// Logical height.
    pub height: f32,
}

/// An axis-aligned rectangle in design-frame units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Region {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
}

impl Region {
    /// The whole of `frame`, as a region at its origin.
    #[must_use]
    pub const fn of(frame: DesignFrame) -> Region {
        Region {
            x: 0.0,
            y: 0.0,
            width: frame.width,
            height: frame.height,
        }
    }

    /// Right edge.
    #[must_use]
    pub fn right(&self) -> f32 {
        self.x + self.width
    }

    /// Bottom edge.
    #[must_use]
    pub fn bottom(&self) -> f32 {
        self.y + self.height
    }

    /// Whether every coordinate is finite and both extents are positive.
    #[must_use]
    pub fn is_sound(&self) -> bool {
        self.x.is_finite()
            && self.y.is_finite()
            && self.width.is_finite()
            && self.height.is_finite()
            && self.width > 0.0
            && self.height > 0.0
    }

    /// Whether the two overlap on a positive area; sharing only an edge does not count.
    #[must_use]
    pub fn intersects(&self, other: &Region) -> bool {
        self.x < other.right()
            && other.x < self.right()
            && self.y < other.bottom()
            && other.y < self.bottom()
    }

    /// Whether `other` lies wholly inside this region.
    #[must_use]
    pub fn contains(&self, other: &Region) -> bool {
        other.x >= self.x
            && other.y >= self.y
            && other.right() <= self.right()
            && other.bottom() <= self.bottom()
    }

    /// The same region moved by `(dx, dy)`.
    #[must_use]
    pub fn translated(&self, dx: f32, dy: f32) -> Region {
        Region {
            x: self.x + dx,
            y: self.y + dy,
            ..*self
        }
    }
}

/// An instrument-contributed extreme state for conformance and digest runs.
#[derive(Debug, Clone, Copy)]
pub struct ExtremeState {
    /// Stable identity of the fixture (lowercase, digits, dashes).
    pub id: &'static str,
    /// Builds the state; a plain fn keeps descriptors `static`.
    pub build: fn() -> AircraftState,
}

/// One instrument, as static data (ADR-0029).
///
/// `required_layers` bit encoding by [`RenderSpace`]:
/// - [`RenderSpace::Planar2D`]: bits `0..6` = [`indicate_instrument_scene::LayerId`].
/// - [`RenderSpace::AngularSphere`]: bits `0..6` =
///   [`indicate_instrument_scene::angular::AngularLayerId`]; when
///   `overlay_frame` is `Some(_)`, bits `8..14` = 2D overlay
///   [`indicate_instrument_scene::LayerId`].
/// - [`RenderSpace::Volume3D`]: bits `0..9` =
///   [`indicate_instrument_scene::volume::VolumeBandId`].
#[derive(Debug, Clone, Copy)]
pub struct InstrumentDescriptor {
    /// Stable identity (lowercase, digits, dashes).
    pub id: &'static str,
    /// Operator-facing title.
    pub title: &'static str,
    /// Coordinate space and sizing law.
    pub space: RenderSpace,
    /// Required scene layers or bands (see struct docs for bit layout).
    pub required_layers: u16,
    /// State groups this instrument consumes.
    pub required_groups: GroupSet,
    /// Host underlay/background contract.
    pub underlay: UnderlayContract,
    /// Configuration keys this instrument understands.
    pub config_schema: &'static [ConfigKey],
    /// Operator intent classes this instrument accepts or surfaces.
    pub accepted_intents: &'static [IntentClass],
    /// Auxiliary domain channels this instrument reads from [`DrawContext`].
    pub aux_channels: &'static [AuxChannelId],
    /// Positional readout regions at `frame_min` (`Planar2D` only).
    pub group_regions: &'static [(GroupId, Region)],
    /// Instrument-contributed stress fixtures beyond canonical states.
    pub extreme_states: &'static [ExtremeState],
    /// Pinned reference-raster SHA-256 hex baselines (`Planar2D` only).
    pub raster_baselines: &'static [(DesignFrame, &'static str)],
    /// Pure draw entry point.
    pub draw: DrawFn,
}

/// Alias preserving compatibility with existing planar panel consumers.
pub type PanelDescriptor = InstrumentDescriptor;
