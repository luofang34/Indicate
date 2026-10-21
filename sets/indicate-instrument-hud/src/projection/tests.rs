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
        body_from_view: indicate_frames::Quat::IDENTITY,
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
        body_from_view: indicate_frames::Quat::IDENTITY,
        source: WorldSource::Synthetic,
    };
    let camera = Camera::new(&data, projection, FRAME);
    assert!(camera.level_basis());
    assert!(!camera.world_basis());
    assert!(camera.project_ned([1000.0, 0.0, 0.0]).is_none());
}

#[test]
fn view_rotation_changes_projection_without_changing_aircraft_attitude() {
    let data = flight(0.0, 0.0, 0.0);
    let body_from_view = Quat::from_euler(0.3, -0.4, 0.6);
    let camera = Camera::new(
        &data,
        HudProjection {
            vertical_fov_rad: 1.0,
            body_from_view,
            source: WorldSource::Video,
        },
        FRAME,
    );
    let optical_axis = body_from_view.rotate([1.0, 0.0, 0.0]).map(|v| v as f32);
    let centre = camera.project_ned(optical_axis).expect("view forward");
    assert!(close(centre.x, 500.0) && close(centre.y, 400.0));
    let nose = camera.project_body([1.0, 0.0, 0.0]).expect("nose in front");
    assert!(nose.x < 500.0 && nose.y < 400.0);
    let behind = optical_axis.map(|v| -v);
    assert!(camera.project_ned(behind).is_none());
    assert_eq!(data.roll_rad.value, 0.0);
    assert_eq!(data.pitch_rad.value, 0.0);
    assert!((camera.view_azimuth_from_heading().expect("azimuth") - 0.6).abs() < 1e-6);
}

#[test]
fn horizon_direction_uses_view_attitude_and_missing_aircraft_data_stays_missing() {
    let mut data = flight(0.0, 0.0, 0.0);
    let projection = HudProjection {
        vertical_fov_rad: 1.0,
        body_from_view: Quat::from_euler(0.0, -1.0, 0.0),
        source: WorldSource::Video,
    };
    let camera = Camera::new(&data, projection, FRAME);
    assert!(camera.horizon_direction().expect("horizon above")[1] < -0.99);
    data.pitch_rad.status = SignalStatus::Missing;
    let camera = Camera::new(&data, projection, FRAME);
    assert!(!camera.level_basis());
    assert!(camera.project_ned([1.0, 0.0, 0.0]).is_none());
    assert!(camera.horizon_direction().is_none());
}
