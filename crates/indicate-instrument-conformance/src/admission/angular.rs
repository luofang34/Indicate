//! Admission matrix for `RenderSpace::AngularSphere` instruments (VOL-01).

use indicate_instrument_glyphs::PANEL_VOCABULARY;
use indicate_instrument_registry::{
    DesignFrame, DrawContext, EMPTY_CONFIG, PanelDescriptor, SceneTarget,
};
use indicate_instrument_scene::angular::{
    AngularCmd, AngularCmds, AngularSceneWriter, MAX_ANGULAR_SCENE_BYTES, validate_angular_layers,
};
use indicate_instrument_scene::{LayerError, MAX_SCENE_BYTES, SceneWriter, validate_layers};
use indicate_instrument_state::{FreshnessPolicy, resolve};

use super::background::check_background;
use super::error::AdmissionError;
use super::geometry::Rect;
use super::provenance::check_provenance;
use super::{
    AdmissionReport, AdmissionWarning, Case, RunsDefect, TextRun, case_matrix, collect_runs,
};

pub(super) fn admit_angular(
    panel: &'static PanelDescriptor,
    overlay_frame: Option<DesignFrame>,
    report: &mut AdmissionReport,
) -> Result<(), AdmissionError> {
    let mut ang_buf = vec![0u8; MAX_ANGULAR_SCENE_BYTES];
    let mut ov_buf = vec![0u8; MAX_SCENE_BYTES];
    for case in case_matrix(panel) {
        check_angular_case(
            panel,
            &case,
            overlay_frame,
            &mut ang_buf,
            &mut ov_buf,
            report,
        )?;
    }
    Ok(())
}

fn check_angular_case(
    panel: &'static PanelDescriptor,
    case: &Case,
    overlay_frame: Option<DesignFrame>,
    ang_buf: &mut [u8],
    ov_buf: &mut [u8],
    report: &mut AdmissionReport,
) -> Result<(), AdmissionError> {
    let data = resolve(&case.state, &FreshnessPolicy::default());
    let ctx = DrawContext::new(&data)
        .with_config(EMPTY_CONFIG)
        .with_alerts(case.alerts.as_ref());
    let (ang_used, ov_used) = draw_angular_pair(panel, case, &ctx, overlay_frame, ang_buf, ov_buf)?;
    let ang_bytes = ang_buf.get(..ang_used).unwrap_or(&[]);
    verify_angular_stream(panel, case, ang_bytes)?;
    if let Some(frame) = overlay_frame {
        let ov_bytes = ov_buf.get(..ov_used).unwrap_or(&[]);
        verify_overlay_stream(panel, case, frame, ov_bytes, report)?;
    }
    report.cases += 1;
    Ok(())
}

fn draw_angular_pair(
    panel: &'static PanelDescriptor,
    case: &Case,
    ctx: &DrawContext<'_>,
    overlay_frame: Option<DesignFrame>,
    ang_buf: &mut [u8],
    ov_buf: &mut [u8],
) -> Result<(usize, usize), AdmissionError> {
    let mut ang_writer = AngularSceneWriter::new(ang_buf).map_err(|e| AdmissionError::Draw {
        panel: panel.id,
        state: case.state_id,
        withheld: case.withheld,
        alerted: case.alerted(),
        source: e.into(),
    })?;
    let ov_used = if let Some(frame) = overlay_frame {
        let mut ov_writer = SceneWriter::new(ov_buf).map_err(|e| AdmissionError::Draw {
            panel: panel.id,
            state: case.state_id,
            withheld: case.withheld,
            alerted: case.alerted(),
            source: e.into(),
        })?;
        (panel.draw)(
            ctx,
            SceneTarget::AngularSphere {
                angular: &mut ang_writer,
                overlay: Some((frame, &mut ov_writer)),
            },
        )
        .map_err(|source| AdmissionError::Draw {
            panel: panel.id,
            state: case.state_id,
            withheld: case.withheld,
            alerted: case.alerted(),
            source,
        })?;
        ov_writer.finish()
    } else {
        (panel.draw)(
            ctx,
            SceneTarget::AngularSphere {
                angular: &mut ang_writer,
                overlay: None,
            },
        )
        .map_err(|source| AdmissionError::Draw {
            panel: panel.id,
            state: case.state_id,
            withheld: case.withheld,
            alerted: case.alerted(),
            source,
        })?;
        0
    };
    Ok((ang_writer.finish(), ov_used))
}

