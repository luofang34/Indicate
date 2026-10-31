//! Fail-closed wire codecs for the three volumetric auxiliary channels (`VOL-01`).

use indicate_instrument_descriptor::{AuxChannelId, PanelDrawError};

/// Fixed header length for [`TerrainPatchAux`] (`cols`, `rows`, `meters_per_unit`).
pub const TERRAIN_PATCH_HEADER_BYTES: usize = 10;

/// Fixed byte length for [`GlobeNavigationAux`] (`waypoint_bearing_rad`, `waypoint_range_nm`, `meters_per_unit`).
pub const GLOBE_NAVIGATION_WIRE_BYTES: usize = 16;

/// Fixed byte length for [`CelestialEphemerisAux`] (`inner_radius_m`, `outer_radius_m`, `phase_rad`, `meters_per_unit`).
pub const CELESTIAL_EPHEMERIS_WIRE_BYTES: usize = 20;

/// Decoded auxiliary terrain relief patch (`AuxChannelId::TERRAIN_PATCH_3D`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TerrainPatchAux<'a> {
    /// Grid column count (`2..=16`).
    pub cols: u8,
    /// Grid row count (`2..=16`).
    pub rows: u8,
    /// World meters represented by one local volume meter (`> 0.0`).
    pub meters_per_unit: f64,
    /// Borrowed little-endian `f32` height samples (`cols * rows * 4` bytes).
    raw_heights: &'a [u8],
}

impl<'a> TerrainPatchAux<'a> {
    /// Encodes a 4×4 terrain relief patch into `out` (`74` bytes).
    #[must_use]
    pub fn encode_4x4(meters_per_unit: f64, heights_m: &[f32; 16]) -> [u8; 74] {
        let mut out = [0u8; 74];
        if let Some(hdr) = out.get_mut(..2) {
            hdr.copy_from_slice(&[4, 4]);
        }
        if let Some(mpu) = out.get_mut(2..10) {
            mpu.copy_from_slice(&meters_per_unit.to_le_bytes());
        }
        let mut idx = 0usize;
        while idx < 16 {
            let start = 10usize.wrapping_add(idx.wrapping_mul(4));
            if let (Some(sample), Some(dst)) = (heights_m.get(idx), out.get_mut(start..start + 4)) {
                dst.copy_from_slice(&sample.to_le_bytes());
            }
            idx = idx.wrapping_add(1);
        }
        out
    }

    /// Decodes a [`TerrainPatchAux`] payload fail-closed.
    pub fn decode(bytes: &'a [u8]) -> Result<Self, PanelDrawError> {
        let bad = || PanelDrawError::InvalidAux(AuxChannelId::TERRAIN_PATCH_3D.0);
        if bytes.len() < TERRAIN_PATCH_HEADER_BYTES {
            return Err(bad());
        }
        let cols = *bytes.first().ok_or_else(bad)?;
        let rows = *bytes.get(1).ok_or_else(bad)?;
        if !(2..=16).contains(&cols) || !(2..=16).contains(&rows) {
            return Err(bad());
        }
        let count = usize::from(cols).wrapping_mul(usize::from(rows));
        let expected_bytes = count.wrapping_mul(4);
        let raw_heights = bytes.get(TERRAIN_PATCH_HEADER_BYTES..).ok_or_else(bad)?;
        if raw_heights.len() != expected_bytes {
            return Err(bad());
        }
        let meters_per_unit = read_f64_le(bytes, 2, AuxChannelId::TERRAIN_PATCH_3D)?;
        if !meters_per_unit.is_finite() || meters_per_unit <= 0.0 {
            return Err(bad());
        }
        let mut i = 0usize;
        while i < count {
            let v = read_f32_le(
                raw_heights,
                i.wrapping_mul(4),
                AuxChannelId::TERRAIN_PATCH_3D,
            )?;
            if !v.is_finite() {
                return Err(bad());
            }
            i = i.wrapping_add(1);
        }
        Ok(Self {
            cols,
            rows,
            meters_per_unit,
            raw_heights,
        })
    }

    /// Reads sample `index` (`0..cols*rows`) in local volume height units.
    #[must_use]
    pub fn sample(&self, index: usize) -> Option<f32> {
        read_f32_le(
            self.raw_heights,
            index.wrapping_mul(4),
            AuxChannelId::TERRAIN_PATCH_3D,
        )
        .ok()
    }
}

/// Decoded auxiliary planetary navigation overlay (`AuxChannelId::GLOBE_NAVIGATION_3D`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlobeNavigationAux {
    /// Active waypoint great-circle bearing in radians.
    pub waypoint_bearing_rad: f32,
    /// Active waypoint great-circle distance in nautical miles (`>= 0.0`).
    pub waypoint_range_nm: f32,
    /// World meters represented by one local volume meter (`> 0.0`).
    pub meters_per_unit: f64,
}

impl GlobeNavigationAux {
    /// Encodes the 16-byte `GLOBE_NAVIGATION_3D` wire payload.
    #[must_use]
    pub fn encode(&self) -> [u8; GLOBE_NAVIGATION_WIRE_BYTES] {
        let mut out = [0u8; GLOBE_NAVIGATION_WIRE_BYTES];
        if let Some(s) = out.get_mut(0..4) {
            s.copy_from_slice(&self.waypoint_bearing_rad.to_le_bytes());
        }
        if let Some(s) = out.get_mut(4..8) {
            s.copy_from_slice(&self.waypoint_range_nm.to_le_bytes());
        }
        if let Some(s) = out.get_mut(8..16) {
            s.copy_from_slice(&self.meters_per_unit.to_le_bytes());
        }
        out
    }

