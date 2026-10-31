//! Bounded 3D volumetric scene IR (`RenderSpace::Volume3D`, VOL-01).
//!
//! Volumetric instruments emit metric 3D primitives (`LINE_3D`, `POLYLINE_3D`,
//! `SPHERE`, `BOX_3D`, `CYLINDER_3D`, `RING_3D`, `HEIGHTFIELD_GRID`,
//! `UNDERLAY_ANCHOR_3D`, `BILLBOARD_TEXT`, `TARGET_HANDLE`) partitioned into
//! nine criticality-ordered [`VolumeBandId`] bands.

use indicate_frames::Quat;
use libm::sqrtf;

mod band;
mod cmd;
mod decode;
mod encode;

#[cfg(test)]
mod tests;

pub use crate::angular::Points3Ref;
pub use band::{VolumeBandReport, validate_volume_bands};
pub use cmd::VolumeCmd;
pub use decode::VolumeCmds;
pub use encode::VolumeSceneWriter;

/// Wire format version byte at the start of every 3D volume scene.
pub const VOLUME_SCENE_VERSION: u8 = 1;

/// Maximum encoded byte length of one 3D volume scene (`64 KiB`).
pub const MAX_VOLUME_SCENE_BYTES: usize = 65536;

/// Number of defined [`VolumeBandId`] bands.
pub const VOLUME_BAND_COUNT: usize = 9;

/// Maximum heightfield sample count (`cols * rows <= 1024`).
pub const MAX_HEIGHTFIELD_SAMPLES: usize = 1024;

pub(crate) const UNIT_TOL: f32 = 1.0e-3;
pub(crate) const MAX_POLYLINE_POINTS: usize = 512;

pub(crate) const OP_SAVE: u8 = 0x01;
pub(crate) const OP_RESTORE: u8 = 0x02;
pub(crate) const OP_TRANSLATE_3D: u8 = 0x03;
pub(crate) const OP_ROTATE_QUAT: u8 = 0x04;
pub(crate) const OP_CLIP_BOX_3D: u8 = 0x05;
pub(crate) const OP_FILL_COLOR: u8 = 0x10;
pub(crate) const OP_STROKE_3D: u8 = 0x11;
pub(crate) const OP_ATTRIBUTE: u8 = 0x12;
pub(crate) const OP_TARGET_HANDLE: u8 = 0x13;
pub(crate) const OP_LINE_3D: u8 = 0x20;
pub(crate) const OP_POLYLINE_3D: u8 = 0x21;
pub(crate) const OP_SPHERE: u8 = 0x23;
pub(crate) const OP_BOX_3D: u8 = 0x24;
pub(crate) const OP_CYLINDER_3D: u8 = 0x25;
pub(crate) const OP_RING_3D: u8 = 0x26;
pub(crate) const OP_HEIGHTFIELD_GRID: u8 = 0x27;
pub(crate) const OP_UNDERLAY_ANCHOR_3D: u8 = 0x28;
pub(crate) const OP_BILLBOARD_TEXT: u8 = 0x30;
pub(crate) const OP_BEGIN_BAND: u8 = 0x50;
pub(crate) const OP_END_BAND: u8 = 0x51;

/// Physical bounding-box dimensions of a bounded 3D volumetric display (meters).
///
/// The local origin `(0, 0, 0)` sits at the volume center, spanning
/// `[-width_m / 2, +width_m / 2]`, `[-height_m / 2, +height_m / 2]`, and
/// `[-depth_m / 2, +depth_m / 2]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Extent3D {
    /// Width along local X in meters (`> 0.0`).
    pub width_m: f32,
    /// Height along local Y in meters (`> 0.0`).
    pub height_m: f32,
    /// Depth along local Z in meters (`> 0.0`).
    pub depth_m: f32,
}