fn verify_angular_stream(
    panel: &'static PanelDescriptor,
    case: &Case,
    ang_bytes: &[u8],
) -> Result<(), AdmissionError> {
    let state_id = case.state_id;
    let layers = validate_angular_layers(ang_bytes).map_err(|error| match error {
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
    let missing = (panel.required_layers & 0x00FF) & !u16::from(layers.present);
    if missing != 0 {
        return Err(AdmissionError::MissingRequiredLayers {
            panel: panel.id,
            state: state_id,
            withheld: case.withheld,
            alerted: case.alerted(),
            missing,
        });
    }
    let runs = collect_angular_runs(ang_bytes).map_err(|defect| match defect {
        RunsDefect::Decode => AdmissionError::Decode {
            panel: panel.id,
            state: state_id,
        },
        RunsDefect::MisplacedClaim => AdmissionError::MisplacedClaim {
            panel: panel.id,
            state: state_id,
        },
    })?;
    check_provenance(panel, state_id, case.withheld, &runs)?;
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
    }
    Ok(())
}

fn verify_overlay_stream(
    panel: &'static PanelDescriptor,
    case: &Case,
    frame: DesignFrame,
    ov_bytes: &[u8],
    report: &mut AdmissionReport,
) -> Result<(), AdmissionError> {
    let state_id = case.state_id;
    let layers = validate_layers(ov_bytes).map_err(|error| match error {
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
    let req_ov = (panel.required_layers >> 8) as u8;
    let missing_ov = req_ov & !layers.present;
    if missing_ov != 0 {
        return Err(AdmissionError::MissingRequiredLayers {
            panel: panel.id,
            state: state_id,
            withheld: case.withheld,
            alerted: case.alerted(),
            missing: u16::from(missing_ov) << 8,
        });
    }
    check_background(panel, state_id, frame, ov_bytes)?;
    let runs = collect_runs(ov_bytes).map_err(|defect| match defect {
        RunsDefect::Decode => AdmissionError::Decode {
            panel: panel.id,
            state: state_id,
        },
        RunsDefect::MisplacedClaim => AdmissionError::MisplacedClaim {
            panel: panel.id,
            state: state_id,
        },
    })?;
    check_provenance(panel, state_id, case.withheld, &runs)?;
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
    Ok(())
}

fn collect_angular_runs(scene: &[u8]) -> Result<Vec<TextRun>, RunsDefect> {
    let cmds = AngularCmds::new(scene).map_err(|_| RunsDefect::Decode)?;
    let mut runs = Vec::new();
    let mut pending: Option<u8> = None;
    let zero_rect = Rect {
        min_x: 0.0,
        min_y: 0.0,
        max_x: 0.0,
        max_y: 0.0,
    };
    for cmd in cmds {
        if pending.is_some() && !matches!(cmd, Ok(AngularCmd::AngularText { .. })) {
            return Err(RunsDefect::MisplacedClaim);
        }
        match cmd {
            Ok(AngularCmd::Attribute { group }) => pending = Some(group),
            Ok(AngularCmd::AngularText { text, .. }) => {
                runs.push(TextRun {
                    rect: zero_rect,
                    text: text.to_string(),
                    attribution: pending.take(),
                    clip: None,
                    visible: true,
                });
            }
            Ok(_) => {}
            Err(_) => return Err(RunsDefect::Decode),
        }
    }
    if pending.is_some() {
        return Err(RunsDefect::MisplacedClaim);
    }
    Ok(runs)
}
