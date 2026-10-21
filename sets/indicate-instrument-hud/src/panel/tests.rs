use super::*;
use crate::context::{HudProfile, WeaponMode, WorldSource};
use crate::target::{Contact, Identity, TargetPicture};
use indicate_instrument_conformance::admit;
use indicate_instrument_registry::Registry;
use indicate_instrument_scene::{Cmd, SceneCmds};
use indicate_instrument_state::{
    AirData, AircraftState, AltitudeClass, AltitudeDeclaration, Attitude, EstimateQuality,
    FreshnessPolicy, GeoidModelId, HeadingReference, HeadingSample, Kinematics, Quat, Stamped,
    ValidFlags, resolve,
};
use std::string::String;
use std::vec::Vec;

/// Flight at 250 m/s along the heading, with the given attitude and a true heading.
pub(crate) fn flight(roll: f32, pitch: f32, heading: f32) -> PanelData {
    resolve(
        &flight_state(roll, pitch, heading),
        &FreshnessPolicy::default(),
    )
}

/// The aircraft state behind [`flight`].
pub(crate) fn flight_state(roll: f32, pitch: f32, heading: f32) -> AircraftState {
    let stamp = Some(10.0);
    let speed = 250.0;
    AircraftState {
        quality: EstimateQuality::Good,
        valid: ValidFlags {
            position: true,
            velocity_horizontal: true,
            velocity_vertical: true,
            attitude: true,
            heading: true,
            ..ValidFlags::default()
        },
        kinematics: Stamped {
            data: Some(Kinematics {
                pos_ned_m: [0.0, 0.0, -3000.0],
                vel_ned_mps: [
                    speed * libm::cosf(heading),
                    speed * libm::sinf(heading),
                    0.0,
                ],
            }),
            age_ms: stamp,
        },
        altitude: AltitudeDeclaration {
            reference_class: AltitudeClass::GeometricMsl,
            sample_m: Some(3000.0),
            geoid_model: GeoidModelId(1),
            ..AltitudeDeclaration::default()
        },
        air: Stamped {
            data: Some(AirData {
                ias_mps: Some(220.0),
                ..AirData::default()
            }),
            age_ms: stamp,
        },
        attitude: Stamped {
            data: Some(Attitude {
                quat: Quat::from_euler(roll, pitch, heading),
                rates_rps: [0.0; 3],
            }),
            age_ms: stamp,
        },
        heading: Stamped {
            data: Some(HeadingSample {
                heading_rad: heading,
                reference: HeadingReference::True,
            }),
            age_ms: stamp,
        },
        ..AircraftState::default()
    }
}

pub(super) fn synthetic() -> Sig<HudProjection> {
    Sig::valid(HudProjection {
        vertical_fov_rad: 1.0,
        body_from_view: indicate_frames::Quat::IDENTITY,
        source: WorldSource::Synthetic,
    })
}

pub(super) fn hostile(position: [f32; 3], velocity: [f32; 3]) -> Contact {
    Contact {
        relative_position_m: position,
        relative_velocity_mps: velocity,
        identity: Identity::Hostile,
        domain: crate::target::Domain::Air,
        status: SignalStatus::Valid,
    }
}

pub(super) fn picture(contacts: &[Contact], designated: Option<usize>) -> TargetPicture {
    let mut picture = TargetPicture::default();
    for contact in contacts {
        picture.push(*contact).expect("room");
    }
    picture.designate(designated);
    picture
}

pub(super) struct Drawn {
    pub(super) commands: Vec<String>,
    pub(super) texts: Vec<String>,
    pub(super) placed: Vec<(String, f32)>,
    pub(super) lines: Vec<Vec<[f32; 2]>>,
}

pub(super) fn draw(data: &PanelData, context: &HudContext) -> Drawn {
    let mut storage = std::vec![0; 65536];
    let mut writer = SceneWriter::new(&mut storage).expect("scene buffer");
    draw_hud(data, context, None, FRAME_LANDSCAPE, &mut writer).expect("scene");
    let size = writer.finish();
    let mut drawn = Drawn {
        commands: Vec::new(),
        texts: Vec::new(),
        placed: Vec::new(),
        lines: Vec::new(),
    };
    for command in SceneCmds::new(&storage[..size]).expect("decode") {
        let command = command.expect("valid command");
        if let Cmd::Polyline { points } = command {
            drawn
                .lines
                .push((0..points.len()).filter_map(|i| points.get(i)).collect());
        }
        if let Cmd::Text { text, y, .. } = command {
            drawn.texts.push(text.into());
            drawn.placed.push((text.into(), y));
        }
        drawn.commands.push(std::format!("{command:?}"));
    }
    drawn
}

impl Drawn {
    pub(super) fn has(&self, text: &str) -> bool {
        self.texts.iter().any(|t| t == text)
    }

