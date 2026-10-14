//! Unusual-attitude thresholds for a fighter flown in air combat.
//!
//! Fighter crews hold steep pitch and high bank routinely, so the shared
//! simulator thresholds would declutter weapon cues in ordinary manoeuvres.
//! These values are benchmark inputs for a simulator display and imply no
//! aircraft approval: recovery engages near the vertical nose up, in a steep
//! dive, or past inverted, each with a 5- or 10-degree exit margin.
use indicate_instrument_state::{AirframeDisplayProfile, Hysteresis, ProfileError, ProfileLimits};

const DEG: f32 = core::f32::consts::PI / 180.0;

const fn pair(entry: f32, exit: f32) -> Hysteresis {
    Hysteresis {
        entry: entry * DEG,
        exit: exit * DEG,
    }
}

/// The thresholds of [`fighter_profile`].
pub const FIGHTER_LIMITS: ProfileLimits = ProfileLimits {
    unusual_pitch_up: pair(80.0, 75.0),
    unusual_pitch_down: pair(60.0, 55.0),
    unusual_bank: pair(135.0, 125.0),
    chevron_pitch_up: pair(80.0, 75.0),
    chevron_pitch_down: pair(60.0, 55.0),
    bank_hold_pitch: pair(88.0, 87.0),
    min_reverse_band: 2.5 * DEG,
};

/// A validated airframe display profile for an air-combat simulator.
///
/// # Errors
///
/// Returns [`ProfileError`] only if [`FIGHTER_LIMITS`] break the profile rules.
pub fn fighter_profile() -> Result<AirframeDisplayProfile, ProfileError> {
    AirframeDisplayProfile::new(FIGHTER_LIMITS)
}

#[cfg(test)]
mod tests;
