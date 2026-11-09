//! The gyro, heading, clock and synchroscope dials: which way each part
//! moves, what each shows with no value, and their pictures.

use std::format;

use indicate_instrument_descriptor::states;
use indicate_instrument_scene::{Cmd, SceneCmds};
use indicate_instrument_state::{
    BesideSamples, EngineSamples, GroupId, TankSamples, withhold_group,
};

use super::{data_of, dial_scene, pixels};
use crate::{
    CLOCK, DIRECTIONAL_GYRO, DialReading, GYRO_HORIZON, Lighting, MAGNETIC_COMPASS, SYNCHROSCOPE,
    TURN_AND_BANK, pose, reading,
};

fn at(primary: f32, secondary: Option<f32>) -> DialReading {
    DialReading {
        primary: Some(primary),
        secondary,
        ..DialReading::default()
    }
}

fn deg(rad: f32) -> f32 {
    rad.to_degrees()
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

#[test]
fn the_horizon_bar_stays_level_with_the_earth() {
    // Right bank: the bar and the index turn counter-clockwise, so the
    // bar's right end rises.
    let p = pose(&GYRO_HORIZON, &at(30.0, Some(0.0)));
    assert!(close(deg(p.cards[0].angle_rad), -30.0));
    assert!(close(deg(p.pointers[0].angle_rad), -30.0));
    // Nose up: the bar slides down the card, toward the face's bottom.
    let p = pose(&GYRO_HORIZON, &at(0.0, Some(10.0)));
    assert!(close(p.cards[0].offset, 0.12));
    let p = pose(&GYRO_HORIZON, &at(0.0, Some(80.0)));
    assert!(close(p.cards[0].offset, 0.42), "held at the stop");
    // No pitch: the bar is left out and the flag shows.
    let p = pose(&GYRO_HORIZON, &at(10.0, None));
    assert!(!p.cards[0].visible && p.flag);
}

#[test]
fn the_turn_needle_and_ball_follow_rate_and_force() {
    // AEI 1944: the tip deflects 5/32 inch for 180 degrees of turn per
    // minute (3 °/s). The needle is 1.22 face units of a 35.5 mm face.
    let tip_mm = 1.22 * 35.5;
    let p = pose(&TURN_AND_BANK, &at(3.0, Some(0.0)));
    let deflection_mm = tip_mm * p.pointers[1].angle_rad.sin();
    assert!(
        (deflection_mm - 25.4 * 5.0 / 32.0).abs() < 0.01,
        "{deflection_mm}"
    );
    // TM 1-413: the pointer coincides with the index in a one-minute
    // turn (6 °/s, to the right here), and the index is centred on that
    // needle line.
    let p = pose(&TURN_AND_BANK, &at(6.0, Some(0.0)));
    let (s, c) = (p.pointers[1].angle_rad.sin(), p.pointers[1].angle_rad.cos());
    let line = [1.2 * s, 0.55 - 1.2 * c];
    let crate::Shape::Polygon(index) = TURN_AND_BANK.marks[1].shape else {
        panic!("the right index is a polygon");
    };
    let centre_x = index.iter().map(|p| p[0]).sum::<f32>() / index.len() as f32;
    assert!((centre_x - line[0]).abs() < 0.01, "{centre_x} vs {line:?}");
    // Force to the right sends the ball left: clockwise from six
    // o'clock about the tube's centre of curvature.
    let p = pose(&TURN_AND_BANK, &at(0.0, Some(1.6)));
    assert!(close(deg(p.pointers[0].angle_rad), 189.267));
    let p = pose(&TURN_AND_BANK, &at(0.0, Some(-9.0)));
    assert!(
        close(deg(p.pointers[0].angle_rad), 162.0),
        "held at the stop"
    );
    // No rate: the needle rests upright and the flag shows.
    let none = DialReading {
        secondary: Some(0.0),
        ..DialReading::default()
    };
    let p = pose(&TURN_AND_BANK, &none);
    assert!(close(p.pointers[1].angle_rad, 0.0) && !p.pointers[1].live && p.flag);
}

#[test]
fn heading_cards_wrap_and_read_the_right_way_round() {
    assert!(close(
        pose(&DIRECTIONAL_GYRO, &at(370.0, None)).tapes[0].value,
        10.0
    ));
    assert!(close(
        pose(&DIRECTIONAL_GYRO, &at(-10.0, None)).tapes[0].value,
        350.0
    ));
    assert_eq!(
        DIRECTIONAL_GYRO.tapes[0].sense,
        crate::TapeSense::RightIncreasing
    );
    assert_eq!(
        MAGNETIC_COMPASS.tapes[0].sense,
        crate::TapeSense::LeftIncreasing
    );
}

/// The x of every heading numeral in a dial scene, by its text.
fn numeral_x(scene: &[u8], text: &str) -> std::vec::Vec<f32> {
    let mut translate = 0.0;
    let mut out = std::vec::Vec::new();
    for cmd in SceneCmds::new(scene).expect("decodes") {
        match cmd.expect("command") {
            Cmd::Translate { x, .. } => translate = x,
            Cmd::Text { x, text: t, .. } if t == text => out.push(translate + x),
            _ => {}
        }
    }
    out
}

#[test]
fn the_gyro_card_rises_to_the_right_and_the_compass_card_to_the_left() {
    // Heading 80: E (90) lies right of the lubber line on the gyro and
    // left of it on the compass. The window is centred on the frame.
    let centre = 240.0;
    let dg = dial_scene(
        &DIRECTIONAL_GYRO,
        &pose(&DIRECTIONAL_GYRO, &at(80.0, None)),
        Lighting::Day,
        480.0,
    );
    assert!(numeral_x(&dg, "E").iter().all(|x| *x > centre));
    let compass = dial_scene(
        &MAGNETIC_COMPASS,
        &pose(&MAGNETIC_COMPASS, &at(80.0, None)),
        Lighting::Day,
        480.0,
    );
    assert!(numeral_x(&compass, "E").iter().all(|x| *x < centre));
    assert!(!numeral_x(&compass, "E").is_empty());
}

#[test]
fn a_card_with_no_heading_prints_no_heading() {
    let blank = dial_scene(
        &DIRECTIONAL_GYRO,
        &pose(&DIRECTIONAL_GYRO, &DialReading::default()),
        Lighting::Day,
        480.0,
    );
    let heading = GroupId::Heading.to_u8();
    assert!(
        !SceneCmds::new(&blank)
            .expect("decodes")
            .any(|c| c == Ok(Cmd::Attribute { group: heading }))
    );
}

#[test]
fn the_clock_hands_and_the_synchroscope_read_the_corpus() {
    let typical = states::typical();
    let beside = BesideSamples {
        engines: EngineSamples::Slice(states::TYPICAL_ENGINES),
        tanks: TankSamples::Slice(states::TYPICAL_TANKS),
    };
    let data = data_of(&typical);
    // 10:30:15: hour hand at 315.125°, minute at 181.5°, second at 90°.
    let p = pose(&CLOCK, &reading(&CLOCK, &data, &beside));
    let turn = |rad: f32| deg(rad).rem_euclid(360.0);
    assert!(close(turn(p.pointers[0].angle_rad), 315.125));
    assert!(close(turn(p.pointers[1].angle_rad), 181.5));
    assert!(close(turn(p.pointers[2].angle_rad), 90.0));
    // The left shaft stands at 40° and the right at 95°: the left lags
    // by 55°, which reads 305°.
    let r = reading(&SYNCHROSCOPE, &data, &beside);
    assert_eq!(r.primary, Some(305.0));
    let stopped = data_of(&withhold_group(&typical, GroupId::Clock));
    let p = pose(&CLOCK, &reading(&CLOCK, &stopped, &beside));
    assert!(p.pointers.iter().take(3).all(|h| !h.visible) && p.flag);
}

/// Each dial at the readings it is pictured at.
#[allow(clippy::type_complexity)]
const PICTURES: [(&str, &crate::AnalogDialSpec<'static>, f32, Option<f32>); 13] = [
    ("gyro-horizon-level", &GYRO_HORIZON, 0.0, Some(0.0)),
    (
        "gyro-horizon-right-30-climb-10",
        &GYRO_HORIZON,
        30.0,
        Some(10.0),
    ),
    (
        "gyro-horizon-left-60-dive-20",
        &GYRO_HORIZON,
        -60.0,
        Some(-20.0),
    ),
    ("directional-gyro-047", &DIRECTIONAL_GYRO, 47.0, None),
    ("directional-gyro-268", &DIRECTIONAL_GYRO, 268.0, None),
    (
        "turn-and-bank-one-minute-right",
        &TURN_AND_BANK,
        6.0,
        Some(0.0),
    ),
    ("turn-and-bank-left-skid", &TURN_AND_BANK, -2.0, Some(2.5)),
    ("magnetic-compass-000", &MAGNETIC_COMPASS, 0.0, None),
    ("magnetic-compass-080", &MAGNETIC_COMPASS, 80.0, None),
    ("clock-1030-15", &CLOCK, 37_815.0, None),
    ("clock-0148-42", &CLOCK, 6_522.0, None),
    ("synchroscope-305", &SYNCHROSCOPE, 305.0, None),
    ("synchroscope-090", &SYNCHROSCOPE, 90.0, None),
];

#[test]
fn every_new_dial_renders_at_its_pictured_readings() {
    let out = std::env::var("INDICATE_ANALOG_PNG_DIR").ok();
    for (name, spec, primary, secondary) in PICTURES {
        assert_eq!(spec.check(), Ok(()), "{name}");
        let scene = dial_scene(
            spec,
            &pose(spec, &at(primary, secondary)),
            Lighting::Day,
            480.0,
        );
        let rgba = pixels(&scene, 480);
        assert!(rgba.chunks(4).any(|px| px[0] > 200), "{name} painted");
        if let Some(dir) = &out {
            std::fs::write(
                format!("{dir}/{name}.png"),
                super::png::encode(480, 480, &rgba),
            )
            .expect("writes");
        }
    }
}

#[test]
fn the_build_turns_a_tape_drum_by_arc_length() {
    let build = crate::physical_build(&DIRECTIONAL_GYRO).expect("builds");
    let drum = build.tapes[0].expect("the heading drum");
    // 360 degrees of card at its pitch go once round the drum.
    let tape = &DIRECTIONAL_GYRO.tapes[0];
    let circumference = tape.pitch * tape.period * DIRECTIONAL_GYRO.face_radius_mm;
    assert!(close(
        drum.drum_radius_mm * core::f32::consts::TAU,
        circumference
    ));
    let horizon = crate::physical_build(&GYRO_HORIZON).expect("builds");
    let bar = horizon.cards[0].expect("the horizon bar");
    assert_eq!(bar.travel_limit_mm, Some([-0.42 * 35.5, 0.42 * 35.5]));
    assert_eq!(horizon.overlays.len(), 1, "the miniature aircraft");
    // The ball is an octagon of the ball's radius about its centre.
    let ball = crate::physical_build(&TURN_AND_BANK)
        .expect("builds")
        .pointers[0]
        .expect("the ball");
    let r = 0.085 * 35.5;
    for [x, y] in ball.outline_mm.points() {
        assert!(close((x * x + (y + 2.0 * 35.5).powi(2)).sqrt(), r));
    }
}

#[test]
fn a_part_that_cannot_be_placed_is_refused() {
    let tape = crate::TapeSpec {
        period: 0.0,
        ..DIRECTIONAL_GYRO.tapes[0]
    };
    let spec = crate::AnalogDialSpec {
        tapes: core::slice::from_ref(&tape),
        ..DIRECTIONAL_GYRO
    };
    assert_eq!(
        spec.check(),
        Err(crate::SpecError::BadGeometry { what: "tape" })
    );
    let card = crate::CardSpec {
        travel: Some(crate::Travel {
            channel: crate::Channel::Extra(0),
            per_unit: 0.01,
            limit: [-0.1, 0.1],
        }),
        ..GYRO_HORIZON.cards[0]
    };
    let spec = crate::AnalogDialSpec {
        cards: core::slice::from_ref(&card),
        ..GYRO_HORIZON
    };
    assert_eq!(
        spec.check(),
        Err(crate::SpecError::MissingChannel {
            channel: crate::Channel::Extra(0)
        })
    );
    let wedge = [[0.0, 0.0], [0.1, 0.0]];
    let marks = [crate::Mark {
        shape: crate::Shape::Polygon(&wedge),
        paint: GYRO_HORIZON.overlays[0].shapes[0].paint,
    }];
    let spec = crate::AnalogDialSpec {
        marks: &marks,
        ..GYRO_HORIZON
    };
    assert_eq!(
        spec.check(),
        Err(crate::SpecError::BadGeometry { what: "mark" })
    );
}
