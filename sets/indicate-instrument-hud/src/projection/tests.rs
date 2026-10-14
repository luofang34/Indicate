#![allow(clippy::expect_used)]
use super::*;
use crate::context::WorldSource;
use crate::panel::tests::flight;

const FRAME: DesignFrame = DesignFrame {
    width: 1000.0,
    height: 800.0,
};

fn camera(roll: f32, pitch: f32, heading: f32) -> Camera {
    let data = flight(roll, pitch, heading);
    let projection = HudProjection {
        vertical_fov_rad: 1.0,
        source: WorldSource::Synthetic,
    };
    Camera::new(&data, projection, FRAME)
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.5
}

#[test]
fn a_point_on_the_heading_and_level_projects_to_the_boresight() {
    let camera = camera(0.0, 0.0, 0.0);
    let point = camera.project_ned([1000.0, 0.0, 0.0]).expect("ahead");
    assert!(close(point.x, 500.0) && close(point.y, 400.0), "{point:?}");
}

#[test]
fn nose_up_puts_the_horizon_below_the_boresight_by_the_pitch_angle() {
    let camera = camera(0.0, 0.2, 0.0);
    let point = camera.project_ned([1000.0, 0.0, 0.0]).expect("ahead");
    assert!(
        close(point.y, 400.0 + camera.focal() * libm::tanf(0.2)),
        "{point:?}"
    );
    assert!(close(point.x, 500.0));
}

#[test]
fn right_bank_moves_a_point_above_the_nose_to_the_left() {
    let camera = camera(0.5, 0.0, 0.0);
    let above = camera.project_ned([1000.0, 0.0, -200.0]).expect("ahead");
    assert!(above.x < 490.0 && above.y < 400.0, "{above:?}");
}

#[test]
fn heading_rotates_world_points_and_points_behind_do_not_project() {
    let camera = camera(0.0, 0.0, core::f32::consts::FRAC_PI_2);
    let east = camera
        .project_ned([0.0, 1000.0, 0.0])
        .expect("east is ahead");
    assert!(close(east.x, 500.0) && close(east.y, 400.0), "{east:?}");
    assert!(camera.project_ned([0.0, -1000.0, 0.0]).is_none());
    let left = camera.screen_direction(camera.ned_to_body([0.0, -1000.0, 0.0]).expect("basis"));
    assert!(
        left.is_none(),
        "a point straight behind has no screen direction"
    );
    let north = camera.ned_to_body([1000.0, 0.0, 0.0]).expect("basis");
    let direction = camera
        .screen_direction(north)
        .expect("north is to the left");
    assert!(direction[0] < -0.99, "{direction:?}");
}

#[test]
fn magnetic_heading_cannot_place_world_vectors() {
    let mut data = flight(0.0, 0.0, 0.0);
    data.heading.reference = indicate_instrument_state::HeadingReference::Magnetic;
    let projection = HudProjection {
        vertical_fov_rad: 1.0,
        source: WorldSource::Synthetic,
    };
    let camera = Camera::new(&data, projection, FRAME);
    assert!(camera.level_basis());
    assert!(!camera.world_basis());
    assert!(camera.project_ned([1000.0, 0.0, 0.0]).is_none());
}
