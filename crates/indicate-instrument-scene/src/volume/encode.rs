//! Allocation-free encoder for [`crate::volume`] scenes.

use indicate_frames::Quat;

use crate::cmd::{HAlign, MAX_TEXT_BYTES, PaintMode};
use crate::color::Rgba8;
use crate::encode::SceneError;

use super::{
    HandleKind, MAX_HEIGHTFIELD_SAMPLES, MAX_POLYLINE_POINTS, OP_ATTRIBUTE, OP_BEGIN_BAND,
    OP_BILLBOARD_TEXT, OP_BOX_3D, OP_CLIP_BOX_3D, OP_CYLINDER_3D, OP_END_BAND, OP_FILL_COLOR,
    OP_HEIGHTFIELD_GRID, OP_LINE_3D, OP_POLYLINE_3D, OP_RESTORE, OP_RING_3D, OP_ROTATE_QUAT,
    OP_SAVE, OP_SPHERE, OP_STROKE_3D, OP_TARGET_HANDLE, OP_TRANSLATE_3D, OP_UNDERLAY_ANCHOR_3D,
    VOLUME_SCENE_VERSION, VolumeBandId, VolumeUnderlayKind, is_finite_vec3, is_ordered_box,
    is_unit_quat, is_unit_vec3,
};

/// Encodes bounded 3D volumetric commands into a caller-provided byte slice.
#[derive(Debug)]
pub struct VolumeSceneWriter<'a> {
    buf: &'a mut [u8],
    len: usize,
}

