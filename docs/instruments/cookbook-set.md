# Cookbook: write an instrument set

A set is a crate under `sets/` that exports at least one `PanelSet`.
The [panel contract](panel-contract.md) is binding; the template set is
the worked example; this page is the order of work.

## Steps

1. **Copy the template.** `sets/indicate-instrument-template` is the
   smallest set that passes admission. Rename the crate, the descriptor,
   and the set id.
2. **Declare honestly.** `required_groups` is the withholding matrix:
   list every group a readout folds into its status. `required_layers`
   is what you open on every frame. The frame range is what you lay out
   against; a degenerate range is honest when the geometry is fixed.
3. **Draw from resolved state only.** Every numeral goes through
   `text_attributed` with its group; every dash goes through `text`.
   Failure colours come from `indicate-instrument-symbology::safety`.
   Characters come from the controlled vocabulary.
4. **Paint warnings in the criticality bands.** Only `Annunciation` and
   `Failure` ink is protected from obscuration when the panel is
   composed.
5. **Admit yourself.** Your crate's test composes the set with
   `Registry::from_sets`, runs `admit`, and pins the out-of-frame ink
   count. Run `cargo test -p <your crate>` before opening a change.
6. **Write the set document.** Create `docs/instruments/set-<name>.md`
   with the intended function, inputs, failure presentation,
   verification, and the requirements the set answers to, ending with
   the two descriptor-facts markers. Run `cargo xtask gen-set-docs` to
   fill the block. `scripts/check-set-docs.sh` fails CI without it.
7. **Register the crate.** Workspace members and dependencies in the
   root `Cargo.toml`, a row in `crates/README.md`, the CI allowlist and
   bare-metal closure, and `scripts/check-structure.sh`'s tier list.
   `scripts/check-ci-closure.sh` and `check-structure.sh` name what is
   missing.
8. **Add a row to the function matrix.** Every display feature cites a
   requirement ([`AIR-BAS-005`](requirements.md#air-bas-005)); a new
   surface gets a row in [intended-functions.md](intended-functions.md).

## What admission proves and what it does not

It proves the layer contract, the background claim, glyph coverage, the
provenance rules, populated regions, and frame variation across the
whole case matrix. It does not prove that a failure cue is painted at
all, that a stale value is not painted with its claim, or anything
about configuration or alerts. Cover those with your own tests.
