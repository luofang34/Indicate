use crate::context::{HudContext, HudMode, HudProfile, HudProjection, WorldSource, hud_mode};
use crate::projection::{Camera, live};
use indicate_alerts::AlertOutput;
use indicate_instrument_descriptor::{
    BackgroundCapability, ConfigBlob, DesignFrame, GroupSet, PanelDescriptor, PanelDrawError,
    PanelSet,
};
use indicate_instrument_scene::{Anchor, LayerId, PaintMode, Rgba8, SceneWriter};
use indicate_instrument_state::{GroupId, PanelData, Sig, SignalStatus};
use indicate_instrument_symbology::{fmt_label, safety};

mod annunciation;
mod approach;
mod ladder;
mod readout;
mod recovery;
mod repeater;
mod rung;
mod tape;
mod targets;
mod weapon;

const HUD_GREEN: Rgba8 = Rgba8::rgba(90, 255, 130, 255);
const HALO: Rgba8 = Rgba8::rgba(0, 10, 6, 150);

/// The camera that the `hud-screen` descriptor declares to its host: 50 degrees.
pub const REFERENCE_SCREEN_FOV_RAD: f32 = 0.872_664_6;

/// The calibrated combiner field that the `hud-combiner` descriptor declares: 24 degrees.
pub const REFERENCE_COMBINER_FOV_RAD: f32 = 0.418_879;

const FRAME_LANDSCAPE: DesignFrame = DesignFrame {
    width: 1280.0,
    height: 720.0,
};
const FRAME_TABLET: DesignFrame = DesignFrame {
    width: 1024.0,
    height: 768.0,
};
const FRAME_PHONE: DesignFrame = DesignFrame {
    width: 844.0,
    height: 390.0,
};
const FRAME_PORTRAIT: DesignFrame = DesignFrame {
    width: 390.0,
    height: 844.0,
};
const FRAME_MIN: DesignFrame = DesignFrame {
    width: 390.0,
    height: 390.0,
};
const FRAME_MAX: DesignFrame = DesignFrame {
    width: 2560.0,
    height: 1600.0,
};
const FRAMES: &[DesignFrame] = &[
    FRAME_MIN,
    FRAME_PHONE,
    FRAME_PORTRAIT,
    FRAME_TABLET,
    FRAME_LANDSCAPE,
    FRAME_MAX,
];

const REQUIRED_GROUPS: GroupSet = GroupSet::of(&[
    GroupId::Air,
    GroupId::Kinematics,
    GroupId::Altitude,
    GroupId::Attitude,
    GroupId::Heading,
    GroupId::SpeedPresentation,
]);

const fn descriptor(
    id: &'static str,
    title: &'static str,
    draw: indicate_instrument_descriptor::DrawFn,
) -> PanelDescriptor {
    PanelDescriptor {
        id,
        title,
        required_layers: (1 << LayerId::Tapes.to_u8()) | (1 << LayerId::Annunciation.to_u8()),
        required_groups: REQUIRED_GROUPS,
        frame_min: FRAME_MIN,
        frame_max: FRAME_MAX,
        frame_step: (1.0, 1.0),
        aspect_min: 0.45,
        aspect_max: 2.4,
        canonical_frames: FRAMES,
        background: BackgroundCapability::NotUsed,
        config_schema: &[],
        group_regions: &[],
        extreme_states: &[],
        raster_baselines: &[],
        draw,
    }
}

/// The screen profile over a synthetic world drawn with [`REFERENCE_SCREEN_FOV_RAD`].
pub const HUD_SCREEN_DESCRIPTOR: PanelDescriptor =
    descriptor("hud-screen", "Screen head-up display", draw_screen);

/// The combiner profile with a calibration for [`REFERENCE_COMBINER_FOV_RAD`].
pub const HUD_COMBINER_DESCRIPTOR: PanelDescriptor =
    descriptor("hud-combiner", "Combiner head-up display", draw_combiner);

/// The combiner profile without a valid calibration: a non-conformal repeater.
pub const HUD_REPEATER_DESCRIPTOR: PanelDescriptor = descriptor(
    "hud-repeater",
    "Non-conformal flight repeater",
    draw_repeater,
);

/// Flight and simulated combat symbology over a host-rendered world on a phone, tablet, or computer.
pub const HUD_SCREEN_SET: PanelSet = PanelSet {
    id: "hud-screen",
    panels: &[HUD_SCREEN_DESCRIPTOR],
};

/// Flight symbology for a reflective combiner, with its calibration-loss repeater.
pub const HUD_COMBINER_SET: PanelSet = PanelSet {
    id: "hud-combiner",
    panels: &[HUD_COMBINER_DESCRIPTOR, HUD_REPEATER_DESCRIPTOR],
};

fn reference(profile: HudProfile, vertical_fov_rad: f32) -> HudContext {
    HudContext {
        profile,
        projection: Sig::valid(HudProjection {
            vertical_fov_rad,
            body_from_view: indicate_frames::Quat::IDENTITY,
            source: WorldSource::Synthetic,
        }),
        ..HudContext::default()
    }
}

fn draw_screen(
    data: &PanelData,
    config: &ConfigBlob<'_>,
    alerts: Option<&AlertOutput>,
    frame: DesignFrame,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    config.require_schema(&[])?;
    let context = reference(HudProfile::Screen, REFERENCE_SCREEN_FOV_RAD);
    draw_hud(data, &context, alerts, frame, scene)
}

