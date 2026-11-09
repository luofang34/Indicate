//! Airspeed indicators: the six-pack's 40-200 knot dial with its
//! certification arcs, and the fighter's nonlinear 0-700 mph dial.

use super::{BEZEL, CASE_3_1_8_MM, FACE, FACE_3_1_8_MM, LEGEND, MARKING, NEEDLE, OFF, park};
use crate::spec::{
    AnalogDialSpec, Channel, DialChannel, DialInput, DialUnit, Legend, MarkArc, MarkColour,
    Numeral, PointerSpec, Radial, ScaleLaw, ScalePoint, ScaleRing, TickSet,
};

const KT_POINTS: [ScalePoint; 9] = [
    ScalePoint {
        value: 0.0,
        angle_deg: 0.0,
    },
    ScalePoint {
        value: 40.0,
        angle_deg: 30.0,
    },
    ScalePoint {
        value: 60.0,
        angle_deg: 75.0,
    },
    ScalePoint {
        value: 80.0,
        angle_deg: 125.0,
    },
    ScalePoint {
        value: 100.0,
        angle_deg: 170.0,
    },
    ScalePoint {
        value: 120.0,
        angle_deg: 210.0,
    },
    ScalePoint {
        value: 140.0,
        angle_deg: 250.0,
    },
    ScalePoint {
        value: 160.0,
        angle_deg: 285.0,
    },
    ScalePoint {
        value: 200.0,
        angle_deg: 345.0,
    },
];

const KT_LAW: ScaleLaw<'static> = ScaleLaw::Piecewise {
    reference_deg: 0.0,
    points: &KT_POINTS,
};

const KT_RING: ScaleRing<'static> = ScaleRing {
    center: [0.0, 0.0],
    radius: 1.0,
    law: KT_LAW,
    ticks: &[
        TickSet {
            from: 40.0,
            to: 200.0,
            every: 5.0,
            inner: 0.88,
            outer: 0.97,
            width: 0.012,
        },
        TickSet {
            from: 40.0,
            to: 200.0,
            every: 10.0,
            inner: 0.80,
            outer: 0.97,
            width: 0.022,
        },
    ],
    numerals: &[
        Numeral {
            value: 40.0,
            text: "40",
        },
        Numeral {
            value: 60.0,
            text: "60",
        },
        Numeral {
            value: 80.0,
            text: "80",
        },
        Numeral {
            value: 100.0,
            text: "100",
        },
        Numeral {
            value: 120.0,
            text: "120",
        },
        Numeral {
            value: 140.0,
            text: "140",
        },
        Numeral {
            value: 160.0,
            text: "160",
        },
        Numeral {
            value: 200.0,
            text: "200",
        },
    ],
    numeral_radius: 0.64,
    numeral_size: 0.12,
    arcs: &[
        MarkArc {
            from: 40.0,
            to: 85.0,
            colour: MarkColour::White,
            inner: 0.76,
            outer: 0.82,
        },
        MarkArc {
            from: 48.0,
            to: 129.0,
            colour: MarkColour::Green,
            inner: 0.86,
            outer: 0.95,
        },
        MarkArc {
            from: 129.0,
            to: 163.0,
            colour: MarkColour::Yellow,
            inner: 0.86,
            outer: 0.95,
        },
    ],
    radials: &[Radial {
        at: 163.0,
        colour: MarkColour::Red,
        inner: 0.78,
        outer: 0.98,
        width: 0.03,
    }],
    paint: MARKING,
};

