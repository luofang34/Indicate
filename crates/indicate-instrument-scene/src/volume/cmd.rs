//! Decoded command vocabulary for [`crate::volume`].

use indicate_frames::Quat;

use crate::cmd::{HAlign, PaintMode};
use crate::color::Rgba8;

use super::{HandleKind, HeightsRef, Points3Ref, VolumeBandId, VolumeUnderlayKind};

/// One decoded 3D volumetric drawing command.
#[derive(Debug, Clone, PartialEq)]
pub enum VolumeCmd<'a> {
    /// Push 3D transform, clip box, and paint state (`0x01`).
    Save,
    /// Pop 3D transform, clip box, and paint state (`0x02`).
    Restore,
    /// Translate current 3D frame by `delta` meters (`0x03`).
    Translate3D {
        /// Translation `[dx, dy, dz]` in meters.
        delta: [f32; 3],
    },
    /// Rotate current 3D frame by unit quaternion (`0x04`).
    RotateQuat {
        /// Unit rotation quaternion.
        rotation: Quat,
    },
    /// Intersect current 3D clip volume with axis-aligned box (`0x05`).
    ClipBox3D {
        /// Minimum corner `[x, y, z]` in meters.
        min: [f32; 3],
        /// Maximum corner `[x, y, z]` in meters (`min <= max`).
        max: [f32; 3],
    },
    /// Set fill paint color (`0x10`).
    FillColor {
        /// RGBA8 color.
        color: Rgba8,
    },
    /// Set 3D tube/wire stroke color and radius in meters (`0x11`).
    Stroke3D {
        /// RGBA8 color.
        color: Rgba8,
        /// Tube radius in meters (`>= 0.0`).
        radius_m: f32,
    },
    /// Declare state-group provenance for the next billboard text run (`0x12`).
    Attribute {
        /// State-group wire discriminant, or [`crate::ATTR_CONFIG`].
        group: u8,
    },
    /// Emit an interactive spherical hit handle (`0x13`).
    TargetHandle {
        /// Non-zero handle identifier.
        target_id: u32,
        /// Semantic handle kind (`1..=5`).
        kind: HandleKind,
        /// Handle center `[x, y, z]` in meters.
        center: [f32; 3],
        /// Hit sphere radius in meters (`> 0.0`).
        radius_m: f32,
    },
    /// Straight 3D line segment between `a` and `b` (`0x20`).
    Line3D {
        /// Start point in meters.
        a: [f32; 3],
        /// End point in meters.
        b: [f32; 3],
    },
    /// Open 3D polyline through `2..=512` vertices (`0x21`).
    Polyline3D {
        /// Borrowed `[x, y, z]` vertices in meters.
        points: Points3Ref<'a>,
    },
    /// Sphere primitive (`0x23`).
    Sphere {
        /// Fill/stroke paint mode (`0..=2` on wire).
        mode: PaintMode,
        /// Sphere center in meters.
        center: [f32; 3],
        /// Sphere radius in meters (`> 0.0`).
        radius_m: f32,
    },
    /// Axis-aligned 3D box primitive (`0x24`).
    Box3D {
        /// Fill/stroke paint mode (`0..=2` on wire).
        mode: PaintMode,
        /// Minimum corner in meters.
        min: [f32; 3],
        /// Maximum corner in meters (`min <= max`).
        max: [f32; 3],
    },
    /// Finite cylinder between `base` and `top` (`0x25`).
    Cylinder3D {
        /// Fill/stroke paint mode (`0..=2` on wire).
        mode: PaintMode,
        /// Base cap center in meters.
        base: [f32; 3],
        /// Top cap center in meters.
        top: [f32; 3],
        /// Cylinder radius in meters (`> 0.0`).
        radius_m: f32,
    },
    /// Stroked planar ring in 3D (`0x26`).
    Ring3D {
        /// Ring center in meters.
        center: [f32; 3],
        /// Unit plane normal.
        normal: [f32; 3],
        /// Ring radius in meters (`> 0.0`).
        radius_m: f32,
    },
    /// Regular `cols × rows` elevation mesh (`0x27`).
    HeightfieldGrid {
        /// Column count (`>= 2`).
        cols: u8,
        /// Row count (`>= 2`).
        rows: u8,
        /// Grid origin `[x, y, z]` in meters.
        origin: [f32; 3],
        /// Column spacing along X in meters (`> 0.0`).
        step_x: f32,
        /// Row spacing along Z in meters (`> 0.0`).
        step_z: f32,
        /// Borrowed `cols * rows` Y-elevation offsets in meters.
        heights: HeightsRef<'a>,
    },
    /// Host-rendered 3D surface underlay registration anchor (`0x28`).
    UnderlayAnchor3D {
        /// Requested 3D underlay kind (`1..=3`).
        kind: VolumeUnderlayKind,
        /// Anchor center in local volume meters.
        center: [f32; 3],
        /// Positive bounding extent `[w, h, d]` in meters (`> 0.0`).
        extent: [f32; 3],
        /// Unit orientation quaternion.
        orientation: Quat,
        /// World meters represented per local volume meter (`> 0.0`).
        meters_per_unit: f64,
    },
    /// Camera-facing 3D billboard text (`0x30`).
    BillboardText {
        /// Anchor position in local volume meters.
        pos: [f32; 3],
        /// Glyph height in local volume meters (`> 0.0`).
        size_m: f32,
        /// Horizontal alignment (`0..=2`).
        anchor: HAlign,
        /// UTF-8 text run (`1..=250` bytes).
        text: &'a str,
    },
    /// Open criticality band (`0x50`).
    BeginBand {
        /// Opened volume band (`0..=8`).
        band: VolumeBandId,
    },
    /// Close criticality band (`0x51`).
    EndBand {
        /// Closed volume band (`0..=8`).
        band: VolumeBandId,
    },
    /// Forward-compatible unknown opcode skipped using its length prefix.
    Unknown {
        /// Raw wire opcode byte.
        opcode: u8,
    },
}
