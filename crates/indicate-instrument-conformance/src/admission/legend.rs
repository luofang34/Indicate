//! The printed-legend family: configuration-claimed text must be the
//! same in every case of the matrix.
//!
//! A dial face carries numerals that are not data: the graduations and
//! unit legend a manufacturer printed. A failed source does not erase
//! them on a real instrument, so a panel draws them in every case and
//! claims configuration provenance ([`ATTR_CONFIG`]) for them. The
//! claim is honest only while the run can carry no information about
//! state. That is what this family proves: at each canonical frame,
//! the configuration-claimed runs of every case — every state, every
//! withholding, quiet and alerted — have the same text, the same
//! placement, and the same clip as the first case drawn. A numeral
//! that moves or changes with state is a value, and a value claimed as
//! configuration is refused as [`AdmissionError::ConfigClaim`].
//!
//! The comparison is exact. Placement is the run's design-space ink
//! rectangle after the transform in force, so a legend cannot follow a
//! needle under a `rotate` any more than it can change its digits.

use indicate_instrument_registry::PanelDescriptor;
use indicate_instrument_scene::ATTR_CONFIG;

use super::geometry::Rect;
use super::{AdmissionError, TextRun};

/// One configuration-claimed run, as much of it as a viewer could see.
#[derive(Debug, Clone, PartialEq)]
struct LegendRun {
    text: String,
    rect: Rect,
    clip: Option<Rect>,
}

/// The legends of the first case drawn at one frame: what every later
/// case at that frame must reproduce.
#[derive(Debug, Default)]
pub(super) struct LegendWitness {
    runs: Option<Vec<LegendRun>>,
}

fn legends(runs: &[TextRun]) -> Vec<LegendRun> {
    runs.iter()
        .filter(|run| run.attribution == Some(ATTR_CONFIG))
        .map(|run| LegendRun {
            text: run.text.clone(),
            rect: run.rect,
            clip: run.clip,
        })
        .collect()
}

/// Compares one case's configuration-claimed runs with the witness,
/// recording them as the witness when this is the first case.
pub(super) fn check_legends(
    panel: &'static PanelDescriptor,
    state_id: &'static str,
    runs: &[TextRun],
    witness: &mut LegendWitness,
) -> Result<(), AdmissionError> {
    let current = legends(runs);
    let Some(reference) = witness.runs.as_ref() else {
        witness.runs = Some(current);
        return Ok(());
    };
    if *reference == current {
        return Ok(());
    }
    // Name the first run that differs, or the surplus one, so the
    // error points at the legend that moved rather than at the panel.
    let differing = current
        .iter()
        .zip(reference.iter())
        .find(|(now, then)| now != then)
        .map(|(now, _)| now)
        .or_else(|| current.get(reference.len()))
        .or_else(|| reference.get(current.len()));
    Err(AdmissionError::ConfigClaim {
        panel: panel.id,
        state: state_id,
        text: differing.map(|run| run.text.clone()).unwrap_or_default(),
    })
}
