//! Per-frame host input that is independent of the aircraft state ABI.
use crate::runway::Runway;
use crate::target::TargetPicture;
use indicate_frames::{Quat, ROTATION_NORM_TOLERANCE};
use indicate_instrument_descriptor::{AuxChannelId, PanelDrawError};
use indicate_instrument_state::{PanelData, Sig, SignalStatus};

/// Fixed byte length of the compact [`HudContext`] wire header on `AuxChannelId::HUD_CONTEXT`.
pub const HUD_CONTEXT_WIRE_BYTES: usize = 24;

/// The display the HUD is drawn for. Each profile answers to its own requirements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HudProfile {
    /// A reflective combiner design: one colour, no dark ink, flight symbology
    /// only, and conformal cues only with a valid calibration.
    #[default]
    Combiner,
    /// A phone, tablet, or computer screen that shows a host-rendered world,
    /// with simulated contacts and weapon cues.
    Screen,
}

/// Where the outside-world image behind the HUD comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WorldSource {
    /// The host renders the world itself through the declared camera, so
    /// world and symbology share one projection by construction.
    #[default]
    Synthetic,
    /// Camera video; registration needs a valid camera calibration.
    Video,
}

/// The host camera through which the outside world is drawn.
///
/// Resolved display geometry with square pixels and a centred principal point.
/// The host must align the view, aircraft state, and image before marking this geometry valid.
/// A fixed installation may use its calibrated rotation without claiming a measured camera attitude.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HudProjection {
    /// Full vertical field of view of the frame, radians.
    pub vertical_fov_rad: f32,
    /// Rotation from view forward/right/down coordinates into aircraft body coordinates.
    /// This is independent of the aircraft attitude in the navigation frame.
    pub body_from_view: Quat,
    /// Origin of the outside-world image.
    pub source: WorldSource,
}

impl HudProjection {
    /// A field of view that a frame can use: finite and between 10 and 150 degrees.
    pub fn usable(&self) -> bool {
        self.vertical_fov_rad.is_finite()
            && (0.17..=2.62).contains(&self.vertical_fov_rad)
            && self
                .body_from_view
                .renormalized(ROTATION_NORM_TOLERANCE)
                .is_ok()
    }
}

/// The weapon symbology that the operator selected; it never fires a weapon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WeaponMode {
    /// Flight symbology and contacts only.
    #[default]
    Navigation,
    /// Gun funnel, with the lead pipper on a designated contact in range.
    Gun,
    /// Infrared short-range missile: seeker circle, lock, and launch envelope.
    ShortRangeMissile,
    /// Radar medium-range missile: steering circle, steering dot, and launch zone.
    MediumRangeMissile,
    /// Unguided rockets: a continuously computed impact point on the terrain.
    Rockets,
    /// Laser-guided bomb on a designated surface target: steering line and release cue.
    GuidedBomb,
    /// Directed-energy weapon: aim reticle, range, and heat.
    DirectedEnergy,
}

/// Simulated limits of one missile type.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MissileEnvelope {
    /// Smallest launch range, metres.
    pub min_range_m: f32,
    /// Range inside which a manoeuvring target cannot escape, metres.
    pub no_escape_range_m: f32,
    /// Largest launch range, metres.
    pub max_range_m: f32,
    /// Seeker field (short range) or allowable steering error (medium range), radians.
    pub circle_half_angle_rad: f32,
    /// Largest angle off the nose at which the seeker or radar holds a lock, radians.
    pub lock_limit_rad: f32,
}

