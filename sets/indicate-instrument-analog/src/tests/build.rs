//! The physical build agrees with the 2D scene it stands beside.

use crate::{
    ALTIMETER, FACE_LAYERS, MAX_OUTLINE_POINTS, PointerShape, PointerStyle, physical_build,
    pointer_outline,
};

#[test]
fn the_altimeter_builds_three_stacked_hands_and_a_kollsman_drum() {
    let build = physical_build(&ALTIMETER).expect("builds");
    let heights: std::vec::Vec<f32> = build
        .pointers
        .iter()
        .flatten()
        .map(|p| p.stack_height_mm)
        .collect();
    assert_eq!(heights.len(), 3);
    assert!(heights.windows(2).all(|w| w[0] < w[1]), "{heights:?}");
    let window = build.windows[0].expect("a Kollsman window");
    assert_eq!(window.wheels, 4);
    let mm = ALTIMETER.face_radius_mm;
    assert!((window.aperture_mm[2] - 0.42 * mm).abs() < 1e-4);
    assert!(build.windows[1].is_none());
    assert_eq!(build.face_layers, FACE_LAYERS);
}

#[test]
fn a_built_outline_is_the_drawn_outline_in_millimetres() {
    let build = physical_build(&ALTIMETER).expect("builds");
    for (built, spec) in build.pointers.iter().flatten().zip(ALTIMETER.pointers) {
        let drawn = pointer_outline(&spec.style);
        assert_eq!(built.outline_mm.points().len(), drawn.points().len());
        for (a, b) in built.outline_mm.points().iter().zip(drawn.points()) {
            assert!((a[0] - b[0] * ALTIMETER.face_radius_mm).abs() < 1e-4);
            assert!((a[1] - b[1] * ALTIMETER.face_radius_mm).abs() < 1e-4);
        }
        assert_eq!(built.law, spec.law, "one law for both backends");
    }
}

#[test]
fn every_shape_fits_the_outline_capacity_and_points_at_twelve() {
    for shape in [
        PointerShape::Lance,
        PointerShape::Sword,
        PointerShape::Baton,
        PointerShape::Triangle,
    ] {
        let style = PointerStyle {
            shape,
            ..crate::ALTIMETER.pointers[2].style
        };
        let outline = pointer_outline(&style);
        assert!(outline.points().len() >= 4 && outline.points().len() <= MAX_OUTLINE_POINTS);
        let tip = outline
            .points()
            .iter()
            .map(|p| p[1])
            .fold(f32::INFINITY, f32::min);
        assert!((tip + style.length).abs() < 1e-6, "{shape:?} tip at {tip}");
    }
}
