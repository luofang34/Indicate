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
        far_down_offset_m: 0.0,
        pre_threshold_m: 0.0,
        reciprocal_displaced_m: 0.0,
        unpaved: false,
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
            facility: indicate_instrument_state::NavFacility::Localizer,
            sequencing: indicate_instrument_state::NavSequencing::Sequencing,
            position_source: indicate_instrument_state::PositionSource::None,
            position_integrity: indicate_instrument_state::PositionIntegrity::Normal,
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

#[test]
fn rollout_draws_centreline_dashes_suppresses_gp_and_prevents_ceiling_inversion() {
    let cx = FRAME_LANDSCAPE.width * 0.5;
    // Simulate landed rollout 250 m past threshold where WAAS vertical error (-2.5 m) places
    // raw threshold_m[2] = -1.1 m (1.1 m above estimated eye) on a 7 m downhill-sloped runway.
    let runway = Runway {
        threshold_m: [-250.0, 0.0, -1.1],
        far_down_offset_m: 7.0,
        pre_threshold_m: 0.0,
        reciprocal_displaced_m: 0.0,
        unpaved: false,
        course_rad: 0.0,
        length_m: 2000.0,
        width_m: 45.0,
        glide_path_rad: GLIDE,
        status: SignalStatus::Valid,
    };
    let data = with_ils(0.0, 0.0);
    let camera = Camera::new(&data, synthetic().value, FRAME_LANDSCAPE);
    let horizon_y = camera.project_ned([1.0, 0.0, 0.0]).expect("horizon").y;
    let drawn = draw(&data, &approach(runway));
    assert!(!drawn.has("GP"), "airborne GP bar declutters on rollout");
    let centreline_dashes = drawn
        .lines
        .iter()
        .filter(|pts| {
            pts.len() == 2
                && (pts[0][0] - cx).abs() < 0.5
                && (pts[1][0] - cx).abs() < 0.5
                && pts[0][1] > horizon_y
                && pts[1][1] > horizon_y
        })
        .count();
    assert!(
        centreline_dashes >= 20,
        "expected conformal rollout centreline dashes below horizon, got {centreline_dashes}"
    );
    // Diagonal runway sidelines converge near the horizon and fan outward downward (> horizon_y),
    // never upward into the sky (< horizon_y).
    let inverted_sideline = drawn.lines.iter().any(|pts| {
        pts.len() == 2
            && (pts[0][0] - pts[1][0]).abs() > 80.0
            && (pts[0][1] - pts[1][1]).abs() > 10.0
            && pts[0][1].min(pts[1][1]) < horizon_y - 2.0
    });
    assert!(
        !inverted_sideline,
        "rollout sidelines must stay below horizon instead of inverting into the sky"
    );
}

#[test]
fn unpaved_and_displaced_threshold_runways_distinguish_surface_and_zones() {
    let cx = FRAME_LANDSCAPE.width * 0.5;
    let data = with_ils(0.0, 0.0);
    let mut turf = final_approach(600.0);
    turf.unpaved = true;
    turf.width_m = 9.0;
    let drawn_turf = draw(&data, &approach(turf));
    let closed_box_turf = drawn_turf
        .lines
        .iter()
        .filter(|l| l.len() == 5 && l.first() == l.last())
        .filter(|l| (l[0][0] - l[2][0]).abs() > 0.5)
        .count();
    assert_eq!(
        closed_box_turf, 0,
        "unpaved turf strip uses dashed sidelines"
    );
    let camera = Camera::new(&data, synthetic().value, FRAME_LANDSCAPE);
    let horizon_y = camera.project_ned([1.0, 0.0, 0.0]).expect("horizon").y;
    let thr_y = camera.project_ned(turf.threshold_m).expect("thr").y;
    let count_interior = |d: &Drawn| {
        d.lines
            .iter()
            .filter(|pts| {
                pts.len() == 2
                    && (pts[0][0] - cx).abs() < 0.5
                    && (pts[1][0] - cx).abs() < 0.5
                    && pts[0][1].min(pts[1][1]) > horizon_y + 1.0
                    && pts[0][1].max(pts[1][1]) < thr_y - 0.5
            })
            .count()
    };
    assert!(count_interior(&draw(&data, &approach(final_approach(600.0)))) >= 10);
    assert_eq!(
        count_interior(&drawn_turf),
        0,
        "unpaved strip suppresses painted interior centreline stripes"
    );

    let mut composite = final_approach(600.0);
    composite.pre_threshold_m = 165.8;
    composite.reciprocal_displaced_m = 402.0;
    let drawn_comp = draw(&data, &approach(composite));
    let closed_box_comp = drawn_comp
        .lines
        .iter()
        .filter(|l| l.len() == 5 && l.first() == l.last())
        .filter(|l| (l[0][0] - l[2][0]).abs() > 0.5)
        .count();
    assert_eq!(
        closed_box_comp, 2,
        "between-thresholds box stays solid (halo + stroke)"
    );
    assert!(
        drawn_comp.lines.len() > draw(&data, &approach(final_approach(600.0))).lines.len(),
        "displaced ends add dashed pre-threshold and reciprocal sidelines"
    );
}

