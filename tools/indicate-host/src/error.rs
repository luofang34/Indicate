//! Why the host could not make a frame or route a request. A paint
//! error means no frame was made: the render generation does not
//! advance, and the shell paints the failure page.

use indicate_instrument_descriptor::PanelDrawError;
use indicate_instrument_scene::{LayerError, SceneError};
use indicate_intent::IntentFault;

/// Why a paint produced no frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum HostError {
    /// The caller's clock reading could not be trusted: not a number,
    /// negative, or behind the last reading the host acted on.
    #[error("host clock reading could not be trusted")]
    InvalidClock,
    /// The scene buffer could not hold a writer.
    #[error("scene buffer refused: {0:?}")]
    Scene(SceneError),
    /// The panel refused to draw.
    #[error("panel refused to draw: {0}")]
    Draw(PanelDrawError),
    /// The emitted scene failed the layer contract.
    #[error("scene failed the layer contract: {0}")]
    Layers(LayerError),
    /// The scene omits a layer the panel declares it always emits.
    #[error("scene omits required layers (mask {missing:#04x})")]
    MissingLayers {
        /// The required-layer bits that were absent.
        missing: u8,
    },
}

/// Why an operator request was not routed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RequestRefused {
    /// The intent gate refused it.
    #[error("intent refused: {0}")]
    Intent(#[from] IntentFault),
    /// The alert-event queue is full; the acknowledgement would be lost,
    /// so it is refused rather than dropped silently.
    #[error("alert event queue is full")]
    QueueFull,
}
