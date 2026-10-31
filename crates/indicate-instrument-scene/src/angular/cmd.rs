//! Decoded command vocabulary for [`crate::angular`].

use crate::cmd::HAlign;
use crate::color::Rgba8;

use super::{AngularLayerId, AngularReferenceKind, Points3Ref};

/// One decoded unit-sphere angular drawing command.
#[derive(Debug, Clone, PartialEq)]
pub enum AngularCmd<'a> {
    /// Push graphics state (`0x01`).
    Save,
    /// Pop graphics state (`0x02`).
    Restore,
    /// Set the active orthonormal angular basis (`0x03`).
    Basis {
        /// Reference frame kind.
        kind: AngularReferenceKind,
        /// Right unit axis.
        right: [f32; 3],
        /// Up unit axis.
        up: [f32; 3],
        /// Forward unit axis.
        forward: [f32; 3],
    },
    /// Set fill paint color (`0x10`).
    FillColor {
        /// RGBA8 color.
        color: Rgba8,
    },
    /// Set stroke color and angular line width in milliradians (`0x11`).
    StrokeMrad {
        /// RGBA8 color.
        color: Rgba8,
        /// Line width in milliradians (`>= 0.0`).
        width_mrad: f32,
    },
    /// Declare state-group provenance for the next text run (`0x12`).
    Attribute {
        /// State-group wire discriminant, or [`crate::ATTR_CONFIG`].
        group: u8,
    },
    /// Tag subsequent geometry with an interactive target identifier (`0x13`).
    TargetTag {
        /// Target handle identifier (`0` clears active tag).
        target_id: u32,
    },
    /// Geodesic / ray segment between two unit directions (`0x20`).
    RayLine {
        /// Start unit direction.
        a: [f32; 3],
        /// End unit direction.
        b: [f32; 3],
    },
    /// Connected ray polyline through `2..=512` unit directions (`0x21`).
    RayPolyline {
        /// Borrowed unit direction vertices.
        points: Points3Ref<'a>,
    },
    /// Great-circle arc swept around `axis` starting at `start` (`0x22`).
    GreatCircleArc {
        /// Unit rotation axis.
        axis: [f32; 3],
        /// Unit start direction.
        start: [f32; 3],
        /// Signed sweep angle in radians.
        sweep_rad: f32,
    },
    /// Small-circle reticle ring around `center` (`0x23`).
    ReticleRing {
        /// Unit center direction.
        center: [f32; 3],
        /// Angular radius in radians (`> 0.0`).
        radius_rad: f32,
    },
    /// Billboarded angular text anchored at unit direction `dir` (`0x30`).
    AngularText {
        /// Unit placement direction.
        dir: [f32; 3],
        /// Glyph height in milliradians (`> 0.0`).
        size_mrad: f32,
        /// Horizontal alignment (`0..=2`).
        anchor: HAlign,
        /// UTF-8 text run (`1..=250` bytes).
        text: &'a str,
    },
    /// Open criticality layer (`0x50`).
    BeginLayer {
        /// Opened angular layer.
        layer: AngularLayerId,
    },
    /// Close criticality layer (`0x51`).
    EndLayer {
        /// Closed angular layer.
        layer: AngularLayerId,
    },
    /// Forward-compatible unknown opcode skipped using its length prefix.
    Unknown {
        /// Raw wire opcode byte.
        opcode: u8,
    },
}
