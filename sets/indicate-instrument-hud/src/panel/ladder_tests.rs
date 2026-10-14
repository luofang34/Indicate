//! Pitch ladder, ghost horizon, and recovery ladder.
#![allow(clippy::expect_used, clippy::panic)]
use super::tests::*;
use super::*;
use crate::context::HudProfile;
use indicate_instrument_scene::{Cmd, SceneCmds};
use std::vec::Vec;

#[test]
fn dive_rungs_droop_their_outer_ends_away_from_the_horizon() {
    let mut storage = std::vec![0; 4096];
    let mut writer = SceneWriter::new(&mut storage).expect("buffer");
    let layout = Layout::new(FRAME_LANDSCAPE, HudProfile::Screen);
    let mut rung = |degrees| {
        rung::Rung {
            centre: [0.0, 0.0],
            along: [1.0, 0.0],
            toward: [0.0, -1.0],
            half: 100.0,
            gap: 20.0,
        }
        .draw(degrees, &layout, &mut writer)
    };
    rung(-30).expect("dive rung");
    rung(30).expect("climb rung");
    let size = writer.finish();
    let mut ends = Vec::new();
    for command in SceneCmds::new(&storage[..size]).expect("decode") {
        if let Cmd::Polyline { points } = command.expect("command") {
            for i in 0..points.len() {
                ends.push(points.get(i).expect("point"));
            }
        }
    }
    // The horizon is toward -y. A 30-degree dive rung droops 15 degrees toward +y;
    // its end leg still reaches back toward the horizon.
    let outer_dive = ends
        .iter()
        .filter(|p| p[0] > 60.0 && p[0] < 85.0)
        .map(|p| p[1])
        .fold(0.0_f32, f32::max);
    assert!(
        outer_dive > 10.0,
        "outer end droops away from the horizon: {outer_dive}"
    );
    let climb_ends: Vec<_> = ends.iter().filter(|p| p[0] > 95.0).collect();
    assert!(
        climb_ends.iter().any(|p| p[1].abs() < 0.01),
        "a climb rung stays straight"
    );
}

#[test]
fn a_ghost_horizon_marks_the_field_edge_when_the_horizon_leaves_the_field() {
    let near_edge = |drawn: &Drawn| {
        let layout = Layout::new(FRAME_LANDSCAPE, HudProfile::Screen);
        let ghost = layout.centre[1] + layout.field[1] - 10.0 * layout.unit;
        drawn
            .lines
            .iter()
            .filter(|line| line.len() == 2)
            .filter(|line| line.iter().all(|p| (p[1] - ghost).abs() < 1.0))
            .count()
    };
    // 26 degrees nose up: the horizon is below the field, short of recovery.
    assert!(
        near_edge(&draw(&flight(0.0, 0.45, 0.0), &screen())) >= 6,
        "nose high: ghost at the bottom edge"
    );
    assert_eq!(near_edge(&draw(&flight(0.0, 0.0, 0.0), &screen())), 0);
}

#[test]
fn recovery_compresses_the_ladder_so_the_horizon_stays_in_view() {
    let drawn = draw(&flight(0.0, 1.3, 0.0), &screen());
    assert!(drawn.has("UNUSUAL ATTITUDE") && drawn.has("ATT COMPRESSED"));
    assert!(
        drawn
            .commands
            .iter()
            .any(|c| c.contains("r: 255, g: 0, b: 0")),
        "red chevrons toward the horizon"
    );
    let (cy, hh) = (FRAME_LANDSCAPE.height * 0.5, FRAME_LANDSCAPE.height * 0.36);
    let horizon_inside = drawn.commands.iter().any(|c| c.starts_with("Translate"))
        && recovery::scale(&Layout::new(FRAME_LANDSCAPE, HudProfile::Screen))
            * 1.3_f32.to_degrees()
            < hh;
    assert!(
        horizon_inside,
        "the horizon offset {cy} stays inside the field"
    );
    assert!(!drawn.has("IN RNG"));
    assert!(
        drawn.has("30") && drawn.has("60") && !drawn.has("10") && !drawn.has("20"),
        "{:?}",
        drawn.texts
    );
}
