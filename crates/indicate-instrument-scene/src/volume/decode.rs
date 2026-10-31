//! Borrowing decoder for [`crate::volume`] scenes.

use indicate_frames::Quat;

use crate::cmd::{HAlign, MAX_TEXT_BYTES, PaintMode};
use crate::color::Rgba8;
use crate::decode::DecodeError;

use super::{
    HandleKind, HeightsRef, MAX_HEIGHTFIELD_SAMPLES, MAX_POLYLINE_POINTS, OP_ATTRIBUTE,
    OP_BEGIN_BAND, OP_BILLBOARD_TEXT, OP_BOX_3D, OP_CLIP_BOX_3D, OP_CYLINDER_3D, OP_END_BAND,
    OP_FILL_COLOR, OP_HEIGHTFIELD_GRID, OP_LINE_3D, OP_POLYLINE_3D, OP_RESTORE, OP_RING_3D,
    OP_ROTATE_QUAT, OP_SAVE, OP_SPHERE, OP_STROKE_3D, OP_TARGET_HANDLE, OP_TRANSLATE_3D,
    OP_UNDERLAY_ANCHOR_3D, Points3Ref, VOLUME_SCENE_VERSION, VolumeBandId, VolumeCmd,
    VolumeUnderlayKind, is_finite_vec3, is_ordered_box, is_unit_quat, is_unit_vec3, read_f32,
};

/// Iterator over commands in an encoded 3D volume scene.
#[derive(Debug, Clone)]
pub struct VolumeCmds<'a> {
    rest: &'a [u8],
    failed: bool,
}

impl<'a> VolumeCmds<'a> {
    /// Checks [`VOLUME_SCENE_VERSION`] and returns a borrowing command iterator.
    pub fn new(scene: &'a [u8]) -> Result<Self, DecodeError> {
        match scene.split_first() {
            Some((&v, rest)) if v == VOLUME_SCENE_VERSION => Ok(Self {
                rest,
                failed: false,
            }),
            Some((&found, _)) => Err(DecodeError::BadVersion { found }),
            None => Err(DecodeError::Truncated),
        }
    }

    /// Remaining unconsumed byte count.
    #[must_use]
    pub const fn remaining_len(&self) -> usize {
        self.rest.len()
    }

    fn step(&mut self) -> Result<VolumeCmd<'a>, DecodeError> {
        let (&opcode, rest) = self.rest.split_first().ok_or(DecodeError::Truncated)?;
        let (len_bytes, rest) = rest
            .split_first_chunk::<2>()
            .ok_or(DecodeError::Truncated)?;
        let plen = usize::from(u16::from_le_bytes(*len_bytes));
        if rest.len() < plen {
            return Err(DecodeError::Truncated);
        }
        let (payload, tail) = rest.split_at(plen);
        self.rest = tail;
        decode_volume_payload(opcode, payload)
    }
}

impl<'a> Iterator for VolumeCmds<'a> {
    type Item = Result<VolumeCmd<'a>, DecodeError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.failed || self.rest.is_empty() {
            return None;
        }
        let item = self.step();
        if item.is_err() {
            self.failed = true;
        }
        Some(item)
    }
}

fn decode_volume_payload(opcode: u8, p: &[u8]) -> Result<VolumeCmd<'_>, DecodeError> {
    let bad = DecodeError::BadPayload { opcode };
    match opcode {
        OP_SAVE | OP_RESTORE | OP_TRANSLATE_3D | OP_ROTATE_QUAT | OP_CLIP_BOX_3D => {
            decode_state_op(opcode, p, bad)
        }
        OP_FILL_COLOR | OP_STROKE_3D | OP_ATTRIBUTE | OP_TARGET_HANDLE => {
            decode_style_op(opcode, p, bad)
        }
        OP_LINE_3D | OP_POLYLINE_3D | OP_SPHERE => decode_shape_op(opcode, p, bad),
        OP_BOX_3D | OP_CYLINDER_3D | OP_RING_3D => decode_solid_op(opcode, p, bad),
        OP_HEIGHTFIELD_GRID => decode_grid(p, bad),
        OP_UNDERLAY_ANCHOR_3D => decode_underlay(p, bad),
        OP_BILLBOARD_TEXT => decode_billboard(p, bad),
        OP_BEGIN_BAND => {
            if p.len() != 1 {
                return Err(bad);
            }
            Ok(VolumeCmd::BeginBand {
                band: VolumeBandId::from_u8(p[0]).ok_or(bad)?,
            })
        }
        OP_END_BAND => {
            if p.len() != 1 {
                return Err(bad);
            }
            Ok(VolumeCmd::EndBand {
                band: VolumeBandId::from_u8(p[0]).ok_or(bad)?,
            })
        }
        0x52..=0x5F => Err(bad),
        other => Ok(VolumeCmd::Unknown { opcode: other }),
    }
}

