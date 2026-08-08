#![allow(clippy::expect_used, clippy::panic)]

use indicate_alerts::AlertOutput;
use indicate_instrument_scene::SceneWriter;
use indicate_instrument_state::PanelData;

use crate::config::ConfigBlob;
use crate::descriptor::{BackgroundCapability, DesignFrame, PanelDescriptor, PanelDrawError};
use crate::frame::FrameRefusal;
use crate::group_set::GroupSet;

fn draw_nothing(
    _data: &PanelData,
    _config: &ConfigBlob<'_>,
    _alerts: Option<&AlertOutput>,
    _frame: DesignFrame,
    _scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    Ok(())
}

const fn frame(width: f32, height: f32) -> DesignFrame {
    DesignFrame { width, height }
}

const MIN: DesignFrame = frame(480.0, 360.0);
const MAX: DesignFrame = frame(600.0, 450.0);

/// 480×360 to 600×450, both 4:3, on a 40×30 grid — a range wide enough
/// that each bound can be violated on its own.
const RANGED: PanelDescriptor = PanelDescriptor {
    id: "ranged",
    title: "Ranged",
    required_layers: 0b0000_0110,
    required_groups: GroupSet::EMPTY,
    frame_min: MIN,
    frame_max: MAX,
    frame_step: (40.0, 30.0),
    aspect_min: 1.30,
    aspect_max: 1.37,
    canonical_frames: &[MIN, MAX],
    background: BackgroundCapability::NotUsed,
    config_schema: &[],
    group_regions: &[],
    extreme_states: &[],
    raster_baselines: &[],
    draw: draw_nothing,
};

#[test]
fn both_ends_of_the_declared_range_are_accepted() {
    assert_eq!(RANGED.accepts(MIN), Ok(()));
    assert_eq!(RANGED.accepts(MAX), Ok(()));
}

#[test]
fn an_interior_frame_on_the_grid_is_accepted() {
    assert_eq!(RANGED.accepts(frame(520.0, 390.0)), Ok(()));
}

#[test]
fn each_bound_refuses_by_name() {
    // Below the minimum, and above the maximum.
    assert_eq!(
        RANGED.accepts(frame(440.0, 330.0)),
        Err(FrameRefusal::OutOfRange)
    );
    assert_eq!(
        RANGED.accepts(frame(640.0, 480.0)),
        Err(FrameRefusal::OutOfRange)
    );
    // In range and 4:3, so only the grid can refuse it.
    assert_eq!(
        RANGED.accepts(frame(500.0, 375.0)),
        Err(FrameRefusal::OffStep)
    );
    // In range and on the grid, but a shape the layout never declared.
    assert_eq!(
        RANGED.accepts(frame(600.0, 360.0)),
        Err(FrameRefusal::Aspect)
    );
}

#[test]
fn a_frame_that_is_not_a_frame_is_refused_before_any_bound() {
    for bad in [
        frame(f32::NAN, 360.0),
        frame(480.0, f32::INFINITY),
        frame(0.0, 360.0),
        frame(480.0, -360.0),
    ] {
        assert_eq!(
            RANGED.accepts(bad),
            Err(FrameRefusal::Degenerate),
            "{bad:?} is not a frame"
        );
    }
}

/// A degenerate range accepts exactly one frame, which is what every
/// shipped panel declares today.
#[test]
fn a_degenerate_range_accepts_only_its_single_frame() {
    const FIXED: PanelDescriptor = PanelDescriptor {
        frame_max: MIN,
        canonical_frames: &[MIN],
        ..RANGED
    };
    assert_eq!(FIXED.accepts(MIN), Ok(()));
    assert_eq!(
        FIXED.accepts(frame(520.0, 390.0)),
        Err(FrameRefusal::OutOfRange)
    );
}

/// A shell may hold a descriptor this crate never validated — an
/// out-of-repo set, a hand-built fixture — so a bound that is not a
/// number must refuse every frame rather than accept every frame.
#[test]
fn bounds_that_are_not_numbers_refuse_rather_than_admit() {
    const BAD_MAX: PanelDescriptor = PanelDescriptor {
        frame_max: frame(f32::NAN, f32::NAN),
        ..RANGED
    };
    assert_eq!(BAD_MAX.accepts(MIN), Err(FrameRefusal::OutOfRange));

    const BAD_ASPECT: PanelDescriptor = PanelDescriptor {
        aspect_max: f32::NAN,
        ..RANGED
    };
    assert_eq!(BAD_ASPECT.accepts(MIN), Err(FrameRefusal::Aspect));

    // A zero step divides nothing evenly; fmod yields NaN, which is not
    // on any grid line.
    const ZERO_STEP: PanelDescriptor = PanelDescriptor {
        frame_step: (0.0, 0.0),
        ..RANGED
    };
    assert_eq!(ZERO_STEP.accepts(MIN), Err(FrameRefusal::OffStep));
}
