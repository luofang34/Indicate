//! Release-manifest generation (`gen-release-manifest`).
//!
//! A consumer taking a new revision has to re-pin what this repository
//! holds down, and reading that set out of source across several crates
//! is what makes an advance archaeology. The manifest answers it in one
//! committed file, generated from the definitions themselves so it
//! cannot disagree with them, and `scripts/check-release-manifest.sh`
//! regenerates and diffs it in CI so a pin edited without regenerating
//! is a red build.
//!
//! `CHANGELOG.md` keeps the human summary; this is the machine-readable
//! form, and it covers more values. They are not independent claims —
//! both are checked against the same tree.

mod corpus;
mod document;
mod json;
mod state_abi;

use std::path::PathBuf;

use crate::error::XtaskError;
use crate::output::print_line;
use crate::workspace::repo_root;

/// The committed manifest, relative to the workspace root.
const MANIFEST_PATH: &str = "release-manifest.json";

/// Writes the manifest, to `out` when the caller names one.
///
/// The guard passes a temporary path so it can diff the generated
/// document against the committed one; a generator that could only
/// overwrite in place would repair the drift it exists to report.
pub fn run(out: Option<PathBuf>) -> Result<(), XtaskError> {
    let root = repo_root();
    let content = document::render(&root)?;
    let path = out.unwrap_or_else(|| root.join(MANIFEST_PATH));
    std::fs::write(&path, &content).map_err(|source| XtaskError::File {
        action: "writing",
        path: path.display().to_string(),
        source,
    })?;
    print_line(&format!(
        "wrote {} ({} bytes)",
        path.display(),
        content.len()
    ));
    Ok(())
}

/// Parses `[--out <path>]`.
pub fn parse_args(args: &[String]) -> Result<Option<PathBuf>, XtaskError> {
    match args {
        [] => Ok(None),
        [flag, path] if flag == "--out" => Ok(Some(PathBuf::from(path))),
        _ => Err(XtaskError::Usage {
            message: format!(
                "gen-release-manifest takes an optional --out <path>, got {:?}",
                args.join(" ")
            ),
        }),
    }
}
