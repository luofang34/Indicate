//! The navigation display profile (ADR-0031): the one place where the
//! wire's canonical guidance units become the instrument model's
//! display vocabulary.
//!
//! The wire carries meters and radians; the HSI's CDI and vertical
//! scale are calibrated in dots, and the meters-per-dot deflection is
//! per airframe class, so it lives here as named constants with tests
//! rather than on the wire where it would bind every display to one
//! policy.

use indicate_instrument_state::{
    HeadingReference, NavData, NavFromTo, NavScale, NavSource, Stamped,
};

use crate::nav_guidance::NavSnapshot;

/// Meters of cross-track error per dot, by declared scale.
///
/// Two dots is two dots on the glass whatever the phase, so the scale
/// is what makes a needle position mean a distance. The table is the
/// one place that mapping lives, and the scale the guidance declares is
/// what selects a row — never the distance, and never a default, or the
/// same picture would silently mean different things.
///
/// The values are the terminal-area scale a small unmanned airframe is
/// flown to, and one step either side of it.
pub const LATERAL_M_PER_DOT_ENROUTE: f32 = 100.0;
/// Meters of cross-track error per dot, terminal.
pub const LATERAL_M_PER_DOT_TERMINAL: f32 = 25.0;
/// Meters of cross-track error per dot, approach.
pub const LATERAL_M_PER_DOT_APPROACH: f32 = 10.0;

/// The lateral scale a declared mode selects. An undeclared or unknown
/// mode selects none: guidance whose scale is not known cannot be drawn
/// at any scale.
#[must_use]
pub const fn lateral_m_per_dot(scale: NavScale) -> Option<f32> {
    match scale {
        NavScale::Enroute => Some(LATERAL_M_PER_DOT_ENROUTE),
        NavScale::Terminal => Some(LATERAL_M_PER_DOT_TERMINAL),
        NavScale::Approach => Some(LATERAL_M_PER_DOT_APPROACH),
        NavScale::Unknown => None,
    }
}
/// Full-scale vertical deflection is ±2.5 dots, so ±2.5 dots = ±20 m
/// off the vertical profile.
pub const VDEV_M_PER_DOT: f32 = 8.0;
/// Meters per nautical mile.
pub const M_PER_NM: f32 = 1852.0;

/// Solution qualities this build can present. Unusable, and any coding
/// a later host introduces, remove the display rather than drawing
/// guidance the display cannot vouch for.
const PRESENTABLE_QUALITIES: [u32; 2] = [0, 1];

/// Converts one accepted guidance snapshot into the instrument model's
/// nav group, or `None` when guidance must not display at all —
/// the ADR-0031 contract that absent guidance is displayed as absent,
/// never as a centered needle.
pub fn nav_display_state(snapshot: Option<&NavSnapshot>) -> Option<Stamped<NavData>> {
    let snapshot = snapshot?;
    if !snapshot.age_ms.is_finite() {
        return None;
    }
    let guidance = &snapshot.guidance;
    if !PRESENTABLE_QUALITIES.contains(&guidance.solution_quality) {
        return None;
    }
    // Guidance that tracks no lateral course has no cross-track
    // geometry to draw. Clearing the TO/FROM flag removes the deviation
    // bar, and the deflection goes to a finite zero the panel never
    // paints — the instrument model requires a finite CDI value, and
    // NaN would fail the whole group including the course and distance
    // that are still valid.
    // The scale this feeder flies to. It is declared here rather than
    // read from the wire because this lane carries no scale of its own
    // yet; the panel is told which one the dots are on either way.
    const SCALE: NavScale = NavScale::Terminal;
    let m_per_dot = lateral_m_per_dot(SCALE)?;
    let tracking = guidance.lateral_deviation_m.is_finite();
    Some(Stamped {
        data: Some(NavData {
            source: NavSource::Gps,
            scale: SCALE,
            fromto: if tracking {
                NavFromTo::To
            } else {
                NavFromTo::Off
            },
            course_rad: guidance.course_rad,
            // The wire's course is measured from true north; the rose's
            // own reference and the wire both measure from true, so the
            // conversion needs no variation sample.
            course_reference: HeadingReference::True,
            cdi_dots: if tracking {
                lateral_dots(guidance.lateral_deviation_m, m_per_dot)
            } else {
                0.0
            },
            vdev_dots: vertical_dots(guidance.vertical_deviation_m),
            dist_nm: Some(guidance.distance_to_waypoint_m / M_PER_NM),
            to_ident: guidance.to_ident,
            from_ident: guidance.from_ident,
        }),
        age_ms: Some(snapshot.age_ms as f32),
    })
}

/// Fly-to convention. The panel draws the deviation bar at
/// `cdi_dots * PX_PER_DOT` in a course-up frame where +x is the pilot's
/// right, and the bar marks where the course line is relative to
/// ownship. The wire's cross-track deviation is positive when ownship
/// is RIGHT of course, which puts the course to ownship's LEFT — so the
/// deflection is negative, and flying toward the bar closes the error.
fn lateral_dots(lateral_deviation_m: f32, m_per_dot: f32) -> f32 {
    -lateral_deviation_m / m_per_dot
}

/// Fly-to convention, with the screen's downward y accounted for. The
/// panel draws the vertical pointer at `CY + vdev_dots * PX_PER_DOT`
/// where larger y is LOWER on the display, and the pointer marks where
/// the profile is relative to ownship. The wire's vertical deviation is
/// positive when ownship is ABOVE the profile, which puts the profile
/// BELOW ownship — lower on the display — so the deflection is
/// positive, and flying down toward the pointer closes the error. The
/// sign is therefore NOT the mirror of the lateral one: both are
/// fly-to, and the axes disagree on which way is up.
///
/// An unconstrained vertical profile stays `None`, the instrument
/// model's coding for a quantity with no sample, and the scale is not
/// drawn.
fn vertical_dots(vertical_deviation_m: f32) -> Option<f32> {
    vertical_deviation_m
        .is_finite()
        .then(|| vertical_deviation_m / VDEV_M_PER_DOT)
}

#[cfg(test)]
mod tests;
