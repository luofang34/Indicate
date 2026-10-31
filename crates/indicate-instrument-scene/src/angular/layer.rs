//! Criticality-layer validation for [`crate::angular`] scenes.

use crate::layer::{LayerError, LayerId, MAX_LAYER_COMMANDS, MAX_STACK_DEPTH};

use super::{
    ANGULAR_LAYER_COUNT, AngularCmd, AngularCmds, AngularLayerId, MAX_ANGULAR_SCENE_BYTES,
};

/// Summary produced by [`validate_angular_layers`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AngularLayerReport {
    /// Bit `i` is set when [`AngularLayerId`] `i` is present.
    pub present: u8,
    /// Command count per layer (excluding `BeginLayer` / `EndLayer` markers).
    pub commands: [u16; ANGULAR_LAYER_COUNT],
    /// Byte span `(start, end)` of each present layer's isolated body.
    pub ranges: [Option<(usize, usize)>; ANGULAR_LAYER_COUNT],
    /// Count of forward-compatible unknown opcodes skipped inside layers.
    pub unknown: u32,
}

impl AngularLayerReport {
    /// Whether `layer` was emitted in this scene.
    #[must_use]
    pub const fn contains(&self, layer: AngularLayerId) -> bool {
        (self.present & (1u8 << layer.index())) != 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IsolationState {
    AwaitingSave,
    Active,
    Closed,
}

struct OpenLayer {
    layer: AngularLayerId,
    start: usize,
    depth: usize,
    isolation: IsolationState,
    commands: usize,
}

/// Validates an encoded angular scene against the layer-isolation contract.
pub fn validate_angular_layers(scene: &[u8]) -> Result<AngularLayerReport, LayerError> {
    if scene.len() > MAX_ANGULAR_SCENE_BYTES {
        return Err(LayerError::SceneTooLarge { bytes: scene.len() });
    }
    let mut cmds = AngularCmds::new(scene)?;
    let mut report = AngularLayerReport::default();
    let mut highest: Option<AngularLayerId> = None;
    let mut open: Option<OpenLayer> = None;

    while cmds.remaining_len() > 0 {
        let before = scene.len() - cmds.remaining_len();
        let Some(item) = cmds.next() else {
            break;
        };
        let cmd = item?;
        let after = scene.len() - cmds.remaining_len();
        match cmd {
            AngularCmd::BeginLayer { layer } => {
                begin_layer(&mut open, &mut highest, &report, layer, after)?;
            }
            AngularCmd::EndLayer { layer } => {
                end_layer(&mut open, &mut report, layer, before)?;
            }
            other => {
                let state = open.as_mut().ok_or(LayerError::CommandOutsideLayer)?;
                step_command(state, &other, &mut report)?;
            }
        }
    }

    if let Some(left) = open {
        return Err(LayerError::UnclosedLayer {
            layer: map_layer(left.layer),
        });
    }
    Ok(report)
}

fn begin_layer(
    open: &mut Option<OpenLayer>,
    highest: &mut Option<AngularLayerId>,
    report: &AngularLayerReport,
    layer: AngularLayerId,
    after: usize,
) -> Result<(), LayerError> {
    let mapped = map_layer(layer);
    if open.is_some() {
        return Err(LayerError::NestedLayer { layer: mapped });
    }
    if report.contains(layer) {
        return Err(LayerError::DuplicateLayer { layer: mapped });
    }
    if let Some(prev) = *highest
        && layer <= prev
    {
        return Err(LayerError::OutOfOrder { layer: mapped });
    }
    *highest = Some(layer);
    *open = Some(OpenLayer {
        layer,
        start: after,
        depth: 0,
        isolation: IsolationState::AwaitingSave,
        commands: 0,
    });
    Ok(())
}

fn end_layer(
    open: &mut Option<OpenLayer>,
    report: &mut AngularLayerReport,
    layer: AngularLayerId,
    before: usize,
) -> Result<(), LayerError> {
    let mapped_end = map_layer(layer);
    let Some(cur) = open.take() else {
        return Err(LayerError::EndWithoutBegin { layer: mapped_end });
    };
    let mapped_open = map_layer(cur.layer);
    if cur.layer != layer {
        return Err(LayerError::EndMismatch {
            open: mapped_open,
            end: mapped_end,
        });
    }
    match cur.isolation {
        IsolationState::Closed if cur.depth == 0 => {}
        IsolationState::AwaitingSave => {
            return Err(LayerError::UnisolatedState { layer: mapped_open });
        }
        _ => {
            return Err(LayerError::UnbalancedState { layer: mapped_open });
        }
    }
    let idx = layer.index();
    report.present |= 1u8 << idx;
    report.commands[idx] =
        u16::try_from(cur.commands).map_err(|_| LayerError::OverCapacity { layer: mapped_open })?;
    report.ranges[idx] = Some((cur.start, before));
    Ok(())
}

fn step_command(
    state: &mut OpenLayer,
    cmd: &AngularCmd<'_>,
    report: &mut AngularLayerReport,
) -> Result<(), LayerError> {
    let mapped = map_layer(state.layer);
    state.commands = state.commands.wrapping_add(1);
    if state.commands > MAX_LAYER_COMMANDS {
        return Err(LayerError::OverCapacity { layer: mapped });
    }
    match cmd {
        AngularCmd::Save => match state.isolation {
            IsolationState::AwaitingSave => {
                state.depth = 1;
                state.isolation = IsolationState::Active;
            }
            IsolationState::Active => {
                let next = state.depth.wrapping_add(1);
                if next > MAX_STACK_DEPTH {
                    return Err(LayerError::StackOverCapacity {
                        layer: mapped,
                        depth: next,
                    });
                }
                state.depth = next;
            }
            IsolationState::Closed => {
                return Err(LayerError::UnisolatedState { layer: mapped });
            }
        },
        AngularCmd::Restore => match state.isolation {
            IsolationState::Active if state.depth > 1 => {
                state.depth -= 1;
            }
            IsolationState::Active if state.depth == 1 => {
                state.depth = 0;
                state.isolation = IsolationState::Closed;
            }
            _ => return Err(LayerError::UnbalancedState { layer: mapped }),
        },
        AngularCmd::Unknown { .. } => {
            if state.isolation != IsolationState::Active {
                return Err(LayerError::UnisolatedState { layer: mapped });
            }
            report.unknown = report.unknown.wrapping_add(1);
        }
        _ => {
            if state.isolation != IsolationState::Active {
                return Err(LayerError::UnisolatedState { layer: mapped });
            }
        }
    }
    Ok(())
}

const fn map_layer(layer: AngularLayerId) -> LayerId {
    match LayerId::from_u8(layer.to_u8()) {
        Some(id) => id,
        None => LayerId::Failure,
    }
}
