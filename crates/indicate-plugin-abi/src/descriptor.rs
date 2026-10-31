//! Binary codec for static plugin metadata (`PluginDescriptorWire`, `PLUG-01`).

use indicate_instrument_descriptor::{
    AuxChannelId, ConfigKey, DesignFrame, GroupSet, UnderlayContract,
};
use indicate_instrument_scene::{
    LAYER_COUNT,
    angular::ANGULAR_LAYER_COUNT,
    volume::{Extent3D, VOLUME_BAND_COUNT},
};
use indicate_intent::IntentClass;
use indicate_interaction::ViewLimits;

/// Wire version of [`PluginDescriptorWire`].
pub const PLUGIN_DESCRIPTOR_VERSION: u8 = 1;

/// Maximum UTF-8 byte length of a plugin identifier.
pub const MAX_PLUGIN_ID_LEN: usize = 64;
/// Maximum UTF-8 byte length of a plugin title.
pub const MAX_PLUGIN_TITLE_LEN: usize = 128;
/// Maximum canonical 2D frames or 3D extents declared by a plugin.
pub const MAX_PLUGIN_CANONICAL: usize = 8;
/// Maximum config schema keys declared by a plugin.
pub const MAX_PLUGIN_CONFIG_KEYS: usize = 32;
/// Maximum accepted intent classes declared by a plugin.
pub const MAX_PLUGIN_INTENTS: usize = 32;
/// Maximum auxiliary channels declared by a plugin.
pub const MAX_PLUGIN_AUX_CHANNELS: usize = 16;

/// Render-space declaration inside [`PluginDescriptorWire`].
#[derive(Debug, Clone, Copy)]
pub enum PluginSpaceWire {
    /// Planar 2D frame range and canonical frame list.
    Planar2D {
        /// Minimum admissible frame.
        frame_min: DesignFrame,
        /// Maximum admissible frame.
        frame_max: DesignFrame,
        /// Quantization step `(step_w, step_h)`.
        frame_step: (f32, f32),
        /// Minimum aspect ratio.
        aspect_min: f32,
        /// Maximum aspect ratio.
        aspect_max: f32,
        /// Canonical frames buffer.
        canonical_buf: [DesignFrame; MAX_PLUGIN_CANONICAL],
        /// Active canonical frame count (`1..=MAX_PLUGIN_CANONICAL`).
        canonical_len: u8,
    },
    /// Unit-sphere angular space with optional 2D peripheral overlay frame.
    AngularSphere {
        /// Optional 2D overlay design frame.
        overlay_frame: Option<DesignFrame>,
    },
    /// Bounded 3D metric volume space.
    Volume3D {
        /// Minimum metric extent.
        extent_min_m: Extent3D,
        /// Maximum metric extent.
        extent_max_m: Extent3D,
        /// Canonical metric extents buffer.
        canonical_buf: [Extent3D; MAX_PLUGIN_CANONICAL],
        /// Active canonical extent count (`1..=MAX_PLUGIN_CANONICAL`).
        canonical_len: u8,
        /// View pose evolution bounds.
        view_limits: ViewLimits,
    },
}

impl PartialEq for PluginSpaceWire {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Planar2D {
                    frame_min: a_min,
                    frame_max: a_max,
                    frame_step: a_step,
                    aspect_min: a_amin,
                    aspect_max: a_amax,
                    canonical_buf: a_buf,
                    canonical_len: a_len,
                },
                Self::Planar2D {
                    frame_min: b_min,
                    frame_max: b_max,
                    frame_step: b_step,
                    aspect_min: b_amin,
                    aspect_max: b_amax,
                    canonical_buf: b_buf,
                    canonical_len: b_len,
                },
            ) => {
                a_min == b_min
                    && a_max == b_max
                    && a_step == b_step
                    && a_amin == b_amin
                    && a_amax == b_amax
                    && a_len == b_len
                    && a_buf[..usize::from(*a_len)] == b_buf[..usize::from(*b_len)]
            }
            (
                Self::AngularSphere { overlay_frame: a },
                Self::AngularSphere { overlay_frame: b },
            ) => a == b,
            (
                Self::Volume3D {
                    extent_min_m: a_min,
                    extent_max_m: a_max,
                    canonical_buf: a_buf,
                    canonical_len: a_len,
                    view_limits: a_lim,
                },
                Self::Volume3D {
                    extent_min_m: b_min,
                    extent_max_m: b_max,
                    canonical_buf: b_buf,
                    canonical_len: b_len,
                    view_limits: b_lim,
                },
            ) => {
                a_min == b_min
                    && a_max == b_max
                    && a_lim == b_lim
                    && a_len == b_len
                    && a_buf[..usize::from(*a_len)] == b_buf[..usize::from(*b_len)]
            }
            _ => false,
        }
    }
}

