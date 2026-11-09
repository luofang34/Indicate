//! Engine indications: what each engine's gauges read, for any number
//! of engines of any kind.
//!
//! The values are the indicated ones. Mechanism lag and instrument
//! error are the source's physics, so a simulator that models a gauge
//! writes the reading its mechanism shows.
//!
//! # Header and samples
//!
//! The owned [`crate::AircraftState`] carries only the group's header,
//! [`EngineGroup`]: how many engines the source reports, and whether
//! the decoder found the samples sound. That is all status, withholding,
//! and validation need, and it keeps the state a fixed-size `Copy`
//! value. The samples travel beside the state as an [`EngineSamples`]
//! view: borrowed from the admitted frame bytes on a consumer, from the
//! producer's own slice on a producer. No capacity bounds the engine
//! count, and nothing allocates.
//!
//! # Presence is meaning
//!
//! Engines differ: a piston engine has manifold pressure and speed, a
//! turbine N1, N2, and gas temperature, a rocket chamber pressure. So a
//! sample carries only the fields its engine has, named by a presence
//! mask over the [`EngineField`] registry. A dial bound to a field an
//! engine lacks shows its failure pose, never zero.

use crate::validate::GroupFault;

/// One field of the engine indication registry. Each field owns the
/// presence-mask bit equal to its discriminant. The registry is
/// append-only: a bit never changes meaning and is never reused. A new
/// field is a new state ABI version, because a decoder cannot size a
/// sample that carries a bit it does not know.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineField {
    /// Manifold absolute pressure, hectopascals, f32.
    ManifoldPressureHpa = 0,
    /// Shaft speed, revolutions per minute, f32.
    Rpm = 1,
}

impl EngineField {
    /// Every registered field, in bit order: the order a sample's
    /// present fields follow on the wire.
    pub const ALL: [EngineField; 2] = [EngineField::ManifoldPressureHpa, EngineField::Rpm];

    /// This field's bit in a presence mask.
    #[must_use]
    pub const fn bit(self) -> u16 {
        1 << (self as u8)
    }

    /// Bytes this field occupies on the wire.
    #[must_use]
    pub const fn wire_bytes(self) -> usize {
        match self {
            Self::ManifoldPressureHpa | Self::Rpm => 4,
        }
    }
}

/// Every bit the registry assigns; any other bit faults the group.
pub const ENGINE_FIELDS_MASK: u16 = EngineField::ManifoldPressureHpa.bit() | EngineField::Rpm.bit();

/// Wire bytes of a sample whose presence mask is `mask`: the two mask
/// bytes and each present field. `None` when the mask names a field
/// outside the registry.
#[must_use]
pub const fn sample_wire_bytes(mask: u16) -> Option<usize> {
    if mask & !ENGINE_FIELDS_MASK != 0 {
        return None;
    }
    let mut bytes = 2;
    let mut k = 0;
    while k < EngineField::ALL.len() {
        let field = EngineField::ALL[k];
        if mask & field.bit() != 0 {
            bytes += field.wire_bytes();
        }
        k += 1;
    }
    Some(bytes)
}

/// The engine group's header, carried in the owned state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EngineGroup {
    /// How many engines the source reports, in panel order.
    pub count: u16,
    /// Why the decoder refused the samples, if it did. Validation
    /// faults the group with it, and the decoder exposes no samples.
    pub fault: Option<GroupFault>,
}

/// One engine's indicated readings. A field is `Some` exactly when the
/// engine has that indication; its presence bit is set on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct EngineSample {
    /// Manifold absolute pressure, hectopascals.
    pub manifold_pressure_hpa: Option<f32>,
    /// Shaft speed, revolutions per minute.
    pub rpm: Option<f32>,
}

impl EngineSample {
    /// The sample's presence mask.
    #[must_use]
    pub fn presence(&self) -> u16 {
        EngineField::ALL
            .iter()
            .filter(|field| self.field(**field).is_some())
            .fold(0, |mask, field| mask | field.bit())
    }

    /// The reading of `field`, if this engine has it.
    #[must_use]
    pub fn field(&self, field: EngineField) -> Option<f32> {
        match field {
            EngineField::ManifoldPressureHpa => self.manifold_pressure_hpa,
            EngineField::Rpm => self.rpm,
        }
    }

    fn set(&mut self, field: EngineField, value: f32) {
        match field {
            EngineField::ManifoldPressureHpa => self.manifold_pressure_hpa = Some(value),
            EngineField::Rpm => self.rpm = Some(value),
        }
    }

    /// Whether every present reading is a finite, non-negative number.
    #[must_use]
    pub fn is_sound(&self) -> bool {
        EngineField::ALL
            .iter()
            .filter_map(|field| self.field(*field))
            .all(|v| v.is_finite() && v >= 0.0)
    }