fn decode_state_op<'a>(
    opcode: u8,
    p: &'a [u8],
    bad: DecodeError,
) -> Result<VolumeCmd<'a>, DecodeError> {
    match opcode {
        OP_SAVE => {
            if p.is_empty() {
                Ok(VolumeCmd::Save)
            } else {
                Err(bad)
            }
        }
        OP_RESTORE => {
            if p.is_empty() {
                Ok(VolumeCmd::Restore)
            } else {
                Err(bad)
            }
        }
        OP_TRANSLATE_3D => {
            if p.len() != 12 {
                return Err(bad);
            }
            let delta = read_vec3(&p[0..12]);
            if !is_finite_vec3(delta) {
                return Err(bad);
            }
            Ok(VolumeCmd::Translate3D { delta })
        }
        OP_ROTATE_QUAT => {
            if p.len() != 16 {
                return Err(bad);
            }
            let rotation = read_quat(&p[0..16]);
            if !is_unit_quat(rotation) {
                return Err(bad);
            }
            Ok(VolumeCmd::RotateQuat { rotation })
        }
        _ => {
            if p.len() != 24 {
                return Err(bad);
            }
            let min = read_vec3(&p[0..12]);
            let max = read_vec3(&p[12..24]);
            if !is_ordered_box(min, max) {
                return Err(bad);
            }
            Ok(VolumeCmd::ClipBox3D { min, max })
        }
    }
}

fn decode_style_op<'a>(
    opcode: u8,
    p: &'a [u8],
    bad: DecodeError,
) -> Result<VolumeCmd<'a>, DecodeError> {
    match opcode {
        OP_FILL_COLOR => {
            if p.len() != 4 {
                return Err(bad);
            }
            Ok(VolumeCmd::FillColor {
                color: Rgba8::from_u32(read_u32(&p[0..4])),
            })
        }
        OP_STROKE_3D => {
            if p.len() != 8 {
                return Err(bad);
            }
            let radius_m = read_f32(&p[4..8]);
            if !radius_m.is_finite() || radius_m < 0.0 {
                return Err(bad);
            }
            Ok(VolumeCmd::Stroke3D {
                color: Rgba8::from_u32(read_u32(&p[0..4])),
                radius_m,
            })
        }
        OP_ATTRIBUTE => {
            if p.len() != 1 {
                return Err(bad);
            }
            Ok(VolumeCmd::Attribute { group: p[0] })
        }
        _ => {
            if p.len() != 21 {
                return Err(bad);
            }
            let target_id = read_u32(&p[0..4]);
            let kind = HandleKind::from_u8(p[4]).ok_or(bad)?;
            let center = read_vec3(&p[5..17]);
            let radius_m = read_f32(&p[17..21]);
            if target_id == 0 || !is_finite_vec3(center) || !radius_m.is_finite() || radius_m <= 0.0
            {
                return Err(bad);
            }
            Ok(VolumeCmd::TargetHandle {
                target_id,
                kind,
                center,
                radius_m,
            })
        }
    }
}

fn decode_shape_op<'a>(
    opcode: u8,
    p: &'a [u8],
    bad: DecodeError,
) -> Result<VolumeCmd<'a>, DecodeError> {
    match opcode {
        OP_LINE_3D => {
            if p.len() != 24 {
                return Err(bad);
            }
            let a = read_vec3(&p[0..12]);
            let b = read_vec3(&p[12..24]);
            if !(is_finite_vec3(a) && is_finite_vec3(b)) {
                return Err(bad);
            }
            Ok(VolumeCmd::Line3D { a, b })
        }
        OP_POLYLINE_3D => {
            if p.len() < 24 || !p.len().is_multiple_of(12) || p.len() / 12 > MAX_POLYLINE_POINTS {
                return Err(bad);
            }
            let points = Points3Ref::from_bytes(p);
            if !points.iter().all(is_finite_vec3) {
                return Err(bad);
            }
            Ok(VolumeCmd::Polyline3D { points })
        }
        _ => {
            if p.len() != 17 {
                return Err(bad);
            }
            let mode = decode_mode(p[0], bad)?;
            let center = read_vec3(&p[1..13]);
            let radius_m = read_f32(&p[13..17]);
            if !is_finite_vec3(center) || !radius_m.is_finite() || radius_m <= 0.0 {
                return Err(bad);
            }
            Ok(VolumeCmd::Sphere {
                mode,
                center,
                radius_m,
            })
        }
    }
}

