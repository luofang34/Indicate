//! Engine-system and fuel gauges: dials with more than two sources,
//! temperatures, pressures and fuel by tank.

use std::format;

use indicate_instrument_descriptor::states;
use indicate_instrument_state::{
    BesideSamples, EngineSamples, GroupId, TankSample, TankSamples, withhold_group,
};

use super::{GAUGES, data_of, dial_scene, pixels};
use crate::{
    AnalogDialSpec, Channel, DialChannel, DialInput, DialUnit, Legend, Lighting, Numeral,
    PointerSpec, ScaleLaw, ScalePoint, ScaleRing, SpecError, TickSet, pose, reading,
};

const TYPICAL: BesideSamples<'static> = BesideSamples {
    engines: EngineSamples::Slice(states::TYPICAL_ENGINES),
    tanks: TankSamples::Slice(states::TYPICAL_TANKS),
};

const fn channel(input: DialInput, unit: DialUnit) -> DialChannel {
    DialChannel { input, unit }
}

const fn sub_dial(
    center: [f32; 2],
    law: ScaleLaw<'static>,
    ticks: &'static [TickSet],
    numerals: &'static [Numeral<'static>],
) -> ScaleRing<'static> {
    ScaleRing {
        center,
        radius: 0.42,
        law,
        ticks,
        numerals,
        numeral_radius: 0.6,
        numeral_size: 0.09,
        arcs: &[],
        radials: &[],
        paint: crate::AIRSPEED_KT.rings[0].paint,
    }
}

const fn tens(top: f32) -> TickSet {
    TickSet {
        from: 0.0,
        to: top,
        every: top / 10.0,
        inner: 0.78,
        outer: 0.98,
        width: 0.012,
    }
}

const fn sweep(top: f32) -> &'static [ScalePoint] {
    match top as u32 {
        200 => &[
            ScalePoint {
                value: 0.0,
                angle_deg: 0.0,
            },
            ScalePoint {
                value: 200.0,
                angle_deg: 120.0,
            },
        ],
        25 => &[
            ScalePoint {
                value: 0.0,
                angle_deg: 0.0,
            },
            ScalePoint {
                value: 25.0,
                angle_deg: 120.0,
            },
        ],
        _ => &[
            ScalePoint {
                value: 0.0,
                angle_deg: 0.0,
            },
            ScalePoint {
                value: 100.0,
                angle_deg: 120.0,
            },
        ],
    }
}

const OIL_PSI: ScaleLaw<'static> = ScaleLaw::Piecewise {
    reference_deg: -60.0,
    points: sweep(200.0),
};
const FUEL_PSI: ScaleLaw<'static> = ScaleLaw::Piecewise {
    reference_deg: 120.0,
    points: sweep(25.0),
};
const OIL_C: ScaleLaw<'static> = ScaleLaw::Piecewise {
    reference_deg: 120.0,
    points: sweep(100.0),
};

