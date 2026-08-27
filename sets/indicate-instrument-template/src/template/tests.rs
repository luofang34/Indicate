#![allow(clippy::expect_used, clippy::panic)]

use indicate_instrument_conformance::admit;
use indicate_instrument_registry::{PanelSet, Registry};

use super::TEMPLATE_SET;

/// The claim this crate makes about itself, as a build result rather
/// than a comment: the template is judged by the same harness every
/// other set is, over the shared canonical corpus and its own
/// withholding matrix.
#[test]
fn the_template_set_passes_admission() {
    static SETS: [&PanelSet; 1] = [&TEMPLATE_SET];
    let registry = Registry::from_sets(&SETS).expect("the template set composes");
    let report = admit(&registry).expect("the template must be admissible");
    // Six canonical states x (one fed case + one per required group
    // withheld) x (quiet, alerted); the panel contributes no extreme
    // states of its own. The alert axis is not optional: a composed
    // screen fans one AlertOutput to every slot, so a panel's
    // criticality band is only honest if it was measured with the stack
    // drawn.
    assert_eq!(report.cases, 36);
    // A set copied from this template starts with nothing tolerated:
    // every run's nominal ink sits inside the design frame, so no
    // frame-overflow observation is counted. Keeping the line at zero is
    // what makes a new warning a decision instead of a drift.
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
}
