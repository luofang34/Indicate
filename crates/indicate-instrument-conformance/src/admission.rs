//! The admission matrix and its check families.
//!
//! The matrix runs at every canonical frame a panel pins, and every
//! geometry check is expressed against the frame being drawn rather
//! than a descriptor constant — a panel that lays out differently at a
//! different size is judged at each size it declares.
//!
//! All geometry tests happen in DESIGN-FRAME space: text runs are
//! reduced to conservative ink rectangles (nominal metrics around the
//! anchor) and mapped through the scene's transform state, exactly as
//! a backend would place them, so a panel cannot move a run out of a
//! check's sight with a `translate`/`rotate` it already uses for
//! legitimate drawing.
//!
//! Honest status is a provenance rule, not a positional one: every
//! numeric run must claim the state group its value derives from
//! ([`Cmd::Attribute`]), and a claimed run may not be visible when its
//! group shows no value — wherever it is drawn. Declared
//! `group_regions` are a separate family with a separate purpose: each
//! declared readout surface must be one the group's readout really
//! uses, so a compositor above plans obscuration against a populated
//! declaration rather than an empty rectangle.

use indicate_alerts::AlertOutput;
use indicate_instrument_glyphs::PANEL_VOCABULARY;
use indicate_instrument_glyphs::typeface::FaceTable;
use indicate_instrument_registry::{
    CANONICAL_STATES, DesignFrame, DrawContext, EMPTY_CONFIG, PanelCriticality, PanelDescriptor,
    PanelDrawError, Registry, RenderSpace, SceneTarget,
};
use indicate_instrument_scene::{LayerError, MAX_SCENE_BYTES, SceneWriter, validate_layers};
use indicate_instrument_state::{
    AircraftState, EngineSample, EngineSamples, FreshnessPolicy, GroupId, PanelData, TankSample,
    TankSamples, resolve, withhold_group,
};

mod alerts;
mod angular;
mod background;
mod criticality;
mod error;
mod fixtures;
mod frame_varies;
mod geometry;
mod ink;
mod legend;
mod lettering;
mod provenance;
mod regions;
mod runs;
mod volume;

use alerts::saturated_stack;
use background::check_background;
use fixtures::check_fixture_engines;
use frame_varies::check_frame_varies;
use geometry::Rect;
use legend::{LegendWitness, check_legends};
use provenance::{ConfigClaims, check_provenance};
use regions::check_non_vacuity;
use runs::{RunsDefect, collect_runs};

pub use criticality::{criticality_bands, criticality_bands_with_faces};
pub use error::AdmissionError;

/// One admission run's outcome: how much was covered, and what was
/// tolerated. Failures are typed errors, never entries here.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AdmissionReport {
    /// Drawn and checked panel × state × withholding cases.
    pub cases: usize,
    /// Tolerated-but-counted observations.
    pub warnings: Vec<AdmissionWarning>,
    /// The measured criticality band of every panel × canonical frame,
    /// for a consumer to pin and a composition to plan around.
    pub criticality: Vec<PanelCriticality>,
}

/// A tolerated observation, counted so growth is visible.
#[derive(Debug, Clone, PartialEq)]
pub enum AdmissionWarning {
    /// A text run whose ink extends outside the frame it was drawn in
    /// without a bounding clip. The frame is part of the observation:
    /// a run that fits at one canonical size and overhangs at another
    /// is two different facts, and the ratchet counts them separately.
    FrameOverflow {
        /// The drawing panel.
        panel: &'static str,
        /// The corpus state.
        state: &'static str,
        /// The frame the panel was drawn at.
        frame: DesignFrame,
        /// Whether the alert stack was drawn in this case. A run that
        /// overhangs only with alerts fed is a different fact from one
        /// that overhangs on a quiet frame, and the ratchet counts them
        /// separately.
        alerted: bool,
        /// The text run's content.
        text: String,
    },
}

/// One decoded text run as a conservative design-space ink rectangle.
#[derive(Debug, Clone, PartialEq)]
struct TextRun {
    rect: Rect,
    text: String,
    /// The provenance claim prefixing the run, if any.
    attribution: Option<u8>,
    /// The clip in force when the run painted, if any.
    clip: Option<Rect>,
    /// Whether the ink rectangle intersects the active clip — a tape
    /// label scrolled past its strip's clip edge paints nothing.
    visible: bool,
}

impl TextRun {
    fn numeric(&self) -> bool {
        self.text.chars().any(|c| c.is_ascii_digit())
    }

