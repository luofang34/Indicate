//! Conformance admission sweep for loaded `.indpack` Wasm plugins (`PLUG-01`).

use indicate_alerts::{
    AlertCondition, AlertContext, AlertEvent, AlertManager, AlertOutput, AlertProfile,
    MiscompareFault,
};
use indicate_instrument_descriptor::{DrawContext, EMPTY_CONFIG};
use indicate_instrument_registry::CANONICAL_STATES;
use indicate_instrument_scene::MAX_SCENE_BYTES;
use indicate_instrument_scene::angular::MAX_ANGULAR_SCENE_BYTES;
use indicate_instrument_scene::volume::MAX_VOLUME_SCENE_BYTES;
use indicate_instrument_state::{AircraftState, FreshnessPolicy, GroupId, resolve, withhold_group};
use indicate_plugin_abi::PluginSpaceWire;

mod scan;

use crate::error::PluginHostError;
use crate::host::{LoadedPlugin, PluginDrawTarget};

use scan::{verify_angular_scene, verify_planar_scene, verify_volume_scene};

/// Summary returned when [`admit_plugin`] accepts a `.indpack` plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PluginAdmissionReport {
    /// Total `(canonical state × withholding × alert)` cases drawn and verified.
    pub cases_checked: usize,
    /// Peak deterministic Wasm fuel consumed by any single case.
    pub max_case_fuel: u64,
}

struct PluginCase {
    state_id: &'static str,
    withheld: Option<GroupId>,
    state: AircraftState,
    alerts: Option<AlertOutput>,
}

/// Runs the canonical-state and withholding admission matrix over `plugin`.
pub fn admit_plugin(plugin: &mut LoadedPlugin) -> Result<PluginAdmissionReport, PluginHostError> {
    let required_groups = plugin.descriptor().required_groups;
    let space = plugin.descriptor().space;
    let cases = build_cases(required_groups);
    let mut report = PluginAdmissionReport {
        cases_checked: 0,
        max_case_fuel: 0,
    };

    match space {
        PluginSpaceWire::Planar2D {
            frame_min,
            frame_max,
            canonical_buf,
            canonical_len,
            ..
        } => {
            let mut out = vec![0u8; MAX_SCENE_BYTES];
            for frame in &canonical_buf[..usize::from(canonical_len)] {
                for case in &cases {
                    let fuel = check_planar_case(plugin, case, *frame, &mut out)?;
                    report.cases_checked += 1;
                    report.max_case_fuel = report.max_case_fuel.max(fuel);
                }
            }
            if frame_min != frame_max {
                check_planar_range_varies(plugin, &cases, frame_min, frame_max)?;
            }
        }
        PluginSpaceWire::AngularSphere { overlay_frame } => {
            let mut ang = vec![0u8; MAX_ANGULAR_SCENE_BYTES];
            let mut ov = vec![0u8; MAX_SCENE_BYTES];
            for case in &cases {
                let fuel = check_angular_case(plugin, case, overlay_frame, &mut ang, &mut ov)?;
                report.cases_checked += 1;
                report.max_case_fuel = report.max_case_fuel.max(fuel);
            }
        }
        PluginSpaceWire::Volume3D {
            extent_min_m,
            extent_max_m,
            canonical_buf,
            canonical_len,
            ..
        } => {
            let mut out = vec![0u8; MAX_VOLUME_SCENE_BYTES];
            for ext in &canonical_buf[..usize::from(canonical_len)] {
                for case in &cases {
                    let fuel = check_volume_case(plugin, case, *ext, &mut out)?;
                    report.cases_checked += 1;
                    report.max_case_fuel = report.max_case_fuel.max(fuel);
                }
            }
            if extent_min_m != extent_max_m {
                check_volume_range_varies(plugin, &cases, extent_min_m, extent_max_m)?;
            }
        }
    }
    Ok(report)
}

fn build_cases(required_groups: indicate_instrument_descriptor::GroupSet) -> Vec<PluginCase> {
    let mut mgr = AlertManager::new();
    let alerted = mgr.step(
        &AlertProfile::simulator(),
        &[AlertEvent::Assert(AlertCondition::Miscompare(
            MiscompareFault::Attitude,
        ))],
        AlertContext::default(),
        1_000,
    );
    let mut cases = Vec::new();
    for entry in CANONICAL_STATES {
        let base = (entry.build)();
        let mut withholdings = vec![(None, base)];
        for group in GroupId::ALL {
            if required_groups.contains(group) {
                withholdings.push((Some(group), withhold_group(&base, group)));
            }
        }
        for (withheld, state) in withholdings {
            for alerts in [None, Some(alerted)] {
                cases.push(PluginCase {
                    state_id: entry.id,
                    withheld,
                    state,
                    alerts,
                });
            }
        }
    }
    cases
}

fn check_planar_case(
    plugin: &mut LoadedPlugin,
    case: &PluginCase,
    frame: indicate_instrument_descriptor::DesignFrame,
    out: &mut [u8],
) -> Result<u64, PluginHostError> {
    let data = resolve(&case.state, &FreshnessPolicy::default());
    let ctx = DrawContext::new(&data)
        .with_config(EMPTY_CONFIG)
        .with_alerts(case.alerts.as_ref());
    let rep = plugin.draw(&ctx, PluginDrawTarget::Planar2D { frame, out })?;
    let desc = *plugin.descriptor();
    verify_planar_scene(
        &desc,
        case.state_id,
        case.withheld,
        frame,
        &out[..rep.primary_bytes],
    )?;
    Ok(rep.fuel_consumed)
}

