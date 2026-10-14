#![allow(clippy::expect_used)]
use super::*;
use indicate_instrument_hmd::draw_hwd;
use indicate_instrument_scene::{Anchor, SceneWriter};
use indicate_instrument_state::SignalStatus;

#[test]
fn collision_guard_rejects_touching_halos_and_accepts_clear_text() {
    for (second_y, valid) in [(31.0, false), (34.0, true)] {
        let mut bytes = [0; 512];
        let mut writer = SceneWriter::new(&mut bytes).expect("buffer");
        writer
            .text(40.0, 20.0, 10.0, Anchor::CENTER, "FIRST")
            .expect("text");
        writer
            .text(40.0, second_y, 10.0, Anchor::CENTER, "SECOND")
            .expect("text");
        let size = writer.finish();
        assert_eq!(check(&bytes[..size]).is_ok(), valid);
    }
}

#[test]
fn review_cases_keep_text_clear_with_invalid_sources_and_large_readouts() {
    for case in crate::cases::all() {
        for status in [
            SignalStatus::Valid,
            SignalStatus::Stale,
            SignalStatus::Degraded,
            SignalStatus::Failed,
            SignalStatus::Missing,
        ] {
            let mut data = case.data;
            for signal in [
                &mut data.gs_kt,
                &mut data.vsi_fpm,
                &mut data.altitude.value_ft,
                &mut data.baro_hpa,
                &mut data.heading.value_rad,
                &mut data.track_rad,
            ] {
                signal.status = status;
            }
            data.baro_hpa.value = 999999.0;
            data.gs_kt.value = 999999.0;
            data.vsi_fpm.value = -999999.0;
            data.altitude.value_ft.value = -999999.0;
            data.altitude.setting_mismatch = true;
            for context in crate::review::contexts() {
                for compact in [false, true] {
                    let mut bytes = [0; 8192];
                    let mut writer = SceneWriter::new(&mut bytes).expect("buffer");
                    draw_hwd(&data, context, compact, case.alerts.as_ref(), &mut writer)
                        .expect("scene");
                    let size = writer.finish();
                    assert!(
                        check(&bytes[..size]).is_ok(),
                        "{} {status:?} {context:?} compact={compact}: {:?}",
                        case.name,
                        check(&bytes[..size])
                    );
                }
            }
        }
    }
}

#[test]
fn moving_ticks_keep_clearance_between_fixture_values() {
    let cases = crate::cases::all();
    for index in [0, 42] {
        for step in 0..1000 {
            let mut data = cases[index].data;
            data.ias_kt.value = 280.0 + step as f32 / 100.0;
            data.altitude.value_ft.value = 35000.0 + step as f32 / 5.0;
            data.flight.speed.value.current = Some(data.ias_kt.value / 1.9438445);
            let mut bytes = [0; 8192];
            let mut writer = SceneWriter::new(&mut bytes).expect("buffer");
            draw_hwd(&data, Default::default(), false, None, &mut writer).expect("scene");
            let size = writer.finish();
            assert!(
                check(&bytes[..size]).is_ok(),
                "case={index} step={step}: {:?}",
                check(&bytes[..size])
            );
        }
    }
}
