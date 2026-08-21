#![allow(clippy::expect_used, clippy::panic)]

use indicate_instrument_panels::{BUILTIN_CRITICALITY_BANDS, BUILTIN_PANELS};
use indicate_instrument_registry::{
    BackgroundCapability, DesignFrame, GroupSet, PanelDescriptor, Registry,
};

use super::{admit, criticality_bands};

#[test]
fn builtin_panels_pass_admission() {
    let registry = Registry::new(BUILTIN_PANELS).expect("composes");
    let report = admit(&registry).expect("shipped panels must be admissible");
    // PFD: (6 canonical + 4 extreme) states × (1 fed + 8 withheld);
    // HSI: (6 + 3) × 9; autoflight: (6 + 2) × 3; monitor: 7 × 2 — each
    // drawn twice, quiet and with the saturated alert stack.
    assert_eq!(report.cases, 418);
    // Every warning is the PFD's groundspeed or baro readout: each box
    // is 90 units wide but a wide value at size 16 has ~107 units of
    // nominal ink, so the run overhangs its box and the frame edge —
    // `status_paint::readout_box` paints at the size it is given. Real
    // display debt, honestly counted across every corpus and extreme
    // state; fixing it moves frame hashes and is its own change, for
    // both boxes at once.
    //
    // Thirty per PFD state that paints both boxes with wide values, and
    // sixteen for source-unusable, where only the groundspeed box
    // dashes — its baro box still paints a wide value, because a dialed
    // setting is not an estimate and does not fold source quality. The
    // true-airspeed box adds none of them: it sizes its label to its own
    // width, so a third readout arrives without a third overflow.
    // Twice the quiet-frame count, because the alert stack does not
    // touch these boxes: each overhangs on both sides of the alert axis.
    assert_eq!(report.warnings.len(), 256);
    assert!(report.warnings.iter().all(|w| matches!(
        w,
        super::AdmissionWarning::FrameOverflow { panel: "pfd", text, .. }
            if text.starts_with("GS ") || text.starts_with("SET ")
    )));
}

/// The pinned bands must be what the emitted scenes actually measure:
/// a composition validates obscuration against the pin, so a paint
/// change that moves a warning has to move the pin in the same change.
#[test]
fn the_pinned_criticality_bands_are_the_measured_ones() {
    let registry = Registry::new(BUILTIN_PANELS).expect("composes");
    let measured = criticality_bands(&registry).expect("measures");
    assert_eq!(measured, BUILTIN_CRITICALITY_BANDS.panels);
}

mod background_checks;
mod provenance_checks;
mod region_checks;

/// The one frame every fixture panel below declares: a degenerate
/// range, so each fixture is drawn exactly once per case and the counts
/// asserted above stay readable.
const FIXTURE_FRAME: DesignFrame = DesignFrame {
    width: 480.0,
    height: 360.0,
};

/// One-panel descriptor around a draw fixture, shared by the
/// background and provenance fixture suites.
fn opaque_panel(draw: indicate_instrument_registry::DrawFn) -> [PanelDescriptor; 1] {
    [PanelDescriptor {
        id: "probe",
        title: "Probe",
        required_layers: 1 << 2, // Tapes
        required_groups: GroupSet::EMPTY,
        frame_min: FIXTURE_FRAME,
        frame_max: FIXTURE_FRAME,
        frame_step: (1.0, 1.0),
        aspect_min: 1.30,
        aspect_max: 1.37,
        canonical_frames: &[FIXTURE_FRAME],
        background: BackgroundCapability::Opaque,
        config_schema: &[],
        group_regions: &[],
        extreme_states: &[],
        raster_baselines: &[],
        draw,
    }]
}

/// The configuration set is judged by the same harness as every other
/// set, over the shared canonical corpus and its own withholding
/// matrix. It ships outside `BUILTIN_PANELS`, so nothing else would
/// exercise it.
#[test]
fn the_config_set_passes_admission() {
    use indicate_instrument_panels::CONFIG_SET;
    use indicate_instrument_registry::PanelSet;

    static SETS: [&PanelSet; 1] = [&CONFIG_SET];
    let registry = Registry::from_sets(&SETS).expect("the config set composes");
    let report = admit(&registry).expect("the config panel must be admissible");
    // Six canonical states x (one fed case + one per required group
    // withheld) x (quiet, alerted); the panel declares no extreme state
    // of its own. It requires two groups: the configuration it draws,
    // and the trust its status folds.
    assert_eq!(report.cases, 36);
    // Nothing tolerated: every run's nominal ink sits inside the design
    // frame, so a first warning here would be a decision rather than a
    // drift.
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
}
