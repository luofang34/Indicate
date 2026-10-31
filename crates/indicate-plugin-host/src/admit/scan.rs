//! Scene command scanners for plugin glyph, provenance, underlay, and handle checks.

use indicate_frames::Quat;
use indicate_instrument_descriptor::{DesignFrame, GroupSet, UnderlayContract};
use indicate_instrument_glyphs::PANEL_VOCABULARY;
use indicate_instrument_scene::angular::{AngularCmd, AngularCmds};
use indicate_instrument_scene::volume::{
    Extent3D, VolumeBandId, VolumeCmd, VolumeCmds, VolumeUnderlayKind,
};
use indicate_instrument_scene::{ATTR_CONFIG, Cmd, LayerId, PaintMode, SceneCmds};
use indicate_instrument_state::GroupId;
use indicate_plugin_abi::PluginDescriptorWire;

use crate::error::PluginHostError;

struct ClaimedRun {
    text: String,
    attribution: Option<u8>,
    visible: bool,
}

#[derive(Clone, Copy)]
struct Tf3D {
    pos: [f32; 3],
    rot: Quat,
}

impl Tf3D {
    const ID: Self = Self {
        pos: [0.0, 0.0, 0.0],
        rot: Quat::IDENTITY,
    };

    fn map(self, p: [f32; 3]) -> [f32; 3] {
        let r = self
            .rot
            .rotate([f64::from(p[0]), f64::from(p[1]), f64::from(p[2])]);
        [
            self.pos[0] + r[0] as f32,
            self.pos[1] + r[1] as f32,
            self.pos[2] + r[2] as f32,
        ]
    }
}

pub(super) fn verify_planar_scene(
    desc: &PluginDescriptorWire,
    state: &'static str,
    withheld: Option<GroupId>,
    frame: DesignFrame,
    scene: &[u8],
) -> Result<(), PluginHostError> {
    let cmds = SceneCmds::new(scene).map_err(indicate_instrument_scene::LayerError::Decode)?;
    let mut runs = Vec::new();
    let mut pending: Option<u8> = None;
    let mut in_bg = false;
    let mut bg_painted = false;
    let mut bg_covered = false;
    let mut fill_alpha = 255u8;
    let mut alpha_stack = vec![255u8];

    for item in cmds {
        if pending.is_some() && !matches!(item, Ok(Cmd::Text { .. })) {
            return Err(PluginHostError::MisplacedClaim { state });
        }
        let cmd = item.map_err(indicate_instrument_scene::LayerError::Decode)?;
        match cmd {
            Cmd::BeginLayer {
                layer: LayerId::Background,
            } => in_bg = true,
            Cmd::EndLayer {
                layer: LayerId::Background,
            } => in_bg = false,
            Cmd::Save => alpha_stack.push(fill_alpha),
            Cmd::Restore => fill_alpha = alpha_stack.pop().unwrap_or(255),
            Cmd::FillColor { color } => fill_alpha = color.a,
            Cmd::Attribute { group } => pending = Some(group),
            Cmd::Text { text, .. } => runs.push(ClaimedRun {
                text: text.to_string(),
                attribution: pending.take(),
                visible: true,
            }),
            Cmd::Rect { mode, x, y, w, h } if in_bg => {
                bg_painted = true;
                if fill_alpha == 255
                    && matches!(mode, PaintMode::Fill | PaintMode::FillStroke)
                    && x <= 0.0
                    && y <= 0.0
                    && x + w >= frame.width
                    && y + h >= frame.height
                {
                    bg_covered = true;
                }
            }
            Cmd::Circle { .. }
            | Cmd::Arc { .. }
            | Cmd::Line { .. }
            | Cmd::Polyline { .. }
            | Cmd::Polygon { .. }
                if in_bg =>
            {
                bg_painted = true;
            }
            _ => {}
        }
    }
    if pending.is_some() {
        return Err(PluginHostError::MisplacedClaim { state });
    }
    check_2d_underlay(desc.underlay, state, bg_painted, bg_covered)?;
    check_runs(desc.required_groups, state, withheld, &runs)
}

