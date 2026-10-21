//! Head-up flight and weapon symbology for flat-screen simulator views.
//!
//! Both profiles share a drawing path with an independent observation view.
//! The host declares aligned view geometry in [`HudContext::projection`].
//!
//! - [`HudProfile::Screen`] draws over a world that the host renders through
//!   that camera, on a phone, tablet, or computer. It adds simulated contacts
//!   and weapon cues.
//! - [`HudProfile::Combiner`] is a reflective combiner design: one colour, no
//!   dark ink, and flight symbology only. Its cues are conformal (`HUD-SIM`)
//!   only with a valid calibration; otherwise it is a
//!   `NON-CONFORMAL / NOT A HUD` repeater.
#![no_std]

#[cfg(test)]
extern crate std;

mod context;
mod panel;
mod profile;
mod projection;
mod runway;
mod target;

pub use context::{
    EnergyState, HudContext, HudMode, HudProfile, HudProjection, MissileEnvelope, WeaponEnvelope,
    WeaponMode, WorldSource, hud_mode,
};
pub use panel::{
    HUD_COMBINER_DESCRIPTOR, HUD_COMBINER_SET, HUD_REPEATER_DESCRIPTOR, HUD_SCREEN_DESCRIPTOR,
    HUD_SCREEN_SET, REFERENCE_COMBINER_FOV_RAD, REFERENCE_SCREEN_FOV_RAD, draw_hud,
};
pub use profile::{FIGHTER_LIMITS, fighter_profile};
pub use projection::{Camera, ScreenPoint};
pub use runway::Runway;
pub use target::{Contact, Domain, Identity, MAX_CONTACTS, TargetPicture};
