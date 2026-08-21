//! The obscuration floor, measured in pixels.
//!
//! `validate_composition` is the gate, but a gate is only as good as the
//! band it reads. These reproduce two compositions that were once
//! admitted and would have covered warning ink: the floor has to refuse
//! them, and the pixel measurement says what was at stake if it does
//! not.
#![allow(clippy::expect_used, clippy::panic)]

use indicate_alerts::{
    AlertCondition, AlertContext, AlertEvent, AlertManager, AlertOutput, AlertProfile,
    MiscompareFault,
};
use indicate_instrument_panels::{BUILTIN_CRITICALITY_BANDS, BUILTIN_PANELS};
use indicate_instrument_registry::{
    CompositionDescriptor, CompositionError, DesignFrame, Region, Registry, Slot,
    validate_composition,
};
use indicate_instrument_scene::MAX_SCENE_BYTES;

use std::vec;
use std::vec::Vec;

use super::{PANEL_FRAME, typical};
use crate::composition::{CompositionInputs, render_composition};
use crate::{FrameId, FramebufferDims};

/// Alert-row red, the never-skinnable failure color every panel paints a
/// warning row in. Restated rather than imported because the rasterizer
/// does not depend on the symbology crate; a drift would fail the
/// coverage assertion loudly rather than silently pass.
const ALERT_RED: [u8; 4] = [255, 0, 0, 255];

/// One warning-class alert: enough to put a red row in every panel's
/// annunciation band, which is the ink these compositions would cover.
fn one_warning() -> AlertOutput {
    let mut manager = AlertManager::new();
    manager.step(
        &AlertProfile::simulator(),
        &[AlertEvent::Assert(AlertCondition::Miscompare(
            MiscompareFault::Attitude,
        ))],
        AlertContext::default(),
        1_000,
    )
}

fn screen(width: f32, height: f32) -> DesignFrame {
    DesignFrame { width, height }
}

const fn placed(panel: &'static str, y: f32, occludes: &'static [&'static str]) -> Slot {
    Slot {
        panel,
        rect: Region {
            x: 0.0,
            y,
            width: 480.0,
            height: 360.0,
        },
        occludes,
    }
}

fn builtin() -> Registry {
    Registry::new(BUILTIN_PANELS).expect("shipped panels compose")
}

/// Renders `composition` with one warning alert active and counts the
/// alert-red pixels anywhere on the screen.
fn red_pixels(composition: &CompositionDescriptor) -> usize {
    let (w, h) = (
        composition.screen.width as u32,
        composition.screen.height as u32,
    );
    let mut pixels = vec![0u8; (w * h * 4) as usize];
    let mut scratch = vec![0u8; MAX_SCENE_BYTES];
    let data = typical();
    let alerts = one_warning();
    let mut inputs = CompositionInputs {
        data: &data,
        alerts: Some(&alerts),
        scratch: &mut scratch,
    };
    render_composition(
        &builtin(),
        composition,
        &mut inputs,
        &mut pixels,
        FramebufferDims::tight(w, h),
        FrameId::default(),
    )
    .expect("renders");
    count_red(&pixels)
}

fn count_red(pixels: &[u8]) -> usize {
    pixels
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|px| **px == ALERT_RED)
        .count()
}

/// The PFD alone, so the measurement below has a baseline for how much
/// warning ink there was to lose.
const PFD_ALONE_TALL: CompositionDescriptor = CompositionDescriptor {
    screen: DesignFrame {
        width: 480.0,
        height: 660.0,
    },
    slots: &[placed("pfd", 0.0, &[])],
};

/// A declared obscuration of the PFD by a panel below it. The monitor's
/// rect starts at y 300, so it covers the PFD's alert stack — which a
/// declaration must never buy.
const MONITOR_OVER_PFD: CompositionDescriptor = CompositionDescriptor {
    screen: DesignFrame {
        width: 480.0,
        height: 660.0,
    },
    slots: &[placed("pfd", 0.0, &[]), placed("monitor", 300.0, &["pfd"])],
};

