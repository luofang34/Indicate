//! The navigation display profile (ADR-0031): the one place where the
//! wire's canonical guidance units become the instrument model's
//! display vocabulary.
//!
//! The wire carries meters, radians, and the phase scale the source
//! flies to. Every deviation display is calibrated in dots: the HSI,
//! the HUD approach scales, and the head-worn CDI all read the same nav
//! group this module writes. The meters a dot is worth in each phase
//! depend on the airframe class, so a [`DeviationScaleProfile`]
//! supplies them. They do not travel on the wire, where they would bind
//! every display to one policy.

use indicate_instrument_state::{
    HeadingReference, NavData, NavFromTo, NavScale, NavSource, Stamped,
};

use crate::nav_guidance::NavSnapshot;

/// Dots from the center of a deviation scale to its outer dot.
///
/// A width in a [`PhaseWidths`] is the distance at this many dots, on
/// both axes. The displays draw their outer dot here and peg the
/// pointer a little past it.
pub const OUTER_DOT: f32 = 2.0;

/// Meters per nautical mile.
pub const M_PER_NM: f32 = 1852.0;

/// Meters per foot.
pub const M_PER_FT: f32 = 0.3048;

/// The deviation at the outer dot in one phase, meters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhaseWidths {
    /// Cross-track deviation at the outer dot, meters.
    pub lateral_m: f32,
    /// Vertical deviation at the outer dot, meters.
    pub vertical_m: f32,
}

/// The deviation widths for each phase scale a guidance source can
/// declare.
///
/// The source declares the phase, and the profile gives its widths. Two
/// dots is two dots on the glass whatever the phase. The profile is
/// therefore what makes a pointer position mean a distance. A phase
/// never selects a width by default: an undeclared phase draws no
/// guidance, so one picture cannot silently mean two distances.
///
/// The profile has no `Default`. Each display chooses the profile for
/// its airframe class by name, or writes its own.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeviationScaleProfile {
    /// Widths in the enroute phase.
    pub enroute: PhaseWidths,
    /// Widths in the terminal phase.
    pub terminal: PhaseWidths,
    /// Widths in the approach phase.
    pub approach: PhaseWidths,
}

impl DeviationScaleProfile {
    /// The widths a GNSS navigator flies on linear deviation scales.
    ///
    /// Lateral: 2.0 NM enroute, 1.0 NM terminal, and 0.3 NM approach,
    /// the RTCA DO-229 phase widths. Vertical: 1000 ft enroute and
    /// terminal, the baro-VNAV indicator of a Garmin G1000 class
    /// display, and 150 ft approach, the width a Honeywell FMS gives
    /// within 2 NM of the final approach fix.
    ///
    /// The profile is linear in each phase. It does not model the
    /// gradual change between phases, or the angular scale of a
    /// vertically guided approach. A source that flies an angular
    /// approach scales that deviation itself and fills the nav group
    /// directly.
    pub const GNSS_NAVIGATOR: Self = Self {
        enroute: PhaseWidths {
            lateral_m: 2.0 * M_PER_NM,
            vertical_m: 1000.0 * M_PER_FT,
        },
        terminal: PhaseWidths {
            lateral_m: 1.0 * M_PER_NM,
            vertical_m: 1000.0 * M_PER_FT,
        },
        approach: PhaseWidths {
            lateral_m: 0.3 * M_PER_NM,
            vertical_m: 150.0 * M_PER_FT,
        },
    };

    /// The widths a small unmanned airframe flies: 200 m, 50 m, and
    /// 20 m lateral, and 16 m vertical in every phase.
    pub const SMALL_UNMANNED: Self = Self {
        enroute: PhaseWidths {
            lateral_m: 200.0,
            vertical_m: 16.0,
        },
        terminal: PhaseWidths {
            lateral_m: 50.0,
            vertical_m: 16.0,
        },
        approach: PhaseWidths {
            lateral_m: 20.0,
            vertical_m: 16.0,
        },
    };

    /// The widths for `scale`, or `None` for an unknown phase.
    #[must_use]
    pub const fn widths(&self, scale: NavScale) -> Option<PhaseWidths> {
        match scale {
            NavScale::Enroute => Some(self.enroute),
            NavScale::Terminal => Some(self.terminal),
            NavScale::Approach => Some(self.approach),
            NavScale::Unknown => None,
        }
    }

    /// Meters of cross-track deviation per dot for `scale`, or `None`
    /// when the profile has no usable width for it.
    #[must_use]
    pub fn lateral_m_per_dot(&self, scale: NavScale) -> Option<f32> {
        self.widths(scale).and_then(|w| per_dot(w.lateral_m))
    }

    /// Meters of vertical deviation per dot for `scale`, or `None` when
    /// the profile has no usable width for it.
    #[must_use]
    pub fn vertical_m_per_dot(&self, scale: NavScale) -> Option<f32> {
        self.widths(scale).and_then(|w| per_dot(w.vertical_m))
    }
}

/// Meters per dot for a width at the outer dot. A width that is not a
/// positive finite number has none.
fn per_dot(width_m: f32) -> Option<f32> {
    (width_m.is_finite() && width_m > 0.0).then(|| width_m / OUTER_DOT)
}

/// Solution qualities this build can present. Unusable, and any coding
/// a later host introduces, remove the display rather than drawing
/// guidance the display cannot vouch for.
const PRESENTABLE_QUALITIES: [u32; 2] = [0, 1];

/// Converts one accepted guidance snapshot into the instrument model's
/// nav group, or `None` when guidance must not display at all —
/// the ADR-0031 contract that absent guidance is displayed as absent,
/// never as a centered needle.
///
/// The guidance's own phase scale selects both widths from `profile`,
/// and its own TO/FROM resolution sets the flag. An unknown phase, a
/// phase without a usable lateral and vertical width, or an unknown
/// TO/FROM removes the display.
pub fn nav_display_state(
    snapshot: Option<&NavSnapshot>,
    profile: &DeviationScaleProfile,
) -> Option<Stamped<NavData>> {
    let snapshot = snapshot?;
    if !snapshot.age_ms.is_finite() {
        return None;
    }
    let guidance = &snapshot.guidance;
    if !PRESENTABLE_QUALITIES.contains(&guidance.solution_quality) {
        return None;
    }
    let lateral_m_per_dot = profile.lateral_m_per_dot(guidance.scale)?;
    let vertical_m_per_dot = profile.vertical_m_per_dot(guidance.scale)?;
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
                lateral_dots(guidance.lateral_deviation_m, lateral_m_per_dot)
            },
            vdev_dots: vertical_dots(guidance.vertical_deviation_m, vertical_m_per_dot),
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
fn vertical_dots(vertical_deviation_m: f32, m_per_dot: f32) -> Option<f32> {
    vertical_deviation_m
        .is_finite()
        .then(|| vertical_deviation_m / m_per_dot)
}

#[cfg(test)]
mod tests;
