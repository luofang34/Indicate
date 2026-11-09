//! The registered panels: one per built-in gauge, each drawing its
//! `static` spec from resolved state under daylight.

use indicate_instrument_descriptor::{
    DesignFrame, DrawContext, ExtremeState, GroupSet, PanelDescriptor, PanelDrawError, PanelSet,
    Region, RenderSpace, SceneTarget, UnderlayContract, states,
};
use indicate_instrument_scene::LayerId;
use indicate_instrument_state::{AircraftState, AltitudeClass, AltitudeDeclaration, GroupId};

use crate::gauges::{
    AIRSPEED_KT, AIRSPEED_MPH, ALTIMETER, MANIFOLD_PRESSURE, TACHOMETER, VERTICAL_SPEED,
};
use crate::pose::{pose, reading};

mod period;

use crate::scene::draw_dial;
use crate::spec::{AnalogDialSpec, Lighting};
pub use period::{
    CLOCK_DESCRIPTOR, DIRECTIONAL_GYRO_DESCRIPTOR, GYRO_HORIZON_DESCRIPTOR,
    MAGNETIC_COMPASS_DESCRIPTOR, SYNCHROSCOPE_DESCRIPTOR, TURN_AND_BANK_DESCRIPTOR,
    magnetic_cruise,
};

/// Every gauge is laid out for this one square frame.
pub const DIAL_FRAME: DesignFrame = DesignFrame {
    width: 240.0,
    height: 240.0,
};

const SPACE: RenderSpace = RenderSpace::Planar2D {
    frame_min: DIAL_FRAME,
    frame_max: DIAL_FRAME,
    frame_step: (1.0, 1.0),
    aspect_min: 0.95,
    aspect_max: 1.05,
    canonical_frames: &[DIAL_FRAME],
};

/// Both bands are opened on every frame; the flag band may be empty.
const LAYERS: u16 = (1 << LayerId::Tapes.to_u8()) | (1 << LayerId::Annunciation.to_u8());

fn draw_registered(
    spec: &AnalogDialSpec<'_>,
    ctx: &DrawContext<'_>,
    target: SceneTarget<'_, '_>,
) -> Result<(), PanelDrawError> {
    let SceneTarget::Planar2D { frame, writer } = target else {
        return Err(PanelDrawError::WrongTarget);
    };
    ctx.config().require_schema(&[])?;
    let beside = indicate_instrument_state::BesideSamples {
        engines: *ctx.engines(),
        tanks: *ctx.tanks(),
    };
    let dial_pose = pose(spec, &reading(spec, ctx.data(), &beside));
    draw_dial(spec, &dial_pose, Lighting::Day, frame, writer)
}

const fn panel(
    id: &'static str,
    title: &'static str,
    groups: GroupSet,
    group_regions: &'static [(GroupId, Region)],
    extreme_states: &'static [ExtremeState],
    draw: indicate_instrument_descriptor::DrawFn,
) -> PanelDescriptor {
    PanelDescriptor {
        id,
        title,
        space: SPACE,
        required_layers: LAYERS,
        required_groups: groups,
        underlay: UnderlayContract::Opaque,
        config_schema: &[],
        accepted_intents: &[],
        aux_channels: &[],
        group_regions,
        extreme_states,
        raster_baselines: &[],
        draw,
    }
}

fn draw_airspeed_kt(ctx: &DrawContext<'_>, t: SceneTarget<'_, '_>) -> Result<(), PanelDrawError> {
    draw_registered(&AIRSPEED_KT, ctx, t)
}
fn draw_airspeed_mph(ctx: &DrawContext<'_>, t: SceneTarget<'_, '_>) -> Result<(), PanelDrawError> {
    draw_registered(&AIRSPEED_MPH, ctx, t)
}
fn draw_altimeter(ctx: &DrawContext<'_>, t: SceneTarget<'_, '_>) -> Result<(), PanelDrawError> {
    draw_registered(&ALTIMETER, ctx, t)
}
fn draw_vertical_speed(
    ctx: &DrawContext<'_>,
    t: SceneTarget<'_, '_>,
) -> Result<(), PanelDrawError> {
    draw_registered(&VERTICAL_SPEED, ctx, t)
}
fn draw_manifold(ctx: &DrawContext<'_>, t: SceneTarget<'_, '_>) -> Result<(), PanelDrawError> {
    draw_registered(&MANIFOLD_PRESSURE, ctx, t)
}
fn draw_tachometer(ctx: &DrawContext<'_>, t: SceneTarget<'_, '_>) -> Result<(), PanelDrawError> {
    draw_registered(&TACHOMETER, ctx, t)
}