#[test]
fn a_declared_obscuration_may_not_cover_the_alert_stack() {
    // What is at stake, stated in pixels rather than in prose: the PFD
    // paints warning ink, and the composition below would erase it.
    let alone = red_pixels(&PFD_ALONE_TALL);
    assert!(alone > 0, "the fixture must actually raise a warning row");

    let refusal = validate_composition(&builtin(), &MONITOR_OVER_PFD, &BUILTIN_CRITICALITY_BANDS)
        .expect_err("covering a warning is refused however it is declared");
    assert!(
        matches!(refusal, CompositionError::CriticalityObscured { .. }),
        "expected the criticality floor, got {refusal:?}"
    );
}

/// The monitor under the HSI. The monitor's own annunciation band —
/// its alert stack, and the failure X it paints over the whole frame
/// when its channel fails — lies under the HSI's rect.
const HSI_OVER_MONITOR: CompositionDescriptor = CompositionDescriptor {
    screen: DesignFrame {
        width: 480.0,
        height: 540.0,
    },
    slots: &[
        placed("monitor", 0.0, &[]),
        placed("hsi", 180.0, &["monitor"]),
    ],
};

#[test]
fn a_declared_obscuration_may_not_cover_the_monitors_annunciation() {
    let refusal = validate_composition(&builtin(), &HSI_OVER_MONITOR, &BUILTIN_CRITICALITY_BANDS)
        .expect_err("the monitor's annunciation band is not coverable either");
    assert!(
        matches!(refusal, CompositionError::CriticalityObscured { .. }),
        "expected the criticality floor, got {refusal:?}"
    );
}

/// The floor is only meaningful if the band it reads was measured with
/// alerts fed, because a composed frame fans one `AlertOutput` to every
/// slot. This asserts the band actually contains the stack rather than
/// trusting that it does.
#[test]
fn the_pinned_bands_contain_the_alert_stack() {
    let stack = stack_ink_bound();
    for panel in ["pfd", "hsi", "monitor"] {
        let entry = BUILTIN_CRITICALITY_BANDS
            .entry(panel, PANEL_FRAME)
            .expect("every shipped panel pins a band");
        let band = entry.band.expect("every shipped panel paints alert ink");
        assert!(
            band.contains(&stack),
            "{panel}'s band {band:?} does not contain the alert stack {stack:?}"
        );
    }
}

/// Where the shared alert stack inks, measured from the rendered pixels
/// rather than from the symbology crate's layout constants.
fn stack_ink_bound() -> Region {
    let quiet = render_pfd(None);
    let alerted = render_pfd(Some(&one_warning()));
    let (w, h) = (PANEL_FRAME.width as usize, PANEL_FRAME.height as usize);
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (w, h, 0usize, 0usize);
    for y in 0..h {
        for x in 0..w {
            let at = (y * w + x) * 4;
            if quiet.get(at..at + 4) == alerted.get(at..at + 4) {
                continue;
            }
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x + 1);
            max_y = max_y.max(y + 1);
        }
    }
    assert!(min_x < max_x, "the alert made no difference to the frame");
    Region {
        x: min_x as f32,
        y: min_y as f32,
        width: (max_x - min_x) as f32,
        height: (max_y - min_y) as f32,
    }
}

fn render_pfd(alerts: Option<&AlertOutput>) -> Vec<u8> {
    const ONE: CompositionDescriptor = CompositionDescriptor {
        screen: PANEL_FRAME,
        slots: &[placed("pfd", 0.0, &[])],
    };
    let (w, h) = (PANEL_FRAME.width as u32, PANEL_FRAME.height as u32);
    let mut pixels = vec![0u8; (w * h * 4) as usize];
    let mut scratch = vec![0u8; MAX_SCENE_BYTES];
    let data = typical();
    let mut inputs = CompositionInputs {
        data: &data,
        alerts,
        scratch: &mut scratch,
    };
    render_composition(
        &builtin(),
        &ONE,
        &mut inputs,
        &mut pixels,
        FramebufferDims::tight(w, h),
        FrameId::default(),
    )
    .expect("renders");
    pixels
}

/// `screen` is used by the descriptors above through their literal
/// dimensions; keeping the helper honest about that avoids a dead
/// function warning if a fixture is reshaped.
#[test]
fn fixture_screens_are_sound() {
    assert!(Region::of(screen(480.0, 660.0)).is_sound());
}
