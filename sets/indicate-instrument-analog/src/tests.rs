//! Shared fixtures for the set's tests.
#![allow(clippy::expect_used, clippy::panic)]

mod admission;
mod behaviour;
mod build;
mod instruments;
mod png;
mod render;
mod systems;

use std::vec;
use std::vec::Vec;

use indicate_instrument_descriptor::DesignFrame;
use indicate_instrument_raster::{FrameId, FramebufferDims, render};
use indicate_instrument_scene::{MAX_SCENE_BYTES, SceneWriter};
use indicate_instrument_state::{AircraftState, FreshnessPolicy, PanelData, resolve};

use crate::{AnalogDialSpec, DialPose, Lighting, draw_dial, draw_face_scene};

/// Every built-in gauge, with the name its pictures are filed under.
pub(crate) const GAUGES: [(&str, &AnalogDialSpec<'static>); 6] = [
    ("airspeed-kt", &crate::AIRSPEED_KT),
    ("airspeed-mph", &crate::AIRSPEED_MPH),
    ("altimeter", &crate::ALTIMETER),
    ("vertical-speed", &crate::VERTICAL_SPEED),
    ("manifold-pressure", &crate::MANIFOLD_PRESSURE),
    ("tachometer", &crate::TACHOMETER),
];

pub(crate) fn data_of(state: &AircraftState) -> PanelData {
    resolve(state, &FreshnessPolicy::default())
}

pub(crate) fn frame(side: f32) -> DesignFrame {
    DesignFrame {
        width: side,
        height: side,
    }
}

pub(crate) fn dial_scene(
    spec: &AnalogDialSpec<'_>,
    pose: &DialPose,
    lighting: Lighting,
    side: f32,
) -> Vec<u8> {
    let mut buf = vec![0u8; MAX_SCENE_BYTES];
    let mut writer = SceneWriter::new(&mut buf).expect("writer");
    draw_dial(spec, pose, lighting, frame(side), &mut writer).expect("draws");
    let used = writer.finish();
    buf.truncate(used);
    buf
}

pub(crate) fn face_scene(spec: &AnalogDialSpec<'_>, lighting: Lighting, side: f32) -> Vec<u8> {
    let mut buf = vec![0u8; MAX_SCENE_BYTES];
    let mut writer = SceneWriter::new(&mut buf).expect("writer");
    draw_face_scene(spec, lighting, frame(side), &mut writer).expect("draws");
    let used = writer.finish();
    buf.truncate(used);
    buf
}

/// RGBA8 pixels of `scene` on a `side`-pixel square surface.
pub(crate) fn pixels(scene: &[u8], side: u32) -> Vec<u8> {
    let mut out = vec![0u8; (side * side * 4) as usize];
    render(
        scene,
        &mut out,
        FramebufferDims::tight(side, side),
        FrameId {
            frame_generation: 1,
            render_generation: 1,
        },
    )
    .expect("renders");
    out
}
