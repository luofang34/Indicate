//! The state-ABI pin, and the guard that this generator reads the
//! module the crate actually ships.
//!
//! The ABI in force is the highest version module
//! `indicate-instrument-state` declares — the rule
//! `scripts/check-release-markers.sh` follows. A Rust program cannot
//! enumerate the modules of a crate it links, so it names one and the
//! guard here refuses to emit a manifest once a newer module exists:
//! adding `v7` fails the generator instead of silently pinning `v6`
//! while the changelog moves on.

#[cfg(test)]
mod tests;

use std::path::Path;

use indicate_instrument_state::abi::v6::VERSION;

use crate::error::XtaskError;

/// The module this generator reads [`VERSION`] from.
const COMPILED_MODULE: u32 = 6;

/// Where the crate declares its ABI modules, relative to the workspace root.
const MODULE_SOURCE: &str = "crates/indicate-instrument-state/src/abi.rs";

/// The ABI module in force and the version constant it declares.
pub struct StateAbi {
    /// The module name, `v6` and so on.
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
fn newest_module(source: &str) -> Option<u32> {
    source
        .lines()
        .filter_map(|line| {
            let digits = line.trim().strip_prefix("pub mod v")?.strip_suffix(';')?;
            digits.parse::<u32>().ok()
        })
        .max()
}
