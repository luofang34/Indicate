//! The cross-shell scene digest (ADR-0033): one number that proves two
//! shells show the same instruments across 2D, angular, and 3D spaces.

use indicate_instrument_descriptor::{
    CANONICAL_STATES, DesignFrame, DrawContext, PanelDescriptor, PanelDrawError, RenderSpace,
    SceneTarget,
};
use indicate_instrument_scene::{
    SCENE_FORMAT_VERSION, SceneWriter,
    angular::{ANGULAR_SCENE_VERSION, AngularSceneWriter},
    volume::{Extent3D, VOLUME_SCENE_VERSION, VolumeSceneWriter},
};
use indicate_instrument_state::{FreshnessPolicy, abi::v8, resolve};
use indicate_sha256::Sha256Ctx;

use crate::registry::Registry;

/// Domain separator; a new value is a deliberate contract break.
pub const SCENE_DIGEST_DOMAIN: &[u8] = b"pilotage-scene-digest-v1";

/// Why a digest run failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DigestError {
    /// An instrument refused to draw a corpus state.
    #[error("panel {panel} failed to draw corpus state {state}")]
    Draw {
        /// The refusing instrument.
        panel: &'static str,
        /// The corpus state being drawn.
        state: &'static str,
        /// The instrument's own reason.
        #[source]
        source: PanelDrawError,
    },
    /// The caller's scratch buffer cannot hold a scene.
    #[error("scene scratch buffer of {len} bytes is too small")]
    Scratch {
        /// The offending buffer length.
        len: usize,
    },
}

const ROLE_PANEL: u8 = 1;
const ROLE_STATE: u8 = 2;
const ROLE_SCENE: u8 = 3;
const ROLE_FRAME: u8 = 4;
const ROLE_ANGULAR_SCENE: u8 = 5;
const ROLE_VOLUME_EXTENT: u8 = 6;
const ROLE_VOLUME_SCENE: u8 = 7;

/// Digests `registry` over the shared corpus plus each instrument's own
/// extreme states across all canonical 2D frames, angular spheres, and 3D extents.
pub fn scene_digest(registry: &Registry, scratch: &mut [u8]) -> Result<[u8; 32], DigestError> {
    let mut ctx = Sha256Ctx::new();
    ctx.update(SCENE_DIGEST_DOMAIN);
    ctx.update(&[
        SCENE_FORMAT_VERSION,
        ANGULAR_SCENE_VERSION,
        VOLUME_SCENE_VERSION,
        v8::VERSION,
    ]);
    for panel in registry.panels() {
        digest_panel_contract(&mut ctx, panel);
        for state in CANONICAL_STATES {
            digest_state(&mut ctx, panel, state.id, (state.build)(), scratch)?;
        }
        for extreme in panel.extreme_states {
            digest_state(&mut ctx, panel, extreme.id, (extreme.build)(), scratch)?;
        }
    }
    Ok(ctx.finalize())
}

fn digest_panel_contract(ctx: &mut Sha256Ctx, panel: &PanelDescriptor) {
    update_framed(ctx, ROLE_PANEL, panel.id.as_bytes());
    ctx.update(&[panel.space.kind_byte()]);
    ctx.update(&panel.required_layers.to_le_bytes());
    ctx.update(&panel.required_groups.bits().to_le_bytes());
    digest_space_contract(ctx, &panel.space);
    ctx.update(&[panel.underlay.to_u8()]);
    ctx.update(&(panel.config_schema.len() as u32).to_le_bytes());
    for key in panel.config_schema {
        ctx.update(&key.0.to_le_bytes());
    }
    ctx.update(&(panel.accepted_intents.len() as u32).to_le_bytes());
    for intent in panel.accepted_intents {
        ctx.update(&intent.namespace.0.to_le_bytes());
        ctx.update(&intent.opcode.to_le_bytes());
    }
    ctx.update(&(panel.aux_channels.len() as u32).to_le_bytes());
    for ch in panel.aux_channels {
        ctx.update(&ch.0.to_le_bytes());
    }
}

