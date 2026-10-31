//! Binary wire codec for [`InteractionFrame`].

use indicate_frames::Quat;

use crate::types::{
    GesturePhase, InteractionError, InteractionFrame, InteractionStamp, KeyState, SpatialEvent,
    TargetId, check_unit_quat, check_unit_vec3, check_vec3,
};

/// Wire version of the [`InteractionFrame`] envelope.
pub const INTERACTION_FORMAT_VERSION: u8 = 1;

/// Maximum encoded byte length of one [`InteractionFrame`] (`20` header + `52` max payload).
pub const MAX_INTERACTION_BYTES: usize = 72;

const HEADER_BYTES: usize = 20;

const OP_ACTIVATE: u8 = 0x01;
const OP_DRAG_3D: u8 = 0x02;
const OP_TRANSFORM_3D: u8 = 0x03;
const OP_RAY_HOVER: u8 = 0x04;
const OP_DISCRETE_KEY: u8 = 0x05;
const OP_SCRUB_TIME: u8 = 0x06;
const OP_RESET_VIEW: u8 = 0x07;

/// Encodes `frame` into `out`, returning the number of bytes written.
pub fn encode_interaction(
    frame: &InteractionFrame,
    out: &mut [u8],
) -> Result<usize, InteractionError> {
    let mut payload = [0u8; MAX_INTERACTION_BYTES - HEADER_BYTES];
    let (opcode, payload_len) = encode_event(&frame.event, &mut payload)?;
    let total = HEADER_BYTES + payload_len;
    if total > MAX_INTERACTION_BYTES {
        return Err(InteractionError::FrameTooLarge { len: total });
    }
    if out.len() < total {
        return Err(InteractionError::BufferTooSmall {
            needed: total,
            available: out.len(),
        });
    }
    out[0] = INTERACTION_FORMAT_VERSION;
    out[1] = opcode;
    let len_u16 =
        u16::try_from(payload_len).map_err(|_| InteractionError::FrameTooLarge { len: total })?;
    out[2..4].copy_from_slice(&len_u16.to_le_bytes());
    out[4..8].copy_from_slice(&frame.stamp.device_id.to_le_bytes());
    out[8..12].copy_from_slice(&frame.stamp.sequence.to_le_bytes());
    out[12..20].copy_from_slice(&frame.stamp.timestamp_us.to_le_bytes());
    out[HEADER_BYTES..total].copy_from_slice(&payload[..payload_len]);
    Ok(total)
}

/// Decodes a wire byte slice into a validated [`InteractionFrame`].
pub fn decode_interaction(bytes: &[u8]) -> Result<InteractionFrame, InteractionError> {
    if bytes.len() < HEADER_BYTES {
        return Err(InteractionError::InvalidLength {
            expected: HEADER_BYTES,
            actual: bytes.len(),
        });
    }
    if bytes.len() > MAX_INTERACTION_BYTES {
        return Err(InteractionError::FrameTooLarge { len: bytes.len() });
    }
    if bytes[0] != INTERACTION_FORMAT_VERSION {
        return Err(InteractionError::UnsupportedVersion(bytes[0]));
    }
    let opcode = bytes[1];
    let payload_len = usize::from(read_u16(&bytes[2..4]));
    let expected = HEADER_BYTES + payload_len;
    if expected > MAX_INTERACTION_BYTES {
        return Err(InteractionError::FrameTooLarge { len: expected });
    }
    if bytes.len() != expected {
        return Err(InteractionError::InvalidLength {
            expected,
            actual: bytes.len(),
        });
    }
    let stamp = InteractionStamp {
        device_id: read_u32(&bytes[4..8]),
        sequence: read_u32(&bytes[8..12]),
        timestamp_us: read_u64(&bytes[12..20]),
    };
    let event = decode_event(opcode, &bytes[HEADER_BYTES..expected])?;
    Ok(InteractionFrame { stamp, event })
}

