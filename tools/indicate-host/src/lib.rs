//! Reference host: the loop the backend contract describes and no crate
//! implemented (HOST-01, AIR-IN-013, AIR-UNAV-007).
//!
//! A shell that shows Indicate panels owes the same five things whatever
//! its platform: admit one snapshot at a time and advance the snapshot
//! generation once per admission; repaint on the display clock and
//! never on data arrival; resolve the snapshot once per paint and fan
//! one `PanelData` and one `AlertOutput` to every panel; watch its own
//! liveness so a renderer that stops making progress is annunciated
//! rather than retained; and paint a failure page in place of a frame
//! it could not make. This crate is those five things, once, so a
//! simulator user does not write the fail-safe loop again.
//!
//! [`Host`] is sans-IO and `no_std`: time enters as the caller's
//! millisecond clock, judged by the feeder's clock rule; scenes are
//! written into a caller-supplied buffer; nothing here touches a
//! surface, a socket, or a thread. The return path is routed here too:
//! an operator [`Request`] is admitted by the intent gate, alert
//! acknowledgements go to the alert manager, selections are held until
//! the source echoes them, and page or declutter requests are handed
//! back to the shell.
#![no_std]

#[cfg(test)]
extern crate std;

mod error;
mod failure;
mod host;
mod liveness;

pub use error::{HostError, RequestRefused};
pub use failure::failure_page;
pub use host::{Host, HostConfig, Painted, PaintedVolume, Routed};
pub use indicate_intent::{EchoStatus, EchoSummary, Intent, IntentStamp, Request};
pub use indicate_interaction::{
    InteractionFrame, InteractionOutcome, InteractionStamp, SpatialEvent, ViewLimits, ViewPose3D,
};
pub use liveness::Liveness;

#[cfg(test)]
mod tests;