fn digest_space_contract(ctx: &mut Sha256Ctx, space: &RenderSpace) {
    match space {
        RenderSpace::Planar2D {
            frame_min,
            frame_max,
            frame_step,
            aspect_min,
            aspect_max,
            canonical_frames,
        } => {
            digest_frame(ctx, *frame_min);
            digest_frame(ctx, *frame_max);
            ctx.update(&frame_step.0.to_le_bytes());
            ctx.update(&frame_step.1.to_le_bytes());
            ctx.update(&aspect_min.to_le_bytes());
            ctx.update(&aspect_max.to_le_bytes());
            ctx.update(&(canonical_frames.len() as u32).to_le_bytes());
            for frame in *canonical_frames {
                digest_frame(ctx, *frame);
            }
        }
        RenderSpace::AngularSphere { overlay_frame } => match overlay_frame {
            Some(frame) => {
                ctx.update(&[1]);
                digest_frame(ctx, *frame);
            }
            None => ctx.update(&[0]),
        },
        RenderSpace::Volume3D {
            extent_min_m,
            extent_max_m,
            canonical_extents_m,
            view_limits,
        } => {
            digest_extent(ctx, *extent_min_m);
            digest_extent(ctx, *extent_max_m);
            ctx.update(&view_limits.pan_limit.to_le_bytes());
            ctx.update(&view_limits.zoom_min.to_le_bytes());
            ctx.update(&view_limits.zoom_max.to_le_bytes());
            ctx.update(&view_limits.time_scrub_limit_s.to_le_bytes());
            ctx.update(&[
                u8::from(view_limits.allow_orbit),
                u8::from(view_limits.allow_time_scrub),
            ]);
            ctx.update(&(canonical_extents_m.len() as u32).to_le_bytes());
            for ext in *canonical_extents_m {
                digest_extent(ctx, *ext);
            }
        }
    }
}

fn digest_state(
    ctx: &mut Sha256Ctx,
    panel: &PanelDescriptor,
    state_id: &'static str,
    state: indicate_instrument_state::AircraftState,
    scratch: &mut [u8],
) -> Result<(), DigestError> {
    update_framed(ctx, ROLE_STATE, state_id.as_bytes());
    let data = resolve(&state, &FreshnessPolicy::default());
    let draw_ctx = DrawContext::new(&data).with_raw_state(&state);
    match panel.space {
        RenderSpace::Planar2D {
            canonical_frames, ..
        } => {
            for frame in canonical_frames {
                digest_planar_scene(ctx, panel, state_id, &draw_ctx, *frame, scratch)?;
            }
        }
        RenderSpace::AngularSphere { overlay_frame } => {
            digest_angular_scene(ctx, panel, state_id, &draw_ctx, overlay_frame, scratch)?;
        }
        RenderSpace::Volume3D {
            canonical_extents_m,
            ..
        } => {
            for ext in canonical_extents_m {
                digest_volume_scene(ctx, panel, state_id, &draw_ctx, *ext, scratch)?;
            }
        }
    }
    Ok(())
}

fn digest_planar_scene(
    ctx: &mut Sha256Ctx,
    panel: &PanelDescriptor,
    state_id: &'static str,
    draw_ctx: &DrawContext<'_>,
    frame: DesignFrame,
    scratch: &mut [u8],
) -> Result<(), DigestError> {
    ctx.update(&[ROLE_FRAME]);
    digest_frame(ctx, frame);
    let scratch_len = scratch.len();
    let mut writer =
        SceneWriter::new(scratch).map_err(|_| DigestError::Scratch { len: scratch_len })?;
    let target = SceneTarget::Planar2D {
        frame,
        writer: &mut writer,
    };
    (panel.draw)(draw_ctx, target).map_err(|source| DigestError::Draw {
        panel: panel.id,
        state: state_id,
        source,
    })?;
    let used = writer.finish();
    let scene = scratch
        .get(..used)
        .ok_or(DigestError::Scratch { len: scratch_len })?;
    update_framed(ctx, ROLE_SCENE, scene);
    Ok(())
}

