#![allow(clippy::expect_used, clippy::panic)]

use std::vec;
use std::vec::Vec;

use super::{
    Face, FaceDef, GlyphDef, GlyphKind, Kern, Seg, Shape, StrokeDef, TextStyle, Typeface,
    TypefaceError, encode_face, encoded_len, tessellate_run,
};

const BAR: &[StrokeDef<'static>] = &[StrokeDef {
    start: [100, 0],
    segments: &[Seg::Line([100, 1000])],
}];
const FLAG: &[StrokeDef<'static>] = &[StrokeDef {
    start: [250, 0],
    segments: &[Seg::Line([250, 1000]), Seg::Line([100, 850])],
}];
const RING: &[StrokeDef<'static>] = &[StrokeDef {
    start: [400, 500],
    segments: &[Seg::Arc {
        center: [300, 500],
        radii: [200, 430],
        start: 0,
        sweep: 3600,
    }],
}];
const DOT: &[StrokeDef<'static>] = &[StrokeDef {
    start: [100, 70],
    segments: &[],
}];
/// A square with a square hole, wound so the hole is empty.
const BOX: &[&[[i16; 2]]] = &[
    &[[0, 0], [600, 0], [600, 1000], [0, 1000]],
    &[[150, 150], [150, 850], [450, 850], [450, 150]],
];

pub(crate) const TEST_FACE: FaceDef<'static> = FaceDef {
    id: "test-face",
    units_per_em: 1250,
    cap_height: 1000,
    ascender: 1000,
    descender: -250,
    default_pen: 140,
    alternates: &[(1, "one.flag")],
    glyphs: &[
        GlyphDef {
            ch: ' ',
            alternate: 0,
            advance: 400,
            shape: Shape::Stroke(&[]),
        },
        GlyphDef {
            ch: '.',
            alternate: 0,
            advance: 340,
            shape: Shape::Stroke(DOT),
        },
        GlyphDef {
            ch: '0',
            alternate: 0,
            advance: 800,
            shape: Shape::Stroke(RING),
        },
        GlyphDef {
            ch: '1',
            alternate: 0,
            advance: 340,
            shape: Shape::Stroke(BAR),
        },
        GlyphDef {
            ch: '1',
            alternate: 1,
            advance: 490,
            shape: Shape::Stroke(FLAG),
        },
        GlyphDef {
            ch: 'O',
            alternate: 0,
            advance: 700,
            shape: Shape::Outline(BOX),
        },
    ],
    kerning: &[Kern {
        left: '1',
        right: '0',
        adjust: -60,
    }],
};

pub(crate) fn test_face_bytes() -> Vec<u8> {
    let mut buf = vec![0u8; encoded_len(&TEST_FACE)];
    let n = encode_face(&TEST_FACE, &mut buf).expect("encodes");
    assert_eq!(n, buf.len());
    buf
}

#[test]
fn a_face_round_trips_with_its_metrics_and_hash() {
    let bytes = test_face_bytes();
    let face = Typeface::parse(&bytes).expect("parses");
    assert_eq!(face.id(), "test-face");
    assert_eq!(face.cap_height(), 1000);
    assert_eq!(face.glyph_count(), 6);
    assert_eq!(
        face.content_hash(),
        indicate_sha256::sha256(&bytes[..bytes.len() - 32])
    );
    assert_eq!(face.glyph('O', 0).expect("O").kind, GlyphKind::Outline);
    assert_eq!(face.kerning('1', '0'), -60);
    assert_eq!(face.kerning('0', '1'), 0);
    assert_eq!(face.alternate_tag("one.flag"), Some(1));
    assert_eq!(face.glyph('1', 0).expect("1").advance, 340);
    assert_eq!(face.glyph('1', 0b1).expect("flagged 1").advance, 490);
    assert_eq!(
        face.glyph('X', 0),
        Err(TypefaceError::MissingGlyph { ch: 'X' })
    );
}

#[test]
fn equal_definitions_give_equal_bytes() {
    assert_eq!(test_face_bytes(), test_face_bytes());
}

#[test]
fn any_corrupted_byte_is_refused() {
    let bytes = test_face_bytes();
    for k in 0..bytes.len() {
        let mut bad = bytes.clone();
        bad[k] ^= 0x40;
        assert!(
            Typeface::parse(&bad).is_err(),
            "a flip at byte {k} was accepted"
        );
    }
    assert!(Typeface::parse(&bytes[..bytes.len() - 1]).is_err());
    let mut longer = bytes.clone();
    longer.push(0);
    assert_eq!(Typeface::parse(&longer), Err(TypefaceError::TrailingBytes));
}

#[test]
fn unsorted_glyphs_are_refused_by_the_encoder() {
    let glyphs = [TEST_FACE.glyphs[3], TEST_FACE.glyphs[2]];
    let def = FaceDef {
        glyphs: &glyphs,
        kerning: &[],
        ..TEST_FACE
    };
    let mut buf = vec![0u8; encoded_len(&def)];
    assert_eq!(
        encode_face(&def, &mut buf),
        Err(TypefaceError::Unsorted { table: "glyph" })
    );
}

