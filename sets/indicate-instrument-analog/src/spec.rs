//! [`AnalogDialSpec`]: one description of a round instrument that both
//! the flat scene and the physical build are made from.
//!
//! # Dial frame
//!
//! Every position in a spec is in face units: the origin is the face
//! centre, `1.0` is the face radius, `+x` is right and `+y` is down,
//! like the scene. Every angle is in degrees clockwise from twelve
//! o'clock. [`AnalogDialSpec::face_radius_mm`] turns face units into
//! millimetres for the physical build.
//!
//! # Borrowed, not owned
//!
//! A spec holds borrowed slices, so it costs no allocation and works in
//! `no_std`. The built-in gauges borrow `static` data; a simulator that
//! reads its instrument packs at run time builds the slices in its own
//! storage and lends them for as long as it draws.

mod check;
mod input;

pub use check::SpecError;
pub use input::{Channel, DialChannel, DialInput, DialUnit};

use indicate_instrument_scene::Rgba8;
use indicate_instrument_symbology::{palette, safety};

/// The most pointers one dial carries.
pub const MAX_POINTERS: usize = 4;
/// The most drum windows one dial carries.
pub const MAX_WINDOWS: usize = 2;
/// The most digit wheels one drum window carries.
pub const MAX_WHEELS: usize = 6;
/// The most corners one pointer outline has.
pub const MAX_OUTLINE_POINTS: usize = 8;

/// A colour as printed, and the colour it glows when the paint is
/// luminous (radium or a modern phosphor).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Paint {
    /// The paint under daylight.
    pub day: Rgba8,
    /// The glow under night lighting; `None` for a paint that does not glow.
    pub luminous: Option<Rgba8>,
}

impl Paint {
    /// A paint that does not glow.
    #[must_use]
    pub const fn plain(day: Rgba8) -> Self {
        Self {
            day,
            luminous: None,
        }
    }

    /// A paint that glows `glow` under night lighting.
    #[must_use]
    pub const fn luminous(day: Rgba8, glow: Rgba8) -> Self {
        Self {
            day,
            luminous: Some(glow),
        }
    }

    /// The colour this paint shows under `lighting`. At night a paint
    /// that does not glow is only lit by the cockpit flood, so it dims.
    #[must_use]
    pub fn under(&self, lighting: Lighting) -> Rgba8 {
        match (lighting, self.luminous) {
            (Lighting::Day, _) => self.day,
            (Lighting::Night, Some(glow)) => glow,
            (Lighting::Night, None) => dim(self.day),
        }
    }
}

fn dim(colour: Rgba8) -> Rgba8 {
    let faint = |channel: u8| channel / 6;
    Rgba8::rgba(faint(colour.r), faint(colour.g), faint(colour.b), colour.a)
}

/// How the face is lit. The face is static under one lighting, so a 3D
/// backend rasterises it once per lighting it shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lighting {
    /// Daylight: every paint shows its printed colour.
    Day,
    /// Night: luminous paint glows and the rest dims.
    Night,
}

/// The colour of a range marking, as the certification rules assign them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkColour {
    /// Flap operating range.
    White,
    /// Normal operating range.
    Green,
    /// Caution range.
    Yellow,
    /// Limit.
    Red,
    /// Best single-engine rate of climb, or another advisory value.
    Blue,
}

/// The advisory blue of a range marking; no safety meaning attaches to it.
const MARK_BLUE: Rgba8 = Rgba8::rgb(40, 110, 255);

impl MarkColour {
    /// The printed colour. Red and yellow are the never-skinnable safety
    /// constants, so a limit on a dial is the same red as a failure flag.
    #[must_use]
    pub const fn rgba(self) -> Rgba8 {
        match self {
            Self::White => palette::WHITE,
            Self::Green => palette::BAND_GREEN,
            Self::Yellow => safety::BAND_CAUTION,
            Self::Red => safety::FAILURE_RED,
            Self::Blue => MARK_BLUE,
        }
    }
}

