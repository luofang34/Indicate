//! Heading cards read through a window: the drum-window directional
//! gyro and the panel magnetic compass.
//!
//! The P-38G-13's directional gyro is the square-cased drum-window type
//! (Army types A-3 to C-1, TM 1-413 §XXVIII), and its compass is the
//! boxed pilot's compass (type B-17). Both show a heading card behind a
//! lubber line. The gyro's card reads with values rising to the right;
//! the compass's floating card is read from behind its north, so its
//! values rise to the left.

use super::{BEZEL, CASE_3_1_8_MM, DRUM, FACE, FACE_3_1_8_MM, LEGEND, MARKING, OFF, SYMBOL};
use crate::spec::{
    AnalogDialSpec, Channel, DialChannel, DialInput, DialUnit, Legend, Mark, Numeral, OverlaySpec,
    Shape, TapeSense, TapeSpec, TapeTicks,
};

/// A heading card's numerals: the cardinal letters and tens of degrees
/// every thirty.
const CARD_NUMERALS: &[Numeral<'static>] = &[
    Numeral {
        value: 0.0,
        text: "N",
    },
    Numeral {
        value: 30.0,
        text: "3",
    },
    Numeral {
        value: 60.0,
        text: "6",
    },
    Numeral {
        value: 90.0,
        text: "E",
    },
    Numeral {
        value: 120.0,
        text: "12",
    },
    Numeral {
        value: 150.0,
        text: "15",
    },
    Numeral {
        value: 180.0,
        text: "S",
    },
    Numeral {
        value: 210.0,
        text: "21",
    },
    Numeral {
        value: 240.0,
        text: "24",
    },
    Numeral {
        value: 270.0,
        text: "W",
    },
    Numeral {
        value: 300.0,
        text: "30",
    },
    Numeral {
        value: 330.0,
        text: "33",
    },
];

/// Five-degree graduations, longer every ten.
const CARD_TICKS: &[TapeTicks] = &[
    TapeTicks {
        every: 5.0,
        top: 0.68,
        bottom: 1.0,
        width: 0.018,
    },
    TapeTicks {
        every: 10.0,
        top: 0.56,
        bottom: 1.0,
        width: 0.026,
    },
];

/// A heading card in a window `aperture` wide enough for `span`
/// degrees.
const fn heading_card(
    aperture: [f32; 4],
    span: f32,
    sense: TapeSense,
    numeral_size: f32,
) -> TapeSpec<'static> {
    TapeSpec {
        channel: Channel::Primary,
        aperture,
        period: 360.0,
        pitch: aperture[2] / span,
        sense,
        ticks: CARD_TICKS,
        numerals: CARD_NUMERALS,
        numeral_at: 0.32,
        numeral_size,
        paint: MARKING,
        drum: DRUM,
    }
}

/// A lubber line across a window whose top and bottom are `top` and
/// `bottom`.
const fn lubber(top: f32, bottom: f32) -> Mark<'static> {
    Mark {
        shape: Shape::Line {
            from: [0.0, top],
            to: [0.0, bottom],
            width: 0.028,
        },
        paint: SYMBOL,
    }
}

const DG_WINDOW: [f32; 4] = [-0.78, -0.52, 1.56, 0.56];
const DG_LUBBER: &[Mark<'static>] = &[lubber(-0.58, 0.1)];

/// The drum-window directional gyro: about 75 degrees of card in view.
/// It shows the heading in the reference the source declares, as a gyro
/// shows whatever it was last set to; precession is the source's
/// physics.
pub const DIRECTIONAL_GYRO: AnalogDialSpec<'static> = AnalogDialSpec {
    case_diameter_mm: CASE_3_1_8_MM,
    face_radius_mm: FACE_3_1_8_MM,
    face: FACE,
    bezel: BEZEL,
    primary: DialChannel {
        input: DialInput::GyroHeading,
        unit: DialUnit::Degrees,
    },
    secondary: None,
    extra: &[],
    input_range: None,
    rings: &[],
    legends: &[Legend {
        text: "DIRECTIONAL GYRO",
        at: [0.0, 0.3],
        size: 0.065,
        paint: LEGEND,
    }],
    pointers: &[],
    windows: &[],
    off_flag: Some(OFF),
    marks: &[],
    cards: &[],
    tapes: &[heading_card(
        DG_WINDOW,
        75.0,
        TapeSense::RightIncreasing,
        0.16,
    )],
    overlays: &[OverlaySpec {
        shapes: DG_LUBBER,
        stack_height_mm: 2.0,
    }],
};

const COMPASS_WINDOW: [f32; 4] = [-0.72, -0.27, 1.44, 0.54];
const COMPASS_LUBBER: &[Mark<'static>] = &[lubber(-0.33, 0.33)];

/// The panel magnetic compass: about 60 degrees of floating card in
/// view, reading magnetic heading. Swing and turning error are the
/// source's physics.
pub const MAGNETIC_COMPASS: AnalogDialSpec<'static> = AnalogDialSpec {
    case_diameter_mm: CASE_3_1_8_MM,
    face_radius_mm: FACE_3_1_8_MM,
    face: FACE,
    bezel: BEZEL,
    primary: DialChannel {
        input: DialInput::MagneticHeading,
        unit: DialUnit::Degrees,
    },
    secondary: None,
    extra: &[],
    input_range: None,
    rings: &[],
    legends: &[],
    pointers: &[],
    windows: &[],
    off_flag: Some(super::OffFlag {
        at: [0.0, 0.52],
        ..OFF
    }),
    marks: &[],
    cards: &[],
    tapes: &[heading_card(
        COMPASS_WINDOW,
        60.0,
        TapeSense::LeftIncreasing,
        0.15,
    )],
    overlays: &[OverlaySpec {
        shapes: COMPASS_LUBBER,
        stack_height_mm: 2.0,
    }],
};