fn encode_event(event: &SpatialEvent, out: &mut [u8]) -> Result<(u8, usize), InteractionError> {
    match *event {
        SpatialEvent::Activate {
            target,
            point_local,
        } => {
            check_vec3(point_local, "point_local")?;
            out[0..4].copy_from_slice(&target.0.to_le_bytes());
            write_vec3(&mut out[4..16], point_local);
            Ok((OP_ACTIVATE, 16))
        }
        SpatialEvent::Drag3D {
            phase,
            target,
            origin_local,
            delta_local,
        } => {
            check_vec3(origin_local, "origin_local")?;
            check_vec3(delta_local, "delta_local")?;
            out[0] = phase.to_u8();
            out[1..5].copy_from_slice(&target.0.to_le_bytes());
            write_vec3(&mut out[5..17], origin_local);
            write_vec3(&mut out[17..29], delta_local);
            Ok((OP_DRAG_3D, 29))
        }
        SpatialEvent::Transform3D {
            phase,
            pivot_local,
            translation_local,
            rotation,
            scale,
        } => encode_transform(out, phase, pivot_local, translation_local, rotation, scale),
        SpatialEvent::RayHover {
            target,
            origin_local,
            dir_local,
        } => {
            check_vec3(origin_local, "origin_local")?;
            check_unit_vec3(dir_local, "dir_local")?;
            out[0..4].copy_from_slice(&target.0.to_le_bytes());
            write_vec3(&mut out[4..16], origin_local);
            write_vec3(&mut out[16..28], dir_local);
            Ok((OP_RAY_HOVER, 28))
        }
        SpatialEvent::DiscreteKey { key_id, state } => {
            out[0..2].copy_from_slice(&key_id.to_le_bytes());
            out[2] = state.to_u8();
            Ok((OP_DISCRETE_KEY, 3))
        }
        SpatialEvent::ScrubTime { offset_s } => {
            if !offset_s.is_finite() {
                return Err(InteractionError::NonFinite { field: "offset_s" });
            }
            out[0..8].copy_from_slice(&offset_s.to_le_bytes());
            Ok((OP_SCRUB_TIME, 8))
        }
        SpatialEvent::ResetView => Ok((OP_RESET_VIEW, 0)),
        SpatialEvent::Unknown { opcode } => {
            if (OP_ACTIVATE..=OP_RESET_VIEW).contains(&opcode) {
                return Err(InteractionError::ReservedUnknownOpcode(opcode));
            }
            Ok((opcode, 0))
        }
    }
}

fn encode_transform(
    out: &mut [u8],
    phase: GesturePhase,
    pivot_local: [f32; 3],
    translation_local: [f32; 3],
    rotation: Quat,
    scale: f32,
) -> Result<(u8, usize), InteractionError> {
    check_vec3(pivot_local, "pivot_local")?;
    check_vec3(translation_local, "translation_local")?;
    check_unit_quat(rotation, "rotation")?;
    if !scale.is_finite() {
        return Err(InteractionError::NonFinite { field: "scale" });
    }
    if scale <= 0.0 {
        return Err(InteractionError::NonPositiveScale { field: "scale" });
    }
    out[0] = phase.to_u8();
    write_vec3(&mut out[1..13], pivot_local);
    write_vec3(&mut out[13..25], translation_local);
    out[25..29].copy_from_slice(&rotation.x.to_le_bytes());
    out[29..33].copy_from_slice(&rotation.y.to_le_bytes());
    out[33..37].copy_from_slice(&rotation.z.to_le_bytes());
    out[37..41].copy_from_slice(&rotation.w.to_le_bytes());
    out[41..45].copy_from_slice(&scale.to_le_bytes());
    Ok((OP_TRANSFORM_3D, 45))
}

