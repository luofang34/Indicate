//! Bounded 3D view pose, camera limits, wire codec, and event reduction.

use indicate_frames::Quat;

use crate::types::{
    GesturePhase, InteractionError, KeyState, SpatialEvent, TargetId, check_unit_quat,
    check_unit_vec3, check_vec3,
};

/// Wire version of the 48-byte [`ViewPose3D`] structure.
pub const VIEW_POSE_VERSION: u8 = 1;

/// Exact encoded byte length of [`ViewPose3D`].
pub const VIEW_POSE_BYTES: usize = 48;

/// Bounded 3D view state supplied to volumetric instruments via `DrawContext`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewPose3D {
    /// Camera focus offset in local volume coordinates (meters).
    pub focus_local: [f32; 3],
    /// Orbit orientation quaternion (unit norm).
    pub orientation: Quat,
    /// Uniform zoom factor (`> 0.0`, default `1.0`).
    pub zoom_scale: f32,
    /// Currently hovered or selected scene handle, or [`TargetId::NONE`].
    pub active_target: TargetId,
    /// Preview time scrub offset in seconds (`0.0` = live).
    pub time_offset_s: f64,
    /// Whether non-essential volumetric annotations are decluttered.
    pub decluttered: bool,
}

impl ViewPose3D {
    /// Canonical default pose: centered, identity orientation, unit zoom, live time.
    pub const IDENTITY: Self = Self {
        focus_local: [0.0, 0.0, 0.0],
        orientation: Quat::IDENTITY,
        zoom_scale: 1.0,
        active_target: TargetId::NONE,
        time_offset_s: 0.0,
        decluttered: false,
    };

    /// Validates that every floating-point and rotation invariant holds.
    pub fn validate(&self) -> Result<(), InteractionError> {
        check_vec3(self.focus_local, "focus_local")?;
        check_unit_quat(self.orientation, "orientation")?;
        if !self.zoom_scale.is_finite() {
            return Err(InteractionError::NonFinite {
                field: "zoom_scale",
            });
        }
        if self.zoom_scale <= 0.0 {
            return Err(InteractionError::NonPositiveScale {
                field: "zoom_scale",
            });
        }
        if !self.time_offset_s.is_finite() {
            return Err(InteractionError::NonFinite {
                field: "time_offset_s",
            });
        }
        Ok(())
    }
}

impl Default for ViewPose3D {
    fn default() -> Self {
        Self::IDENTITY
    }
}

/// Descriptor-declared bounds constraining [`ViewPose3D`] evolution.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewLimits {
    /// Maximum absolute component of `focus_local` in meters (`>= 0.0`).
    pub pan_limit: f32,
    /// Minimum permitted `zoom_scale` (`> 0.0`).
    pub zoom_min: f32,
    /// Maximum permitted `zoom_scale` (`>= zoom_min`).
    pub zoom_max: f32,
    /// Maximum absolute `time_offset_s` in seconds (`>= 0.0`).
    pub time_scrub_limit_s: f64,
    /// Whether orbit rotation gestures may alter `orientation`.
    pub allow_orbit: bool,
    /// Whether time-scrub events may alter `time_offset_s`.
    pub allow_time_scrub: bool,
}

impl ViewLimits {
    /// Fixed camera: zero pan, unit zoom, no orbit, no time scrub.
    pub const LOCKED: Self = Self {
        pan_limit: 0.0,
        zoom_min: 1.0,
        zoom_max: 1.0,
        time_scrub_limit_s: 0.0,
        allow_orbit: false,
        allow_time_scrub: false,
    };

    /// Standard interactive bounds for tabletop volumetric displays.
    pub const VOLUMETRIC_DEFAULT: Self = Self {
        pan_limit: 1.0,
        zoom_min: 0.25,
        zoom_max: 4.0,
        time_scrub_limit_s: 3600.0,
        allow_orbit: true,
        allow_time_scrub: true,
    };

    /// Checks that all limit ranges are finite and well-ordered.
    #[must_use]
    pub const fn is_valid(&self) -> bool {
        self.pan_limit.is_finite()
            && self.pan_limit >= 0.0
            && self.zoom_min.is_finite()
            && self.zoom_max.is_finite()
            && self.zoom_min > 0.0
            && self.zoom_min <= self.zoom_max
            && self.time_scrub_limit_s.is_finite()
            && self.time_scrub_limit_s >= 0.0
    }
}