const fn needle(channel: Channel, law: ScaleLaw<'static>, pivot: [f32; 2]) -> PointerSpec<'static> {
    let mut style = crate::AIRSPEED_KT.pointers[0].style;
    style.length = 0.38;
    style.tail = 0.08;
    style.hub_radius = 0.04;
    PointerSpec {
        channel,
        law,
        pivot,
        style,
        stack_height_mm: 1.2,
        on_failure: crate::FailurePose::Park { angle_deg: 0.0 },
    }
}

/// A three-pointer engine gauge unit: oil pressure on the top sub-dial,
/// fuel pressure lower left, oil temperature lower right, each from its
/// own lane.
const ENGINE_UNIT: AnalogDialSpec<'static> = AnalogDialSpec {
    primary: channel(
        DialInput::OilPressure { engine: 0 },
        DialUnit::PoundsPerSquareInch,
    ),
    secondary: Some(channel(
        DialInput::FuelPressure { engine: 0 },
        DialUnit::PoundsPerSquareInch,
    )),
    extra: &[channel(
        DialInput::OilTemperature { engine: 0 },
        DialUnit::DegreesCelsius,
    )],
    input_range: None,
    rings: &[
        sub_dial(
            [0.0, -0.4],
            OIL_PSI,
            &[tens(200.0)],
            &[
                Numeral {
                    value: 0.0,
                    text: "0",
                },
                Numeral {
                    value: 200.0,
                    text: "200",
                },
            ],
        ),
        sub_dial(
            [-0.42, 0.32],
            FUEL_PSI,
            &[tens(25.0)],
            &[
                Numeral {
                    value: 0.0,
                    text: "0",
                },
                Numeral {
                    value: 25.0,
                    text: "25",
                },
            ],
        ),
        sub_dial(
            [0.42, 0.32],
            OIL_C,
            &[tens(100.0)],
            &[
                Numeral {
                    value: 0.0,
                    text: "0",
                },
                Numeral {
                    value: 100.0,
                    text: "100",
                },
            ],
        ),
    ],
    legends: &[
        Legend {
            text: "OIL",
            at: [0.0, -0.18],
            size: 0.06,
            paint: crate::AIRSPEED_KT.legends[0].paint,
        },
        Legend {
            text: "FUEL",
            at: [-0.42, 0.62],
            size: 0.06,
            paint: crate::AIRSPEED_KT.legends[0].paint,
        },
        Legend {
            text: "°C",
            at: [0.42, 0.62],
            size: 0.06,
            paint: crate::AIRSPEED_KT.legends[0].paint,
        },
    ],
    pointers: &[
        needle(Channel::Primary, OIL_PSI, [0.0, -0.4]),
        needle(Channel::Secondary, FUEL_PSI, [-0.42, 0.32]),
        needle(Channel::Extra(0), OIL_C, [0.42, 0.32]),
    ],
    windows: &[],
    ..crate::AIRSPEED_KT
};

const GALLONS: ScaleLaw<'static> = ScaleLaw::Piecewise {
    reference_deg: -135.0,
    points: &[
        ScalePoint {
            value: 0.0,
            angle_deg: 0.0,
        },
        ScalePoint {
            value: 100.0,
            angle_deg: 270.0,
        },
    ],
};

/// A FRONT tanks gauge: one pointer per wing tank, in US gallons.
const FUEL_FRONT: AnalogDialSpec<'static> = AnalogDialSpec {
    primary: channel(DialInput::FuelQuantity { tank: 0 }, DialUnit::UsGallons),
    secondary: Some(channel(
        DialInput::FuelQuantity { tank: 1 },
        DialUnit::UsGallons,
    )),
    extra: &[],
    input_range: None,
    rings: &[ScaleRing {
        law: GALLONS,
        ticks: &[TickSet {
            from: 0.0,
            to: 100.0,
            every: 10.0,
            inner: 0.82,
            outer: 0.97,
            width: 0.02,
        }],
        numerals: &[
            Numeral {
                value: 0.0,
                text: "0",
            },
            Numeral {
                value: 50.0,
                text: "50",
            },
            Numeral {
                value: 100.0,
                text: "100",
            },
        ],
        ..crate::TACHOMETER.rings[0]
    }],
    legends: &[Legend {
        text: "FRONT",
        at: [0.0, 0.3],
        size: 0.08,
        paint: crate::AIRSPEED_KT.legends[0].paint,
    }],
    pointers: &[
        PointerSpec {
            channel: Channel::Primary,
            law: GALLONS,
            ..crate::TACHOMETER.pointers[0]
        },
        PointerSpec {
            channel: Channel::Secondary,
            law: GALLONS,
            stack_height_mm: 2.0,
            ..crate::TACHOMETER.pointers[0]
        },
    ],
    ..crate::TACHOMETER
};

fn close(a: Option<f32>, b: f32) -> bool {
    a.is_some_and(|a| (a - b).abs() < 0.05)
}

