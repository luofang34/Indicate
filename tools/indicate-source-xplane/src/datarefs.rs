//! The datarefs a flight display subscribes to, and their place in the
//! `RREF` index space.

/// How often each dataref is requested, per second.
pub const SUBSCRIPTION_HZ: i32 = 20;

/// One subscribed dataref. The discriminant is the `RREF` index the
/// simulator echoes back, so a reply record names its dataref by
/// position here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Dataref {
    /// Pitch, degrees, nose up positive.
    PitchDeg = 0,
    /// Roll, degrees, right wing down positive.
    RollDeg = 1,
    /// True heading, degrees.
    HeadingTrueDeg = 2,
    /// Magnetic heading, degrees.
    HeadingMagDeg = 3,
    /// Roll rate about the body x axis, degrees per second.
    RollRateDps = 4,
    /// Pitch rate about the body y axis, degrees per second.
    PitchRateDps = 5,
    /// Yaw rate about the body z axis, degrees per second.
    YawRateDps = 6,
    /// Local OpenGL x, meters (east).
    LocalX = 7,
    /// Local OpenGL y, meters (up).
    LocalY = 8,
    /// Local OpenGL z, meters (south).
    LocalZ = 9,
    /// Local OpenGL x velocity, meters per second.
    LocalVx = 10,
    /// Local OpenGL y velocity, meters per second.
    LocalVy = 11,
    /// Local OpenGL z velocity, meters per second.
    LocalVz = 12,
    /// Indicated airspeed, knots.
    IasKt = 13,
    /// True airspeed, meters per second.
    TasMps = 14,
    /// Pilot-side indicated altitude, feet.
    AltitudeFt = 15,
    /// Pilot-side barometric setting, inches of mercury.
    BaroInHg = 16,
    /// Autopilot heading bug, degrees magnetic.
    HeadingBugMagDeg = 17,
    /// Autopilot altitude selection, feet.
    AltitudeSelFt = 18,
    /// Wind direction the wind comes from, degrees magnetic.
    WindFromMagDeg = 19,
    /// Wind speed, knots.
    WindSpeedKt = 20,
    /// Simulation running time, seconds; the simulation clock.
    SimTimeS = 21,
}

impl Dataref {
    /// Every subscribed dataref, in index order.
    pub const ALL: [Dataref; 22] = [
        Dataref::PitchDeg,
        Dataref::RollDeg,
        Dataref::HeadingTrueDeg,
        Dataref::HeadingMagDeg,
        Dataref::RollRateDps,
        Dataref::PitchRateDps,
        Dataref::YawRateDps,
        Dataref::LocalX,
        Dataref::LocalY,
        Dataref::LocalZ,
        Dataref::LocalVx,
        Dataref::LocalVy,
        Dataref::LocalVz,
        Dataref::IasKt,
        Dataref::TasMps,
        Dataref::AltitudeFt,
        Dataref::BaroInHg,
        Dataref::HeadingBugMagDeg,
        Dataref::AltitudeSelFt,
        Dataref::WindFromMagDeg,
        Dataref::WindSpeedKt,
        Dataref::SimTimeS,
    ];

    /// The `RREF` index this dataref is subscribed under.
    pub const fn index(self) -> i32 {
        self as i32
    }

    /// The dataref by its `RREF` index, or `None` for an index this
    /// build never subscribed: a reply for it is counted and skipped.
    pub fn from_index(index: i32) -> Option<Self> {
        usize::try_from(index)
            .ok()
            .and_then(|i| Self::ALL.get(i).copied())
    }

    /// The simulator's path for this dataref.
    pub const fn path(self) -> &'static str {
        match self {
            Dataref::PitchDeg => "sim/flightmodel/position/theta",
            Dataref::RollDeg => "sim/flightmodel/position/phi",
            Dataref::HeadingTrueDeg => "sim/flightmodel/position/psi",
            Dataref::HeadingMagDeg => "sim/flightmodel/position/magpsi",
            Dataref::RollRateDps => "sim/flightmodel/position/P",
            Dataref::PitchRateDps => "sim/flightmodel/position/Q",
            Dataref::YawRateDps => "sim/flightmodel/position/R",
            Dataref::LocalX => "sim/flightmodel/position/local_x",
            Dataref::LocalY => "sim/flightmodel/position/local_y",
            Dataref::LocalZ => "sim/flightmodel/position/local_z",
            Dataref::LocalVx => "sim/flightmodel/position/local_vx",
            Dataref::LocalVy => "sim/flightmodel/position/local_vy",
            Dataref::LocalVz => "sim/flightmodel/position/local_vz",
            Dataref::IasKt => "sim/flightmodel/position/indicated_airspeed",
            Dataref::TasMps => "sim/flightmodel/position/true_airspeed",
            Dataref::AltitudeFt => "sim/cockpit2/gauges/indicators/altitude_ft_pilot",
            Dataref::BaroInHg => "sim/cockpit/misc/barometer_setting",
            Dataref::HeadingBugMagDeg => "sim/cockpit/autopilot/heading_mag",
            Dataref::AltitudeSelFt => "sim/cockpit/autopilot/altitude",
            Dataref::WindFromMagDeg => "sim/cockpit2/gauges/indicators/wind_heading_deg_mag",
            Dataref::WindSpeedKt => "sim/cockpit2/gauges/indicators/wind_speed_kts",
            Dataref::SimTimeS => "sim/time/total_running_time_sec",
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]

    use super::Dataref;

    #[test]
    fn every_dataref_sits_at_its_own_index_and_round_trips() {
        for (position, dataref) in Dataref::ALL.iter().enumerate() {
            let index = i32::try_from(position).expect("small");
            assert_eq!(dataref.index(), index);
            assert_eq!(Dataref::from_index(index), Some(*dataref));
            assert!(dataref.path().starts_with("sim/"));
        }
        assert_eq!(Dataref::from_index(-1), None);
        assert_eq!(Dataref::from_index(99), None);
    }
}
