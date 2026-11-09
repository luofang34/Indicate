//! The Air group's signals: indicated and true airspeed, the applied
//! altimeter setting, and the rate-of-climb instrument's reading.

use super::fault_status;
use crate::aircraft::AircraftState;
use crate::signal::{FreshnessPolicy, Sig, SignalStatus};
use crate::units::{MPS_TO_FPM, MPS_TO_KT};
use crate::validate::StateIntegrity;

/// The Air group resolved into display units.
pub(super) struct AirSignals {
    pub(super) ias_kt: Sig<f32>,
    pub(super) tas_kt: Sig<f32>,
    pub(super) baro_hpa: Sig<f32>,
    pub(super) vsi_indicated_fpm: Sig<f32>,
}

pub(super) fn air_signals(
    state: &AircraftState,
    policy: &FreshnessPolicy,
    quality: SignalStatus,
    integrity: &StateIntegrity,
) -> AirSignals {
    let air_fresh = policy.status_for_age(state.air.age_ms);
    let air_fault = fault_status(integrity.air);
    let air = state.air.data.unwrap_or_default();
    // Sensed air-data values fold source quality; the applied altimeter
    // setting does not, because a setting is a dialed configuration
    // value and an estimate's quality says nothing about it.
    let sensed = |value: Option<f32>, scale: f32| match value {
        Some(v) => Sig::with_status(v * scale, air_fresh.worst(quality).worst(air_fault)),
        None => Sig::missing(),
    };
    let baro_hpa = match air.baro_setting_hpa {
        Some(v) => Sig::with_status(v, air_fresh.worst(air_fault)),
        None => Sig::missing(),
    };
    AirSignals {
        ias_kt: sensed(air.ias_mps, MPS_TO_KT),
        tas_kt: sensed(air.tas_mps, MPS_TO_KT),
        baro_hpa,
        vsi_indicated_fpm: sensed(air.vsi_mps, MPS_TO_FPM),
    }
}
