//! Typefaces as runtime data: stroke and outline glyphs, per-glyph
//! advance, ink box and kerning, loaded from bytes and pinned by a
//! content hash (format version 2).
//!
//! A face is borrowed bytes, not compiled-in data. [`Typeface::parse`]
//! reads a face in place, with no allocation, and checks every offset,
//! length, ordering and geometry rule before it returns. It then
//! recomputes the SHA-256 the face carries in its trailer. The bytes are
//! their own canonical serialisation (see [`format`]), so equal faces
//! have equal bytes and equal hashes.
//!
//! Two glyph kinds share one face. A stroke glyph is a centreline
//! (polylines and elliptical arcs) drawn with a round pen, as the
//! lettering standards define characters: MS33558 and MIL-M-18012B give a
//! constant stroke over a centreline. One stroke skeleton therefore
//! serves several letterings through a [`TextStyle`]: narrowing,
//! pen width, slant and glyph alternates. An outline glyph is filled
//! contours, for scripts and faces that exist only as outlines.
//!
//! The built-in 5×7 bitmap pack ([`crate::PANEL_GLYPHS`]) is unchanged
//! and stays the default face: [`Face::Reference`] stands for it, so a
//! scene drawn in the default face renders exactly as before.
//!
//! Lookup fails closed. A character the face lacks is
//! [`TypefaceError::MissingGlyph`]; nothing substitutes.

pub mod format;

mod check;
mod encode;
mod face;
mod geometry;
mod style;
mod tessellate;

pub use encode::{FaceDef, GlyphDef, Kern, Seg, Shape, StrokeDef, encode_face, encoded_len};
pub use face::{Face, InkBox, PlacedGlyph};
pub use geometry::{GlyphXform, OutlineEdges, StrokeSegments};
pub use style::TextStyle;
pub use tessellate::{Triangle, tessellate_glyph, tessellate_run};

use format::{GLYPH_RECORD_LEN, HASH_LEN, KERN_RECORD_LEN, Reader};

/// Why a face was refused or a lookup failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TypefaceError {
    /// The bytes do not start with the typeface magic.
    #[error("not a typeface: bad magic")]
    BadMagic,
    /// A format version this build does not read.
    #[error("typeface format version {found} is not readable here")]
    UnsupportedVersion {
        /// The version found.
        found: u16,
    },
    /// The bytes end before a field they announce.
    #[error("typeface truncated near byte {at}")]
    Truncated {
        /// Where the short read started.
        at: usize,
    },
    /// A header field is outside its allowed range.
    #[error("typeface header field {field} is invalid")]
    InvalidHeader {
        /// The field.
        field: &'static str,
    },
    /// Glyph records, kerning pairs or alternate tags are not strictly
    /// ascending, which is what makes the bytes canonical.
    #[error("typeface {table} table is not strictly ascending")]
    Unsorted {
        /// The table.
        table: &'static str,
    },
    /// A glyph's record or geometry cannot be read.
    #[error("glyph U+{codepoint:04X} is malformed")]
    InvalidGlyph {
        /// The glyph's code point.
        codepoint: u32,
    },
    /// Bytes remain after the trailer, or the geometry block holds bytes
    /// no glyph claims.
    #[error("typeface carries bytes no field accounts for")]
    TrailingBytes,
    /// The trailer hash is not the hash of the bytes before it.
    #[error("typeface content hash mismatch")]
    ContentHashMismatch,
    /// The face has no glyph for a character.
    #[error("the face has no glyph for {ch:?}")]
    MissingGlyph {
        /// The character.
        ch: char,
    },
    /// A style parameter is outside its allowed range.
    #[error("text style parameter {field} is invalid")]
    InvalidStyle {
        /// The parameter.
        field: &'static str,
    },
    /// An output buffer is too small for the encoded face.
    #[error("output buffer of {available} bytes; {needed} needed")]
    BufferTooSmall {
        /// Bytes needed.
        needed: usize,
        /// Bytes available.
        available: usize,
    },
}

/// The kind of a glyph's geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlyphKind {
    /// Centreline strokes drawn with the face's pen.
    Stroke,
    /// Filled contours.
    Outline,
}