    /// Decodes one sample from the front of `bytes`, returning it and
    /// its wire length; `None` when the mask names an unknown field or
    /// the bytes run out.
    pub(crate) fn decode(bytes: &[u8]) -> Option<(Self, usize)> {
        let mask = u16::from_le_bytes([*bytes.first()?, *bytes.get(1)?]);
        let len = sample_wire_bytes(mask)?;
        let mut at = 2;
        let mut sample = EngineSample::default();
        for field in EngineField::ALL {
            if mask & field.bit() != 0 {
                let raw: [u8; 4] = bytes.get(at..at + 4)?.try_into().ok()?;
                sample.set(field, f32::from_le_bytes(raw));
                at += field.wire_bytes();
            }
        }
        Some((sample, len))
    }

    /// Wire bytes this sample encodes to.
    #[must_use]
    pub fn wire_bytes(&self) -> usize {
        sample_wire_bytes(self.presence()).unwrap_or(2)
    }

    /// Writes this sample at the front of `out`, returning its length;
    /// `None` when `out` is too short.
    pub(crate) fn encode(&self, out: &mut [u8]) -> Option<usize> {
        let mask = self.presence();
        out.get_mut(..2)?.copy_from_slice(&mask.to_le_bytes());
        let mut at = 2;
        for field in EngineField::ALL {
            if let Some(value) = self.field(field) {
                out.get_mut(at..at + 4)?
                    .copy_from_slice(&value.to_le_bytes());
                at += field.wire_bytes();
            }
        }
        Some(at)
    }
}

/// The engine samples beside a state: a borrowed view, with no
/// capacity and no allocation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EngineSamples<'a> {
    /// A producer's or fixture's own samples.
    Slice(&'a [EngineSample]),
    /// The samples region of an admitted frame: `count` self-sizing
    /// samples packed back to back, already checked by the decoder.
    Wire {
        /// The samples region.
        bytes: &'a [u8],
        /// How many samples it holds.
        count: u16,
    },
}

impl<'a> EngineSamples<'a> {
    /// No engines.
    pub const EMPTY: EngineSamples<'static> = EngineSamples::Slice(&[]);

    /// How many engines the view holds.
    #[must_use]
    pub fn len(&self) -> usize {
        match self {
            Self::Slice(samples) => samples.len(),
            Self::Wire { count, .. } => usize::from(*count),
        }
    }

    /// Whether the view holds no engines.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Engine `index`, zero-based in panel order. A wire view walks
    /// the samples before it, because each sample sizes itself; use
    /// [`Self::iter`] to read them all.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<EngineSample> {
        match self {
            Self::Slice(samples) => samples.get(index).copied(),
            Self::Wire { .. } => self.iter().nth(index),
        }
    }

    /// Every engine, in panel order.
    #[must_use]
    pub fn iter(&self) -> EngineIter<'a> {
        match *self {
            Self::Slice(samples) => EngineIter::Slice(samples.iter()),
            Self::Wire { bytes, count } => EngineIter::Wire { bytes, left: count },
        }
    }
}

/// Iterator over an [`EngineSamples`] view.
#[derive(Debug, Clone)]
pub enum EngineIter<'a> {
    /// Over a borrowed slice.
    Slice(core::slice::Iter<'a, EngineSample>),
    /// Over wire bytes.
    Wire {
        /// The samples not yet read.
        bytes: &'a [u8],
        /// How many samples remain.
        left: u16,
    },
}

impl Iterator for EngineIter<'_> {
    type Item = EngineSample;

    fn next(&mut self) -> Option<EngineSample> {
        match self {
            Self::Slice(iter) => iter.next().copied(),
            Self::Wire { bytes, left } => {
                if *left == 0 {
                    return None;
                }
                let (sample, len) = EngineSample::decode(bytes)?;
                *bytes = bytes.get(len..).unwrap_or(&[]);
                *left = left.saturating_sub(1);
                Some(sample)
            }
        }
    }
}

/// Checks a samples region against `count`: the view when it holds
/// exactly `count` well-formed, sound samples and nothing else. This is
/// the whole wire validation of the group. A length that disagrees with
/// the presence masks or an unknown bit is
/// [`GroupFault::MalformedPayload`]; a reading that is not a finite,
/// non-negative number is [`GroupFault::NonFinite`].
pub fn parse_samples(bytes: &[u8], count: u16) -> Result<EngineSamples<'_>, GroupFault> {
    let mut rest = bytes;
    let mut unsound = false;
    for _ in 0..count {
        let (sample, len) = EngineSample::decode(rest).ok_or(GroupFault::MalformedPayload)?;
        unsound |= !sample.is_sound();
        rest = rest.get(len..).ok_or(GroupFault::MalformedPayload)?;
    }
    if !rest.is_empty() {
        return Err(GroupFault::MalformedPayload);
    }
    if unsound {
        return Err(GroupFault::NonFinite);
    }
    Ok(EngineSamples::Wire { bytes, count })
}

/// The engine group's own fault: why the decoder refused its samples.
pub(crate) fn engine_fault(group: &EngineGroup) -> Option<GroupFault> {
    group.fault
}
