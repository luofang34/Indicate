//! Presence-masked sample groups: any number of records, each carrying
//! only the fields it has.
//!
//! The engine group and the fuel tank group share this shape. On the
//! wire a sample is a presence mask u16, then one f32 per set bit in
//! bit order, so a sample sizes itself and a region of samples validates
//! exactly. In memory the owned state carries only a [`SampleGroup`]
//! header (count and fault); the samples travel beside it as a
//! [`Samples`] view, borrowed from the admitted frame or from a
//! producer's slice, with no capacity and no allocation.

use crate::validate::GroupFault;

/// Most fields one sample registry may assign.
pub const MAX_FIELDS: usize = 16;

/// One decoded sample: its presence mask and a value per field bit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RawSample {
    /// Bit `k` set: field `k` is present.
    pub mask: u16,
    /// Field `k`'s value, meaningful only when bit `k` is set.
    pub values: [f32; MAX_FIELDS],
}

impl RawSample {
    /// An empty sample: no field present.
    pub const EMPTY: RawSample = RawSample {
        mask: 0,
        values: [0.0; MAX_FIELDS],
    };

    /// Field `bit`, if present.
    #[must_use]
    pub fn get(&self, bit: u8) -> Option<f32> {
        if u32::from(bit) < 16 && self.mask & (1 << bit) != 0 {
            self.values.get(usize::from(bit)).copied()
        } else {
            None
        }
    }

    /// Sets field `bit` present with `value`, or absent for `None`.
    pub fn set(&mut self, bit: u8, value: Option<f32>) {
        let Some(slot) = self.values.get_mut(usize::from(bit)) else {
            return;
        };
        match value {
            Some(v) => {
                *slot = v;
                self.mask |= 1 << bit;
            }
            None => self.mask &= !(1 << bit),
        }
    }
}

/// A record type of a presence-masked group: which field bits its
/// registry assigns, and how it maps to and from a raw sample.
pub trait Sample: Copy + Default {
    /// Every bit the registry assigns; any other bit faults the group.
    const KNOWN: u16;
    /// The record of a raw sample.
    fn from_raw(raw: &RawSample) -> Self;
    /// The raw sample of this record.
    fn to_raw(&self) -> RawSample;
    /// Whether every present value is in the field's allowed range.
    fn is_sound(&self) -> bool;

    /// Wire bytes of this record.
    fn wire_bytes(&self) -> usize {
        2 + 4 * self.to_raw().mask.count_ones() as usize
    }
}

/// The header of a presence-masked group, carried in the owned state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SampleGroup {
    /// How many records the source reports, in index order.
    pub count: u16,
    /// Why the decoder refused the records, if it did. Validation faults
    /// the group with it, and the decoder exposes no records.
    pub fault: Option<GroupFault>,
}

/// The records beside a state: a borrowed view, with no capacity and
/// no allocation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Samples<'a, S> {
    /// A producer's or fixture's own records.
    Slice(&'a [S]),
    /// The records region of an admitted frame: `count` self-sizing
    /// records packed back to back, already checked by the decoder.
    Wire {
        /// The records region.
        bytes: &'a [u8],
        /// How many records it holds.
        count: u16,
    },
}

impl<S: Sample + 'static> Samples<'_, S> {
    /// No records.
    pub const EMPTY: Samples<'static, S> = Samples::Slice(&[]);
}

impl<'a, S: Sample> Samples<'a, S> {
    /// How many records the view holds.
    #[must_use]
    pub fn len(&self) -> usize {
        match self {
            Self::Slice(records) => records.len(),
            Self::Wire { count, .. } => usize::from(*count),
        }
    }

