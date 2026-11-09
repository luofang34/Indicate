//! The sensitive altimeter: three hands on one face and a Kollsman
//! window, 0-20,000 ft with a 28.10-31.00 inHg subscale.

use super::{
    BEZEL, CASE_3_1_8_MM, DRUM, FACE, FACE_3_1_8_MM, LEGEND, MARKING, NEEDLE, NEEDLE_PAINT, OFF,
    park,
};
use crate::spec::{
    AnalogDialSpec, Channel, DialChannel, DialInput, DialUnit, Legend, Numeral, PointerShape,
    PointerSpec, PointerStyle, ScaleLaw, ScaleRing, TickSet, WindowSpec,
};

/// The face is graduated for the hundreds hand: one turn per 1,000 ft.
const HUNDREDS: ScaleLaw<'static> = ScaleLaw::Turns {
    reference_deg: 0.0,
    value_per_turn: 1000.0,
};
const THOUSANDS: ScaleLaw<'static> = ScaleLaw::Turns {
    reference_deg: 0.0,
    value_per_turn: 10_000.0,
};
const TEN_THOUSANDS: ScaleLaw<'static> = ScaleLaw::Turns {
    reference_deg: 0.0,
    value_per_turn: 100_000.0,
};

const RING: ScaleRing<'static> = ScaleRing {
    center: [0.0, 0.0],
    radius: 1.0,
    law: HUNDREDS,
    ticks: &[
        TickSet {
            from: 0.0,
            to: 980.0,
            every: 20.0,
            inner: 0.89,
            outer: 0.97,
            width: 0.012,
        },
        TickSet {
            from: 0.0,
            to: 900.0,
            every: 100.0,
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
        Numeral {
            value: 800.0,
            text: "8",
        },
        Numeral {
            value: 900.0,
            text: "9",
        },
    ],
    numeral_radius: 0.68,
    numeral_size: 0.16,
    arcs: &[],
    radials: &[],
    paint: MARKING,
};

/// The broad thousands hand.
const SWORD: PointerStyle = PointerStyle {
    shape: PointerShape::Sword,
    length: 0.55,
    width: 0.12,
    tail: 0.12,
    hub_radius: 0.0,
    paint: NEEDLE_PAINT,
};

/// The ten-thousands pointer: a triangle near the rim on a thin shaft.
const TRIANGLE: PointerStyle = PointerStyle {
    shape: PointerShape::Triangle,
    length: 0.95,
    width: 0.05,
    tail: 0.0,
    hub_radius: 0.0,
    paint: NEEDLE_PAINT,
};

/// The sensitive altimeter of the six-pack. The hands stack lowest to
/// highest as ten-thousands, thousands, hundreds, as the instrument's
/// arbors do. The Kollsman window sits at three o'clock and reads the
/// applied setting to a hundredth of an inch.
pub const ALTIMETER: AnalogDialSpec<'static> = AnalogDialSpec {
    case_diameter_mm: CASE_3_1_8_MM,
    face_radius_mm: FACE_3_1_8_MM,
    face: FACE,
    bezel: BEZEL,
    primary: DialChannel {
        input: DialInput::BaroAltitude,
        unit: DialUnit::Feet,
    },
    secondary: Some(DialChannel {
        input: DialInput::BaroSetting,
        unit: DialUnit::InchesOfMercury,
    }),
    extra: &[],
    input_range: Some([-1000.0, 20_000.0]),
    rings: &[RING],
    legends: &[
        Legend {
            text: "ALT",
            at: [0.0, -0.32],
            size: 0.09,
            paint: LEGEND,
        },
        Legend {
            text: "100 FEET",
            at: [0.0, 0.3],
            size: 0.07,
            paint: LEGEND,
        },
    ],
    pointers: &[
        PointerSpec {
            channel: Channel::Primary,
            law: TEN_THOUSANDS,
            pivot: [0.0, 0.0],
            style: TRIANGLE,
            stack_height_mm: 0.6,
            on_failure: park(0.0),
        },
        PointerSpec {
            channel: Channel::Primary,
            law: THOUSANDS,
            pivot: [0.0, 0.0],
            style: SWORD,
            stack_height_mm: 1.2,
            on_failure: park(0.0),
        },
        PointerSpec {
            channel: Channel::Primary,
            law: HUNDREDS,
            pivot: [0.0, 0.0],
            style: NEEDLE,
            stack_height_mm: 1.8,
            on_failure: park(0.0),
        },
    ],
    windows: &[WindowSpec {
        channel: Channel::Secondary,
        aperture: [0.3, -0.09, 0.42, 0.18],
        wheels: 4,
        value_scale: 100.0,
        range: [28.1, 31.0],
        separator: Some((2, ".")),
        digit_size: 0.1,
        paint: MARKING,
        drum: DRUM,
    }],
    off_flag: Some(OFF),
};
