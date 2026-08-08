use super::number;

/// The emitted bytes must depend on the value alone. A formatter that
/// chose a shortest round-trip form would make the manifest's diff
/// depend on the toolchain rather than on the tree.
#[test]
fn whole_values_print_without_a_fraction() {
    for (value, expected) in [(480.0, "480"), (0.0, "0"), (6.0, "6"), (65536.0, "65536")] {
        assert_eq!(number(value, "field").ok().as_deref(), Some(expected));
    }
}

#[test]
fn fractional_values_keep_their_digits_and_drop_the_padding() {
    assert_eq!(number(90.85715, "field").ok().as_deref(), Some("90.857147"));
    assert_eq!(number(0.5, "field").ok().as_deref(), Some("0.5"));
}

/// A value that cannot be pinned must stop the generator rather than
/// reach a consumer as a number that means nothing.
#[test]
fn non_finite_values_are_refused() {
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert!(number(bad, "field").is_err(), "{bad} is not pinnable");
    }
}
