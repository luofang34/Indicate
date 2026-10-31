#![allow(clippy::expect_used, clippy::panic)]

use indicate_instrument_scene::volume::Extent3D;
use indicate_interaction::ViewLimits;

use crate::context::DrawContext;
use crate::descriptor::{DesignFrame, InstrumentDescriptor, InstrumentDrawError};
use crate::frame::{ExtentRefusal, FrameRefusal};
use crate::group_set::GroupSet;
use crate::space::{RenderSpace, SceneTarget, UnderlayContract};

pub(super) fn draw_nothing(
    _ctx: &DrawContext<'_>,
    _target: SceneTarget<'_, '_>,
) -> Result<(), InstrumentDrawError> {
    Ok(())
}

pub(super) const fn frame(width: f32, height: f32) -> DesignFrame {
    DesignFrame { width, height }
}

pub(super) const fn planar_space(
    frame_min: DesignFrame,
    frame_max: DesignFrame,
    frame_step: (f32, f32),
    aspect_min: f32,
    aspect_max: f32,
    canonical_frames: &'static [DesignFrame],
) -> RenderSpace {
    RenderSpace::Planar2D {
        frame_min,
        frame_max,
        frame_step,
        aspect_min,
        aspect_max,
        canonical_frames,
    }
}

pub(super) const MIN: DesignFrame = frame(480.0, 360.0);
pub(super) const MAX: DesignFrame = frame(600.0, 450.0);

