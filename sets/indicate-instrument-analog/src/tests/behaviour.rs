//! What the needles, drums, and flag do for a given state. Admission
//! cannot see a needle, so these are the failure-presentation proofs.

use indicate_instrument_descriptor::states;
use indicate_instrument_scene::{Cmd, SceneCmds};
use indicate_instrument_state::{GroupId, withhold_group};

use super::{data_of, dial_scene, face_scene};
use crate::{
    AIRSPEED_KT, AIRSPEED_MPH, ALTIMETER, Lighting, MANIFOLD_PRESSURE, TACHOMETER, VERTICAL_SPEED,
    baro_cruise, pose, reading,
};

#[test]
fn a_live_needle_reads_the_state_in_its_dial_units() {
    let data = data_of(&states::typical());
    // The corpus flies 53 m/s indicated: 103.0 kt, 118.6 mph.
    let kt = reading(&AIRSPEED_KT, &data)
        .primary
        .expect("airspeed shows");
    let mph = reading(&AIRSPEED_MPH, &data)
        .primary
        .expect("airspeed shows");
    assert!((kt - 103.03).abs() < 0.05, "{kt}");
    assert!((mph / kt - 1.150_78).abs() < 1e-4);
    let p = pose(&AIRSPEED_KT, &reading(&AIRSPEED_KT, &data));
    assert!(p.pointers[0].live && !p.flag);
    assert!((p.pointers[0].angle_rad - AIRSPEED_KT.pointers[0].law.angle_rad(kt)).abs() < 1e-6);
}

#[test]
fn every_withheld_required_group_parks_the_needle_and_raises_the_flag() {
    for (spec, groups) in [
        (&AIRSPEED_KT, &[GroupId::Air, GroupId::Trust][..]),
        (&VERTICAL_SPEED, &[GroupId::Air, GroupId::Trust][..]),
        (
            &ALTIMETER,
            &[GroupId::Altitude, GroupId::Air, GroupId::Trust][..],
        ),
        (&MANIFOLD_PRESSURE, &[GroupId::Engine, GroupId::Trust][..]),
        (&TACHOMETER, &[GroupId::Engine, GroupId::Trust][..]),
    ] {
        let fed = pose(spec, &reading(spec, &data_of(&baro_cruise())));
        assert!(!fed.flag, "fed {:?}", spec.primary.input);
        for group in groups {
            let state = withhold_group(&baro_cruise(), *group);
            let p = pose(spec, &reading(spec, &data_of(&state)));
            assert!(p.flag, "{:?} without {group:?}", spec.primary.input);
            for (pointer, spec_pointer) in p.pointers.iter().zip(spec.pointers) {
                if spec_pointer.channel == crate::Channel::Primary {
                    assert!(!pointer.live, "{:?} without {group:?}", spec.primary.input);
                }
            }
        }
    }
}

#[test]
fn an_altimeter_shows_no_height_that_is_not_barometric() {
    // The corpus declares local height above a session origin; an
    // altimeter has no such reading, so it flags rather than show it.
    let data = data_of(&states::typical());
    let p = pose(&ALTIMETER, &reading(&ALTIMETER, &data));
    assert!(p.flag);
    assert!(p.pointers.iter().take(3).all(|ptr| !ptr.live));
    assert!(p.windows[0].live, "the Kollsman setting is still applied");
}

#[test]
fn a_drum_without_its_value_draws_no_digits() {
    let state = withhold_group(&baro_cruise(), GroupId::Air);
    let p = pose(&ALTIMETER, &reading(&ALTIMETER, &data_of(&state)));
    assert!(!p.windows[0].live);
    let scene = dial_scene(&ALTIMETER, &p, Lighting::Day, 240.0);
    let air = GroupId::Air.to_u8();
    let claims = SceneCmds::new(&scene)
        .expect("decodes")
        .filter(|cmd| matches!(cmd, Ok(Cmd::Attribute { group }) if *group == air))
        .count();
    assert_eq!(claims, 0, "no digit may claim the withheld group");
}

#[test]
fn an_engine_the_source_does_not_report_shows_nothing() {
    let mut spec = TACHOMETER;
    spec.primary.input = crate::DialInput::EngineRpm { engine: 2 };
    let data = data_of(&states::typical());
    assert_eq!(
        reading(&spec, &data).primary,
        None,
        "the corpus has two engines"
    );
    let second = crate::DialInput::EngineRpm { engine: 1 }.native(&data);
    assert_eq!(second, Some(2320.0));
}

#[test]
fn the_face_scene_holds_nothing_that_moves_and_claims_no_state() {
    for (name, spec) in super::GAUGES {
        let scene = face_scene(spec, Lighting::Day, 240.0);
        for cmd in SceneCmds::new(&scene).expect("decodes") {
            match cmd.expect("well formed") {
                Cmd::Rotate { .. } | Cmd::Polygon { .. } | Cmd::ClipRect { .. } => {
                    panic!("{name}: the face carries a moving part's command")
                }
                Cmd::Attribute { group } => assert_eq!(
                    group,
                    indicate_instrument_scene::ATTR_CONFIG,
                    "{name}: a face numeral claims state"
                ),
                _ => {}
            }
        }
    }
}
