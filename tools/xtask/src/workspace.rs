//! Where the workspace root is, for commands that read and write
//! tracked files.
//!
//! The root is derived from this crate's manifest directory rather than
//! from the current working directory, so `cargo xtask` reads and writes
//! the same files whichever subdirectory it is invoked from.

use std::path::{Path, PathBuf};

/// The workspace root: this crate's manifest directory is
/// `<root>/tools/xtask`, so the root is its second ancestor.
///
/// Walking ancestors rather than joining `..` keeps the path free of
/// relative segments, which is what a written path is reported as. Cargo
/// sets the manifest directory to an absolute path, and a path with two
/// components still has a second ancestor, so the fallback stands only
/// for a shape the variable cannot take.
pub fn repo_root() -> PathBuf {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .ancestors()
        .nth(2)
        .unwrap_or(manifest_dir)
        .to_path_buf()
}