impl PluginSpaceWire {
    /// Constructs a `Planar2D` wire space from a canonical frame slice.
    pub fn planar_2d(
        frame_min: DesignFrame,
        frame_max: DesignFrame,
        frame_step: (f32, f32),
        aspect_min: f32,
        aspect_max: f32,
        canonical: &[DesignFrame],
    ) -> Result<Self, DescriptorWireError> {
        if canonical.is_empty() || canonical.len() > MAX_PLUGIN_CANONICAL {
            return Err(DescriptorWireError::CollectionTooLarge);
        }
        let mut canonical_buf = [DesignFrame {
            width: 0.0,
            height: 0.0,
        }; MAX_PLUGIN_CANONICAL];
        canonical_buf[..canonical.len()].copy_from_slice(canonical);
        Ok(Self::Planar2D {
            frame_min,
            frame_max,
            frame_step,
            aspect_min,
            aspect_max,
            canonical_buf,
            canonical_len: canonical.len() as u8,
        })
    }

    /// Constructs a `Volume3D` wire space from a canonical extent slice.
    pub fn volume_3d(
        extent_min_m: Extent3D,
        extent_max_m: Extent3D,
        canonical: &[Extent3D],
        view_limits: ViewLimits,
    ) -> Result<Self, DescriptorWireError> {
        if canonical.is_empty() || canonical.len() > MAX_PLUGIN_CANONICAL {
            return Err(DescriptorWireError::CollectionTooLarge);
        }
        let mut canonical_buf = [Extent3D::new(0.0, 0.0, 0.0); MAX_PLUGIN_CANONICAL];
        canonical_buf[..canonical.len()].copy_from_slice(canonical);
        Ok(Self::Volume3D {
            extent_min_m,
            extent_max_m,
            canonical_buf,
            canonical_len: canonical.len() as u8,
            view_limits,
        })
    }
}

/// Owned, stack-allocated static plugin descriptor decoded from `.indpack` (`PLUG-01`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PluginDescriptorWire {
    id_buf: [u8; MAX_PLUGIN_ID_LEN],
    id_len: u8,
    title_buf: [u8; MAX_PLUGIN_TITLE_LEN],
    title_len: u8,
    /// Declared render space and sizing bounds.
    pub space: PluginSpaceWire,
    /// Required layer or band bitset.
    pub required_layers: u16,
    /// Consumed flight state groups.
    pub required_groups: GroupSet,
    /// Declared underlay contract.
    pub underlay: UnderlayContract,
    config_buf: [ConfigKey; MAX_PLUGIN_CONFIG_KEYS],
    config_len: u8,
    intents_buf: [IntentClass; MAX_PLUGIN_INTENTS],
    intents_len: u8,
    aux_buf: [AuxChannelId; MAX_PLUGIN_AUX_CHANNELS],
    aux_len: u8,
}

/// Fail-closed error returned when encoding or decoding [`PluginDescriptorWire`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DescriptorWireError {
    /// Output buffer is too small.
    #[error("descriptor wire buffer too small: needed {needed}, got {available}")]
    BufferTooSmall {
        /// Needed bytes.
        needed: usize,
        /// Available bytes.
        available: usize,
    },
    /// Input byte slice is truncated or carries trailing bytes.
    #[error("descriptor wire truncated or length mismatch")]
    InvalidLength,
    /// Unsupported descriptor wire version.
    #[error("unsupported plugin descriptor version {0}")]
    UnsupportedVersion(u8),
    /// Invalid plugin identifier or title string.
    #[error("invalid plugin identifier or UTF-8 title")]
    InvalidString,
    /// Invalid render space discriminant or geometric bounds.
    #[error("invalid plugin render space or bounds")]
    InvalidSpace,
    /// `required_layers` sets undefined bits for the declared render space.
    #[error("invalid required_layers bitmask {0:#06x}")]
    InvalidRequiredLayers(u16),
    /// `required_groups` sets undefined group bits.
    #[error("invalid required_groups bitmask {0:#010x}")]
    InvalidRequiredGroups(u32),
    /// Invalid or incompatible [`UnderlayContract`].
    #[error("invalid or incompatible underlay contract")]
    InvalidUnderlay,
    /// Collection exceeds maximum inline capacity.
    #[error("plugin descriptor collection exceeds bounded capacity")]
    CollectionTooLarge,
}

