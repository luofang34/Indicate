//! Round analog instruments: airspeed, sensitive altimeter, vertical
//! speed, manifold pressure, tachometer, and any other dial an
//! [`AnalogDialSpec`] describes.
//!
//! One description serves two backends. A 2D backend draws the scene
//! [`draw_dial`] emits, like any panel. A 3D backend builds a physical
//! gauge from [`physical_build`]: it rasterises the face once from
//! [`draw_face_scene`], extrudes each pointer outline at its stack
//! height, and per frame applies the angles and drum offsets [`pose`]
//! computes, with no scene drawn or decoded.
//!
//! Needles show the indicated value the state carries. Mechanism lag
//! and instrument error are the source's physics, so nothing here
//! filters a value.

#![no_std]

#[cfg(test)]
extern crate std;

mod build;
mod descriptors;
mod face;
mod gauges;
mod law;
mod moving;
mod outline;
mod pose;
mod scene;
mod shape;
mod spec;

pub use build::{
    CardBuild, FACE_LAYERS, PhysicalBuild, PointerBuild, TapeBuild, WindowBuild, physical_build,
};
pub use descriptors::{
    AIRSPEED_KT_DESCRIPTOR, AIRSPEED_MPH_DESCRIPTOR, ALTIMETER_DESCRIPTOR, ANALOG_PANELS,
    ANALOG_SET, CLOCK_DESCRIPTOR, DIAL_FRAME, DIRECTIONAL_GYRO_DESCRIPTOR, GYRO_HORIZON_DESCRIPTOR,
    MAGNETIC_COMPASS_DESCRIPTOR, MANIFOLD_PRESSURE_DESCRIPTOR, SYNCHROSCOPE_DESCRIPTOR,
    TACHOMETER_DESCRIPTOR, TURN_AND_BANK_DESCRIPTOR, VERTICAL_SPEED_DESCRIPTOR, baro_cruise,
    baro_high, magnetic_cruise,
};
pub use face::Placement;
pub use gauges::{
    AIRSPEED_KT, AIRSPEED_MPH, ALTIMETER, CLOCK, DIRECTIONAL_GYRO, GYRO_HORIZON, MAGNETIC_COMPASS,
    MANIFOLD_PRESSURE, SYNCHROSCOPE, TACHOMETER, TURN_AND_BANK, VERTICAL_SPEED,
};
pub use law::wheel_positions;
pub use outline::{PointerOutline, pointer_outline};
pub use pose::{CardPose, DialPose, DialReading, PointerPose, TapePose, WindowPose, pose, reading};
pub use scene::{draw_dial, draw_face_scene};
pub use shape::MAX_SHAPE_POINTS;
pub use spec::{
    AnalogDialSpec, ArcLegend, CardSpec, CardTurn, Channel, DialChannel, DialInput, DialUnit,
    FailurePose, Legend, Lettering, Lighting, MAX_CARDS, MAX_EXTRA_CHANNELS, MAX_OUTLINE_POINTS,
    MAX_POINTERS, MAX_TAPES, MAX_WHEELS, MAX_WINDOWS, Mark, MarkArc, MarkColour, Numeral, OffFlag,
    OverlaySpec, Paint, PointerShape, PointerSpec, PointerStyle, Radial, ScaleLaw, ScalePoint,
    ScaleRing, Shape, SpecError, TapeSense, TapeSpec, TapeTicks, TickSet, Travel, WindowSpec,
};

#[cfg(test)]
mod tests;
