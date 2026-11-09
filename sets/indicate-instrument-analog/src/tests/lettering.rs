//! Dials lettered in a bound face: the P-38G airspeed indicator and
//! altimeter in the `us-ms33558` face, the way a 1943 cockpit would bind
//! it, and the rules that keep lettering out of the scene's identity.

use std::format;

use indicate_instrument_conformance::admit;
use indicate_instrument_descriptor::{
    DrawContext, GroupSet, PanelDescriptor, PanelDrawError, RenderSpace, SceneTarget,
    UnderlayContract,
};
use indicate_instrument_glyphs::typeface::{Face, FaceTable, TextStyle, Typeface};
use indicate_instrument_registry::Registry;
use indicate_instrument_scene::{ArcSense, Cmd, FaceSlot, SceneCmds};
use indicate_instrument_state::GroupId;

use super::{dial_scene, face_scene, pixels_with_faces};
use crate::{
    AIRSPEED_MPH, ALTIMETER, AnalogDialSpec, ArcLegend, DialReading, Legend, Lettering, Lighting,
    Paint, physical_build, pose,
};

const MS33558: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/typefaces/us-ms33558/us-ms33558.idtf"
));

/// The lettering of 1940s Kollsman dials (TM 1-413 fig. 60): the
/// MS33558 skeleton narrowed to three quarters at a sixth-of-height pen.
const KOLLSMAN_1940: TextStyle = TextStyle {
    condense: 0.75,
    pen: Some(1.0 / 6.0),
    slant: 0.0,
    alternates: 0,
};

/// Slot 1 letters the P-38G's flight instruments.
const P38G_SLOT: FaceSlot = FaceSlot(1);
const P38G_LETTERING: Lettering = Lettering {
    numerals: P38G_SLOT,
    legends: P38G_SLOT,
};

const LEGEND: Paint = Paint::plain(indicate_instrument_scene::Rgba8::rgb(220, 220, 220));

/// The fighter airspeed indicator with its period legends.
const P38G_AIRSPEED: AnalogDialSpec<'static> = AnalogDialSpec {
    legends: &[
        Legend {
            text: "AIR SPEED",
            at: [0.0, -0.3],
            size: 0.08,
            paint: LEGEND,
        },
        Legend {
            text: "M.P.H.",
            at: [0.0, 0.3],
            size: 0.075,
            paint: LEGEND,
        },
    ],
    lettering: P38G_LETTERING,
    ..AIRSPEED_MPH
};

/// The sensitive altimeter as the fighter pack ranges it, to 50,000 ft,
/// with its period legends.
const P38G_ALTIMETER: AnalogDialSpec<'static> = AnalogDialSpec {
    input_range: Some([-1000.0, 50_000.0]),
    legends: &[
        Legend {
            text: "ALTITUDE",
            at: [0.0, -0.32],
            size: 0.075,
            paint: LEGEND,
        },
        Legend {
            text: "100 FEET",
            at: [0.0, 0.3],
            size: 0.065,
            paint: LEGEND,
        },
    ],
    lettering: P38G_LETTERING,
    ..ALTIMETER
};

fn bound(style: TextStyle) -> [Face<'static>; 2] {
    let face = Typeface::parse(MS33558).expect("the shipped face parses");
    [
        Face::Reference,
        Face::typeface(face, style).expect("a valid style"),
    ]
}

#[test]
fn the_default_lettering_emits_no_face_change() {
    for (name, spec) in super::GAUGES {
        let scene = dial_scene(
            spec,
            &pose(spec, &DialReading::default()),
            Lighting::Day,
            240.0,
        );
        let fonts = SceneCmds::new(&scene)
            .expect("decodes")
            .filter(|c| matches!(c, Ok(Cmd::Font { .. })))
            .count();
        assert_eq!(fonts, 0, "{name}");
    }
}

#[test]
fn a_lettered_dial_names_its_slot_and_the_build_carries_it() {
    let scene = face_scene(&P38G_ALTIMETER, Lighting::Day, 240.0);
    assert!(
        SceneCmds::new(&scene)
            .expect("decodes")
            .any(|c| c == Ok(Cmd::Font { slot: P38G_SLOT }))
    );
    let build = physical_build(&P38G_ALTIMETER).expect("builds");
    assert_eq!(build.lettering, P38G_LETTERING);
    assert_eq!(build.windows[0].expect("Kollsman drum").face, P38G_SLOT);
}

fn draw_p38g_airspeed(
    ctx: &DrawContext<'_>,
    target: SceneTarget<'_, '_>,
) -> Result<(), PanelDrawError> {
    let SceneTarget::Planar2D { frame, writer } = target else {
        return Err(PanelDrawError::WrongTarget);
    };
    let beside = indicate_instrument_state::BesideSamples {
        engines: *ctx.engines(),
        tanks: *ctx.tanks(),
    };
    let reading = crate::reading(&P38G_AIRSPEED, ctx.data(), &beside);
    crate::draw_dial(
        &P38G_AIRSPEED,
        &pose(&P38G_AIRSPEED, &reading),
        Lighting::Day,
        frame,
        writer,
    )
}

