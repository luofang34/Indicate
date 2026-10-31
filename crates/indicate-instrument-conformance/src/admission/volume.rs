//! Admission matrix for `RenderSpace::Volume3D` instruments (VOL-01).

use indicate_frames::Quat;
use indicate_instrument_glyphs::PANEL_VOCABULARY;
use indicate_instrument_registry::{
    DrawContext, EMPTY_CONFIG, PanelDescriptor, SceneTarget, UnderlayContract,
};
use indicate_instrument_scene::LayerError;
use indicate_instrument_scene::volume::{
    Extent3D, MAX_VOLUME_SCENE_BYTES, VolumeBandId, VolumeCmd, VolumeCmds, VolumeSceneWriter,
    VolumeUnderlayKind, validate_volume_bands,
};
use indicate_instrument_state::{FreshnessPolicy, resolve};

use super::error::AdmissionError;
use super::geometry::Rect;
use super::provenance::check_provenance;
use super::{AdmissionReport, Case, RunsDefect, TextRun, case_matrix};

#[derive(Clone, Copy)]
struct Transform3D {
    pos: [f32; 3],
    rot: Quat,
    clip: Option<([f32; 3], [f32; 3])>,
}

impl Transform3D {
    const IDENTITY: Self = Self {
        pos: [0.0, 0.0, 0.0],
        rot: Quat::IDENTITY,
        clip: None,
    };

    fn map_point(self, local: [f32; 3]) -> [f32; 3] {
        let r = self.rot.rotate([
            f64::from(local[0]),
            f64::from(local[1]),
            f64::from(local[2]),
        ]);
        [
            self.pos[0] + r[0] as f32,
            self.pos[1] + r[1] as f32,
            self.pos[2] + r[2] as f32,
        ]
    }

    fn point_visible(self, world: [f32; 3]) -> bool {
        let Some((min, max)) = self.clip else {
            return true;
        };
        world[0] >= min[0]
            && world[0] <= max[0]
            && world[1] >= min[1]
            && world[1] <= max[1]
            && world[2] >= min[2]
            && world[2] <= max[2]
    }

    fn push_clip(&mut self, min: [f32; 3], max: [f32; 3]) {
        let p0 = self.map_point(min);
        let p1 = self.map_point(max);
        let lo = [p0[0].min(p1[0]), p0[1].min(p1[1]), p0[2].min(p1[2])];
        let hi = [p0[0].max(p1[0]), p0[1].max(p1[1]), p0[2].max(p1[2])];
        self.clip = Some(match self.clip {
            None => (lo, hi),
            Some((a, b)) => (
                [a[0].max(lo[0]), a[1].max(lo[1]), a[2].max(lo[2])],
                [b[0].min(hi[0]), b[1].min(hi[1]), b[2].min(hi[2])],
            ),
        });
    }
}

#[derive(Default)]
struct VolumeScan {
    runs: Vec<TextRun>,
    handles: Vec<(u32, [f32; 3])>,
    anchors: Vec<(Option<VolumeBandId>, VolumeUnderlayKind)>,
    painted_surface_band: bool,
}

pub(super) fn admit_volume(
    panel: &'static PanelDescriptor,
    min: Extent3D,
    max: Extent3D,
    canonical: &'static [Extent3D],
    report: &mut AdmissionReport,
) -> Result<(), AdmissionError> {
    let mut buf = vec![0u8; MAX_VOLUME_SCENE_BYTES];
    for extent_m in canonical {
        for case in case_matrix(panel) {
            check_volume_case(panel, &case, *extent_m, &mut buf, report)?;
        }
    }
    check_extent_varies(panel, min, max)
}

