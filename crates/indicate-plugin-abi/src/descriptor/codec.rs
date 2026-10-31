//! Binary encoder and decoder for [`super::PluginDescriptorWire`].

use indicate_instrument_descriptor::{
    AuxChannelId, ConfigKey, DesignFrame, GroupSet, UnderlayContract,
};
use indicate_instrument_scene::volume::Extent3D;
use indicate_intent::{IntentClass, IntentNamespace};
use indicate_interaction::ViewLimits;

use super::{
    DescriptorWireError, MAX_PLUGIN_AUX_CHANNELS, MAX_PLUGIN_CANONICAL, MAX_PLUGIN_CONFIG_KEYS,
    MAX_PLUGIN_INTENTS, PLUGIN_DESCRIPTOR_VERSION, PluginDescriptorWire, PluginSpaceWire,
};

/// Encodes `desc` into `out`, returning the number of bytes written.
pub fn encode_descriptor(
    desc: &PluginDescriptorWire,
    out: &mut [u8],
) -> Result<usize, DescriptorWireError> {
    desc.validate()?;
    let mut off = 0usize;
    push_bytes(out, &mut off, &[PLUGIN_DESCRIPTOR_VERSION])?;
    push_u16(out, &mut off, desc.required_layers)?;
    push_u32(out, &mut off, desc.required_groups.bits())?;
    push_bytes(out, &mut off, &[desc.underlay.to_u8()])?;
    let id = desc.id().as_bytes();
    push_bytes(out, &mut off, &[id.len() as u8])?;
    push_bytes(out, &mut off, id)?;
    let title = desc.title().as_bytes();
    push_bytes(out, &mut off, &[title.len() as u8])?;
    push_bytes(out, &mut off, title)?;

    encode_space(&desc.space, out, &mut off)?;
    encode_collections(desc, out, &mut off)?;
    Ok(off)
}

fn encode_space(
    space: &PluginSpaceWire,
    out: &mut [u8],
    off: &mut usize,
) -> Result<(), DescriptorWireError> {
    match space {
        PluginSpaceWire::Planar2D {
            frame_min,
            frame_max,
            frame_step,
            aspect_min,
            aspect_max,
            canonical_buf,
            canonical_len,
        } => {
            push_bytes(out, off, &[0, *canonical_len])?;
            for v in [
                frame_min.width,
                frame_min.height,
                frame_max.width,
                frame_max.height,
                frame_step.0,
                frame_step.1,
                *aspect_min,
                *aspect_max,
            ] {
                push_f32(out, off, v)?;
            }
            for f in &canonical_buf[..usize::from(*canonical_len)] {
                push_f32(out, off, f.width)?;
                push_f32(out, off, f.height)?;
            }
        }
        PluginSpaceWire::AngularSphere { overlay_frame } => match overlay_frame {
            Some(f) => {
                push_bytes(out, off, &[1, 1])?;
                push_f32(out, off, f.width)?;
                push_f32(out, off, f.height)?;
            }
            None => push_bytes(out, off, &[1, 0])?,
        },
        PluginSpaceWire::Volume3D {
            extent_min_m,
            extent_max_m,
            canonical_buf,
            canonical_len,
            view_limits,
        } => {
            let flags =
                u8::from(view_limits.allow_orbit) | (u8::from(view_limits.allow_time_scrub) << 1);
            push_bytes(out, off, &[2, *canonical_len, flags])?;
            for v in [
                extent_min_m.width_m,
                extent_min_m.height_m,
                extent_min_m.depth_m,
                extent_max_m.width_m,
                extent_max_m.height_m,
                extent_max_m.depth_m,
                view_limits.pan_limit,
                view_limits.zoom_min,
                view_limits.zoom_max,
            ] {
                push_f32(out, off, v)?;
            }
            push_bytes(out, off, &view_limits.time_scrub_limit_s.to_le_bytes())?;
            for e in &canonical_buf[..usize::from(*canonical_len)] {
                push_f32(out, off, e.width_m)?;
                push_f32(out, off, e.height_m)?;
                push_f32(out, off, e.depth_m)?;
            }
        }
    }
    Ok(())
}

fn encode_collections(
    desc: &PluginDescriptorWire,
    out: &mut [u8],
    off: &mut usize,
) -> Result<(), DescriptorWireError> {
    let cfg = desc.config_schema();
    let intents = desc.accepted_intents();
    let aux = desc.aux_channels();
    push_bytes(
        out,
        off,
        &[cfg.len() as u8, intents.len() as u8, aux.len() as u8],
    )?;
    for k in cfg {
        push_u16(out, off, k.0)?;
    }
    for ic in intents {
        push_bytes(out, off, &ic.namespace.0.to_le_bytes())?;
        push_u16(out, off, ic.opcode)?;
    }
    for ch in aux {
        push_u32(out, off, ch.0)?;
    }
    Ok(())
}

