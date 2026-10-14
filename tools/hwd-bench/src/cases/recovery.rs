//! Forward-aligned attitudes exercise recovery before extreme-pitch chevrons appear.
use super::*;

pub(super) fn append(cases: &mut Vec<Case>) {
    for (name, roll, pitch) in [
        ("52 Forward nose high", 0.0_f32, 35.0_f32),
        ("53 Forward nose low", 0.0, -25.0),
        ("54 Forward high bank", 70.0, 0.0),
        ("55 Forward inverted", 180.0, 0.0),
    ] {
        let mut data = nominal();
        data.roll_rad = Sig::valid(roll.to_radians());
        data.pitch_rad = Sig::valid(pitch.to_radians());
        data.presentation = indicate_instrument_state::UnusualAttitudeState::default().step(
            indicate_frames::Quat::from_euler(roll.to_radians(), pitch.to_radians(), 0.0),
            &indicate_instrument_state::AirframeDisplayProfile::simulator(),
        );
        cases.push(Case {
            name,
            note: "Head and aircraft forward directions coincide. Recovery retains an explicit attitude warning and measured aircraft orientation.",
            data,
            head: [90.0, pitch, 0.0],
            background: "terrain",
            alerts: None,
        });
    }
}