pub(super) fn verify_angular_scene(
    desc: &PluginDescriptorWire,
    state: &'static str,
    withheld: Option<GroupId>,
    scene: &[u8],
) -> Result<(), PluginHostError> {
    let cmds = AngularCmds::new(scene).map_err(indicate_instrument_scene::LayerError::Decode)?;
    let mut runs = Vec::new();
    let mut pending: Option<u8> = None;
    for item in cmds {
        if pending.is_some() && !matches!(item, Ok(AngularCmd::AngularText { .. })) {
            return Err(PluginHostError::MisplacedClaim { state });
        }
        let cmd = item.map_err(indicate_instrument_scene::LayerError::Decode)?;
        match cmd {
            AngularCmd::Attribute { group } => pending = Some(group),
            AngularCmd::AngularText { text, .. } => runs.push(ClaimedRun {
                text: text.to_string(),
                attribution: pending.take(),
                visible: true,
            }),
            _ => {}
        }
    }
    if pending.is_some() {
        return Err(PluginHostError::MisplacedClaim { state });
    }
    check_runs(desc.required_groups, state, withheld, &runs)
}

pub(super) fn verify_volume_scene(
    desc: &PluginDescriptorWire,
    state: &'static str,
    withheld: Option<GroupId>,
    extent_m: Extent3D,
    scene: &[u8],
) -> Result<(), PluginHostError> {
    let cmds = VolumeCmds::new(scene).map_err(indicate_instrument_scene::LayerError::Decode)?;
    let mut runs = Vec::new();
    let mut handles = Vec::new();
    let mut anchors = Vec::new();
    let mut surface_painted = false;
    let mut pending: Option<u8> = None;
    let mut band: Option<VolumeBandId> = None;
    let mut stack = vec![Tf3D::ID];

    for item in cmds {
        if pending.is_some() && !matches!(item, Ok(VolumeCmd::BillboardText { .. })) {
            return Err(PluginHostError::MisplacedClaim { state });
        }
        let cmd = item.map_err(indicate_instrument_scene::LayerError::Decode)?;
        step_vol_cmd(
            &cmd,
            &mut band,
            &mut pending,
            &mut stack,
            &mut runs,
            &mut handles,
            &mut anchors,
            &mut surface_painted,
        );
    }
    if pending.is_some() {
        return Err(PluginHostError::MisplacedClaim { state });
    }
    check_3d_underlay(desc.underlay, state, &anchors, surface_painted)?;
    let (hw, hh, hd) = (
        extent_m.width_m * 0.5 + 1.0e-4,
        extent_m.height_m * 0.5 + 1.0e-4,
        extent_m.depth_m * 0.5 + 1.0e-4,
    );
    for (target_id, center) in handles {
        if center[0].abs() > hw || center[1].abs() > hh || center[2].abs() > hd {
            return Err(PluginHostError::HandleOutOfBounds {
                state,
                target_id,
                center,
            });
        }
    }
    check_runs(desc.required_groups, state, withheld, &runs)
}

#[allow(clippy::too_many_arguments)]
fn step_vol_cmd(
    cmd: &VolumeCmd<'_>,
    band: &mut Option<VolumeBandId>,
    pending: &mut Option<u8>,
    stack: &mut Vec<Tf3D>,
    runs: &mut Vec<ClaimedRun>,
    handles: &mut Vec<(u32, [f32; 3])>,
    anchors: &mut Vec<(Option<VolumeBandId>, VolumeUnderlayKind)>,
    surface_painted: &mut bool,
) {
    match *cmd {
        VolumeCmd::BeginBand { band: b } => *band = Some(b),
        VolumeCmd::EndBand { .. } => *band = None,
        VolumeCmd::Save => stack.push(stack.last().copied().unwrap_or(Tf3D::ID)),
        VolumeCmd::Restore => {
            stack.pop();
            if stack.is_empty() {
                stack.push(Tf3D::ID);
            }
        }
        VolumeCmd::Translate3D { delta } => {
            if let Some(tf) = stack.last_mut() {
                tf.pos = tf.map(delta);
            }
        }
        VolumeCmd::RotateQuat { rotation } => {
            if let Some(tf) = stack.last_mut() {
                tf.rot = tf.rot.compose(rotation);
            }
        }
        VolumeCmd::Attribute { group } => *pending = Some(group),
        VolumeCmd::BillboardText { text, .. } => runs.push(ClaimedRun {
            text: text.to_string(),
            attribution: pending.take(),
            visible: true,
        }),
        VolumeCmd::TargetHandle {
            target_id, center, ..
        } => {
            let tf = stack.last().copied().unwrap_or(Tf3D::ID);
            handles.push((target_id, tf.map(center)));
        }
        VolumeCmd::UnderlayAnchor3D { kind, .. } => {
            anchors.push((*band, kind));
            *surface_painted |= *band == Some(VolumeBandId::SurfaceUnderlay);
        }
        VolumeCmd::Line3D { .. }
        | VolumeCmd::Polyline3D { .. }
        | VolumeCmd::Sphere { .. }
        | VolumeCmd::Box3D { .. }
        | VolumeCmd::Cylinder3D { .. }
        | VolumeCmd::Ring3D { .. }
        | VolumeCmd::HeightfieldGrid { .. } => {
            *surface_painted |= *band == Some(VolumeBandId::SurfaceUnderlay);
        }
        VolumeCmd::ClipBox3D { .. }
        | VolumeCmd::FillColor { .. }
        | VolumeCmd::Stroke3D { .. }
        | VolumeCmd::Unknown { .. } => {}
    }
}