/// The cruise corpus state with a barometric altitude of 5,280 ft at
/// the standard setting, 29.92 inHg: the corpus declares only local
/// height, which an altimeter cannot show.
#[must_use]
pub fn baro_cruise() -> AircraftState {
    with_baro(1609.3, 1013.2)
}

/// 13,000 ft at 30.13 inHg: every hand away from twelve and the drum's
/// lower wheels away from zero.
#[must_use]
pub fn baro_high() -> AircraftState {
    with_baro(3962.4, 1020.3)
}

fn with_baro(sample_m: f32, setting_hpa: f32) -> AircraftState {
    let mut state = states::typical();
    state.altitude = AltitudeDeclaration {
        reference_class: AltitudeClass::BaroIndicated,
        sample_m: Some(sample_m),
        ..AltitudeDeclaration::default()
    };
    if let Some(air) = state.air.data.as_mut() {
        air.baro_setting_hpa = Some(setting_hpa);
    }
    state
}

const BARO_STATES: &[ExtremeState] = &[
    ExtremeState {
        id: "baro-cruise",
        build: baro_cruise,
        engines: indicate_instrument_descriptor::states::TYPICAL_ENGINES,

        tanks: indicate_instrument_descriptor::states::TYPICAL_TANKS,
    },
    ExtremeState {
        id: "baro-high",
        build: baro_high,
        engines: indicate_instrument_descriptor::states::TYPICAL_ENGINES,

        tanks: indicate_instrument_descriptor::states::TYPICAL_TANKS,
    },
];

/// The Kollsman window, where the altimeter's only claimed numerals are.
const KOLLSMAN_REGION: &[(GroupId, Region)] = &[(
    GroupId::Air,
    Region {
        x: 148.0,
        y: 106.0,
        width: 54.0,
        height: 28.0,
    },
)];

const AIR: GroupSet = GroupSet::of(&[GroupId::Air, GroupId::Trust]);
const ENGINE: GroupSet = GroupSet::of(&[GroupId::Engine, GroupId::Trust]);

/// The six-pack airspeed indicator in knots.
pub const AIRSPEED_KT_DESCRIPTOR: PanelDescriptor = panel(
    "analog-airspeed-kt",
    "Airspeed (kt)",
    AIR,
    &[],
    &[],
    draw_airspeed_kt,
);
/// The fighter airspeed indicator in miles per hour.
pub const AIRSPEED_MPH_DESCRIPTOR: PanelDescriptor = panel(
    "analog-airspeed-mph",
    "Airspeed (mph)",
    AIR,
    &[],
    &[],
    draw_airspeed_mph,
);
/// The sensitive altimeter.
pub const ALTIMETER_DESCRIPTOR: PanelDescriptor = panel(
    "analog-altimeter",
    "Altimeter",
    GroupSet::of(&[GroupId::Altitude, GroupId::Air, GroupId::Trust]),
    KOLLSMAN_REGION,
    BARO_STATES,
    draw_altimeter,
);
/// The vertical speed indicator.
pub const VERTICAL_SPEED_DESCRIPTOR: PanelDescriptor = panel(
    "analog-vertical-speed",
    "Vertical speed",
    AIR,
    &[],
    &[],
    draw_vertical_speed,
);
/// Engine 1 manifold pressure.
pub const MANIFOLD_PRESSURE_DESCRIPTOR: PanelDescriptor = panel(
    "analog-manifold-pressure",
    "Manifold pressure",
    ENGINE,
    &[],
    &[],
    draw_manifold,
);
/// Engine 1 tachometer.
pub const TACHOMETER_DESCRIPTOR: PanelDescriptor = panel(
    "analog-tachometer",
    "Tachometer",
    ENGINE,
    &[],
    &[],
    draw_tachometer,
);

/// Every analog panel, in set order.
pub const ANALOG_PANELS: &[PanelDescriptor] = &[
    AIRSPEED_KT_DESCRIPTOR,
    AIRSPEED_MPH_DESCRIPTOR,
    ALTIMETER_DESCRIPTOR,
    VERTICAL_SPEED_DESCRIPTOR,
    MANIFOLD_PRESSURE_DESCRIPTOR,
    TACHOMETER_DESCRIPTOR,
    GYRO_HORIZON_DESCRIPTOR,
    DIRECTIONAL_GYRO_DESCRIPTOR,
    TURN_AND_BANK_DESCRIPTOR,
    MAGNETIC_COMPASS_DESCRIPTOR,
    CLOCK_DESCRIPTOR,
    SYNCHROSCOPE_DESCRIPTOR,
];

/// The analog set. It ships outside the flight set: a shell composes
/// it for a cockpit with round instruments.
pub const ANALOG_SET: PanelSet = PanelSet {
    id: "analog",
    panels: ANALOG_PANELS,
};
