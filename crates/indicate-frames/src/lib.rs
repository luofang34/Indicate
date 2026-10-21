//! Physical frame identities, timestamps, and checked spatial transformations.
//!
//! Every transform identifies its source frame, destination frame, and clock instance.
//! Composition requires matching physical frames and timestamps.
//! [`Attitude`] identifies the observed object separately from its reference frame.
//! Transformations preserve producer metadata and use the shared [`Quat`] rotation kernel.
//! This crate is `no_std`, allocation-free, and performs no clock synchronization or state propagation.

#![no_std]

#[cfg(test)]
extern crate std;

mod error;
mod frame;
mod ned;
mod rotation;
mod tagged;
mod time;
mod transform;

pub use error::FrameError;
pub use frame::{FrameId, FrameKind};
pub use ned::ned_attitude;
pub use rotation::{NotARotation, Quat};
pub use tagged::{
    Acceleration, AngularVelocity, Attitude, Pose, Position, Tagged, Twist, Velocity,
    relative_attitude, transform_attitude, transform_position, transform_vector,
};
pub use time::{ClockDomain, ClockId, TimeScale, Timestamp};
pub use transform::{FrameTransform, ROTATION_NORM_TOLERANCE};

#[cfg(test)]
mod fixtures;