/// Decodes a [`PluginDescriptorWire`] from exact byte slice `bytes`.
pub fn decode_descriptor(bytes: &[u8]) -> Result<PluginDescriptorWire, DescriptorWireError> {
    let mut off = 0usize;
    let ver = take_u8(bytes, &mut off)?;
    if ver != PLUGIN_DESCRIPTOR_VERSION {
        return Err(DescriptorWireError::UnsupportedVersion(ver));
    }
    let required_layers = take_u16(bytes, &mut off)?;
    let raw_groups = take_u32(bytes, &mut off)?;
    let required_groups = GroupSet::from_bits(raw_groups)
        .ok_or(DescriptorWireError::InvalidRequiredGroups(raw_groups))?;
    let underlay = UnderlayContract::from_u8(take_u8(bytes, &mut off)?)
        .ok_or(DescriptorWireError::InvalidUnderlay)?;

    let id_len = usize::from(take_u8(bytes, &mut off)?);
    let id_bytes = take_slice(bytes, &mut off, id_len)?;
    let id = core::str::from_utf8(id_bytes).map_err(|_| DescriptorWireError::InvalidString)?;
    let title_len = usize::from(take_u8(bytes, &mut off)?);
    let title_bytes = take_slice(bytes, &mut off, title_len)?;
    let title =
        core::str::from_utf8(title_bytes).map_err(|_| DescriptorWireError::InvalidString)?;

    let space = decode_space(bytes, &mut off)?;
    let (cfg, cfg_n, intents, int_n, aux, aux_n) = decode_collections(bytes, &mut off)?;
    if off != bytes.len() {
        return Err(DescriptorWireError::InvalidLength);
    }
    PluginDescriptorWire::new(
        id,
        title,
        space,
        required_layers,
        required_groups,
        underlay,
        &cfg[..cfg_n],
        &intents[..int_n],
        &aux[..aux_n],
    )
}

fn decode_space(bytes: &[u8], off: &mut usize) -> Result<PluginSpaceWire, DescriptorWireError> {
    let kind = take_u8(bytes, off)?;
    match kind {
        0 => decode_planar_space(bytes, off),
        1 => {
            let has_ov = take_u8(bytes, off)?;
            let overlay_frame = match has_ov {
                0 => None,
                1 => Some(DesignFrame {
                    width: take_f32(bytes, off)?,
                    height: take_f32(bytes, off)?,
                }),
                _ => return Err(DescriptorWireError::InvalidSpace),
            };
            Ok(PluginSpaceWire::AngularSphere { overlay_frame })
        }
        2 => decode_volume_space(bytes, off),
        _ => Err(DescriptorWireError::InvalidSpace),
    }
}

fn decode_planar_space(
    bytes: &[u8],
    off: &mut usize,
) -> Result<PluginSpaceWire, DescriptorWireError> {
    let canonical_len = take_u8(bytes, off)?;
    if canonical_len == 0 || usize::from(canonical_len) > MAX_PLUGIN_CANONICAL {
        return Err(DescriptorWireError::InvalidSpace);
    }
    let frame_min = DesignFrame {
        width: take_f32(bytes, off)?,
        height: take_f32(bytes, off)?,
    };
    let frame_max = DesignFrame {
        width: take_f32(bytes, off)?,
        height: take_f32(bytes, off)?,
    };
    let frame_step = (take_f32(bytes, off)?, take_f32(bytes, off)?);
    let aspect_min = take_f32(bytes, off)?;
    let aspect_max = take_f32(bytes, off)?;
    let mut canonical_buf = [DesignFrame {
        width: 1.0,
        height: 1.0,
    }; MAX_PLUGIN_CANONICAL];
    for slot in &mut canonical_buf[..usize::from(canonical_len)] {
        *slot = DesignFrame {
            width: take_f32(bytes, off)?,
            height: take_f32(bytes, off)?,
        };
    }
    Ok(PluginSpaceWire::Planar2D {
        frame_min,
        frame_max,
        frame_step,
        aspect_min,
        aspect_max,
        canonical_buf,
        canonical_len,
    })
}

fn decode_volume_space(
    bytes: &[u8],
    off: &mut usize,
) -> Result<PluginSpaceWire, DescriptorWireError> {
    let canonical_len = take_u8(bytes, off)?;
    let flags = take_u8(bytes, off)?;
    if canonical_len == 0
        || usize::from(canonical_len) > MAX_PLUGIN_CANONICAL
        || (flags & !0x03) != 0
    {
        return Err(DescriptorWireError::InvalidSpace);
    }
    let extent_min_m = Extent3D::new(
        take_f32(bytes, off)?,
        take_f32(bytes, off)?,
        take_f32(bytes, off)?,
    );
    let extent_max_m = Extent3D::new(
        take_f32(bytes, off)?,
        take_f32(bytes, off)?,
        take_f32(bytes, off)?,
    );
    let pan_limit = take_f32(bytes, off)?;
    let zoom_min = take_f32(bytes, off)?;
    let zoom_max = take_f32(bytes, off)?;
    let t_slice = take_slice(bytes, off, 8)?;
    let mut tb = [0u8; 8];
    tb.copy_from_slice(t_slice);
    let time_scrub_limit_s = f64::from_le_bytes(tb);
    let mut canonical_buf = [Extent3D::new(1.0, 1.0, 1.0); MAX_PLUGIN_CANONICAL];
    for slot in &mut canonical_buf[..usize::from(canonical_len)] {
        *slot = Extent3D::new(
            take_f32(bytes, off)?,
            take_f32(bytes, off)?,
            take_f32(bytes, off)?,
        );
    }
    Ok(PluginSpaceWire::Volume3D {
        extent_min_m,
        extent_max_m,
        canonical_buf,
        canonical_len,
        view_limits: ViewLimits {
            pan_limit,
            zoom_min,
            zoom_max,
            time_scrub_limit_s,
            allow_orbit: (flags & 0x01) != 0,
            allow_time_scrub: (flags & 0x02) != 0,
        },
    })
}