/// One glyph of a face, borrowed from its bytes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlyphView<'a> {
    /// The character it draws.
    pub ch: char,
    /// Its alternate tag; 0 is the default form.
    pub alternate: u8,
    /// Its geometry kind.
    pub kind: GlyphKind,
    /// Its advance, in font units at the regular style.
    pub advance: u16,
    /// Its encoded geometry.
    pub(crate) geometry: &'a [u8],
}

/// A parsed, checked face.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Typeface<'a> {
    bytes: &'a [u8],
    id: &'a str,
    units_per_em: u16,
    cap_height: u16,
    ascender: i16,
    descender: i16,
    default_pen: u16,
    alternates: &'a [u8],
    glyphs: &'a [u8],
    kerning: &'a [u8],
    geometry: &'a [u8],
}

impl<'a> Typeface<'a> {
    /// Reads and checks a face. Every later accessor relies on the
    /// checks having passed, so none of them can read out of bounds.
    pub fn parse(bytes: &'a [u8]) -> Result<Self, TypefaceError> {
        let face = Self::read_layout(bytes)?;
        face.check()?;
        Ok(face)
    }

    fn read_layout(bytes: &'a [u8]) -> Result<Self, TypefaceError> {
        let mut r = Reader::new(bytes);
        let short = |r: &Reader<'_>| TypefaceError::Truncated { at: r.pos() };
        if r.take(4).ok_or(short(&r))? != format::TYPEFACE_MAGIC {
            return Err(TypefaceError::BadMagic);
        }
        let version = r.u16().ok_or(short(&r))?;
        if version != format::TYPEFACE_FORMAT_VERSION {
            return Err(TypefaceError::UnsupportedVersion { found: version });
        }
        let reserved = r.u16().ok_or(short(&r))?;
        if reserved != 0 {
            return Err(TypefaceError::InvalidHeader { field: "reserved" });
        }
        let units_per_em = r.u16().ok_or(short(&r))?;
        let cap_height = r.u16().ok_or(short(&r))?;
        let ascender = r.i16().ok_or(short(&r))?;
        let descender = r.i16().ok_or(short(&r))?;
        let default_pen = r.u16().ok_or(short(&r))?;
        let glyph_count = usize::from(r.u16().ok_or(short(&r))?);
        let kern_count = usize::from(r.u16().ok_or(short(&r))?);
        let alt_count = r.u8().ok_or(short(&r))?;
        let id_len = usize::from(r.u8().ok_or(short(&r))?);
        let id_bytes = r.take(id_len).ok_or(short(&r))?;
        let id = core::str::from_utf8(id_bytes)
            .map_err(|_| TypefaceError::InvalidHeader { field: "id" })?;
        let alt_start = r.pos();
        for _ in 0..alt_count {
            r.u8().ok_or(short(&r))?;
            let len = usize::from(r.u8().ok_or(short(&r))?);
            r.take(len).ok_or(short(&r))?;
        }
        let alternates = bytes.get(alt_start..r.pos()).ok_or(short(&r))?;
        let glyphs = r.take(glyph_count * GLYPH_RECORD_LEN).ok_or(short(&r))?;
        let kerning = r.take(kern_count * KERN_RECORD_LEN).ok_or(short(&r))?;
        let geometry_len = r.u32().ok_or(short(&r))? as usize;
        let geometry = r.take(geometry_len).ok_or(short(&r))?;
        r.take(HASH_LEN).ok_or(short(&r))?;
        if r.pos() != bytes.len() {
            return Err(TypefaceError::TrailingBytes);
        }
        Ok(Self {
            bytes,
            id,
            units_per_em,
            cap_height,
            ascender,
            descender,
            default_pen,
            alternates,
            glyphs,
            kerning,
            geometry,
        })
    }

