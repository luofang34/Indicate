//! The set against the contract: admission, spec checks, and the
//! pinned scene digest.

use std::string::String;
use std::vec;

use indicate_instrument_conformance::admit;
use indicate_instrument_registry::{PanelSet, Registry, scene_digest};
use indicate_instrument_scene::MAX_SCENE_BYTES;

use super::GAUGES;
use crate::{
    ANALOG_PANELS, ANALOG_SET, AnalogDialSpec, Channel, DialChannel, DialInput, DialUnit, ScaleLaw,
    ScalePoint, SpecError,
};

#[test]
fn every_built_in_spec_passes_its_checks() {
    for (name, spec) in GAUGES {
        assert_eq!(spec.check(), Ok(()), "{name}");
    }
}

#[test]
fn the_analog_set_passes_admission() {
    static SETS: [&PanelSet; 1] = [&ANALOG_SET];
    let registry = Registry::from_sets(&SETS).expect("the analog set composes");
    let report = admit(&registry).expect("every analog panel is admissible");
    // Per panel: (canonical + extreme states) x (fed + one per required
    // group withheld) x (quiet, alerted), at the one canonical frame.
    let canonical = indicate_instrument_registry::CANONICAL_STATES.len();
    let expected: usize = ANALOG_PANELS
        .iter()
        .map(|p| (canonical + p.extreme_states.len()) * (1 + p.required_groups.len() as usize) * 2)
        .sum();
    assert_eq!(report.cases, expected);
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
}

fn hex(digest: [u8; 32]) -> String {
    digest.iter().map(|b| std::format!("{b:02x}")).collect()
}

/// The digest of every analog panel's scene over the corpus and its
/// extreme states. A mismatch means the paint changed: re-pin only for
/// a change that meant to move it, and say why in the commit.
const ANALOG_SCENE_DIGEST: &str =
    "b6fa0b3d67423d45f521e36873eb3af714f3fb289db8f290c59b497a76de9c1d";

#[test]
fn the_analog_scene_digest_is_pinned() {
    let registry = Registry::new(ANALOG_PANELS).expect("composes");
    let mut scratch = vec![0u8; MAX_SCENE_BYTES];
    let digest = scene_digest(&registry, &mut scratch).expect("digests");
    assert_eq!(hex(digest), ANALOG_SCENE_DIGEST);
}

#[test]
fn a_spec_that_cannot_place_values_is_refused() {
    const DESCENDING: [ScalePoint; 2] = [
        ScalePoint {
            value: 100.0,
            angle_deg: 0.0,
        },
        ScalePoint {
            value: 50.0,
            angle_deg: 90.0,
        },
    ];
    let mut rings = [crate::AIRSPEED_KT.rings[0]];
    rings[0].law = ScaleLaw::Piecewise {
        reference_deg: 0.0,
        points: &DESCENDING,
    };
    let spec = AnalogDialSpec {
        rings: &rings,
        ..crate::AIRSPEED_KT
    };
    assert_eq!(spec.check(), Err(SpecError::BadLaw { index: 0 }));
}

#[test]
fn a_dial_engraved_in_the_wrong_dimension_is_refused() {
    let spec = AnalogDialSpec {
        primary: DialChannel {
            input: DialInput::IndicatedAirspeed,
            unit: DialUnit::Feet,
        },
        ..crate::AIRSPEED_KT
    };
    assert_eq!(
        spec.check(),
        Err(SpecError::UnitMismatch {
            channel: Channel::Primary
        })
    );
}

#[test]
fn a_moving_part_on_a_missing_channel_is_refused() {
    let spec = AnalogDialSpec {
        secondary: None,
        ..crate::ALTIMETER
    };
    assert_eq!(
        spec.check(),
        Err(SpecError::MissingChannel {
            channel: Channel::Secondary
        })
    );
}
