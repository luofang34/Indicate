#![allow(clippy::expect_used, clippy::panic)]

use indicate_instrument_glyphs::typeface::{Face, TextStyle, Typeface};

use super::{FACES, bytes_of, relative_path, us_ms33558};
use crate::workspace::repo_root;

#[test]
fn the_committed_faces_are_what_their_sources_build() {
    for face in FACES {
        let committed = std::fs::read(repo_root().join(relative_path(face)))
            .expect("the face is committed; run `cargo xtask gen-typeface`");
        assert_eq!(
            committed,
            bytes_of(face).expect("encodes"),
            "{} drifted from its source; run `cargo xtask gen-typeface`",
            face.id
        );
    }
}

fn ms33558() -> (Vec<u8>, Face<'static>) {
    let bytes = bytes_of(&us_ms33558::FACE).expect("encodes");
    let leaked: &'static [u8] = Box::leak(bytes.clone().into_boxed_slice());
    let face = Face::typeface(Typeface::parse(leaked).expect("parses"), TextStyle::REGULAR)
        .expect("regular");
    (bytes, face)
}

/// Outside width and height of one character at a 1000-unit cap
/// height, so a font unit is an output unit.
fn outside(face: &Face<'_>, ch: char) -> (f32, f32) {
    let ink = face
        .ink_box(1000.0, &ch.to_string(), 0.01)
        .expect("lays out")
        .expect("inked");
    (ink.max_x - ink.min_x, ink.max_y - ink.min_y)
}

#[test]
fn the_face_keeps_the_mil_m_18012b_proportions() {
    let (_, face) = ms33558();
    let pen = f32::from(us_ms33558::FACE.default_pen) / f32::from(us_ms33558::FACE.cap_height);
    assert!(
        (1.0 / 8.0..=1.0 / 6.0).contains(&pen),
        "stroke {pen} of the height"
    );
    let off: Vec<String> = "023456789ABCDEFGHJKLNPRSTUVXYZ"
        .chars()
        .filter(|ch| *ch != '4')
        .map(|ch| (ch, outside(&face, ch)))
        .filter(|(_, (w, h))| !(585.0..=615.0).contains(w) || !(985.0..=1015.0).contains(h))
        .map(|(ch, (w, h))| format!("{ch} {w:.0}x{h:.0}"))
        .collect();
    assert!(off.is_empty(), "not 600 x 1000 outside: {off:?}");
    // "4" is one stroke wider; "1" and "I" are one stroke wide; "M" and
    // "W" are a fifth wider.
    assert!((outside(&face, '4').0 - 740.0).abs() < 15.0);
    for ch in ['1', 'I'] {
        assert!((outside(&face, ch).0 - 140.0).abs() < 1.0, "{ch}");
    }
    for ch in ['M', 'W'] {
        assert!((outside(&face, ch).0 - 720.0).abs() < 15.0, "{ch}");
    }
}

#[test]
fn the_face_covers_the_us_dial_legends() {
    let (_, face) = ms33558();
    // Every character the researched US legends paint, and every digit.
    let legends = [
        "M.P.H.",
        "AIR SPEED",
        "ALTITUDE",
        "1000 FEET",
        "INCH OF MERCURY",
        "CLIMB",
        "THOUSAND FT. PER MINUTE",
        "UP",
        "DOWN",
        "VERTICAL SPEED",
        "TURN",
        "BANK",
        "R.P.M.",
        "HUNDREDS",
        "MANIFOLD",
        "PRESSURE",
        "INCHES MERCURY",
        "ABSOLUTE",
        "TEMP. °C",
        "OIL",
        "FUEL",
        "LBS.",
        "SQ.IN.",
        "LEFT ENGINE RIGHT",
        "CYL. TEMP.",
        "CARB.",
        "FUEL AIR RATIO",
        "SUCTION",
        "8 DAYS",
        "KNOTS",
        "ALT",
        "100 FEET PER MIN",
        "DN",
        "TURN COORDINATOR",
        "2 MIN.",
        "NO PITCH INFORMATION",
        "VACUUM",
        "BC",
        "LOC",
        "OBS",
        "0123456789",
        "QWXZJ",
        "+-/=()×,",
    ];
    for legend in legends {
        face.advance_width(10.0, legend)
            .unwrap_or_else(|e| panic!("{legend:?}: {e}"));
    }
}

#[test]
fn the_flagged_one_is_an_alternate_of_the_plain_bar() {
    let (bytes, _) = ms33558();
    let face = Typeface::parse(&bytes).expect("parses");
    assert_eq!(face.alternate_tag("one.flag"), Some(us_ms33558::ONE_FLAG));
    let plain = face.glyph('1', 0).expect("plain one");
    let flagged = face
        .glyph('1', 1 << (us_ms33558::ONE_FLAG - 1))
        .expect("flagged one");
    assert_eq!(plain.alternate, 0);
    assert_eq!(flagged.alternate, us_ms33558::ONE_FLAG);
}
