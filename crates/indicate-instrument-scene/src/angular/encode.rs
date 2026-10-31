//! Allocation-free encoder for [`crate::angular`] scenes.

use crate::cmd::{HAlign, MAX_TEXT_BYTES};
use crate::color::Rgba8;
use crate::encode::SceneError;

use super::{
    ANGULAR_SCENE_VERSION, AngularLayerId, AngularReferenceKind, MAX_POLYLINE_POINTS,
    OP_ANGULAR_TEXT, OP_ATTRIBUTE, OP_BASIS, OP_BEGIN_LAYER, OP_END_LAYER, OP_FILL_COLOR,
    OP_GREAT_CIRCLE_ARC, OP_RAY_LINE, OP_RAY_POLYLINE, OP_RESTORE, OP_RETICLE_RING, OP_SAVE,
    OP_STROKE_MRAD, OP_TARGET_TAG, is_unit_vec3,
};

/// Encodes conformal unit-sphere commands into a caller-provided byte slice.
#[derive(Debug)]
pub struct AngularSceneWriter<'a> {
    buf: &'a mut [u8],
    len: usize,
}

impl<'a> AngularSceneWriter<'a> {
    /// Initializes an angular scene in `buf`, writing [`ANGULAR_SCENE_VERSION`].
    pub fn new(buf: &'a mut [u8]) -> Result<Self, SceneError> {
        let mut writer = Self { buf, len: 0 };
        writer.put(&[ANGULAR_SCENE_VERSION])?;
        Ok(writer)
    }

