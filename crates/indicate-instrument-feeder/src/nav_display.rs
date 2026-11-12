//! The navigation display profile (ADR-0031): the one place where the
//! wire's canonical guidance units become the instrument model's
//! display vocabulary.
//!
//! The wire carries meters, radians, and the phase scale the source
//! flies to. The HSI's CDI and vertical scale are calibrated in dots.
//! The meters a dot is worth in each phase depend on the airframe
//! class, so a [`LateralScaleProfile`] supplies them. They do not
//! travel on the wire, where they would bind every display to one
//! policy.

use indicate_instrument_state::{
    HeadingReference, NavData, NavFromTo, NavScale, NavSource, Stamped,
};

use crate::nav_guidance::NavSnapshot;

/// Dots from the center of the lateral scale to full-scale deflection.
///
/// A full-scale width is the distance at this many dots. The panel
/// draws its outer dot here and pegs the bar a little past it.
pub const FULL_SCALE_DOTS: f32 = 2.0;

/// Meters per nautical mile.
pub const M_PER_NM: f32 = 1852.0;

/// The full-scale lateral deflection, in meters, for each phase scale a
/// guidance source can declare.
///
/// The source declares the phase, and the profile gives its width. Two
/// dots is two dots on the glass whatever the phase. The profile is
/// therefore what makes a needle position mean a distance. A phase
/// never selects a width by default: an undeclared phase draws no
/// guidance, so one picture cannot silently mean two distances.
///
/// The profile has no `Default`. Each display chooses the profile for
/// its airframe class by name, or writes its own.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LateralScaleProfile {
    /// Full-scale width in the enroute phase, meters.
    pub enroute_full_scale_m: f32,
    /// Full-scale width in the terminal phase, meters.
    pub terminal_full_scale_m: f32,
    /// Full-scale width in the approach phase, meters.
    pub approach_full_scale_m: f32,
}

impl LateralScaleProfile {
    /// The widths a GNSS navigator of the RTCA DO-229 class flies on a
    /// linear CDI: 2.0 NM enroute, 1.0 NM terminal, and 0.3 NM
    /// approach.
    ///
    /// The profile is linear in each phase. It does not model the
    /// gradual change between phases, or the angular scale of a
    /// vertically guided approach. A source that flies an angular
    /// approach scales that deviation itself and fills the nav group
    /// directly.
    pub const GNSS_NAVIGATOR: Self = Self {
        enroute_full_scale_m: 2.0 * M_PER_NM,
        terminal_full_scale_m: 1.0 * M_PER_NM,
        approach_full_scale_m: 0.3 * M_PER_NM,
    };

    /// The widths a small unmanned airframe flies: 200 m enroute, 50 m
    /// terminal, and 20 m approach.
    pub const SMALL_UNMANNED: Self = Self {
        enroute_full_scale_m: 200.0,
        terminal_full_scale_m: 50.0,
        approach_full_scale_m: 20.0,
    };

    /// The full-scale width for `scale`. An unknown phase has none, and
    /// a width that is not a positive finite number is none too.
    #[must_use]
    pub fn full_scale_m(&self, scale: NavScale) -> Option<f32> {
        let width = match scale {
            NavScale::Enroute => self.enroute_full_scale_m,
            NavScale::Terminal => self.terminal_full_scale_m,
            NavScale::Approach => self.approach_full_scale_m,
            NavScale::Unknown => return None,
        };
        (width.is_finite() && width > 0.0).then_some(width)
    }

    /// Meters of cross-track deviation per dot for `scale`, or `None`
    /// when the profile has no width for it.
    #[must_use]
    pub fn m_per_dot(&self, scale: NavScale) -> Option<f32> {
        self.full_scale_m(scale)
            .map(|width| width / FULL_SCALE_DOTS)
    }
}

/// Full-scale vertical deflection is ±2.5 dots, so ±2.5 dots = ±20 m
/// off the vertical profile.
pub const VDEV_M_PER_DOT: f32 = 8.0;

/// Solution qualities this build can present. Unusable, and any coding
/// a later host introduces, remove the display rather than drawing
/// guidance the display cannot vouch for.
const PRESENTABLE_QUALITIES: [u32; 2] = [0, 1];

/// Converts one accepted guidance snapshot into the instrument model's
/// nav group, or `None` when guidance must not display at all —
/// the ADR-0031 contract that absent guidance is displayed as absent,
/// never as a centered needle.
///
/// The guidance's own phase scale selects the width from `profile`, and
/// its own TO/FROM resolution sets the flag. An unknown phase, a phase
/// the profile has no width for, or an unknown TO/FROM removes the
/// display.
pub fn nav_display_state(
    snapshot: Option<&NavSnapshot>,
    profile: &LateralScaleProfile,
) -> Option<Stamped<NavData>> {
    let snapshot = snapshot?;
    if !snapshot.age_ms.is_finite() {
        return None;
    }
    let guidance = &snapshot.guidance;
    if !PRESENTABLE_QUALITIES.contains(&guidance.solution_quality) {
        return None;
    }
    let m_per_dot = profile.m_per_dot(guidance.scale)?;
    let fromto = from_to(guidance.to_from, guidance.lateral_deviation_m)?;
    Some(Stamped {
        data: Some(NavData {
            source: NavSource::Gps,
            scale: guidance.scale,
            fromto,
            course_rad: guidance.course_rad,
            // The wire's course is measured from true north; the rose's
            // own reference and the wire both measure from true, so the
            // conversion needs no variation sample.
            course_reference: HeadingReference::True,
            // Without a flag there is no deviation bar. The deflection
            // goes to a finite zero that the panel does not paint: the
            // instrument model requires a finite CDI value, and NaN
            // would fail the whole group, including the course and
            // distance that are still valid.
            cdi_dots: if fromto == NavFromTo::Off {
                0.0
            } else {
                lateral_dots(guidance.lateral_deviation_m, m_per_dot)
            },
            vdev_dots: vertical_dots(guidance.vertical_deviation_m),
            dist_nm: Some(guidance.distance_to_waypoint_m / M_PER_NM),
            to_ident: guidance.to_ident,
            from_ident: guidance.from_ident,
        }),
        age_ms: Some(snapshot.age_ms as f32),
    })
}

/// The flag the panel draws: the source's own resolution while it
/// tracks a lateral course, and `Off` while it does not. A leg with no
/// cross-track geometry has no deviation to flag, whatever the source
/// states. An unknown resolution is `None`, because a flag the build
/// cannot read is not drawn as a known one.
fn from_to(stated: NavFromTo, lateral_deviation_m: f32) -> Option<NavFromTo> {
    match stated {
        NavFromTo::Unknown => None,
        _ if !lateral_deviation_m.is_finite() => Some(NavFromTo::Off),
        known => Some(known),
    }
}

/// Fly-to convention. The panel draws the deviation bar at
/// `cdi_dots * PX_PER_DOT` in a course-up frame where +x is the pilot's
/// right, and the bar marks where the course line is relative to
/// ownship. The wire's cross-track deviation is positive when ownship
/// is RIGHT of course, which puts the course to ownship's LEFT — so the
/// deflection is negative, and flying toward the bar closes the error.
///
/// The deflection is not limited here. The panel pegs the bar at its
/// own edge, so a deviation past full scale stays readable as one.
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
