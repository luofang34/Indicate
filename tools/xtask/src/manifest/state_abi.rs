//! The state-ABI pin, and the guard that this generator reads the
//! module the crate actually ships.
//!
//! The ABI in force is the highest version module
//! `indicate-instrument-state` declares — the same rule
//! `scripts/check-release-markers.sh` follows, matched no more loosely
//! than that script matches it. A Rust program cannot
//! enumerate the modules of a crate it links, so it names one and the
//! guard here refuses to emit a manifest once a newer module exists:
//! adding `v8` fails the generator instead of silently pinning `v7`
//! while the changelog moves on.

#[cfg(test)]
mod tests;

use std::path::Path;

use indicate_instrument_state::abi::v7::VERSION;

use crate::error::XtaskError;

/// The module this generator reads [`VERSION`] from.
const COMPILED_MODULE: u32 = 7;

/// Where the crate declares its ABI modules, relative to the workspace root.
const MODULE_SOURCE: &str = "crates/indicate-instrument-state/src/abi.rs";

/// The ABI module in force and the version constant it declares.
pub struct StateAbi {
    /// The module name, `v7` and so on.
    pub module: String,
    /// The wire version that module declares.
    pub version: u8,
}

/// Reads the ABI pin, refusing when the crate has outgrown this generator.
pub fn read(root: &Path) -> Result<StateAbi, XtaskError> {
    let path = root.join(MODULE_SOURCE);
    let source = std::fs::read_to_string(&path).map_err(|source| XtaskError::File {
        action: "reading",
        path: path.display().to_string(),
        source,
    })?;
    let newest = newest_module(&source).ok_or_else(|| XtaskError::UnpinnableValue {
        value: "the state ABI version",
        reason: format!("{MODULE_SOURCE} declares no `pub mod vN;`"),
    })?;
    if newest != COMPILED_MODULE {
        return Err(XtaskError::AbiModuleDrift {
            newest: format!("v{newest}"),
            compiled: format!("v{COMPILED_MODULE}"),
        });
    }
    Ok(StateAbi {
        module: format!("v{newest}"),
        version: VERSION,
    })
}

/// The highest `pub mod vN;` the source declares, or `None` when it
/// declares no versioned module at all.
///
/// The declaration is matched at its head and read up to the
/// semicolon, so anything trailing it — a comment, whitespace — cannot
/// hide the module. Requiring the line to *end* at the semicolon would
/// make this weaker than the shell rule it mirrors, and weaker in the
/// one direction that matters: a declaration nobody sees is a manifest
/// certifying an ABI the tree no longer ships.
fn newest_module(source: &str) -> Option<u32> {
    source
        .lines()
        .filter_map(|line| {
            let rest = line.trim_start().strip_prefix("pub mod v")?;
            let digits = rest.split(';').next()?;
            if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            digits.parse::<u32>().ok()
        })
        .max()
}
