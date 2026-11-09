//! Whole scenes: the full dial a 2D backend draws, and the face-only
//! scene a 3D backend rasterises into its texture.
//!
//! Bands: the frame cover in `Background`; the face, then the drums,
//! then the pointers in `Tapes`, the air-data band; the failure flag in
//! `Annunciation`, the band composition protects. The dial needs no new
//! band. Each moving part is isolated by its own save, transform, and
//! restore inside `Tapes`, and a 3D backend reads the moving parts from
//! the physical build rather than from bands.

use indicate_instrument_descriptor::{DesignFrame, PanelDrawError};
use indicate_instrument_scene::{LayerId, PaintMode, SceneWriter};
use indicate_instrument_symbology::palette;

use crate::face::{Placement, draw_face};
use crate::moving::{draw_flag, draw_moving_parts};
use crate::pose::DialPose;
use crate::spec::{AnalogDialSpec, Lighting};

fn cover(frame: DesignFrame, scene: &mut SceneWriter<'_>) -> Result<(), PanelDrawError> {
    scene.begin_layer(LayerId::Background)?;
    scene.fill_color(palette::BLACK)?;
    scene.rect(PaintMode::Fill, 0.0, 0.0, frame.width, frame.height)?;
    scene.end_layer(LayerId::Background)?;
    Ok(())
}

/// The whole dial for `pose`, centred in `frame`.
///
/// `spec` is assumed to have passed [`AnalogDialSpec::check`].
pub fn draw_dial(
    spec: &AnalogDialSpec<'_>,
    pose: &DialPose,
    lighting: Lighting,
    frame: DesignFrame,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let place = Placement::centred(spec, frame);
    cover(frame, scene)?;
    scene.begin_layer(LayerId::Tapes)?;
    draw_face(spec, lighting, &place, scene)?;
    draw_moving_parts(spec, pose, lighting, &place, scene)?;
    scene.end_layer(LayerId::Tapes)?;
    scene.begin_layer(LayerId::Annunciation)?;
    if pose.flag
        && let Some(flag) = &spec.off_flag
    {
        draw_flag(flag, &place, scene)?;
    }
    scene.end_layer(LayerId::Annunciation)?;
    Ok(())
}

/// The static face alone, centred in `frame`: what a 3D backend
/// rasterises once per lighting into the gauge's face texture. It
/// emits the bands [`crate::FACE_LAYERS`] names and nothing that moves.
pub fn draw_face_scene(
    spec: &AnalogDialSpec<'_>,
    lighting: Lighting,
    frame: DesignFrame,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let place = Placement::centred(spec, frame);
    cover(frame, scene)?;
    scene.begin_layer(LayerId::Tapes)?;
    draw_face(spec, lighting, &place, scene)?;
    scene.end_layer(LayerId::Tapes)?;
    Ok(())
}
