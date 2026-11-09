//! The registered gyro, heading, clock and synchroscope panels of the
//! 1943 fighter panel.

use indicate_instrument_descriptor::{
    DrawContext, ExtremeState, GroupSet, PanelDescriptor, PanelDrawError, SceneTarget, states,
};
use indicate_instrument_state::{
    AircraftState, GroupId, MagneticVariation, Stamped, VariationSourceId,
};

use super::{draw_registered, panel};
use crate::gauges::{
    CLOCK, DIRECTIONAL_GYRO, GYRO_HORIZON, MAGNETIC_COMPASS, SYNCHROSCOPE, TURN_AND_BANK,
};

fn draw_horizon(ctx: &DrawContext<'_>, t: SceneTarget<'_, '_>) -> Result<(), PanelDrawError> {
    draw_registered(&GYRO_HORIZON, ctx, t)
}
fn draw_gyro(ctx: &DrawContext<'_>, t: SceneTarget<'_, '_>) -> Result<(), PanelDrawError> {
    draw_registered(&DIRECTIONAL_GYRO, ctx, t)
}
fn draw_turn(ctx: &DrawContext<'_>, t: SceneTarget<'_, '_>) -> Result<(), PanelDrawError> {
    draw_registered(&TURN_AND_BANK, ctx, t)
}
fn draw_compass(ctx: &DrawContext<'_>, t: SceneTarget<'_, '_>) -> Result<(), PanelDrawError> {
    draw_registered(&MAGNETIC_COMPASS, ctx, t)
}
fn draw_clock(ctx: &DrawContext<'_>, t: SceneTarget<'_, '_>) -> Result<(), PanelDrawError> {
    draw_registered(&CLOCK, ctx, t)
}
fn draw_synchroscope(ctx: &DrawContext<'_>, t: SceneTarget<'_, '_>) -> Result<(), PanelDrawError> {
    draw_registered(&SYNCHROSCOPE, ctx, t)
}

/// The cruise corpus state with 3° of easterly variation, declared
/// valid: the corpus heading is true, and a compass needs the
/// variation to read magnetic.
#[must_use]
pub fn magnetic_cruise() -> AircraftState {
    let mut state = states::typical();
    state.variation = Stamped {
        data: Some(MagneticVariation {
            east_positive_rad: 3.0_f32.to_radians(),
            source: VariationSourceId(1),
        }),
        age_ms: Some(80.0),
    };
    state.valid.variation = true;
    state
}

const MAGNETIC_STATES: &[ExtremeState] = &[ExtremeState {
    id: "magnetic-cruise",
    build: magnetic_cruise,
    engines: states::TYPICAL_ENGINES,
    tanks: states::TYPICAL_TANKS,
}];

/// The gyro horizon.
pub const GYRO_HORIZON_DESCRIPTOR: PanelDescriptor = panel(
    "analog-gyro-horizon",
    "Gyro horizon",
    GroupSet::of(&[GroupId::Attitude, GroupId::Trust]),
    &[],
    &[],
    draw_horizon,
);
/// The drum-window directional gyro.
pub const DIRECTIONAL_GYRO_DESCRIPTOR: PanelDescriptor = panel(
    "analog-directional-gyro",
    "Directional gyro",
    GroupSet::of(&[GroupId::Heading, GroupId::Trust]),
    &[],
    &[],
    draw_gyro,
);
/// The turn and bank indicator.
pub const TURN_AND_BANK_DESCRIPTOR: PanelDescriptor = panel(
    "analog-turn-and-bank",
    "Turn and bank",
    GroupSet::of(&[GroupId::Attitude, GroupId::Dynamics, GroupId::Trust]),
    &[],
    &[],
    draw_turn,
);
/// The panel magnetic compass.
pub const MAGNETIC_COMPASS_DESCRIPTOR: PanelDescriptor = panel(
    "analog-magnetic-compass",
    "Magnetic compass",
    GroupSet::of(&[GroupId::Heading, GroupId::Variation, GroupId::Trust]),
    &[],
    MAGNETIC_STATES,
    draw_compass,
);
/// The panel clock.
pub const CLOCK_DESCRIPTOR: PanelDescriptor = panel(
    "analog-clock",
    "Clock",
    GroupSet::of(&[GroupId::Clock, GroupId::Trust]),
    &[],
    &[],
    draw_clock,
);
/// The engine synchroscope: engine 1 against engine 2.
pub const SYNCHROSCOPE_DESCRIPTOR: PanelDescriptor = panel(
    "analog-synchroscope",
    "Synchroscope",
    GroupSet::of(&[GroupId::Engine, GroupId::Trust]),
    &[],
    &[],
    draw_synchroscope,
);
