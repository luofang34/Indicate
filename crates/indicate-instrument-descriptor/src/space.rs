//! Render spaces, underlay contracts, scene targets, and layer bit helpers.

use indicate_instrument_scene::{
    LayerId, SceneWriter,
    angular::{AngularLayerId, AngularSceneWriter},
    volume::{Extent3D, VolumeBandId, VolumeSceneWriter},
};
use indicate_interaction::ViewLimits;

use crate::descriptor::DesignFrame;

/// Coordinate space and sizing law an instrument renders against.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RenderSpace {
    /// Two-dimensional logical design frame with bounded resize grid.
    Planar2D {
        /// Smallest supported logical frame (readability floor).
        frame_min: DesignFrame,
        /// Largest supported logical frame.
        frame_max: DesignFrame,
        /// Per-axis quantization step (`frame_min + k * frame_step`).
        frame_step: (f32, f32),
        /// Smallest admissible width/height ratio.
        aspect_min: f32,
        /// Largest admissible width/height ratio.
        aspect_max: f32,
        /// Pinned evidence frames (must include `frame_min` and `frame_max`).
        canonical_frames: &'static [DesignFrame],
    },
    /// Conformal unit-sphere direction space for head-worn or dome displays,
    /// with an optional 2D peripheral overlay frame.
    AngularSphere {
        /// Optional fixed 2D overlay design frame drawn alongside angular cues.
        overlay_frame: Option<DesignFrame>,
    },
    /// Bounded metric 3D volume (`Extent3D` in metres) for volumetric displays.
    Volume3D {
        /// Smallest supported metric volume extent.
        extent_min_m: Extent3D,
        /// Largest supported metric volume extent.
        extent_max_m: Extent3D,
        /// Pinned canonical extents for digest and admission sweeps.
        canonical_extents_m: &'static [Extent3D],
        /// View manipulation bounds enforced by `apply_event`.
        view_limits: ViewLimits,
    },
}

impl RenderSpace {
    /// Wire discriminant (`0 = Planar2D`, `1 = AngularSphere`, `2 = Volume3D`).
    #[must_use]
    pub const fn kind_byte(&self) -> u8 {
        match self {
            Self::Planar2D { .. } => 0,
            Self::AngularSphere { .. } => 1,
            Self::Volume3D { .. } => 2,
        }
    }

    /// Short display label for set documentation and diagnostics.
    #[must_use]
    pub const fn kind_name(&self) -> &'static str {
        match self {
            Self::Planar2D { .. } => "Planar2D",
            Self::AngularSphere { .. } => "AngularSphere",
            Self::Volume3D { .. } => "Volume3D",
        }
    }
}

/// Host underlay/background contract declared by an instrument.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum UnderlayContract {
    /// Transparent background; never paints a background fill or underlay anchor.
    None = 0,
    /// Owns an opaque self-painted background.
    Opaque = 1,
    /// Cedes background on request to a 2D map underlay.
    Map2D = 2,
    /// Draws a default horizon fill and cedes background to conformal 3D SVS.
    ConformalSvs3D = 3,
    /// Cedes background on request to calibrated sensor video (EVS).
    CalibratedVideo = 4,
    /// Cedes background on request to combined synthetic/enhanced vision (XVS).
    CombinedXvs = 5,
    /// Anchors a metric 3D terrain elevation/imagery patch inside `SurfaceUnderlay`.
    VolumetricTerrain3D = 6,
    /// Anchors a 3D planetary globe inside `SurfaceUnderlay`.
    PlanetaryGlobe3D = 7,
    /// Anchors a 3D celestial system underlay inside `SurfaceUnderlay`.
    CelestialSystem3D = 8,
}

impl UnderlayContract {
    /// Wire byte representation (`0..=8`).
    #[must_use]
    pub const fn to_u8(self) -> u8 {
        self as u8
    }

    /// Fail-closed decode from wire byte.
    #[must_use]
    pub const fn from_u8(byte: u8) -> Option<Self> {
        match byte {
            0 => Some(Self::None),
            1 => Some(Self::Opaque),
            2 => Some(Self::Map2D),
            3 => Some(Self::ConformalSvs3D),
            4 => Some(Self::CalibratedVideo),
            5 => Some(Self::CombinedXvs),
            6 => Some(Self::VolumetricTerrain3D),
            7 => Some(Self::PlanetaryGlobe3D),
            8 => Some(Self::CelestialSystem3D),
            _ => None,
        }
    }

