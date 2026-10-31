//! Fallback synthesis of [`AircraftState`] from [`PanelData`] when raw state is absent.

use indicate_instrument_state::{
    AirData, AircraftState, AltitudeClass, Attitude, DynSample, EstimateQuality, FdSample,
    GeoidModelId, GroupId, HeadingSample, Kinematics, PanelData, Quat, SignalStatus,
    SnapshotCoherence, SnapshotMeta, Stamped, TurnSample, ValidFlags,
    units::{M_TO_FT, MPS_TO_FPM, MPS_TO_KT},
};

const FRESH_AGE_MS: f32 = 20.0;
const STALE_AGE_MS: f32 = 1500.0;

pub(super) fn synthesize_aircraft_state(data: &PanelData) -> AircraftState {
    let mut state = AircraftState {
        selections: data.selections,
        quality: synthesis_quality(data),
        snapshot: SnapshotMeta {
            generation: 1,
            coherence: SnapshotCoherence::Coherent,
        },
        ..AircraftState::default()
    };
    fill_primary_groups(&mut state, data);
    fill_navigation_groups(&mut state, data);
    fill_equipment_and_flight(&mut state, data);
    state
}

fn synthesis_quality(data: &PanelData) -> EstimateQuality {
    let mut any_present = false;
    let mut all_degraded = true;
    for g in GroupId::ALL {
        let s = data.groups.status(g);
        if s != SignalStatus::Missing {
            any_present = true;
            if s != SignalStatus::Degraded {
                all_degraded = false;
            }
        }
    }
    if any_present && all_degraded {
        EstimateQuality::Degraded
    } else {
        EstimateQuality::Good
    }
}

const fn age_for_status(status: SignalStatus) -> Option<f32> {
    match status {
        SignalStatus::Missing => None,
        SignalStatus::Stale => Some(STALE_AGE_MS),
        SignalStatus::Valid | SignalStatus::Degraded | SignalStatus::Failed => Some(FRESH_AGE_MS),
    }
}

fn fill_primary_groups(state: &mut AircraftState, data: &PanelData) {
    let mut valid = ValidFlags::default();
    let att_status = data.groups.status(GroupId::Attitude);
    if let Some(age) = age_for_status(att_status) {
        let yaw = if data.heading.value_rad.status.shows_value() {
            data.heading.value_rad.value
        } else {
            0.0
        };
        state.attitude = Stamped {
            data: Some(Attitude {
                quat: Quat::from_euler(data.roll_rad.value, data.pitch_rad.value, yaw),
                rates_rps: [0.0, 0.0, 0.0],
            }),
            age_ms: Some(age),
        };
        let ok = att_status != SignalStatus::Failed;
        valid.attitude = ok;
        valid.rates = ok;
    }

    let kin_status = data.groups.status(GroupId::Kinematics);
    if let Some(age) = age_for_status(kin_status) {
        let alt_m = data.altitude.value_ft.value / M_TO_FT;
        let gs_mps = data.gs_kt.value / MPS_TO_KT;
        let trk = data.track_rad.value;
        let vn = gs_mps * libm::cosf(trk);
        let ve = gs_mps * libm::sinf(trk);
        let vd = -(data.vsi_fpm.value / MPS_TO_FPM);
        state.kinematics = Stamped {
            data: Some(Kinematics {
                pos_ned_m: [0.0, 0.0, -alt_m],
                vel_ned_mps: [vn, ve, vd],
            }),
            age_ms: Some(age),
        };
        let ok = kin_status != SignalStatus::Failed;
        valid.position = ok;
        valid.velocity_horizontal = ok;
        valid.velocity_vertical = ok;
    }
    state.valid = valid;
    fill_air_and_altitude(state, data);
}

