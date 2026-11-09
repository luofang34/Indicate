//! The gyro flight instruments of a 1943 fighter panel: the gyro
//! horizon and the turn and bank indicator.
//!
//! Both follow the AN instruments of the period (Army types C-5 to E-1
//! for the horizon, A-5 to C-1 for the turn indicator; TM 1-413
//! §XXVII and §XXIX): a black face, radium markings, a horizon bar and
//! bank index that turn against a fixed scale and miniature aircraft,
//! and a needle with a ball in a curved glass tube.

use indicate_instrument_scene::Rgba8;

use super::{BEZEL, CASE_3_1_8_MM, FACE, FACE_3_1_8_MM, MARKING, NEEDLE_PAINT, OFF, SYMBOL};
use crate::spec::{
    AnalogDialSpec, CardSpec, CardTurn, Channel, DialChannel, DialInput, DialUnit, FailurePose,
    Mark, Numeral, OverlaySpec, Paint, PointerShape, PointerSpec, PointerStyle, ScaleLaw,
    ScalePoint, ScaleRing, Shape, TickSet, Travel,
};

/// The printed bank scale: a value in degrees of bank sits at that many
/// degrees from twelve o'clock.
const BANK_SCALE: ScaleLaw<'static> = ScaleLaw::Piecewise {
    reference_deg: 0.0,
    points: &[
        ScalePoint {
            value: -90.0,
            angle_deg: -90.0,
        },
        ScalePoint {
            value: 90.0,
            angle_deg: 90.0,
        },
    ],
};

/// The gyro holds the horizon level, so the bar and the bank index turn
/// against the case by the bank angle the other way: right bank turns
/// them counter-clockwise, raising the bar's right end.
const AGAINST_BANK: ScaleLaw<'static> = ScaleLaw::Piecewise {
    reference_deg: 0.0,
    points: &[
        ScalePoint {
            value: -180.0,
            angle_deg: 180.0,
        },
        ScalePoint {
            value: 180.0,
            angle_deg: -180.0,
        },
    ],
};

const BANK_RING: ScaleRing<'static> = ScaleRing {
    center: [0.0, 0.0],
    radius: 0.93,
    law: BANK_SCALE,
    ticks: &[
        TickSet {
            from: -60.0,
            to: 60.0,
            every: 30.0,
            inner: 0.86,
            outer: 1.0,
            width: 0.04,
        },
        TickSet {
            from: -20.0,
            to: 20.0,
            every: 10.0,
            inner: 0.91,
            outer: 1.0,
            width: 0.025,
        },
    ],
    numerals: &[
        Numeral {
            value: -60.0,
            text: "6",
        },
        Numeral {
            value: -30.0,
            text: "3",
        },
        Numeral {
            value: 30.0,
            text: "3",
        },
        Numeral {
            value: 60.0,
            text: "6",
        },
    ],
    numeral_radius: 0.62,
    numeral_size: 0.13,
    arcs: &[],
    radials: &[],
    paint: MARKING,
};

/// The horizon bar: a white bar across the face.
const HORIZON_BAR: &[Mark<'static>] = &[Mark {
    shape: Shape::Rect([-0.62, -0.022, 1.24, 0.044]),
    paint: NEEDLE_PAINT,
}];

/// The miniature aircraft, fixed to the case in front of the bar:
/// wings, a centre dot, and a fin.
const MINIATURE_AIRCRAFT: &[Mark<'static>] = &[
    Mark {
        shape: Shape::Line {
            from: [-0.44, 0.0],
            to: [-0.13, 0.0],
            width: 0.045,
        },
        paint: SYMBOL,
    },
    Mark {
        shape: Shape::Line {
            from: [0.13, 0.0],
            to: [0.44, 0.0],
            width: 0.045,
        },
        paint: SYMBOL,
    },
    Mark {
        shape: Shape::Disc {
            center: [0.0, 0.0],
            radius: 0.05,
        },
        paint: SYMBOL,
    },
    Mark {
        shape: Shape::Line {
            from: [0.0, -0.04],
            to: [0.0, -0.13],
            width: 0.03,
        },
        paint: SYMBOL,
    },
];

