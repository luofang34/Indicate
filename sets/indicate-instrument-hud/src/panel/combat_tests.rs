//! Contact, gun, and missile cues.
#![allow(clippy::expect_used, clippy::panic)]
use super::tests::*;
use super::*;
use crate::context::{HudProfile, WeaponMode};
use crate::target::{Contact, TargetPicture};

#[test]
fn the_gun_pipper_sits_below_a_target_ahead_and_reports_range() {
    let contact = hostile([600.0, 0.0, 0.0], [0.0; 3]);
    let (aim, time) =
        weapon::lead_solution(&contact, &crate::WeaponEnvelope::default()).expect("solution");
    assert!(
        aim[2] < 0.0,
        "gravity drop needs an aim above the target: {aim:?}"
    );
    assert!(time > 0.5 && time < 0.7, "{time}");
    let context = HudContext {
        profile: HudProfile::Screen,
        projection: synthetic(),
        weapon: WeaponMode::Gun,
        targets: picture(&[contact], Some(0)),
        ..HudContext::default()
    };
    let drawn = draw(&flight(0.0, 0.0, 0.0), &context);
    assert!(drawn.has("IN RNG"));
    assert!(drawn.has("GUN"));
}

#[test]
fn a_crossing_target_moves_the_lead_aim_ahead_of_it() {
    let contact = hostile([800.0, 0.0, 0.0], [0.0, 150.0, 0.0]);
    let (aim, _) =
        weapon::lead_solution(&contact, &crate::WeaponEnvelope::default()).expect("solution");
    assert!(
        aim[1] > 80.0,
        "lead must be toward the target's motion: {aim:?}"
    );
}

#[test]
fn missile_cues_follow_the_seeker_field_and_the_launch_envelope() {
    let mut context = HudContext {
        profile: HudProfile::Screen,
        projection: synthetic(),
        weapon: WeaponMode::MediumRangeMissile,
        ..HudContext::default()
    };
    context.targets = picture(&[hostile([3000.0, 50.0, 0.0], [0.0; 3])], Some(0));
    assert!(draw(&flight(0.0, 0.0, 0.0), &context).has("SHOOT"));
    context.targets = picture(&[hostile([3000.0, 1500.0, 0.0], [0.0; 3])], Some(0));
    assert!(draw(&flight(0.0, 0.0, 0.0), &context).has("STEER"));
    context.targets = picture(&[hostile([-3000.0, 100.0, 0.0], [0.0; 3])], Some(0));
    assert!(draw(&flight(0.0, 0.0, 0.0), &context).has("NO LOCK"));
    context.targets = picture(&[hostile([150.0, 0.0, 0.0], [0.0; 3])], Some(0));
    assert!(draw(&flight(0.0, 0.0, 0.0), &context).has("MIN RNG"));
    context.targets = picture(&[hostile([20000.0, 0.0, 0.0], [0.0; 3])], Some(0));
    assert!(draw(&flight(0.0, 0.0, 0.0), &context).has("LOCK"));
}

#[test]
fn a_designated_target_behind_gets_a_locator_marked_aft() {
    let context = HudContext {
        profile: HudProfile::Screen,
        projection: synthetic(),
        targets: picture(&[hostile([-2000.0, 300.0, 0.0], [0.0; 3])], Some(0)),
        ..HudContext::default()
    };
    assert!(draw(&flight(0.0, 0.0, 0.0), &context).has("AFT"));
}

#[test]
fn unusual_attitude_removes_weapon_cues() {
    let context = HudContext {
        profile: HudProfile::Screen,
        projection: synthetic(),
        weapon: WeaponMode::MediumRangeMissile,
        targets: picture(&[hostile([3000.0, 0.0, 0.0], [0.0; 3])], Some(0)),
        ..HudContext::default()
    };
    let drawn = draw(&flight(2.8, 0.0, 0.0), &context);
    assert!(drawn.has("UNUSUAL ATTITUDE"));
    assert!(!drawn.has("SHOOT") && !drawn.has("LOCK"));
}

#[test]
fn the_picture_refuses_a_seventeenth_contact_and_a_dangling_designation() {
    let mut picture = TargetPicture::default();
    for _ in 0..crate::MAX_CONTACTS {
        assert!(picture.push(Contact::default()).is_some());
    }
    assert!(picture.push(Contact::default()).is_none());
    picture.designate(Some(crate::MAX_CONTACTS));
    assert!(picture.designated().is_none());
}

