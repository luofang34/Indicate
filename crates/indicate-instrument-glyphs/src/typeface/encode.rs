//! Building a face's bytes from borrowed definitions, with no
//! allocation: the tool that authors a face and a test that needs one
//! both write through here, so every face on disk is canonical.

use indicate_sha256::sha256;

use super::format::{
    GLYPH_RECORD_LEN, HASH_LEN, HEADER_LEN, KERN_RECORD_LEN, KIND_OUTLINE, KIND_STROKE, SEG_ARC,
    SEG_LINE, TYPEFACE_FORMAT_VERSION, TYPEFACE_MAGIC,
};
use super::{Typeface, TypefaceError};

/// One stroke segment, in font units (y-up, baseline 0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Seg {
    /// A line from the pen to a point.
    Line([i16; 2]),
    /// An elliptical arc, joined to the pen by a line when it starts
    /// elsewhere. Angles are tenths of a degree, counter-clockwise from
    /// `+x`.
    Arc {
        /// Centre.
        center: [i16; 2],
        /// Radii along x and y.
        radii: [u16; 2],
        /// Start angle.
        start: i16,
        /// Sweep; positive turns counter-clockwise.
        sweep: i16,
    },
}

/// One stroke: a start point and the segments that follow it. No
/// segments is a dot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StrokeDef<'a> {
    /// Where the pen starts.
    pub start: [i16; 2],
    /// The segments.
    pub segments: &'a [Seg],
}

/// A glyph's geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape<'a> {
    /// Centreline strokes, drawn with the face's pen.
    Stroke(&'a [StrokeDef<'a>]),
    /// Closed contours, filled by the non-zero winding rule.
    Outline(&'a [&'a [[i16; 2]]]),
}

/// One glyph definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlyphDef<'a> {
    /// The character.
    pub ch: char,
    /// Alternate tag; 0 is the default form.
    pub alternate: u8,
    /// Advance, font units.
    pub advance: u16,
    /// Geometry.
    pub shape: Shape<'a>,
}

/// One kerning pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Kern {
    /// The left character.
    pub left: char,
    /// The right character.
    pub right: char,
    /// The adjustment, font units; negative tightens.
    pub adjust: i16,
}

/// A whole face. Glyphs, kerning pairs and alternates must already be
/// in canonical order; [`encode_face`] refuses them otherwise rather
/// than sorting, so it needs no allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FaceDef<'a> {
    /// Identity: lowercase ASCII, digits and dashes.
    pub id: &'a str,
    /// Font units per em.
    pub units_per_em: u16,
    /// Cap height, font units.
    pub cap_height: u16,
    /// Ascender, font units.
    pub ascender: i16,
    /// Descender, font units (zero or negative).
    pub descender: i16,
    /// Default pen for stroke glyphs, font units.
    pub default_pen: u16,
    /// Alternate names by tag, tags ascending from 1.
    pub alternates: &'a [(u8, &'a str)],
    /// Glyphs, ascending by (character, alternate).
    pub glyphs: &'a [GlyphDef<'a>],
    /// Kerning pairs, ascending by (left, right).
    pub kerning: &'a [Kern],
}

fn geometry_len(shape: &Shape<'_>) -> usize {
    match shape {
        Shape::Stroke(strokes) => {
            2 + strokes
                .iter()
                .map(|s| {
                    6 + s
                        .segments
                        .iter()
                        .map(|seg| match seg {
                            Seg::Line(_) => 5,
                            Seg::Arc { .. } => 13,
                        })
                        .sum::<usize>()
                })
                .sum::<usize>()
        }
        Shape::Outline(contours) => 2 + contours.iter().map(|c| 2 + 4 * c.len()).sum::<usize>(),
    }
}

/// Bytes [`encode_face`] writes for `def`.
#[must_use]
pub fn encoded_len(def: &FaceDef<'_>) -> usize {
    HEADER_LEN
        + def.id.len()
        + def
            .alternates
            .iter()
            .map(|(_, n)| 2 + n.len())
            .sum::<usize>()
        + def.glyphs.len() * GLYPH_RECORD_LEN
        + def.kerning.len() * KERN_RECORD_LEN
        + 4
        + def
            .glyphs
            .iter()
            .map(|g| geometry_len(&g.shape))
            .sum::<usize>()
        + HASH_LEN
}

struct Writer<'b> {
    out: &'b mut [u8],
    pos: usize,
}

