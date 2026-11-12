//! The nav group carries every flag and scale a guidance source can
//! state across the wire unchanged, so a FROM leg or an enroute scale
//! reaches the HSI as the source stated it.

#![allow(clippy::expect_used, clippy::panic)]

use super::decode_state;
use super::tests::encode;
use crate::aircraft::{NavFromTo, NavScale};

#[test]
fn every_known_flag_and_scale_round_trips_the_nav_group() {
    let mut state = super::fixtures::full();
    for fromto in [NavFromTo::Off, NavFromTo::To, NavFromTo::From] {
        for scale in [NavScale::Enroute, NavScale::Terminal, NavScale::Approach] {
            let nav = state.nav.data.as_mut().expect("fixture has nav");
            nav.fromto = fromto;
            nav.scale = scale;
            let decoded = decode_state(&encode(&state)).expect("decodes").state;
            let got = decoded.nav.data.expect("nav present");
            assert_eq!((got.fromto, got.scale), (fromto, scale));
            assert_eq!(decoded.nav, state.nav, "{fromto:?} {scale:?}");
        }
    }
}
