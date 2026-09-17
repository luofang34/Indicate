//! The latest dataref values as an [`AircraftState`] the host can admit.
//!
//! Attitude and kinematics go through the feeder's avionics gate in the
//! simulation-truth role, stamped on the simulation clock with a
//! synthesized wrapping sequence, so they carry provenance and the
//! ordering rules: a paused simulator repeats its clock, the gate
//! refuses the repeat as a time regression, and the groups age into
//! `Stale` instead of standing still as fresh. Every other group is
//! built from its own datarefs, aged from the oldest constituent's
//! receipt, and absent until every constituent has arrived.

use indicate_instrument_feeder::aircraft_state::apply_avionics;
use indicate_instrument_feeder::avionics::{
    AttitudeGroup, AvionicsIngress, AvionicsSample, IncarnationPolicy, IngressConfig,
    KinematicsGroup, Provenance,
};
use indicate_instrument_feeder::clock::clock_ok;
use indicate_instrument_feeder::stamp::{CLOCK_SIMULATION, ROLE_SIMULATION_TRUTH, RawStamp};
use indicate_instrument_state::{
    AirData, AircraftState, AltitudeClass, AltitudeDeclaration, GeoidModelId, HeadingReference,
    HeadingSample, MagneticVariation, OriginId, Quat, Selections, Stamped, VariationSourceId, Wind,
};

use crate::datarefs::Dataref;
use crate::error::WireError;
use crate::units::{
    HPA_PER_INHG, M_PER_FT, MPS_PER_KT, deg_to_rad, opengl_to_ned, wrap_heading, wrap_signed,
};
use crate::wire::decode_rref_reply;

/// The source identity the truth stamps carry; `XP` in ASCII.
pub const SOURCE_ID: u64 = 0x5850;
/// The origin identity the barometric altitude datum is declared under.
pub const BARO_ORIGIN: OriginId = OriginId(1);
/// The integrity class the truth stamps declare: the lowest declared
/// class, because a simulator's data path makes no integrity claim.
/// The gate validates the code and does not weigh it.
pub const INTEGRITY_CLASS: u8 = 1;
const VEHICLE_ID: u64 = 1;
const INCARNATION: [u8; 16] = [1; 16];
const TRUTH_VALID_FLAGS: u32 = 0b1111;
const QUALITY_GOOD: u32 = 0;
const NANOS_PER_SECOND: f64 = 1.0e9;
const COUNT: usize = Dataref::ALL.len();

/// Why a packet was not applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SourceFault {
    /// The packet was refused on the wire.
    #[error("{0}")]
    Wire(#[from] WireError),
    /// The host clock reading could not be trusted.
    #[error("host clock reading could not be trusted")]
    InvalidClock,
}

/// Wrap-counted packet accounting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SourceCounters {
    /// Reply packets applied.
    pub packets: u32,
    /// Records whose index this build never subscribed.
    pub unknown_indices: u32,
    /// Truth publications the gate refused (a paused simulator repeats
    /// its clock; each repeat counts here).
    pub refused_truth: u32,
}

/// The adapter's admitted state.
#[derive(Debug)]
pub struct XPlaneSource {
    values: [Option<f32>; COUNT],
    received_ms: [Option<f64>; COUNT],
    ingress: AvionicsIngress,
    sequence: u32,
    observed_ms: Option<f64>,
    counters: SourceCounters,
}

impl Default for XPlaneSource {
    fn default() -> Self {
        Self::new()
    }
}

impl XPlaneSource {
    /// A source that has heard nothing.
    pub fn new() -> Self {
        Self {
            values: [None; COUNT],
            received_ms: [None; COUNT],
            ingress: AvionicsIngress::new(IngressConfig {
                vehicle_id: VEHICLE_ID,
                role: ROLE_SIMULATION_TRUTH,
                source_id: Some(SOURCE_ID),
                incarnation: Some(INCARNATION),
                incarnation_policy: IncarnationPolicy::PinFirst,
                maximum_seen_incarnations: 1,
                maximum_skew_nanos: 0,
            }),
            sequence: 0,
            observed_ms: None,
            counters: SourceCounters::default(),
        }
    }

