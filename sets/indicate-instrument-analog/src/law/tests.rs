#![allow(clippy::expect_used, clippy::panic)]

use super::wheel_positions;
use crate::spec::{AnalogDialSpec, ScaleLaw};
use crate::{AIRSPEED_KT, AIRSPEED_MPH, ALTIMETER, MANIFOLD_PRESSURE, TACHOMETER, VERTICAL_SPEED};

const BUILT_IN: [&AnalogDialSpec<'static>; 6] = [
    &AIRSPEED_KT,
    &AIRSPEED_MPH,
    &ALTIMETER,
    &VERTICAL_SPEED,
    &MANIFOLD_PRESSURE,
    &TACHOMETER,
];

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

#[test]
fn every_piecewise_law_lands_on_its_scale_points() {
    let mut checked = 0;
    for spec in BUILT_IN {
        let laws = spec
            .rings
            .iter()
            .map(|r| r.law)
            .chain(spec.pointers.iter().map(|p| p.law));
        for law in laws {
            if let ScaleLaw::Piecewise {
                reference_deg,
                points,
            } = law
            {
                for point in points {
                    let got = law.angle_deg(point.value);
                    assert!(
                        close(got, reference_deg + point.angle_deg),
                        "value {} at {got}, engraved at {}",
                        point.value,
                        reference_deg + point.angle_deg
                    );
                    checked += 1;
                }
            }
        }
    }
    assert!(checked > 30, "the built-in scales were exercised");
}

#[test]
fn a_piecewise_law_interpolates_between_points_and_holds_at_its_stops() {
    let law = AIRSPEED_KT.pointers[0].law;
    // 50 kt is half way between 40 kt (30°) and 60 kt (75°).
    assert!(close(law.angle_deg(50.0), 52.5));
    assert!(
        close(law.angle_deg(-20.0), 0.0),
        "below zero rests on the stop"
    );
    assert!(
        close(law.angle_deg(250.0), 345.0),
        "past 200 kt rests on the stop"
    );
    assert!(close(law.angle_deg(f32::NAN), 0.0));
    // The vertical speed scale counts from nine o'clock.
    let vsi = VERTICAL_SPEED.pointers[0].law;
    assert!(close(vsi.angle_deg(0.0), -90.0));
    assert!(
        close(vsi.angle_deg(1000.0), 0.0),
        "1,000 ft/min points at twelve"
    );
}

#[test]
fn the_altimeter_hands_turn_at_their_own_rates() {
    let [ten_k, thousands, hundreds] = [0, 1, 2].map(|k| ALTIMETER.pointers[k].law);
    // 5,280 ft: hundreds hand at 280 of 1,000, thousands at 5.28 of 10,
    // ten-thousands at 0.528 of 10.
    assert!(close(hundreds.angle_deg(5280.0) % 360.0, 100.8));
    assert!(close(thousands.angle_deg(5280.0), 190.08));
    assert!(close(ten_k.angle_deg(5280.0), 19.008));
    assert!(close(
        hundreds.angle_rad(250.0),
        core::f32::consts::FRAC_PI_2
    ));
}

#[test]
fn drum_wheels_stand_on_digits_and_carry_like_an_odometer() {
    let at = |shown: f32| wheel_positions::<4>(shown, 4);
    assert_eq!(at(2992.0), [2.0, 9.0, 9.0, 2.0]);
    let half = at(2992.5);
    assert!(close(half[0], 2.5) && close(half[1], 9.0), "{half:?}");
    // Rolling 2999 -> 3000: every wheel half way.
    let carry = at(2999.5);
    for (got, want) in carry.iter().zip([9.5, 9.5, 9.5, 2.5]) {
        assert!(close(*got, want), "{carry:?}");
    }
    assert_eq!(at(-1.0), [0.0; 4]);
    assert_eq!(wheel_positions::<4>(1234.0, 2), [4.0, 3.0, 0.0, 0.0]);
}