/// Simulated weapon limits that the host supplies with the weapon mode.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeaponEnvelope {
    /// Projectile speed relative to the aircraft, metres per second.
    pub muzzle_speed_mps: f32,
    /// Largest range at which the gun cue shows a firing solution, metres.
    pub gun_max_range_m: f32,
    /// Gun line above the body x axis, radians; the gun cross marks it.
    pub gun_elevation_rad: f32,
    /// Wingspan the gun funnel is sized for, metres.
    pub target_wingspan_m: f32,
    /// Infrared short-range missile.
    pub short_range: MissileEnvelope,
    /// Radar medium-range missile.
    pub medium_range: MissileEnvelope,
    /// Rocket speed relative to the aircraft at launch, metres per second.
    pub rocket_speed_mps: f32,
    /// Largest slant range at which the rocket impact point is a firing cue, metres.
    pub rocket_max_range_m: f32,
    /// Ratio of a guided bomb's reach to the ballistic range of a free fall.
    pub bomb_glide_ratio: f32,
    /// Largest range of the directed-energy weapon, metres.
    pub energy_max_range_m: f32,
    /// Half-angle of the directed-energy aim reticle, radians.
    pub energy_half_angle_rad: f32,
}

impl Default for WeaponEnvelope {
    fn default() -> Self {
        Self {
            muzzle_speed_mps: 1030.0,
            gun_max_range_m: 1200.0,
            gun_elevation_rad: 0.035,
            // 35 feet, the usual funnel setting for a fighter-sized target.
            target_wingspan_m: 10.67,
            short_range: MissileEnvelope {
                min_range_m: 300.0,
                no_escape_range_m: 1800.0,
                max_range_m: 3500.0,
                circle_half_angle_rad: 0.044,
                lock_limit_rad: 0.5,
            },
            medium_range: MissileEnvelope {
                min_range_m: 1000.0,
                no_escape_range_m: 7000.0,
                max_range_m: 18000.0,
                circle_half_angle_rad: 0.1,
                lock_limit_rad: 1.05,
            },
            rocket_speed_mps: 600.0,
            rocket_max_range_m: 5000.0,
            bomb_glide_ratio: 1.6,
            energy_max_range_m: 4000.0,
            energy_half_angle_rad: 0.012,
        }
    }
}

impl WeaponEnvelope {
    /// The gun line as a body-axis direction (forward, right, down).
    pub fn gun_line(&self) -> [f32; 3] {
        let e = self.gun_elevation_rad;
        [libm::cosf(e), 0.0, -libm::sinf(e)]
    }

    /// The envelope of the selected missile, if a missile is selected.
    pub fn missile(&self, mode: WeaponMode) -> Option<&MissileEnvelope> {
        match mode {
            WeaponMode::ShortRangeMissile => Some(&self.short_range),
            WeaponMode::MediumRangeMissile => Some(&self.medium_range),
            _ => None,
        }
    }
}

/// The directed-energy weapon's state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnergyState {
    /// Heat as a fraction of the cut-out limit, 0 to 1.
    pub heat: f32,
    /// The beam is on.
    pub firing: bool,
    /// The weapon is cooling after a cut-out and cannot fire.
    pub locked_out: bool,
}

/// Everything the host supplies for one frame besides aircraft state and alerts.
#[derive(Debug, Clone, Copy)]
pub struct HudContext {
    /// The display the HUD is drawn for.
    pub profile: HudProfile,
    /// The host camera. For a combiner, or for [`WorldSource::Video`], the
    /// status is the design-eye, boresight, and time-alignment calibration. For
    /// a synthetic world on a screen, it states that the world and this frame
    /// come from the same camera and sample.
    pub projection: Sig<HudProjection>,
    /// Selected weapon symbology.
    pub weapon: WeaponMode,
    /// Simulated weapon limits.
    pub envelope: WeaponEnvelope,
    /// Contacts relative to the aircraft.
    pub targets: TargetPicture,
    /// Body rates (roll, pitch, yaw), radians per second, from the same
    /// sample as the aircraft state. The gun funnel needs them.
    pub body_rates_rps: Sig<[f32; 3]>,
    /// The landing runway, when an approach is selected.
    pub approach: Option<Runway>,
    /// Terrain elevation around the weapon impact point, metres in the
    /// altitude's datum; rockets and bombs need it.
    pub terrain_elevation_m: Sig<f32>,
    /// Directed-energy weapon state, from the host's simulation.
    pub energy: Option<EnergyState>,
    /// On-speed angle-of-attack band for the approach, fast edge then slow
    /// edge, radians. The bracket is body-fixed: with the wings level, the
    /// flight path marker sits inside it exactly when the angle of attack does.
    pub on_speed_aoa_rad: Option<[f32; 2]>,
}

