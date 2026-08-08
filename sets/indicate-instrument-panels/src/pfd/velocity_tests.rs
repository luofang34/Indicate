//! The velocity split as painted: a source with a horizontal solution
//! and no vertical-speed estimate gets a groundspeed box that reads and
//! a VSI with no needle and no numeral.
//!
//! Asserting on the emitted scene rather than on [`PanelData`] is the
//! point — a resolved status nothing acts on would leave the fabricated
//! needle exactly where it was.

#![allow(clippy::expect_used, clippy::panic)]

use std::string::String;
use std::vec::Vec;

use indicate_instrument_scene::{Cmd, PaintMode, SceneCmds};
use indicate_instrument_state::{
    AircraftState, EstimateQuality, FreshnessPolicy, Kinematics, PanelData, SnapshotCoherence,
    SnapshotMeta, Stamped, ValidFlags, resolve,
};

use super::PfdConfig;
use super::tests::render;

/// Left edge and width of the VSI needle strip; the needle is the only
/// thing painted there.
const VSI_NEEDLE_X: f32 = 466.0;
const VSI_NEEDLE_W: f32 = 8.0;

/// Anchor x of the VSI numeral beside the needle tip.
const VSI_LABEL_X: f32 = 452.0;

/// Anchor of the groundspeed box's value, centered in its 90×25 box at
/// the panel's left edge. The airspeed tape's failure label shares the
/// x, so the y is part of the address.
const GS_TEXT_XY: (f32, f32) = (45.0, 347.5);

/// Climbing north-east; whether the axes are trusted is `valid`'s job.
fn climbing(valid: ValidFlags) -> PanelData {
    let state = AircraftState {
        kinematics: Stamped {
            data: Some(Kinematics {
                pos_ned_m: [1200.0, 340.0, -305.0],
                vel_ned_mps: [52.0, 9.0, -4.0],
            }),
            age_ms: Some(20.0),
        },
        quality: EstimateQuality::Good,
        valid,
        snapshot: SnapshotMeta {
            generation: 1,
            coherence: SnapshotCoherence::Coherent,
        },
        ..AircraftState::default()
    };
    resolve(&state, &FreshnessPolicy::default())
}

fn horizontal_only() -> ValidFlags {
    ValidFlags {
        position: true,
        velocity_horizontal: true,
        ..ValidFlags::default()
    }
}

fn both_axes() -> ValidFlags {
    ValidFlags {
        velocity_vertical: true,
        ..horizontal_only()
    }
}

/// Every filled rectangle occupying the VSI needle strip.
fn vsi_needles(scene: &[u8]) -> Vec<(f32, f32)> {
    SceneCmds::new(scene)
        .expect("valid scene")
        .map(|c| c.expect("valid command"))
        .filter_map(|c| match c {
            Cmd::Rect {
                mode: PaintMode::Fill,
                x,
                y,
                w,
                h,
            } if x == VSI_NEEDLE_X && w == VSI_NEEDLE_W => Some((y, h)),
            _ => None,
        })
        .collect()
}

/// Every text run anchored at `x`.
fn texts_at(scene: &[u8], at_x: f32) -> Vec<String> {
    texts_where(scene, |x, _| x == at_x)
}

fn texts_where(scene: &[u8], keep: impl Fn(f32, f32) -> bool) -> Vec<String> {
    SceneCmds::new(scene)
        .expect("valid scene")
        .map(|c| c.expect("valid command"))
        .filter_map(|c| match c {
            Cmd::Text { x, y, text, .. } if keep(x, y) => Some(String::from(text)),
            _ => None,
        })
        .collect()
}

/// The acceptance for issue #30, one level below the resolved statuses:
/// the VSI needle and its numeral are gone while the groundspeed box
/// still reads a number.
#[test]
fn a_horizontal_only_source_paints_no_vsi_and_a_live_groundspeed() {
    let data = climbing(horizontal_only());
    assert!(data.gs_kt.status.shows_value());
    assert!(!data.vsi_fpm.status.shows_value());

    let scene = render(&data, &PfdConfig::default());
    assert_eq!(
        vsi_needles(&scene),
        Vec::new(),
        "no needle may be painted for a vertical speed nobody supplied"
    );
    assert_eq!(
        texts_at(&scene, VSI_LABEL_X),
        std::vec![String::from("---")],
        "the strip dashes out rather than reading blank, which scans as zero"
    );

    let (gs_x, gs_y) = GS_TEXT_XY;
    let gs = texts_where(&scene, |x, y| x == gs_x && y == gs_y);
    assert_eq!(
        gs,
        std::vec![String::from("GS 103kt")],
        "groundspeed still reads a number"
    );
}

/// The same source with the vertical axis declared paints both, so the
/// absence above is the split working rather than the VSI being broken.
#[test]
fn declaring_the_vertical_axis_restores_the_needle_and_its_numeral() {
    let scene = render(&climbing(both_axes()), &PfdConfig::default());
    assert_eq!(
        vsi_needles(&scene).len(),
        1,
        "a declared vertical speed paints its needle"
    );
    // -4 m/s down is a climb of about 787 fpm, labelled to the nearest 50.
    assert_eq!(
        texts_at(&scene, VSI_LABEL_X),
        std::vec![String::from("800")]
    );
}