fn check_2d_underlay(
    underlay: UnderlayContract,
    state: &'static str,
    painted: bool,
    covered: bool,
) -> Result<(), PluginHostError> {
    match underlay {
        UnderlayContract::None if painted => Err(PluginHostError::UnderlayViolation {
            state,
            detail: "UnderlayContract::None painted Background layer",
        }),
        UnderlayContract::Opaque
        | UnderlayContract::Map2D
        | UnderlayContract::ConformalSvs3D
        | UnderlayContract::CalibratedVideo
        | UnderlayContract::CombinedXvs
            if !covered =>
        {
            Err(PluginHostError::UnderlayViolation {
                state,
                detail: "owned 2D underlay did not opaquely cover Background layer",
            })
        }
        _ => Ok(()),
    }
}

fn check_3d_underlay(
    underlay: UnderlayContract,
    state: &'static str,
    anchors: &[(Option<VolumeBandId>, VolumeUnderlayKind)],
    surface_painted: bool,
) -> Result<(), PluginHostError> {
    let want = match underlay {
        UnderlayContract::None => {
            if !anchors.is_empty() || surface_painted {
                return Err(PluginHostError::UnderlayViolation {
                    state,
                    detail: "UnderlayContract::None emitted 3D surface underlay",
                });
            }
            return Ok(());
        }
        UnderlayContract::VolumetricTerrain3D => VolumeUnderlayKind::TerrainPatch,
        UnderlayContract::PlanetaryGlobe3D => VolumeUnderlayKind::PlanetaryGlobe,
        UnderlayContract::CelestialSystem3D => VolumeUnderlayKind::CelestialSystem,
        _ => return Ok(()),
    };
    let ok = !anchors.is_empty()
        && anchors
            .iter()
            .all(|(b, k)| *b == Some(VolumeBandId::SurfaceUnderlay) && *k == want);
    if !ok {
        return Err(PluginHostError::UnderlayViolation {
            state,
            detail: "missing or mismatched UNDERLAY_ANCHOR_3D in SurfaceUnderlay band",
        });
    }
    Ok(())
}

fn check_runs(
    required_groups: GroupSet,
    state: &'static str,
    withheld: Option<GroupId>,
    runs: &[ClaimedRun],
) -> Result<(), PluginHostError> {
    for run in runs {
        for ch in run.text.chars() {
            if !PANEL_VOCABULARY.contains(&ch) {
                return Err(PluginHostError::GlyphCoverage { state, ch });
            }
        }
        let Some(tag) = run.attribution else {
            if run.text.chars().any(|c| c.is_ascii_digit()) {
                return Err(PluginHostError::UntaggedNumeral {
                    state,
                    text: run.text.clone(),
                });
            }
            continue;
        };
        if !run.visible {
            continue;
        }
        if tag == ATTR_CONFIG {
            return Err(PluginHostError::InvalidProvenanceClaim {
                state,
                tag,
                text: run.text.clone(),
            });
        }
        let Some(group) = GroupId::from_u8(tag).filter(|g| required_groups.contains(*g)) else {
            return Err(PluginHostError::InvalidProvenanceClaim {
                state,
                tag,
                text: run.text.clone(),
            });
        };
        if withheld == Some(group) {
            return Err(PluginHostError::FabricatedNumeral {
                state,
                group,
                text: run.text.clone(),
            });
        }
    }
    Ok(())
}
