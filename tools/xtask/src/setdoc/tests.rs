//! The splice keeps the author's prose and refuses a document without
//! the markers; the rendered block names what a descriptor declares.

#![allow(clippy::expect_used, clippy::panic)]

use std::path::Path;

use super::{BEGIN_MARKER, DOCUMENTED, END_MARKER, render_block, splice};
use crate::error::XtaskError;

#[test]
fn splice_replaces_only_the_marked_region() {
    let current =
        format!("# Title\n\nprose before\n\n{BEGIN_MARKER}\nold\n{END_MARKER}\n\nprose after\n");
    let out = splice(&current, "\nnew\n", Path::new("x.md")).expect("markers present");
    assert!(out.starts_with("# Title\n\nprose before\n\n"));
    assert!(out.ends_with("\n\nprose after\n"));
    assert!(out.contains(&format!("{BEGIN_MARKER}\n\nnew\n{END_MARKER}")));
    assert!(!out.contains("old"));
}

#[test]
fn a_document_without_markers_is_refused_not_rewritten() {
    let err = splice("# Title\n", "\nnew\n", Path::new("x.md")).expect_err("no markers");
    assert!(matches!(err, XtaskError::SetDocMarkers { .. }));
    let reversed = format!("{END_MARKER}\n{BEGIN_MARKER}\n");
    let err = splice(&reversed, "\nnew\n", Path::new("x.md")).expect_err("reversed markers");
    assert!(matches!(err, XtaskError::SetDocMarkers { .. }));
}

#[test]
fn the_rendered_block_states_every_declared_field_per_panel() {
    let panels = DOCUMENTED
        .iter()
        .find(|d| d.package == "indicate-instrument-panels")
        .expect("panels are documented");
    let block = render_block(panels);
    assert!(block.contains("Crate: `indicate-instrument-panels`"));
    assert!(block.contains("### Set `builtin`"));
    assert!(block.contains("#### Panel `pfd` — PFD"));
    for field in [
        "Required layers",
        "Required groups",
        "Frame range",
        "Canonical frames",
        "Background",
        "Config keys",
        "Group regions",
        "Extreme states",
        "Raster baselines",
    ] {
        assert!(block.contains(&format!("| {field} |")), "{field} missing");
    }
    assert!(block.contains("| Background | Cedeable |"));
    assert!(block.contains("`0x0001`"));
}

#[test]
fn every_set_crate_in_the_tree_has_a_documented_row() {
    let root = crate::workspace::repo_root();
    let mut crates: Vec<String> = std::fs::read_dir(root.join("sets"))
        .expect("sets/ exists")
        .filter_map(Result::ok)
        .filter(|entry| entry.path().join("Cargo.toml").is_file())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    crates.sort();
    let mut documented: Vec<String> = DOCUMENTED.iter().map(|d| d.package.to_string()).collect();
    documented.sort();
    assert_eq!(crates, documented, "a set crate has no row in DOCUMENTED");
}
