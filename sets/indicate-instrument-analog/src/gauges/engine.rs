//! Engine gauges: the fighter's manifold pressure gauge and the
//! six-pack's tachometer, both for engine 1.
//!
//! A twin's second gauge is the same spec with `engine: 1` in its
//! channel.

use super::{
    BEZEL, CASE_2_1_4_MM, CASE_3_1_8_MM, FACE, FACE_2_1_4_MM, FACE_3_1_8_MM, LEGEND, MARKING,
    NEEDLE, OFF, park,
};
use crate::spec::{
    AnalogDialSpec, Channel, DialChannel, DialInput, DialUnit, Legend, MarkArc, MarkColour,
    Numeral, PointerSpec, Radial, ScaleLaw, ScalePoint, ScaleRing, TickSet,
};

const MAP_POINTS: [ScalePoint; 2] = [
    ScalePoint {
        value: 10.0,
        angle_deg: 0.0,
    },
    ScalePoint {
        value: 75.0,
        angle_deg: 300.0,
    },
];

/// The pack's angles count from the 10 inHg stop, which sits at seven
/// o'clock so the 300° sweep is centred on twelve.
const MAP_LAW: ScaleLaw<'static> = ScaleLaw::Piecewise {
    reference_deg: -150.0,
    points: &MAP_POINTS,
};

const MAP_RING: ScaleRing<'static> = ScaleRing {
    center: [0.0, 0.0],
    radius: 1.0,
    law: MAP_LAW,
    ticks: &[
        TickSet {
            from: 10.0,
            to: 75.0,
            every: 1.0,
            inner: 0.89,
            outer: 0.97,
            width: 0.014,
        },
        TickSet {
            from: 10.0,
            to: 75.0,
            every: 5.0,
            inner: 0.80,
            outer: 0.97,
            width: 0.028,
        },
    ],
    numerals: &[
        Numeral {
            value: 10.0,
            text: "10",
        },
        Numeral {
            value: 20.0,
            text: "20",
        },
        Numeral {
            value: 30.0,
            text: "30",
        },
        Numeral {
            value: 40.0,
            text: "40",
        },
        Numeral {
            value: 50.0,
            text: "50",
        },
        Numeral {
            value: 60.0,
            text: "60",
        },
        Numeral {
            value: 70.0,
            text: "70",
        },
    ],
    numeral_radius: 0.64,
    numeral_size: 0.15,
    arcs: &[],
    radials: &[
        Radial {
            at: 44.7,
            colour: MarkColour::Green,
            inner: 0.78,
            outer: 0.98,
            width: 0.035,
        },
        Radial {
            at: 48.0,
            colour: MarkColour::Red,
            inner: 0.78,
            outer: 0.98,
            width: 0.035,
        },
    ],
    paint: MARKING,
};

/// The fighter's manifold pressure gauge for engine 1, 10-75 inHg, with
/// the green radial at 44.7 and the red limit at 48 inHg.
pub const MANIFOLD_PRESSURE: AnalogDialSpec<'static> = AnalogDialSpec {
    case_diameter_mm: CASE_2_1_4_MM,
    face_radius_mm: FACE_2_1_4_MM,
    face: FACE,
    bezel: BEZEL,
    primary: DialChannel {
        input: DialInput::ManifoldPressure { engine: 0 },
        unit: DialUnit::InchesOfMercury,
    },
    secondary: None,
    extra: &[],
    input_range: None,
    rings: &[MAP_RING],
    legends: &[
        Legend {
            text: "MANIFOLD",
            at: [0.0, -0.3],
            size: 0.09,
            paint: LEGEND,
        },
        Legend {
            text: "IN.HG",
            at: [0.0, 0.3],
            size: 0.1,
            paint: LEGEND,
        },
    ],
    pointers: &[PointerSpec {
        channel: Channel::Primary,
        law: MAP_LAW,
        pivot: [0.0, 0.0],
        style: NEEDLE,
        stack_height_mm: 1.2,
        on_failure: park(-150.0),
    }],
    windows: &[],
    off_flag: Some(OFF),
    marks: &[],
    cards: &[],
    tapes: &[],
    overlays: &[],
    lettering: crate::spec::Lettering::DEFAULT,
    arc_legends: &[],
};

const RPM_POINTS: [ScalePoint; 2] = [
    ScalePoint {
        value: 0.0,
        angle_deg: 0.0,
    },
    ScalePoint {
        value: 3500.0,
        angle_deg: 270.0,
    },
];

/// The pack's angles count from zero, which sits at seven-thirty so
/// the 270° sweep is centred on twelve.
const RPM_LAW: ScaleLaw<'static> = ScaleLaw::Piecewise {
    reference_deg: -135.0,
    points: &RPM_POINTS,
};

const RPM_RING: ScaleRing<'static> = ScaleRing {
    center: [0.0, 0.0],
    radius: 1.0,
    law: RPM_LAW,
    ticks: &[
        TickSet {
            from: 0.0,
            to: 3500.0,
            every: 100.0,
            inner: 0.89,
            outer: 0.97,
            width: 0.012,
        },
        TickSet {
            from: 0.0,
            to: 3500.0,
            every: 500.0,
            inner: 0.80,
            outer: 0.97,
            width: 0.025,
        },
    ],
    numerals: &[
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
        Numeral {
            value: 2500.0,
            text: "25",
        },
        Numeral {
            value: 3000.0,
            text: "30",
        },
        Numeral {
            value: 3500.0,
            text: "35",
        },
    ],
    numeral_radius: 0.66,
    numeral_size: 0.13,
    arcs: &[MarkArc {
        from: 2100.0,
        to: 2700.0,
        colour: MarkColour::Green,
        inner: 0.86,
        outer: 0.95,
    }],
    radials: &[Radial {
        at: 2700.0,
        colour: MarkColour::Red,
        inner: 0.78,
        outer: 0.98,
        width: 0.03,
    }],
    paint: MARKING,
};

/// The six-pack tachometer for engine 1, 0-3,500 rpm, with the green
/// arc from 2,100 to 2,700 and the red line at 2,700 rpm.
pub const TACHOMETER: AnalogDialSpec<'static> = AnalogDialSpec {
    case_diameter_mm: CASE_3_1_8_MM,
    face_radius_mm: FACE_3_1_8_MM,
    face: FACE,
    bezel: BEZEL,
    primary: DialChannel {
        input: DialInput::EngineRpm { engine: 0 },
        unit: DialUnit::RevolutionsPerMinute,
    },
    secondary: None,
    extra: &[],
    input_range: None,
    rings: &[RPM_RING],
    legends: &[
        Legend {
            text: "RPM",
            at: [0.0, -0.3],
            size: 0.1,
            paint: LEGEND,
        },
        Legend {
            text: "X100",
            at: [0.0, 0.3],
            size: 0.08,
            paint: LEGEND,
        },
    ],
    pointers: &[PointerSpec {
        channel: Channel::Primary,
        law: RPM_LAW,
        pivot: [0.0, 0.0],
        style: NEEDLE,
        stack_height_mm: 1.5,
        on_failure: park(-135.0),
    }],
    windows: &[],
    off_flag: Some(OFF),
    marks: &[],
    cards: &[],
    tapes: &[],
    overlays: &[],
    lettering: crate::spec::Lettering::DEFAULT,
    arc_legends: &[],
};