impl Default for HudContext {
    fn default() -> Self {
        Self {
            profile: HudProfile::Combiner,
            projection: Sig::with_status(
                HudProjection {
                    vertical_fov_rad: 0.0,
                    body_from_view: indicate_frames::Quat::IDENTITY,
                    source: WorldSource::Video,
                },
                SignalStatus::Missing,
            ),
            weapon: WeaponMode::Navigation,
            envelope: WeaponEnvelope::default(),
            targets: TargetPicture::default(),
            body_rates_rps: Sig::with_status([0.0; 3], SignalStatus::Missing),
            approach: None,
            on_speed_aoa_rad: None,
            terrain_elevation_m: Sig::with_status(0.0, SignalStatus::Missing),
            energy: None,
        }
    }
}

/// The conformality mode of one frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HudMode {
    /// Conformal cues registered to the declared host camera.
    Conformal,
    /// Uncalibrated or missing camera: a repeater layout without conformal claims.
    Repeater,
}

/// Selects the mode from the declared projection alone; flight data never upgrades it.
pub fn hud_mode(_data: &PanelData, context: &HudContext) -> HudMode {
    let projection = context.projection;
    if projection.status == SignalStatus::Valid && projection.value.usable() {
        HudMode::Conformal
    } else {
        HudMode::Repeater
    }
}

fn status_to_u8(status: SignalStatus) -> u8 {
    match status {
        SignalStatus::Valid => 0,
        SignalStatus::Degraded => 1,
        SignalStatus::Stale => 2,
        SignalStatus::Missing => 3,
        SignalStatus::Failed => 4,
    }
}

fn status_from_u8(raw: u8) -> Result<SignalStatus, PanelDrawError> {
    match raw {
        0 => Ok(SignalStatus::Valid),
        1 => Ok(SignalStatus::Degraded),
        2 => Ok(SignalStatus::Stale),
        3 => Ok(SignalStatus::Missing),
        4 => Ok(SignalStatus::Failed),
        _ => Err(PanelDrawError::InvalidAux(AuxChannelId::HUD_CONTEXT.0)),
    }
}

fn read_f32_le(bytes: &[u8], offset: usize) -> Result<f32, PanelDrawError> {
    let end = offset
        .checked_add(4)
        .ok_or(PanelDrawError::InvalidAux(AuxChannelId::HUD_CONTEXT.0))?;
    let slice = bytes
        .get(offset..end)
        .ok_or(PanelDrawError::InvalidAux(AuxChannelId::HUD_CONTEXT.0))?;
    let arr: [u8; 4] = slice
        .try_into()
        .map_err(|_| PanelDrawError::InvalidAux(AuxChannelId::HUD_CONTEXT.0))?;
    Ok(f32::from_le_bytes(arr))
}