    pub(super) fn y(&self, text: &str) -> f32 {
        self.placed
            .iter()
            .find(|(t, _)| t == text)
            .map(|(_, y)| *y)
            .unwrap_or_else(|| panic!("{text} not drawn: {:?}", self.texts))
    }
}

pub(super) fn colors(drawn: &Drawn) -> Vec<String> {
    drawn
        .commands
        .iter()
        .filter(|c| c.starts_with("FillColor") || c.starts_with("Stroke"))
        .cloned()
        .collect()
}

pub(super) fn screen() -> HudContext {
    HudContext {
        profile: HudProfile::Screen,
        projection: synthetic(),
        ..HudContext::default()
    }
}

pub(super) fn rates(value: [f32; 3]) -> Sig<[f32; 3]> {
    Sig::valid(value)
}

#[test]
fn both_descriptors_pass_admission_at_every_device_frame() {
    static SETS: [&indicate_instrument_registry::PanelSet; 2] =
        [&HUD_SCREEN_SET, &HUD_COMBINER_SET];
    let registry = Registry::from_sets(&SETS).expect("set composes");
    let report = admit(&registry).expect("admission");
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
}

#[test]
fn a_synthetic_screen_world_is_conformal_and_carries_no_hud_claim() {
    let context = HudContext {
        profile: HudProfile::Screen,
        projection: synthetic(),
        ..HudContext::default()
    };
    let drawn = draw(&flight(0.0, 0.05, 0.0), &context);
    assert!(!drawn.has("HUD-SIM") && drawn.has("SVS"));
    assert!(drawn.has("SIM / NOT FOR FLIGHT"));
    assert!(!drawn.has("NON-CONFORMAL / NOT A HUD"));
    assert!(
        drawn.has("10"),
        "ladder labels are drawn: {:?}",
        drawn.texts
    );
}

#[test]
fn uncalibrated_video_is_a_repeater_without_conformal_or_target_cues() {
    let mut context = HudContext {
        profile: HudProfile::Screen,
        projection: Sig::with_status(
            HudProjection {
                vertical_fov_rad: 1.0,
                body_from_view: indicate_frames::Quat::IDENTITY,
                source: WorldSource::Video,
            },
            SignalStatus::Degraded,
        ),
        weapon: WeaponMode::Gun,
        ..HudContext::default()
    };
    context.targets = picture(&[hostile([800.0, 0.0, 0.0], [0.0; 3])], Some(0));
    let drawn = draw(&flight(0.0, 0.0, 0.0), &context);
    assert!(drawn.has("NON-CONFORMAL / NOT A HUD"));
    assert!(drawn.has("ATT REPEATER"));
    assert!(drawn.has("TGT CUES OFF"));
    assert!(!drawn.has("HUD-SIM"));
    assert!(!drawn.has("IN RNG"));
    assert!(!drawn.commands.iter().any(|c| c.contains("Guidance")));
}

#[test]
fn calibrated_video_is_conformal() {
    let context = HudContext {
        profile: HudProfile::Screen,
        projection: Sig::valid(HudProjection {
            vertical_fov_rad: 1.0,
            body_from_view: indicate_frames::Quat::IDENTITY,
            source: WorldSource::Video,
        }),
        ..HudContext::default()
    };
    assert_eq!(
        crate::hud_mode(&flight(0.0, 0.0, 0.0), &context),
        HudMode::Conformal
    );
}

#[test]
fn missing_flight_data_draws_dashes_and_no_numerals() {
    let data = resolve(&AircraftState::default(), &FreshnessPolicy::default());
    let context = HudContext {
        profile: HudProfile::Screen,
        projection: synthetic(),
        ..HudContext::default()
    };
    let drawn = draw(&data, &context);
    assert!(drawn.has("---"));
    assert!(drawn.has("ATT MISSING"));
    assert!(
        !drawn
            .texts
            .iter()
            .any(|t| t.chars().any(|c| c.is_ascii_digit())),
        "{:?}",
        drawn.texts
    );
}

#[test]
fn a_calibrated_combiner_is_monochrome_without_dark_ink_or_combat_cues() {
    let mut data = flight(0.0, 0.0, 0.0);
    data.ias_kt.status = SignalStatus::Failed;
    let context = HudContext {
        projection: synthetic(),
        weapon: WeaponMode::Gun,
        targets: picture(&[hostile([600.0, 0.0, 0.0], [0.0; 3])], Some(0)),
        ..HudContext::default()
    };
    let drawn = draw(&data, &context);
    assert!(drawn.has("HUD-SIM"));
    assert!(drawn.has("TGT CUES OFF"));
    assert!(drawn.has("FAILED"), "failure stays readable without colour");
    assert!(!drawn.has("IN RNG") && !drawn.has("GUN"));
    assert!(!drawn.commands.iter().any(|c| c.contains("Guidance")));
    let colors = colors(&drawn);
    assert!(colors.len() > 20, "{colors:?}");
    for color in colors {
        assert!(
            color.contains("r: 90, g: 255, b: 130") || color.contains("a: 0 "),
            "{color}"
        );
    }
}