#[test]
fn the_receiver_labels_follow_the_facility() {
    for (source, facility, lateral, vertical) in [
        (
            NavSource::Nav1,
            indicate_instrument_state::NavFacility::Localizer,
            "LOC",
            "GS",
        ),
        (
            NavSource::Nav1,
            indicate_instrument_state::NavFacility::Vor,
            "VOR",
            "VDEV",
        ),
        (
            NavSource::Gps,
            indicate_instrument_state::NavFacility::None,
            "LNAV",
            "VNAV",
        ),
    ] {
        let mut data = with_ils(0.0, 0.0);
        data.nav.data.source = source;
        data.nav.data.facility = facility;
        let drawn = draw(&data, &approach(final_approach(5000.0)));
        assert!(drawn.has(lateral) && drawn.has(vertical), "{facility:?}");
    }
}

/// A flight-management course names the sensor that positions it beside
/// the lateral label, and the words turn to the caution ink for dead
/// reckoning, a degraded position, or an excluded sensor.
#[test]
fn a_flight_management_course_names_its_position_sensor() {
    use indicate_instrument_state::{NavFacility, PositionIntegrity, PositionSource};
    let ink = |drawn: &Drawn, text: &str| -> Option<std::string::String> {
        let at = drawn
            .commands
            .iter()
            .position(|c| c.starts_with("Text") && c.contains(&std::format!("\"{text}\"")))?;
        drawn.commands[..at]
            .iter()
            .rev()
            .find(|c| c.starts_with("FillColor"))
            .cloned()
    };
    let fms = |position, integrity| {
        let mut data = with_ils(0.5, 0.5);
        data.nav.data.source = NavSource::Fms;
        data.nav.data.facility = NavFacility::None;
        data.nav.data.position_source = position;
        data.nav.data.position_integrity = integrity;
        draw(&data, &approach(final_approach(5000.0)))
    };
    let visual = fms(PositionSource::Visual, PositionIntegrity::Normal);
    assert!(visual.has("LNAV") && visual.has("VNAV") && visual.has("VIS"));
    assert_eq!(
        ink(&visual, "VIS"),
        ink(&visual, "LNAV"),
        "one ink while normal"
    );
    let dead_reckoning = fms(PositionSource::DeadReckoning, PositionIntegrity::Normal);
    assert_ne!(ink(&dead_reckoning, "DR"), ink(&dead_reckoning, "LNAV"));
    for (position, integrity, sensor, word) in [
        (
            PositionSource::Visual,
            PositionIntegrity::Degraded,
            "VIS",
            "DGRD",
        ),
        (
            PositionSource::Blended,
            PositionIntegrity::SensorExcluded,
            "BLND",
            "EXCL",
        ),
    ] {
        let drawn = fms(position, integrity);
        assert!(drawn.has("LNAV") && drawn.has(sensor) && drawn.has(word));
        for caution in [sensor, word] {
            assert_ne!(ink(&drawn, caution), ink(&drawn, "LNAV"), "{caution}");
        }
        // The integrity word has its own line under the label row, so
        // the row stays short enough for the gap in a ladder bar.
        assert_eq!(drawn.y(sensor), drawn.y("LNAV"));
        assert!(
            drawn.y(word) >= drawn.y("LNAV") + 10.0,
            "{:?}",
            drawn.placed
        );
    }
    assert!(!visual.has("DGRD") && !visual.has("EXCL"));
    // A receiver names no position sensor.
    let ils = draw(&with_ils(0.5, 0.5), &approach(final_approach(5000.0)));
    assert!(ils.has("LOC") && !ils.has("VIS") && !ils.has("GPS"));
    // A lane that does not name its sensor is not guidance.
    let unnamed = fms(PositionSource::None, PositionIntegrity::Normal);
    assert!(!unnamed.has("LNAV"), "{:?}", unnamed.texts);
}
