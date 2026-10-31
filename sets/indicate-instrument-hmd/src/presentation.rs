//! Display choices stay separate from aircraft guidance and control state.
use indicate_alerts::{AlertClass, AlertOutput};
use indicate_instrument_descriptor::{AuxChannelId, PanelDrawError};
use indicate_instrument_scene::LayerId;
use indicate_instrument_state::{PanelData, Sig, SignalStatus, flight::presentation};

/// Fixed byte length of the compact [`DisplayContext`] wire payload on `AuxChannelId::HWD_DISPLAY`.
pub const HWD_DISPLAY_WIRE_BYTES: usize = 6;

/// The HWD's responsibility for flight information.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DisplayRole {
    /// Declutter must retain the flight reference.
    #[default]
    Primary,
    /// A confirmed visible flight display can supply duplicate flight information.
    Supplemental,
}

/// The operator's requested information task; this never commands aircraft systems.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DisplayTask {
    /// Flight and navigation information.
    #[default]
    Flight,
    /// Flight essentials with space reserved for mission status.
    Mission,
}

/// Operator-selected detail, independent of active guidance modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DetailLevel {
    /// Scales and secondary flight information.
    #[default]
    Normal,
    /// Flight essentials, targets, limits, modes, and alerts.
    Reduced,
}

/// The host's classified view region.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewRegion {
    /// No duplicate flight display is in view.
    #[default]
    Outside,
    /// The observer looks through a conventional HUD.
    Hud,
    /// The observer looks at cockpit instruments.
    Cockpit,
}

/// Per-frame host input, independent of the aircraft state ABI.
#[derive(Debug, Clone, Copy)]
pub struct DisplayContext {
    /// Responsibility for flight information.
    pub role: DisplayRole,
    /// Requested information task.
    pub task: DisplayTask,
    /// Requested detail level.
    pub detail: DetailLevel,
    /// Classified view region; head angle alone cannot prove display visibility.
    pub region: ViewRegion,
    /// True only while another readable, functioning flight display is visible.
    /// The host must invalidate this signal when visibility evidence expires.
    pub alternate_flight_display: Sig<bool>,
}

impl Default for DisplayContext {
    fn default() -> Self {
        Self {
            role: DisplayRole::Primary,
            task: DisplayTask::Flight,
            detail: DetailLevel::Normal,
            region: ViewRegion::Outside,
            alternate_flight_display: Sig::with_status(false, SignalStatus::Missing),
        }
    }
}

/// Effective choices for one frame; recovery and failures override optional declutter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresentationPlan {
    /// Unusual attitude takes priority over the requested task.
    pub recovery: bool,
    /// Duplicate flight geometry is suppressed; status and alerts remain visible.
    pub blank_flight: bool,
    /// Secondary flight detail is permitted.
    pub detailed: bool,
    /// The mission status reservation is available to a future task renderer.
    pub mission: bool,
}

/// Resolves declutter without changing aircraft state or inferring flight-display visibility.
pub fn presentation_plan(
    data: &PanelData,
    context: DisplayContext,
    alerts: Option<&AlertOutput>,
) -> PresentationPlan {
    let recovery = data.presentation.unusual;
    let urgent = alerts.is_some_and(|output| {
        output.health() != indicate_alerts::ManagerHealth::Nominal
            || output.active().iter().any(|alert| {
                !alert.inhibited && !alert.decluttered && alert.class >= AlertClass::Caution
            })
    });
    let alternate = context.alternate_flight_display;
    let blank_flight = !recovery
        && !urgent
        && flight_reference_valid(data)
        && context.role == DisplayRole::Supplemental
        && context.region != ViewRegion::Outside
        && alternate.status == SignalStatus::Valid
        && alternate.value;
    PresentationPlan {
        recovery,
        blank_flight,
        detailed: !recovery
            && context.detail == DetailLevel::Normal
            && context.task == DisplayTask::Flight,
        mission: !recovery
            && !urgent
            && flight_reference_valid(data)
            && context.task == DisplayTask::Mission,
    }
}

