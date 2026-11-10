//! The faces the per-face checks run over: the reference 5×7 pack and
//! every face the repository ships as test data, at the styles a
//! cockpit binds them in.

use indicate_instrument_glyphs::typeface::{Face, TextStyle, Typeface};

/// The `us-ms33558` stroke face, as committed under `testdata/`.
pub(super) const MS33558: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/typefaces/us-ms33558/us-ms33558.idtf"
));

/// The narrow, heavy lettering of 1940s Kollsman dials: the same
/// skeleton at three quarters of its width and a sixth-of-height pen.
pub(super) const KOLLSMAN_1940: TextStyle = TextStyle {
    condense: 0.75,
    pen: Some(1.0 / 6.0),
    slant: 0.0,
    alternates: 0,
};

pub(super) fn ms33558(style: TextStyle) -> Face<'static> {
    Face::typeface(
        Typeface::parse(MS33558).expect("the shipped face parses"),
        style,
    )
    .expect("a valid style")
}

/// Every face and style the per-face checks cover.
pub(super) fn every_face() -> [(&'static str, Face<'static>); 3] {
    [
        ("reference-5x7", Face::Reference),
        ("us-ms33558", ms33558(TextStyle::REGULAR)),
        ("us-ms33558-kollsman-1940", ms33558(KOLLSMAN_1940)),
    ]
}