fn decode_solid_op<'a>(
    opcode: u8,
    p: &'a [u8],
    bad: DecodeError,
) -> Result<VolumeCmd<'a>, DecodeError> {
    match opcode {
        OP_BOX_3D => {
            if p.len() != 25 {
                return Err(bad);
            }
            let mode = decode_mode(p[0], bad)?;
            let min = read_vec3(&p[1..13]);
            let max = read_vec3(&p[13..25]);
            if !is_ordered_box(min, max) {
                return Err(bad);
            }
            Ok(VolumeCmd::Box3D { mode, min, max })
        }
        OP_CYLINDER_3D => {
            if p.len() != 29 {
                return Err(bad);
            }
            let mode = decode_mode(p[0], bad)?;
            let base = read_vec3(&p[1..13]);
            let top = read_vec3(&p[13..25]);
            let radius_m = read_f32(&p[25..29]);
            if !(is_finite_vec3(base)
                && is_finite_vec3(top)
                && radius_m.is_finite()
                && radius_m > 0.0)
            {
                return Err(bad);
            }
            Ok(VolumeCmd::Cylinder3D {
                mode,
                base,
                top,
                radius_m,
            })
        }
        _ => {
            if p.len() != 28 {
                return Err(bad);
            }
            let center = read_vec3(&p[0..12]);
            let normal = read_vec3(&p[12..24]);
            let radius_m = read_f32(&p[24..28]);
            if !(is_finite_vec3(center)
                && is_unit_vec3(normal)
                && radius_m.is_finite()
                && radius_m > 0.0)
            {
                return Err(bad);
            }
            Ok(VolumeCmd::Ring3D {
                center,
                normal,
                radius_m,
            })
        }
    }
}

fn decode_grid<'a>(p: &'a [u8], bad: DecodeError) -> Result<VolumeCmd<'a>, DecodeError> {
    if p.len() < 22 {
        return Err(bad);
    }
    let cols = p[0];
    let rows = p[1];
    let count = usize::from(cols) * usize::from(rows);
    if cols < 2 || rows < 2 || count > MAX_HEIGHTFIELD_SAMPLES || p.len() != 22 + count * 4 {
        return Err(bad);
    }
    let origin = read_vec3(&p[2..14]);
    let step_x = read_f32(&p[14..18]);
    let step_z = read_f32(&p[18..22]);
    let heights = HeightsRef::from_bytes(&p[22..]);
    if !is_finite_vec3(origin)
        || !step_x.is_finite()
        || step_x <= 0.0
        || !step_z.is_finite()
        || step_z <= 0.0
        || !heights.iter().all(|h| h.is_finite())
    {
        return Err(bad);
    }
    Ok(VolumeCmd::HeightfieldGrid {
        cols,
        rows,
        origin,
        step_x,
        step_z,
        heights,
    })
}

fn decode_underlay<'a>(p: &'a [u8], bad: DecodeError) -> Result<VolumeCmd<'a>, DecodeError> {
    if p.len() != 49 {
        return Err(bad);
    }
    let kind = VolumeUnderlayKind::from_u8(p[0]).ok_or(bad)?;
    let center = read_vec3(&p[1..13]);
    let extent = read_vec3(&p[13..25]);
    let orientation = read_quat(&p[25..41]);
    let meters_per_unit = read_f64(&p[41..49]);
    if !(is_finite_vec3(center)
        && is_finite_vec3(extent)
        && extent[0] > 0.0
        && extent[1] > 0.0
        && extent[2] > 0.0
        && is_unit_quat(orientation)
        && meters_per_unit.is_finite()
        && meters_per_unit > 0.0)
    {
        return Err(bad);
    }
    Ok(VolumeCmd::UnderlayAnchor3D {
        kind,
        center,
        extent,
        orientation,
        meters_per_unit,
    })
}

fn decode_billboard<'a>(p: &'a [u8], bad: DecodeError) -> Result<VolumeCmd<'a>, DecodeError> {
    if p.len() <= 17 || p.len() - 17 > MAX_TEXT_BYTES {
        return Err(bad);
    }
    let pos = read_vec3(&p[0..12]);
    let size_m = read_f32(&p[12..16]);
    if !(is_finite_vec3(pos) && size_m.is_finite() && size_m > 0.0) {
        return Err(bad);
    }
    let anchor = match p[16] {
        0 => HAlign::Left,
        1 => HAlign::Center,
        2 => HAlign::Right,
        _ => return Err(bad),
    };
    let text = core::str::from_utf8(&p[17..]).map_err(|_| bad)?;
    Ok(VolumeCmd::BillboardText {
        pos,
        size_m,
        anchor,
        text,
    })
}

const fn decode_mode(raw: u8, bad: DecodeError) -> Result<PaintMode, DecodeError> {
    match raw {
        0 => Ok(PaintMode::Fill),
        1 => Ok(PaintMode::Stroke),
        2 => Ok(PaintMode::FillStroke),
        _ => Err(bad),
    }
}

fn read_vec3(slice: &[u8]) -> [f32; 3] {
    [
        read_f32(&slice[0..4]),
        read_f32(&slice[4..8]),
        read_f32(&slice[8..12]),
    ]
}

fn read_quat(slice: &[u8]) -> Quat {
    Quat {
        x: read_f32(&slice[0..4]),
        y: read_f32(&slice[4..8]),
        z: read_f32(&slice[8..12]),
        w: read_f32(&slice[12..16]),
    }
}

fn read_u32(slice: &[u8]) -> u32 {
    let mut b = [0u8; 4];
    b.copy_from_slice(&slice[..4]);
    u32::from_le_bytes(b)
}

fn read_f64(slice: &[u8]) -> f64 {
    let mut b = [0u8; 8];
    b.copy_from_slice(&slice[..8]);
    f64::from_le_bytes(b)
}