fn check_volume_case(
    panel: &'static PanelDescriptor,
    case: &Case,
    extent_m: Extent3D,
    buf: &mut [u8],
    report: &mut AdmissionReport,
) -> Result<(), AdmissionError> {
    let state_id = case.state_id;
    let scene = draw_volume_scene(panel, case, extent_m, buf)?;
    let bands = validate_volume_bands(scene).map_err(|error| match error {
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
    let missing = panel.required_layers & !bands.present;
    if missing != 0 {
        return Err(AdmissionError::MissingRequiredLayers {
            panel: panel.id,
            state: state_id,
            withheld: case.withheld,
            alerted: case.alerted(),
            missing,
        });
    }
    let scan = scan_volume_scene(scene).map_err(|defect| match defect {
        RunsDefect::Decode => AdmissionError::Decode {
            panel: panel.id,
            state: state_id,
        },
        RunsDefect::MisplacedClaim => AdmissionError::MisplacedClaim {
            panel: panel.id,
            state: state_id,
        },
    })?;
    check_volume_underlay(panel, state_id, &scan)?;
    check_handles_in_bounds(panel, state_id, extent_m, &scan.handles)?;
    check_provenance(panel, state_id, case.withheld, &scan.runs)?;
    for run in &scan.runs {
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
    report.cases += 1;
    Ok(())
}

fn draw_volume_scene<'b>(
    panel: &'static PanelDescriptor,
    case: &Case,
    extent_m: Extent3D,
    buf: &'b mut [u8],
) -> Result<&'b [u8], AdmissionError> {
    let data = resolve(&case.state, &FreshnessPolicy::default());
    let ctx = DrawContext::new(&data)
        .with_config(EMPTY_CONFIG)
        .with_alerts(case.alerts.as_ref());
    let mut writer = VolumeSceneWriter::new(buf).map_err(|e| AdmissionError::Draw {
        panel: panel.id,
        state: case.state_id,
        withheld: case.withheld,
        alerted: case.alerted(),
        source: e.into(),
    })?;
    (panel.draw)(
        &ctx,
        SceneTarget::Volume3D {
            extent_m,
            writer: &mut writer,
        },
    )
    .map_err(|source| AdmissionError::Draw {
        panel: panel.id,
        state: case.state_id,
        withheld: case.withheld,
        alerted: case.alerted(),
        source,
    })?;
    let used = writer.finish();
    Ok(buf.get(..used).unwrap_or(&[]))
}

fn scan_volume_scene(scene: &[u8]) -> Result<VolumeScan, RunsDefect> {
    let cmds = VolumeCmds::new(scene).map_err(|_| RunsDefect::Decode)?;
    let mut scan = VolumeScan::default();
    let mut pending: Option<u8> = None;
    let mut active_band: Option<VolumeBandId> = None;
    let mut stack = vec![Transform3D::IDENTITY];
    for cmd in cmds {
        if pending.is_some() && !matches!(cmd, Ok(VolumeCmd::BillboardText { .. })) {
            return Err(RunsDefect::MisplacedClaim);
        }
        let cmd = cmd.map_err(|_| RunsDefect::Decode)?;
        apply_volume_cmd(&cmd, &mut active_band, &mut pending, &mut stack, &mut scan)?;
    }
    if pending.is_some() {
        return Err(RunsDefect::MisplacedClaim);
    }
    Ok(scan)
}

fn apply_volume_cmd(
    cmd: &VolumeCmd<'_>,
    active_band: &mut Option<VolumeBandId>,
    pending: &mut Option<u8>,
    stack: &mut Vec<Transform3D>,
    scan: &mut VolumeScan,
) -> Result<(), RunsDefect> {
    match *cmd {
        VolumeCmd::BeginBand { band } => *active_band = Some(band),
        VolumeCmd::EndBand { .. } => *active_band = None,
        VolumeCmd::Save => stack.push(stack.last().copied().ok_or(RunsDefect::Decode)?),
        VolumeCmd::Restore => {
            stack.pop();
            if stack.is_empty() {
                stack.push(Transform3D::IDENTITY);
            }
        }
        VolumeCmd::Translate3D { delta } => {
            if let Some(tf) = stack.last_mut() {
                tf.pos = tf.map_point(delta);
            }
        }
        VolumeCmd::RotateQuat { rotation } => {
            if let Some(tf) = stack.last_mut() {
                tf.rot = tf.rot.compose(rotation);
            }
        }
        VolumeCmd::ClipBox3D { min, max } => {
            if let Some(tf) = stack.last_mut() {
                tf.push_clip(min, max);
            }
        }
        VolumeCmd::Attribute { group } => *pending = Some(group),
        VolumeCmd::BillboardText { pos, text, .. } => {
            let tf = stack.last().copied().ok_or(RunsDefect::Decode)?;
            let world = tf.map_point(pos);
            let zero = Rect {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 0.0,
                max_y: 0.0,
            };
            scan.runs.push(TextRun {
                rect: zero,
                text: text.to_string(),
                attribution: pending.take(),
                clip: None,
                visible: tf.point_visible(world),
            });
        }
        VolumeCmd::TargetHandle {
            target_id, center, ..
        } => {
            let tf = stack.last().copied().ok_or(RunsDefect::Decode)?;
            scan.handles.push((target_id, tf.map_point(center)));
        }
        VolumeCmd::UnderlayAnchor3D { kind, .. } => {
            scan.anchors.push((*active_band, kind));
            scan.painted_surface_band |= *active_band == Some(VolumeBandId::SurfaceUnderlay);
        }
        VolumeCmd::Line3D { .. }
        | VolumeCmd::Polyline3D { .. }
        | VolumeCmd::Sphere { .. }
        | VolumeCmd::Box3D { .. }
        | VolumeCmd::Cylinder3D { .. }
        | VolumeCmd::Ring3D { .. }
        | VolumeCmd::HeightfieldGrid { .. } => {
            scan.painted_surface_band |= *active_band == Some(VolumeBandId::SurfaceUnderlay);
        }
        VolumeCmd::FillColor { .. } | VolumeCmd::Stroke3D { .. } | VolumeCmd::Unknown { .. } => {}
    }
    Ok(())
}

fn check_volume_underlay(
    panel: &'static PanelDescriptor,
    state_id: &'static str,
    scan: &VolumeScan,
) -> Result<(), AdmissionError> {
    let expected = match panel.underlay {
        UnderlayContract::None => {
            if !scan.anchors.is_empty() || scan.painted_surface_band {
                return Err(AdmissionError::BackgroundContract {
                    panel: panel.id,
                    state: state_id,
                    declared: "None",
                    defect: "paints surface underlay or emits UNDERLAY_ANCHOR_3D",
                });
            }
            return Ok(());
        }
        UnderlayContract::VolumetricTerrain3D => {
            ("VolumetricTerrain3D", VolumeUnderlayKind::TerrainPatch)
        }
        UnderlayContract::PlanetaryGlobe3D => {
            ("PlanetaryGlobe3D", VolumeUnderlayKind::PlanetaryGlobe)
        }
        UnderlayContract::CelestialSystem3D => {
            ("CelestialSystem3D", VolumeUnderlayKind::CelestialSystem)
        }
        _ => return Ok(()),
    };
    let ok = !scan.anchors.is_empty()
        && scan.anchors.iter().all(|(band, kind)| {
            *band == Some(VolumeBandId::SurfaceUnderlay) && *kind == expected.1
        });
    if !ok {
        return Err(AdmissionError::BackgroundContract {
            panel: panel.id,
            state: state_id,
            declared: expected.0,
            defect: "missing or mismatched UNDERLAY_ANCHOR_3D in SurfaceUnderlay band",
        });
    }
    Ok(())
}

fn check_handles_in_bounds(
    panel: &'static PanelDescriptor,
    state_id: &'static str,
    extent_m: Extent3D,
    handles: &[(u32, [f32; 3])],
) -> Result<(), AdmissionError> {
    let (hw, hh, hd) = (
        extent_m.width_m * 0.5 + 1e-4,
        extent_m.height_m * 0.5 + 1e-4,
        extent_m.depth_m * 0.5 + 1e-4,
    );
    for &(target_id, center) in handles {
        if center[0].abs() > hw || center[1].abs() > hh || center[2].abs() > hd {
            return Err(AdmissionError::HandleOutOfBounds {
                panel: panel.id,
                state: state_id,
                target_id,
                center,
                extent_m,
            });
        }
    }
    Ok(())
}

fn check_extent_varies(
    panel: &'static PanelDescriptor,
    min: Extent3D,
    max: Extent3D,
) -> Result<(), AdmissionError> {
    if min == max {
        return Ok(());
    }
    let mut first = vec![0u8; MAX_VOLUME_SCENE_BYTES];
    let mut large = vec![0u8; MAX_VOLUME_SCENE_BYTES];
    let mut again = vec![0u8; MAX_VOLUME_SCENE_BYTES];
    for case in case_matrix(panel) {
        let at_min = draw_volume_scene(panel, &case, min, &mut first)?.to_vec();
        let at_max = draw_volume_scene(panel, &case, max, &mut large)?;
        if at_min == at_max {
            continue;
        }
        let reproduced = draw_volume_scene(panel, &case, min, &mut again)?;
        if at_min == reproduced {
            return Ok(());
        }
    }
    Err(AdmissionError::ExtentIgnored {
        panel: panel.id,
        min,
        max,
    })
}
