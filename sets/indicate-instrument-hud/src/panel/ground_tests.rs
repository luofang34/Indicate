//! Rocket impact point, guided-bomb release, directed energy, and surface contacts.
#![allow(clippy::expect_used, clippy::panic)]
use super::tests::*;
use super::*;
use crate::context::{EnergyState, HudProfile, WeaponMode};
use crate::target::{Contact, Domain, Identity, TargetPicture};

fn surface(position: [f32; 3]) -> Contact {
    Contact {
        relative_position_m: position,
        relative_velocity_mps: [0.0; 3],
        identity: Identity::Hostile,
        domain: Domain::Surface,
        status: SignalStatus::Valid,
    }
}

fn armed(weapon: WeaponMode) -> HudContext {
    HudContext {
        profile: HudProfile::Screen,
        projection: synthetic(),
        weapon,
        terrain_elevation_m: Sig::valid(500.0),
        ..HudContext::default()
    }
}

fn designated(contact: Contact) -> TargetPicture {
    let mut picture = TargetPicture::default();
    picture.push(contact);
    picture.designate(Some(0));
    picture
}

#[test]
fn the_rocket_impact_point_lies_on_the_terrain_ahead() {
    let data = flight(0.0, -0.5, 0.0);
    let context = armed(WeaponMode::Rockets);
    let camera = Camera::new(&data, context.projection.value, FRAME_LANDSCAPE);
    let impact = weapon::ground::rocket_impact(&data, &context, &camera).expect("impact");
    assert!(
        (impact[2] - (3000.0 - 500.0)).abs() < 1.0,
        "falls to the terrain: {impact:?}"
    );
    assert!(impact[0] > 0.0 && impact[1].abs() < 1.0);
}

#[test]
fn a_rocket_pass_shows_the_impact_point_in_range_and_needs_terrain() {
    // A 17-degree dive, short of the simulator profile's recovery threshold.
    let mut state = flight_state(0.0, -0.3, 0.0);
    if let Some(k) = state.kinematics.data.as_mut() {
        k.pos_ned_m[2] = -1500.0;
    }
    state.altitude.sample_m = Some(1500.0);
    let data = indicate_instrument_state::resolve(
        &state,
        &indicate_instrument_state::FreshnessPolicy::default(),
    );
    let drawn = draw(&data, &armed(WeaponMode::Rockets));
    assert!(
        drawn.has("RKT") && (drawn.has("IN RNG") || drawn.has("CCIP LIMIT")),
        "{:?}",
        drawn.texts
    );
    let blind = HudContext {
        terrain_elevation_m: Sig::with_status(0.0, SignalStatus::Missing),
        ..armed(WeaponMode::Rockets)
    };
    assert!(draw(&data, &blind).has("CCIP INVALID"));
}

#[test]
fn the_release_cue_steers_then_releases_then_warns_too_close() {
    assert_eq!(
        weapon::ground::release_state(12_000.0, 6000.0, 1.6).0,
        "STEER"
    );
    assert_eq!(weapon::ground::release_state(8000.0, 6000.0, 1.6).0, "REL");
    assert_eq!(
        weapon::ground::release_state(3000.0, 6000.0, 1.6).0,
        "MIN RNG"
    );
    assert!(weapon::ground::release_state(12_000.0, 6000.0, 1.6).1 > 0.0);
    assert_eq!(weapon::ground::release_state(8000.0, 6000.0, 1.6).1, 0.0);
}

#[test]
fn a_guided_bomb_steers_to_a_designated_surface_target_and_marks_the_laser() {
    let context = HudContext {
        targets: designated(surface([6000.0, 300.0, 2900.0])),
        ..armed(WeaponMode::GuidedBomb)
    };
    let drawn = draw(&flight(0.0, 0.0, 0.0), &context);
    assert!(
        drawn.has("LGB") && drawn.has("REL") && drawn.has("L"),
        "{:?}",
        drawn.texts
    );
    assert!(draw(&flight(0.0, 0.0, 0.0), &armed(WeaponMode::GuidedBomb)).has("NO DESIG"));
}

#[test]
fn the_energy_weapon_shows_lasing_heat_cut_out_and_on_target() {
    let with = |energy: EnergyState, targets: TargetPicture| HudContext {
        energy: Some(energy),
        targets,
        ..armed(WeaponMode::DirectedEnergy)
    };
    let cold = EnergyState {
        heat: 0.2,
        firing: false,
        locked_out: false,
    };
    let ahead = designated(Contact {
        domain: Domain::Air,
        ..surface([2000.0, 0.0, 0.0])
    });
    let data = flight(0.0, 0.0, 0.0);
    assert!(draw(&data, &with(cold, ahead)).has("ON TGT"));
    assert!(
        draw(
            &data,
            &with(
                EnergyState {
                    firing: true,
                    ..cold
                },
                TargetPicture::default()
            )
        )
        .has("LASE")
    );
    assert!(
        draw(
            &data,
            &with(
                EnergyState {
                    heat: 1.0,
                    firing: false,
                    locked_out: true
                },
                TargetPicture::default()
            )
        )
        .has("HOT")
    );
    let far = designated(Contact {
        domain: Domain::Air,
        ..surface([6000.0, 0.0, 0.0])
    });
    assert!(!draw(&data, &with(cold, far)).has("ON TGT"));
}

#[test]
fn a_surface_contact_stands_on_a_stake() {
    let air = HudContext {
        targets: designated(Contact {
            domain: Domain::Air,
            ..surface([3000.0, 100.0, 300.0])
        }),
        ..armed(WeaponMode::Navigation)
    };
    let ground = HudContext {
        targets: designated(surface([3000.0, 100.0, 300.0])),
        ..armed(WeaponMode::Navigation)
    };
    let data = flight(0.0, 0.0, 0.0);
    assert_eq!(
        draw(&data, &ground).lines.len(),
        draw(&data, &air).lines.len() + 4,
        "stem and foot, each with a halo"
    );
}
