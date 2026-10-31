#![allow(clippy::expect_used, clippy::panic)]

//! One must-fail case per rule.

use indicate_instrument_descriptor::{DesignFrame, PanelDescriptor};

use super::super::tests::{FRAME, RANGED_MAX, panel, planar, ranged};
use super::super::{Registry, RegistryError};

#[test]
fn a_degenerate_frame_bound_is_refused() {
    static FLAT: [PanelDescriptor; 1] = [{
        let mut p = panel("pfd");
        p.space = planar(
            DesignFrame {
                width: 480.0,
                height: 0.0,
            },
            FRAME,
            (1.0, 1.0),
            1.30,
            1.37,
            &[FRAME],
        );
        p
    }];
    assert_eq!(
        Registry::new(&FLAT).map(|_| ()),
        Err(RegistryError::BadFrameBounds { index: 0 })
    );
}

#[test]
fn an_inverted_frame_range_is_refused() {
    static BACKWARDS: [PanelDescriptor; 1] = [{
        let mut p = panel("pfd");
        p.space = planar(
            FRAME,
            DesignFrame {
                width: 440.0,
                height: 360.0,
            },
            (1.0, 1.0),
            1.30,
            1.37,
            &[FRAME],
        );
        p
    }];
    assert_eq!(
        Registry::new(&BACKWARDS).map(|_| ()),
        Err(RegistryError::FrameRangeInverted { index: 0 })
    );
}

#[test]
fn a_degenerate_frame_step_is_refused() {
    static ZERO: [PanelDescriptor; 1] = [{
        let mut p = panel("pfd");
        p.space = planar(FRAME, FRAME, (0.0, 1.0), 1.30, 1.37, &[FRAME]);
        p
    }];
    assert_eq!(
        Registry::new(&ZERO).map(|_| ()),
        Err(RegistryError::BadFrameStep { index: 0 })
    );
}

#[test]
fn inverted_aspect_bounds_are_refused() {
    static BACKWARDS: [PanelDescriptor; 1] = [{
        let mut p = panel("pfd");
        p.space = planar(FRAME, FRAME, (1.0, 1.0), 2.0, 1.0, &[FRAME]);
        p
    }];
    assert_eq!(
        Registry::new(&BACKWARDS).map(|_| ()),
        Err(RegistryError::BadAspectBounds { index: 0 })
    );
}

#[test]
fn a_panel_pinning_no_canonical_frames_is_refused() {
    static UNPINNED: [PanelDescriptor; 1] = [{
        let mut p = panel("pfd");
        p.space = planar(FRAME, FRAME, (1.0, 1.0), 1.30, 1.37, &[]);
        p
    }];
    assert_eq!(
        Registry::new(&UNPINNED).map(|_| ()),
        Err(RegistryError::NoCanonicalFrames { index: 0 })
    );
}

#[test]
fn canonical_frames_must_include_both_ends_of_the_range() {
    static NO_MIN: [PanelDescriptor; 1] = [{
        let mut p = ranged("pfd");
        p.space = planar(FRAME, RANGED_MAX, (40.0, 30.0), 1.30, 1.37, &[RANGED_MAX]);
        p
    }];
    assert_eq!(
        Registry::new(&NO_MIN).map(|_| ()),
        Err(RegistryError::CanonicalFramesMissingMin { index: 0 })
    );
    static NO_MAX: [PanelDescriptor; 1] = [{
        let mut p = ranged("pfd");
        p.space = planar(FRAME, RANGED_MAX, (40.0, 30.0), 1.30, 1.37, &[FRAME]);
        p
    }];
    assert_eq!(
        Registry::new(&NO_MAX).map(|_| ()),
        Err(RegistryError::CanonicalFramesMissingMax { index: 0 })
    );
}

#[test]
fn a_canonical_frame_outside_the_range_is_refused() {
    static BEYOND: [PanelDescriptor; 1] = [{
        let mut p = ranged("pfd");
        p.space = planar(
            FRAME,
            RANGED_MAX,
            (40.0, 30.0),
            1.30,
            1.37,
            &[
                FRAME,
                RANGED_MAX,
                DesignFrame {
                    width: 640.0,
                    height: 480.0,
                },
            ],
        );
        p
    }];
    assert_eq!(
        Registry::new(&BEYOND).map(|_| ()),
        Err(RegistryError::CanonicalFrameOutOfRange {
            index: 0,
            position: 2,
        })
    );
}

#[test]
fn a_canonical_frame_off_the_step_grid_is_refused() {
    static OFF: [PanelDescriptor; 1] = [{
        let mut p = ranged("pfd");
        p.space = planar(
            FRAME,
            RANGED_MAX,
            (40.0, 30.0),
            1.30,
            1.37,
            &[
                FRAME,
                RANGED_MAX,
                DesignFrame {
                    width: 520.0,
                    height: 380.0,
                },
            ],
        );
        p
    }];
    assert_eq!(
        Registry::new(&OFF).map(|_| ()),
        Err(RegistryError::CanonicalFrameOffStep {
            index: 0,
            position: 2,
        })
    );
}

#[test]
fn a_canonical_frame_outside_the_aspect_bounds_is_refused() {
    static SQUAT: [PanelDescriptor; 1] = [{
        let mut p = ranged("pfd");
        p.space = planar(
            FRAME,
            RANGED_MAX,
            (40.0, 30.0),
            1.30,
            1.37,
            &[
                FRAME,
                RANGED_MAX,
                DesignFrame {
                    width: 600.0,
                    height: 360.0,
                },
            ],
        );
        p
    }];
    assert_eq!(
        Registry::new(&SQUAT).map(|_| ()),
        Err(RegistryError::CanonicalFrameAspect {
            index: 0,
            position: 2,
        })
    );
}

#[test]
fn a_raster_baseline_at_an_unrendered_frame_is_refused() {
    static STRAY: [PanelDescriptor; 1] = [{
        let mut p = panel("pfd");
        p.raster_baselines = &[(
            DesignFrame {
                width: 240.0,
                height: 180.0,
            },
            "00",
        )];
        p
    }];
    assert_eq!(
        Registry::new(&STRAY).map(|_| ()),
        Err(RegistryError::RasterBaselineNotCanonical {
            index: 0,
            position: 0,
        })
    );
}

#[test]
fn a_repeated_canonical_frame_is_refused() {
    static TWICE: [PanelDescriptor; 1] = [{
        let mut p = panel("pfd");
        p.space = planar(
            FRAME,
            RANGED_MAX,
            (40.0, 30.0),
            1.30,
            1.37,
            &[FRAME, RANGED_MAX, FRAME],
        );
        p
    }];
    assert_eq!(
        Registry::new(&TWICE).map(|_| ()),
        Err(RegistryError::DuplicateCanonicalFrame {
            index: 0,
            position: 2,
        })
    );
}

#[test]
fn two_raster_baselines_at_one_frame_are_refused() {
    static CONFLICT: [PanelDescriptor; 1] = [{
        let mut p = panel("pfd");
        p.raster_baselines = &[(FRAME, "aa"), (FRAME, "bb")];
        p
    }];
    assert_eq!(
        Registry::new(&CONFLICT).map(|_| ()),
        Err(RegistryError::DuplicateRasterBaseline {
            index: 0,
            position: 1,
        })
    );
}
