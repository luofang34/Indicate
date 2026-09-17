//! X-Plane source adapter (XPLANE-01): the simulator's UDP data
//! interface into instrument state, and operator intents back out.
//!
//! X-Plane publishes datarefs over UDP on request (`RREF`) and accepts
//! dataref writes (`DREF`). This crate decodes and encodes those
//! packets without touching a socket ([`wire`]), names the datarefs a
//! flight display needs ([`datarefs`]), turns the latest values into an
//! [`AircraftState`] the reference host can admit ([`snapshot`]), and
//! maps an admitted operator intent to the dataref write that carries
//! it ([`intents`]). The binary in this package owns the socket, the
//! clock, and the frames it writes.
//!
//! The simulator is an untrusted source like any other. Attitude and
//! kinematics go through the feeder's avionics gate configured for the
//! simulation-truth role on the simulation clock, so they carry
//! provenance, serial ordering, and the clock rule; every other group
//! is aged from its own receipt and resolves `Missing` until every
//! constituent dataref has arrived. Nothing here paints a simulation
//! label: the shell owes that from the gate's provenance
//! (AIR-BAS-001, AIR-FLAG-007).
//!
//! Dataref names, units, and the packet layouts are taken from
//! X-Plane's documented UDP interface and were not verified against a
//! running simulator when this crate was written; [source-xplane.md]
//! lists each one so a mismatch is a table edit.
//!
//! [source-xplane.md]: ../../docs/instruments/source-xplane.md

pub mod datarefs;
pub mod error;
pub mod intents;
pub mod snapshot;
pub mod units;
pub mod wire;

pub use datarefs::{Dataref, SUBSCRIPTION_HZ};
pub use error::WireError;
pub use indicate_instrument_state::AircraftState;
pub use snapshot::XPlaneSource;