#[test]
fn an_engine_gauge_unit_reads_three_lanes_through_three_channels() {
    assert_eq!(ENGINE_UNIT.check(), Ok(()));
    let r = reading(&ENGINE_UNIT, &data_of(&states::typical()), &TYPICAL);
    // 517 kPa is 75.0 psi; 110 kPa is 15.95 psi; oil at 75 °C.
    assert!(close(r.primary, 74.98), "{r:?}");
    assert!(close(r.secondary, 15.95), "{r:?}");
    assert!(close(r.of(Channel::Extra(0)), 75.0), "{r:?}");
    let p = pose(&ENGINE_UNIT, &r);
    assert!(!p.flag && p.pointers.iter().take(3).all(|ptr| ptr.live));
    let expected = OIL_C.angle_rad(75.0);
    assert!((p.pointers[2].angle_rad - expected).abs() < 1e-5);
}

#[test]
fn a_fahrenheit_dial_reads_a_celsius_lane() {
    let coolant = channel(
        DialInput::CoolantTemperature { engine: 0 },
        DialUnit::DegreesFahrenheit,
    );
    let data = data_of(&states::typical());
    // 105 °C is 221 °F.
    assert!(close(coolant.read(&data, &TYPICAL), 221.0));
}

#[test]
fn a_fuel_gauge_reads_each_tank_by_index_in_its_unit() {
    let r = reading(&FUEL_FRONT, &data_of(&states::typical()), &TYPICAL);
    // 340 L and 331 L in US gallons.
    assert!(
        close(r.primary, 89.82) && close(r.secondary, 87.44),
        "{r:?}"
    );
    let withheld = withhold_group(&states::typical(), GroupId::FuelTanks);
    let p = pose(
        &FUEL_FRONT,
        &reading(&FUEL_FRONT, &data_of(&withheld), &TYPICAL),
    );
    assert!(p.flag && !p.pointers[0].live && !p.pointers[1].live);
}

#[test]
fn a_mass_dial_on_a_volume_tank_shows_nothing_rather_than_assume_a_density() {
    let pounds = channel(DialInput::FuelQuantity { tank: 0 }, DialUnit::Pounds);
    let data = data_of(&states::typical());
    assert_eq!(pounds.read(&data, &TYPICAL), None);
    const BY_MASS: &[TankSample] = &[TankSample {
        quantity_kg: Some(100.0),
        ..TankSample::EMPTY
    }];
    let beside = BesideSamples {
        tanks: TankSamples::Slice(BY_MASS),
        ..TYPICAL
    };
    assert!(close(pounds.read(&data, &beside), 220.46));
}

#[test]
fn extra_channels_are_checked_like_the_others() {
    let wrong_unit = AnalogDialSpec {
        extra: &[channel(
            DialInput::OilTemperature { engine: 0 },
            DialUnit::PoundsPerSquareInch,
        )],
        ..ENGINE_UNIT
    };
    assert_eq!(
        wrong_unit.check(),
        Err(SpecError::UnitMismatch {
            channel: Channel::Extra(0)
        })
    );
    let missing = AnalogDialSpec {
        extra: &[],
        ..ENGINE_UNIT
    };
    assert_eq!(
        missing.check(),
        Err(SpecError::MissingChannel {
            channel: Channel::Extra(0)
        })
    );
    for (name, spec) in GAUGES {
        assert!(spec.extra.is_empty(), "{name}");
    }
}

#[test]
fn the_system_gauges_render() {
    let out = std::env::var("INDICATE_ANALOG_PNG_DIR").ok();
    for (name, spec) in [("engine-unit", &ENGINE_UNIT), ("fuel-front", &FUEL_FRONT)] {
        let r = reading(spec, &data_of(&states::typical()), &TYPICAL);
        let rgba = pixels(
            &dial_scene(spec, &pose(spec, &r), Lighting::Day, 480.0),
            480,
        );
        assert!(rgba.chunks(4).any(|px| px[0] > 200), "{name}");
        if let Some(dir) = &out {
            std::fs::write(
                format!("{dir}/{name}.png"),
                super::png::encode(480, 480, &rgba),
            )
            .expect("writes");
        }
    }
}
