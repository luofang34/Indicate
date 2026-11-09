//! Panel configuration as a bounded key-TLV blob (ADR-0033).
//!
//! Configuration crosses the wasm/FFI boundary as one byte slice, so a
//! new panel option never grows a per-panel binding surface. The wire is
//! a sequence of `[key u16 LE][len u16 LE][payload]` entries in strictly
//! ascending key order, at most [`CONFIG_BLOB_MAX`] bytes total. A shell
//! introspects a panel's declared schema first and refuses a blob
//! carrying any key outside it — unknown configuration is rejected, not
//! skipped, because silently ignoring an option a caller believes is set
//! misstates what the panel displays.

/// Maximum encoded size of a configuration blob.
pub const CONFIG_BLOB_MAX: usize = 256;

/// Wire byte length of [`keys::V_SPEEDS`] when carrying only the five arc-limit speeds.
pub const V_SPEEDS_BANDS_BYTES: usize = 20;

/// Wire byte length of [`keys::V_SPEEDS`] when carrying both the five arc-limit
/// speeds and the six optional reference bugs (`[vs0, vs, vfe, vno, vne, vr, vx, vy, vg, va, vref]`).
pub const V_SPEEDS_EXTENDED_BYTES: usize = 44;

/// A configuration key. Well-known assignments live in [`keys`];
/// out-of-repo panels take keys from `0x8000` up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ConfigKey(pub u16);

/// Well-known configuration keys.
pub mod keys {
    use super::ConfigKey;

    /// PFD background selection: `[0]` horizon, `[1]` none (compositor
    /// band), `[2]` synthetic vision (accept-and-cede until the SVS
    /// renderer lands).
    pub const BACKGROUND_MODE: ConfigKey = ConfigKey(0x0001);
    /// Airspeed color bands (`20 B`: five `f32` LE `vs0, vs, vfe, vno, vne` knots)
    /// or bands plus reference speed bugs (`44 B`: eleven `f32` LE appending
    /// `vr, vx, vy, vg, va, vref`, with `NaN` encoding an absent bug).
    pub const V_SPEEDS: ConfigKey = ConfigKey(0x0002);
    /// SVS viewport within the design frame: four `f32` LE (x, y,
    /// width, height).
    pub const SVS_VIEWPORT: ConfigKey = ConfigKey(0x0003);
    /// SVS quality tier: one byte.
    pub const SVS_QUALITY: ConfigKey = ConfigKey(0x0004);
}

/// Why a configuration blob was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ConfigError {
    /// The blob exceeds [`CONFIG_BLOB_MAX`].
    #[error("config blob of {len} bytes exceeds the {CONFIG_BLOB_MAX}-byte bound")]
    TooLong {
        /// The offending encoded length.
        len: usize,
    },
    /// An entry header or payload runs past the end of the blob.
    #[error("config blob truncated inside the entry with key {key}")]
    Truncated {
        /// The key whose entry is cut short; `0` when truncation lands
        /// before the key itself is readable.
        key: u16,
    },
    /// Keys are not strictly ascending (order is the dedup guarantee).
    #[error("config key {key} repeats or descends")]
    KeysNotAscending {
        /// The out-of-order key.
        key: u16,
    },
    /// A key outside the consulted schema.
    #[error("config key {key} is not in this panel's schema")]
    UnknownKey {
        /// The refused key.
        key: u16,
    },
    /// A known key whose payload cannot mean anything.
    #[error("config key {key} carries an uninterpretable {len}-byte value")]
    BadValue {
        /// The key with the bad payload.
        key: u16,
        /// The payload length delivered.
        len: usize,
    },
    /// A key that is set but meaningless under another key's setting —
    /// refused rather than silently ignored, so a caller cannot believe
    /// an option is in effect when it is not.
    #[error("config key {key} is set but inert under the selected mode")]
    InertKey {
        /// The inert key.
        key: u16,
    },
}

/// A validated configuration blob; construction proves the framing.
#[derive(Debug, Clone, Copy)]
pub struct ConfigBlob<'a> {
    bytes: &'a [u8],
}

/// The empty configuration every panel accepts.
pub const EMPTY_CONFIG: ConfigBlob<'static> = ConfigBlob { bytes: &[] };

impl<'a> ConfigBlob<'a> {
    /// Validates framing: bound, entry structure, strictly ascending
    /// keys. Key *meaning* stays with the consumer ([`Self::get`]).
    pub fn parse(bytes: &'a [u8]) -> Result<ConfigBlob<'a>, ConfigError> {
        if bytes.len() > CONFIG_BLOB_MAX {
            return Err(ConfigError::TooLong { len: bytes.len() });
        }
        let mut off = 0;
        let mut previous: Option<u16> = None;
        while off < bytes.len() {
            let (key, len) = entry_header(bytes, off)?;
            if let Some(previous) = previous
                && key <= previous
            {
                return Err(ConfigError::KeysNotAscending { key });
            }
            previous = Some(key);
            off += 4 + len;
        }
        Ok(ConfigBlob { bytes })
    }

    /// The payload of `key`, or `None` when absent.
    pub fn get(&self, key: ConfigKey) -> Option<&'a [u8]> {
        let mut off = 0;
        while off < self.bytes.len() {
            let (entry_key, len) = entry_header(self.bytes, off).ok()?;
            if entry_key == key.0 {
                return self.bytes.get(off + 4..off + 4 + len);
            }
            off += 4 + len;
        }
        None
    }

