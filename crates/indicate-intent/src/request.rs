//! What an operator can ask for, and the identity a request carries.

use indicate_alerts::AlertId;
use indicate_instrument_state::{
    AltitudeClass, GeoidModelId, HeadingReference, NavSource, OriginId,
};

use crate::namespace::{ExtensionPayload, IntentClass, IntentNamespace};

/// Lowest barometric setting a request may name, hectopascals. Below
/// the lowest sea-level pressure ever recorded, with margin.
pub const BARO_HPA_MIN: f32 = 800.0;
/// Highest barometric setting a request may name, hectopascals.
pub const BARO_HPA_MAX: f32 = 1100.0;
/// Lowest selectable altitude, meters. Below the lowest land surface,
/// with margin.
pub const SELECT_ALTITUDE_M_MIN: f32 = -1_000.0;
/// Highest selectable altitude, meters. Above any aircraft this family
/// draws for, with margin.
pub const SELECT_ALTITUDE_M_MAX: f32 = 30_000.0;

/// A display page an operator can select; meaning is the shell's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageId(pub u8);

/// The identity an input device stamps on each request.
///
/// `sequence` advances with `wrapping_add(1)` per request the device
/// issues, so a duplicate or reordered request is refused by serial
/// order rather than by value. `issued_at_ms` is on the device's own
/// clock and is carried, never compared with the host's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntentStamp {
    /// The input device: a bezel, a knob panel, a touch surface.
    pub device_id: u32,
    /// Wrapping per-device request sequence.
    pub sequence: u32,
    /// When the device issued the request, milliseconds on its clock.
    pub issued_at_ms: u64,
}

/// What the operator asked for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Intent {
    /// Set the barometric reference, hectopascals.
    SelectBaro {
        /// The requested setting.
        hpa: f32,
    },
    /// Move the heading bug, radians clockwise from the named reference.
    SetHeadingBug {
        /// The requested bug, `0 <= rad < 2π`.
        rad: f32,
        /// The reference the bug is expressed against.
        reference: HeadingReference,
    },
    /// Select an altitude against a declared datum.
    SelectAltitude {
        /// The requested altitude, meters.
        meters: f32,
        /// The altitude class the selection is expressed in.
        class: AltitudeClass,
        /// The origin identity for a local or relative class.
        origin: OriginId,
        /// The geoid model identity for a geometric class.
        model: GeoidModelId,
    },
    /// Select the lateral navigation source.
    SelectNavSource(NavSource),
    /// Acknowledge one alert; routed to the alert manager, never a source.
    AcknowledgeAlert(AlertId),
    /// Acknowledge every asserted alert (master caution or warning press).
    AcknowledgeAllAlerts,
    /// Show a display page; routed to the display, never a source.
    SelectPage(PageId),
    /// Request declutter on or off; routed to the display.
    Declutter(bool),
    /// Activate a numbered bezel or spatial softkey (`0x0009`).
    PressSoftkey {
        /// Softkey identifier.
        key_id: u16,
    },
    /// Domain-namespaced extension intent with bounded payload (`<= 64` bytes).
    Custom {
        /// Non-zero domain namespace.
        namespace: IntentNamespace,
        /// Domain-specific opcode.
        opcode: u16,
        /// Inline extension payload.
        payload: ExtensionPayload,
    },
}

impl Intent {
    /// Returns the [`IntentClass`] (`namespace`, `opcode`) of this intent.
    #[must_use]
    pub const fn class(&self) -> IntentClass {
        match *self {
            Self::SelectBaro { .. } => IntentClass::SELECT_BARO,
            Self::SetHeadingBug { .. } => IntentClass::SET_HEADING_BUG,
            Self::SelectAltitude { .. } => IntentClass::SELECT_ALTITUDE,
            Self::SelectNavSource(_) => IntentClass::SELECT_NAV_SOURCE,
            Self::AcknowledgeAlert(_) => IntentClass::ACKNOWLEDGE_ALERT,
            Self::AcknowledgeAllAlerts => IntentClass::ACKNOWLEDGE_ALL_ALERTS,
            Self::SelectPage(_) => IntentClass::SELECT_PAGE,
            Self::Declutter(_) => IntentClass::DECLUTTER,
            Self::PressSoftkey { .. } => IntentClass::PRESS_SOFTKEY,
            Self::Custom {
                namespace, opcode, ..
            } => IntentClass::custom(namespace, opcode),
        }
    }
}

/// One request: the device's stamp and what it asked for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Request {
    /// Who asked, and in what order.
    pub stamp: IntentStamp,
    /// What was asked.
    pub intent: Intent,
}