/// Result of reducing a [`SpatialEvent`] against a [`ViewPose3D`] and [`ViewLimits`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InteractionOutcome {
    /// [`ViewPose3D`] changed within its declared limits; request a redraw.
    ViewUpdated,
    /// User activated a target or point in local instrument coordinates.
    TargetActivated {
        /// Activated target identifier, or [`TargetId::NONE`].
        target: TargetId,
        /// Contact point in local volume coordinates (meters).
        point_local: [f32; 3],
    },
    /// Discrete key press, release, or repeat forwarded to host binding logic.
    KeyInput {
        /// Logical key identifier.
        key_id: u16,
        /// Key transition state.
        state: KeyState,
    },
    /// Event had no effect under the active [`ViewLimits`] or gesture phase.
    Ignored,
}

/// Encodes `pose` into its fixed 48-byte little-endian wire layout.
pub fn encode_view_pose(
    pose: &ViewPose3D,
    out: &mut [u8; VIEW_POSE_BYTES],
) -> Result<(), InteractionError> {
    pose.validate()?;
    *out = [0u8; VIEW_POSE_BYTES];
    out[0] = VIEW_POSE_VERSION;
    out[1] = u8::from(pose.decluttered);
    out[2] = 0;
    out[3] = 0;
    out[4..8].copy_from_slice(&pose.focus_local[0].to_le_bytes());
    out[8..12].copy_from_slice(&pose.focus_local[1].to_le_bytes());
    out[12..16].copy_from_slice(&pose.focus_local[2].to_le_bytes());
    out[16..20].copy_from_slice(&pose.orientation.x.to_le_bytes());
    out[20..24].copy_from_slice(&pose.orientation.y.to_le_bytes());
    out[24..28].copy_from_slice(&pose.orientation.z.to_le_bytes());
    out[28..32].copy_from_slice(&pose.orientation.w.to_le_bytes());
    out[32..36].copy_from_slice(&pose.zoom_scale.to_le_bytes());
    out[36..40].copy_from_slice(&pose.active_target.0.to_le_bytes());
    out[40..48].copy_from_slice(&pose.time_offset_s.to_le_bytes());
    Ok(())
}

/// Encodes `pose` at the start of `out`, returning [`VIEW_POSE_BYTES`].
pub fn encode_view_pose_slice(
    pose: &ViewPose3D,
    out: &mut [u8],
) -> Result<usize, InteractionError> {
    if out.len() < VIEW_POSE_BYTES {
        return Err(InteractionError::BufferTooSmall {
            needed: VIEW_POSE_BYTES,
            available: out.len(),
        });
    }
    let mut buf = [0u8; VIEW_POSE_BYTES];
    encode_view_pose(pose, &mut buf)?;
    out[..VIEW_POSE_BYTES].copy_from_slice(&buf);
    Ok(VIEW_POSE_BYTES)
}

/// Decodes a 48-byte little-endian slice into a validated [`ViewPose3D`].
pub fn decode_view_pose(bytes: &[u8]) -> Result<ViewPose3D, InteractionError> {
    if bytes.len() != VIEW_POSE_BYTES {
        return Err(InteractionError::InvalidLength {
            expected: VIEW_POSE_BYTES,
            actual: bytes.len(),
        });
    }
    if bytes[0] != VIEW_POSE_VERSION {
        return Err(InteractionError::UnsupportedVersion(bytes[0]));
    }
    let flags = bytes[1];
    if (flags & !0x01) != 0 || bytes[2] != 0 || bytes[3] != 0 {
        return Err(InteractionError::NonZeroReserved);
    }
    let pose = ViewPose3D {
        focus_local: [
            read_f32(&bytes[4..8]),
            read_f32(&bytes[8..12]),
            read_f32(&bytes[12..16]),
        ],
        orientation: Quat {
            x: read_f32(&bytes[16..20]),
            y: read_f32(&bytes[20..24]),
            z: read_f32(&bytes[24..28]),
            w: read_f32(&bytes[28..32]),
        },
        zoom_scale: read_f32(&bytes[32..36]),
        active_target: TargetId(read_u32(&bytes[36..40])),
        time_offset_s: read_f64(&bytes[40..48]),
        decluttered: (flags & 0x01) != 0,
    };
    pose.validate()?;
    Ok(pose)
}