#[test]
fn a_lettered_dial_passes_admission() {
    static PANELS: [PanelDescriptor; 1] = [PanelDescriptor {
        id: "p38g-airspeed",
        title: "P-38G airspeed",
        space: RenderSpace::Planar2D {
            frame_min: crate::DIAL_FRAME,
            frame_max: crate::DIAL_FRAME,
            frame_step: (1.0, 1.0),
            aspect_min: 0.95,
            aspect_max: 1.05,
            canonical_frames: &[crate::DIAL_FRAME],
        },
        required_layers: (1 << 2) | (1 << 4),
        required_groups: GroupSet::of(&[GroupId::Air, GroupId::Trust]),
        underlay: UnderlayContract::Opaque,
        config_schema: &[],
        accepted_intents: &[],
        aux_channels: &[],
        group_regions: &[],
        extreme_states: &[],
        raster_baselines: &[],
        draw: draw_p38g_airspeed,
    }];
    let registry = Registry::new(&PANELS).expect("composes");
    let report = admit(&registry).expect("lettering does not change admissibility");
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
}

#[test]
fn an_arc_legend_letters_along_the_face_in_its_slot() {
    const LEFT_RIGHT: AnalogDialSpec<'static> = AnalogDialSpec {
        arc_legends: &[ArcLegend {
            text: "LEFT ENGINE RIGHT",
            radius: 0.5,
            angle_deg: 180.0,
            sense: ArcSense::CounterClockwise,
            size: 0.08,
            paint: LEGEND,
        }],
        legends: &[],
        lettering: P38G_LETTERING,
        ..crate::TACHOMETER
    };
    let scene = face_scene(&LEFT_RIGHT, Lighting::Day, 240.0);
    let arcs: std::vec::Vec<_> = SceneCmds::new(&scene)
        .expect("decodes")
        .filter_map(|c| match c {
            Ok(Cmd::TextArc(run)) => Some(run.text.len()),
            _ => None,
        })
        .collect();
    assert_eq!(arcs, [17]);
    let slots = bound(TextStyle::REGULAR);
    let faces = FaceTable::new(&slots);
    let rgba = pixels_with_faces(&scene, 240, &faces);
    // The legend hangs below the centre: lit legend pixels lie in the
    // lower half, between radius 0.5 and the centre.
    assert!(rgba.chunks(4).any(|px| px[0] > 200));
}

/// Readings the P-38G gauges are pictured at.
const P38G_PICTURES: [(&str, &AnalogDialSpec<'static>, f32, f32); 5] = [
    (
        "p38g-airspeed-ms33558-kollsman1940-250mph",
        &P38G_AIRSPEED,
        250.0,
        29.92,
    ),
    (
        "p38g-airspeed-ms33558-kollsman1940-395mph",
        &P38G_AIRSPEED,
        395.0,
        29.92,
    ),
    (
        "p38g-altimeter-ms33558-kollsman1940-12450ft",
        &P38G_ALTIMETER,
        12_450.0,
        29.92,
    ),
    (
        "p38g-altimeter-ms33558-kollsman1940-31200ft",
        &P38G_ALTIMETER,
        31_200.0,
        30.06,
    ),
    (
        "p38g-altimeter-ms33558-kollsman1940-750ft-drum",
        &P38G_ALTIMETER,
        750.0,
        29.875,
    ),
];

#[test]
fn the_p38g_gauges_render_in_the_ms33558_face() {
    let out = std::env::var("INDICATE_ANALOG_PNG_DIR").ok();
    let side = 480u32;
    for (style_name, style) in [
        ("kollsman1940", KOLLSMAN_1940),
        ("regular", TextStyle::REGULAR),
    ] {
        let slots = bound(style);
        let faces = FaceTable::new(&slots);
        for (name, spec, primary, secondary) in P38G_PICTURES {
            let reading = DialReading {
                primary: Some(primary),
                secondary: Some(secondary),
                ..DialReading::default()
            };
            let scene = dial_scene(spec, &pose(spec, &reading), Lighting::Day, side as f32);
            let rgba = pixels_with_faces(&scene, side, &faces);
            assert!(rgba.chunks(4).any(|px| px[0] > 200), "{name} painted");
            if let Some(dir) = &out {
                let file = name.replace("kollsman1940", style_name);
                std::fs::write(
                    format!("{dir}/{file}.png"),
                    super::png::encode(side, side, &rgba),
                )
                .expect("writes");
            }
        }
    }
}

#[test]
fn a_lettered_heading_card_letters_in_the_numeral_slot() {
    const P38G_GYRO: AnalogDialSpec<'static> = AnalogDialSpec {
        lettering: P38G_LETTERING,
        ..crate::DIRECTIONAL_GYRO
    };
    let reading = DialReading {
        primary: Some(47.0),
        ..DialReading::default()
    };
    let scene = dial_scene(
        &P38G_GYRO,
        &pose(&P38G_GYRO, &reading),
        Lighting::Day,
        240.0,
    );
    let cmds: std::vec::Vec<_> = SceneCmds::new(&scene)
        .expect("decodes")
        .map(|c| c.expect("command"))
        .collect();
    // Inside the card's window clip, the slot is selected before the
    // card's first numeral.
    let numeral = cmds
        .iter()
        .position(|c| matches!(c, Cmd::Text { text: "3", .. }))
        .expect("the card prints 3 at 047");
    let clip = cmds[..numeral]
        .iter()
        .rposition(|c| matches!(c, Cmd::ClipRect { .. }))
        .expect("the window clips the card");
    assert!(
        cmds[clip..numeral].contains(&Cmd::Font { slot: P38G_SLOT }),
        "the card letters in the numeral slot"
    );
    let build = physical_build(&P38G_GYRO).expect("builds");
    assert_eq!(build.tapes[0].expect("the heading drum").face, P38G_SLOT);
}