fn flight_reference_valid(data: &PanelData) -> bool {
    [
        presentation::current_speed(data).1,
        data.altitude.value_ft,
        data.roll_rad,
        data.pitch_rad,
        data.heading.value_rad,
        data.vsi_fpm,
    ]
    .into_iter()
    .all(crate::panel::live)
}

/// Projection frame for nonconformal instrument geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstrumentReference {
    /// Aircraft-relative forward panel.
    Aircraft,
    /// Observer direction with roll stabilization; measured attitude remains independent.
    HeadLevel,
    /// Display-fixed status, unaffected by head motion or panel switching.
    Head,
}

/// Keeps nonconformal instruments in one display frame to preserve text reservations.
/// World geometry from [`crate::directions`] uses its own earth reference.
pub const fn layer_reference(_layer: LayerId, _compact: bool) -> InstrumentReference {
    InstrumentReference::Head
}

impl DisplayContext {
    /// Encodes the compact 6-byte `AuxChannelId::HWD_DISPLAY` wire payload.
    #[must_use]
    pub const fn encode_aux(&self) -> [u8; HWD_DISPLAY_WIRE_BYTES] {
        let role = match self.role {
            DisplayRole::Primary => 0,
            DisplayRole::Supplemental => 1,
        };
        let task = match self.task {
            DisplayTask::Flight => 0,
            DisplayTask::Mission => 1,
        };
        let detail = match self.detail {
            DetailLevel::Normal => 0,
            DetailLevel::Reduced => 1,
        };
        let region = match self.region {
            ViewRegion::Outside => 0,
            ViewRegion::Hud => 1,
            ViewRegion::Cockpit => 2,
        };
        let status = match self.alternate_flight_display.status {
            SignalStatus::Valid => 0,
            SignalStatus::Degraded => 1,
            SignalStatus::Stale => 2,
            SignalStatus::Missing => 3,
            SignalStatus::Failed => 4,
        };
        let alt_val = if self.alternate_flight_display.value {
            1
        } else {
            0
        };
        [role, task, detail, region, status, alt_val]
    }

    /// Decodes a compact 6-byte `AuxChannelId::HWD_DISPLAY` wire payload fail-closed.
    pub fn decode_aux(bytes: &[u8]) -> Result<Self, PanelDrawError> {
        let bad = || PanelDrawError::InvalidAux(AuxChannelId::HWD_DISPLAY.0);
        if bytes.len() != HWD_DISPLAY_WIRE_BYTES {
            return Err(bad());
        }
        let role = match *bytes.first().ok_or_else(bad)? {
            0 => DisplayRole::Primary,
            1 => DisplayRole::Supplemental,
            _ => return Err(bad()),
        };
        let task = match *bytes.get(1).ok_or_else(bad)? {
            0 => DisplayTask::Flight,
            1 => DisplayTask::Mission,
            _ => return Err(bad()),
        };
        let detail = match *bytes.get(2).ok_or_else(bad)? {
            0 => DetailLevel::Normal,
            1 => DetailLevel::Reduced,
            _ => return Err(bad()),
        };
        let region = match *bytes.get(3).ok_or_else(bad)? {
            0 => ViewRegion::Outside,
            1 => ViewRegion::Hud,
            2 => ViewRegion::Cockpit,
            _ => return Err(bad()),
        };
        let status = match *bytes.get(4).ok_or_else(bad)? {
            0 => SignalStatus::Valid,
            1 => SignalStatus::Degraded,
            2 => SignalStatus::Stale,
            3 => SignalStatus::Missing,
            4 => SignalStatus::Failed,
            _ => return Err(bad()),
        };
        let alt_val = match *bytes.get(5).ok_or_else(bad)? {
            0 => false,
            1 => true,
            _ => return Err(bad()),
        };
        Ok(Self {
            role,
            task,
            detail,
            region,
            alternate_flight_display: Sig::with_status(alt_val, status),
        })
    }
}

#[cfg(test)]
mod recovery_tests;
#[cfg(test)]
mod tests;
