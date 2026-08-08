//! The bench's shareable half: the fixture screen composition and the
//! screen-composition digest pinned over it.
//!
//! The binary in this package composes the screen and refuses to run
//! when the digest it computes is not the pinned one. The
//! release-manifest generator records the same pin. Both read the
//! constants here rather than each holding a copy, so the manifest can
//! only ever state the screen the bench actually checks.

mod screen;

pub use screen::{BENCH_COMPOSITION, BENCH_COMPOSITION_DIGEST, BENCH_SCREEN};