type Collections = (
    [ConfigKey; MAX_PLUGIN_CONFIG_KEYS],
    usize,
    [IntentClass; MAX_PLUGIN_INTENTS],
    usize,
    [AuxChannelId; MAX_PLUGIN_AUX_CHANNELS],
    usize,
);

fn decode_collections(bytes: &[u8], off: &mut usize) -> Result<Collections, DescriptorWireError> {
    let cfg_n = usize::from(take_u8(bytes, off)?);
    let int_n = usize::from(take_u8(bytes, off)?);
    let aux_n = usize::from(take_u8(bytes, off)?);
    if cfg_n > MAX_PLUGIN_CONFIG_KEYS
        || int_n > MAX_PLUGIN_INTENTS
        || aux_n > MAX_PLUGIN_AUX_CHANNELS
    {
        return Err(DescriptorWireError::CollectionTooLarge);
    }
    let mut cfg = [ConfigKey(0); MAX_PLUGIN_CONFIG_KEYS];
    for slot in &mut cfg[..cfg_n] {
        *slot = ConfigKey(take_u16(bytes, off)?);
    }
    let mut intents = [IntentClass::core(0); MAX_PLUGIN_INTENTS];
    for slot in &mut intents[..int_n] {
        let ns_slice = take_slice(bytes, off, 8)?;
        let mut nb = [0u8; 8];
        nb.copy_from_slice(ns_slice);
        let opcode = take_u16(bytes, off)?;
        *slot = IntentClass {
            namespace: IntentNamespace(u64::from_le_bytes(nb)),
            opcode,
        };
    }
    let mut aux = [AuxChannelId(0); MAX_PLUGIN_AUX_CHANNELS];
    for slot in &mut aux[..aux_n] {
        *slot = AuxChannelId(take_u32(bytes, off)?);
    }
    Ok((cfg, cfg_n, intents, int_n, aux, aux_n))
}

fn push_bytes(out: &mut [u8], off: &mut usize, src: &[u8]) -> Result<(), DescriptorWireError> {
    let end = off
        .checked_add(src.len())
        .ok_or(DescriptorWireError::InvalidLength)?;
    if out.len() < end {
        return Err(DescriptorWireError::BufferTooSmall {
            needed: end,
            available: out.len(),
        });
    }
    out[*off..end].copy_from_slice(src);
    *off = end;
    Ok(())
}

fn push_u16(out: &mut [u8], off: &mut usize, v: u16) -> Result<(), DescriptorWireError> {
    push_bytes(out, off, &v.to_le_bytes())
}

fn push_u32(out: &mut [u8], off: &mut usize, v: u32) -> Result<(), DescriptorWireError> {
    push_bytes(out, off, &v.to_le_bytes())
}

fn push_f32(out: &mut [u8], off: &mut usize, v: f32) -> Result<(), DescriptorWireError> {
    push_bytes(out, off, &v.to_le_bytes())
}

fn take_slice<'a>(
    bytes: &'a [u8],
    off: &mut usize,
    len: usize,
) -> Result<&'a [u8], DescriptorWireError> {
    let end = off
        .checked_add(len)
        .ok_or(DescriptorWireError::InvalidLength)?;
    let slice = bytes
        .get(*off..end)
        .ok_or(DescriptorWireError::InvalidLength)?;
    *off = end;
    Ok(slice)
}

fn take_u8(bytes: &[u8], off: &mut usize) -> Result<u8, DescriptorWireError> {
    Ok(take_slice(bytes, off, 1)?[0])
}

fn take_u16(bytes: &[u8], off: &mut usize) -> Result<u16, DescriptorWireError> {
    let s = take_slice(bytes, off, 2)?;
    Ok(u16::from_le_bytes([s[0], s[1]]))
}

fn take_u32(bytes: &[u8], off: &mut usize) -> Result<u32, DescriptorWireError> {
    let s = take_slice(bytes, off, 4)?;
    Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn take_f32(bytes: &[u8], off: &mut usize) -> Result<f32, DescriptorWireError> {
    let s = take_slice(bytes, off, 4)?;
    Ok(f32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}
