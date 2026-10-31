//! Conformal unit-sphere angular scene IR (`RenderSpace::AngularSphere`, VOL-01).
//!
//! Head-worn and dome instruments emit directional geometry on the unit sphere
//! (`RAY_LINE`, `RAY_POLYLINE`, `GREAT_CIRCLE_ARC`, `RETICLE_RING`,
//! `ANGULAR_TEXT`) partitioned into six criticality-ordered [`AngularLayerId`]
//! bands.

use libm::sqrtf;

mod cmd;
mod decode;
mod encode;
mod layer;

#[cfg(test)]
mod tests;

pub use cmd::AngularCmd;
pub use decode::AngularCmds;
pub use encode::AngularSceneWriter;
pub use layer::{AngularLayerReport, validate_angular_layers};

/// Wire format version byte at the start of every angular scene.
pub const ANGULAR_SCENE_VERSION: u8 = 1;

/// Maximum encoded byte length of one angular scene (`32 KiB`).
pub const MAX_ANGULAR_SCENE_BYTES: usize = 32768;

/// Number of defined [`AngularLayerId`] bands.
pub const ANGULAR_LAYER_COUNT: usize = 6;

pub(crate) const UNIT_TOL: f32 = 1.0e-3;
pub(crate) const MAX_POLYLINE_POINTS: usize = 512;

pub(crate) const OP_SAVE: u8 = 0x01;
pub(crate) const OP_RESTORE: u8 = 0x02;
pub(crate) const OP_BASIS: u8 = 0x03;
pub(crate) const OP_FILL_COLOR: u8 = 0x10;
pub(crate) const OP_STROKE_MRAD: u8 = 0x11;
pub(crate) const OP_ATTRIBUTE: u8 = 0x12;
pub(crate) const OP_TARGET_TAG: u8 = 0x13;
pub(crate) const OP_RAY_LINE: u8 = 0x20;
pub(crate) const OP_RAY_POLYLINE: u8 = 0x21;
pub(crate) const OP_GREAT_CIRCLE_ARC: u8 = 0x22;
pub(crate) const OP_RETICLE_RING: u8 = 0x23;
pub(crate) const OP_ANGULAR_TEXT: u8 = 0x30;
pub(crate) const OP_BEGIN_LAYER: u8 = 0x50;
pub(crate) const OP_END_LAYER: u8 = 0x51;

/// Z-ordered criticality layers for [`AngularSceneWriter`].
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AngularLayerId {
    /// Horizon, compass, and world-referenced lines.
    WorldCue = 0,
    /// Flight-path vector and energy cues.
    FlightPath = 1,
    /// Commanded attitude or trajectory guidance cues.
    Guidance = 2,
    /// Simulated external contacts or traffic markers.
    Contacts = 3,
    /// Mode, caution, warning, and degraded-state annunciations.
    Annunciation = 4,
    /// Display or source failure indications; highest priority.
    Failure = 5,
}

impl AngularLayerId {
    /// Canonical wire byte (`0..=5`).
    #[must_use]
    pub const fn to_u8(self) -> u8 {
        self as u8
    }

    /// Decodes a wire byte into an [`AngularLayerId`].
    #[must_use]
    pub const fn from_u8(raw: u8) -> Option<Self> {
        match raw {
            0 => Some(Self::WorldCue),
            1 => Some(Self::FlightPath),
            2 => Some(Self::Guidance),
            3 => Some(Self::Contacts),
            4 => Some(Self::Annunciation),
            5 => Some(Self::Failure),
            _ => None,
        }
    }

    /// Human-readable layer name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::WorldCue => "WorldCue",
            Self::FlightPath => "FlightPath",
            Self::Guidance => "Guidance",
            Self::Contacts => "Contacts",
            Self::Annunciation => "Annunciation",
            Self::Failure => "Failure",
        }
    }

    pub(crate) const fn index(self) -> usize {
        self as usize
    }
}

/// Reference frame declared by an angular `BASIS` command.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AngularReferenceKind {
    /// Head-fixed / unreferenced sight coordinates.
    Unreferenced = 0,
    /// Local-level horizon frame (NED / ENU).
    LocalLevel = 1,
    /// Vehicle body frame.
    Body = 2,
    /// Inertial or ecliptic celestial frame.
    Ecliptic = 3,
}

impl AngularReferenceKind {
    /// Canonical wire byte (`0..=3`).
    #[must_use]
    pub const fn to_u8(self) -> u8 {
        self as u8
    }

    /// Decodes a wire byte into an [`AngularReferenceKind`].
    #[must_use]
    pub const fn from_u8(raw: u8) -> Option<Self> {
        match raw {
            0 => Some(Self::Unreferenced),
            1 => Some(Self::LocalLevel),
            2 => Some(Self::Body),
            3 => Some(Self::Ecliptic),
            _ => None,
        }
    }
}

/// Zero-copy view over packed little-endian `[f32; 3]` vertices (`12` bytes each).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Points3Ref<'a> {
    bytes: &'a [u8],
}

impl<'a> Points3Ref<'a> {
    pub(crate) const fn from_bytes(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }

    /// Number of 3D vertices in this sequence.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.bytes.len() / 12
    }

    /// Whether the sequence contains zero vertices.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Reads vertex `index`, if in bounds.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<[f32; 3]> {
        let start = index.checked_mul(12)?;
        let chunk = self.bytes.get(start..start + 12)?;
        Some([
            read_f32(&chunk[0..4]),
            read_f32(&chunk[4..8]),
            read_f32(&chunk[8..12]),
        ])
    }

    /// Iterates over all `[f32; 3]` vertices in order.
    pub fn iter(&self) -> impl Iterator<Item = [f32; 3]> + 'a {
        let len = self.len();
        let copy = *self;
        (0..len).filter_map(move |idx| copy.get(idx))
    }
}

pub(crate) fn is_unit_vec3(v: [f32; 3]) -> bool {
    if !(v[0].is_finite() && v[1].is_finite() && v[2].is_finite()) {
        return false;
    }
    let norm = sqrtf(v[0] * v[0] + v[1] * v[1] + v[2] * v[2]);
    norm.is_finite() && (norm - 1.0).abs() <= UNIT_TOL
}

pub(crate) fn read_f32(slice: &[u8]) -> f32 {
    let mut b = [0u8; 4];
    b.copy_from_slice(&slice[..4]);
    f32::from_le_bytes(b)
}
