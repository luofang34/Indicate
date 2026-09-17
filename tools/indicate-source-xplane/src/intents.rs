//! An admitted operator intent as the dataref write that carries it.

use indicate_instrument_state::{AltitudeClass, HeadingReference};
use indicate_intent::Intent;

use crate::datarefs::Dataref;
use crate::units::{HPA_PER_INHG, M_PER_FT, rad_to_deg};

/// The dataref and value an intent writes, or `None` for an intent the
/// simulator has no dataref for in the reference the request named.
/// A heading bug is written only against the magnetic reference the
/// simulator's bug uses, and an altitude only in the barometric class
/// its selector uses; a request in another reference is not silently
/// converted.
pub fn dref_for(intent: &Intent) -> Option<(Dataref, f32)> {
    match *intent {
        Intent::SelectBaro { hpa } => Some((Dataref::BaroInHg, hpa / HPA_PER_INHG)),
        Intent::SetHeadingBug {
            rad,
            reference: HeadingReference::Magnetic,
        } => Some((Dataref::HeadingBugMagDeg, rad_to_deg(rad))),
        Intent::SelectAltitude {
            meters,
            class: AltitudeClass::BaroIndicated,
            ..
        } => Some((Dataref::AltitudeSelFt, meters / M_PER_FT)),
        Intent::SetHeadingBug { .. }
        | Intent::SelectAltitude { .. }
        | Intent::SelectNavSource(_)
        | Intent::AcknowledgeAlert(_)
        | Intent::AcknowledgeAllAlerts
        | Intent::SelectPage(_)
        | Intent::Declutter(_) => None,
    }
}

#[cfg(test)]
mod tests;