    /// The face's identity, such as `us-ms33558`.
    #[must_use]
    pub fn id(&self) -> &'a str {
        self.id
    }

    /// Font units per em.
    #[must_use]
    pub fn units_per_em(&self) -> u16 {
        self.units_per_em
    }

    /// Height of a capital, font units: a run's size maps to it.
    #[must_use]
    pub fn cap_height(&self) -> u16 {
        self.cap_height
    }

    /// Highest point the face reaches above the baseline, font units.
    #[must_use]
    pub fn ascender(&self) -> i16 {
        self.ascender
    }

    /// Lowest point below the baseline, font units (zero or negative).
    #[must_use]
    pub fn descender(&self) -> i16 {
        self.descender
    }

    /// The pen width a stroke glyph is drawn with at the regular style,
    /// font units.
    #[must_use]
    pub fn default_pen(&self) -> u16 {
        self.default_pen
    }

    /// The SHA-256 the face carries and [`Self::parse`] verified.
    #[must_use]
    pub fn content_hash(&self) -> [u8; 32] {
        let start = self.bytes.len().saturating_sub(HASH_LEN);
        self.bytes
            .get(start..)
            .and_then(|tail| tail.try_into().ok())
            .unwrap_or([0; 32])
    }

    /// The bytes the face was parsed from.
    #[must_use]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.bytes
    }

    /// Number of glyph records, alternates included.
    #[must_use]
    pub fn glyph_count(&self) -> usize {
        self.glyphs.len() / GLYPH_RECORD_LEN
    }

    /// Every glyph, in canonical order.
    pub fn glyphs(&self) -> impl Iterator<Item = GlyphView<'a>> + '_ {
        (0..self.glyph_count()).filter_map(|k| self.record(k))
    }

    /// The tag of the alternate named `name`, such as `one.flag`.
    #[must_use]
    pub fn alternate_tag(&self, name: &str) -> Option<u8> {
        let mut r = Reader::new(self.alternates);
        while let Some(tag) = r.u8() {
            let len = usize::from(r.u8()?);
            if r.take(len)? == name.as_bytes() {
                return Some(tag);
            }
        }
        None
    }

    pub(crate) fn record(&self, k: usize) -> Option<GlyphView<'a>> {
        let mut r = Reader::at(self.glyphs, k.checked_mul(GLYPH_RECORD_LEN)?);
        let ch = char::from_u32(r.u32()?)?;
        let alternate = r.u8()?;
        let kind = match r.u8()? {
            format::KIND_STROKE => GlyphKind::Stroke,
            format::KIND_OUTLINE => GlyphKind::Outline,
            _ => return None,
        };
        let advance = r.u16()?;
        let offset = r.u32()? as usize;
        let len = r.u32()? as usize;
        let geometry = self.geometry.get(offset..offset.checked_add(len)?)?;
        Some(GlyphView {
            ch,
            alternate,
            kind,
            advance,
            geometry,
        })
    }

    /// The glyph for `ch`: the highest-tagged alternate whose bit is
    /// set in `alternates` (bit `tag - 1`), else the default form.
    pub fn glyph(&self, ch: char, alternates: u32) -> Result<GlyphView<'a>, TypefaceError> {
        let n = self.glyph_count();
        let first = partition(n, |k| self.record(k).is_some_and(|g| g.ch < ch));
        let mut chosen = None;
        for k in first..n {
            let Some(g) = self.record(k) else { break };
            if g.ch != ch {
                break;
            }
            let enabled = g.alternate == 0
                || (g.alternate <= 32 && alternates & (1u32 << (g.alternate - 1)) != 0);
            if enabled {
                chosen = Some(g);
            }
        }
        chosen.ok_or(TypefaceError::MissingGlyph { ch })
    }

    /// The kerning adjustment between `left` and `right`, font units.
    #[must_use]
    pub fn kerning(&self, left: char, right: char) -> i16 {
        let n = self.kerning.len() / KERN_RECORD_LEN;
        let key = (u32::from(left), u32::from(right));
        let k = partition(n, |k| {
            self.kern_record(k).is_some_and(|(l, r, _)| (l, r) < key)
        });
        match self.kern_record(k) {
            Some((l, r, adjust)) if (l, r) == key => adjust,
            _ => 0,
        }
    }

    pub(crate) fn kern_record(&self, k: usize) -> Option<(u32, u32, i16)> {
        let mut r = Reader::at(self.kerning, k.checked_mul(KERN_RECORD_LEN)?);
        Some((r.u32()?, r.u32()?, r.i16()?))
    }
}

/// The first index in `0..n` for which `before` is false, by binary
/// search over a predicate that is true then false.
fn partition(n: usize, before: impl Fn(usize) -> bool) -> usize {
    let (mut lo, mut hi) = (0, n);
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if before(mid) {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    lo
}

#[cfg(test)]
mod tests;