/// The six-pack airspeed indicator, 40-200 knots, with its white,
/// green, and yellow arcs and the red never-exceed line at 163 knots.
pub const AIRSPEED_KT: AnalogDialSpec<'static> = AnalogDialSpec {
    case_diameter_mm: CASE_3_1_8_MM,
    face_radius_mm: FACE_3_1_8_MM,
    face: FACE,
    bezel: BEZEL,
    primary: DialChannel {
        input: DialInput::IndicatedAirspeed,
        unit: DialUnit::Knots,
    },
    secondary: None,
    extra: &[],
    input_range: None,
    rings: &[KT_RING],
    legends: &[
        Legend {
            text: "AIRSPEED",
            at: [0.0, -0.3],
            size: 0.08,
            paint: LEGEND,
        },
        Legend {
            text: "KNOTS",
            at: [0.0, 0.3],
            size: 0.09,
            paint: LEGEND,
        },
    ],
    pointers: &[PointerSpec {
        channel: Channel::Primary,
        law: KT_LAW,
        pivot: [0.0, 0.0],
        style: NEEDLE,
        stack_height_mm: 1.5,
        on_failure: park(0.0),
    }],
    windows: &[],
    off_flag: Some(OFF),
    marks: &[],
    cards: &[],
    tapes: &[],
    overlays: &[],
};

const MPH_POINTS: [ScalePoint; 7] = [
    ScalePoint {
        value: 0.0,
        angle_deg: 0.0,
    },
    ScalePoint {
        value: 100.0,
        angle_deg: 50.0,
    },
    ScalePoint {
        value: 200.0,
        angle_deg: 120.0,
    },
    ScalePoint {
        value: 300.0,
        angle_deg: 180.0,
    },
    ScalePoint {
        value: 400.0,
        angle_deg: 235.0,
    },
    ScalePoint {
        value: 500.0,
        angle_deg: 280.0,
    },
    ScalePoint {
        value: 700.0,
        angle_deg: 345.0,
    },
];

const MPH_LAW: ScaleLaw<'static> = ScaleLaw::Piecewise {
    reference_deg: 0.0,
    points: &MPH_POINTS,
};

const MPH_RING: ScaleRing<'static> = ScaleRing {
    center: [0.0, 0.0],
    radius: 1.0,
    law: MPH_LAW,
    ticks: &[
        TickSet {
            from: 0.0,
            to: 700.0,
            every: 10.0,
            inner: 0.89,
            outer: 0.97,
            width: 0.011,
        },
        TickSet {
            from: 0.0,
            to: 700.0,
            every: 50.0,
            inner: 0.80,
            outer: 0.97,
            width: 0.022,
        },
    ],
    numerals: &[
        Numeral {
            value: 100.0,
            text: "1",
        },
        Numeral {
            value: 200.0,
            text: "2",
        },
        Numeral {
            value: 300.0,
            text: "3",
        },
        Numeral {
            value: 400.0,
            text: "4",
        },
        Numeral {
            value: 500.0,
            text: "5",
        },
        Numeral {
            value: 600.0,
            text: "6",
        },
        Numeral {
            value: 700.0,
            text: "7",
        },
    ],
    numeral_radius: 0.66,
    numeral_size: 0.15,
    arcs: &[],
    radials: &[Radial {
        at: 420.0,
        colour: MarkColour::Red,
        inner: 0.78,
        outer: 0.98,
        width: 0.03,
    }],
    paint: MARKING,
};

/// The fighter's airspeed indicator, 0-700 mph on a scale that opens up
/// at low speed, with the red limit line at 420 mph. Its numerals are
/// hundreds of miles per hour, as the legend says. Zero carries no
/// numeral: it sits 15 degrees from 700, and the pair would read 70.
pub const AIRSPEED_MPH: AnalogDialSpec<'static> = AnalogDialSpec {
    rings: &[MPH_RING],
    primary: DialChannel {
        input: DialInput::IndicatedAirspeed,
        unit: DialUnit::MilesPerHour,
    },
    legends: &[
        Legend {
            text: "AIRSPEED",
            at: [0.0, -0.3],
            size: 0.08,
            paint: LEGEND,
        },
        Legend {
            text: "MPH X100",
            at: [0.0, 0.3],
            size: 0.08,
            paint: LEGEND,
        },
    ],
    pointers: &[PointerSpec {
        channel: Channel::Primary,
        law: MPH_LAW,
        pivot: [0.0, 0.0],
        style: NEEDLE,
        stack_height_mm: 1.5,
        on_failure: park(0.0),
    }],
    ..AIRSPEED_KT
};