    /// Decodes the 16-byte `GLOBE_NAVIGATION_3D` wire payload fail-closed.
    pub fn decode(bytes: &[u8]) -> Result<Self, PanelDrawError> {
        let ch = AuxChannelId::GLOBE_NAVIGATION_3D;
        if bytes.len() != GLOBE_NAVIGATION_WIRE_BYTES {
            return Err(PanelDrawError::InvalidAux(ch.0));
        }
        let waypoint_bearing_rad = read_f32_le(bytes, 0, ch)?;
        let waypoint_range_nm = read_f32_le(bytes, 4, ch)?;
        let meters_per_unit = read_f64_le(bytes, 8, ch)?;
        if !waypoint_bearing_rad.is_finite()
            || !waypoint_range_nm.is_finite()
            || waypoint_range_nm < 0.0
            || !meters_per_unit.is_finite()
            || meters_per_unit <= 0.0
        {
            return Err(PanelDrawError::InvalidAux(ch.0));
        }
        Ok(Self {
            waypoint_bearing_rad,
            waypoint_range_nm,
            meters_per_unit,
        })
    }
}

/// Decoded auxiliary celestial ephemeris (`AuxChannelId::CELESTIAL_EPHEMERIS_3D`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CelestialEphemerisAux {
    /// Inner orbit radius in normalized volume fraction (`0.05..=0.45`).
    pub inner_orbit_frac: f32,
    /// Outer orbit radius in normalized volume fraction (`0.10..=0.48`).
    pub outer_orbit_frac: f32,
    /// Reference epoch orbital phase in radians.
    pub phase_rad: f32,
    /// World meters represented by one local volume meter (`> 0.0`).
    pub meters_per_unit: f64,
}

impl CelestialEphemerisAux {
    /// Encodes the 20-byte `CELESTIAL_EPHEMERIS_3D` wire payload.
    #[must_use]
    pub fn encode(&self) -> [u8; CELESTIAL_EPHEMERIS_WIRE_BYTES] {
        let mut out = [0u8; CELESTIAL_EPHEMERIS_WIRE_BYTES];
        if let Some(s) = out.get_mut(0..4) {
            s.copy_from_slice(&self.inner_orbit_frac.to_le_bytes());
        }
        if let Some(s) = out.get_mut(4..8) {
            s.copy_from_slice(&self.outer_orbit_frac.to_le_bytes());
        }
        if let Some(s) = out.get_mut(8..12) {
            s.copy_from_slice(&self.phase_rad.to_le_bytes());
        }
        if let Some(s) = out.get_mut(12..20) {
            s.copy_from_slice(&self.meters_per_unit.to_le_bytes());
        }
        out
    }

    /// Decodes the 20-byte `CELESTIAL_EPHEMERIS_3D` wire payload fail-closed.
    pub fn decode(bytes: &[u8]) -> Result<Self, PanelDrawError> {
        let ch = AuxChannelId::CELESTIAL_EPHEMERIS_3D;
        if bytes.len() != CELESTIAL_EPHEMERIS_WIRE_BYTES {
            return Err(PanelDrawError::InvalidAux(ch.0));
        }
        let inner_orbit_frac = read_f32_le(bytes, 0, ch)?;
        let outer_orbit_frac = read_f32_le(bytes, 4, ch)?;
        let phase_rad = read_f32_le(bytes, 8, ch)?;
        let meters_per_unit = read_f64_le(bytes, 12, ch)?;
        if !inner_orbit_frac.is_finite()
            || !outer_orbit_frac.is_finite()
            || !phase_rad.is_finite()
            || !(0.05..=0.45).contains(&inner_orbit_frac)
            || !(0.10..=0.48).contains(&outer_orbit_frac)
            || inner_orbit_frac >= outer_orbit_frac
            || !meters_per_unit.is_finite()
            || meters_per_unit <= 0.0
        {
            return Err(PanelDrawError::InvalidAux(ch.0));
        }
        Ok(Self {
            inner_orbit_frac,
            outer_orbit_frac,
            phase_rad,
            meters_per_unit,
        })
    }
}

fn read_f32_le(bytes: &[u8], offset: usize, ch: AuxChannelId) -> Result<f32, PanelDrawError> {
    let end = offset
        .checked_add(4)
        .ok_or(PanelDrawError::InvalidAux(ch.0))?;
    let slice = bytes
        .get(offset..end)
        .ok_or(PanelDrawError::InvalidAux(ch.0))?;
    let arr: [u8; 4] = slice
        .try_into()
        .map_err(|_| PanelDrawError::InvalidAux(ch.0))?;
    Ok(f32::from_le_bytes(arr))
}

fn read_f64_le(bytes: &[u8], offset: usize, ch: AuxChannelId) -> Result<f64, PanelDrawError> {
    let end = offset
        .checked_add(8)
        .ok_or(PanelDrawError::InvalidAux(ch.0))?;
    let slice = bytes
        .get(offset..end)
        .ok_or(PanelDrawError::InvalidAux(ch.0))?;
    let arr: [u8; 8] = slice
        .try_into()
        .map_err(|_| PanelDrawError::InvalidAux(ch.0))?;
    Ok(f64::from_le_bytes(arr))
}
