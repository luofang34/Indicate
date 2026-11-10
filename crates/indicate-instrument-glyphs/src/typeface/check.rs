//! The checks a face passes before anything reads it.
//!
//! They make the bytes canonical as well as safe: tables strictly
//! ascending, glyph geometry laid out back to back in record order with
//! no gap, and nothing after the trailer. Two faces with equal content
//! therefore have equal bytes and equal hashes.

use indicate_sha256::sha256;

use super::format::{HASH_LEN, KIND_OUTLINE, KIND_STROKE, Reader, SEG_ARC, SEG_LINE};
use super::{Typeface, TypefaceError};

/// Largest alternate tag: a style enables alternates by a 32-bit mask.
const MAX_ALTERNATE_TAG: u8 = 32;

impl Typeface<'_> {
    /// Checks every rule the format states; [`Typeface::parse`] runs it.
    pub fn check(&self) -> Result<(), TypefaceError> {
        self.check_header()?;
        self.check_alternates()?;
        self.check_glyphs()?;
        self.check_kerning()?;
        let body = self.bytes.len().saturating_sub(HASH_LEN);
        let (content, trailer) = self.bytes.split_at(body);
        if sha256(content) != trailer {
            return Err(TypefaceError::ContentHashMismatch);
        }
        Ok(())
    }

    fn check_header(&self) -> Result<(), TypefaceError> {
        let bad = |field| Err(TypefaceError::InvalidHeader { field });
        if self.units_per_em == 0 {
            return bad("units_per_em");
        }
        if self.cap_height == 0 || i32::from(self.cap_height) > i32::from(self.ascender) {
            return bad("cap_height");
        }
        if self.descender > 0 {
            return bad("descender");
        }
        let id_ok = !self.id.is_empty()
            && self
                .id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        if !id_ok {
            return bad("id");
        }
        Ok(())
    }

    fn check_alternates(&self) -> Result<(), TypefaceError> {
        let mut r = Reader::new(self.alternates);
        let mut previous = 0u8;
        while let Some(tag) = r.u8() {
            let len = usize::from(r.u8().ok_or(TypefaceError::InvalidHeader {
                field: "alternates",
            })?);
            let name = r.take(len).ok_or(TypefaceError::InvalidHeader {
                field: "alternates",
            })?;
            if tag <= previous || tag > MAX_ALTERNATE_TAG {
                return Err(TypefaceError::Unsorted { table: "alternate" });
            }
            if name.is_empty() || !name.iter().all(|b| b.is_ascii_graphic()) {
                return Err(TypefaceError::InvalidHeader {
                    field: "alternates",
                });
            }
            previous = tag;
        }
        Ok(())
    }

    fn alternate_declared(&self, tag: u8) -> bool {
        let mut r = Reader::new(self.alternates);
        while let (Some(t), Some(len)) = (r.u8(), r.u8()) {
            if t == tag {
                return true;
            }
            if r.take(usize::from(len)).is_none() {
                return false;
            }
        }
        false
    }

    fn check_glyphs(&self) -> Result<(), TypefaceError> {
        let mut previous: Option<(char, u8)> = None;
        let mut expected_offset = 0usize;
        for k in 0..self.glyph_count() {
            let raw = self
                .raw_record(k)
                .ok_or(TypefaceError::InvalidGlyph { codepoint: 0 })?;
            let bad = TypefaceError::InvalidGlyph {
                codepoint: raw.codepoint,
            };
            let ch = char::from_u32(raw.codepoint).ok_or(bad)?;
            if previous.is_some_and(|p| p >= (ch, raw.alternate)) {
                return Err(TypefaceError::Unsorted { table: "glyph" });
            }
            previous = Some((ch, raw.alternate));
            if raw.alternate != 0 && !self.alternate_declared(raw.alternate) {
                return Err(bad);
            }
            if raw.offset != expected_offset {
                return Err(TypefaceError::TrailingBytes);
            }
            expected_offset = raw.offset.checked_add(raw.len).ok_or(bad)?;
            let geometry = self.geometry.get(raw.offset..expected_offset).ok_or(bad)?;
            let well_formed = match raw.kind {
                KIND_STROKE if self.default_pen > 0 => stroke_ok(geometry),
                KIND_OUTLINE => outline_ok(geometry),
                _ => false,
            };
            if !well_formed {
                return Err(bad);
            }
        }
        if expected_offset != self.geometry.len() {
            return Err(TypefaceError::TrailingBytes);
        }
        Ok(())
    }

    fn check_kerning(&self) -> Result<(), TypefaceError> {
        let mut previous: Option<(u32, u32)> = None;
        for k in 0..self.kerning.len() / super::format::KERN_RECORD_LEN {
            let (l, r, _) = self
                .kern_record(k)
                .ok_or(TypefaceError::Unsorted { table: "kerning" })?;
            if char::from_u32(l).is_none() || char::from_u32(r).is_none() {
                return Err(TypefaceError::InvalidGlyph { codepoint: l });
            }
            if previous.is_some_and(|p| p >= (l, r)) {
                return Err(TypefaceError::Unsorted { table: "kerning" });
            }
            previous = Some((l, r));
        }
        Ok(())
    }

    fn raw_record(&self, k: usize) -> Option<RawRecord> {
        let mut r = Reader::at(self.glyphs, k.checked_mul(super::format::GLYPH_RECORD_LEN)?);
        Some(RawRecord {
            codepoint: r.u32()?,
            alternate: r.u8()?,
            kind: r.u8()?,
            offset: {
                r.u16()?;
                r.u32()? as usize
            },
            len: r.u32()? as usize,
        })
    }
}

struct RawRecord {
    codepoint: u32,
    alternate: u8,
    kind: u8,
    offset: usize,
    len: usize,
}

/// Whether `bytes` is exactly one well-formed stroke geometry.
fn stroke_ok(bytes: &[u8]) -> bool {
    let mut r = Reader::new(bytes);
    let Some(strokes) = r.u16() else { return false };
    for _ in 0..strokes {
        let (Some(segments), Some(_)) = (r.u16(), r.point()) else {
            return false;
        };
        for _ in 0..segments {
            let ok = match r.u8() {
                Some(SEG_LINE) => r.point().is_some(),
                Some(SEG_ARC) => arc_ok(&mut r),
                _ => false,
            };
            if !ok {
                return false;
            }
        }
    }
    r.pos() == bytes.len()
}

fn arc_ok(r: &mut Reader<'_>) -> bool {
    let (Some(_), Some(rx), Some(ry), Some(_), Some(sweep)) =
        (r.point(), r.u16(), r.u16(), r.i16(), r.i16())
    else {
        return false;
    };
    rx > 0 && ry > 0 && sweep != 0 && sweep.unsigned_abs() <= 3600
}

/// Whether `bytes` is exactly one well-formed outline geometry: every
/// contour has at least three points.
fn outline_ok(bytes: &[u8]) -> bool {
    let mut r = Reader::new(bytes);
    let Some(contours) = r.u16() else {
        return false;
    };
    for _ in 0..contours {
        let Some(points) = r.u16() else { return false };
        if points < 3 {
            return false;
        }
        for _ in 0..points {
            if r.point().is_none() {
                return false;
            }
        }
    }
    r.pos() == bytes.len()
}
