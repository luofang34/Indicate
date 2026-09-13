//! Approach deviations and recovery cases use declared sources and measured attitudes.
use super::*;
use indicate_instrument_state::{NavFromTo, NavScale, NavSource};

fn ils(data: &mut PanelData) {
    data.nav.status = SignalStatus::Valid;
    data.nav.data.source = NavSource::Nav1;
    data.nav.data.course_reference = HeadingReference::True;
    data.nav.data.course_rad = 90_f32.to_radians();
    data.nav.course_rose_rad = Sig::valid(90_f32.to_radians());
    data.nav.data.scale = NavScale::Approach;
    data.nav.data.fromto = NavFromTo::To;
    data.nav.data.cdi_dots = 0.0;
    data.nav.data.vdev_dots = Some(0.0);
}

pub(super) fn all(add: &mut AddCase<'_>) {
    approach(add);
    unavailable(add);
    recovery(add);
}

fn approach(add: &mut AddCase<'_>) {
    add(
        "29 ILS centered",
        "Centered diamonds indicate zero lateral and vertical deviation from NAV1.",
        ils,
        [90.0, 0.0, 0.0],
        "terrain",
    );
    add(
        "30 Right and below",
        "The course is left. The glideslope is above. Both diamonds indicate the path direction.",
        |d| {
            ils(d);
            d.nav.data.cdi_dots = -1.2;
            d.nav.data.vdev_dots = Some(-0.8);
        },
        [90.0, 0.0, 0.0],
        "terrain",
    );
    add(
        "31 Left and above",
        "The course is right. The glideslope is below. Deviation is not a flight-director command.",
        |d| {
            ils(d);
            d.nav.data.cdi_dots = 1.2;
            d.nav.data.vdev_dots = Some(0.8);
        },
        [90.0, 0.0, 0.0],
        "terrain",
    );
    add(
        "32 Deviation off scale",
        "Open half-diamonds retain direction beyond full scale. They do not indicate centered guidance.",
        |d| {
            ils(d);
            d.nav.data.cdi_dots = -4.0;
            d.nav.data.vdev_dots = Some(4.0);
        },
        [90.0, 0.0, 0.0],
        "terrain",
    );
    add(
        "33 GPS vertical path",
        "Magenta identifies GPS. VDEV does not claim an ILS glideslope or an undeclared approach service.",
        |d| {
            ils(d);
            d.nav.data.source = NavSource::Gps;
            d.nav.data.vdev_dots = Some(-0.5);
        },
        [90.0, 0.0, 0.0],
        "terrain",
    );
    add(
        "34 Lateral only",
        "Lateral guidance remains available. No vertical measurement means no vertical pointer.",
        |d| {
            ils(d);
            d.nav.data.vdev_dots = None;
        },
        [90.0, 0.0, 0.0],
        "terrain",
    );
    add(
        "35 Off-axis ILS",
        "Navigation deviations remain readable beside the compact aircraft attitude display.",
        |d| {
            ils(d);
            d.nav.data.cdi_dots = 1.0;
            d.nav.data.vdev_dots = Some(-1.0);
        },
        [145.0, 0.0, 0.0],
        "terrain",
    );
}

fn unavailable(add: &mut AddCase<'_>) {
    add(
        "36 Stale navigation receiver",
        "A stale receiver removes both deviation pointers and shows NAV STALE.",
        |d| {
            ils(d);
            d.nav.status = SignalStatus::Stale;
        },
        [90.0, 0.0, 0.0],
        "terrain",
    );
    add(
        "37 Navigation flag",
        "An unresolved TO or FROM flag removes the needles. Their values cannot appear centered.",
        |d| {
            ils(d);
            d.nav.data.fromto = NavFromTo::Off;
        },
        [90.0, 0.0, 0.0],
        "terrain",
    );
    add(
        "38 Unknown course reference",
        "An unavailable course reference prevents guidance from appearing with a guessed orientation.",
        |d| {
            ils(d);
            d.nav.course_rose_rad = Sig::missing();
        },
        [90.0, 0.0, 0.0],
        "terrain",
    );
}

fn recovery(add: &mut AddCase<'_>) {
    add(
        "39 Extreme nose high",
        "Red chevrons point toward the aircraft horizon. Recovery removes navigation clutter.",
        |d| {
            ils(d);
            d.pitch_rad = Sig::valid(60_f32.to_radians());
        },
        [90.0, 60.0, 0.0],
        "terrain",
    );
    add(
        "40 Extreme nose low",
        "Bank rotates the recovery chevrons with the aircraft attitude. Head orientation cannot reverse them.",
        |d| {
            ils(d);
            d.pitch_rad = Sig::valid(-40_f32.to_radians());
            d.roll_rad = Sig::valid(45_f32.to_radians());
        },
        [90.0, -35.0, 0.0],
        "terrain",
    );
    add(
        "41 Inverted nose high",
        "The recovery chevrons retain the horizon direction while inverted. The aircraft symbol remains fixed.",
        |d| {
            ils(d);
            d.pitch_rad = Sig::valid(55_f32.to_radians());
            d.roll_rad = Sig::valid(180_f32.to_radians());
        },
        [90.0, 55.0, 0.0],
        "terrain",
    );
    add(
        "42 Recovery attitude failed",
        "Failed attitude removes recovery marks. A red ATT FAILED indication replaces plausible guidance.",
        |d| {
            ils(d);
            d.pitch_rad.status = SignalStatus::Failed;
            d.roll_rad.status = SignalStatus::Failed;
        },
        [150.0, 0.0, 0.0],
        "terrain",
    );
}
