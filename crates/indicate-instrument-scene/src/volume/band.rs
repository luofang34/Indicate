//! Criticality-band validation for [`crate::volume`] scenes.

use crate::layer::{LayerError, LayerId, MAX_LAYER_COMMANDS, MAX_STACK_DEPTH};

use super::{MAX_VOLUME_SCENE_BYTES, VOLUME_BAND_COUNT, VolumeBandId, VolumeCmd, VolumeCmds};

/// Summary produced by [`validate_volume_bands`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct VolumeBandReport {
    /// Bit `i` (`0..=8`) is set when [`VolumeBandId`] `i` is present.
    pub present: u16,
    /// Command count per band (excluding `BeginBand` / `EndBand` markers).
    pub commands: [u16; VOLUME_BAND_COUNT],
    /// Byte span `(start, end)` of each present band's isolated body.
    pub ranges: [Option<(usize, usize)>; VOLUME_BAND_COUNT],
    /// Count of forward-compatible unknown opcodes skipped inside bands.
    pub unknown: u32,
}

impl VolumeBandReport {
    /// Whether `band` was emitted in this scene.
    #[must_use]
    pub const fn contains(&self, band: VolumeBandId) -> bool {
        (self.present & (1u16 << band.index())) != 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IsolationState {
    AwaitingSave,
    Active,
    Closed,
}

struct OpenBand {
    band: VolumeBandId,
    start: usize,
    depth: usize,
    isolation: IsolationState,
    commands: usize,
}

/// Validates an encoded 3D volume scene against the band-isolation contract.
pub fn validate_volume_bands(scene: &[u8]) -> Result<VolumeBandReport, LayerError> {
    if scene.len() > MAX_VOLUME_SCENE_BYTES {
        return Err(LayerError::SceneTooLarge { bytes: scene.len() });
    }
    let mut cmds = VolumeCmds::new(scene)?;
    let mut report = VolumeBandReport::default();
    let mut highest: Option<VolumeBandId> = None;
    let mut open: Option<OpenBand> = None;

    while cmds.remaining_len() > 0 {
        let before = scene.len() - cmds.remaining_len();
        let Some(item) = cmds.next() else {
            break;
        };
        let cmd = item?;
        let after = scene.len() - cmds.remaining_len();
        match cmd {
            VolumeCmd::BeginBand { band } => {
                begin_band(&mut open, &mut highest, &report, band, after)?;
            }
            VolumeCmd::EndBand { band } => {
                end_band(&mut open, &mut report, band, before)?;
            }
            other => {
                let state = open.as_mut().ok_or(LayerError::CommandOutsideLayer)?;
                step_command(state, &other, &mut report)?;
            }
        }
    }

    if let Some(left) = open {
        return Err(LayerError::UnclosedLayer {
            layer: map_band(left.band),
        });
    }
    Ok(report)
}

fn begin_band(
    open: &mut Option<OpenBand>,
    highest: &mut Option<VolumeBandId>,
    report: &VolumeBandReport,
    band: VolumeBandId,
    after: usize,
) -> Result<(), LayerError> {
    let mapped = map_band(band);
    if open.is_some() {
        return Err(LayerError::NestedLayer { layer: mapped });
    }
    if report.contains(band) {
        return Err(LayerError::DuplicateLayer { layer: mapped });
    }
    if let Some(prev) = *highest
        && band <= prev
    {
        return Err(LayerError::OutOfOrder { layer: mapped });
    }
    *highest = Some(band);
    *open = Some(OpenBand {
        band,
        start: after,
        depth: 0,
        isolation: IsolationState::AwaitingSave,
        commands: 0,
    });
    Ok(())
}

fn end_band(
    open: &mut Option<OpenBand>,
    report: &mut VolumeBandReport,
    band: VolumeBandId,
    before: usize,
) -> Result<(), LayerError> {
    let mapped_end = map_band(band);
    let Some(cur) = open.take() else {
        return Err(LayerError::EndWithoutBegin { layer: mapped_end });
    };
    let mapped_open = map_band(cur.band);
    if cur.band != band {
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
    let idx = band.index();
    report.present |= 1u16 << idx;
    report.commands[idx] =
        u16::try_from(cur.commands).map_err(|_| LayerError::OverCapacity { layer: mapped_open })?;
    report.ranges[idx] = Some((cur.start, before));
    Ok(())
}

fn step_command(
    state: &mut OpenBand,
    cmd: &VolumeCmd<'_>,
    report: &mut VolumeBandReport,
) -> Result<(), LayerError> {
    let mapped = map_band(state.band);
    state.commands = state.commands.wrapping_add(1);
    if state.commands > MAX_LAYER_COMMANDS {
        return Err(LayerError::OverCapacity { layer: mapped });
    }
    match cmd {
        VolumeCmd::Save => match state.isolation {
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
        VolumeCmd::Restore => match state.isolation {
            IsolationState::Active if state.depth > 1 => {
                state.depth -= 1;
            }
            IsolationState::Active if state.depth == 1 => {
                state.depth = 0;
                state.isolation = IsolationState::Closed;
            }
            _ => return Err(LayerError::UnbalancedState { layer: mapped }),
        },
        VolumeCmd::Unknown { .. } => {
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

const fn map_band(band: VolumeBandId) -> LayerId {
    match LayerId::from_u8(band.to_u8()) {
        Some(id) => id,
        None => LayerId::Failure,
    }
}
