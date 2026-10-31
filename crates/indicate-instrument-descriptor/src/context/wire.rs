//! Frozen v1 wire codec for [`crate::DrawContext`].

use indicate_alerts::{AlertOutput, AlertWireError, decode_alerts, encode_alerts};
use indicate_instrument_state::{
    AircraftState, FreshnessPolicy, PanelData,
    abi::v8::{self, AbiError},
    resolve,
};
use indicate_intent::{
    ECHO_SUMMARY_BYTES, EchoSummary, IntentWireError, decode_echo_summary, encode_echo_summary,
};
use indicate_interaction::{
    InteractionError, VIEW_POSE_BYTES, ViewPose3D, decode_view_pose, encode_view_pose,
};

use super::DrawContext;
use super::synth::synthesize_aircraft_state;
use crate::aux::{AuxChannelId, AuxEntry, MAX_AUX_ENTRIES};
use crate::config::{ConfigBlob, ConfigError};
use crate::space::UnderlayStatus;

/// Wire version of the 72-byte [`DrawContext`] envelope header.
pub const DRAW_CONTEXT_WIRE_VERSION: u8 = 1;

/// Exact byte length of the fixed [`DrawContext`] wire header.
pub const DRAW_CONTEXT_HEADER_BYTES: usize = 72;