    /// Whether the view holds no records.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Record `index`, zero-based. A wire view walks the records before
    /// it, because each record sizes itself; use [`Self::iter`] to read
    /// them all.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<S> {
        match self {
            Self::Slice(records) => records.get(index).copied(),
            Self::Wire { .. } => self.iter().nth(index),
        }
    }

    /// Every record, in index order.
    #[must_use]
    pub fn iter(&self) -> SampleIter<'a, S> {
        match *self {
            Self::Slice(records) => SampleIter::Slice(records.iter()),
            Self::Wire { bytes, count } => SampleIter::Wire { bytes, left: count },
        }
    }

    /// Checks a records region against `count`: the view when it holds
    /// exactly `count` well-formed, sound records and nothing else. A
    /// length that disagrees with the masks or an unknown bit is
    /// [`GroupFault::MalformedPayload`]; a value outside its field's
    /// range is [`GroupFault::NonFinite`].
    pub fn parse(bytes: &'a [u8], count: u16) -> Result<Self, GroupFault> {
        let mut rest = bytes;
        let mut unsound = false;
        for _ in 0..count {
            let (raw, len) = decode_raw(rest, S::KNOWN).ok_or(GroupFault::MalformedPayload)?;
            unsound |= !S::from_raw(&raw).is_sound();
            rest = rest.get(len..).ok_or(GroupFault::MalformedPayload)?;
        }
        if !rest.is_empty() {
            return Err(GroupFault::MalformedPayload);
        }
        if unsound {
            return Err(GroupFault::NonFinite);
        }
        Ok(Self::Wire { bytes, count })
    }
}

/// Iterator over a [`Samples`] view.
#[derive(Debug, Clone)]
pub enum SampleIter<'a, S> {
    /// Over a borrowed slice.
    Slice(core::slice::Iter<'a, S>),
    /// Over wire bytes.
    Wire {
        /// The records not yet read.
        bytes: &'a [u8],
        /// How many records remain.
        left: u16,
    },
}

impl<S: Sample> Iterator for SampleIter<'_, S> {
    type Item = S;

    fn next(&mut self) -> Option<S> {
        match self {
            Self::Slice(iter) => iter.next().copied(),
            Self::Wire { bytes, left } => {
                if *left == 0 {
                    return None;
                }
                let (raw, len) = decode_raw(bytes, S::KNOWN)?;
                *bytes = bytes.get(len..).unwrap_or(&[]);
                *left = left.saturating_sub(1);
                Some(S::from_raw(&raw))
            }
        }
    }
}

/// Wire bytes of a sample with presence mask `mask` under a registry
/// that assigns `known`; `None` for a bit outside it.
#[must_use]
pub const fn raw_wire_bytes(mask: u16, known: u16) -> Option<usize> {
    if mask & !known != 0 {
        return None;
    }
    Some(2 + 4 * mask.count_ones() as usize)
}

/// Decodes one raw sample from the front of `bytes`, returning it and
/// its wire length.
pub(crate) fn decode_raw(bytes: &[u8], known: u16) -> Option<(RawSample, usize)> {
    let mask = u16::from_le_bytes([*bytes.first()?, *bytes.get(1)?]);
    let len = raw_wire_bytes(mask, known)?;
    let mut raw = RawSample::EMPTY;
    let mut at = 2;
    for bit in 0..16u8 {
        if mask & (1 << bit) != 0 {
            let b: [u8; 4] = bytes.get(at..at + 4)?.try_into().ok()?;
            raw.set(bit, Some(f32::from_le_bytes(b)));
            at += 4;
        }
    }
    Some((raw, len))
}

/// Writes `raw` at the front of `out`, returning its length; `None`
/// when `out` is too short.
pub(crate) fn encode_raw(raw: &RawSample, out: &mut [u8]) -> Option<usize> {
    out.get_mut(..2)?.copy_from_slice(&raw.mask.to_le_bytes());
    let mut at = 2;
    for bit in 0..16u8 {
        if let Some(v) = raw.get(bit) {
            out.get_mut(at..at + 4)?.copy_from_slice(&v.to_le_bytes());
            at += 4;
        }
    }
    Some(at)
}

/// Every record group that travels beside a state: what an encoder
/// needs besides the owned state, and what a draw context carries.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BesideSamples<'a> {
    /// Engine samples.
    pub engines: crate::engine::EngineSamples<'a>,
    /// Fuel tank samples.
    pub tanks: crate::tank::TankSamples<'a>,
}

impl BesideSamples<'_> {
    /// No records.
    pub const EMPTY: BesideSamples<'static> = BesideSamples {
        engines: Samples::Slice(&[]),
        tanks: Samples::Slice(&[]),
    };
}
