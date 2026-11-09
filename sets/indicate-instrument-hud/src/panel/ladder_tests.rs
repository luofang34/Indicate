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

#[test]
fn crosswind_and_sideslip_keep_pitch_rungs_horizontally_centred() {
    let mut state = flight_state(0.0, 0.0, 0.0);
    if let Some(kin) = state.kinematics.data.as_mut() {
        // 15 degrees of track drift to the right at 250 m/s (> 20 kt).
        let drift = 15.0_f32.to_radians();
        kin.vel_ned_mps = [250.0 * libm::cosf(drift), 250.0 * libm::sinf(drift), 0.0];
    }
    let data = indicate_instrument_state::resolve(
        &state,
        &indicate_instrument_state::FreshnessPolicy::default(),
    );
    let mut storage = std::vec![0; 65536];
    let mut writer = SceneWriter::new(&mut storage).expect("scene buffer");
    draw_hud(&data, &screen(), None, FRAME_LANDSCAPE, &mut writer).expect("scene");
    let size = writer.finish();
    let cx = FRAME_LANDSCAPE.width * 0.5;
    let mut ten_xs = Vec::new();
    for cmd in SceneCmds::new(&storage[..size]).expect("decode") {
        if let Cmd::Text { text: "10", x, .. } = cmd.expect("cmd") {
            ten_xs.push(x);
        }
    }
    assert!(
        ten_xs.len() >= 4,
        "left and right +10/-10 labels: {ten_xs:?}"
    );
    let min_x = ten_xs.iter().copied().fold(f32::INFINITY, f32::min);
    let max_x = ten_xs.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let mid_x = (min_x + max_x) * 0.5;
    assert!(
        (mid_x - cx).abs() < 1.0,
        "pitch ladder stays centred on {cx} under 15-deg drift, got {mid_x} ({ten_xs:?})"
    );
}

#[test]
fn runway_sidelines_remain_visible_during_rollout_past_the_threshold() {
    let mut context = screen();
    context.approach = Some(crate::Runway {
        // Aircraft is 300 m down a 2000 m x 45 m north-aligned runway, eye 1.36 m above pavement.
        threshold_m: [-300.0, 0.0, 1.36],
        far_down_offset_m: 0.0,
        pre_threshold_m: 0.0,
        reciprocal_displaced_m: 0.0,
        unpaved: false,
        course_rad: 0.0,
        length_m: 2000.0,
        width_m: 45.0,
        glide_path_rad: 0.052_36,
        status: SignalStatus::Valid,
    });
    let drawn = draw(&flight(0.0, 0.0, 0.0), &context);
    let cx = FRAME_LANDSCAPE.width * 0.5;
    let cy = FRAME_LANDSCAPE.height * 0.5;
    // Left sideline starts left of centre below horizon and converges toward centre near horizon.
    let left_sideline = drawn.lines.iter().any(|pts| {
        pts.len() == 2
            && pts[0][0] < cx - 2.0
            && pts[1][0] < pts[0][0] - 80.0
            && pts[1][1] > cy + 20.0
    });
    let right_sideline = drawn.lines.iter().any(|pts| {
        pts.len() == 2
            && pts[0][0] > cx + 2.0
            && pts[0][1] > cy + 20.0
            && pts[1][0] > cx + 2.0
            && pts[1][0] < pts[0][0] - 80.0
    });
    assert!(
        left_sideline && right_sideline,
        "clipped left and right runway sidelines must draw past threshold: {:?}",
        drawn.lines
    );
}

#[test]
fn overflight_and_grazing_poles_stay_within_raster_coordinate_bounds() {
    let frame = DesignFrame {
        width: 2560.0,
        height: 1600.0,
    };
    let mut context = HudContext {
        profile: HudProfile::Screen,
        projection: Sig::valid(HudProjection {
            vertical_fov_rad: 45.0_f32.to_radians(),
            body_from_view: indicate_frames::Quat::from_euler(0.0, -3.0_f32.to_radians(), 0.0),
            source: WorldSource::Synthetic,
        }),
        approach: Some(crate::Runway {
            threshold_m: [-300.0, 0.0, 450.0],
            far_down_offset_m: 0.0,
            pre_threshold_m: 0.0,
            reciprocal_displaced_m: 0.0,
            unpaved: false,
            course_rad: 0.0,
            length_m: 2926.0,
            width_m: 45.7,
            glide_path_rad: 0.052_36,
            status: SignalStatus::Valid,
        }),
        ..HudContext::default()
    };
    let mut storage = std::vec![0; 65536];
    for half_deg in -180..=180 {
        let pitch_rad = (half_deg as f32 * 0.5).to_radians();
        let data = flight(0.0, pitch_rad, 0.0);
        for fov_deg in [45.0_f32, 24.0] {
            context.projection.value.vertical_fov_rad = fov_deg.to_radians();
            let mut writer = SceneWriter::new(&mut storage).expect("scene buffer");
            draw_hud(&data, &context, None, frame, &mut writer).expect("draw_hud");
            let size = writer.finish();
            for cmd in SceneCmds::new(&storage[..size]).expect("decode") {
                match cmd.expect("cmd") {
                    Cmd::Line { x1, y1, x2, y2 } => {
                        for c in [x1, y1, x2, y2] {
                            assert!(c.abs() <= 20_000.0, "line coord {c} at pitch {half_deg}");
                        }
                    }
                    Cmd::Polyline { points } | Cmd::Polygon { points, .. } => {
                        for [x, y] in points.iter() {
                            assert!(
                                x.abs() <= 20_000.0 && y.abs() <= 20_000.0,
                                "poly coord ({x}, {y}) at pitch {half_deg}"
                            );
                        }
                    }
                    Cmd::Circle { cx, cy, r, .. } => {
                        assert!(
                            cx.abs() + r <= 20_000.0 && cy.abs() + r <= 20_000.0,
                            "circle ({cx}, {cy}, {r}) at pitch {half_deg}"
                        );
                    }
                    _ => {}
                }
            }
        }
    }
}