impl PluginDescriptorWire {
    /// Constructs and validates a [`PluginDescriptorWire`] from borrowed components.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: &str,
        title: &str,
        space: PluginSpaceWire,
        required_layers: u16,
        required_groups: GroupSet,
        underlay: UnderlayContract,
        config_schema: &[ConfigKey],
        accepted_intents: &[IntentClass],
        aux_channels: &[AuxChannelId],
    ) -> Result<Self, DescriptorWireError> {
        if !valid_id(id) || title.is_empty() || title.len() > MAX_PLUGIN_TITLE_LEN {
            return Err(DescriptorWireError::InvalidString);
        }
        if config_schema.len() > MAX_PLUGIN_CONFIG_KEYS
            || accepted_intents.len() > MAX_PLUGIN_INTENTS
            || aux_channels.len() > MAX_PLUGIN_AUX_CHANNELS
        {
            return Err(DescriptorWireError::CollectionTooLarge);
        }
        let mut id_buf = [0u8; MAX_PLUGIN_ID_LEN];
        id_buf[..id.len()].copy_from_slice(id.as_bytes());
        let mut title_buf = [0u8; MAX_PLUGIN_TITLE_LEN];
        title_buf[..title.len()].copy_from_slice(title.as_bytes());
        let mut config_buf = [ConfigKey(0); MAX_PLUGIN_CONFIG_KEYS];
        config_buf[..config_schema.len()].copy_from_slice(config_schema);
        let mut intents_buf = [IntentClass::core(0); MAX_PLUGIN_INTENTS];
        intents_buf[..accepted_intents.len()].copy_from_slice(accepted_intents);
        let mut aux_buf = [AuxChannelId(0); MAX_PLUGIN_AUX_CHANNELS];
        aux_buf[..aux_channels.len()].copy_from_slice(aux_channels);

        let desc = Self {
            id_buf,
            id_len: id.len() as u8,
            title_buf,
            title_len: title.len() as u8,
            space,
            required_layers,
            required_groups,
            underlay,
            config_buf,
            config_len: config_schema.len() as u8,
            intents_buf,
            intents_len: accepted_intents.len() as u8,
            aux_buf,
            aux_len: aux_channels.len() as u8,
        };
        desc.validate()?;
        Ok(desc)
    }

    /// Validated plugin identifier (`[a-z0-9-]+`).
    #[must_use]
    pub fn id(&self) -> &str {
        core::str::from_utf8(&self.id_buf[..usize::from(self.id_len)]).unwrap_or("")
    }

    /// Validated operator-facing UTF-8 title.
    #[must_use]
    pub fn title(&self) -> &str {
        core::str::from_utf8(&self.title_buf[..usize::from(self.title_len)]).unwrap_or("")
    }

    /// Declared configuration schema keys.
    #[must_use]
    pub fn config_schema(&self) -> &[ConfigKey] {
        &self.config_buf[..usize::from(self.config_len)]
    }

    /// Declared accepted operator intent classes.
    #[must_use]
    pub fn accepted_intents(&self) -> &[IntentClass] {
        &self.intents_buf[..usize::from(self.intents_len)]
    }

    /// Declared auxiliary channel identifiers.
    #[must_use]
    pub fn aux_channels(&self) -> &[AuxChannelId] {
        &self.aux_buf[..usize::from(self.aux_len)]
    }

    /// Validates all structural, layer-bit, underlay, and geometric bounds.
    pub fn validate(&self) -> Result<(), DescriptorWireError> {
        if !valid_id(self.id()) || self.title().is_empty() {
            return Err(DescriptorWireError::InvalidString);
        }
        validate_space_and_layers(&self.space, self.required_layers, self.underlay)
    }
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_PLUGIN_ID_LEN
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn validate_space_and_layers(
    space: &PluginSpaceWire,
    required_layers: u16,
    underlay: UnderlayContract,
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
            if !underlay.valid_for_planar_2d() {
                return Err(DescriptorWireError::InvalidUnderlay);
            }
            let mask = (1u16 << LAYER_COUNT) - 1;
            if (required_layers & !mask) != 0 {
                return Err(DescriptorWireError::InvalidRequiredLayers(required_layers));
            }
            validate_planar_bounds(
                *frame_min,
                *frame_max,
                *frame_step,
                (*aspect_min, *aspect_max),
                &canonical_buf[..usize::from(*canonical_len)],
            )
        }
        PluginSpaceWire::AngularSphere { overlay_frame } => {
            if !underlay.valid_for_angular_sphere() {
                return Err(DescriptorWireError::InvalidUnderlay);
            }
            let ang_mask = (1u16 << ANGULAR_LAYER_COUNT) - 1;
            let ov_mask = if overlay_frame.is_some() {
                ((1u16 << LAYER_COUNT) - 1) << 8
            } else {
                0
            };
            if (required_layers & !(ang_mask | ov_mask)) != 0 {
                return Err(DescriptorWireError::InvalidRequiredLayers(required_layers));
            }
            if let Some(f) = overlay_frame
                && !(f.width.is_finite() && f.width > 0.0 && f.height.is_finite() && f.height > 0.0)
            {
                return Err(DescriptorWireError::InvalidSpace);
            }
            Ok(())
        }
        PluginSpaceWire::Volume3D {
            extent_min_m,
            extent_max_m,
            canonical_buf,
            canonical_len,
            view_limits,
        } => {
            if !underlay.valid_for_volume_3d() {
                return Err(DescriptorWireError::InvalidUnderlay);
            }
            let mask = (1u16 << VOLUME_BAND_COUNT) - 1;
            if (required_layers & !mask) != 0 {
                return Err(DescriptorWireError::InvalidRequiredLayers(required_layers));
            }
            validate_volume_bounds(
                *extent_min_m,
                *extent_max_m,
                &canonical_buf[..usize::from(*canonical_len)],
                view_limits,
            )
        }
    }
}