    /// Applies one reply packet received at `now_ms` on the host clock.
    /// Returns the number of records applied.
    pub fn observe(&mut self, packet: &[u8], now_ms: f64) -> Result<usize, SourceFault> {
        if !clock_ok(now_ms, self.observed_ms) {
            return Err(SourceFault::InvalidClock);
        }
        let records = decode_rref_reply(packet)?;
        self.observed_ms = Some(now_ms);
        let mut applied = 0;
        for record in records {
            let Some(dataref) = Dataref::from_index(record.index) else {
                self.counters.unknown_indices = self.counters.unknown_indices.wrapping_add(1);
                continue;
            };
            if !record.value.is_finite() {
                continue;
            }
            let slot = dataref as usize;
            self.values[slot] = Some(record.value);
            self.received_ms[slot] = Some(now_ms);
            applied += 1;
        }
        self.counters.packets = self.counters.packets.wrapping_add(1);
        self.publish_truth(now_ms);
        Ok(applied)
    }

    /// The state the host admits next, aged at `now_ms`.
    pub fn snapshot(&self, now_ms: f64) -> AircraftState {
        let mut state = AircraftState::default();
        apply_avionics(&self.ingress.snapshot(now_ms), &mut state);
        let clock = clock_ok(now_ms, self.observed_ms);
        let age = |datarefs: &[Dataref]| self.group_age(datarefs, now_ms, clock);

        if let Some(age_ms) = age(&[Dataref::IasKt, Dataref::BaroInHg, Dataref::TasMps]) {
            state.air = Stamped {
                data: Some(AirData {
                    ias_mps: self.get(Dataref::IasKt).map(|kt| kt * MPS_PER_KT),
                    baro_setting_hpa: self.get(Dataref::BaroInHg).map(|inhg| inhg * HPA_PER_INHG),
                    tas_mps: self.get(Dataref::TasMps),
                }),
                age_ms: Some(age_ms),
            };
        }
        if let Some(ft) = self.get(Dataref::AltitudeFt) {
            state.altitude = AltitudeDeclaration {
                reference_class: AltitudeClass::BaroIndicated,
                sample_m: Some(ft * M_PER_FT),
                geoid_model: GeoidModelId::UNDECLARED,
                origin: BARO_ORIGIN,
            };
        }
        if let Some(age_ms) = age(&[Dataref::HeadingMagDeg]) {
            state.heading = Stamped {
                data: self.get(Dataref::HeadingMagDeg).map(|deg| HeadingSample {
                    heading_rad: wrap_heading(deg_to_rad(deg)),
                    reference: HeadingReference::Magnetic,
                }),
                age_ms: Some(age_ms),
            };
            state.valid.heading = true;
        }
        if let Some(age_ms) = age(&[Dataref::HeadingTrueDeg, Dataref::HeadingMagDeg]) {
            state.variation = Stamped {
                data: self.variation(),
                age_ms: Some(age_ms),
            };
            state.valid.variation = true;
        }
        if let Some(age_ms) = age(&[Dataref::WindFromMagDeg, Dataref::WindSpeedKt]) {
            state.wind = Stamped {
                data: self.wind(),
                age_ms: Some(age_ms),
            };
        }
        state.selections = self.selections();
        state
    }

    /// Where the truth lane's data came from.
    pub fn provenance(&self) -> Provenance {
        self.ingress.provenance()
    }

    /// Packet accounting.
    pub const fn counters(&self) -> SourceCounters {
        self.counters
    }

    fn get(&self, dataref: Dataref) -> Option<f32> {
        self.values[dataref as usize]
    }

    /// The age of a group is its oldest constituent's; `None` while any
    /// constituent is missing or the clock cannot be trusted.
    fn group_age(&self, datarefs: &[Dataref], now_ms: f64, clock: bool) -> Option<f32> {
        if !clock {
            return None;
        }
        let mut oldest = 0.0f64;
        for dataref in datarefs {
            let received = self.received_ms[*dataref as usize]?;
            oldest = oldest.max(now_ms - received);
        }
        (oldest <= f64::from(f32::MAX)).then_some(oldest as f32)
    }