/// Fail-closed error returned when encoding or decoding a [`DrawContext`] frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DrawContextWireError {
    /// Output slice cannot hold the encoded context.
    #[error("draw context buffer too small: needed at least {needed} bytes, got {available}")]
    BufferTooSmall {
        /// Minimum byte capacity needed.
        needed: usize,
        /// Provided slice length.
        available: usize,
    },
    /// Input buffer is truncated or carries unannounced trailing bytes.
    #[error("invalid draw context wire length: expected {expected} bytes, got {actual}")]
    InvalidLength {
        /// Expected byte length.
        expected: usize,
        /// Actual byte length.
        actual: usize,
    },
    /// Wire version byte is unsupported.
    #[error("unsupported draw context wire version {0}")]
    UnsupportedVersion(u8),
    /// Reserved bits in `underlay_flags` are non-zero.
    #[error("invalid underlay flags byte {0:#04x}")]
    InvalidUnderlayFlags(u8),
    /// `underlay_quality_pct` exceeds `100`.
    #[error("underlay quality {0}% exceeds 100%")]
    InvalidUnderlayQuality(u8),
    /// `has_alerts` byte is neither `0` nor `1`, or disagrees with `alerts_len`.
    #[error("invalid alerts presence flag {flag} with length {len}")]
    InvalidAlertsFlag {
        /// Raw `has_alerts` byte.
        flag: u8,
        /// Declared `alerts_len`.
        len: u16,
    },
    /// Auxiliary channel count exceeds [`MAX_AUX_ENTRIES`].
    #[error("aux entry count {0} exceeds maximum {MAX_AUX_ENTRIES}")]
    TooManyAuxEntries(usize),
    /// Auxiliary channel identifiers repeat or are not strictly ascending.
    #[error("aux channel {0:#010x} repeats or is out of ascending order")]
    AuxOrder(u32),
    /// Section or payload length overflows wire width.
    #[error("section length overflows wire integer width")]
    LengthOverflow,
    /// State v8 codec failed or left trailing bytes.
    #[error("state wire error: {0}")]
    State(#[from] AbiError),
    /// State v8 frame carried unannounced trailing bytes.
    #[error("state frame carried {0} trailing bytes")]
    StateTrailingBytes(u16),
    /// Config TLV blob failed validation.
    #[error("config wire error: {0}")]
    Config(#[from] ConfigError),
    /// Alert snapshot codec failed.
    #[error("alert wire error: {0}")]
    Alerts(#[from] AlertWireError),
    /// Echo summary codec failed.
    #[error("echo summary wire error: {0}")]
    Echo(#[from] IntentWireError),
    /// 3D view pose codec failed.
    #[error("view pose wire error: {0}")]
    ViewPose(#[from] InteractionError),
}

/// Stack-allocated decoded [`DrawContext`] owning resolved [`PanelData`] and borrowing slices.
#[derive(Debug, Clone, Copy)]
pub struct DecodedDrawContext<'a> {
    state: AircraftState,
    data: PanelData,
    config: ConfigBlob<'a>,
    alerts: Option<AlertOutput>,
    echo: EchoSummary,
    view: ViewPose3D,
    underlay: UnderlayStatus,
    aux_buf: [AuxEntry<'a>; MAX_AUX_ENTRIES],
    aux_len: usize,
}

impl<'a> DecodedDrawContext<'a> {
    /// Borrows this decoded envelope as a [`DrawContext`].
    #[must_use]
    pub fn as_draw_context<'b>(&'b self) -> DrawContext<'b>
    where
        'a: 'b,
    {
        DrawContext::new(&self.data)
            .with_raw_state(&self.state)
            .with_config(self.config)
            .with_alerts(self.alerts.as_ref())
            .with_echo(self.echo)
            .with_view(self.view)
            .with_underlay(self.underlay)
            .with_aux(&self.aux_buf[..self.aux_len])
    }

    /// Decoded raw [`AircraftState`].
    #[must_use]
    pub const fn state(&self) -> &AircraftState {
        &self.state
    }

    /// Resolved [`PanelData`].
    #[must_use]
    pub const fn data(&self) -> &PanelData {
        &self.data
    }
}

/// Encodes `ctx` into `out` using the frozen v1 72-byte header envelope.
pub fn encode_draw_context(
    ctx: &DrawContext<'_>,
    out: &mut [u8],
) -> Result<usize, DrawContextWireError> {
    let underlay = ctx.underlay();
    if underlay.quality_pct > 100 {
        return Err(DrawContextWireError::InvalidUnderlayQuality(
            underlay.quality_pct,
        ));
    }
    let sorted_aux = sorted_aux_indices(ctx.aux_entries())?;
    if out.len() < DRAW_CONTEXT_HEADER_BYTES {
        return Err(DrawContextWireError::BufferTooSmall {
            needed: DRAW_CONTEXT_HEADER_BYTES,
            available: out.len(),
        });
    }
    let mut echo_buf = [0u8; ECHO_SUMMARY_BYTES];
    encode_echo_summary(ctx.echo(), &mut echo_buf);
    let mut view_buf = [0u8; VIEW_POSE_BYTES];
    encode_view_pose(ctx.view(), &mut view_buf)?;

    let mut off = DRAW_CONTEXT_HEADER_BYTES;
    let synthesized;
    let state_ref = if let Some(s) = ctx.raw_state() {
        s
    } else {
        synthesized = synthesize_aircraft_state(ctx.data());
        &synthesized
    };
    let state_len = encode_state_section(state_ref, out, &mut off)?;
    let config_len = encode_config_section(ctx.config().as_bytes(), out, &mut off)?;
    let (has_alerts, alerts_len) = encode_alerts_section(ctx.alerts(), out, &mut off)?;
    encode_aux_section(ctx.aux_entries(), &sorted_aux, out, &mut off)?;

    write_header(
        &mut out[..DRAW_CONTEXT_HEADER_BYTES],
        underlay,
        has_alerts,
        (state_len, config_len, alerts_len, sorted_aux.1),
        &echo_buf,
        &view_buf,
    );
    Ok(off)
}

fn write_header(
    hdr: &mut [u8],
    underlay: UnderlayStatus,
    has_alerts: u8,
    lens: (u16, u16, u16, u16),
    echo_buf: &[u8; ECHO_SUMMARY_BYTES],
    view_buf: &[u8; VIEW_POSE_BYTES],
) {
    let flags = u8::from(underlay.available) | (u8::from(underlay.calibrated) << 1);
    hdr[0] = DRAW_CONTEXT_WIRE_VERSION;
    hdr[1] = flags;
    hdr[2] = underlay.quality_pct;
    hdr[3] = has_alerts;
    hdr[4..6].copy_from_slice(&lens.0.to_le_bytes());
    hdr[6..8].copy_from_slice(&lens.1.to_le_bytes());
    hdr[8..10].copy_from_slice(&lens.2.to_le_bytes());
    hdr[10..12].copy_from_slice(&lens.3.to_le_bytes());
    hdr[12..24].copy_from_slice(echo_buf);
    hdr[24..72].copy_from_slice(view_buf);
}

fn sorted_aux_indices(
    entries: &[AuxEntry<'_>],
) -> Result<([usize; MAX_AUX_ENTRIES], u16), DrawContextWireError> {
    if entries.len() > MAX_AUX_ENTRIES {
        return Err(DrawContextWireError::TooManyAuxEntries(entries.len()));
    }
    let mut idx = [0usize; MAX_AUX_ENTRIES];
    let mut i = 0;
    while i < entries.len() {
        idx[i] = i;
        i = i.wrapping_add(1);
    }
    // Insertion sort up to 16 entries by ascending AuxChannelId.
    let mut a = 1;
    while a < entries.len() {
        let cur = idx[a];
        let mut b = a;
        while b > 0 && entries[idx[b - 1]].id.0 > entries[cur].id.0 {
            idx[b] = idx[b - 1];
            b -= 1;
        }
        idx[b] = cur;
        a = a.wrapping_add(1);
    }
    let mut k = 1;
    while k < entries.len() {
        if entries[idx[k - 1]].id == entries[idx[k]].id {
            return Err(DrawContextWireError::AuxOrder(entries[idx[k]].id.0));
        }
        k = k.wrapping_add(1);
    }
    let count = u16::try_from(entries.len()).map_err(|_| DrawContextWireError::LengthOverflow)?;
    Ok((idx, count))
}

fn encode_state_section(
    state: &AircraftState,
    out: &mut [u8],
    off: &mut usize,
) -> Result<u16, DrawContextWireError> {
    let avail = out.len();
    let tail = out
        .get_mut(*off..)
        .ok_or(DrawContextWireError::BufferTooSmall {
            needed: *off + 2,
            available: avail,
        })?;
    let written = v8::encode_state(state, tail).map_err(|err| match err {
        AbiError::Truncated => DrawContextWireError::BufferTooSmall {
            needed: avail.saturating_add(64),
            available: avail,
        },
        other => DrawContextWireError::State(other),
    })?;
    *off += written;
    u16::try_from(written).map_err(|_| DrawContextWireError::LengthOverflow)
}

fn encode_config_section(
    cfg: &[u8],
    out: &mut [u8],
    off: &mut usize,
) -> Result<u16, DrawContextWireError> {
    let needed = off
        .checked_add(cfg.len())
        .ok_or(DrawContextWireError::LengthOverflow)?;
    if out.len() < needed {
        return Err(DrawContextWireError::BufferTooSmall {
            needed,
            available: out.len(),
        });
    }
    out[*off..needed].copy_from_slice(cfg);
    *off = needed;
    u16::try_from(cfg.len()).map_err(|_| DrawContextWireError::LengthOverflow)
}

fn encode_alerts_section(
    alerts: Option<&AlertOutput>,
    out: &mut [u8],
    off: &mut usize,
) -> Result<(u8, u16), DrawContextWireError> {
    let Some(alert_out) = alerts else {
        return Ok((0, 0));
    };
    let avail = out.len();
    let written = encode_alerts(alert_out, &mut out[*off..]).map_err(|err| match err {
        AlertWireError::BufferTooSmall { needed, .. } => DrawContextWireError::BufferTooSmall {
            needed: off.saturating_add(needed),
            available: avail,
        },
        other => DrawContextWireError::Alerts(other),
    })?;
    *off += written;
    let len = u16::try_from(written).map_err(|_| DrawContextWireError::LengthOverflow)?;
    Ok((1, len))
}

fn encode_aux_section(
    entries: &[AuxEntry<'_>],
    sorted: &([usize; MAX_AUX_ENTRIES], u16),
    out: &mut [u8],
    off: &mut usize,
) -> Result<(), DrawContextWireError> {
    let count = usize::from(sorted.1);
    let mut i = 0;
    while i < count {
        let entry = entries[sorted.0[i]];
        let payload_len =
            u32::try_from(entry.payload.len()).map_err(|_| DrawContextWireError::LengthOverflow)?;
        let needed = off
            .checked_add(8)
            .and_then(|v| v.checked_add(entry.payload.len()))
            .ok_or(DrawContextWireError::LengthOverflow)?;
        if out.len() < needed {
            return Err(DrawContextWireError::BufferTooSmall {
                needed,
                available: out.len(),
            });
        }
        out[*off..*off + 4].copy_from_slice(&entry.id.0.to_le_bytes());
        out[*off + 4..*off + 8].copy_from_slice(&payload_len.to_le_bytes());
        out[*off + 8..needed].copy_from_slice(entry.payload);
        *off = needed;
        i = i.wrapping_add(1);
    }
    Ok(())
}

/// Decodes a frozen v1 [`DrawContext`] wire frame without heap allocation.
pub fn decode_draw_context(bytes: &[u8]) -> Result<DecodedDrawContext<'_>, DrawContextWireError> {
    if bytes.len() < DRAW_CONTEXT_HEADER_BYTES {
        return Err(DrawContextWireError::InvalidLength {
            expected: DRAW_CONTEXT_HEADER_BYTES,
            actual: bytes.len(),
        });
    }
    if bytes[0] != DRAW_CONTEXT_WIRE_VERSION {
        return Err(DrawContextWireError::UnsupportedVersion(bytes[0]));
    }
    let underlay_flags = bytes[1];
    if (underlay_flags & !0x03) != 0 {
        return Err(DrawContextWireError::InvalidUnderlayFlags(underlay_flags));
    }
    let quality_pct = bytes[2];
    if quality_pct > 100 {
        return Err(DrawContextWireError::InvalidUnderlayQuality(quality_pct));
    }
    let has_alerts = bytes[3];
    let state_len = usize::from(read_u16(&bytes[4..6]));
    let config_len = usize::from(read_u16(&bytes[6..8]));
    let alerts_len_u16 = read_u16(&bytes[8..10]);
    let alerts_len = usize::from(alerts_len_u16);
    if (has_alerts == 0 && alerts_len != 0)
        || (has_alerts == 1 && alerts_len == 0)
        || has_alerts > 1
    {
        return Err(DrawContextWireError::InvalidAlertsFlag {
            flag: has_alerts,
            len: alerts_len_u16,
        });
    }
    let aux_count = usize::from(read_u16(&bytes[10..12]));
    if aux_count > MAX_AUX_ENTRIES {
        return Err(DrawContextWireError::TooManyAuxEntries(aux_count));
    }
    let echo = decode_echo_summary(&bytes[12..24])?;
    let view = decode_view_pose(&bytes[24..72])?;
    let underlay = UnderlayStatus {
        available: (underlay_flags & 0x01) != 0,
        calibrated: (underlay_flags & 0x02) != 0,
        quality_pct,
    };
    decode_sections(
        bytes,
        (state_len, config_len, alerts_len, aux_count),
        underlay,
        echo,
        view,
    )
}

fn decode_sections<'a>(
    bytes: &'a [u8],
    lens: (usize, usize, usize, usize),
    underlay: UnderlayStatus,
    echo: EchoSummary,
    view: ViewPose3D,
) -> Result<DecodedDrawContext<'a>, DrawContextWireError> {
    let (state_len, config_len, alerts_len, aux_count) = lens;
    let body_end = DRAW_CONTEXT_HEADER_BYTES
        .checked_add(state_len)
        .and_then(|v| v.checked_add(config_len))
        .and_then(|v| v.checked_add(alerts_len))
        .ok_or(DrawContextWireError::LengthOverflow)?;
    if bytes.len() < body_end {
        return Err(DrawContextWireError::InvalidLength {
            expected: body_end,
            actual: bytes.len(),
        });
    }
    let s_end = DRAW_CONTEXT_HEADER_BYTES + state_len;
    let c_end = s_end + config_len;
    let report = v8::decode_state(&bytes[DRAW_CONTEXT_HEADER_BYTES..s_end])?;
    if report.trailing_bytes != 0 {
        return Err(DrawContextWireError::StateTrailingBytes(
            report.trailing_bytes,
        ));
    }
    let data = resolve(&report.state, &FreshnessPolicy::default());
    let config = ConfigBlob::parse(&bytes[s_end..c_end])?;
    let alerts = if alerts_len > 0 {
        Some(decode_alerts(&bytes[c_end..body_end])?)
    } else {
        None
    };
    let (aux_buf, off) = decode_aux_entries(bytes, body_end, aux_count)?;
    if off != bytes.len() {
        return Err(DrawContextWireError::InvalidLength {
            expected: off,
            actual: bytes.len(),
        });
    }
    Ok(DecodedDrawContext {
        state: report.state,
        data,
        config,
        alerts,
        echo,
        view,
        underlay,
        aux_buf,
        aux_len: aux_count,
    })
}

fn decode_aux_entries(
    bytes: &[u8],
    mut off: usize,
    aux_count: usize,
) -> Result<([AuxEntry<'_>; MAX_AUX_ENTRIES], usize), DrawContextWireError> {
    let mut aux_buf = [AuxEntry {
        id: AuxChannelId(0),
        payload: &[],
    }; MAX_AUX_ENTRIES];
    let mut prev_id: Option<u32> = None;
    let mut i = 0;
    while i < aux_count {
        let header_end = off
            .checked_add(8)
            .ok_or(DrawContextWireError::LengthOverflow)?;
        if bytes.len() < header_end {
            return Err(DrawContextWireError::InvalidLength {
                expected: header_end,
                actual: bytes.len(),
            });
        }
        let id = read_u32(&bytes[off..off + 4]);
        if let Some(prev) = prev_id
            && id <= prev
        {
            return Err(DrawContextWireError::AuxOrder(id));
        }
        prev_id = Some(id);
        let payload_len = usize::try_from(read_u32(&bytes[off + 4..off + 8]))
            .map_err(|_| DrawContextWireError::LengthOverflow)?;
        let entry_end = header_end
            .checked_add(payload_len)
            .ok_or(DrawContextWireError::LengthOverflow)?;
        if bytes.len() < entry_end {
            return Err(DrawContextWireError::InvalidLength {
                expected: entry_end,
                actual: bytes.len(),
            });
        }
        aux_buf[i] = AuxEntry {
            id: AuxChannelId(id),
            payload: &bytes[header_end..entry_end],
        };
        off = entry_end;
        i = i.wrapping_add(1);
    }
    Ok((aux_buf, off))
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