fn validate_planar_bounds(
    min: DesignFrame,
    max: DesignFrame,
    step: (f32, f32),
    aspect: (f32, f32),
    canon: &[DesignFrame],
) -> Result<(), DescriptorWireError> {
    let ok = min.width.is_finite()
        && min.width > 0.0
        && min.height.is_finite()
        && min.height > 0.0
        && max.width.is_finite()
        && max.height.is_finite()
        && min.width <= max.width
        && min.height <= max.height
        && step.0.is_finite()
        && step.0 > 0.0
        && step.1.is_finite()
        && step.1 > 0.0
        && aspect.0.is_finite()
        && aspect.0 > 0.0
        && aspect.1.is_finite()
        && aspect.0 <= aspect.1
        && !canon.is_empty();
    if !ok {
        return Err(DescriptorWireError::InvalidSpace);
    }
    for f in canon {
        if !(f.width >= min.width
            && f.width <= max.width
            && f.height >= min.height
            && f.height <= max.height)
        {
            return Err(DescriptorWireError::InvalidSpace);
        }
    }
    Ok(())
}

fn validate_volume_bounds(
    min: Extent3D,
    max: Extent3D,
    canon: &[Extent3D],
    limits: &ViewLimits,
) -> Result<(), DescriptorWireError> {
    let ok = min.is_valid()
        && max.is_valid()
        && min.width_m <= max.width_m
        && min.height_m <= max.height_m
        && min.depth_m <= max.depth_m
        && limits.is_valid()
        && !canon.is_empty();
    if !ok {
        return Err(DescriptorWireError::InvalidSpace);
    }
    for e in canon {
        if !(e.is_valid()
            && e.width_m >= min.width_m
            && e.width_m <= max.width_m
            && e.height_m >= min.height_m
            && e.height_m <= max.height_m
            && e.depth_m >= min.depth_m
            && e.depth_m <= max.depth_m)
        {
            return Err(DescriptorWireError::InvalidSpace);
        }
    }
    Ok(())
}

mod codec;
pub use codec::{decode_descriptor, encode_descriptor};
