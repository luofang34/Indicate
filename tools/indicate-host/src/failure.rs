//! The failure page: what replaces a frame the host could not make
//! (AIR-UNAV-007).

use indicate_instrument_descriptor::DesignFrame;
use indicate_instrument_scene::{LayerId, SceneError, SceneWriter};
use indicate_instrument_symbology::status_paint::draw_red_x;

/// Writes a failure page into `scene_buf`: a full-frame red X in the
/// `Failure` band with `label` across it, and nothing else. Returns the
/// encoded length. The label must stay inside the controlled glyph
/// vocabulary; the shell passes one of its own display reasons, in
/// uppercase.
pub fn failure_page(
    scene_buf: &mut [u8],
    frame: DesignFrame,
    label: &str,
) -> Result<usize, SceneError> {
    let mut writer = SceneWriter::new(scene_buf)?;
    writer.begin_layer(LayerId::Failure)?;
    draw_red_x(&mut writer, 0.0, 0.0, frame.width, frame.height, label)?;
    writer.end_layer(LayerId::Failure)?;
    Ok(writer.finish())
}
