//! The fixture guard: every state the matrix draws carries the record
//! samples its headers declare.

use indicate_instrument_registry::{CANONICAL_STATES, PanelDescriptor};

use super::AdmissionError;

/// Every fixture the panel is drawn over carries engine and fuel tank
/// samples that number its state's headers, so the matrix never draws a
/// state whose samples contradict it. A host is held to the same rule by
/// `encode_state_with`.
pub(super) fn check_fixture_engines(panel: &'static PanelDescriptor) -> Result<(), AdmissionError> {
    let fixtures = CANONICAL_STATES
        .iter()
        .map(|s| (s.id, s.build, s.engines, s.tanks))
        .chain(
            panel
                .extreme_states
                .iter()
                .map(|e| (e.id, e.build, e.engines, e.tanks)),
        );
    for (state, build, engines, tanks) in fixtures {
        let built = build();
        let groups = [
            (built.engine.data, engines.len()),
            (built.fuel_tanks.data, tanks.len()),
        ];
        for (header, supplied) in groups {
            let declared = header.map_or(0, |group| usize::from(group.count));
            if declared != supplied {
                return Err(AdmissionError::FixtureEngines {
                    panel: panel.id,
                    state,
                    declared,
                    supplied,
                });
            }
        }
    }
    Ok(())
}