/// The gyro horizon (flight indicator). The bar turns against the bank
/// and slides down as the nose rises, 0.012 face units a degree, to the
/// mechanism's stops near 35 degrees; a failed attitude source leaves
/// the bar and the index out.
pub const GYRO_HORIZON: AnalogDialSpec<'static> = AnalogDialSpec {
    case_diameter_mm: CASE_3_1_8_MM,
    face_radius_mm: FACE_3_1_8_MM,
    face: FACE,
    bezel: BEZEL,
    primary: DialChannel {
        input: DialInput::Roll,
        unit: DialUnit::Degrees,
    },
    secondary: Some(DialChannel {
        input: DialInput::Pitch,
        unit: DialUnit::Degrees,
    }),
    extra: &[],
    input_range: None,
    rings: &[BANK_RING],
    legends: &[],
    pointers: &[PointerSpec {
        channel: Channel::Primary,
        law: AGAINST_BANK,
        pivot: [0.0, 0.0],
        style: PointerStyle {
            shape: PointerShape::Index,
            length: 0.79,
            width: 0.1,
            tail: 0.0,
            hub_radius: 0.0,
            paint: SYMBOL,
        },
        stack_height_mm: 1.0,
        on_failure: FailurePose::Hide,
    }],
    windows: &[],
    off_flag: Some(OFF),
    marks: &[
        Mark {
            shape: Shape::Line {
                from: [-0.95, 0.0],
                to: [-0.76, 0.0],
                width: 0.04,
            },
            paint: MARKING,
        },
        Mark {
            shape: Shape::Line {
                from: [0.76, 0.0],
                to: [0.95, 0.0],
                width: 0.04,
            },
            paint: MARKING,
        },
    ],
    cards: &[CardSpec {
        shapes: HORIZON_BAR,
        pivot: [0.0, 0.0],
        turn: Some(CardTurn {
            channel: Channel::Primary,
            law: AGAINST_BANK,
        }),
        travel: Some(Travel {
            channel: Channel::Secondary,
            per_unit: 0.012,
            limit: [-0.42, 0.42],
        }),
        stack_height_mm: 0.5,
        on_failure: FailurePose::Hide,
    }],
    tapes: &[],
    overlays: &[OverlaySpec {
        shapes: MINIATURE_AIRCRAFT,
        stack_height_mm: 2.5,
    }],
};

/// The needle's pivot, below the tube: the needle rises through it.
const NEEDLE_PIVOT: [f32; 2] = [0.0, 0.55];
/// The tube's centre of curvature, far above the face, and its radius:
/// the ball rests at the tube's lowest point, 0.18 below the centre.
const TUBE_CENTRE: [f32; 2] = [0.0, -1.82];
const TUBE_RADIUS: f32 = 2.0;

/// Five degrees of needle a degree per second of yaw, so a standard
/// rate turn (3 °/s) lays the needle on a doghouse mark.
const TURN_LAW: ScaleLaw<'static> = ScaleLaw::Piecewise {
    reference_deg: 0.0,
    points: &[
        ScalePoint {
            value: -8.0,
            angle_deg: -40.0,
        },
        ScalePoint {
            value: 8.0,
            angle_deg: 40.0,
        },
    ],
};

/// The ball rests where the tube is square to the apparent gravity:
/// `atan(f / g)` round the tube, clockwise for force to the right
/// (which sends the ball left), to its stops at 18 degrees.
/// `g` = 9.80665 m/s², so the stop is at 3.186 m/s².
const BALL_LAW: ScaleLaw<'static> = ScaleLaw::Piecewise {
    reference_deg: 180.0,
    points: &[
        ScalePoint {
            value: -3.186,
            angle_deg: -18.0,
        },
        ScalePoint {
            value: -1.6,
            angle_deg: -9.267,
        },
        ScalePoint {
            value: 0.0,
            angle_deg: 0.0,
        },
        ScalePoint {
            value: 1.6,
            angle_deg: 9.267,
        },
        ScalePoint {
            value: 3.186,
            angle_deg: 18.0,
        },
    ],
};

