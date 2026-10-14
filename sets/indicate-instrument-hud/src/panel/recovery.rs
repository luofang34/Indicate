//! The compressed recovery ladder for unusual attitude.
//!
//! The ladder fits ±90 degrees into the field, so the horizon and the sky and
//! ground hemispheres stay in view in any attitude. It is not registered to
//! the outside world; the annunciation names that state.
use super::rung::{self, Rung};
use super::*;
use indicate_instrument_state::ChevronSense;

/// The top of the compressed ladder; the band above it holds the mode labels.
fn top(layout: &Layout) -> f32 {
    layout.centre[1] - layout.field[1] + 84.0 * layout.unit
}

/// The first label line of the band above the compressed ladder.
pub(super) fn label_band(layout: &Layout) -> f32 {
    layout.centre[1] - layout.field[1] + 40.0 * layout.unit
}

/// Pixels per degree; 90 degrees of pitch fits inside the ladder's shorter half.
pub(super) fn scale(layout: &Layout) -> f32 {
    let above = layout.centre[1] - top(layout);
    above.min(layout.field[1]) * 0.9 / 90.0
}

pub(super) fn draw(
    data: &PanelData,
    layout: &Layout,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let (Some(roll), Some(pitch)) = (live(data.roll_rad), live(data.pitch_rad)) else {
        return Ok(());
    };
    let u = layout.unit;
    let [cx, cy] = layout.centre;
    let [hw, hh] = layout.field;
    let per_degree = scale(layout);
    let pitch_degrees = pitch.to_degrees();
    scene.save()?;
    let top = top(layout);
    scene.clip_rect(cx - hw, top, hw * 2.0, cy + hh - top)?;
    scene.translate(cx, cy)?;
    scene.rotate(-roll)?;
    // Every 30 degrees: compression makes 10-degree rungs too dense to read.
    for degrees in (-90_i32..=90).step_by(30) {
        let y = (pitch_degrees - degrees as f32) * per_degree;
        if libm::fabsf(y) > hh * 1.4 {
            continue;
        }
        if degrees.abs() == 90 {
            rung::pole([0.0, y], degrees > 0, layout, scene)?;
            continue;
        }
        let toward = if degrees > 0 { 1.0 } else { -1.0 };
        Rung {
            centre: [0.0, y],
            along: [1.0, 0.0],
            toward: [0.0, toward],
            half: if degrees == 0 { hw * 1.5 } else { hw * 0.36 },
            gap: if degrees == 0 { 30.0 * u } else { 20.0 * u },
        }
        .draw(degrees, layout, scene)?;
    }
    chevrons(data, layout, scene)?;
    scene.restore()?;
    Ok(())
}

/// Two chevrons on the far side of the boresight, pointing toward the horizon.
fn chevrons(
    data: &PanelData,
    layout: &Layout,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let toward = match data.presentation.chevrons {
        Some(ChevronSense::HorizonBelow) => 1.0,
        Some(ChevronSense::HorizonAbove) => -1.0,
        None => return Ok(()),
    };
    let u = layout.unit;
    for offset in [34.0, 58.0] {
        let y = -toward * offset * u;
        let points = [[-26.0 * u, y], [0.0, y + toward * 16.0 * u], [26.0 * u, y]];
        halo_polyline(layout, scene, &points, safety::FAILURE_RED, 3.0 * u)?;
    }
    Ok(())
}
