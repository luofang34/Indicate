//! The required-panel-profiles table in the layer-protocol document is
//! prose a backend author trusts, so it is checked against the shipped
//! descriptors rather than maintained beside them.

#![allow(clippy::expect_used, clippy::panic)]

use std::format;
use std::string::String;
use std::vec::Vec;

use pilotage_instrument_registry::PanelDescriptor;
use pilotage_instrument_scene::{LAYER_COUNT, LayerId};

use super::{BUILTIN_PANELS, layer_bit};

const PROTOCOL_DOC: &str = include_str!("../../../../docs/instruments/scene-layer-protocol.md");

const SECTION: &str = "## Required panel profiles";

/// Every defined layer, ascending — the order both table columns list.
fn defined_layers() -> Vec<LayerId> {
    (0..LAYER_COUNT as u8)
        .map(|id| LayerId::from_u8(id).expect("ids below LAYER_COUNT are defined"))
        .collect()
}

/// The two cells a descriptor's mask implies: the required layers, and
/// its complement over the defined layers.
fn cells_from_mask(panel: &PanelDescriptor) -> (String, String) {
    let (required, optional): (Vec<LayerId>, Vec<LayerId>) = defined_layers()
        .into_iter()
        .partition(|layer| panel.required_layers & layer_bit(*layer) != 0);
    (backticked(&required), backticked(&optional))
}

fn backticked(layers: &[LayerId]) -> String {
    layers
        .iter()
        .map(|layer| format!("`{layer:?}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The table's data rows as (panel, required, optional).
fn documented_rows() -> Vec<(String, String, String)> {
    let section = PROTOCOL_DOC
        .split_once(SECTION)
        .expect("the layer-protocol document still has a required-profiles section")
        .1;
    let mut rows: Vec<(String, String, String)> = Vec::new();
    for line in section.lines() {
        let line = line.trim();
        if !line.starts_with('|') {
            // The profiles table is one contiguous block. Stopping at its
            // end keeps a later table in the document from being read as
            // a profile row.
            if rows.is_empty() {
                continue;
            }
            break;
        }
        let cells: Vec<&str> = line.trim_matches('|').split('|').map(str::trim).collect();
        assert_eq!(
            cells.len(),
            3,
            "a profiles row must carry panel, required, and optional: {line}"
        );
        let is_header = cells[0] == "Panel";
        let is_rule = cells.iter().all(|cell| cell.chars().all(|c| c == '-'));
        if is_header || is_rule {
            continue;
        }
        rows.push((
            String::from(cells[0]),
            String::from(cells[1]),
            String::from(cells[2]),
        ));
    }
    rows
}

#[test]
fn every_shipped_panel_has_a_row_in_shell_order() {
    let documented: Vec<String> = documented_rows().into_iter().map(|row| row.0).collect();
    let shipped: Vec<String> = BUILTIN_PANELS
        .iter()
        .map(|panel| String::from(panel.title))
        .collect();
    assert_eq!(
        documented, shipped,
        "the profiles table must list every shipped panel, in shell display order"
    );
}

#[test]
fn the_documented_profiles_are_the_descriptor_masks() {
    let rows = documented_rows();
    // Without this, `zip` would truncate to the shorter side and a table
    // missing its last panel would pass by describing nothing.
    assert_eq!(
        rows.len(),
        BUILTIN_PANELS.len(),
        "every shipped panel needs a row before its mask can be checked"
    );
    for (panel, row) in BUILTIN_PANELS.iter().zip(rows) {
        let (required, optional) = cells_from_mask(panel);
        assert_eq!(
            row.1, required,
            "{}: documented required layers disagree with required_layers",
            panel.title
        );
        assert_eq!(
            row.2, optional,
            "{}: documented optional layers are not the complement of required_layers",
            panel.title
        );
    }
}

/// The guard is only worth its line count if a drifting mask breaks it.
#[test]
fn a_drifting_mask_is_caught() {
    let mut drifted = *BUILTIN_PANELS
        .first()
        .expect("the family ships at least one panel");
    drifted.required_layers &= !layer_bit(LayerId::Annunciation);
    let (required, _) = cells_from_mask(&drifted);
    let documented = documented_rows();
    assert_ne!(
        required, documented[0].1,
        "dropping a required layer must change the cell this test compares"
    );
}
