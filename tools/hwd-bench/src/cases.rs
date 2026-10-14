//! Source-qualified conditions for repeatable HWD review.
use indicate_instrument_state::{
    AircraftState, AltitudeClass, FreshnessPolicy, HeadingReference, PanelData, Sig, SignalStatus,
    resolve,
};

pub(super) struct Case {
    pub name: &'static str,
    pub note: &'static str,
    pub data: PanelData,
    pub head: [f32; 3],
    pub background: &'static str,
    pub alerts: Option<indicate_alerts::AlertOutput>,
}

pub(super) mod flight;
mod guidance;
mod recovery;

fn nominal() -> PanelData {
    let mut data = resolve(&AircraftState::default(), &FreshnessPolicy::default());
    data.ias_kt = Sig::valid(125.0);
    data.gs_kt = Sig::valid(138.0);
    data.vsi_fpm = Sig::valid(-650.0);
    data.ias_trend_kt_s = Sig::valid(-0.8);
    data.altitude.value_ft = Sig::valid(4500.0);
    data.altitude.class = AltitudeClass::BaroIndicated;
    data.baro_hpa = Sig::valid(1018.0);
    data.roll_rad = Sig::valid(0.0);
    data.pitch_rad = Sig::valid(0.0);
    data.heading.value_rad = Sig::valid(90_f32.to_radians());
    data.heading.reference = HeadingReference::True;
    data.track_rad = Sig::valid(94_f32.to_radians());
    data
}

pub(super) fn all() -> Vec<Case> {
    let mut out = Vec::new();
    let mut add = |name, note, change: fn(&mut PanelData), head, background| {
        let mut data = nominal();
        change(&mut data);
        if data.roll_rad.status == SignalStatus::Valid
            && data.pitch_rad.status == SignalStatus::Valid
        {
            let quat =
                indicate_frames::Quat::from_euler(data.roll_rad.value, data.pitch_rad.value, 0.0);
            data.presentation = indicate_instrument_state::UnusualAttitudeState::default().step(
                quat,
                &indicate_instrument_state::AirframeDisplayProfile::simulator(),
            );
        }
        out.push(Case {
            name,
            note,
            data,
            head,
            background,
            alerts: None,
        });
    };
    flight_cases(&mut add);
    head_motion_cases(&mut add);
    attitude_cases(&mut add);
    availability_cases(&mut add);
    vertical_cases(&mut add);
    contrast_cases(&mut add);
    guidance::all(&mut add);
    flight::append(&mut out);
    recovery::append(&mut out);
    out
}

