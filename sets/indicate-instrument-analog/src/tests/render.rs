//! Reference-raster renders: the face alone carries no pointer ink, and
//! every gauge at a few readings, written as PNG pictures to the
//! directory `INDICATE_ANALOG_PNG_DIR` names when it is set.

use std::format;
use std::string::String;

use indicate_instrument_scene::Rgba8;

use super::{GAUGES, dial_scene, face_scene, pixels};
use crate::{AIRSPEED_KT, AnalogDialSpec, DialReading, Lighting, Paint, PointerSpec, pose};

const SIDE: u32 = 480;

/// A colour no built-in face uses, so its pixels can only be needle.
const PROBE: Rgba8 = Rgba8::rgb(255, 0, 255);

fn count_probe(rgba: &[u8]) -> usize {
    rgba.chunks(4)
        .filter(|px| px[0] > 200 && px[1] < 60 && px[2] > 200)
        .count()
}

#[test]
fn the_face_rendered_alone_contains_no_pointer_pixels() {
    let mut pointers = [AIRSPEED_KT.pointers[0]];
    pointers[0].style.paint = Paint::plain(PROBE);
    let spec = AnalogDialSpec {
        pointers: &pointers,
        ..AIRSPEED_KT
    };
    let face = pixels(&face_scene(&spec, Lighting::Day, SIDE as f32), SIDE);
    assert_eq!(count_probe(&face), 0, "the face carries needle ink");
    let reading = DialReading {
        primary: Some(120.0),
        secondary: None,
    };
    let full = pixels(
        &dial_scene(&spec, &pose(&spec, &reading), Lighting::Day, SIDE as f32),
        SIDE,
    );
    assert!(
        count_probe(&full) > 500,
        "the needle is drawn in the full dial"
    );
}

#[test]
fn the_face_is_the_full_dial_wherever_no_moving_part_is() {
    // With the needle hidden and no flag, the full dial is the face.
    let mut pointers: [PointerSpec<'static>; 1] = [AIRSPEED_KT.pointers[0]];
    pointers[0].on_failure = crate::FailurePose::Hide;
    let spec = AnalogDialSpec {
        pointers: &pointers,
        off_flag: None,
        ..AIRSPEED_KT
    };
    let hidden = pose(&spec, &DialReading::default());
    let full = pixels(&dial_scene(&spec, &hidden, Lighting::Day, 240.0), 240);
    let face = pixels(&face_scene(&spec, Lighting::Day, 240.0), 240);
    assert_eq!(full, face);
}

/// Readings each gauge is pictured at: primary, secondary, picture tag.
fn readings(name: &str) -> &'static [(Option<f32>, Option<f32>, &'static str)] {
    match name {
        "airspeed-kt" => &[
            (Some(62.0), None, "62kt"),
            (Some(118.0), None, "118kt"),
            (Some(171.0), None, "171kt"),
            (None, None, "off"),
        ],
        "airspeed-mph" => &[
            (Some(150.0), None, "150mph"),
            (Some(395.0), None, "395mph"),
            (None, None, "off"),
        ],
        "altimeter" => &[
            (Some(5280.0), Some(29.92), "5280ft-2992"),
            (Some(13_000.0), Some(30.13), "13000ft-3013"),
            (Some(750.0), Some(29.875), "750ft-drum-rolling"),
            (None, None, "off"),
        ],
        "vertical-speed" => &[
            (Some(-1500.0), None, "down1500"),
            (Some(0.0), None, "level"),
            (Some(800.0), None, "up800"),
            (None, None, "off"),
        ],
        "manifold-pressure" => &[
            (Some(15.0), None, "15inhg"),
            (Some(30.0), None, "30inhg"),
            (Some(46.0), None, "46inhg"),
        ],
        _ => &[
            (Some(800.0), None, "800rpm"),
            (Some(2400.0), None, "2400rpm"),
            (Some(3200.0), None, "3200rpm"),
            (None, None, "off"),
        ],
    }
}

#[test]
fn every_gauge_renders_at_its_pictured_readings() {
    let out = std::env::var("INDICATE_ANALOG_PNG_DIR").ok();
    for (name, spec) in GAUGES {
        let mut pictures = readings(name).to_vec();
        pictures.push((spec_value(spec), None, "night"));
        for (primary, secondary, tag) in pictures {
            let lighting = if tag == "night" {
                Lighting::Night
            } else {
                Lighting::Day
            };
            let reading = DialReading {
                primary,
                secondary: secondary.or(spec.secondary.map(|_| 29.92)),
            };
            let rgba = pixels(
                &dial_scene(spec, &pose(spec, &reading), lighting, SIDE as f32),
                SIDE,
            );
            assert!(
                rgba.chunks(4).any(|px| px.iter().take(3).any(|c| *c > 200)),
                "{name} {tag} painted"
            );
            if let Some(dir) = &out {
                let path: String = format!("{dir}/{name}-{tag}.png");
                std::fs::write(&path, super::png::encode(SIDE, SIDE, &rgba)).expect("writes");
            }
        }
    }
}

/// A mid-scale reading for the night picture.
fn spec_value(spec: &AnalogDialSpec<'_>) -> Option<f32> {
    readings_mid(spec.primary.input)
}

fn readings_mid(input: crate::DialInput) -> Option<f32> {
    Some(match input {
        crate::DialInput::IndicatedAirspeed => 110.0,
        crate::DialInput::IndicatedVerticalSpeed => 500.0,
        crate::DialInput::BaroAltitude => 2350.0,
        crate::DialInput::BaroSetting => 29.92,
        crate::DialInput::ManifoldPressure { .. } => 28.0,
        crate::DialInput::EngineRpm { .. } => 2300.0,
    })
}
