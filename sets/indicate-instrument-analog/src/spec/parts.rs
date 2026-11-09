//! Parts beyond pointers and digit drums: printed marks, moving cards,
//! scrolling cards seen through a window, and fixed parts in front of
//! everything that moves.
//!
//! Each moving part still moves by transforms alone. A card turns about
//! its pivot and slides along its own vertical axis, such as a gyro
//! horizon bar. A tape is a card printed round a drum and read through a
//! window, such as a directional gyro's or a compass's card: the 2D scene
//! slides it as a flat strip, and the physical build turns the drum
//! about its axis by the same arc length.

use super::{Channel, Paint, ScaleLaw};

/// The most moving cards one dial carries.
pub const MAX_CARDS: usize = 2;
/// The most tapes one dial carries.
pub const MAX_TAPES: usize = 1;

/// A static shape, in face units. Angles are degrees clockwise from
/// twelve o'clock.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Shape<'a> {
    /// A straight stroke.
    Line {
        /// Start point.
        from: [f32; 2],
        /// End point.
        to: [f32; 2],
        /// Stroke width.
        width: f32,
    },
    /// A stroke along a circle, such as a turn indicator's ball tube.
    Arc {
        /// Circle centre.
        center: [f32; 2],
        /// Radius of the stroke's middle.
        radius: f32,
        /// Start angle.
        from_deg: f32,
        /// End angle, clockwise of the start.
        to_deg: f32,
        /// Stroke width.
        width: f32,
    },
    /// A filled circle.
    Disc {
        /// Centre.
        center: [f32; 2],
        /// Radius.
        radius: f32,
    },
    /// A filled rectangle `[x, y, width, height]`.
    Rect([f32; 4]),
    /// A filled polygon.
    Polygon(&'a [[f32; 2]]),
}

/// A shape and its paint.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mark<'a> {
    /// The geometry.
    pub shape: Shape<'a>,
    /// Its paint.
    pub paint: Paint,
}

/// How a card turns: a channel and its value-to-angle law.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CardTurn<'a> {
    /// The channel that turns the card.
    pub channel: Channel,
    /// Its value-to-angle law.
    pub law: ScaleLaw<'a>,
}

/// How a card slides along its own vertical axis, after it turns.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Travel {
    /// The channel that slides the card.
    pub channel: Channel,
    /// Face units of travel per dial unit, positive down the card.
    pub per_unit: f32,
    /// Mechanism stops, face units: the travel holds here.
    pub limit: [f32; 2],
}

/// A moving card: shapes that turn about a pivot and slide along the
/// card's own vertical axis, drawn translate, turn, slide.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CardSpec<'a> {
    /// The card's shapes, in face units about the pivot, as they stand
    /// at no turn and no travel.
    pub shapes: &'a [Mark<'a>],
    /// Pivot, face units.
    pub pivot: [f32; 2],
    /// How it turns; `None` for a card that only slides.
    pub turn: Option<CardTurn<'a>>,
    /// How it slides; `None` for a card that only turns.
    pub travel: Option<Travel>,
    /// Height of its plane above the face, millimetres.
    pub stack_height_mm: f32,
    /// What it does when a channel it follows shows no value: `Park`
    /// turns it to the angle with no travel, `Hide` leaves it out.
    pub on_failure: super::FailurePose,
}

/// Which way a tape's values run across its window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TapeSense {
    /// Higher values lie to the right of the reading point, as on a
    /// directional gyro's drum.
    RightIncreasing,
    /// Higher values lie to the left, as on a magnetic compass card,
    /// which is read from behind its north.
    LeftIncreasing,
}

/// A run of evenly spaced graduations on a tape.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TapeTicks {
    /// Spacing, dial units; a whole number of them fills the period.
    pub every: f32,
    /// Top end, fraction of the window height from its top.
    pub top: f32,
    /// Bottom end, fraction of the window height from its top.
    pub bottom: f32,
    /// Stroke width, face units.
    pub width: f32,
}

/// A card printed round a drum, read through a window at the point
/// under its centre, such as a heading card.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TapeSpec<'a> {
    /// Which of the dial's channels drives it.
    pub channel: Channel,
    /// Window `[x, y, width, height]`, face units.
    pub aperture: [f32; 4],
    /// Dial units in one turn of the drum: 360 for a heading card.
    pub period: f32,
    /// Face units of card per dial unit.
    pub pitch: f32,
    /// Which way values run.
    pub sense: TapeSense,
    /// Graduations.
    pub ticks: &'a [TapeTicks],
    /// Printed numerals; their values lie in `[0, period)`.
    pub numerals: &'a [super::Numeral<'a>],
    /// Numeral centres, fraction of the window height from its top.
    pub numeral_at: f32,
    /// Numeral size, face units.
    pub numeral_size: f32,
    /// Paint of the graduations and numerals.
    pub paint: Paint,
    /// Colour of the card behind them.
    pub drum: indicate_instrument_scene::Rgba8,
}

/// A fixed part in front of the moving parts, such as a gyro horizon's
/// miniature aircraft or a lubber line.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OverlaySpec<'a> {
    /// Its shapes, face units.
    pub shapes: &'a [Mark<'a>],
    /// Height of its plane above the face, millimetres: above every
    /// moving part it covers.
    pub stack_height_mm: f32,
}
