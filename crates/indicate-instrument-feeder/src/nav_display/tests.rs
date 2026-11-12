//! The deviation-to-dots profile: fly-to signs, unit conversions, the
//! phase scale and TO/FROM flag the source states, and the
//! absent-not-centered contract.

#![allow(clippy::expect_used, clippy::panic)]

use indicate_instrument_state::{
    IdentStr, NavData, NavFacility, NavFromTo, NavScale, NavSequencing, NavSource,
    PositionIntegrity, PositionSource, validate_state,
};

use super::{DeviationScaleProfile, M_PER_FT, M_PER_NM, OUTER_DOT, PhaseWidths, nav_display_state};
use crate::nav_guidance::{Guidance, NavGuidanceTracker, NavSnapshot};
use crate::stamp::{CLOCK_HOST_MONOTONIC, ROLE_NAVIGATION_SOLUTION, RawStamp};

const GNSS: DeviationScaleProfile = DeviationScaleProfile::GNSS_NAVIGATOR;
const PHASES: [NavScale; 3] = [NavScale::Enroute, NavScale::Terminal, NavScale::Approach];

fn snapshot(guidance: Guidance) -> NavSnapshot {
    NavSnapshot {
        guidance,
        age_ms: 120.0,
    }
}

fn guidance() -> Guidance {
    Guidance {
        to_ident: IdentStr::new("WPT-2").expect("valid"),
        from_ident: IdentStr::new("KMRY").expect("valid"),
        course_rad: 0.6,
        lateral_deviation_m: 926.0,
        vertical_deviation_m: 500.0 * M_PER_FT,
        distance_to_waypoint_m: 1852.0,
        scale: NavScale::Terminal,
        to_from: NavFromTo::To,
        sequencing: NavSequencing::Sequencing,
        position_source: PositionSource::Gps,
        position_integrity: PositionIntegrity::Normal,
        leg_index: 1,
        waypoint_count: 4,
        solution_quality: 0,
    }
}

fn nav(guidance: Guidance, profile: &DeviationScaleProfile) -> Option<NavData> {
    nav_display_state(Some(&snapshot(guidance)), profile).and_then(|stamped| stamped.data)
}

#[test]
fn absent_guidance_is_absent_not_centered() {
    assert!(nav_display_state(None, &GNSS).is_none());
    let mut unusable = guidance();
    unusable.solution_quality = 2;
    assert!(nav(unusable, &GNSS).is_none());
    let mut unknown_quality = guidance();
    unknown_quality.solution_quality = 7;
    assert!(nav(unknown_quality, &GNSS).is_none());
}

#[test]
fn fly_to_signs_and_units_convert_exactly() {
    let stamped = nav_display_state(Some(&snapshot(guidance())), &GNSS).expect("displays");
    let nav = stamped.data.expect("group present");
    // Ownship 926 m (half a nautical mile) RIGHT of course at terminal
    // scale: the course line is LEFT, so the deflection is one dot
    // NEGATIVE and flying toward the bar closes the error.
    assert!((nav.cdi_dots + 1.0).abs() < 1e-6);
    // Ownship 500 ft ABOVE profile at terminal scale: the profile is
    // LOWER on the display, one dot POSITIVE — the axes disagree on
    // which way is up.
    assert!((nav.vdev_dots.expect("constrained") - 1.0).abs() < 1e-5);
    assert!((nav.dist_nm.expect("distance") - 1852.0 / M_PER_NM).abs() < 1e-6);
    assert_eq!(nav.fromto, NavFromTo::To);
    assert_eq!(nav.to_ident.as_str(), "WPT-2");
    assert_eq!(stamped.age_ms, Some(120.0));
}

#[test]
fn the_shipped_profiles_carry_their_stated_widths() {
    let lateral = PHASES.map(|phase| GNSS.lateral_m_per_dot(phase).expect("width"));
    assert_eq!(lateral, [1852.0, 926.0, 0.15 * 1852.0]);
    let vertical = PHASES.map(|phase| GNSS.vertical_m_per_dot(phase).expect("width"));
    assert_eq!(
        vertical,
        [500.0 * M_PER_FT, 500.0 * M_PER_FT, 75.0 * M_PER_FT]
    );
    let unmanned = DeviationScaleProfile::SMALL_UNMANNED;
    assert_eq!(
        PHASES.map(|phase| unmanned.lateral_m_per_dot(phase).expect("width")),
        [100.0, 25.0, 10.0]
    );
    assert_eq!(
        PHASES.map(|phase| unmanned.vertical_m_per_dot(phase).expect("width")),
        [8.0; 3]
    );
}