#[test]
fn the_gun_pipper_shows_only_within_twice_gun_range() {
    let near = HudContext {
        profile: HudProfile::Screen,
        projection: synthetic(),
        weapon: WeaponMode::Gun,
        targets: picture(&[hostile([900.0, 0.0, 0.0], [0.0; 3])], Some(0)),
        ..HudContext::default()
    };
    let arcs = |drawn: &Drawn| {
        drawn
            .commands
            .iter()
            .filter(|c| c.starts_with("Arc"))
            .count()
    };
    assert_eq!(arcs(&draw(&flight(0.0, 0.0, 0.0), &near)), 1);
    let far = HudContext {
        targets: picture(&[hostile([5000.0, 0.0, 0.0], [0.0; 3])], Some(0)),
        ..near
    };
    assert_eq!(arcs(&draw(&flight(0.0, 0.0, 0.0), &far)), 0);
}

#[test]
fn the_funnel_drops_below_the_gun_line_and_narrows_with_range() {
    let context = HudContext {
        weapon: WeaponMode::Gun,
        body_rates_rps: rates([0.0; 3]),
        ..screen()
    };
    let data = flight(0.0, 0.0, 0.0);
    let camera = Camera::new(&data, context.projection.value, FRAME_LANDSCAPE);
    let points = weapon::funnel::stream(&data, &context, &camera).expect("stream");
    let layout = Layout::new(FRAME_LANDSCAPE, HudProfile::Screen);
    let gun = weapon::gun_point(&context, &camera, &layout);
    assert!(
        gun[1] < layout.centre[1],
        "the gun line sits above the boresight"
    );
    assert!(
        points.iter().all(|(p, _)| p[1] > gun[1]),
        "gravity drop puts every round below the gun line"
    );
    assert!(
        points
            .windows(2)
            .all(|w| w[1].0[1] > w[0].0[1] && w[1].1 > w[0].1),
        "farther rounds drop further"
    );
    let drawn = draw(&data, &context);
    assert!(!drawn.has("FUNNEL OFF"));
}

#[test]
fn pulling_lays_the_stream_below_the_current_nose() {
    let data = flight(0.0, 0.0, 0.0);
    let level = HudContext {
        weapon: WeaponMode::Gun,
        body_rates_rps: rates([0.0; 3]),
        ..screen()
    };
    let pulling = HudContext {
        body_rates_rps: rates([0.0, 0.3, 0.0]),
        ..level
    };
    let camera = Camera::new(&data, level.projection.value, FRAME_LANDSCAPE);
    let straight = weapon::funnel::stream(&data, &level, &camera).expect("stream");
    let turning = weapon::funnel::stream(&data, &pulling, &camera).expect("stream");
    assert!(
        turning[5].0[1] > straight[5].0[1] + 20.0,
        "rounds fired before the pull trail below the nose"
    );
}

#[test]
fn the_funnel_needs_body_rates() {
    let context = HudContext {
        weapon: WeaponMode::Gun,
        ..screen()
    };
    assert!(draw(&flight(0.0, 0.0, 0.0), &context).has("FUNNEL OFF"));
}

#[test]
fn the_infrared_seeker_locks_off_the_nose_inside_its_limit() {
    let mut context = HudContext {
        weapon: WeaponMode::ShortRangeMissile,
        ..screen()
    };
    context.targets = picture(&[hostile([2000.0, 600.0, 0.0], [0.0; 3])], Some(0));
    assert!(draw(&flight(0.0, 0.0, 0.0), &context).has("SHOOT"));
    context.targets = picture(&[hostile([1000.0, 1000.0, 0.0], [0.0; 3])], Some(0));
    assert!(draw(&flight(0.0, 0.0, 0.0), &context).has("NO LOCK"));
    context.targets = picture(&[hostile([5000.0, 0.0, 0.0], [0.0; 3])], Some(0));
    assert!(draw(&flight(0.0, 0.0, 0.0), &context).has("LOCK"));
}

#[test]
fn edge_cues_stay_clear_of_the_simulation_label() {
    // A contact far below and behind the nose points at the bottom edge.
    let context = HudContext {
        targets: picture(&[hostile([-200.0, 0.0, 3000.0], [0.0; 3])], None),
        ..screen()
    };
    let drawn = draw(&flight(0.0, 0.0, 0.0), &context);
    let label = drawn.y("SIM / NOT FOR FLIGHT");
    let layout = Layout::new(FRAME_LANDSCAPE, HudProfile::Screen);
    let lowest = drawn
        .lines
        .iter()
        .filter(|l| l.len() == 3)
        .flat_map(|l| l.iter().map(|p| p[1]))
        .fold(0.0_f32, f32::max);
    assert!(
        lowest > 0.0 && lowest < label - 12.0 * layout.unit,
        "chevron {lowest} vs label {label}"
    );
}
