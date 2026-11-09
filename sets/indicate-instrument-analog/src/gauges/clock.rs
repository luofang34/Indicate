//! The 2-1/4 in panel clock and engine synchroscope of a 1943 fighter
//! panel.

use indicate_instrument_scene::Rgba8;

use super::{
    BEZEL, CASE_2_1_4_MM, FACE, FACE_2_1_4_MM, LEGEND, MARKING, NEEDLE_PAINT, OFF, SYMBOL,
};
use crate::spec::{
    AnalogDialSpec, Channel, DialChannel, DialInput, DialUnit, FailurePose, Legend, Mark, Numeral,
    Paint, PointerShape, PointerSpec, PointerStyle, ScaleLaw, ScaleRing, Shape, TickSet,
};

/// A hand that turns once per `seconds`, from twelve at midnight.
const fn hand(seconds: f32, style: PointerStyle, stack_height_mm: f32) -> PointerSpec<'static> {
    PointerSpec {
        channel: Channel::Primary,
        law: ScaleLaw::Turns {
            reference_deg: 0.0,
            value_per_turn: seconds,
        },
        pivot: [0.0, 0.0],
        style,
        stack_height_mm,
        // A clock with no reading shows no hands: hands at any position
        // would claim a time.
        on_failure: FailurePose::Hide,
    }
}

/// Minute graduations, longer every five, and the quarter numerals.
const CLOCK_RING: ScaleRing<'static> = ScaleRing {
    center: [0.0, 0.0],
    radius: 0.95,
    law: ScaleLaw::Turns {
        reference_deg: 0.0,
        value_per_turn: 60.0,
    },
    ticks: &[
        TickSet {
            from: 0.0,
            to: 59.0,
            every: 1.0,
            inner: 0.92,
            outer: 1.0,
            width: 0.02,
        },
        TickSet {
            from: 0.0,
            to: 55.0,
            every: 5.0,
            inner: 0.82,
            outer: 1.0,
            width: 0.05,
        },
    ],
    numerals: &[
        Numeral {
            value: 0.0,
            text: "12",
        },
        Numeral {
            value: 15.0,
            text: "3",
        },
        Numeral {
            value: 30.0,
            text: "6",
        },
        Numeral {
            value: 45.0,
            text: "9",
        },
    ],
    numeral_radius: 0.66,
    numeral_size: 0.2,
    arcs: &[],
    radials: &[],
    paint: MARKING,
};

/// The sweep second hand's red.
const SECONDS_RED: Paint = Paint::plain(Rgba8::rgb(200, 40, 30));

/// The panel clock: hour, minute, and sweep-second hands from the time
/// of day the clock shows. It reads twelve hours a turn, so 15:00 and
/// 03:00 look alike, as on the instrument.
pub const CLOCK: AnalogDialSpec<'static> = AnalogDialSpec {
    case_diameter_mm: CASE_2_1_4_MM,
    face_radius_mm: FACE_2_1_4_MM,
    face: FACE,
    bezel: BEZEL,
    primary: DialChannel {
        input: DialInput::ClockTime,
        unit: DialUnit::Seconds,
    },
    secondary: None,
    extra: &[],
    input_range: None,
    rings: &[CLOCK_RING],
    legends: &[],
    pointers: &[
        hand(
            43_200.0,
            PointerStyle {
                shape: PointerShape::Sword,
                length: 0.5,
                width: 0.13,
                tail: 0.1,
                hub_radius: 0.08,
                paint: NEEDLE_PAINT,
            },
            1.0,
        ),
        hand(
            3_600.0,
            PointerStyle {
                shape: PointerShape::Lance,
                length: 0.8,
                width: 0.075,
                tail: 0.12,
                hub_radius: 0.06,
                paint: NEEDLE_PAINT,
            },
            1.5,
        ),
        hand(
            60.0,
            PointerStyle {
                shape: PointerShape::Lance,
                length: 0.88,
                width: 0.03,
                tail: 0.22,
                hub_radius: 0.04,
                paint: SECONDS_RED,
            },
            2.0,
        ),
    ],
    windows: &[],
    off_flag: Some(super::OffFlag {
        at: [0.0, 0.42],
        size: 0.12,
        ..OFF
    }),
    marks: &[],
    cards: &[],
    tapes: &[],
    overlays: &[],
    lettering: crate::spec::Lettering::DEFAULT,
    arc_legends: &[],
};

/// The arrows beside SLOW and FAST: the way the pointer turns when the
/// left engine runs slow or fast.
const SLOW_HEAD: &[[f32; 2]] = &[[-0.595, -0.121], [-0.611, -0.222], [-0.517, -0.188]];
const FAST_HEAD: &[[f32; 2]] = &[[0.595, -0.121], [0.517, -0.188], [0.611, -0.222]];

const fn arrow_arc(from_deg: f32, to_deg: f32) -> Mark<'static> {
    Mark {
        shape: Shape::Arc {
            center: [0.0, 0.0],
            radius: 0.6,
            from_deg,
            to_deg,
            width: 0.035,
        },
        paint: MARKING,
    }
}

/// The engine synchroscope: a pointer that stands still while the
/// engines turn together and turns, clockwise for FAST, while the left
/// engine outruns the right. Its angle is how far the left shaft leads
/// the right, so it turns once per revolution the left engine gains.
pub const SYNCHROSCOPE: AnalogDialSpec<'static> = AnalogDialSpec {
    case_diameter_mm: CASE_2_1_4_MM,
    face_radius_mm: FACE_2_1_4_MM,
    face: FACE,
    bezel: BEZEL,
    primary: DialChannel {
        input: DialInput::ShaftPhase {
            engine: 0,
            reference: 1,
        },
        unit: DialUnit::Degrees,
    },
    secondary: None,
    extra: &[],
    input_range: None,
    rings: &[],
    legends: &[
        Legend {
            text: "SLOW",
            at: [-0.38, -0.68],
            size: 0.14,
            paint: MARKING,
        },
        Legend {
            text: "FAST",
            at: [0.38, -0.68],
            size: 0.14,
            paint: MARKING,
        },
        Legend {
            text: "SYNCHROSCOPE",
            at: [0.0, 0.42],
            size: 0.075,
            paint: LEGEND,
        },
        Legend {
            text: "LEFT ENGINE",
            at: [0.0, 0.56],
            size: 0.075,
            paint: LEGEND,
        },
    ],
    pointers: &[PointerSpec {
        channel: Channel::Primary,
        law: ScaleLaw::Turns {
            reference_deg: 0.0,
            value_per_turn: 360.0,
        },
        pivot: [0.0, 0.0],
        style: PointerStyle {
            shape: PointerShape::Lance,
            length: 0.78,
            width: 0.09,
            tail: 0.78,
            hub_radius: 0.08,
            paint: SYMBOL,
        },
        stack_height_mm: 1.5,
        on_failure: FailurePose::Hide,
    }],
    windows: &[],
    off_flag: Some(super::OffFlag {
        at: [0.0, 0.2],
        size: 0.12,
        ..OFF
    }),
    marks: &[
        arrow_arc(-70.0, -30.0),
        Mark {
            shape: Shape::Polygon(SLOW_HEAD),
            paint: MARKING,
        },
        arrow_arc(30.0, 70.0),
        Mark {
            shape: Shape::Polygon(FAST_HEAD),
            paint: MARKING,
        },
    ],
    cards: &[],
    tapes: &[],
    overlays: &[],
    lettering: crate::spec::Lettering::DEFAULT,
    arc_legends: &[],
};