#[test]
fn each_declared_phase_puts_its_own_full_scale_width_on_the_outer_dot() {
    for profile in [GNSS, DeviationScaleProfile::SMALL_UNMANNED] {
        for phase in PHASES {
            let width = profile.widths(phase).expect("width").lateral_m;
            for (deviation_m, dots) in [
                (width, -OUTER_DOT),
                (-width, OUTER_DOT),
                (width / 2.0, -1.0),
                (0.0, 0.0),
            ] {
                let mut sample = guidance();
                sample.scale = phase;
                sample.lateral_deviation_m = deviation_m;
                let nav = nav(sample, &profile).expect("displays");
                assert!(
                    (nav.cdi_dots - dots).abs() < 1e-5,
                    "{phase:?} {deviation_m} m: {} dots, want {dots}",
                    nav.cdi_dots
                );
                // The annunciated scale is the one the dots are on.
                assert_eq!(nav.scale, phase);
            }
        }
    }
}

#[test]
fn one_distance_reads_differently_at_each_phase() {
    // 0.3 NM right of course: full scale on approach, 0.6 of a dot
    // terminal, and 0.3 of a dot enroute.
    let dots = PHASES.map(|phase| {
        let mut sample = guidance();
        sample.scale = phase;
        sample.lateral_deviation_m = 0.3 * M_PER_NM;
        nav(sample, &GNSS).expect("displays").cdi_dots
    });
    for (got, want) in dots.into_iter().zip([-0.3, -0.6, -2.0]) {
        assert!((got - want).abs() < 1e-5, "{dots:?}");
    }
}

#[test]
fn each_declared_phase_puts_its_own_vertical_width_on_the_outer_dot() {
    for phase in PHASES {
        let width = GNSS.widths(phase).expect("width").vertical_m;
        for (deviation_m, dots) in [(width, OUTER_DOT), (-width / 2.0, -1.0)] {
            let mut sample = guidance();
            sample.scale = phase;
            sample.vertical_deviation_m = deviation_m;
            let vdev = nav(sample, &GNSS).expect("displays").vdev_dots;
            let vdev = vdev.expect("constrained");
            assert!((vdev - dots).abs() < 1e-5, "{phase:?}: {vdev} dots");
        }
    }
}

#[test]
fn a_deviation_past_full_scale_stays_finite_and_unclamped_for_the_panel_to_peg() {
    let mut far = guidance();
    far.scale = NavScale::Approach;
    far.lateral_deviation_m = 10.0 * M_PER_NM;
    let nav = nav(far, &GNSS).expect("displays");
    assert!(nav.cdi_dots.is_finite());
    assert!(nav.cdi_dots < -OUTER_DOT * 10.0);
}

#[test]
fn an_undeclared_phase_is_absent_never_a_default_width() {
    let mut undeclared = guidance();
    undeclared.scale = NavScale::Unknown;
    assert!(nav(undeclared, &GNSS).is_none());
    // Not tracking does not excuse it: the course and distance would
    // still be drawn against a scale nobody named.
    undeclared.lateral_deviation_m = f32::NAN;
    assert!(nav(undeclared, &GNSS).is_none());
}

#[test]
fn a_profile_without_a_usable_width_is_absent() {
    for width in [0.0, -50.0, f32::NAN, f32::INFINITY] {
        let lateral = DeviationScaleProfile {
            terminal: PhaseWidths {
                lateral_m: width,
                ..GNSS.terminal
            },
            ..GNSS
        };
        let vertical = DeviationScaleProfile {
            terminal: PhaseWidths {
                vertical_m: width,
                ..GNSS.terminal
            },
            ..GNSS
        };
        assert_eq!(lateral.lateral_m_per_dot(NavScale::Terminal), None);
        assert_eq!(vertical.vertical_m_per_dot(NavScale::Terminal), None);
        assert!(nav(guidance(), &lateral).is_none(), "{width}");
        assert!(nav(guidance(), &vertical).is_none(), "{width}");
    }
}

#[test]
fn the_stated_flag_is_the_drawn_flag() {
    for flag in [NavFromTo::To, NavFromTo::From] {
        let mut sample = guidance();
        sample.to_from = flag;
        let nav = nav(sample, &GNSS).expect("displays");
        assert_eq!(nav.fromto, flag);
        // FROM does not reverse the needle: the HSI's course arrow
        // turns with the course, so right of course is right of course.
        assert!((nav.cdi_dots + 1.0).abs() < 1e-6);
    }
}

#[test]
fn a_stated_off_flag_removes_the_bar_but_keeps_the_course() {
    let mut off = guidance();
    off.to_from = NavFromTo::Off;
    let nav = nav(off, &GNSS).expect("displays");
    assert_eq!(nav.fromto, NavFromTo::Off);
    assert_eq!(nav.cdi_dots, 0.0);
    assert!((nav.course_rad - 0.6).abs() < 1e-6);
}

#[test]
fn an_unknown_flag_is_absent_never_a_benign_one() {
    let mut unknown = guidance();
    unknown.to_from = NavFromTo::Unknown;
    assert!(nav(unknown, &GNSS).is_none());
    unknown.lateral_deviation_m = f32::NAN;
    assert!(nav(unknown, &GNSS).is_none());
}

