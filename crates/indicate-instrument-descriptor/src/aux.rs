//! Auxiliary domain channel identifiers and entries delivered in [`crate::DrawContext`].

/// Maximum number of auxiliary entries allowed in one [`crate::DrawContext`] frame.
pub const MAX_AUX_ENTRIES: usize = 16;

/// Identifier of an auxiliary domain payload channel delivered alongside flight state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AuxChannelId(pub u32);

impl AuxChannelId {
    /// Head-worn display posture, gaze, and presentation mode (`0x0001`).
    pub const HWD_DISPLAY: Self = Self(0x0001);
    /// Head-up display combiner boresight and eye-box context (`0x0002`).
    pub const HUD_CONTEXT: Self = Self(0x0002);
    /// Metric 3D terrain elevation patch and coverage metadata (`0x0010`).
    pub const TERRAIN_PATCH_3D: Self = Self(0x0010);
    /// Planetary 3D globe waypoints, track, and anchor metadata (`0x0011`).
    pub const GLOBE_NAVIGATION_3D: Self = Self(0x0011);
    /// Celestial 3D ephemeris and orbital trajectory metadata (`0x0012`).
    pub const CELESTIAL_EPHEMERIS_3D: Self = Self(0x0012);
    /// Electronic flight-bag navigation route/chart channel (`0x0020`).
    pub const FLIGHTBAG_NAV: Self = Self(0x0020);
    /// World airmass and weather hazard channel (`0x0021`).
    pub const WORLD_AIRMASS: Self = Self(0x0021);
    /// World traffic/surveillance channel (`0x0022`).
    pub const WORLD_SURVEILLANCE: Self = Self(0x0022);
    /// World operational notices channel (`0x0023`).
    pub const WORLD_NOTICES: Self = Self(0x0023);

    /// Derives a deterministic custom channel identifier with bit 31 set.
    #[must_use]
    pub const fn custom(domain: &str) -> Self {
        custom_aux_id(domain)
    }

    /// Canonical symbolic name for well-known channels, or `None` for custom IDs.
    #[must_use]
    pub const fn well_known_name(self) -> Option<&'static str> {
        match self.0 {
            0x0001 => Some("HWD_DISPLAY"),
            0x0002 => Some("HUD_CONTEXT"),
            0x0010 => Some("TERRAIN_PATCH_3D"),
            0x0011 => Some("GLOBE_NAVIGATION_3D"),
            0x0012 => Some("CELESTIAL_EPHEMERIS_3D"),
            0x0020 => Some("FLIGHTBAG_NAV"),
            0x0021 => Some("WORLD_AIRMASS"),
            0x0022 => Some("WORLD_SURVEILLANCE"),
            0x0023 => Some("WORLD_NOTICES"),
            _ => None,
        }
    }
}

/// Deterministic 32-bit FNV-1a custom auxiliary channel identifier (`| 0x8000_0000`).
#[must_use]
pub const fn custom_aux_id(domain: &str) -> AuxChannelId {
    let bytes = domain.as_bytes();
    let mut hash: u32 = 0x811c_9dc5;
    let mut i = 0;
    while i < bytes.len() {
        hash ^= bytes[i] as u32;
        hash = hash.wrapping_mul(0x0100_0193);
        i = i.wrapping_add(1);
    }
    AuxChannelId(hash | 0x8000_0000)
}

/// Borrowed auxiliary channel payload carried in a [`crate::DrawContext`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuxEntry<'a> {
    /// Channel identifier.
    pub id: AuxChannelId,
    /// Raw channel payload bytes.
    pub payload: &'a [u8],
}