impl Extent3D {
    /// Constructs a metric [`Extent3D`].
    #[must_use]
    pub const fn new(width_m: f32, height_m: f32, depth_m: f32) -> Self {
        Self {
            width_m,
            height_m,
            depth_m,
        }
    }

    /// Whether all three dimensions are finite and strictly positive.
    #[must_use]
    pub const fn is_valid(&self) -> bool {
        self.width_m.is_finite()
            && self.height_m.is_finite()
            && self.depth_m.is_finite()
            && self.width_m > 0.0
            && self.height_m > 0.0
            && self.depth_m > 0.0
    }

    /// Half-extents `[width_m / 2, height_m / 2, depth_m / 2]`.
    #[must_use]
    pub fn half_extents(&self) -> [f32; 3] {
        [self.width_m * 0.5, self.height_m * 0.5, self.depth_m * 0.5]
    }

    /// Whether `point` lies within the centered local bounding volume.
    #[must_use]
    pub fn contains_point(&self, point: [f32; 3]) -> bool {
        let h = self.half_extents();
        point[0].is_finite()
            && point[1].is_finite()
            && point[2].is_finite()
            && point[0].abs() <= h[0]
            && point[1].abs() <= h[1]
            && point[2].abs() <= h[2]
    }
}

/// Ordered criticality and composition bands in a 3D volume scene.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum VolumeBandId {
    /// Base metric reference grid and axes (`0`).
    ReferenceGrid = 0,
    /// Host-rendered terrain, planetary globe, or celestial underlay (`1`).
    SurfaceUnderlay = 1,
    /// Solid or translucent 3D airspace / hazard volumes (`2`).
    Volumes3D = 2,
    /// 3D flight paths, tracks, and orbital arcs (`3`).
    Trajectories3D = 3,
    /// 3D aircraft, waypoint, and body symbols (`4`).
    Symbols3D = 4,
    /// Camera-facing billboard labels and numeric readouts (`5`).
    Billboards = 5,
    /// Direct-manipulation spatial hit handles (`6`).
    Handles = 6,
    /// Caution, warning, and degraded-mode annunciations (`7`).
    Annunciation = 7,
    /// Display or source failure indications; highest priority (`8`).
    Failure = 8,
}

impl VolumeBandId {
    /// Canonical wire byte (`0..=8`).
    #[must_use]
    pub const fn to_u8(self) -> u8 {
        self as u8
    }

    /// Decodes a wire byte into a [`VolumeBandId`].
    #[must_use]
    pub const fn from_u8(raw: u8) -> Option<Self> {
        match raw {
            0 => Some(Self::ReferenceGrid),
            1 => Some(Self::SurfaceUnderlay),
            2 => Some(Self::Volumes3D),
            3 => Some(Self::Trajectories3D),
            4 => Some(Self::Symbols3D),
            5 => Some(Self::Billboards),
            6 => Some(Self::Handles),
            7 => Some(Self::Annunciation),
            8 => Some(Self::Failure),
            _ => None,
        }
    }

    /// Human-readable band name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::ReferenceGrid => "ReferenceGrid",
            Self::SurfaceUnderlay => "SurfaceUnderlay",
            Self::Volumes3D => "Volumes3D",
            Self::Trajectories3D => "Trajectories3D",
            Self::Symbols3D => "Symbols3D",
            Self::Billboards => "Billboards",
            Self::Handles => "Handles",
            Self::Annunciation => "Annunciation",
            Self::Failure => "Failure",
        }
    }

    pub(crate) const fn index(self) -> usize {
        self as usize
    }
}

/// Host-supplied 3D surface underlay requested by `UNDERLAY_ANCHOR_3D`.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VolumeUnderlayKind {
    /// Local terrain relief patch (`1`).
    TerrainPatch = 1,
    /// Planetary globe mesh and surface imagery (`2`).
    PlanetaryGlobe = 2,
    /// Multi-body celestial / orbital system backdrop (`3`).
    CelestialSystem = 3,
}