fn decode_event(opcode: u8, payload: &[u8]) -> Result<SpatialEvent, InteractionError> {
    match opcode {
        OP_ACTIVATE => {
            require_len(payload, 16)?;
            let point_local = read_vec3(&payload[4..16]);
            check_vec3(point_local, "point_local")?;
            Ok(SpatialEvent::Activate {
                target: TargetId(read_u32(&payload[0..4])),
                point_local,
            })
        }
        OP_DRAG_3D => {
            require_len(payload, 29)?;
            let phase = decode_phase(payload[0])?;
            let origin_local = read_vec3(&payload[5..17]);
            let delta_local = read_vec3(&payload[17..29]);
            check_vec3(origin_local, "origin_local")?;
            check_vec3(delta_local, "delta_local")?;
            Ok(SpatialEvent::Drag3D {
                phase,
                target: TargetId(read_u32(&payload[1..5])),
                origin_local,
                delta_local,
            })
        }
        OP_TRANSFORM_3D => decode_transform(payload),
        OP_RAY_HOVER => {
            require_len(payload, 28)?;
            let origin_local = read_vec3(&payload[4..16]);
            let dir_local = read_vec3(&payload[16..28]);
            check_vec3(origin_local, "origin_local")?;
            check_unit_vec3(dir_local, "dir_local")?;
            Ok(SpatialEvent::RayHover {
                target: TargetId(read_u32(&payload[0..4])),
                origin_local,
                dir_local,
            })
        }
        OP_DISCRETE_KEY => {
            require_len(payload, 3)?;
            let state =
                KeyState::from_u8(payload[2]).ok_or(InteractionError::InvalidDiscriminant {
                    field: "KeyState",
                    value: payload[2],
                })?;
            Ok(SpatialEvent::DiscreteKey {
                key_id: read_u16(&payload[0..2]),
                state,
            })
        }
        OP_SCRUB_TIME => {
            require_len(payload, 8)?;
            let offset_s = read_f64(&payload[0..8]);
            if !offset_s.is_finite() {
                return Err(InteractionError::NonFinite { field: "offset_s" });
            }
            Ok(SpatialEvent::ScrubTime { offset_s })
        }
        OP_RESET_VIEW => {
            require_len(payload, 0)?;
            Ok(SpatialEvent::ResetView)
        }
        other => Ok(SpatialEvent::Unknown { opcode: other }),
    }
}

fn decode_transform(payload: &[u8]) -> Result<SpatialEvent, InteractionError> {
    require_len(payload, 45)?;
    let phase = decode_phase(payload[0])?;
    let pivot_local = read_vec3(&payload[1..13]);
    let translation_local = read_vec3(&payload[13..25]);
    let rotation = Quat {
        x: read_f32(&payload[25..29]),
        y: read_f32(&payload[29..33]),
        z: read_f32(&payload[33..37]),
        w: read_f32(&payload[37..41]),
    };
    let scale = read_f32(&payload[41..45]);
    check_vec3(pivot_local, "pivot_local")?;
    check_vec3(translation_local, "translation_local")?;
    check_unit_quat(rotation, "rotation")?;
    if !scale.is_finite() {
        return Err(InteractionError::NonFinite { field: "scale" });
    }
    if scale <= 0.0 {
        return Err(InteractionError::NonPositiveScale { field: "scale" });
    }
    Ok(SpatialEvent::Transform3D {
        phase,
        pivot_local,
        translation_local,
        rotation,
        scale,
    })
}

fn decode_phase(raw: u8) -> Result<GesturePhase, InteractionError> {
    GesturePhase::from_u8(raw).ok_or(InteractionError::InvalidDiscriminant {
        field: "GesturePhase",
        value: raw,
    })
}

fn require_len(payload: &[u8], expected: usize) -> Result<(), InteractionError> {
    if payload.len() == expected {
        Ok(())
    } else {
        Err(InteractionError::InvalidLength {
            expected: HEADER_BYTES + expected,
            actual: HEADER_BYTES + payload.len(),
        })
    }
}

fn write_vec3(out: &mut [u8], v: [f32; 3]) {
    out[0..4].copy_from_slice(&v[0].to_le_bytes());
    out[4..8].copy_from_slice(&v[1].to_le_bytes());
    out[8..12].copy_from_slice(&v[2].to_le_bytes());
}

fn read_vec3(slice: &[u8]) -> [f32; 3] {
    [
        read_f32(&slice[0..4]),
        read_f32(&slice[4..8]),
        read_f32(&slice[8..12]),
    ]
}

fn read_u16(slice: &[u8]) -> u16 {
    let mut b = [0u8; 2];
    b.copy_from_slice(&slice[..2]);
    u16::from_le_bytes(b)
}

fn read_u32(slice: &[u8]) -> u32 {
    let mut b = [0u8; 4];
    b.copy_from_slice(&slice[..4]);
    u32::from_le_bytes(b)
}

fn read_u64(slice: &[u8]) -> u64 {
    let mut b = [0u8; 8];
    b.copy_from_slice(&slice[..8]);
    u64::from_le_bytes(b)
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