    fn clipped(&self) -> bool {
        self.clip.is_some()
    }

    /// The ink that reaches the surface: the nominal run rectangle
    /// cropped by whatever clip was in force.
    fn painted_rect(&self) -> Rect {
        match self.clip {
            None => self.rect,
            Some(clip) => self.rect.intersect(&clip),
        }
    }
}

/// Runs the full admission matrix over `registry` with no faces bound:
/// every face slot letters in the reference face.
pub fn admit(registry: &Registry) -> Result<AdmissionReport, AdmissionError> {
    admit_with_faces(registry, &FaceTable::DEFAULT)
}

/// Runs the full admission matrix over `registry` with the host's face
/// binding, so a dial lettered in a face slot is judged in the face it
/// will be drawn in: frame overflow, printed-legend invariance, readout
/// regions and criticality bands all use that face's ink, and a run
/// the bound face cannot letter is refused.
pub fn admit_with_faces(
    registry: &Registry,
    faces: &FaceTable<'_>,
) -> Result<AdmissionReport, AdmissionError> {
    // Bands are measured before the judgements, so a report that
    // survives the matrix always carries them, and so the measurement
    // itself depends on nothing the matrix decides.
    let mut report = AdmissionReport {
        criticality: criticality_bands_with_faces(registry, faces)?,
        ..AdmissionReport::default()
    };
    for panel in registry.panels() {
        admit_panel(panel, &mut report, faces)?;
    }
    Ok(report)
}

fn admit_panel(
    panel: &'static PanelDescriptor,
    report: &mut AdmissionReport,
    faces: &FaceTable<'_>,
) -> Result<(), AdmissionError> {
    match panel.space {
        RenderSpace::Planar2D {
            canonical_frames, ..
        } => {
            check_fixture_engines(panel)?;
            for frame in canonical_frames {
                admit_panel_at_frame(panel, *frame, report, faces)?;
            }
            check_non_vacuity(panel, faces)?;
            check_frame_varies(panel)
        }
        RenderSpace::AngularSphere { overlay_frame } => {
            angular::admit_angular(panel, overlay_frame, report, faces)
        }
        RenderSpace::Volume3D {
            extent_min_m,
            extent_max_m,
            canonical_extents_m,
            ..
        } => volume::admit_volume(
            panel,
            extent_min_m,
            extent_max_m,
            canonical_extents_m,
            report,
        ),
    }
}

fn admit_panel_at_frame(
    panel: &'static PanelDescriptor,
    frame: DesignFrame,
    report: &mut AdmissionReport,
    faces: &FaceTable<'_>,
) -> Result<(), AdmissionError> {
    let mut legends = LegendWitness::default();
    for case in case_matrix(panel) {
        check_case(panel, &case, frame, report, &mut legends, faces)?;
    }
    Ok(())
}

/// One case of the admission matrix.
struct Case {
    state_id: &'static str,
    withheld: Option<GroupId>,
    state: AircraftState,
    /// The fixture's engine samples, beside the state as a host passes
    /// them. Withholding the engine group clears its header, so a
    /// panel that reads them only under the group's status sees none.
    engines: &'static [EngineSample],
    /// The fixture's fuel tank samples, on the same terms.
    tanks: &'static [TankSample],
    /// The alert state fed to the draw. `None` is the quiet frame; the
    /// saturated stack is the other end of the axis.
    alerts: Option<AlertOutput>,
}

impl Case {
    /// Whether alerts were fed, for the error and warning context: a
    /// defect that only appears with the stack drawn is a different
    /// fact from one that appears without it.
    fn alerted(&self) -> bool {
        self.alerts.is_some()
    }
}

/// Every case one panel is judged over: each canonical and extreme
/// state fully fed, then once per declared group with that group
/// withheld — and each of those twice, quiet and with the saturated
/// alert stack.
///
/// The alert axis is not decoration. A composed frame fans one
/// `AlertOutput` to every slot, so a band measured only on quiet frames
/// would omit the shared stack and licence covering warning rows.
fn case_matrix(panel: &'static PanelDescriptor) -> Vec<Case> {
    let states = CANONICAL_STATES
        .iter()
        .map(|s| (s.id, s.build, s.engines, s.tanks))
        .chain(
            panel
                .extreme_states
                .iter()
                .map(|e| (e.id, e.build, e.engines, e.tanks)),
        );
    let saturated = saturated_stack();
    let mut cases = Vec::new();
    for (state_id, build, engines, tanks) in states {
        let mut withholdings = vec![(None, build())];
        for group in GroupId::ALL {
            if panel.required_groups.contains(group) {
                withholdings.push((Some(group), withhold_group(&build(), group)));
            }
        }
        for (withheld, state) in withholdings {
            for alerts in [None, Some(saturated)] {
                cases.push(Case {
                    state_id,
                    withheld,
                    state,
                    engines,
                    tanks,
                    alerts,
                });
            }
        }
    }
    cases
}

