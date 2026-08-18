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
    // PFD: (5 canonical + 3 extreme) states × (1 fed + 8 withheld);
    // HSI: (5 + 2) × 8; monitor: 6 × 2 — each drawn twice, quiet and
    // with the saturated alert stack.
    assert_eq!(report.cases, 280);
    // Every warning is one of the PFD's three `readout_box` values —
    // true airspeed, groundspeed, baro: each box is 90 units wide but a
    // wide value at size 16 has ~107 units of nominal ink, so the run
    // overhangs its box and the frame edge (status_paint::readout_box
    // draws at the requested size with no fit shrink, unlike the
    // pointed readouts, which fit). Real display debt, honestly counted
    // across every corpus and extreme state; fixing the paint moves
    // frame hashes and is its own change, for all three at once. The
    // ratchet makes any NEW unclipped off-frame text a deliberate
    // decision, and this count grows for two such decisions: a third
    // box of that shape, and a fifth canonical state that exercises all
    // three.
    // Twice the quiet-frame count, because the alert stack does not
    // touch these boxes: each overhangs on both sides of the alert axis.
    assert_eq!(report.warnings.len(), 266);
    assert!(report.warnings.iter().all(|w| matches!(
        w,
        super::AdmissionWarning::FrameOverflow { panel: "pfd", text, .. }
            if text.starts_with("TAS ") || text.starts_with("GS ") || text.starts_with("SET ")
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
