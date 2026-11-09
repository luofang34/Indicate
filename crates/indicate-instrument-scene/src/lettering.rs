//! Lettering vocabulary: face slots and text along an arc.
//!
//! A scene names faces by slot, never by typeface identity or hash. The
//! host binds each slot to a typeface and a style out of band, so a
//! panel's scene stays the same whatever face a cockpit letters it in,
//! and one instrument can mix faces (scale numerals in one, a bezel
//! legend in another). Slot 0 is the default face.
//!
//! Glyph placement stays with the backend (ADR-0017). A panel that wants
//! numerals or a legend along a dial's arc emits one `TEXT_ARC` run; the
//! backend places each glyph from the bound face's advances and kerning.
//! A panel never positions individual glyphs, so a face change cannot
//! silently change what a panel emits.

/// A face slot: which of the host's bound faces later text uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct FaceSlot(pub u8);

impl FaceSlot {
    /// The default face, in force at the start of every scene.
    pub const DEFAULT: FaceSlot = FaceSlot(0);
}

/// Which way an arc run reads, and so which way its glyphs face.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArcSense {
    /// Reads clockwise round the centre, glyph tops away from it: a
    /// legend along the top of a dial.
    Clockwise,
    /// Reads counter-clockwise, glyph tops toward the centre: a legend
    /// along the bottom of a dial that still reads left to right.
    CounterClockwise,
}

impl ArcSense {
    /// Wire encoding.
    #[must_use]
    pub const fn to_u8(self) -> u8 {
        match self {
            Self::Clockwise => 0,
            Self::CounterClockwise => 1,
        }
    }

    /// Decodes the wire byte; `None` for an unassigned value.
    #[must_use]
    pub const fn from_u8(b: u8) -> Option<Self> {
        match b {
            0 => Some(Self::Clockwise),
            1 => Some(Self::CounterClockwise),
            _ => None,
        }
    }
}

/// Which point of an arc run its anchor angle names, measured along
/// the arc by the run's advance length.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArcAlign {
    /// The anchor is where the run starts.
    Start,
    /// The anchor is the middle of the run.
    Center,
    /// The anchor is where the run ends.
    End,
}

impl ArcAlign {
    /// Wire encoding.
    #[must_use]
    pub const fn to_u8(self) -> u8 {
        match self {
            Self::Start => 0,
            Self::Center => 1,
            Self::End => 2,
        }
    }

    /// Decodes the wire byte; `None` for an unassigned value.
    #[must_use]
    pub const fn from_u8(b: u8) -> Option<Self> {
        match b {
            0 => Some(Self::Start),
            1 => Some(Self::Center),
            2 => Some(Self::End),
            _ => None,
        }
    }
}

/// A text run laid along a circle, as [`crate::Cmd::TextArc`] carries
/// it. Glyph baselines lie on the circle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArcRun<'a> {
    /// Circle centre x.
    pub cx: f32,
    /// Circle centre y.
    pub cy: f32,
    /// Radius of the baseline circle.
    pub radius: f32,
    /// Angle of the anchor point, radians; 0 is `+x` and positive turns
    /// clockwise in the y-down space, as for [`crate::Cmd::Arc`].
    pub angle: f32,
    /// Reading direction.
    pub sense: ArcSense,
    /// Which point of the run `angle` names.
    pub align: ArcAlign,
    /// Run size, as for [`crate::Cmd::Text`]: the face's cap height.
    pub size: f32,
    /// The UTF-8 text.
    pub text: &'a str,
}

impl ArcRun<'_> {
    /// The baseline position and glyph-up rotation at arc length `s`
    /// past the run's start, given the run's total advance length:
    /// `(x, y, rotation)`, where `rotation` is the transform a backend
    /// applies so the glyph's own `+x` runs along the reading direction.
    #[must_use]
    pub fn place(&self, run_length: f32, s: f32) -> (f32, f32, f32) {
        let offset = match self.align {
            ArcAlign::Start => 0.0,
            ArcAlign::Center => run_length * 0.5,
            ArcAlign::End => run_length,
        };
        let r = if self.radius > 0.0 { self.radius } else { 1.0 };
        let (turn, up) = match self.sense {
            ArcSense::Clockwise => (1.0, core::f32::consts::FRAC_PI_2),
            ArcSense::CounterClockwise => (-1.0, -core::f32::consts::FRAC_PI_2),
        };
        let theta = self.angle + turn * (s - offset) / r;
        let x = self.cx + self.radius * libm::cosf(theta);
        let y = self.cy + self.radius * libm::sinf(theta);
        (x, y, theta + up)
    }
}

#[cfg(test)]
mod tests;
