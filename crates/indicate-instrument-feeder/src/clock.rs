//! The caller's clock, judged once for every lane (AIR-IN-008,
//! AIR-FLAG-003).
//!
//! Every tracker ages its admitted state against a millisecond clock
//! the shell supplies. A reading that is not a number, runs backwards,
//! or starts before zero cannot age anything, and an age computed from
//! it would read as fresh: `f64::max` discards a NaN operand, and every
//! comparison against NaN is false, so a broken clock would pass for a
//! perfect one. The rule lives here so no lane ships a weaker copy.

/// Whether `now_ms` is a reading a tracker may act on: finite,
/// non-negative, and not behind the last reading the tracker acted on.
pub(crate) fn clock_ok(now_ms: f64, observed_ms: Option<f64>) -> bool {
    now_ms.is_finite() && now_ms >= 0.0 && observed_ms.is_none_or(|last| now_ms >= last)
}

/// Milliseconds from `since_ms` to `now_ms`, or NaN when `now_ms` is a
/// reading the tracker may not act on.
///
/// NaN is the fail-closed age: the instrument model's freshness policy
/// resolves a NaN age as `Missing`, and no consumer may read it as
/// fresh or as any finite number.
pub(crate) fn age_ms(now_ms: f64, since_ms: f64, observed_ms: Option<f64>) -> f64 {
    if clock_ok(now_ms, observed_ms) {
        (now_ms - since_ms).max(0.0)
    } else {
        f64::NAN
    }
}

#[cfg(test)]
mod tests;