    /// Canonical name used in generated set documentation.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Opaque => "Opaque",
            Self::Map2D => "Map2D",
            Self::ConformalSvs3D => "ConformalSvs3D",
            Self::CalibratedVideo => "CalibratedVideo",
            Self::CombinedXvs => "CombinedXvs",
            Self::VolumetricTerrain3D => "VolumetricTerrain3D",
            Self::PlanetaryGlobe3D => "PlanetaryGlobe3D",
            Self::CelestialSystem3D => "CelestialSystem3D",
        }
    }

    /// Whether a 2D slot with this contract owns an opaque cover by default.
    #[must_use]
    pub const fn owns_2d_cover(self) -> bool {
        matches!(
            self,
            Self::Opaque
                | Self::Map2D
                | Self::ConformalSvs3D
                | Self::CalibratedVideo
                | Self::CombinedXvs
        )
    }

    /// Whether this contract is valid for a `Planar2D` instrument.
    #[must_use]
    pub const fn valid_for_planar_2d(self) -> bool {
        matches!(
            self,
            Self::None
                | Self::Opaque
                | Self::Map2D
                | Self::ConformalSvs3D
                | Self::CalibratedVideo
                | Self::CombinedXvs
        )
    }

    /// Whether this contract is valid for an `AngularSphere` instrument.
    #[must_use]
    pub const fn valid_for_angular_sphere(self) -> bool {
        matches!(
            self,
            Self::None | Self::ConformalSvs3D | Self::CalibratedVideo | Self::CombinedXvs
        )
    }

    /// Whether this contract is valid for a `Volume3D` instrument.
    #[must_use]
    pub const fn valid_for_volume_3d(self) -> bool {
        matches!(
            self,
            Self::None
                | Self::Opaque
                | Self::VolumetricTerrain3D
                | Self::PlanetaryGlobe3D
                | Self::CelestialSystem3D
        )
    }
}

/// Runtime availability and calibration status of the host underlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnderlayStatus {
    /// Whether the host underlay feed is present.
    pub available: bool,
    /// Whether the underlay registration/alignment is calibrated.
    pub calibrated: bool,
    /// Feed quality percentage (`0..=100`).
    pub quality_pct: u8,
}

impl UnderlayStatus {
    /// Absent underlay status supplied when no host underlay is active.
    pub const UNAVAILABLE: Self = Self {
        available: false,
        calibrated: false,
        quality_pct: 0,
    };
}

impl Default for UnderlayStatus {
    fn default() -> Self {
        Self::UNAVAILABLE
    }
}

/// Destination sink passed into [`crate::DrawFn`], matched to [`RenderSpace`].
pub enum SceneTarget<'a, 'b> {
    /// Planar 2D scene sink at a chosen logical `DesignFrame`.
    Planar2D {
        /// Logical frame the instrument must lay out against.
        frame: DesignFrame,
        /// Planar 2D scene writer.
        writer: &'a mut SceneWriter<'b>,
    },
    /// Unit-sphere angular scene sink with optional 2D peripheral overlay.
    AngularSphere {
        /// Conformal angular scene writer.
        angular: &'a mut AngularSceneWriter<'b>,
        /// Optional 2D peripheral overlay frame and writer.
        overlay: Option<(DesignFrame, &'a mut SceneWriter<'b>)>,
    },
    /// Bounded 3D volumetric scene sink at a chosen metric `Extent3D`.
    Volume3D {
        /// Metric volume dimensions in metres.
        extent_m: Extent3D,
        /// Volumetric 3D scene writer.
        writer: &'a mut VolumeSceneWriter<'b>,
    },
}

/// Bit position (`0..6`) of a planar 2D [`LayerId`] inside `required_layers`.
#[must_use]
pub const fn planar_layer_bit(layer: LayerId) -> u16 {
    1u16 << (layer as u8)
}

/// Bit position (`0..6`) of an [`AngularLayerId`] inside `required_layers`.
#[must_use]
pub const fn angular_layer_bit(layer: AngularLayerId) -> u16 {
    1u16 << (layer as u8)
}

/// Bit position (`8..14`) of a 2D overlay [`LayerId`] on `AngularSphere` descriptors.
#[must_use]
pub const fn angular_overlay_layer_bit(layer: LayerId) -> u16 {
    1u16 << (8 + layer as u8)
}

/// Bit position (`0..9`) of a [`VolumeBandId`] inside `required_layers`.
#[must_use]
pub const fn volume_band_bit(band: VolumeBandId) -> u16 {
    1u16 << (band as u8)
}