/// Evolves `pose` in response to `event`, clamping to `limits`.
pub fn apply_event(
    pose: &mut ViewPose3D,
    event: &SpatialEvent,
    limits: &ViewLimits,
) -> Result<InteractionOutcome, InteractionError> {
    if !limits.is_valid() {
        return Err(InteractionError::InvalidLimits);
    }
    pose.validate()?;
    match *event {
        SpatialEvent::Activate {
            target,
            point_local,
        } => {
            check_vec3(point_local, "point_local")?;
            pose.active_target = target;
            Ok(InteractionOutcome::TargetActivated {
                target,
                point_local,
            })
        }
        SpatialEvent::Drag3D {
            phase,
            target,
            origin_local,
            delta_local,
        } => apply_drag(pose, phase, target, origin_local, delta_local, limits),
        SpatialEvent::Transform3D {
            phase,
            pivot_local,
            translation_local,
            rotation,
            scale,
        } => apply_transform(
            pose,
            phase,
            pivot_local,
            translation_local,
            rotation,
            scale,
            limits,
        ),
        SpatialEvent::RayHover {
            target,
            origin_local,
            dir_local,
        } => {
            check_vec3(origin_local, "origin_local")?;
            check_unit_vec3(dir_local, "dir_local")?;
            pose.active_target = target;
            Ok(InteractionOutcome::ViewUpdated)
        }
        SpatialEvent::DiscreteKey { key_id, state } => {
            Ok(InteractionOutcome::KeyInput { key_id, state })
        }
        SpatialEvent::ScrubTime { offset_s } => {
            if !offset_s.is_finite() {
                return Err(InteractionError::NonFinite { field: "offset_s" });
            }
            if !limits.allow_time_scrub {
                return Ok(InteractionOutcome::Ignored);
            }
            pose.time_offset_s =
                offset_s.clamp(-limits.time_scrub_limit_s, limits.time_scrub_limit_s);
            Ok(InteractionOutcome::ViewUpdated)
        }
        SpatialEvent::ResetView => {
            *pose = ViewPose3D {
                zoom_scale: 1.0_f32.clamp(limits.zoom_min, limits.zoom_max),
                ..ViewPose3D::IDENTITY
            };
            Ok(InteractionOutcome::ViewUpdated)
        }
        SpatialEvent::Unknown { .. } => Ok(InteractionOutcome::Ignored),
    }
}

fn apply_drag(
    pose: &mut ViewPose3D,
    phase: GesturePhase,
    target: TargetId,
    origin_local: [f32; 3],
    delta_local: [f32; 3],
    limits: &ViewLimits,
) -> Result<InteractionOutcome, InteractionError> {
    check_vec3(origin_local, "origin_local")?;
    check_vec3(delta_local, "delta_local")?;
    if phase == GesturePhase::Cancelled {
        return Ok(InteractionOutcome::Ignored);
    }
    let mut changed = false;
    if !target.is_none() {
        pose.active_target = target;
        changed = true;
    }
    if limits.pan_limit > 0.0 {
        for (slot, delta) in pose.focus_local.iter_mut().zip(delta_local) {
            *slot = (*slot + delta).clamp(-limits.pan_limit, limits.pan_limit);
        }
        changed = true;
    }
    Ok(if changed {
        InteractionOutcome::ViewUpdated
    } else {
        InteractionOutcome::Ignored
    })
}

fn apply_transform(
    pose: &mut ViewPose3D,
    phase: GesturePhase,
    pivot_local: [f32; 3],
    translation_local: [f32; 3],
    rotation: Quat,
    scale: f32,
    limits: &ViewLimits,
) -> Result<InteractionOutcome, InteractionError> {
    check_vec3(pivot_local, "pivot_local")?;
    check_vec3(translation_local, "translation_local")?;
    check_unit_quat(rotation, "rotation")?;
    if !scale.is_finite() {
        return Err(InteractionError::NonFinite { field: "scale" });
    }
    if scale <= 0.0 {
        return Err(InteractionError::NonPositiveScale { field: "scale" });
    }
    if phase == GesturePhase::Cancelled {
        return Ok(InteractionOutcome::Ignored);
    }
    let can_zoom = limits.zoom_max > limits.zoom_min;
    if limits.pan_limit == 0.0 && !limits.allow_orbit && !can_zoom {
        return Ok(InteractionOutcome::Ignored);
    }
    if limits.pan_limit > 0.0 {
        for (slot, delta) in pose.focus_local.iter_mut().zip(translation_local) {
            *slot = (*slot + delta).clamp(-limits.pan_limit, limits.pan_limit);
        }
    }
    if limits.allow_orbit {
        pose.orientation = rotation
            .compose(pose.orientation)
            .renormalized(1.0e-2)
            .map_err(|_| InteractionError::NonUnit { field: "rotation" })?;
    }
    pose.zoom_scale = (pose.zoom_scale * scale).clamp(limits.zoom_min, limits.zoom_max);
    Ok(InteractionOutcome::ViewUpdated)
}

fn read_u32(slice: &[u8]) -> u32 {
    let mut b = [0u8; 4];
    b.copy_from_slice(&slice[..4]);
    u32::from_le_bytes(b)
}

fn read_f32(slice: &[u8]) -> f32 {
    let mut b = [0u8; 4];
    b.copy_from_slice(&slice[..4]);
    f32::from_le_bytes(b)
}

fn read_f64(slice: &[u8]) -> f64 {
    let mut b = [0u8; 8];
    b.copy_from_slice(&slice[..8]);
    f64::from_le_bytes(b)
}