fn check_case(
    panel: &'static PanelDescriptor,
    case: &Case,
    frame: DesignFrame,
    report: &mut AdmissionReport,
    legends: &mut LegendWitness,
    faces: &FaceTable<'_>,
) -> Result<(), AdmissionError> {
    let state_id = case.state_id;
    let data = resolve(&case.state, &FreshnessPolicy::default());
    let runs = draw_runs(panel, case, &data, frame, faces)?;
    check_provenance(
        panel,
        state_id,
        case.withheld,
        &runs,
        ConfigClaims::InvariantAcrossCases,
    )?;
    check_legends(panel, state_id, &runs, legends)?;
    let bounds = Rect {
        min_x: 0.0,
        min_y: 0.0,
        max_x: frame.width,
        max_y: frame.height,
    };
    for run in &runs {
        for ch in run.text.chars() {
            if !PANEL_VOCABULARY.contains(&ch) {
                return Err(AdmissionError::GlyphCoverage {
                    panel: panel.id,
                    state: state_id,
                    ch,
                });
            }
        }
        if !bounds.contains(&run.rect) && !run.clipped() {
            report.warnings.push(AdmissionWarning::FrameOverflow {
                panel: panel.id,
                state: state_id,
                frame,
                alerted: case.alerted(),
                text: run.text.clone(),
            });
        }
    }
    report.cases += 1;
    Ok(())
}

fn draw_runs(
    panel: &'static PanelDescriptor,
    case: &Case,
    data: &PanelData,
    frame: DesignFrame,
    faces: &FaceTable<'_>,
) -> Result<Vec<TextRun>, AdmissionError> {
    let state_id = case.state_id;
    let mut buf = vec![0u8; MAX_SCENE_BYTES];
    let scene =
        draw_scene(panel, data, case, frame, &mut buf).map_err(|source| AdmissionError::Draw {
            panel: panel.id,
            state: state_id,
            withheld: case.withheld,
            alerted: case.alerted(),
            source,
        })?;
    let layers = validate_layers(scene).map_err(|error| match error {
        LayerError::Decode(_) => AdmissionError::Decode {
            panel: panel.id,
            state: state_id,
        },
        _ => AdmissionError::LayerContract {
            panel: panel.id,
            state: state_id,
            withheld: case.withheld,
            alerted: case.alerted(),
        },
    })?;
    let missing = panel.required_layers & !u16::from(layers.present);
    if missing != 0 {
        return Err(AdmissionError::MissingRequiredLayers {
            panel: panel.id,
            state: state_id,
            withheld: case.withheld,
            alerted: case.alerted(),
            missing,
        });
    }
    check_background(panel, state_id, frame, scene)?;
    collect_runs(scene, faces).map_err(|defect| defect.into_error(panel.id, state_id))
}

impl RunsDefect {
    fn into_error(self, panel: &'static str, state: &'static str) -> AdmissionError {
        match self {
            RunsDefect::Decode => AdmissionError::Decode { panel, state },
            RunsDefect::MisplacedClaim => AdmissionError::MisplacedClaim { panel, state },
            RunsDefect::Face { slot, source } => AdmissionError::FaceLettering {
                panel,
                state,
                slot,
                source,
            },
        }
    }
}

fn draw_scene<'b>(
    panel: &PanelDescriptor,
    data: &PanelData,
    case: &Case,
    frame: DesignFrame,
    buf: &'b mut [u8],
) -> Result<&'b [u8], PanelDrawError> {
    let mut writer = SceneWriter::new(buf)?;
    let ctx = DrawContext::new(data)
        .with_config(EMPTY_CONFIG)
        .with_alerts(case.alerts.as_ref())
        .with_engines(EngineSamples::Slice(case.engines))
        .with_tanks(TankSamples::Slice(case.tanks));
    (panel.draw)(
        &ctx,
        SceneTarget::Planar2D {
            frame,
            writer: &mut writer,
        },
    )?;
    let used = writer.finish();
    Ok(buf.get(..used).unwrap_or(&[]))
}

#[cfg(test)]
mod tests;