fn fill_air_and_altitude(state: &mut AircraftState, data: &PanelData) {
    let air_status = data.groups.status(GroupId::Air);
    if let Some(age) = age_for_status(air_status) {
        let air = if air_status == SignalStatus::Failed {
            AirData {
                ias_mps: Some(f32::INFINITY),
                baro_setting_hpa: None,
                tas_mps: None,
            }
        } else {
            AirData {
                ias_mps: (data.ias_kt.status != SignalStatus::Missing)
                    .then_some(data.ias_kt.value / MPS_TO_KT),
                baro_setting_hpa: (data.baro_hpa.status != SignalStatus::Missing)
                    .then_some(data.baro_hpa.value),
                tas_mps: (data.tas_kt.status != SignalStatus::Missing)
                    .then_some(data.tas_kt.value / MPS_TO_KT),
            }
        };
        state.air = Stamped {
            data: Some(air),
            age_ms: Some(age),
        };
    }

    let alt_m = data.altitude.value_ft.value / M_TO_FT;
    state.altitude.reference_class = data.altitude.class;
    state.altitude.origin = data.altitude.origin;
    if data.altitude.class != AltitudeClass::LocalRelative {
        state.altitude.sample_m = data.altitude.value_ft.status.shows_value().then_some(alt_m);
        if data.altitude.class == AltitudeClass::GeometricMsl {
            state.altitude.geoid_model = GeoidModelId(1);
        }
    }
}

fn fill_navigation_groups(state: &mut AircraftState, data: &PanelData) {
    let hdg_status = data.groups.status(GroupId::Heading);
    if let Some(age) = age_for_status(hdg_status) {
        state.heading = Stamped {
            data: Some(HeadingSample {
                heading_rad: data.heading.value_rad.value,
                reference: data.heading.reference,
            }),
            age_ms: Some(age),
        };
        state.valid.heading = hdg_status != SignalStatus::Failed;
    }

    let nav_status = data.groups.status(GroupId::Nav);
    if let Some(age) = age_for_status(nav_status) {
        state.nav = Stamped {
            data: Some(data.nav.data),
            age_ms: Some(age),
        };
    }

    let wind_status = data.groups.status(GroupId::Wind);
    if let Some(age) = age_for_status(wind_status) {
        state.wind = Stamped {
            data: Some(data.wind.value),
            age_ms: Some(age),
        };
    }

    let dyn_status = data.groups.status(GroupId::Dynamics);
    if let Some(age) = age_for_status(dyn_status) {
        let ok = dyn_status != SignalStatus::Failed;
        state.dynamics = Stamped {
            data: Some(DynSample {
                turn: (data.turn.rate_rps.status != SignalStatus::Missing).then_some(TurnSample {
                    rate_rps: data.turn.rate_rps.value,
                    basis: data.turn.basis,
                }),
                lateral_mps2: (data.slip_lat_mps2.status != SignalStatus::Missing)
                    .then_some(data.slip_lat_mps2.value),
                ias_trend_mps2: (data.ias_trend_kt_s.status != SignalStatus::Missing)
                    .then_some(data.ias_trend_kt_s.value / MPS_TO_KT),
            }),
            age_ms: Some(age),
        };
        state.valid.turn = ok;
        state.valid.slip = ok;
        state.valid.ias_trend = ok;
    }
}

fn fill_equipment_and_flight(state: &mut AircraftState, data: &PanelData) {
    if let Some(age) = age_for_status(data.groups.status(GroupId::FlightDirector)) {
        state.director = Stamped {
            data: Some(FdSample {
                pitch_cmd_rad: data.flight.director.pitch_cmd_rad,
                roll_cmd_rad: data.flight.director.roll_cmd_rad,
                mode: data.flight.director.mode,
                engagement: data.flight.director.engagement,
            }),
            age_ms: Some(age),
        };
    }
    if let Some(age) = age_for_status(data.groups.status(GroupId::MonitorText)) {
        state.monitor_text = Stamped {
            data: Some(data.monitor_text.value),
            age_ms: Some(age),
        };
    }
    if let Some(age) = age_for_status(data.groups.status(GroupId::BearingPointers)) {
        state.bearings = Stamped {
            data: Some(data.bearings.value),
            age_ms: Some(age),
        };
    }
    if let Some(age) = age_for_status(data.groups.status(GroupId::AirframeConfig)) {
        state.airframe = Stamped {
            data: Some(data.airframe.value),
            age_ms: Some(age),
        };
    }
    if let Some(age) = age_for_status(data.groups.status(GroupId::ApModes)) {
        state.ap_modes = Stamped {
            data: Some(data.flight.ap_modes.value),
            age_ms: Some(age),
        };
    }
    if data.flight.guidance_present {
        state.flight.guidance = Stamped {
            data: Some(data.flight.guidance.value),
            age_ms: data.flight.guidance_age_ms.or(Some(FRESH_AGE_MS)),
        };
    }
    if data.flight.speed_present {
        state.flight.speed = Stamped {
            data: Some(data.flight.speed.value),
            age_ms: Some(FRESH_AGE_MS),
        };
    }
}