/// One engraved point of a scale: a value and where it sits.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScalePoint {
    /// The engraved value, in dial units.
    pub value: f32,
    /// Degrees clockwise from the law's reference mark.
    pub angle_deg: f32,
}

/// How a value maps to an angle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScaleLaw<'a> {
    /// Linear between engraved points, held at the end points beyond
    /// them, as a needle stops at its pins.
    Piecewise {
        /// Where the scale's zero angle sits, degrees clockwise from twelve.
        reference_deg: f32,
        /// At least two points, values strictly ascending.
        points: &'a [ScalePoint],
    },
    /// One full turn per `value_per_turn`, unbounded: an altimeter hand.
    Turns {
        /// Where value zero sits, degrees clockwise from twelve.
        reference_deg: f32,
        /// Dial units per revolution.
        value_per_turn: f32,
    },
}

/// A run of evenly spaced graduations.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TickSet {
    /// First graduated value, dial units.
    pub from: f32,
    /// Last graduated value, dial units.
    pub to: f32,
    /// Spacing, dial units.
    pub every: f32,
    /// Inner end, fraction of the ring radius.
    pub inner: f32,
    /// Outer end, fraction of the ring radius.
    pub outer: f32,
    /// Stroke width, face units.
    pub width: f32,
}

/// A printed number on a ring. The text is what the dial prints, which
/// is often not the value: `2` at 200 knots on a dial marked in hundreds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Numeral<'a> {
    /// Where the numeral sits, dial units.
    pub value: f32,
    /// What is printed there.
    pub text: &'a str,
}

/// A coloured arc between two values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarkArc {
    /// Start, dial units.
    pub from: f32,
    /// End, dial units; above `from`.
    pub to: f32,
    /// Its colour.
    pub colour: MarkColour,
    /// Inner edge, fraction of the ring radius.
    pub inner: f32,
    /// Outer edge, fraction of the ring radius.
    pub outer: f32,
}

/// A coloured line across the scale at one value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Radial {
    /// Position, dial units.
    pub at: f32,
    /// Its colour.
    pub colour: MarkColour,
    /// Inner end, fraction of the ring radius.
    pub inner: f32,
    /// Outer end, fraction of the ring radius.
    pub outer: f32,
    /// Stroke width, face units.
    pub width: f32,
}

/// One graduated scale: the main scale of a dial, or a sub-dial.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScaleRing<'a> {
    /// Ring centre, face units.
    pub center: [f32; 2],
    /// Ring radius, face units.
    pub radius: f32,
    /// Where the graduations sit.
    pub law: ScaleLaw<'a>,
    /// Graduations.
    pub ticks: &'a [TickSet],
    /// Printed numerals.
    pub numerals: &'a [Numeral<'a>],
    /// Numeral centres, fraction of the ring radius.
    pub numeral_radius: f32,
    /// Numeral size, face units.
    pub numeral_size: f32,
    /// Coloured range arcs.
    pub arcs: &'a [MarkArc],
    /// Coloured radial lines.
    pub radials: &'a [Radial],
    /// Paint of the graduations and numerals.
    pub paint: Paint,
}

/// Printed text on the face: a units legend, a maker's legend.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Legend<'a> {
    /// What is printed, from the panel glyph vocabulary.
    pub text: &'a str,
    /// Its centre, face units.
    pub at: [f32; 2],
    /// Its size, face units.
    pub size: f32,
    /// Its paint.
    pub paint: Paint,
}

/// The silhouette family of a pointer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerShape {
    /// A long tapered needle.
    Lance,
    /// A short broad hand, such as an altimeter's thousands hand.
    Sword,
    /// A straight bar with a square end.
    Baton,
    /// A thin shaft carrying a triangle near the rim, such as an
    /// altimeter's ten-thousands pointer.
    Triangle,
}