#[test]
fn run_metrics_follow_advance_kerning_and_style() {
    let bytes = test_face_bytes();
    let face = Typeface::parse(&bytes).expect("parses");
    let regular = Face::typeface(face, TextStyle::REGULAR).expect("style");
    // At size 10 a font unit is 0.01: "10" is 340 + 800 - 60 units.
    let width = regular.advance_width(10.0, "10").expect("lays out");
    assert!((width - 10.8).abs() < 1e-4, "{width}");
    let narrow = Face::typeface(
        face,
        TextStyle {
            condense: 0.5,
            ..TextStyle::REGULAR
        },
    )
    .expect("style");
    let narrow_width = narrow.advance_width(10.0, "10").expect("lays out");
    assert!((narrow_width - 5.4).abs() < 1e-4, "{narrow_width}");
    // A heavier pen widens each glyph by its extra ink.
    let heavy = Face::typeface(
        face,
        TextStyle {
            pen: Some(0.24),
            ..TextStyle::REGULAR
        },
    )
    .expect("style");
    let heavy_width = heavy.advance_width(10.0, "1").expect("lays out");
    assert!((heavy_width - (3.4 + 1.0)).abs() < 1e-4, "{heavy_width}");
    let flagged = Face::typeface(
        face,
        TextStyle {
            alternates: 1,
            ..TextStyle::REGULAR
        },
    )
    .expect("style");
    assert!((flagged.advance_width(10.0, "1").expect("lays out") - 4.9).abs() < 1e-4);
}

#[test]
fn the_ink_box_holds_the_pen_on_both_sides_of_the_centreline() {
    let bytes = test_face_bytes();
    let face = Face::typeface(Typeface::parse(&bytes).expect("parses"), TextStyle::REGULAR)
        .expect("style");
    let ink = face
        .ink_box(10.0, "1", 0.0)
        .expect("lays out")
        .expect("inked");
    // The bar's centreline is x = 1, y from 0 to -10; the pen is 1.4.
    assert!(
        (ink.min_x - 0.3).abs() < 1e-4 && (ink.max_x - 1.7).abs() < 1e-4,
        "{ink:?}"
    );
    assert!(
        (ink.min_y + 10.7).abs() < 1e-4 && (ink.max_y - 0.7).abs() < 1e-4,
        "{ink:?}"
    );
    assert_eq!(face.ink_box(10.0, " ", 0.0).expect("lays out"), None);
}

#[test]
fn a_bad_style_is_refused() {
    let bytes = test_face_bytes();
    let face = Typeface::parse(&bytes).expect("parses");
    for style in [
        TextStyle {
            condense: 0.0,
            ..TextStyle::REGULAR
        },
        TextStyle {
            pen: Some(f32::NAN),
            ..TextStyle::REGULAR
        },
        TextStyle {
            slant: 2.0,
            ..TextStyle::REGULAR
        },
    ] {
        assert!(matches!(
            Face::typeface(face, style),
            Err(TypefaceError::InvalidStyle { .. })
        ));
    }
}

fn area(triangles: &[[[f32; 2]; 3]]) -> f32 {
    triangles
        .iter()
        .map(|[a, b, c]| {
            ((b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1])).abs() * 0.5
        })
        .sum()
}

#[test]
fn an_outline_tessellates_to_its_exact_area_with_the_hole_left_empty() {
    let bytes = test_face_bytes();
    let face = Face::typeface(Typeface::parse(&bytes).expect("parses"), TextStyle::REGULAR)
        .expect("style");
    let mut triangles = Vec::new();
    tessellate_run(&face, 10.0, "O", 0.01, |t| triangles.push(t)).expect("tessellates");
    // 6 × 10 minus the 3 × 7 hole.
    assert!(
        (area(&triangles) - 39.0).abs() < 1e-3,
        "{}",
        area(&triangles)
    );
}

#[test]
fn a_stroke_tessellates_to_its_capsule() {
    let bytes = test_face_bytes();
    let face = Face::typeface(Typeface::parse(&bytes).expect("parses"), TextStyle::REGULAR)
        .expect("style");
    let mut triangles = Vec::new();
    tessellate_run(&face, 10.0, "1", 0.001, |t| triangles.push(t)).expect("tessellates");
    let ink = face
        .ink_box(10.0, "1", 0.001)
        .expect("lays out")
        .expect("inked");
    for t in &triangles {
        for p in t {
            assert!(
                p[0] >= ink.min_x - 1e-4 && p[0] <= ink.max_x + 1e-4,
                "{p:?}"
            );
            assert!(
                p[1] >= ink.min_y - 1e-4 && p[1] <= ink.max_y + 1e-4,
                "{p:?}"
            );
        }
    }
    // A 10 × 1.4 rectangle plus two half discs, counted twice where
    // the discs overlap the rectangle: at least the capsule's area.
    let capsule = 10.0 * 1.4 + core::f32::consts::PI * 0.7 * 0.7;
    assert!(area(&triangles) >= capsule * 0.99, "{}", area(&triangles));
}

#[test]
fn the_reference_face_tessellates_its_pixels() {
    let mut triangles = Vec::new();
    tessellate_run(&Face::Reference, 7.0, "1", 0.0, |t| triangles.push(t)).expect("tessellates");
    let lit = (0..7)
        .flat_map(|r| (0..5).map(move |c| (c, r)))
        .filter(|(c, r)| {
            crate::PANEL_GLYPHS
                .glyph('1')
                .expect("1")
                .glyph
                .pixel(*c, *r)
        })
        .count();
    assert_eq!(triangles.len(), lit * 2);
    assert!((area(&triangles) - lit as f32).abs() < 1e-4);
}