impl Writer<'_> {
    fn put(&mut self, bytes: &[u8]) -> Result<(), TypefaceError> {
        let end = self.pos + bytes.len();
        let available = self.out.len();
        self.out
            .get_mut(self.pos..end)
            .ok_or(TypefaceError::BufferTooSmall {
                needed: end,
                available,
            })?
            .copy_from_slice(bytes);
        self.pos = end;
        Ok(())
    }

    fn u16(&mut self, v: u16) -> Result<(), TypefaceError> {
        self.put(&v.to_le_bytes())
    }

    fn i16(&mut self, v: i16) -> Result<(), TypefaceError> {
        self.put(&v.to_le_bytes())
    }

    fn point(&mut self, p: [i16; 2]) -> Result<(), TypefaceError> {
        self.i16(p[0])?;
        self.i16(p[1])
    }
}

fn narrow<T: TryFrom<usize>>(n: usize, field: &'static str) -> Result<T, TypefaceError> {
    T::try_from(n).map_err(|_| TypefaceError::InvalidHeader { field })
}

/// Writes `def` as a face, hash trailer included, into `out`; returns
/// the length. The result is parsed back before returning, so a face
/// this function writes is one [`Typeface::parse`] accepts.
pub fn encode_face(def: &FaceDef<'_>, out: &mut [u8]) -> Result<usize, TypefaceError> {
    let needed = encoded_len(def);
    if out.len() < needed {
        return Err(TypefaceError::BufferTooSmall {
            needed,
            available: out.len(),
        });
    }
    let mut w = Writer { out, pos: 0 };
    write_header(&mut w, def)?;
    let mut offset = 0usize;
    for g in def.glyphs {
        let len = geometry_len(&g.shape);
        w.put(&u32::from(g.ch).to_le_bytes())?;
        w.put(&[g.alternate])?;
        w.put(&[match g.shape {
            Shape::Stroke(_) => KIND_STROKE,
            Shape::Outline(_) => KIND_OUTLINE,
        }])?;
        w.u16(g.advance)?;
        w.put(&narrow::<u32>(offset, "geometry")?.to_le_bytes())?;
        w.put(&narrow::<u32>(len, "geometry")?.to_le_bytes())?;
        offset += len;
    }
    for k in def.kerning {
        w.put(&u32::from(k.left).to_le_bytes())?;
        w.put(&u32::from(k.right).to_le_bytes())?;
        w.i16(k.adjust)?;
    }
    w.put(&narrow::<u32>(offset, "geometry")?.to_le_bytes())?;
    for g in def.glyphs {
        write_shape(&mut w, &g.shape)?;
    }
    let body = w.pos;
    let hash = sha256(w.out.get(..body).unwrap_or(&[]));
    w.put(&hash)?;
    let written = w.pos;
    Typeface::parse(out.get(..written).unwrap_or(&[]))?;
    Ok(written)
}

fn write_header(w: &mut Writer<'_>, def: &FaceDef<'_>) -> Result<(), TypefaceError> {
    w.put(&TYPEFACE_MAGIC)?;
    w.u16(TYPEFACE_FORMAT_VERSION)?;
    w.u16(0)?;
    w.u16(def.units_per_em)?;
    w.u16(def.cap_height)?;
    w.i16(def.ascender)?;
    w.i16(def.descender)?;
    w.u16(def.default_pen)?;
    w.u16(narrow(def.glyphs.len(), "glyph count")?)?;
    w.u16(narrow(def.kerning.len(), "kerning count")?)?;
    w.put(&[narrow(def.alternates.len(), "alternate count")?])?;
    w.put(&[narrow(def.id.len(), "id")?])?;
    w.put(def.id.as_bytes())?;
    for (tag, name) in def.alternates {
        w.put(&[*tag, narrow(name.len(), "alternates")?])?;
        w.put(name.as_bytes())?;
    }
    Ok(())
}

fn write_shape(w: &mut Writer<'_>, shape: &Shape<'_>) -> Result<(), TypefaceError> {
    match shape {
        Shape::Stroke(strokes) => {
            w.u16(narrow(strokes.len(), "strokes")?)?;
            for s in *strokes {
                w.u16(narrow(s.segments.len(), "segments")?)?;
                w.point(s.start)?;
                for seg in s.segments {
                    match *seg {
                        Seg::Line(p) => {
                            w.put(&[SEG_LINE])?;
                            w.point(p)?;
                        }
                        Seg::Arc {
                            center,
                            radii,
                            start,
                            sweep,
                        } => {
                            w.put(&[SEG_ARC])?;
                            w.point(center)?;
                            w.u16(radii[0])?;
                            w.u16(radii[1])?;
                            w.i16(start)?;
                            w.i16(sweep)?;
                        }
                    }
                }
            }
        }
        Shape::Outline(contours) => {
            w.u16(narrow(contours.len(), "contours")?)?;
            for c in *contours {
                w.u16(narrow(c.len(), "points")?)?;
                for p in *c {
                    w.point(*p)?;
                }
            }
        }
    }
    Ok(())
}
