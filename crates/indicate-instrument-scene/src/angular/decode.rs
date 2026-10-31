//! Borrowing decoder for [`crate::angular`] scenes.

use crate::cmd::{HAlign, MAX_TEXT_BYTES};
use crate::color::Rgba8;
use crate::decode::DecodeError;

use super::{
    ANGULAR_SCENE_VERSION, AngularCmd, AngularLayerId, AngularReferenceKind, MAX_POLYLINE_POINTS,
    OP_ANGULAR_TEXT, OP_ATTRIBUTE, OP_BASIS, OP_BEGIN_LAYER, OP_END_LAYER, OP_FILL_COLOR,
    OP_GREAT_CIRCLE_ARC, OP_RAY_LINE, OP_RAY_POLYLINE, OP_RESTORE, OP_RETICLE_RING, OP_SAVE,
    OP_STROKE_MRAD, OP_TARGET_TAG, Points3Ref, is_unit_vec3, read_f32,
};

/// Iterator over commands in an encoded angular scene.
#[derive(Debug, Clone)]
pub struct AngularCmds<'a> {
    rest: &'a [u8],
    failed: bool,
}

impl<'a> AngularCmds<'a> {
    /// Checks [`ANGULAR_SCENE_VERSION`] and returns a borrowing command iterator.
    pub fn new(scene: &'a [u8]) -> Result<Self, DecodeError> {
        match scene.split_first() {
            Some((&v, rest)) if v == ANGULAR_SCENE_VERSION => Ok(Self {
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

    fn step(&mut self) -> Result<AngularCmd<'a>, DecodeError> {
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
        decode_angular_payload(opcode, payload)
    }
}

impl<'a> Iterator for AngularCmds<'a> {
    type Item = Result<AngularCmd<'a>, DecodeError>;

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

fn decode_angular_payload(opcode: u8, p: &[u8]) -> Result<AngularCmd<'_>, DecodeError> {
    let bad = DecodeError::BadPayload { opcode };
    match opcode {
        OP_SAVE => exact_zero(p, AngularCmd::Save, bad),
        OP_RESTORE => exact_zero(p, AngularCmd::Restore, bad),
        OP_BASIS => decode_basis(p, bad),
        OP_FILL_COLOR => {
            if p.len() != 4 {
                return Err(bad);
            }
            Ok(AngularCmd::FillColor {
                color: Rgba8::from_u32(read_u32(&p[0..4])),
            })
        }
        OP_STROKE_MRAD => {
            if p.len() != 8 {
                return Err(bad);
            }
            let width_mrad = read_f32(&p[4..8]);
            if !width_mrad.is_finite() || width_mrad < 0.0 {
                return Err(bad);
            }
            Ok(AngularCmd::StrokeMrad {
                color: Rgba8::from_u32(read_u32(&p[0..4])),
                width_mrad,
            })
        }
        OP_ATTRIBUTE => {
            if p.len() != 1 {
                return Err(bad);
            }
            Ok(AngularCmd::Attribute { group: p[0] })
        }
        OP_TARGET_TAG => {
            if p.len() != 4 {
                return Err(bad);
            }
            Ok(AngularCmd::TargetTag {
                target_id: read_u32(&p[0..4]),
            })
        }
        OP_RAY_LINE | OP_RAY_POLYLINE | OP_GREAT_CIRCLE_ARC | OP_RETICLE_RING => {
            decode_geometry(opcode, p, bad)
        }
        OP_ANGULAR_TEXT => decode_text(p, bad),
        OP_BEGIN_LAYER => {
            if p.len() != 1 {
                return Err(bad);
            }
            Ok(AngularCmd::BeginLayer {
                layer: AngularLayerId::from_u8(p[0]).ok_or(bad)?,
            })
        }
        OP_END_LAYER => {
            if p.len() != 1 {
                return Err(bad);
            }
            Ok(AngularCmd::EndLayer {
                layer: AngularLayerId::from_u8(p[0]).ok_or(bad)?,
            })
        }
        0x52..=0x5F => Err(bad),
        other => Ok(AngularCmd::Unknown { opcode: other }),
    }
}

fn exact_zero<'a>(
    p: &[u8],
    cmd: AngularCmd<'a>,
    bad: DecodeError,
) -> Result<AngularCmd<'a>, DecodeError> {
    if p.is_empty() { Ok(cmd) } else { Err(bad) }
}

fn decode_basis<'a>(p: &'a [u8], bad: DecodeError) -> Result<AngularCmd<'a>, DecodeError> {
    if p.len() != 37 {
        return Err(bad);
    }
    let kind = AngularReferenceKind::from_u8(p[0]).ok_or(bad)?;
    let right = read_vec3(&p[1..13]);
    let up = read_vec3(&p[13..25]);
    let forward = read_vec3(&p[25..37]);
    if !(is_unit_vec3(right) && is_unit_vec3(up) && is_unit_vec3(forward)) {
        return Err(bad);
    }
    Ok(AngularCmd::Basis {
        kind,
        right,
        up,
        forward,
    })
}