type AddCase<'a> =
    dyn FnMut(&'static str, &'static str, fn(&mut PanelData), [f32; 3], &'static str) + 'a;

fn flight_cases(add: &mut AddCase<'_>) {
    add(
        "01 Approach",
        "IAS and altitude scales; six-second supplied trends; ground-relative flight path.",
        |_| {},
        [90.0, 0.0, 0.0],
        "terrain",
    );
    add(
        "02 Climb",
        "Climb rate remains signed. The velocity marker follows the actual three-dimensional velocity.",
        |d| {
            d.vsi_fpm = Sig::valid(1800.0);
            d.ias_trend_kt_s = Sig::valid(1.2);
            d.pitch_rad = Sig::valid(0.15);
        },
        [90.0, 8.0, 0.0],
        "terrain",
    );
    add(
        "03 Level",
        "The vertical-speed pointer rests at zero. No artificial acceleration is generated.",
        |d| {
            d.vsi_fpm = Sig::valid(0.0);
            d.ias_trend_kt_s = Sig::missing();
        },
        [90.0, 0.0, 0.0],
        "terrain",
    );
    add(
        "04 Crosswind",
        "Track and heading differ. The open track bug and closed heading bug retain separate earth bearings.",
        |d| {
            d.track_rad = Sig::valid(110_f32.to_radians());
        },
        [90.0, 0.0, 0.0],
        "terrain",
    );
    add(
        "05 North crossing",
        "Rounded 359.8 degrees displays zero. Bearing geometry wraps continuously through north.",
        |d| {
            d.heading.value_rad = Sig::valid(359.8_f32.to_radians());
            d.track_rad = Sig::valid(0.01);
        },
        [0.0, 0.0, 0.0],
        "terrain",
    );
}

fn head_motion_cases(add: &mut AddCase<'_>) {
    add(
        "06 Look left",
        "An integrated off-axis attitude instrument appears. Its aircraft symbol never represents head attitude.",
        |_| {},
        [30.0, 0.0, 0.0],
        "terrain",
    );
    add(
        "07 Look right",
        "Earth compass, flight path, and terrain rotate through the view together.",
        |_| {},
        [150.0, 0.0, 0.0],
        "terrain",
    );
    add(
        "08 Look up",
        "The off-axis attitude instrument retains measured aircraft pitch while the head points upward.",
        |_| {},
        [90.0, 48.0, 0.0],
        "terrain",
    );
    add(
        "09 Look down",
        "The altitude and IAS remain available when looking below the aircraft nose.",
        |_| {},
        [90.0, -48.0, 0.0],
        "terrain",
    );
    add(
        "10 Head roll",
        "Head roll changes projection, not any measurement or earth-relative direction.",
        |_| {},
        [90.0, 0.0, 30.0],
        "terrain",
    );
    add(
        "11 Look aft",
        "The crossed retrograde ring is opposite the ground-relative velocity vector.",
        |_| {},
        [274.0, 3.0, 0.0],
        "terrain",
    );
}

fn attitude_cases(add: &mut AddCase<'_>) {
    add(
        "12 Banked",
        "Right bank rotates the horizon left. Ground hatching identifies the lower hemisphere.",
        |d| {
            d.roll_rad = Sig::valid(45_f32.to_radians());
        },
        [150.0, 0.0, 0.0],
        "terrain",
    );
    add(
        "13 Inverted",
        "The compact aircraft-attitude display remains visible during inversion; ground is above the symbol.",
        |d| {
            d.roll_rad = Sig::valid(core::f32::consts::PI);
        },
        [90.0, 0.0, 0.0],
        "terrain",
    );
    add(
        "14 Vertical climb",
        "Bounded attitude geometry retains pitch context near the attitude singularity.",
        |d| {
            d.pitch_rad = Sig::valid(89_f32.to_radians());
            d.vsi_fpm = Sig::valid(8000.0);
        },
        [90.0, 30.0, 0.0],
        "terrain",
    );
    add(
        "15 Vertical descent",
        "Ground hatching fills the compact sphere. No extrapolated flight-director commands are shown.",
        |d| {
            d.pitch_rad = Sig::valid(-89_f32.to_radians());
            d.vsi_fpm = Sig::valid(-8000.0);
        },
        [90.0, -30.0, 0.0],
        "terrain",
    );
}

fn availability_cases(add: &mut AddCase<'_>) {
    add(
        "16 ADS-B replay",
        "Groundspeed and geometric MSL are available. IAS, heading, and attitude remain explicitly missing.",
        |d| {
            d.ias_kt = Sig::missing();
            d.baro_hpa = Sig::missing();
            d.roll_rad = Sig::missing();
            d.pitch_rad = Sig::missing();
            d.heading.value_rad = Sig::missing();
            d.altitude.class = AltitudeClass::GeometricMsl;
        },
        [94.0, 0.0, 0.0],
        "terrain",
    );
    add(
        "17 ADS-B off-axis",
        "Looking away does not conceal missing IAS behind a substituted groundspeed value.",
        |d| {
            d.ias_kt = Sig::missing();
            d.roll_rad = Sig::missing();
            d.pitch_rad = Sig::missing();
            d.heading.value_rad = Sig::missing();
        },
        [160.0, 0.0, 0.0],
        "terrain",
    );
    add(
        "18 Stale navigation",
        "Stale numbers are amber and labelled. Frozen conformal cues are suppressed.",
        |d| {
            d.track_rad.status = SignalStatus::Stale;
            d.gs_kt.status = SignalStatus::Stale;
            d.vsi_fpm.status = SignalStatus::Stale;
            d.heading.value_rad.status = SignalStatus::Stale;
            d.roll_rad.status = SignalStatus::Stale;
        },
        [90.0, 0.0, 0.0],
        "terrain",
    );
    add(
        "19 Attitude failure",
        "A failed attitude source cannot leave a plausible horizon or pitch ladder.",
        |d| {
            d.roll_rad.status = SignalStatus::Failed;
            d.pitch_rad.status = SignalStatus::Failed;
        },
        [150.0, 0.0, 0.0],
        "terrain",
    );
    add(
        "20 All missing",
        "No numeric measurement or conformal cue is fabricated.",
        |d| {
            *d = resolve(&AircraftState::default(), &FreshnessPolicy::default());
        },
        [90.0, 0.0, 0.0],
        "night",
    );
}

fn vertical_cases(add: &mut AddCase<'_>) {
    add(
        "21 High descent rate",
        "The VS pointer saturates. Its exact signed readout remains visible.",
        |d| {
            d.vsi_fpm = Sig::valid(-9500.0);
        },
        [90.0, 0.0, 0.0],
        "terrain",
    );
    add(
        "22 Below sea level",
        "Negative datum-qualified altitude remains negative; no terrain or zero clamp changes it.",
        |d| {
            d.altitude.value_ft = Sig::valid(-180.0);
        },
        [90.0, 0.0, 0.0],
        "terrain",
    );
    add(
        "23 Flight level",
        "Pressure altitude carries STD rather than MSL. Selected targets are visually separate.",
        |d| {
            d.altitude.class = AltitudeClass::Pressure;
            d.altitude.value_ft = Sig::valid(35000.0);
            d.flight.ap_targets.altitude_ft = Sig::valid(37000.0);
            d.flight.ap_targets.airspeed_kt = Sig::valid(140.0);
        },
        [90.0, 0.0, 0.0],
        "terrain",
    );
    add(
        "24 Baro disagreement",
        "A mismatched setting remains annunciated beside the altitude source.",
        |d| {
            d.altitude.setting_mismatch = true;
        },
        [90.0, 0.0, 0.0],
        "terrain",
    );
}

fn contrast_cases(add: &mut AddCase<'_>) {
    add(
        "25 Bright terrain",
        "Dark halos preserve symbol edges over a bright background.",
        |_| {},
        [90.0, 0.0, 0.0],
        "snow",
    );
    add(
        "26 Night",
        "The same symbology uses the display-brightness control; data and color meanings stay unchanged.",
        |_| {},
        [90.0, 0.0, 0.0],
        "night",
    );
    add(
        "27 Magnetic heading",
        "Magnetic heading remains labelled M. It cannot position a true-world aircraft boresight without conversion.",
        |d| {
            d.heading.reference = HeadingReference::Magnetic;
        },
        [94.0, 0.0, 0.0],
        "terrain",
    );
    add(
        "28 Missing vertical speed",
        "No three-dimensional flight-path marker is inferred from track and groundspeed alone.",
        |d| {
            d.vsi_fpm = Sig::missing();
        },
        [90.0, 0.0, 0.0],
        "terrain",
    );
}
