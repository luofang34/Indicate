//! The vertical speed indicator, ±2,000 ft/min, zero at nine o'clock.

use super::{BEZEL, CASE_3_1_8_MM, FACE, FACE_3_1_8_MM, LEGEND, MARKING, NEEDLE, OFF, park};
use crate::spec::{
    AnalogDialSpec, Channel, DialChannel, DialInput, DialUnit, Legend, Numeral, PointerSpec,
    ScaleLaw, ScalePoint, ScaleRing, TickSet,
};

const POINTS: [ScalePoint; 7] = [
    ScalePoint {
        value: -2000.0,
        angle_deg: -170.0,
    },
    ScalePoint {
        value: -1000.0,
        angle_deg: -90.0,
    },
    ScalePoint {
        value: -500.0,
        angle_deg: -45.0,
    },
    ScalePoint {
        value: 0.0,
        angle_deg: 0.0,
    },
    ScalePoint {
        value: 500.0,
        angle_deg: 45.0,
    },
    ScalePoint {
        value: 1000.0,
        angle_deg: 90.0,
    },
    ScalePoint {
        value: 2000.0,
        angle_deg: 170.0,
    },
];

/// The pack's angles count from the zero mark, which a vertical speed
/// indicator prints at nine o'clock.
const LAW: ScaleLaw<'static> = ScaleLaw::Piecewise {
    reference_deg: -90.0,
    points: &POINTS,
};

const RING: ScaleRing<'static> = ScaleRing {
    center: [0.0, 0.0],
    radius: 1.0,
    law: LAW,
    ticks: &[
        TickSet {
            from: -2000.0,
            to: 2000.0,
            every: 100.0,
            inner: 0.89,
            outer: 0.97,
            width: 0.012,
        },
        TickSet {
            from: -2000.0,
            to: 2000.0,
            every: 500.0,
            inner: 0.80,
            outer: 0.97,
            width: 0.025,
        },
    ],
    numerals: &[
        Numeral {
            value: -2000.0,
            text: "20",
        },
        Numeral {
            value: -1500.0,
            text: "15",
        },
        Numeral {
            value: -1000.0,
            text: "10",
        },
        Numeral {
            value: -500.0,
            text: "5",
        },
        Numeral {
            value: 0.0,
            text: "0",
        },
        Numeral {
            value: 500.0,
            text: "5",
        },
        Numeral {
            value: 1000.0,
            text: "10",
        },
        Numeral {
            value: 1500.0,
            text: "15",
        },
        Numeral {
            value: 2000.0,
            text: "20",
        },
    ],
    numeral_radius: 0.66,
    numeral_size: 0.13,
    arcs: &[],
    radials: &[],
    paint: MARKING,
};

/// The six-pack vertical speed indicator. Numerals are hundreds of feet
/// per minute; the needle reads the instrument's own lagged rate, not
/// the kinematic vertical velocity.
pub const VERTICAL_SPEED: AnalogDialSpec<'static> = AnalogDialSpec {
    case_diameter_mm: CASE_3_1_8_MM,
    face_radius_mm: FACE_3_1_8_MM,
    face: FACE,
    bezel: BEZEL,
    primary: DialChannel {
        input: DialInput::IndicatedVerticalSpeed,
        unit: DialUnit::FeetPerMinute,
    },
    secondary: None,
    extra: &[],
    input_range: None,
    rings: &[RING],
    legends: &[
        Legend {
            text: "UP",
            at: [-0.32, -0.34],
            size: 0.08,
            paint: LEGEND,
        },
        Legend {
            text: "DN",
            at: [-0.32, 0.34],
            size: 0.08,
            paint: LEGEND,
        },
        Legend {
            text: "VERTICAL SPEED",
            at: [0.0, -0.16],
            size: 0.055,
            paint: LEGEND,
        },
        Legend {
            text: "100 FT PER MIN",
            at: [0.0, 0.16],
            size: 0.055,
            paint: LEGEND,
        },
    ],
    pointers: &[PointerSpec {
        channel: Channel::Primary,
        law: LAW,
        pivot: [0.0, 0.0],
        style: NEEDLE,
        stack_height_mm: 1.5,
        on_failure: park(-90.0),
    }],
    windows: &[],
    off_flag: Some(OFF),
    marks: &[],
    cards: &[],
    tapes: &[],
    overlays: &[],
};