impl VolumeUnderlayKind {
    /// Canonical wire byte (`1..=3`).
    #[must_use]
    pub const fn to_u8(self) -> u8 {
        self as u8
    }

    /// Decodes a wire byte into a [`VolumeUnderlayKind`].
    #[must_use]
    pub const fn from_u8(raw: u8) -> Option<Self> {
        match raw {
            1 => Some(Self::TerrainPatch),
            2 => Some(Self::PlanetaryGlobe),
            3 => Some(Self::CelestialSystem),
            _ => None,
        }
    }
}

/// Semantic kind of an interactive 3D `TARGET_HANDLE`.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HandleKind {
    /// Generic selectable 3D point (`1`).
    Point = 1,
    /// Lateral/3D navigation waypoint handle (`2`).
    Waypoint = 2,
    /// Vertical altitude bug handle (`3`).
    AltitudeBug = 3,
    /// Orbital or trajectory manoeuvre node (`4`).
    ManoeuvreNode = 4,
    /// Bounding box or sector boundary handle (`5`).
    VolumeBoundary = 5,
}

impl HandleKind {
    /// Canonical wire byte (`1..=5`).
    #[must_use]
    pub const fn to_u8(self) -> u8 {
        self as u8
    }

    /// Decodes a wire byte into a [`HandleKind`].
    #[must_use]
    pub const fn from_u8(raw: u8) -> Option<Self> {
        match raw {
            1 => Some(Self::Point),
            2 => Some(Self::Waypoint),
            3 => Some(Self::AltitudeBug),
            4 => Some(Self::ManoeuvreNode),
            5 => Some(Self::VolumeBoundary),
            _ => None,
        }
    }
}

/// Zero-copy view over packed little-endian `f32` height samples (`4` bytes each).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeightsRef<'a> {
    bytes: &'a [u8],
}

impl<'a> HeightsRef<'a> {
    pub(crate) const fn from_bytes(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }

    /// Number of height samples (`cols * rows`).
    #[must_use]
    pub const fn len(&self) -> usize {
        self.bytes.len() / 4
    }

    /// Whether there are zero height samples.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Reads sample `index`, if in bounds.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<f32> {
        let start = index.checked_mul(4)?;
        let chunk = self.bytes.get(start..start + 4)?;
        Some(read_f32(chunk))
    }

    /// Iterates over all height samples in row-major order.
    pub fn iter(&self) -> impl Iterator<Item = f32> + 'a {
        let len = self.len();
        let copy = *self;
        (0..len).filter_map(move |idx| copy.get(idx))
    }
}

pub(crate) fn is_finite_vec3(v: [f32; 3]) -> bool {
    v[0].is_finite() && v[1].is_finite() && v[2].is_finite()
}

pub(crate) fn is_ordered_box(min: [f32; 3], max: [f32; 3]) -> bool {
    is_finite_vec3(min)
        && is_finite_vec3(max)
        && min[0] <= max[0]
        && min[1] <= max[1]
        && min[2] <= max[2]
}

pub(crate) fn is_unit_vec3(v: [f32; 3]) -> bool {
    if !is_finite_vec3(v) {
        return false;
    }
    let norm = sqrtf(v[0] * v[0] + v[1] * v[1] + v[2] * v[2]);
    norm.is_finite() && (norm - 1.0).abs() <= UNIT_TOL
}

pub(crate) fn is_unit_quat(q: Quat) -> bool {
    if !(q.x.is_finite() && q.y.is_finite() && q.z.is_finite() && q.w.is_finite()) {
        return false;
    }
    let norm = sqrtf(q.x * q.x + q.y * q.y + q.z * q.z + q.w * q.w);
    norm.is_finite() && (norm - 1.0).abs() <= UNIT_TOL
}

pub(crate) fn read_f32(slice: &[u8]) -> f32 {
    let mut b = [0u8; 4];
    b.copy_from_slice(&slice[..4]);
    f32::from_le_bytes(b)
}
