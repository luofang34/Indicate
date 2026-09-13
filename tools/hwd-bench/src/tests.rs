#![allow(clippy::expect_used)]
use super::*;
#[test]
fn every_review_case_fits_the_apple_scene_budget_and_supported_glyph_pack() {
    for case in cases::all() {
        let encoded = encode_case(&case).expect("bounded, supported review scene");
        assert!(!encoded.contains("NaN"));
        let scene = directions(&case.data);
        assert!(
            scene.strokes[..scene.length as usize]
                .iter()
                .flat_map(|s| s.a.into_iter().chain(s.b))
                .all(f32::is_finite)
        );
        assert!(directions(&case.data).length <= 1024);
    }
}
