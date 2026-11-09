//! The built-in gauges, as `static` [`AnalogDialSpec`] data.
//!
//! Their scales and markings are the simulator's instrument packs: the
//! general-aviation six-pack (`ga-analog-six-pack`) and the 1943 USAAF
//! fighter panel (`usaaf-1943-fighter`). What a pack does not state —
//! graduation spacing, numerals, legends, pointer shapes, where the
//! scale's zero sits on the face — follows the instruments those packs
//! name.

mod airspeed;
mod altimeter;
mod clock;
mod engine;
mod flight;
mod heading;
mod vertical_speed;

pub use airspeed::{AIRSPEED_KT, AIRSPEED_MPH};
pub use altimeter::ALTIMETER;
pub use clock::{CLOCK, SYNCHROSCOPE};
pub use engine::{MANIFOLD_PRESSURE, TACHOMETER};
pub use flight::{GYRO_HORIZON, TURN_AND_BANK};
pub use heading::{DIRECTIONAL_GYRO, MAGNETIC_COMPASS};
pub use vertical_speed::VERTICAL_SPEED;

use indicate_instrument_scene::Rgba8;

use crate::spec::{FailurePose, OffFlag, Paint, PointerShape, PointerStyle};

/// The 3-1/8 in case of primary flight instruments, millimetres.
const CASE_3_1_8_MM: f32 = 79.375;
/// Face radius inside a 3-1/8 in case, millimetres.
const FACE_3_1_8_MM: f32 = 35.5;
/// The 2-1/4 in case of engine instruments, millimetres.
const CASE_2_1_4_MM: f32 = 57.15;
/// Face radius inside a 2-1/4 in case, millimetres.
const FACE_2_1_4_MM: f32 = 25.0;

/// A matte black face.
const FACE: Paint = Paint::plain(Rgba8::rgb(16, 16, 16));
/// The bezel ring.
const BEZEL: Rgba8 = Rgba8::rgb(48, 48, 48);
/// The green-white glow of luminous paint.
const GLOW: Rgba8 = Rgba8::rgb(170, 255, 150);
/// White markings in luminous paint.
const MARKING: Paint = Paint::luminous(Rgba8::rgb(235, 235, 235), GLOW);
/// White legends, not luminous.
const LEGEND: Paint = Paint::plain(Rgba8::rgb(220, 220, 220));
/// A white needle in luminous paint.
const NEEDLE_PAINT: Paint = Paint::luminous(Rgba8::rgb(245, 245, 245), GLOW);
/// The drum behind a window.
const DRUM: Rgba8 = Rgba8::rgb(8, 8, 8);
/// The amber radium paint of a gyro instrument's symbols: a miniature
/// aircraft, a lubber line, a turn needle.
const SYMBOL: Paint = Paint::luminous(Rgba8::rgb(240, 175, 70), GLOW);

/// The long tapered needle of a single-pointer gauge.
const NEEDLE: PointerStyle = PointerStyle {
    shape: PointerShape::Lance,
    length: 0.9,
    width: 0.06,
    tail: 0.2,
    hub_radius: 0.07,
    paint: NEEDLE_PAINT,
};

/// Where a failed source shows itself: below the pivot, where no
/// pointer of a built-in gauge rests.
const OFF: OffFlag<'static> = OffFlag {
    text: "OFF",
    at: [0.0, 0.47],
    size: 0.08,
};

/// A needle parks on its scale's stop when its source fails.
const fn park(angle_deg: f32) -> FailurePose {
    FailurePose::Park { angle_deg }
}