/// 480x360 to 600x450, both 4:3, on a 40x30 grid.
pub(super) const RANGED: InstrumentDescriptor = InstrumentDescriptor {
    id: "ranged",
    title: "Ranged",
    space: planar_space(MIN, MAX, (40.0, 30.0), 1.30, 1.37, &[MIN, MAX]),
    required_layers: 0b0000_0110,
    required_groups: GroupSet::EMPTY,
    underlay: UnderlayContract::None,
    config_schema: &[],
    accepted_intents: &[],
    aux_channels: &[],
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
    assert_eq!(
        RANGED.accepts(frame(440.0, 330.0)),
        Err(FrameRefusal::OutOfRange)
    );
    assert_eq!(
        RANGED.accepts(frame(640.0, 480.0)),
        Err(FrameRefusal::OutOfRange)
    );
    assert_eq!(
        RANGED.accepts(frame(500.0, 375.0)),
        Err(FrameRefusal::OffStep)
    );
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

#[test]
fn a_degenerate_range_accepts_only_its_single_frame() {
    const FIXED: InstrumentDescriptor = InstrumentDescriptor {
        space: planar_space(MIN, MIN, (40.0, 30.0), 1.30, 1.37, &[MIN]),
        ..RANGED
    };
    assert_eq!(FIXED.accepts(MIN), Ok(()));
    assert_eq!(
        FIXED.accepts(frame(520.0, 390.0)),
        Err(FrameRefusal::OutOfRange)
    );
}

#[test]
fn bounds_that_are_not_numbers_refuse_rather_than_admit() {
    const BAD_MAX: InstrumentDescriptor = InstrumentDescriptor {
        space: planar_space(
            MIN,
            frame(f32::NAN, f32::NAN),
            (40.0, 30.0),
            1.30,
            1.37,
            &[MIN],
        ),
        ..RANGED
    };
    assert_eq!(BAD_MAX.accepts(MIN), Err(FrameRefusal::OutOfRange));

    const BAD_ASPECT: InstrumentDescriptor = InstrumentDescriptor {
        space: planar_space(MIN, MAX, (40.0, 30.0), 1.30, f32::NAN, &[MIN, MAX]),
        ..RANGED
    };
    assert_eq!(BAD_ASPECT.accepts(MIN), Err(FrameRefusal::Aspect));

    const ZERO_STEP: InstrumentDescriptor = InstrumentDescriptor {
        space: planar_space(MIN, MAX, (0.0, 0.0), 1.30, 1.37, &[MIN, MAX]),
        ..RANGED
    };
    assert_eq!(ZERO_STEP.accepts(MIN), Err(FrameRefusal::OffStep));
}

#[test]
fn a_space_larger_than_the_range_gets_the_largest_declared_frame() {
    assert_eq!(RANGED.choose_frame(frame(4096.0, 2160.0)), Ok(MAX));
    assert_eq!(RANGED.choose_frame(MAX), Ok(MAX));
}

#[test]
fn the_unconstrained_shell_gets_what_it_asks_for() {
    const FIXED: InstrumentDescriptor = InstrumentDescriptor {
        space: planar_space(MIN, MIN, (1.0, 1.0), 1.30, 1.37, &[MIN]),
        ..RANGED
    };
    assert_eq!(RANGED.choose_frame(MAX), Ok(MAX));
    assert_eq!(FIXED.choose_frame(MIN), Ok(MIN));
}

#[test]
fn a_space_between_grid_lines_floors_onto_the_grid() {
    let chosen = RANGED
        .choose_frame(frame(599.0, 449.0))
        .expect("inside the range");
    assert_eq!(chosen, frame(560.0, 420.0));
    assert_eq!(RANGED.accepts(chosen), Ok(()));
}

#[test]
fn a_space_of_the_wrong_shape_gets_the_largest_frame_of_the_right_shape() {
    let chosen = RANGED
        .choose_frame(frame(640.0, 400.0))
        .expect("a 4:3 frame fits");
    assert_eq!(chosen, frame(520.0, 390.0));
    assert_eq!(RANGED.accepts(chosen), Ok(()));
    for bigger in [frame(560.0, 390.0), frame(600.0, 390.0)] {
        assert!(
            RANGED.accepts(bigger).is_err(),
            "{bigger:?} would have been a larger admissible answer"
        );
    }
}

#[test]
fn a_space_smaller_than_the_minimum_is_refused() {
    assert_eq!(
        RANGED.choose_frame(frame(479.0, 360.0)),
        Err(FrameRefusal::OutOfRange)
    );
    assert_eq!(
        RANGED.choose_frame(frame(480.0, 359.0)),
        Err(FrameRefusal::OutOfRange)
    );
}

#[test]
fn a_degenerate_space_is_refused() {
    for bad in [
        frame(f32::NAN, 360.0),
        frame(480.0, f32::INFINITY),
        frame(0.0, 360.0),
        frame(-480.0, -360.0),
    ] {
        assert_eq!(
            RANGED.choose_frame(bad),
            Err(FrameRefusal::Degenerate),
            "{bad:?}"
        );
    }
}

#[test]
fn every_chosen_frame_is_one_the_panel_accepts() {
    let mut checked = 0;
    let mut w = 400.0f32;
    while w <= 700.0 {
        let mut h = 300.0f32;
        while h <= 520.0 {
            if let Ok(chosen) = RANGED.choose_frame(frame(w, h)) {
                assert_eq!(
                    RANGED.accepts(chosen),
                    Ok(()),
                    "chose {chosen:?} for {w}x{h}"
                );
                assert!(
                    chosen.width <= w && chosen.height <= h,
                    "chose {chosen:?}, which does not fit {w}x{h}"
                );
                checked += 1;
            }
            h += 7.0;
        }
        w += 11.0;
    }
    assert!(checked > 100, "the sweep exercised {checked} spaces");
}

#[test]
fn a_single_frame_panel_offers_only_its_one_frame() {
    const FIXED: InstrumentDescriptor = InstrumentDescriptor {
        space: planar_space(MIN, MIN, (1.0, 1.0), 1.30, 1.37, &[MIN]),
        ..RANGED
    };
    assert_eq!(FIXED.choose_frame(frame(1920.0, 1080.0)), Ok(MIN));
    assert_eq!(FIXED.choose_frame(MIN), Ok(MIN));
    assert_eq!(
        FIXED.choose_frame(frame(479.0, 359.0)),
        Err(FrameRefusal::OutOfRange)
    );
}

#[test]
fn volume_3d_descriptor_accepts_extent_bounds_and_refuses_2d() {
    const EXT_MIN: Extent3D = Extent3D::new(0.30, 0.30, 0.30);
    const EXT_CANON: Extent3D = Extent3D::new(0.60, 0.45, 0.60);
    const EXT_MAX: Extent3D = Extent3D::new(1.50, 1.50, 1.50);
    const VOL: InstrumentDescriptor = InstrumentDescriptor {
        id: "vol",
        title: "Volume",
        space: RenderSpace::Volume3D {
            extent_min_m: EXT_MIN,
            extent_max_m: EXT_MAX,
            canonical_extents_m: &[EXT_CANON],
            view_limits: ViewLimits::VOLUMETRIC_DEFAULT,
        },
        ..RANGED
    };
    assert_eq!(VOL.accepts_2d(MIN), Err(FrameRefusal::NotPlanar2D));
    assert_eq!(
        RANGED.accepts_volume_3d(EXT_CANON),
        Err(ExtentRefusal::NotVolume3D)
    );
    assert_eq!(VOL.accepts_volume_3d(EXT_CANON), Ok(()));
    assert_eq!(
        VOL.accepts_volume_3d(Extent3D::new(0.20, 0.45, 0.60)),
        Err(ExtentRefusal::OutOfRange)
    );
    assert_eq!(
        VOL.accepts_volume_3d(Extent3D::new(0.0, 0.45, 0.60)),
        Err(ExtentRefusal::Degenerate)
    );
}
