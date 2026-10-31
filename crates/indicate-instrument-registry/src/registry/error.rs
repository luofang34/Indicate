//! Why a composition was refused: one variant per rule.

/// Why a composition was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RegistryError {
    /// A shell with no panels has nothing to display.
    #[error("a registry must contain at least one panel")]
    Empty,
    /// A composition naming no sets has nothing to display.
    #[error("a registry must contain at least one set")]
    NoSets,
    /// A set id violates the lowercase/digits/dashes charset.
    #[error("set {set} has a malformed id")]
    BadSetId {
        /// Position in the composed set list.
        set: usize,
    },
    /// Two sets share an id, so neither can be named unambiguously.
    #[error("set {set} repeats an earlier set's id")]
    DuplicateSetId {
        /// Position of the second occurrence.
        set: usize,
    },
    /// A set contributing no panels is a provider wired up wrongly.
    #[error("set {set} contributes no panels")]
    EmptySet {
        /// Position in the composed set list.
        set: usize,
    },
    /// A panel id violates the lowercase/digits/dashes charset.
    #[error("panel {index} has a malformed id")]
    BadId {
        /// Position in the flattened composition.
        index: usize,
    },
    /// Two panels share an id.
    #[error("panel {index} repeats an earlier panel's id")]
    DuplicateId {
        /// Position of the second occurrence.
        index: usize,
    },
    /// An empty title cannot label health or layout surfaces.
    #[error("panel {index} has an empty title")]
    EmptyTitle {
        /// Position in the flattened composition.
        index: usize,
    },
    /// A panel that requires no layers would pass completeness vacuously.
    #[error("panel {index} declares no required layers")]
    NoRequiredLayers {
        /// Position in the flattened composition.
        index: usize,
    },
    /// Required-layer bits beyond the defined scene layers or bands.
    #[error("panel {index} requires undefined layer bits {bits:#06x}")]
    UndefinedLayerBits {
        /// Position in the flattened composition.
        index: usize,
        /// The offending mask.
        bits: u16,
    },
    /// Underlay contract incompatible with the declared render space.
    #[error("panel {index} declares an underlay incompatible with its render space")]
    IncompatibleUnderlay {
        /// Position in the flattened composition.
        index: usize,
    },
    /// A non-finite or non-positive frame bound.
    #[error("panel {index} has a degenerate frame bound")]
    BadFrameBounds {
        /// Position in the flattened composition.
        index: usize,
    },
    /// A maximum frame smaller than the minimum on some axis.
    #[error("panel {index} declares a maximum frame below its minimum")]
    FrameRangeInverted {
        /// Position in the flattened composition.
        index: usize,
    },
    /// A non-finite or non-positive quantization step.
    #[error("panel {index} has a degenerate frame step")]
    BadFrameStep {
        /// Position in the flattened composition.
        index: usize,
    },
    /// Aspect bounds that are non-finite, non-positive, or inverted.
    #[error("panel {index} has degenerate aspect bounds")]
    BadAspectBounds {
        /// Position in the flattened composition.
        index: usize,
    },
    /// A `Planar2D` panel pinning no evidence sizes.
    #[error("panel {index} pins no canonical frames")]
    NoCanonicalFrames {
        /// Position in the flattened composition.
        index: usize,
    },
    /// The readability floor is not among the pinned evidence sizes.
    #[error("panel {index} does not pin its minimum frame as canonical")]
    CanonicalFramesMissingMin {
        /// Position in the flattened composition.
        index: usize,
    },
    /// The largest declared frame is not among the pinned evidence sizes.
    #[error("panel {index} does not pin its maximum frame as canonical")]
    CanonicalFramesMissingMax {
        /// Position in the flattened composition.
        index: usize,
    },
    /// A canonical frame outside the declared range.
    #[error("panel {index} canonical frame {position} is outside its declared range")]
    CanonicalFrameOutOfRange {
        /// Position in the flattened composition.
        index: usize,
        /// Position of the offending frame within the panel.
        position: usize,
    },
    /// A canonical frame that is not `frame_min + k * frame_step`.
    #[error("panel {index} canonical frame {position} is off the step grid")]
    CanonicalFrameOffStep {
        /// Position in the flattened composition.
        index: usize,
        /// Position of the offending frame within the panel.
        position: usize,
    },
    /// A canonical frame whose ratio violates `aspect_min..=aspect_max`.
    #[error("panel {index} canonical frame {position} violates its aspect bounds")]
    CanonicalFrameAspect {
        /// Position in the flattened composition.
        index: usize,
        /// Position of the offending frame within the panel.
        position: usize,
    },
    /// A raster baseline pinned at a frame nothing is ever rendered at.
    #[error("panel {index} raster baseline {position} names a frame that is not canonical")]
    RasterBaselineNotCanonical {
        /// Position in the flattened composition.
        index: usize,
        /// Position of the offending baseline within the panel.
        position: usize,
    },
    /// The same frame is pinned twice in `canonical_frames`.
    #[error("panel {index} repeats canonical frame {position}")]
    DuplicateCanonicalFrame {
        /// Position in the flattened composition.
        index: usize,
        /// Position of the second occurrence within the panel.
        position: usize,
    },
    /// Two raster baselines name the same canonical frame.
    #[error("panel {index} pins raster baseline {position} at an already-pinned frame")]
    DuplicateRasterBaseline {
        /// Position in the flattened composition.
        index: usize,
        /// Position of the second occurrence within the panel.
        position: usize,
    },
    /// Degenerate 2D overlay frame on `AngularSphere`.
    #[error("panel {index} declares a degenerate AngularSphere overlay frame")]
    BadOverlayFrame {
        /// Position in the flattened composition.
        index: usize,
    },
    /// Degenerate 3D metric extent bound on `Volume3D`.
    #[error("panel {index} has a degenerate Volume3D extent bound")]
    BadExtentBounds {
        /// Position in the flattened composition.
        index: usize,
    },
    /// Inverted 3D metric extent range (`max < min`).
    #[error("panel {index} declares a maximum Volume3D extent below its minimum")]
    ExtentRangeInverted {
        /// Position in the flattened composition.
        index: usize,
    },
    /// Invalid `ViewLimits` on `Volume3D`.
    #[error("panel {index} declares invalid ViewLimits")]
    BadViewLimits {
        /// Position in the flattened composition.
        index: usize,
    },
    /// `Volume3D` panel pinning no canonical extents.
    #[error("panel {index} pins no canonical Volume3D extents")]
    NoCanonicalExtents {
        /// Position in the flattened composition.
        index: usize,
    },
    /// Canonical `Volume3D` extent outside declared range or degenerate.
    #[error("panel {index} canonical extent {position} is outside its declared range")]
    CanonicalExtentOutOfRange {
        /// Position in the flattened composition.
        index: usize,
        /// Position within `canonical_extents_m`.
        position: usize,
    },
    /// Duplicate canonical `Volume3D` extent.
    #[error("panel {index} repeats canonical extent {position}")]
    DuplicateCanonicalExtent {
        /// Position in the flattened composition.
        index: usize,
        /// Position of duplicate within `canonical_extents_m`.
        position: usize,
    },
    /// `group_regions` or `raster_baselines` declared on a non-`Planar2D` instrument.
    #[error("panel {index} declares 2D regions or raster baselines on a non-Planar2D space")]
    NonPlanar2DMetadata {
        /// Position in the flattened composition.
        index: usize,
    },
    /// Schema keys must be strictly ascending.
    #[error("panel {index} schema key {key} repeats or descends")]
    SchemaKeysNotAscending {
        /// Position in the flattened composition.
        index: usize,
        /// The out-of-order key.
        key: u16,
    },
    /// A group region for a group the panel does not consume.
    #[error("panel {index} declares a region for group {group} it does not require")]
    RegionGroupNotRequired {
        /// Position in the flattened composition.
        index: usize,
        /// The wire tag of the unrequired group.
        group: u8,
    },
    /// A group region outside the minimum frame (or degenerate).
    #[error("panel {index} declares a region for group {group} outside its minimum frame")]
    RegionOutsideFrame {
        /// Position in the flattened composition.
        index: usize,
        /// The wire tag of the group.
        group: u8,
    },
    /// Two extreme states of one panel share an id.
    #[error("panel {index} repeats the extreme-state id at position {position}")]
    DuplicateExtremeId {
        /// Position in the flattened composition.
        index: usize,
        /// Position of the second occurrence within the panel.
        position: usize,
    },
    /// An extreme-state id violates the lowercase/digits/dashes charset.
    #[error("panel {index} extreme state {position} has a malformed id")]
    BadExtremeId {
        /// Position in the flattened composition.
        index: usize,
        /// Position of the offending extreme state within the panel.
        position: usize,
    },
}