fn decode_geometry<'a>(
    opcode: u8,
    p: &'a [u8],
    bad: DecodeError,
) -> Result<AngularCmd<'a>, DecodeError> {
    match opcode {
        OP_RAY_LINE => {
            if p.len() != 24 {
                return Err(bad);
            }
            let a = read_vec3(&p[0..12]);
            let b = read_vec3(&p[12..24]);
            if !(is_unit_vec3(a) && is_unit_vec3(b)) {
                return Err(bad);
            }
            Ok(AngularCmd::RayLine { a, b })
        }
        OP_RAY_POLYLINE => {
            if p.len() < 24 || !p.len().is_multiple_of(12) || p.len() / 12 > MAX_POLYLINE_POINTS {
                return Err(bad);
            }
            let points = Points3Ref::from_bytes(p);
            if !points.iter().all(is_unit_vec3) {
                return Err(bad);
            }
            Ok(AngularCmd::RayPolyline { points })
        }
        OP_GREAT_CIRCLE_ARC => {
            if p.len() != 28 {
                return Err(bad);
            }
            let axis = read_vec3(&p[0..12]);
            let start = read_vec3(&p[12..24]);
            let sweep_rad = read_f32(&p[24..28]);
            if !(is_unit_vec3(axis) && is_unit_vec3(start) && sweep_rad.is_finite()) {
                return Err(bad);
            }
            Ok(AngularCmd::GreatCircleArc {
                axis,
                start,
                sweep_rad,
            })
        }
        _ => {
            if p.len() != 16 {
                return Err(bad);
            }
            let center = read_vec3(&p[0..12]);
            let radius_rad = read_f32(&p[12..16]);
            if !(is_unit_vec3(center) && radius_rad.is_finite() && radius_rad > 0.0) {
                return Err(bad);
            }
            Ok(AngularCmd::ReticleRing { center, radius_rad })
        }
    }
}

fn decode_text<'a>(p: &'a [u8], bad: DecodeError) -> Result<AngularCmd<'a>, DecodeError> {
    if p.len() <= 17 || p.len() - 17 > MAX_TEXT_BYTES {
        return Err(bad);
    }
    let dir = read_vec3(&p[0..12]);
    let size_mrad = read_f32(&p[12..16]);
    if !(is_unit_vec3(dir) && size_mrad.is_finite() && size_mrad > 0.0) {
        return Err(bad);
    }
    let anchor = match p[16] {
        0 => HAlign::Left,
        1 => HAlign::Center,
        2 => HAlign::Right,
        _ => return Err(bad),
    };
    let text = core::str::from_utf8(&p[17..]).map_err(|_| bad)?;
    Ok(AngularCmd::AngularText {
        dir,
        size_mrad,
        anchor,
        text,
    })
}

fn read_vec3(slice: &[u8]) -> [f32; 3] {
    [
        read_f32(&slice[0..4]),
        read_f32(&slice[4..8]),
        read_f32(&slice[8..12]),
    ]
}

fn read_u32(slice: &[u8]) -> u32 {
    let mut b = [0u8; 4];
    b.copy_from_slice(&slice[..4]);
    u32::from_le_bytes(b)
}
