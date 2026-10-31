//! Spatial interaction frames, bounded 3D view-pose wire codecs, and view
//! state evolution (IXN-01).
//!
//! Spatial surfaces receive direct 3D pointer, gesture, key, and time-scrub
//! events from a host shell and reduce permitted camera motion into a bounded
//! [`ViewPose3D`] before drawing. Neither gesture stream nor view pose carries
//! avionics state authority: flight selections still enter exclusively through
//! admitted operator intents.

#![no_std]

#[cfg(test)]
extern crate std;

mod pose;
mod types;
mod wire;

#[cfg(test)]
mod tests;

pub use pose::{
    InteractionOutcome, VIEW_POSE_BYTES, VIEW_POSE_VERSION, ViewLimits, ViewPose3D, apply_event,
    decode_view_pose, encode_view_pose, encode_view_pose_slice,
};
pub use types::{
    GesturePhase, InteractionError, InteractionFrame, InteractionStamp, KeyState, SpatialEvent,
    TargetId,
};
pub use wire::{
    INTERACTION_FORMAT_VERSION, MAX_INTERACTION_BYTES, decode_interaction, encode_interaction,
};