impl HudContext {
    /// Encodes the compact 24-byte `AuxChannelId::HUD_CONTEXT` wire payload.
    #[must_use]
    pub fn encode_aux(&self) -> [u8; HUD_CONTEXT_WIRE_BYTES] {
        let mut out = [0u8; HUD_CONTEXT_WIRE_BYTES];
        if let Some(slot) = out.first_mut() {
            *slot = match self.profile {
                HudProfile::Combiner => 0,
                HudProfile::Screen => 1,
            };
        }
        if let Some(slot) = out.get_mut(1) {
            *slot = status_to_u8(self.projection.status);
        }
        let fov = self.projection.value.vertical_fov_rad.to_le_bytes();
        let q = self.projection.value.body_from_view;
        let qw = q.w.to_le_bytes();
        let qx = q.x.to_le_bytes();
        let qy = q.y.to_le_bytes();
        let qz = q.z.to_le_bytes();
        if let Some(dst) = out.get_mut(2..6) {
            dst.copy_from_slice(&fov);
        }
        if let Some(dst) = out.get_mut(6..10) {
            dst.copy_from_slice(&qw);
        }
        if let Some(dst) = out.get_mut(10..14) {
            dst.copy_from_slice(&qx);
        }
        if let Some(dst) = out.get_mut(14..18) {
            dst.copy_from_slice(&qy);
        }
        if let Some(dst) = out.get_mut(18..22) {
            dst.copy_from_slice(&qz);
        }
        if let Some(slot) = out.get_mut(22) {
            *slot = match self.projection.value.source {
                WorldSource::Synthetic => 0,
                WorldSource::Video => 1,
            };
        }
        if let Some(slot) = out.get_mut(23) {
            *slot = match self.weapon {
                WeaponMode::Navigation => 0,
                WeaponMode::Gun => 1,
                WeaponMode::ShortRangeMissile => 2,
                WeaponMode::MediumRangeMissile => 3,
                WeaponMode::Rockets => 4,
                WeaponMode::GuidedBomb => 5,
                WeaponMode::DirectedEnergy => 6,
            };
        }
        out
    }

    /// Decodes a compact 24-byte `AuxChannelId::HUD_CONTEXT` wire payload fail-closed.
    pub fn decode_aux(bytes: &[u8]) -> Result<Self, PanelDrawError> {
        if bytes.len() != HUD_CONTEXT_WIRE_BYTES {
            return Err(PanelDrawError::InvalidAux(AuxChannelId::HUD_CONTEXT.0));
        }
        let profile = match *bytes
            .first()
            .ok_or(PanelDrawError::InvalidAux(AuxChannelId::HUD_CONTEXT.0))?
        {
            0 => HudProfile::Combiner,
            1 => HudProfile::Screen,
            _ => return Err(PanelDrawError::InvalidAux(AuxChannelId::HUD_CONTEXT.0)),
        };
        let status = status_from_u8(
            *bytes
                .get(1)
                .ok_or(PanelDrawError::InvalidAux(AuxChannelId::HUD_CONTEXT.0))?,
        )?;
        let vertical_fov_rad = read_f32_le(bytes, 2)?;
        let body_from_view = Quat {
            w: read_f32_le(bytes, 6)?,
            x: read_f32_le(bytes, 10)?,
            y: read_f32_le(bytes, 14)?,
            z: read_f32_le(bytes, 18)?,
        };
        let source = match *bytes
            .get(22)
            .ok_or(PanelDrawError::InvalidAux(AuxChannelId::HUD_CONTEXT.0))?
        {
            0 => WorldSource::Synthetic,
            1 => WorldSource::Video,
            _ => return Err(PanelDrawError::InvalidAux(AuxChannelId::HUD_CONTEXT.0)),
        };
        let weapon = match *bytes
            .get(23)
            .ok_or(PanelDrawError::InvalidAux(AuxChannelId::HUD_CONTEXT.0))?
        {
            0 => WeaponMode::Navigation,
            1 => WeaponMode::Gun,
            2 => WeaponMode::ShortRangeMissile,
            3 => WeaponMode::MediumRangeMissile,
            4 => WeaponMode::Rockets,
            5 => WeaponMode::GuidedBomb,
            6 => WeaponMode::DirectedEnergy,
            _ => return Err(PanelDrawError::InvalidAux(AuxChannelId::HUD_CONTEXT.0)),
        };
        Ok(Self {
            profile,
            projection: Sig::with_status(
                HudProjection {
                    vertical_fov_rad,
                    body_from_view,
                    source,
                },
                status,
            ),
            weapon,
            ..Self::default()
        })
    }
}
