//! Runway, glide path reference, and receiver deviations.
#![allow(clippy::expect_used, clippy::panic)]
use super::tests::*;
use super::*;
use crate::context::HudProfile;
use crate::runway::Runway;
use indicate_instrument_state::{
    FreshnessPolicy, HeadingReference, IdentStr, NavData, NavFromTo, NavScale, NavSource, Stamped,
    resolve,
};
use std::vec::Vec;

const GLIDE: f32 = 3.0 * core::f32::consts::PI / 180.0;

/// On a 3-degree final, `distance` metres from the threshold, landing north.
fn final_approach(distance: f32) -> Runway {
    Runway {
        threshold_m: [distance, 0.0, distance * libm::tanf(GLIDE)],
        course_rad: 0.0,
        length_m: 2000.0,
        width_m: 45.0,
        glide_path_rad: GLIDE,
        status: SignalStatus::Valid,
    }
}

fn with_ils(cdi: f32, vdev: f32) -> PanelData {
    let mut state = flight_state(0.0, -0.02, 0.0);
    state.nav = Stamped {
        data: Some(NavData {
            source: NavSource::Nav1,
            course_rad: 0.0,
            course_reference: HeadingReference::SimLocalTrue,
            cdi_dots: cdi,
            fromto: NavFromTo::To,
            vdev_dots: Some(vdev),
            dist_nm: None,
            to_ident: IdentStr::EMPTY,
            from_ident: IdentStr::EMPTY,
            scale: NavScale::Approach,
        }),
        age_ms: Some(10.0),
    };
    resolve(&state, &FreshnessPolicy::default())
}

fn approach(runway: Runway) -> HudContext {
    HudContext {
        approach: Some(runway),
        ..screen()
    }
}

#[test]
fn an_approach_draws_the_runway_the_glide_path_line_and_the_mode() {
    let drawn = draw(&with_ils(0.0, 0.0), &approach(final_approach(5000.0)));
    assert!(drawn.has("APP") && drawn.has("GP") && drawn.has("SVS"));
    assert!(drawn.has("LOC") && drawn.has("GS"));
    let outline = drawn
        .lines
        .iter()
        .any(|line| line.len() == 5 && line.first() == line.last());
    assert!(outline, "a closed runway outline");
}

#[test]
fn on_the_glide_path_the_threshold_sits_on_the_reference_line() {
    let data = with_ils(0.0, 0.0);
    let runway = final_approach(5000.0);
    let camera = Camera::new(&data, synthetic().value, FRAME_LANDSCAPE);
    let threshold = camera.project_ned(runway.threshold_m).expect("ahead");
    let along_path = camera
        .project_ned([1.0, 0.0, libm::tanf(GLIDE)])
        .expect("ahead");
    assert!(
        (threshold.y - along_path.y).abs() < 0.5,
        "{threshold:?} {along_path:?}"
    );
}

#[test]
fn deviation_diamonds_move_with_the_receiver() {
    let centred = draw(&with_ils(0.0, 0.0), &approach(final_approach(5000.0)));
    let right_and_low_path = draw(&with_ils(1.0, 1.0), &approach(final_approach(5000.0)));
    // A diamond is the only closed five-point polyline besides the runway outline.
    let diamonds = |drawn: &Drawn| -> Vec<[f32; 2]> {
        drawn
            .lines
            .iter()
            .filter(|l| l.len() == 5 && l.first() == l.last())
            .filter(|l| (l[0][0] - l[2][0]).abs() < 0.01)
            .map(|l| [l[0][0], (l[0][1] + l[2][1]) * 0.5])
            .collect()
    };
    let (a, b) = (diamonds(&centred), diamonds(&right_and_low_path));
    assert_eq!(a.len(), b.len());
    assert!(
        b.iter().zip(&a).any(|(b, a)| b[0] > a[0] + 10.0),
        "the course diamond moves right"
    );
    assert!(
        b.iter().zip(&a).any(|(b, a)| b[1] > a[1] + 10.0),
        "the path diamond moves down"
    );
}

#[test]
fn without_a_receiver_the_deviations_say_so_and_the_geometry_stays() {
    let drawn = draw(&flight(0.0, -0.02, 0.0), &approach(final_approach(5000.0)));
    assert!(drawn.has("APP") && drawn.has("GP"));
    assert!(
        drawn.texts.iter().any(|t| t.starts_with("LOC GS")),
        "{:?}",
        drawn.texts
    );
    assert!(!drawn.has("LOC") && !drawn.has("GS"));
}

#[test]
fn invalid_runway_geometry_draws_no_approach() {
    let mut runway = final_approach(5000.0);
    runway.status = SignalStatus::Stale;
    let drawn = draw(&with_ils(0.0, 0.0), &approach(runway));
    assert!(!drawn.has("APP") && !drawn.has("GP"));
}

#[test]
fn a_combiner_draws_the_approach_in_one_colour() {
    let context = HudContext {
        profile: HudProfile::Combiner,
        ..approach(final_approach(5000.0))
    };
    let drawn = draw(&with_ils(0.0, 0.0), &context);
    assert!(drawn.has("APP") && drawn.has("HUD-SIM") && !drawn.has("SVS"));
}

#[test]
fn the_aoa_bracket_brackets_the_flight_path_at_on_speed() {
    // Level path, nose 8 degrees up: the angle of attack is 8 degrees.
    let data = flight(0.0, 8.0_f32.to_radians(), 0.0);
    let context = HudContext {
        on_speed_aoa_rad: Some([7.4_f32.to_radians(), 8.8_f32.to_radians()]),
        ..approach(final_approach(5000.0))
    };
    let drawn = draw(&data, &context);
    let camera = Camera::new(&data, synthetic().value, FRAME_LANDSCAPE);
    let marker = camera
        .project_ned([1.0, 0.0, 0.0])
        .expect("level flight path ahead");
    let bracket = drawn
        .lines
        .iter()
        .find(|l| {
            l.len() == 4
                && l[0][1] == l[1][1]
                && l[2][1] == l[3][1]
                && l[1][0] == l[2][0]
                && l[0][0] > l[1][0]
                && l[1][1] < l[2][1]
        })
        .expect("an E bracket");
    assert!(
        bracket[1][1] < marker.y && marker.y < bracket[2][1],
        "{bracket:?} {marker:?}"
    );
}