    /// Number of bytes written so far.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Whether nothing beyond the version byte has been written.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len <= 1
    }

    /// Completes encoding and returns the total written byte count.
    #[must_use]
    pub const fn finish(self) -> usize {
        self.len
    }

    fn put(&mut self, bytes: &[u8]) -> Result<(), SceneError> {
        let end = self
            .len
            .checked_add(bytes.len())
            .ok_or(SceneError::BufferFull)?;
        let dst = self
            .buf
            .get_mut(self.len..end)
            .ok_or(SceneError::BufferFull)?;
        dst.copy_from_slice(bytes);
        self.len = end;
        Ok(())
    }

    fn cmd(&mut self, op: u8, parts: &[&[u8]]) -> Result<(), SceneError> {
        let total: usize = parts.iter().map(|p| p.len()).sum();
        let plen = u16::try_from(total).map_err(|_| SceneError::TooManyPoints)?;
        let rollback = self.len;
        let res = self.write_parts(op, plen, parts);
        if res.is_err() {
            self.len = rollback;
        }
        res
    }

    fn write_parts(&mut self, op: u8, plen: u16, parts: &[&[u8]]) -> Result<(), SceneError> {
        self.put(&[op])?;
        self.put(&plen.to_le_bytes())?;
        for part in parts {
            self.put(part)?;
        }
        Ok(())
    }

    /// Pushes graphics state (`0x01`).
    pub fn save(&mut self) -> Result<(), SceneError> {
        self.cmd(OP_SAVE, &[])
    }

    /// Pops graphics state (`0x02`).
    pub fn restore(&mut self) -> Result<(), SceneError> {
        self.cmd(OP_RESTORE, &[])
    }

    /// Sets the active unit-vector reference basis (`0x03`).
    pub fn basis(
        &mut self,
        kind: AngularReferenceKind,
        right: [f32; 3],
        up: [f32; 3],
        forward: [f32; 3],
    ) -> Result<(), SceneError> {
        if !(is_unit_vec3(right) && is_unit_vec3(up) && is_unit_vec3(forward)) {
            return Err(SceneError::InvalidGeometry);
        }
        let r = vec3_bytes(right);
        let u = vec3_bytes(up);
        let f = vec3_bytes(forward);
        self.cmd(OP_BASIS, &[&[kind.to_u8()], &r, &u, &f])
    }

    /// Sets fill color (`0x10`).
    pub fn fill_color(&mut self, color: Rgba8) -> Result<(), SceneError> {
        self.cmd(OP_FILL_COLOR, &[&color.to_u32().to_le_bytes()])
    }

    /// Sets stroke color and milliradian width (`0x11`).
    pub fn stroke_mrad(&mut self, color: Rgba8, width_mrad: f32) -> Result<(), SceneError> {
        if !width_mrad.is_finite() || width_mrad < 0.0 {
            return Err(SceneError::InvalidGeometry);
        }
        self.cmd(
            OP_STROKE_MRAD,
            &[&color.to_u32().to_le_bytes(), &width_mrad.to_le_bytes()],
        )
    }

    /// Declares provenance group for the following text command (`0x12`).
    pub fn attribute(&mut self, group: u8) -> Result<(), SceneError> {
        self.cmd(OP_ATTRIBUTE, &[&[group]])
    }

    /// Sets active interactive target tag (`0x13`).
    pub fn target_tag(&mut self, target_id: u32) -> Result<(), SceneError> {
        self.cmd(OP_TARGET_TAG, &[&target_id.to_le_bytes()])
    }

    /// Emits a unit-sphere line segment (`0x20`).
    pub fn ray_line(&mut self, a: [f32; 3], b: [f32; 3]) -> Result<(), SceneError> {
        if !(is_unit_vec3(a) && is_unit_vec3(b)) {
            return Err(SceneError::InvalidGeometry);
        }
        let ab = vec3_bytes(a);
        let bb = vec3_bytes(b);
        self.cmd(OP_RAY_LINE, &[&ab, &bb])
    }

    /// Emits a unit-sphere polyline of `2..=512` vertices (`0x21`).
    pub fn ray_polyline(&mut self, points: &[[f32; 3]]) -> Result<(), SceneError> {
        if points.len() < 2 || points.len() > MAX_POLYLINE_POINTS {
            return Err(SceneError::TooManyPoints);
        }
        if !points.iter().copied().all(is_unit_vec3) {
            return Err(SceneError::InvalidGeometry);
        }
        let plen = u16::try_from(points.len() * 12).map_err(|_| SceneError::TooManyPoints)?;
        let rollback = self.len;
        let res = self.write_polyline(plen, points);
        if res.is_err() {
            self.len = rollback;
        }
        res
    }

    fn write_polyline(&mut self, plen: u16, points: &[[f32; 3]]) -> Result<(), SceneError> {
        self.put(&[OP_RAY_POLYLINE])?;
        self.put(&plen.to_le_bytes())?;
        for &pt in points {
            self.put(&vec3_bytes(pt))?;
        }
        Ok(())
    }

    /// Emits a great-circle arc (`0x22`).
    pub fn great_circle_arc(
        &mut self,
        axis: [f32; 3],
        start: [f32; 3],
        sweep_rad: f32,
    ) -> Result<(), SceneError> {
        if !(is_unit_vec3(axis) && is_unit_vec3(start) && sweep_rad.is_finite()) {
            return Err(SceneError::InvalidGeometry);
        }
        let ax = vec3_bytes(axis);
        let st = vec3_bytes(start);
        self.cmd(OP_GREAT_CIRCLE_ARC, &[&ax, &st, &sweep_rad.to_le_bytes()])
    }

    /// Emits a reticle ring around unit `center` (`0x23`).
    pub fn reticle_ring(&mut self, center: [f32; 3], radius_rad: f32) -> Result<(), SceneError> {
        if !(is_unit_vec3(center) && radius_rad.is_finite() && radius_rad > 0.0) {
            return Err(SceneError::InvalidGeometry);
        }
        let c = vec3_bytes(center);
        self.cmd(OP_RETICLE_RING, &[&c, &radius_rad.to_le_bytes()])
    }

    /// Emits billboarded angular text (`0x30`).
    pub fn angular_text(
        &mut self,
        dir: [f32; 3],
        size_mrad: f32,
        anchor: HAlign,
        text: &str,
    ) -> Result<(), SceneError> {
        if text.is_empty() {
            return Err(SceneError::InvalidGeometry);
        }
        if text.len() > MAX_TEXT_BYTES {
            return Err(SceneError::TextTooLong);
        }
        if !(is_unit_vec3(dir) && size_mrad.is_finite() && size_mrad > 0.0) {
            return Err(SceneError::InvalidGeometry);
        }
        let d = vec3_bytes(dir);
        let a = encode_halign(anchor);
        self.cmd(
            OP_ANGULAR_TEXT,
            &[&d, &size_mrad.to_le_bytes(), &[a], text.as_bytes()],
        )
    }

    /// Emits attributed angular text (`ATTRIBUTE` followed by `ANGULAR_TEXT`).
    pub fn attributed_angular_text(
        &mut self,
        group: u8,
        dir: [f32; 3],
        size_mrad: f32,
        anchor: HAlign,
        text: &str,
    ) -> Result<(), SceneError> {
        let rollback = self.len;
        self.attribute(group)?;
        if let Err(err) = self.angular_text(dir, size_mrad, anchor, text) {
            self.len = rollback;
            return Err(err);
        }
        Ok(())
    }

    /// Opens `layer` and writes the mandatory outer `SAVE` (`0x50` + `0x01`).
    pub fn begin_layer(&mut self, layer: AngularLayerId) -> Result<(), SceneError> {
        let rollback = self.len;
        self.cmd(OP_BEGIN_LAYER, &[&[layer.to_u8()]])?;
        if let Err(err) = self.save() {
            self.len = rollback;
            return Err(err);
        }
        Ok(())
    }

    /// Writes the mandatory outer `RESTORE` and closes `layer` (`0x02` + `0x51`).
    pub fn end_layer(&mut self, layer: AngularLayerId) -> Result<(), SceneError> {
        let rollback = self.len;
        self.restore()?;
        if let Err(err) = self.cmd(OP_END_LAYER, &[&[layer.to_u8()]]) {
            self.len = rollback;
            return Err(err);
        }
        Ok(())
    }
}

const fn encode_halign(anchor: HAlign) -> u8 {
    match anchor {
        HAlign::Left => 0,
        HAlign::Center => 1,
        HAlign::Right => 2,
    }
}

fn vec3_bytes(v: [f32; 3]) -> [u8; 12] {
    let mut out = [0u8; 12];
    out[0..4].copy_from_slice(&v[0].to_le_bytes());
    out[4..8].copy_from_slice(&v[1].to_le_bytes());
    out[8..12].copy_from_slice(&v[2].to_le_bytes());
    out
}