    /// Refuses any key outside `schema` (the shell-side gate).
    pub fn require_schema(&self, schema: &[ConfigKey]) -> Result<(), ConfigError> {
        let mut off = 0;
        while off < self.bytes.len() {
            // Construction proved the framing; a gate must still fail
            // closed rather than stop scanning if that ever breaks.
            let (key, len) = entry_header(self.bytes, off)?;
            if !schema.iter().any(|k| k.0 == key) {
                return Err(ConfigError::UnknownKey { key });
            }
            off += 4 + len;
        }
        Ok(())
    }

    /// The validated bytes.
    pub const fn as_bytes(&self) -> &'a [u8] {
        self.bytes
    }
}

/// Reads `[key u16 LE][len u16 LE]` at `off` and bounds-checks the
/// payload it announces.
fn entry_header(bytes: &[u8], off: usize) -> Result<(u16, usize), ConfigError> {
    let header: [u8; 4] = match bytes.get(off..off + 4).and_then(|h| h.try_into().ok()) {
        Some(header) => header,
        None => {
            let key = bytes
                .get(off..off + 2)
                .and_then(|k| k.try_into().ok())
                .map_or(0, u16::from_le_bytes);
            return Err(ConfigError::Truncated { key });
        }
    };
    let key = u16::from_le_bytes([header[0], header[1]]);
    let len = u16::from_le_bytes([header[2], header[3]]) as usize;
    if bytes.len() < off + 4 + len {
        return Err(ConfigError::Truncated { key });
    }
    Ok((key, len))
}

/// Encodes `(speeds, bugs)` into the canonical `44`-byte [`keys::V_SPEEDS`] payload.
#[must_use]
pub fn encode_v_speeds_payload(
    speeds: &indicate_instrument_state::VSpeeds,
    bugs: &indicate_instrument_state::VSpeedBugs,
) -> [u8; V_SPEEDS_EXTENDED_BYTES] {
    let opt = |v: Option<f32>| v.unwrap_or(f32::NAN);
    let words = [
        speeds.vs0_kt,
        speeds.vs_kt,
        speeds.vfe_kt,
        speeds.vno_kt,
        speeds.vne_kt,
        opt(bugs.vr_kt),
        opt(bugs.vx_kt),
        opt(bugs.vy_kt),
        opt(bugs.vg_kt),
        opt(bugs.va_kt),
        opt(bugs.vref_kt),
    ];
    let mut out = [0u8; V_SPEEDS_EXTENDED_BYTES];
    for (i, val) in words.iter().enumerate() {
        if let Some(slot) = out.get_mut(i * 4..i * 4 + 4) {
            slot.copy_from_slice(&val.to_le_bytes());
        }
    }
    out
}

/// Decodes a `20`-byte (bands only) or `44`-byte (bands + reference bugs)
/// [`keys::V_SPEEDS`] payload fail-closed.
pub fn decode_v_speeds_payload(
    bytes: &[u8],
) -> Result<
    (
        indicate_instrument_state::VSpeeds,
        indicate_instrument_state::VSpeedBugs,
    ),
    ConfigError,
> {
    let bad = || ConfigError::BadValue {
        key: keys::V_SPEEDS.0,
        len: bytes.len(),
    };
    let word = |idx: usize| -> Option<f32> {
        let raw: [u8; 4] = bytes.get(idx * 4..idx * 4 + 4)?.try_into().ok()?;
        Some(f32::from_le_bytes(raw))
    };
    if bytes.len() != V_SPEEDS_BANDS_BYTES && bytes.len() != V_SPEEDS_EXTENDED_BYTES {
        return Err(bad());
    }
    let speeds = indicate_instrument_state::VSpeeds {
        vs0_kt: word(0).ok_or_else(bad)?,
        vs_kt: word(1).ok_or_else(bad)?,
        vfe_kt: word(2).ok_or_else(bad)?,
        vno_kt: word(3).ok_or_else(bad)?,
        vne_kt: word(4).ok_or_else(bad)?,
    };
    if !speeds.is_valid() {
        return Err(bad());
    }
    if bytes.len() == V_SPEEDS_BANDS_BYTES {
        return Ok((speeds, indicate_instrument_state::VSpeedBugs::default()));
    }
    let bug = |idx: usize| -> Result<Option<f32>, ConfigError> {
        let v = word(idx).ok_or_else(bad)?;
        if v.is_nan() {
            Ok(None)
        } else if v.is_finite() && v > 0.0 && v <= speeds.vne_kt {
            Ok(Some(v))
        } else {
            Err(bad())
        }
    };
    let bugs = indicate_instrument_state::VSpeedBugs {
        vr_kt: bug(5)?,
        vx_kt: bug(6)?,
        vy_kt: bug(7)?,
        vg_kt: bug(8)?,
        va_kt: bug(9)?,
        vref_kt: bug(10)?,
    };
    if !bugs.is_valid_for(Some(&speeds)) {
        return Err(bad());
    }
    Ok((speeds, bugs))
}

#[cfg(test)]
mod tests;