#[test]
fn an_uncalibrated_combiner_is_labelled_not_a_hud() {
    let drawn = draw(&flight(0.0, 0.0, 0.0), &HudContext::default());
    assert!(drawn.has("NON-CONFORMAL / NOT A HUD"));
    assert!(drawn.has("ATT REPEATER"));
}

#[test]
fn a_screen_without_its_camera_says_so_without_a_hud_label() {
    let context = HudContext {
        profile: HudProfile::Screen,
        projection: Sig::with_status(
            HudProjection {
                vertical_fov_rad: 1.0,
                body_from_view: indicate_frames::Quat::IDENTITY,
                source: WorldSource::Synthetic,
            },
            SignalStatus::Missing,
        ),
        ..HudContext::default()
    };
    let drawn = draw(&flight(0.0, 0.0, 0.0), &context);
    assert!(drawn.has("CAMERA INVALID"));
    assert!(!drawn.has("NON-CONFORMAL / NOT A HUD"));
}

#[test]
fn tapes_put_higher_values_above_the_index_and_box_the_exact_value() {
    let context = HudContext {
        profile: HudProfile::Screen,
        projection: synthetic(),
        ..HudContext::default()
    };
    let drawn = draw(&flight(0.0, 0.0, 0.0), &context);
    let centre = FRAME_LANDSCAPE.height * 0.5;
    // IAS 220 m/s is 428 kt; altitude 3000 m is 9843 ft.
    assert!(drawn.has("428") && drawn.has("9843"), "{:?}", drawn.texts);
    assert!(drawn.y("450") < centre && drawn.y("400") > centre);
    assert!(drawn.y("10500") < centre && drawn.y("9500") > centre);
    assert!(drawn.y("10000") < drawn.y("9500"));
}

#[test]
fn a_failed_altitude_stops_its_tape_and_says_so() {
    let mut data = flight(0.0, 0.0, 0.0);
    data.altitude.value_ft.status = SignalStatus::Failed;
    let context = HudContext {
        profile: HudProfile::Screen,
        projection: synthetic(),
        ..HudContext::default()
    };
    let drawn = draw(&data, &context);
    assert!(drawn.has("FAILED"));
    assert!(!drawn.has("10000") && !drawn.has("9843"));
}

#[test]
fn the_vertical_speed_scale_keeps_polarity_and_order() {
    let samples = [
        -9000.0, -6000.0, -1500.0, -1000.0, -10.0, 0.0, 10.0, 999.0, 1000.0, 4000.0, 9000.0,
    ];
    let fractions: Vec<f32> = samples
        .iter()
        .map(|v| tape::vertical_fraction(*v))
        .collect();
    assert!(fractions.windows(2).all(|w| w[0] <= w[1]), "{fractions:?}");
    assert_eq!(tape::vertical_fraction(0.0), 0.0);
    assert!(tape::vertical_fraction(500.0) > 0.0 && tape::vertical_fraction(-500.0) < 0.0);
    assert_eq!(
        tape::vertical_fraction(9000.0),
        tape::vertical_fraction(6000.0)
    );
}

#[test]
fn a_side_view_keeps_flight_readouts_and_does_not_draw_the_nose_at_its_centre() {
    let mut context = HudContext {
        projection: synthetic(),
        ..HudContext::default()
    };
    let data = flight(0.0, 0.0, 0.0);
    let ahead = draw(&data, &context);
    assert!(ahead.lines.iter().any(|points| points.len() == 7));
    context.projection.value.body_from_view =
        Quat::from_euler(0.0, 0.0, core::f32::consts::FRAC_PI_2);
    let side = draw(&data, &context);
    assert!(side.has("428") && side.has("9843"));
    assert!(
        side.has("10"),
        "ladder remains visible toward the side view"
    );
    assert!(
        !side.lines.iter().any(|points| points.len() == 7),
        "nose is outside the field"
    );
    let bank = |drawn: &Drawn| {
        drawn
            .lines
            .iter()
            .filter(|points| points.len() == 3)
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(
        bank(&ahead),
        bank(&side),
        "aircraft bank indication stays independent of view yaw"
    );
}

#[test]
fn invalid_view_rotation_cannot_claim_conformal_registration() {
    let mut context = HudContext {
        projection: synthetic(),
        ..HudContext::default()
    };
    context.projection.value.body_from_view.w = f32::NAN;
    let data = flight(0.0, 0.0, 0.0);
    assert_eq!(hud_mode(&data, &context), HudMode::Repeater);
    let drawn = draw(&data, &context);
    assert!(drawn.has("ATT REPEATER") && drawn.has("428"));
}