fn draw_combiner(
    data: &PanelData,
    config: &ConfigBlob<'_>,
    alerts: Option<&AlertOutput>,
    frame: DesignFrame,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    config.require_schema(&[])?;
    let context = reference(HudProfile::Combiner, REFERENCE_COMBINER_FOV_RAD);
    draw_hud(data, &context, alerts, frame, scene)
}

fn draw_repeater(
    data: &PanelData,
    config: &ConfigBlob<'_>,
    alerts: Option<&AlertOutput>,
    frame: DesignFrame,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    config.require_schema(&[])?;
    draw_hud(data, &HudContext::default(), alerts, frame, scene)
}

/// Frame-relative positions shared by every element.
#[derive(Debug, Clone, Copy)]
struct Layout {
    width: f32,
    height: f32,
    centre: [f32; 2],
    /// Scale for text and stroke sizes.
    unit: f32,
    /// Half extents of the conformal field around the boresight.
    field: [f32; 2],
    /// A reflective combiner shows one colour and cannot show dark ink.
    combiner: bool,
}

impl Layout {
    fn new(frame: DesignFrame, profile: HudProfile) -> Self {
        let unit = (frame.width.min(frame.height) / 600.0).clamp(0.8, 2.4);
        let half_width = (frame.width * 0.36)
            .min(frame.width * 0.5 - 96.0 * unit)
            .min(frame.height * 0.45)
            .max(60.0 * unit);
        Self {
            width: frame.width,
            height: frame.height,
            centre: [frame.width * 0.5, frame.height * 0.5],
            unit,
            field: [half_width, frame.height * 0.36],
            combiner: profile == HudProfile::Combiner,
        }
    }

    /// The colour a profile can show: a combiner shows every symbol in its one colour.
    fn ink(&self, color: Rgba8) -> Rgba8 {
        if self.combiner { HUD_GREEN } else { color }
    }

    /// The contrast halo; a combiner cannot show it, so it is transparent there.
    fn halo(&self) -> Rgba8 {
        if self.combiner {
            Rgba8::rgba(0, 0, 0, 0)
        } else {
            HALO
        }
    }

    fn text(&self, size: f32) -> f32 {
        size * self.unit
    }
}

/// Draws the HUD for one frame.
///
/// The host projects nothing: every element is already in frame coordinates
/// for the camera that `context.projection` declares.
pub fn draw_hud(
    data: &PanelData,
    context: &HudContext,
    alerts: Option<&AlertOutput>,
    frame: DesignFrame,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let layout = Layout::new(frame, context.profile);
    let mode = hud_mode(data, context);
    let recovery = data.presentation.unusual;
    let camera = Camera::new(data, context.projection.value, frame);
    scene.begin_layer(LayerId::Attitude)?;
    match mode {
        HudMode::Conformal if recovery => {
            ladder::waterline(&camera, &layout, scene)?;
            recovery::draw(data, &layout, scene)?;
            ladder::bank_scale(data, &layout, scene)?;
        }
        HudMode::Conformal => ladder::draw(data, &camera, &layout, scene)?,
        HudMode::Repeater => repeater::draw(data, &layout, scene)?,
    }
    scene.end_layer(LayerId::Attitude)?;
    scene.begin_layer(LayerId::Tapes)?;
    readout::draw(data, &layout, scene)?;
    scene.end_layer(LayerId::Tapes)?;
    let runway = context.approach.as_ref().filter(|r| r.usable());
    let combat = context.profile == HudProfile::Screen;
    if mode == HudMode::Conformal && !recovery && (runway.is_some() || combat) {
        scene.begin_layer(LayerId::Guidance)?;
        if let Some(runway) = runway {
            approach::draw(
                data,
                runway,
                context.on_speed_aoa_rad,
                &camera,
                &layout,
                scene,
            )?;
        }
        if combat {
            targets::draw(context, &camera, &layout, scene)?;
            weapon::draw(data, context, &camera, &layout, scene)?;
        }
        scene.end_layer(LayerId::Guidance)?;
    }
    annunciation::draw(data, context, mode, alerts, &layout, scene)
}

/// Strokes a polyline twice: a dark halo for contrast, then the symbol colour.
fn halo_polyline(
    layout: &Layout,
    scene: &mut SceneWriter<'_>,
    points: &[[f32; 2]],
    color: Rgba8,
    width: f32,
) -> Result<(), PanelDrawError> {
    if !layout.combiner {
        scene.stroke(HALO, width + 2.0)?;
        scene.polyline(points)?;
    }
    scene.stroke(layout.ink(color), width)?;
    scene.polyline(points)?;
    Ok(())
}

fn status_color(layout: &Layout, status: SignalStatus) -> Rgba8 {
    layout.ink(match status {
        SignalStatus::Valid => HUD_GREEN,
        SignalStatus::Failed => safety::FAILURE_RED,
        _ => safety::CAUTION_AMBER,
    })
}

fn status_label(status: SignalStatus) -> &'static str {
    match status {
        SignalStatus::Valid => "",
        SignalStatus::Stale => "STALE",
        SignalStatus::Degraded => "CHECK",
        SignalStatus::Failed => "FAILED",
        SignalStatus::Missing => "MISSING",
    }
}

/// A displayed status: a value that is not finite reads as failed.
fn shown_status(signal: Sig<f32>) -> SignalStatus {
    if signal.status.shows_value() && (!signal.value.is_finite() || signal.value.abs() >= 1e6) {
        SignalStatus::Failed
    } else {
        signal.status
    }
}

#[cfg(test)]
mod approach_tests;
#[cfg(test)]
mod combat_tests;
#[cfg(test)]
mod ground_tests;
#[cfg(test)]
mod ladder_tests;
#[cfg(test)]
pub(crate) mod tests;