#[test]
fn untracked_lateral_clears_the_flag_with_a_finite_zero() {
    for flag in [NavFromTo::To, NavFromTo::From] {
        let mut untracked = guidance();
        untracked.to_from = flag;
        untracked.lateral_deviation_m = f32::NAN;
        let nav = nav(untracked, &GNSS).expect("displays");
        assert_eq!(nav.fromto, NavFromTo::Off);
        assert_eq!(nav.cdi_dots, 0.0);
        // The still-valid course and distance survive.
        assert!((nav.course_rad - 0.6).abs() < 1e-6);
    }
}

#[test]
fn unconstrained_vertical_stays_absent() {
    let mut flat = guidance();
    flat.vertical_deviation_m = f32::NAN;
    assert_eq!(nav(flat, &GNSS).expect("group").vdev_dots, None);
}

fn stamp(sequence: u32) -> RawStamp {
    RawStamp {
        role: ROLE_NAVIGATION_SOLUTION,
        integrity: 2,
        source_id: 11,
        incarnation: [2; 16],
        epoch: 0,
        sequence,
        acquired_at_ns: 0,
        clock: CLOCK_HOST_MONOTONIC,
    }
}

#[test]
fn the_flag_and_scale_flip_in_the_sample_that_flips_them() {
    // A holding pattern: inbound TO the fix, past it FROM, back TO. The
    // source sequences the phase too. Each change reaches the display in
    // the sample that carries it, through the tracker a shell runs.
    let steps = [
        (NavFromTo::To, NavScale::Enroute),
        (NavFromTo::From, NavScale::Terminal),
        (NavFromTo::To, NavScale::Terminal),
        (NavFromTo::To, NavScale::Approach),
    ];
    let mut tracker = NavGuidanceTracker::new();
    for (sequence, (flag, phase)) in (1u32..).zip(steps) {
        let mut sample = guidance();
        sample.to_from = flag;
        sample.scale = phase;
        let now_ms = f64::from(sequence) * 100.0;
        let snapshot = tracker.observe(Some(&(stamp(sequence), sample)), now_ms);
        let nav = nav_display_state(snapshot.as_ref(), &GNSS)
            .and_then(|stamped| stamped.data)
            .expect("displays");
        assert_eq!((nav.fromto, nav.scale), (flag, phase), "step {sequence}");
        let m_per_dot = GNSS.lateral_m_per_dot(phase).expect("width");
        assert!((nav.cdi_dots + 926.0 / m_per_dot).abs() < 1e-5);
    }
}

#[test]
fn the_stated_sequencing_reaches_the_group_and_the_lane_names_no_facility() {
    for state in [
        NavSequencing::Sequencing,
        NavSequencing::Hold,
        NavSequencing::Suspended,
    ] {
        let mut sample = guidance();
        sample.sequencing = state;
        let nav = nav(sample, &GNSS).expect("displays");
        assert_eq!(nav.sequencing, state);
        assert_eq!(nav.facility, NavFacility::None);
    }
    let mut unknown = guidance();
    unknown.sequencing = NavSequencing::Unknown;
    assert!(nav(unknown, &GNSS).is_none(), "an unknown state is absent");
}

/// The lane is flight-management guidance on every position sensor, so
/// the group never claims a GPS receiver the vehicle is not steering on.
#[test]
fn the_lane_is_flight_management_and_carries_the_stated_position() {
    for position in [
        PositionSource::Gps,
        PositionSource::Visual,
        PositionSource::Inertial,
        PositionSource::DeadReckoning,
        PositionSource::Blended,
    ] {
        for integrity in [
            PositionIntegrity::Normal,
            PositionIntegrity::Degraded,
            PositionIntegrity::SensorExcluded,
        ] {
            let mut sample = guidance();
            sample.position_source = position;
            sample.position_integrity = integrity;
            let nav = nav(sample, &GNSS).expect("displays");
            assert_eq!(nav.source, NavSource::Fms);
            assert_eq!(
                (nav.position_source, nav.position_integrity),
                (position, integrity)
            );
            let state = indicate_instrument_state::AircraftState {
                nav: indicate_instrument_state::Stamped {
                    data: Some(nav),
                    age_ms: Some(120.0),
                },
                ..Default::default()
            };
            assert_eq!(
                validate_state(&state).nav,
                None,
                "{position:?} {integrity:?}"
            );
        }
    }
}

#[test]
fn a_lane_without_a_stated_position_or_integrity_is_absent() {
    for position in [PositionSource::None, PositionSource::Unknown] {
        let mut sample = guidance();
        sample.position_source = position;
        assert!(nav(sample, &GNSS).is_none(), "{position:?}");
    }
    let mut sample = guidance();
    sample.position_integrity = PositionIntegrity::Unknown;
    assert!(
        nav(sample, &GNSS).is_none(),
        "an unknown integrity is absent"
    );
}
