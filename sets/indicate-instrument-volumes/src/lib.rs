//! Reference 3D volumetric instrument set (`RenderSpace::Volume3D`, `VOL-01`, `IXN-01`).
//!
//! Exports three bounded, `#![no_std]` volumetric instruments in [`VOLUMES_SET`]:
//! - [`TERRAIN_VOLUME_DESCRIPTOR`] (`terrain-volume`): 3D terrain relief, flight path,
//!   ownship symbol, attributed altitude/groundspeed readouts, and waypoint/altitude handles.
//! - [`GLOBE_VOLUME_DESCRIPTOR`] (`globe-volume`): 3D planetary globe, latitude/longitude
//!   graticule rings, great-circle track, attributed heading/distance readouts, and waypoint handles.
//! - [`ORRERY_VOLUME_DESCRIPTOR`] (`orrery-volume`): 3D celestial system, orbital rings,
//!   time-scrubbable transfer arc, planetary spheres, and manoeuvre-node handles.

#![no_std]

#[cfg(test)]
extern crate std;

mod aux_wire;
mod globe;
mod orrery;
mod shared;
mod terrain;

#[cfg(test)]
mod tests;

pub use aux_wire::{
    CELESTIAL_EPHEMERIS_WIRE_BYTES, CelestialEphemerisAux, GLOBE_NAVIGATION_WIRE_BYTES,
    GlobeNavigationAux, TERRAIN_PATCH_HEADER_BYTES, TerrainPatchAux,
};
pub use globe::{GLOBE_VOLUME_DESCRIPTOR, draw_globe_volume};
pub use orrery::{ORRERY_VOLUME_DESCRIPTOR, draw_orrery_volume};
pub use shared::{
    ADJUST_MANOEUVRE_NODE, CANONICAL_EXTENT, CANONICAL_EXTENTS, MAX_EXTENT, MIN_EXTENT,
    SELECT_WAYPOINT_3D, VOLUMES_NAMESPACE, VOLUMES_PANELS, VOLUMES_SCENE_DIGEST, VOLUMES_SET,
};
pub use terrain::{TERRAIN_VOLUME_DESCRIPTOR, draw_terrain_volume};

/// Every instrument set exported by this crate (`VOL-01`).
pub const ALL_SETS: &[indicate_instrument_descriptor::InstrumentSet] = &[VOLUMES_SET];