    /// Variation as the difference the simulator itself reports between
    /// true and magnetic heading, east positive, so no sign convention
    /// of the simulator's own variation dataref is assumed.
    fn variation(&self) -> Option<MagneticVariation> {
        let true_deg = self.get(Dataref::HeadingTrueDeg)?;
        let mag_deg = self.get(Dataref::HeadingMagDeg)?;
        Some(MagneticVariation {
            east_positive_rad: wrap_signed(deg_to_rad(true_deg - mag_deg)),
            source: VariationSourceId::UNDECLARED,
        })
    }

    /// Wind from-direction converted from the simulator's magnetic
    /// reading into true through the reported variation.
    fn wind(&self) -> Option<Wind> {
        let from_mag = self.get(Dataref::WindFromMagDeg)?;
        let speed_kt = self.get(Dataref::WindSpeedKt)?;
        let variation = self.variation()?;
        Some(Wind {
            from_rad: wrap_heading(deg_to_rad(from_mag) + variation.east_positive_rad),
            speed_mps: speed_kt * MPS_PER_KT,
        })
    }

    fn selections(&self) -> Selections {
        Selections {
            heading_bug_rad: self
                .get(Dataref::HeadingBugMagDeg)
                .map_or(0.0, |deg| wrap_heading(deg_to_rad(deg))),
            heading_bug_reference: if self.get(Dataref::HeadingBugMagDeg).is_some() {
                HeadingReference::Magnetic
            } else {
                HeadingReference::Unknown
            },
            altitude_sel_m: self.get(Dataref::AltitudeSelFt).map(|ft| ft * M_PER_FT),
            altitude_sel_class: AltitudeClass::BaroIndicated,
            altitude_sel_origin: BARO_ORIGIN,
            altitude_sel_model: GeoidModelId::UNDECLARED,
            baro_sel_hpa: self.get(Dataref::BaroInHg).map(|inhg| inhg * HPA_PER_INHG),
        }
    }

    /// Offers attitude and kinematics to the truth gate once every
    /// constituent has arrived; the gate decides.
    fn publish_truth(&mut self, now_ms: f64) {
        let Some(sample) = self.truth_sample() else {
            return;
        };
        if !self.ingress.ingest(&sample, now_ms) {
            self.counters.refused_truth = self.counters.refused_truth.wrapping_add(1);
        }
    }

    fn truth_sample(&mut self) -> Option<AvionicsSample> {
        let sim_time_s = self.get(Dataref::SimTimeS)?;
        let quat = Quat::from_euler(
            deg_to_rad(self.get(Dataref::RollDeg)?),
            deg_to_rad(self.get(Dataref::PitchDeg)?),
            deg_to_rad(self.get(Dataref::HeadingTrueDeg)?),
        );
        let rates = [
            deg_to_rad(self.get(Dataref::RollRateDps)?),
            deg_to_rad(self.get(Dataref::PitchRateDps)?),
            deg_to_rad(self.get(Dataref::YawRateDps)?),
        ];
        let pos_ned = opengl_to_ned([
            self.get(Dataref::LocalX)?,
            self.get(Dataref::LocalY)?,
            self.get(Dataref::LocalZ)?,
        ]);
        let vel_ned = opengl_to_ned([
            self.get(Dataref::LocalVx)?,
            self.get(Dataref::LocalVy)?,
            self.get(Dataref::LocalVz)?,
        ]);
        self.sequence = self.sequence.wrapping_add(1);
        let acquired_at_ns = (f64::from(sim_time_s).max(0.0) * NANOS_PER_SECOND) as u64;
        let stamp = RawStamp {
            role: ROLE_SIMULATION_TRUTH,
            integrity: INTEGRITY_CLASS,
            source_id: SOURCE_ID,
            incarnation: INCARNATION,
            epoch: 0,
            sequence: self.sequence,
            acquired_at_ns,
            clock: CLOCK_SIMULATION,
        };
        Some(AvionicsSample {
            vehicle_id: VEHICLE_ID,
            attitude: AttitudeGroup {
                quat: [quat.w, quat.x, quat.y, quat.z],
                rates,
                arm_state: 0,
            },
            kinematics: KinematicsGroup {
                pos_ned,
                vel_ned,
                arm_state: 0,
            },
            valid_flags: TRUTH_VALID_FLAGS,
            quality: QUALITY_GOOD,
            attitude_stamp: Some(stamp),
            kinematics_stamp: Some(stamp),
            estimator_status_stamp: Some(stamp),
        })
    }
}

#[cfg(test)]
mod tests;
