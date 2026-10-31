//! Typed operator intents: the return path from a human controller to
//! the system (INT-01, AIR-IN-007).
//!
//! Every shipped surface is display-only: the selections group (heading
//! bug, altitude select, barometric setting) is fed *from* the source.
//! This crate is the other direction. An operator's knob, key, or touch
//! becomes a typed [`Intent`] carried in a [`Request`] with the input
//! device's identity, sequence, and clock. [`IntentGate`] admits a
//! request the way the feeder admits a measurement: the device is
//! pinned, the sequence must advance in wrapping serial order, the host
//! clock must be one the gate can trust, the value must be finite and
//! inside the declared bounds, and a reference the state cannot name
//! (`Unknown`) is refused.
//!
//! The rule that keeps the channel honest is echo-back. **An intent is
//! a request, never a value.** The display paints only what the source
//! reports back through the selections group; nothing here writes
//! state. [`EchoMonitor`] holds each admitted selection request until
//! the state echoes it, and tells the shell whether the request is
//! pending, confirmed, overdue, or unmeasurable, so a request the
//! source has not honoured is annunciated rather than assumed.
//!
//! Alert acknowledgements, page selection, and declutter are admitted
//! here and routed by the host to the alert manager or the display; they
//! never reach a source and never command an aircraft. The crate is
//! `no_std`, allocation-free, and sans-IO: a simulator adapter turns an
//! admitted request into its own write (a dataref, an event, a command)
//! outside this crate.
#![no_std]

mod echo;
mod error;
mod gate;
mod namespace;
mod request;
mod wire;

pub use echo::{EchoMonitor, EchoStatus, SELECTION_KINDS, SelectionKind};
pub use error::IntentFault;
pub use gate::{Admitted, IntentGate};
pub use namespace::{
    ExtensionPayload, IntentClass, IntentNamespace, MAX_EXTENSION_PAYLOAD, core_opcode,
    namespace_id,
};
pub use request::{
    BARO_HPA_MAX, BARO_HPA_MIN, Intent, IntentStamp, PageId, Request, SELECT_ALTITUDE_M_MAX,
    SELECT_ALTITUDE_M_MIN,
};
pub use wire::{
    ECHO_SUMMARY_BYTES, ECHO_SUMMARY_VERSION, EchoSummary, INTENT_WIRE_VERSION, IntentWireError,
    MAX_INTENT_WIRE_BYTES, decode_echo_summary, decode_request, encode_echo_summary,
    encode_echo_summary_slice, encode_request,
};
