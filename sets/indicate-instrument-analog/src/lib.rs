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
mod spec;

pub use build::{FACE_LAYERS, PhysicalBuild, PointerBuild, WindowBuild, physical_build};
pub use descriptors::{
    AIRSPEED_KT_DESCRIPTOR, AIRSPEED_MPH_DESCRIPTOR, ALTIMETER_DESCRIPTOR, ANALOG_PANELS,
    ANALOG_SET, DIAL_FRAME, MANIFOLD_PRESSURE_DESCRIPTOR, TACHOMETER_DESCRIPTOR,
    VERTICAL_SPEED_DESCRIPTOR, baro_cruise, baro_high,
};
pub use face::Placement;
pub use gauges::{
    AIRSPEED_KT, AIRSPEED_MPH, ALTIMETER, MANIFOLD_PRESSURE, TACHOMETER, VERTICAL_SPEED,
};
pub use law::wheel_positions;
pub use outline::{PointerOutline, pointer_outline};
pub use pose::{DialPose, DialReading, PointerPose, WindowPose, pose, reading};
pub use scene::{draw_dial, draw_face_scene};
pub use spec::{
    AnalogDialSpec, Channel, DialChannel, DialInput, DialUnit, FailurePose, Legend, Lighting,
    MAX_OUTLINE_POINTS, MAX_POINTERS, MAX_WHEELS, MAX_WINDOWS, MarkArc, MarkColour, Numeral,
    OffFlag, Paint, PointerShape, PointerSpec, PointerStyle, Radial, ScaleLaw, ScalePoint,
    ScaleRing, SpecError, TickSet, WindowSpec,
};

#[cfg(test)]
mod tests;