impl<'a> VolumeSceneWriter<'a> {
    /// Initializes a 3D volume scene in `buf`, writing [`VOLUME_SCENE_VERSION`].
    pub fn new(buf: &'a mut [u8]) -> Result<Self, SceneError> {
        let mut writer = Self { buf, len: 0 };
        writer.put(&[VOLUME_SCENE_VERSION])?;
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

    /// Pushes 3D graphics state (`0x01`).
    pub fn save(&mut self) -> Result<(), SceneError> {
        self.cmd(OP_SAVE, &[])
    }

    /// Pops 3D graphics state (`0x02`).
    pub fn restore(&mut self) -> Result<(), SceneError> {
        self.cmd(OP_RESTORE, &[])
    }

    /// Translates the 3D frame by `delta` meters (`0x03`).
    pub fn translate_3d(&mut self, delta: [f32; 3]) -> Result<(), SceneError> {
        if !is_finite_vec3(delta) {
            return Err(SceneError::InvalidGeometry);
        }
        self.cmd(OP_TRANSLATE_3D, &[&vec3_bytes(delta)])
    }

    /// Rotates the 3D frame by unit quaternion `rotation` (`0x04`).
    pub fn rotate_quat(&mut self, rotation: Quat) -> Result<(), SceneError> {
        if !is_unit_quat(rotation) {
            return Err(SceneError::InvalidGeometry);
        }
        self.cmd(OP_ROTATE_QUAT, &[&quat_bytes(rotation)])
    }

    /// Intersects the active 3D clip volume with `[min, max]` (`0x05`).
    pub fn clip_box_3d(&mut self, min: [f32; 3], max: [f32; 3]) -> Result<(), SceneError> {
        if !is_ordered_box(min, max) {
            return Err(SceneError::InvalidGeometry);
        }
        let a = vec3_bytes(min);
        let b = vec3_bytes(max);
        self.cmd(OP_CLIP_BOX_3D, &[&a, &b])
    }

    /// Sets fill color (`0x10`).
    pub fn fill_color(&mut self, color: Rgba8) -> Result<(), SceneError> {
        self.cmd(OP_FILL_COLOR, &[&color.to_u32().to_le_bytes()])
    }

    /// Sets 3D stroke color and tube radius in meters (`0x11`).
    pub fn stroke_3d(&mut self, color: Rgba8, radius_m: f32) -> Result<(), SceneError> {
        if !radius_m.is_finite() || radius_m < 0.0 {
            return Err(SceneError::InvalidGeometry);
        }
        self.cmd(
            OP_STROKE_3D,
            &[&color.to_u32().to_le_bytes(), &radius_m.to_le_bytes()],
        )
    }

    /// Declares state-group provenance for the next billboard text (`0x12`).
    pub fn attribute(&mut self, group: u8) -> Result<(), SceneError> {
        self.cmd(OP_ATTRIBUTE, &[&[group]])
    }

    /// Emits an interactive 3D hit handle (`0x13`).
    pub fn target_handle(
        &mut self,
        target_id: u32,
        kind: HandleKind,
        center: [f32; 3],
        radius_m: f32,
    ) -> Result<(), SceneError> {
        if target_id == 0 || !is_finite_vec3(center) || !radius_m.is_finite() || radius_m <= 0.0 {
            return Err(SceneError::InvalidGeometry);
        }
        let c = vec3_bytes(center);
        self.cmd(
            OP_TARGET_HANDLE,
            &[
                &target_id.to_le_bytes(),
                &[kind.to_u8()],
                &c,
                &radius_m.to_le_bytes(),
            ],
        )
    }

    /// Emits a 3D line segment (`0x20`).
    pub fn line_3d(&mut self, a: [f32; 3], b: [f32; 3]) -> Result<(), SceneError> {
        if !(is_finite_vec3(a) && is_finite_vec3(b)) {
            return Err(SceneError::InvalidGeometry);
        }
        let ab = vec3_bytes(a);
        let bb = vec3_bytes(b);
        self.cmd(OP_LINE_3D, &[&ab, &bb])
    }

    /// Emits a 3D polyline of `2..=512` vertices (`0x21`).
    pub fn polyline_3d(&mut self, points: &[[f32; 3]]) -> Result<(), SceneError> {
        if points.len() < 2 || points.len() > MAX_POLYLINE_POINTS {
            return Err(SceneError::TooManyPoints);
        }
        if !points.iter().copied().all(is_finite_vec3) {
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
        self.put(&[OP_POLYLINE_3D])?;
        self.put(&plen.to_le_bytes())?;
        for &pt in points {
            self.put(&vec3_bytes(pt))?;
        }
        Ok(())
    }

    /// Emits a sphere primitive (`0x23`).
    pub fn sphere(
        &mut self,
        mode: PaintMode,
        center: [f32; 3],
        radius_m: f32,
    ) -> Result<(), SceneError> {
        if !is_finite_vec3(center) || !radius_m.is_finite() || radius_m <= 0.0 {
            return Err(SceneError::InvalidGeometry);
        }
        let c = vec3_bytes(center);
        self.cmd(
            OP_SPHERE,
            &[&[encode_mode(mode)], &c, &radius_m.to_le_bytes()],
        )
    }

    /// Emits an axis-aligned 3D box (`0x24`).
    pub fn box_3d(
        &mut self,
        mode: PaintMode,
        min: [f32; 3],
        max: [f32; 3],
    ) -> Result<(), SceneError> {
        if !is_ordered_box(min, max) {
            return Err(SceneError::InvalidGeometry);
        }
        let a = vec3_bytes(min);
        let b = vec3_bytes(max);
        self.cmd(OP_BOX_3D, &[&[encode_mode(mode)], &a, &b])
    }

    /// Emits a cylinder between `base` and `top` (`0x25`).
    pub fn cylinder_3d(
        &mut self,
        mode: PaintMode,
        base: [f32; 3],
        top: [f32; 3],
        radius_m: f32,
    ) -> Result<(), SceneError> {
        if !(is_finite_vec3(base) && is_finite_vec3(top) && radius_m.is_finite() && radius_m > 0.0)
        {
            return Err(SceneError::InvalidGeometry);
        }
        let b = vec3_bytes(base);
        let t = vec3_bytes(top);
        self.cmd(
            OP_CYLINDER_3D,
            &[&[encode_mode(mode)], &b, &t, &radius_m.to_le_bytes()],
        )
    }

    /// Emits a planar 3D ring (`0x26`).
    pub fn ring_3d(
        &mut self,
        center: [f32; 3],
        normal: [f32; 3],
        radius_m: f32,
    ) -> Result<(), SceneError> {
        if !(is_finite_vec3(center)
            && is_unit_vec3(normal)
            && radius_m.is_finite()
            && radius_m > 0.0)
        {
            return Err(SceneError::InvalidGeometry);
        }
        let c = vec3_bytes(center);
        let n = vec3_bytes(normal);
        self.cmd(OP_RING_3D, &[&c, &n, &radius_m.to_le_bytes()])
    }

    /// Emits a `cols × rows` elevation grid (`0x27`).
    pub fn heightfield_grid(
        &mut self,
        cols: u8,
        rows: u8,
        origin: [f32; 3],
        step_x: f32,
        step_z: f32,
        heights: &[f32],
    ) -> Result<(), SceneError> {
        let count = usize::from(cols) * usize::from(rows);
        if cols < 2
            || rows < 2
            || count > MAX_HEIGHTFIELD_SAMPLES
            || heights.len() != count
            || !is_finite_vec3(origin)
            || !step_x.is_finite()
            || step_x <= 0.0
            || !step_z.is_finite()
            || step_z <= 0.0
            || !heights.iter().all(|h| h.is_finite())
        {
            return Err(SceneError::InvalidGeometry);
        }
        let plen = u16::try_from(22 + count * 4).map_err(|_| SceneError::TooManyPoints)?;
        let rollback = self.len;
        let res = self.write_grid(plen, cols, rows, origin, step_x, step_z, heights);
        if res.is_err() {
            self.len = rollback;
        }
        res
    }

    #[allow(clippy::too_many_arguments)]
    fn write_grid(
        &mut self,
        plen: u16,
        cols: u8,
        rows: u8,
        origin: [f32; 3],
        step_x: f32,
        step_z: f32,
        heights: &[f32],
    ) -> Result<(), SceneError> {
        self.put(&[OP_HEIGHTFIELD_GRID])?;
        self.put(&plen.to_le_bytes())?;
        self.put(&[cols, rows])?;
        self.put(&vec3_bytes(origin))?;
        self.put(&step_x.to_le_bytes())?;
        self.put(&step_z.to_le_bytes())?;
        for &h in heights {
            self.put(&h.to_le_bytes())?;
        }
        Ok(())
    }

    /// Emits a host 3D underlay registration anchor (`0x28`).
    pub fn underlay_anchor_3d(
        &mut self,
        kind: VolumeUnderlayKind,
        center: [f32; 3],
        extent: [f32; 3],
        orientation: Quat,
        meters_per_unit: f64,
    ) -> Result<(), SceneError> {
        if !(is_finite_vec3(center)
            && is_finite_vec3(extent)
            && extent[0] > 0.0
            && extent[1] > 0.0
            && extent[2] > 0.0
            && is_unit_quat(orientation)
            && meters_per_unit.is_finite()
            && meters_per_unit > 0.0)
        {
            return Err(SceneError::InvalidGeometry);
        }
        let c = vec3_bytes(center);
        let e = vec3_bytes(extent);
        let q = quat_bytes(orientation);
        self.cmd(
            OP_UNDERLAY_ANCHOR_3D,
            &[&[kind.to_u8()], &c, &e, &q, &meters_per_unit.to_le_bytes()],
        )
    }

    /// Emits camera-facing 3D billboard text (`0x30`).
    pub fn billboard_text(
        &mut self,
        pos: [f32; 3],
        size_m: f32,
        anchor: HAlign,
        text: &str,
    ) -> Result<(), SceneError> {
        if text.is_empty() {
            return Err(SceneError::InvalidGeometry);
        }
        if text.len() > MAX_TEXT_BYTES {
            return Err(SceneError::TextTooLong);
        }
        if !(is_finite_vec3(pos) && size_m.is_finite() && size_m > 0.0) {
            return Err(SceneError::InvalidGeometry);
        }
        let p = vec3_bytes(pos);
        self.cmd(
            OP_BILLBOARD_TEXT,
            &[
                &p,
                &size_m.to_le_bytes(),
                &[encode_halign(anchor)],
                text.as_bytes(),
            ],
        )
    }

    /// Emits attributed 3D billboard text (`ATTRIBUTE` followed by `BILLBOARD_TEXT`).
    pub fn attributed_billboard_text(
        &mut self,
        group: u8,
        pos: [f32; 3],
        size_m: f32,
        anchor: HAlign,
        text: &str,
    ) -> Result<(), SceneError> {
        let rollback = self.len;
        self.attribute(group)?;
        if let Err(err) = self.billboard_text(pos, size_m, anchor, text) {
            self.len = rollback;
            return Err(err);
        }
        Ok(())
    }

    /// Opens `band` and writes the mandatory outer `SAVE` (`0x50` + `0x01`).
    pub fn begin_band(&mut self, band: VolumeBandId) -> Result<(), SceneError> {
        let rollback = self.len;
        self.cmd(OP_BEGIN_BAND, &[&[band.to_u8()]])?;
        if let Err(err) = self.save() {
            self.len = rollback;
            return Err(err);
        }
        Ok(())
    }

    /// Writes the mandatory outer `RESTORE` and closes `band` (`0x02` + `0x51`).
    pub fn end_band(&mut self, band: VolumeBandId) -> Result<(), SceneError> {
        let rollback = self.len;
        self.restore()?;
        if let Err(err) = self.cmd(OP_END_BAND, &[&[band.to_u8()]]) {
            self.len = rollback;
            return Err(err);
        }
        Ok(())
    }
}

const fn encode_mode(mode: PaintMode) -> u8 {
    match mode {
        PaintMode::Fill => 0,
        PaintMode::Stroke => 1,
        PaintMode::FillStroke => 2,
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

fn quat_bytes(q: Quat) -> [u8; 16] {
    let mut out = [0u8; 16];
    out[0..4].copy_from_slice(&q.x.to_le_bytes());
    out[4..8].copy_from_slice(&q.y.to_le_bytes());
    out[8..12].copy_from_slice(&q.z.to_le_bytes());
    out[12..16].copy_from_slice(&q.w.to_le_bytes());
    out
}
