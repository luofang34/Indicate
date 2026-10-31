//! Per-frame draw context and its frozen v1 binary wire codec.

mod synth;
mod wire;

#[cfg(test)]
mod tests;

use indicate_alerts::AlertOutput;
use indicate_instrument_state::{AircraftState, PanelData};
use indicate_intent::EchoSummary;
use indicate_interaction::ViewPose3D;

use crate::aux::{AuxChannelId, AuxEntry};
use crate::config::{ConfigBlob, EMPTY_CONFIG};
use crate::space::UnderlayStatus;

pub use wire::{
    DRAW_CONTEXT_HEADER_BYTES, DRAW_CONTEXT_WIRE_VERSION, DecodedDrawContext, DrawContextWireError,
    decode_draw_context, encode_draw_context,
};

/// Immutable per-frame context delivered to every instrument draw entry point.
#[derive(Debug, Clone, Copy)]
pub struct DrawContext<'a> {
    data: &'a PanelData,
    raw_state: Option<&'a AircraftState>,
    config: ConfigBlob<'a>,
    alerts: Option<&'a AlertOutput>,
    echo: EchoSummary,
    view: ViewPose3D,
    underlay: UnderlayStatus,
    aux: &'a [AuxEntry<'a>],
}

impl<'a> DrawContext<'a> {
    /// Creates a minimal draw context around resolved [`PanelData`] with neutral defaults.
    #[must_use]
    pub const fn new(data: &'a PanelData) -> Self {
        Self {
            data,
            raw_state: None,
            config: EMPTY_CONFIG,
            alerts: None,
            echo: EchoSummary::IDLE,
            view: ViewPose3D::IDENTITY,
            underlay: UnderlayStatus::UNAVAILABLE,
            aux: &[],
        }
    }

    /// Attaches the raw [`AircraftState`] snapshot so wire serialization preserves exact inputs.
    #[must_use]
    pub const fn with_raw_state(mut self, state: &'a AircraftState) -> Self {
        self.raw_state = Some(state);
        self
    }

    /// Attaches a validated [`ConfigBlob`].
    #[must_use]
    pub const fn with_config(mut self, config: ConfigBlob<'a>) -> Self {
        self.config = config;
        self
    }

    /// Attaches optional central flightcrew [`AlertOutput`].
    #[must_use]
    pub const fn with_alerts(mut self, alerts: Option<&'a AlertOutput>) -> Self {
        self.alerts = alerts;
        self
    }

    /// Attaches the operator-intent [`EchoSummary`].
    #[must_use]
    pub const fn with_echo(mut self, echo: EchoSummary) -> Self {
        self.echo = echo;
        self
    }

    /// Attaches the bounded 3D [`ViewPose3D`].
    #[must_use]
    pub const fn with_view(mut self, view: ViewPose3D) -> Self {
        self.view = view;
        self
    }

    /// Attaches host [`UnderlayStatus`].
    #[must_use]
    pub const fn with_underlay(mut self, underlay: UnderlayStatus) -> Self {
        self.underlay = underlay;
        self
    }

    /// Attaches auxiliary domain channel payloads.
    #[must_use]
    pub const fn with_aux(mut self, aux: &'a [AuxEntry<'a>]) -> Self {
        self.aux = aux;
        self
    }

    /// Resolved flight and navigation [`PanelData`].
    #[must_use]
    pub const fn data(&self) -> &'a PanelData {
        self.data
    }

    /// Raw [`AircraftState`] if supplied by the host.
    #[must_use]
    pub const fn raw_state(&self) -> Option<&'a AircraftState> {
        self.raw_state
    }

    /// Validated configuration blob.
    #[must_use]
    pub const fn config(&self) -> &ConfigBlob<'a> {
        &self.config
    }

    /// Optional central alert manager output.
    #[must_use]
    pub const fn alerts(&self) -> Option<&'a AlertOutput> {
        self.alerts
    }

    /// Operator-intent echo summary.
    #[must_use]
    pub const fn echo(&self) -> &EchoSummary {
        &self.echo
    }

    /// Bounded 3D view pose.
    #[must_use]
    pub const fn view(&self) -> &ViewPose3D {
        &self.view
    }

    /// Host underlay status.
    #[must_use]
    pub const fn underlay(&self) -> UnderlayStatus {
        self.underlay
    }

    /// Looks up the payload bytes of auxiliary channel `id`, if present.
    #[must_use]
    pub fn aux(&self, id: AuxChannelId) -> Option<&'a [u8]> {
        for entry in self.aux {
            if entry.id == id {
                return Some(entry.payload);
            }
        }
        None
    }

    /// All auxiliary channel entries attached to this context.
    #[must_use]
    pub const fn aux_entries(&self) -> &'a [AuxEntry<'a>] {
        self.aux
    }
}
