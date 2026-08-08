//! Golden-frame generation for the state ABI (`gen-state-fixture`).
//!
//! Encodes the shared posture fixtures with the same Rust codec the
//! runtime uses and writes one lowercase-hex line per fixture into
//! `crates/indicate-instrument-state/fixtures/` — inside the crate that
//! owns the codec, so the fixtures travel with it. The Rust golden test
//! and downstream state writers (the Pilotage browser shell's
//! `state-abi.js` among them) pin against these committed files, so the
//! sides of that boundary can only drift by turning a consumer's CI red.

use indicate_instrument_state::AircraftState;
use indicate_instrument_state::abi::v7::{CAPACITY, encode_state, fixtures};

use crate::error::XtaskError;
use crate::output::print_line;
use crate::workspace::repo_root;

/// Builds one posture fixture.
type FixtureBuilder = fn() -> AircraftState;

/// The committed fixtures: stable file stem and the state behind it.
const FIXTURES: [(&str, FixtureBuilder); 3] = [
    ("state-abi-v7.full", fixtures::full),
    ("state-abi-v7.data-gateway", fixtures::data_gateway),
    (
        "state-abi-v7.flight-controller",
        fixtures::flight_controller,
    ),
];

fn hex_of(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    for byte in bytes {
        // Writing to a String cannot fail; ignore the fmt plumbing.
        write!(out, "{byte:02x}").ok();
    }
    out
}

/// Writes every golden frame, printing each path and byte count.
pub fn run() -> Result<(), XtaskError> {
    let dir = repo_root()
        .join("crates")
        .join("indicate-instrument-state")
        .join("fixtures");
    std::fs::create_dir_all(&dir).map_err(|source| XtaskError::Io {
        context: "creating crates/indicate-instrument-state/fixtures",
        source,
    })?;
    for (stem, build) in FIXTURES {
        let state = build();
        let mut buf = [0u8; CAPACITY];
        let len = encode_state(&state, &mut buf).map_err(|error| XtaskError::Usage {
            message: format!("encoding fixture {stem}: {error}"),
        })?;
        let path = dir.join(format!("{stem}.hex"));
        let mut content = hex_of(&buf[..len]);
        content.push('\n');
        std::fs::write(&path, content).map_err(|source| XtaskError::Io {
            context: "writing a state-ABI golden frame",
            source,
        })?;
        print_line(&format!("wrote {} ({len} bytes)", path.display()));
    }
    Ok(())
}