/// How a pointer looks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointerStyle {
    /// Silhouette.
    pub shape: PointerShape,
    /// Pivot to tip, face units.
    pub length: f32,
    /// Widest part, face units.
    pub width: f32,
    /// Counterweight tail behind the pivot, face units.
    pub tail: f32,
    /// Hub cap radius, face units; zero for none.
    pub hub_radius: f32,
    /// Its paint.
    pub paint: Paint,
}

/// Where a pointer goes when its channel shows no value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FailurePose {
    /// Rest at this angle, degrees clockwise from twelve, as a needle
    /// falls to its stop.
    Park {
        /// The resting angle.
        angle_deg: f32,
    },
    /// Not drawn.
    Hide,
}

/// A moving pointer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointerSpec<'a> {
    /// Which of the dial's channels drives it.
    pub channel: Channel,
    /// Its value-to-angle law.
    pub law: ScaleLaw<'a>,
    /// Pivot, face units.
    pub pivot: [f32; 2],
    /// Its look.
    pub style: PointerStyle,
    /// Height of its plane above the face, millimetres. Pointers draw
    /// in ascending height, so the highest is on top in the scene too.
    pub stack_height_mm: f32,
    /// What it does when its channel shows no value.
    pub on_failure: FailurePose,
}

/// A drum window: digit wheels seen through an aperture in the face,
/// such as a Kollsman window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowSpec<'a> {
    /// Which of the dial's channels drives it.
    pub channel: Channel,
    /// Aperture `[x, y, width, height]`, face units.
    pub aperture: [f32; 4],
    /// Digit wheels, rightmost first in the odometer sense.
    pub wheels: u8,
    /// Dial units times this is the integer the wheels show: `100` for
    /// inches of mercury to two decimals.
    pub value_scale: f32,
    /// Mechanism stops, dial units.
    pub range: [f32; 2],
    /// A printed separator after this many wheels from the right, and
    /// its text, such as a decimal point after two.
    pub separator: Option<(u8, &'a str)>,
    /// Digit size, face units.
    pub digit_size: f32,
    /// Paint of the digits.
    pub paint: Paint,
    /// Colour of the drum behind the digits.
    pub drum: Rgba8,
}

/// A failure flag shown in the face when any channel shows no value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OffFlag<'a> {
    /// What the flag reads, from the panel glyph vocabulary.
    pub text: &'a str,
    /// Its centre, face units.
    pub at: [f32; 2],
    /// Its text size, face units.
    pub size: f32,
}

/// One round instrument, described once for every backend.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnalogDialSpec<'a> {
    /// Outside diameter of the case, millimetres.
    pub case_diameter_mm: f32,
    /// Radius of the face, millimetres: one face unit.
    pub face_radius_mm: f32,
    /// The face background.
    pub face: Paint,
    /// The bezel ring between face and case edge.
    pub bezel: Rgba8,
    /// The quantity the dial measures.
    pub primary: DialChannel,
    /// A second quantity, such as a Kollsman setting.
    pub secondary: Option<DialChannel>,
    /// Mechanism stops on the primary, dial units: the input clamps here
    /// before any law reads it.
    pub input_range: Option<[f32; 2]>,
    /// Graduated scales; the first is the main scale.
    pub rings: &'a [ScaleRing<'a>],
    /// Printed text.
    pub legends: &'a [Legend<'a>],
    /// Moving pointers.
    pub pointers: &'a [PointerSpec<'a>],
    /// Drum windows.
    pub windows: &'a [WindowSpec<'a>],
    /// The failure flag, if the instrument has one.
    pub off_flag: Option<OffFlag<'a>>,
}

impl AnalogDialSpec<'_> {
    /// The channel `channel` names, if the dial has it.
    #[must_use]
    pub fn channel(&self, channel: Channel) -> Option<DialChannel> {
        match channel {
            Channel::Primary => Some(self.primary),
            Channel::Secondary => self.secondary,
        }
    }
}
