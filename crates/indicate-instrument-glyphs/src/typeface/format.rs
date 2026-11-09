//! The byte layout of a typeface, and a bounds-checked reader over it.
//!
//! Every field is little-endian, with no padding and no alignment, so
//! the bytes are their own canonical serialisation:
//!
//! | offset | field |
//! |-------:|-------|
//! | 0 | magic `IDTF` |
//! | 4 | format version u16 (= [`TYPEFACE_FORMAT_VERSION`]) |
//! | 6 | reserved u16, zero |
//! | 8 | units per em u16 |
//! | 10 | cap height u16 |
//! | 12 | ascender i16 |
//! | 14 | descender i16 (zero or negative) |
//! | 16 | default pen u16, in font units (zero for a face with no stroke glyph) |
//! | 18 | glyph count u16 |
//! | 20 | kerning pair count u16 |
//! | 22 | alternate name count u8 |
//! | 23 | id length u8 |
//! | 24 | id, ASCII |
//! | … | alternate names: tag u8, length u8, ASCII name; tags ascending from 1 |
//! | … | glyph records, 16 bytes each, ascending by (code point, alternate) |
//! | … | kerning pairs, 10 bytes each, ascending by (left, right) |
//! | … | geometry length u32, then the geometry of every glyph |
//! | … | SHA-256 of every byte before it |
//!
//! A glyph record is: code point u32, alternate tag u8 (0 = the default
//! form), kind u8 (1 stroke, 2 outline), advance u16, geometry offset
//! u32 and geometry length u32, both into the geometry block.
//!
//! Stroke geometry is a stroke count u16, then per stroke a segment
//! count u16, a start point (two i16), and its segments: `1` line to a
//! point (two i16), or `2` an elliptical arc (centre two i16, radii two
//! u16, start and sweep angles i16 in tenths of a degree,
//! counter-clockwise from `+x` in the y-up font space). An arc begins at
//! its own start point; when the pen is elsewhere, a straight join leads
//! to it. A stroke with no segments is a dot.
//!
//! Outline geometry is a contour count u16, then per contour a point
//! count u16 and its points (two i16 each), closed implicitly and filled
//! by the non-zero winding rule. Curves are flattened when the face is
//! built.

/// The format version a parser of this build reads.
pub const TYPEFACE_FORMAT_VERSION: u16 = 2;

/// The four bytes every typeface starts with.
pub const TYPEFACE_MAGIC: [u8; 4] = *b"IDTF";

/// Bytes of the fixed header, before the id.
pub(crate) const HEADER_LEN: usize = 24;

/// Bytes of one glyph record.
pub(crate) const GLYPH_RECORD_LEN: usize = 16;

/// Bytes of one kerning pair.
pub(crate) const KERN_RECORD_LEN: usize = 10;

/// Bytes of the trailing content hash.
pub(crate) const HASH_LEN: usize = 32;

/// Stroke segment opcode: a line to a point.
pub(crate) const SEG_LINE: u8 = 1;

/// Stroke segment opcode: an elliptical arc.
pub(crate) const SEG_ARC: u8 = 2;

/// Glyph kind byte: centreline strokes with a pen.
pub(crate) const KIND_STROKE: u8 = 1;

/// Glyph kind byte: filled contours.
pub(crate) const KIND_OUTLINE: u8 = 2;

/// A forward reader over borrowed bytes; every read is bounds-checked
/// and a short read is `None`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub(crate) const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    pub(crate) const fn at(bytes: &'a [u8], pos: usize) -> Self {
        Self { bytes, pos }
    }

    pub(crate) const fn pos(&self) -> usize {
        self.pos
    }

    pub(crate) fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let end = self.pos.checked_add(n)?;
        let out = self.bytes.get(self.pos..end)?;
        self.pos = end;
        Some(out)
    }

    pub(crate) fn u8(&mut self) -> Option<u8> {
        self.take(1).and_then(|b| b.first().copied())
    }

    pub(crate) fn u16(&mut self) -> Option<u16> {
        let b: [u8; 2] = self.take(2)?.try_into().ok()?;
        Some(u16::from_le_bytes(b))
    }

    pub(crate) fn i16(&mut self) -> Option<i16> {
        let b: [u8; 2] = self.take(2)?.try_into().ok()?;
        Some(i16::from_le_bytes(b))
    }

    pub(crate) fn u32(&mut self) -> Option<u32> {
        let b: [u8; 4] = self.take(4)?.try_into().ok()?;
        Some(u32::from_le_bytes(b))
    }

    pub(crate) fn point(&mut self) -> Option<[i16; 2]> {
        Some([self.i16()?, self.i16()?])
    }
}
