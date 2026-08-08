//! The corpus pin, read out of the artifact consumers pin.
//!
//! Every backend replays the shared conformance corpus and pins the
//! version and content hash the golden JSON declares. That file is the
//! source of truth — a consumer that vendors a copy vendors *this* file
//! — so the manifest quotes its header rather than a constant that
//! merely agrees with it today.

#[cfg(test)]
mod tests;

use std::path::Path;

use crate::error::XtaskError;

/// Where the corpus lives, relative to the workspace root.
pub const CORPUS_PATH: &str =
    "crates/indicate-instrument-scene/corpus/scene-conformance-corpus.json";

/// The corpus identity a backend pins.
pub struct Corpus {
    /// The version the corpus declares for itself.
    pub version: u32,
    /// SHA-256 over the concatenated replay bytes of every entry.
    pub sha256: String,
}

/// Reads the corpus pin, refusing a header that does not answer once
/// and unambiguously.
pub fn read(root: &Path) -> Result<Corpus, XtaskError> {
    let path = root.join(CORPUS_PATH);
    let source = std::fs::read_to_string(&path).map_err(|source| XtaskError::File {
        action: "reading",
        path: path.display().to_string(),
        source,
    })?;
    let version = header_value(&source, "corpusVersion", "the corpus version")?;
    let version = version
        .parse::<u32>()
        .map_err(|error| XtaskError::UnpinnableValue {
            value: "the corpus version",
            reason: format!("{CORPUS_PATH} states {version:?}, which is not a version: {error}"),
        })?;
    let sha256 = header_value(&source, "corpusSha256", "the corpus sha256")?;
    let sha256 = sha256
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .filter(|hex| hex.len() == 64 && hex.chars().all(|c| c.is_ascii_hexdigit()))
        .ok_or_else(|| XtaskError::UnpinnableValue {
            value: "the corpus sha256",
            reason: format!("{CORPUS_PATH} states {sha256:?}, which is not a 64-digit hex string"),
        })?;
    Ok(Corpus {
        version,
        sha256: sha256.to_string(),
    })
}

/// The header value for `key`, as it is written in the file.
///
/// Only the header is searched — the entry array below it holds
/// per-entry keys and a nested key with the same name would otherwise
/// answer for the document. A key that appears twice is refused rather
/// than resolved by position, because there is no honest way to pick.
fn header_value<'a>(
    source: &'a str,
    key: &str,
    value: &'static str,
) -> Result<&'a str, XtaskError> {
    let prefix = format!("  \"{key}\": ");
    let mut found: Option<&'a str> = None;
    for line in source.lines() {
        if line.starts_with("  \"entries\"") {
            break;
        }
        let Some(rest) = line.strip_prefix(&prefix) else {
            continue;
        };
        if found.is_some() {
            return Err(XtaskError::UnpinnableValue {
                value,
                reason: format!("{CORPUS_PATH} states \"{key}\" more than once in its header"),
            });
        }
        found = Some(rest.trim_end().trim_end_matches(','));
    }
    found.ok_or_else(|| XtaskError::UnpinnableValue {
        value,
        reason: format!("{CORPUS_PATH} has no \"{key}\" in its header"),
    })
}