fn digest_angular_scene(
    ctx: &mut Sha256Ctx,
    panel: &PanelDescriptor,
    state_id: &'static str,
    draw_ctx: &DrawContext<'_>,
    overlay_frame: Option<DesignFrame>,
    scratch: &mut [u8],
) -> Result<(), DigestError> {
    let scratch_len = scratch.len();
    match overlay_frame {
        Some(ov_frame) => {
            if scratch_len < 4 {
                return Err(DigestError::Scratch { len: scratch_len });
            }
            let (ang_buf, ov_buf) = scratch.split_at_mut(scratch_len / 2);
            let mut ang_writer = AngularSceneWriter::new(ang_buf)
                .map_err(|_| DigestError::Scratch { len: scratch_len })?;
            let mut ov_writer =
                SceneWriter::new(ov_buf).map_err(|_| DigestError::Scratch { len: scratch_len })?;
            let target = SceneTarget::AngularSphere {
                angular: &mut ang_writer,
                overlay: Some((ov_frame, &mut ov_writer)),
            };
            (panel.draw)(draw_ctx, target).map_err(|source| DigestError::Draw {
                panel: panel.id,
                state: state_id,
                source,
            })?;
            let ang_used = ang_writer.finish();
            let ov_used = ov_writer.finish();
            let ang_scene = ang_buf
                .get(..ang_used)
                .ok_or(DigestError::Scratch { len: scratch_len })?;
            let ov_scene = ov_buf
                .get(..ov_used)
                .ok_or(DigestError::Scratch { len: scratch_len })?;
            update_framed(ctx, ROLE_ANGULAR_SCENE, ang_scene);
            ctx.update(&[ROLE_FRAME]);
            digest_frame(ctx, ov_frame);
            update_framed(ctx, ROLE_SCENE, ov_scene);
        }
        None => {
            let mut ang_writer = AngularSceneWriter::new(scratch)
                .map_err(|_| DigestError::Scratch { len: scratch_len })?;
            let target = SceneTarget::AngularSphere {
                angular: &mut ang_writer,
                overlay: None,
            };
            (panel.draw)(draw_ctx, target).map_err(|source| DigestError::Draw {
                panel: panel.id,
                state: state_id,
                source,
            })?;
            let ang_used = ang_writer.finish();
            let ang_scene = scratch
                .get(..ang_used)
                .ok_or(DigestError::Scratch { len: scratch_len })?;
            update_framed(ctx, ROLE_ANGULAR_SCENE, ang_scene);
        }
    }
    Ok(())
}

fn digest_volume_scene(
    ctx: &mut Sha256Ctx,
    panel: &PanelDescriptor,
    state_id: &'static str,
    draw_ctx: &DrawContext<'_>,
    extent_m: Extent3D,
    scratch: &mut [u8],
) -> Result<(), DigestError> {
    ctx.update(&[ROLE_VOLUME_EXTENT]);
    digest_extent(ctx, extent_m);
    let scratch_len = scratch.len();
    let mut writer =
        VolumeSceneWriter::new(scratch).map_err(|_| DigestError::Scratch { len: scratch_len })?;
    let target = SceneTarget::Volume3D {
        extent_m,
        writer: &mut writer,
    };
    (panel.draw)(draw_ctx, target).map_err(|source| DigestError::Draw {
        panel: panel.id,
        state: state_id,
        source,
    })?;
    let used = writer.finish();
    let scene = scratch
        .get(..used)
        .ok_or(DigestError::Scratch { len: scratch_len })?;
    update_framed(ctx, ROLE_VOLUME_SCENE, scene);
    Ok(())
}

pub(crate) fn digest_frame(ctx: &mut Sha256Ctx, frame: DesignFrame) {
    ctx.update(&frame.width.to_le_bytes());
    ctx.update(&frame.height.to_le_bytes());
}

fn digest_extent(ctx: &mut Sha256Ctx, ext: Extent3D) {
    ctx.update(&ext.width_m.to_le_bytes());
    ctx.update(&ext.height_m.to_le_bytes());
    ctx.update(&ext.depth_m.to_le_bytes());
}

pub(crate) fn update_framed(ctx: &mut Sha256Ctx, role: u8, bytes: &[u8]) {
    ctx.update(&[role]);
    ctx.update(&(bytes.len() as u32).to_le_bytes());
    ctx.update(bytes);
}

#[cfg(test)]
mod tests;