fn check_angular_case(
    plugin: &mut LoadedPlugin,
    case: &PluginCase,
    overlay_frame: Option<indicate_instrument_descriptor::DesignFrame>,
    ang_buf: &mut [u8],
    ov_buf: &mut [u8],
) -> Result<u64, PluginHostError> {
    let data = resolve(&case.state, &FreshnessPolicy::default());
    let ctx = DrawContext::new(&data)
        .with_config(EMPTY_CONFIG)
        .with_alerts(case.alerts.as_ref());
    let desc = *plugin.descriptor();
    let rep = if let Some(frame) = overlay_frame {
        let rep = plugin.draw(
            &ctx,
            PluginDrawTarget::AngularSphere {
                angular_out: ang_buf,
                overlay_out: Some((frame, ov_buf)),
            },
        )?;
        verify_planar_scene(
            &desc,
            case.state_id,
            case.withheld,
            frame,
            &ov_buf[..rep.overlay_bytes],
        )?;
        rep
    } else {
        plugin.draw(
            &ctx,
            PluginDrawTarget::AngularSphere {
                angular_out: ang_buf,
                overlay_out: None,
            },
        )?
    };
    verify_angular_scene(
        &desc,
        case.state_id,
        case.withheld,
        &ang_buf[..rep.primary_bytes],
    )?;
    Ok(rep.fuel_consumed)
}

fn check_volume_case(
    plugin: &mut LoadedPlugin,
    case: &PluginCase,
    extent_m: indicate_instrument_scene::volume::Extent3D,
    out: &mut [u8],
) -> Result<u64, PluginHostError> {
    let data = resolve(&case.state, &FreshnessPolicy::default());
    let ctx = DrawContext::new(&data)
        .with_config(EMPTY_CONFIG)
        .with_alerts(case.alerts.as_ref());
    let rep = plugin.draw(&ctx, PluginDrawTarget::Volume3D { extent_m, out })?;
    let desc = *plugin.descriptor();
    verify_volume_scene(
        &desc,
        case.state_id,
        case.withheld,
        extent_m,
        &out[..rep.primary_bytes],
    )?;
    Ok(rep.fuel_consumed)
}

fn check_planar_range_varies(
    plugin: &mut LoadedPlugin,
    cases: &[PluginCase],
    min: indicate_instrument_descriptor::DesignFrame,
    max: indicate_instrument_descriptor::DesignFrame,
) -> Result<(), PluginHostError> {
    let mut a = vec![0u8; MAX_SCENE_BYTES];
    let mut b = vec![0u8; MAX_SCENE_BYTES];
    let mut c = vec![0u8; MAX_SCENE_BYTES];
    for case in cases {
        let data = resolve(&case.state, &FreshnessPolicy::default());
        let ctx = DrawContext::new(&data)
            .with_config(EMPTY_CONFIG)
            .with_alerts(case.alerts.as_ref());
        let ra = plugin.draw(
            &ctx,
            PluginDrawTarget::Planar2D {
                frame: min,
                out: &mut a,
            },
        )?;
        let rb = plugin.draw(
            &ctx,
            PluginDrawTarget::Planar2D {
                frame: max,
                out: &mut b,
            },
        )?;
        if a[..ra.primary_bytes] != b[..rb.primary_bytes] {
            let rc = plugin.draw(
                &ctx,
                PluginDrawTarget::Planar2D {
                    frame: min,
                    out: &mut c,
                },
            )?;
            if a[..ra.primary_bytes] == c[..rc.primary_bytes] {
                return Ok(());
            }
        }
    }
    Err(PluginHostError::RangeIgnored)
}

fn check_volume_range_varies(
    plugin: &mut LoadedPlugin,
    cases: &[PluginCase],
    min: indicate_instrument_scene::volume::Extent3D,
    max: indicate_instrument_scene::volume::Extent3D,
) -> Result<(), PluginHostError> {
    let mut a = vec![0u8; MAX_VOLUME_SCENE_BYTES];
    let mut b = vec![0u8; MAX_VOLUME_SCENE_BYTES];
    let mut c = vec![0u8; MAX_VOLUME_SCENE_BYTES];
    for case in cases {
        let data = resolve(&case.state, &FreshnessPolicy::default());
        let ctx = DrawContext::new(&data)
            .with_config(EMPTY_CONFIG)
            .with_alerts(case.alerts.as_ref());
        let ra = plugin.draw(
            &ctx,
            PluginDrawTarget::Volume3D {
                extent_m: min,
                out: &mut a,
            },
        )?;
        let rb = plugin.draw(
            &ctx,
            PluginDrawTarget::Volume3D {
                extent_m: max,
                out: &mut b,
            },
        )?;
        if a[..ra.primary_bytes] != b[..rb.primary_bytes] {
            let rc = plugin.draw(
                &ctx,
                PluginDrawTarget::Volume3D {
                    extent_m: min,
                    out: &mut c,
                },
            )?;
            if a[..ra.primary_bytes] == c[..rc.primary_bytes] {
                return Ok(());
            }
        }
    }
    Err(PluginHostError::RangeIgnored)
}