/// The doghouse marks at standard rate either side of the needle's
/// upright position, 1.2 from its pivot.
const DOGHOUSE_LEFT: &[[f32; 2]] = &[
    [-0.311, -0.68],
    [-0.251, -0.63],
    [-0.251, -0.55],
    [-0.371, -0.55],
    [-0.371, -0.63],
];
const DOGHOUSE_RIGHT: &[[f32; 2]] = &[
    [0.311, -0.68],
    [0.371, -0.63],
    [0.371, -0.55],
    [0.251, -0.55],
    [0.251, -0.63],
];

/// The liquid in the tube, behind the ball.
const TUBE_LIQUID: Paint = Paint::plain(Rgba8::rgb(196, 190, 170));
/// The tube's glass, in front of the ball and the needle: translucent,
/// so both show through it as they do through the real tube.
const TUBE_GLASS: Paint = Paint::plain(Rgba8::rgba(205, 200, 185, 90));
/// The two reference wires on the tube's glass.
const WIRE: Paint = Paint::plain(Rgba8::rgb(20, 20, 20));

/// The tube's bore along its centre of curvature.
const fn tube(paint: Paint) -> Mark<'static> {
    Mark {
        shape: Shape::Arc {
            center: TUBE_CENTRE,
            radius: TUBE_RADIUS,
            from_deg: 159.5,
            to_deg: 200.5,
            width: 0.2,
        },
        paint,
    }
}

const TUBE: &[Mark<'static>] = &[
    tube(TUBE_GLASS),
    Mark {
        shape: Shape::Line {
            from: [-0.1, 0.07],
            to: [-0.1, 0.29],
            width: 0.014,
        },
        paint: WIRE,
    },
    Mark {
        shape: Shape::Line {
            from: [0.1, 0.07],
            to: [0.1, 0.29],
            width: 0.014,
        },
        paint: WIRE,
    },
];

/// The turn and bank indicator. The needle reads the body yaw rate a
/// rate gyro senses; the ball reads the lateral specific force. A needle
/// with no rate rests upright, as a run-down gyro's does, and raises the
/// flag; a ball with no force is left out.
pub const TURN_AND_BANK: AnalogDialSpec<'static> = AnalogDialSpec {
    case_diameter_mm: CASE_3_1_8_MM,
    face_radius_mm: FACE_3_1_8_MM,
    face: FACE,
    bezel: BEZEL,
    primary: DialChannel {
        input: DialInput::YawRate,
        unit: DialUnit::DegreesPerSecond,
    },
    secondary: Some(DialChannel {
        input: DialInput::LateralForce,
        unit: DialUnit::MetresPerSecondSquared,
    }),
    extra: &[],
    input_range: None,
    rings: &[],
    legends: &[],
    pointers: &[
        PointerSpec {
            channel: Channel::Secondary,
            law: BALL_LAW,
            pivot: TUBE_CENTRE,
            style: PointerStyle {
                shape: PointerShape::Ball,
                length: TUBE_RADIUS,
                width: 0.17,
                tail: 0.0,
                hub_radius: 0.0,
                paint: Paint::plain(Rgba8::rgb(12, 12, 12)),
            },
            stack_height_mm: 0.5,
            on_failure: FailurePose::Hide,
        },
        PointerSpec {
            channel: Channel::Primary,
            law: TURN_LAW,
            pivot: NEEDLE_PIVOT,
            style: PointerStyle {
                shape: PointerShape::Lance,
                length: 1.22,
                width: 0.11,
                tail: 0.0,
                hub_radius: 0.0,
                paint: SYMBOL,
            },
            stack_height_mm: 0.3,
            on_failure: FailurePose::Park { angle_deg: 0.0 },
        },
    ],
    windows: &[],
    off_flag: Some(super::OffFlag {
        at: [0.0, 0.72],
        ..OFF
    }),
    marks: &[
        Mark {
            shape: Shape::Polygon(DOGHOUSE_LEFT),
            paint: MARKING,
        },
        Mark {
            shape: Shape::Polygon(DOGHOUSE_RIGHT),
            paint: MARKING,
        },
        tube(TUBE_LIQUID),
    ],
    cards: &[],
    tapes: &[],
    overlays: &[OverlaySpec {
        shapes: TUBE,
        stack_height_mm: 1.0,
    }],
};
