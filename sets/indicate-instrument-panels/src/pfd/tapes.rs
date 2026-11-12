//! Speed and altitude tapes and the vertical-speed indicator.
//!
//! Tape scaling from the G5 proportions: speed ±25 kt over the panel
//! height (7.2 px/kt), altitude ±150 ft (1.2 px/ft), VSI ±1500 fpm full
//! scale.
//!
//! Each tape draws from its own module, because the two share no
//! geometry beyond the strip they run down. What stays here is exactly
//! that shared part: the vertical extent both span, and the pointed
//! readout each hangs at the pointer line.

use indicate_instrument_scene::{
    Anchor, PaintMode, SceneError, SceneWriter, nominal_text_ink_width, nominal_text_width,
};
use indicate_instrument_state::{ComparisonState, Sig, SignalStatus, SourcedFunction};
use indicate_instrument_symbology::{palette, safety, status_paint};

mod airspeed;
mod altitude;

pub use airspeed::speed_tape;
pub use altitude::{altitude_tape, vsi};

/// The pointer line: the height at which either tape reads its value.
const CENTER_Y: f32 = 180.0;

/// Top of the visible speed strip, below its true-airspeed box.
pub(super) const SPEED_TAPE_TOP: f32 = 25.0;

/// Top of the visible altitude strip, below its selection box.
pub(super) const ALTITUDE_TAPE_TOP: f32 = 24.0;

/// The foot both tapes end at. The readout boxes hang from it, and a
/// cue drawn past it would mark a value the tape beside it is not
/// showing.
pub(super) const TAPE_BOTTOM: f32 = 335.0;

/// Top edge of both pointed readout boxes.
pub(super) const READOUT_TOP: f32 = 155.0;

/// Bottom edge of both pointed readout boxes.
pub(super) const READOUT_BOTTOM: f32 = 205.0;

/// Clearance between a boxed readout's border and its text, each side.
/// The border is painted on the box edge, so text fitted to the full
/// box width would run into it.
const BOX_TEXT_INSET: f32 = 4.0;

/// Whether a vertically centered ladder label stays wholly inside its
/// visible tape and wholly clear of every band in `keep_out` (top,
/// bottom): the readout box, and the labels and flags drawn over the
/// tape. A `None` band is one that is not drawn this frame.
///
/// A label that crossed either would be cut by the tape edge or half
/// hidden under a box or another label, and a cut label can read as a
/// different number. Omitting it keeps every painted label whole.
fn ladder_label_fits(y: f32, size: f32, top: f32, keep_out: &[Option<(f32, f32)>]) -> bool {
    let half_ink = size / 2.0;
    let (ink_top, ink_bottom) = (y - half_ink, y + half_ink);
    ink_top >= top
        && ink_bottom <= TAPE_BOTTOM
        && keep_out
            .iter()
            .flatten()
            .all(|&(band_top, band_bottom)| ink_bottom <= band_top || ink_top >= band_bottom)
}

/// Row of the source label (`IAS1`, `ALT2`) the annunciation band draws
/// over each tape.
pub(crate) const SOURCE_LABEL_Y: f32 = 250.0;

/// Run size of the source label and of its `CMP` row.
pub(crate) const SOURCE_LABEL_SIZE: f32 = 12.0;

/// The band a tape's source label occupies when it draws: its own row,
/// and the `CMP` row under it on a sustained miscompare.
fn source_band<T: Copy>(selection: &SourcedFunction<T>) -> Option<(f32, f32)> {
    selection.selected?;
    let rows = if selection.state == ComparisonState::Miscompare {
        2.0
    } else {
        1.0
    };
    let top = SOURCE_LABEL_Y - SOURCE_LABEL_SIZE / 2.0;
    Some((top, top + rows * SOURCE_LABEL_SIZE))
}

/// Fits the text that [`status_paint::readout_box`] will paint inside a
/// box `box_width` wide, clear of its border.
fn fitted_box_text_size(box_width: f32, text: &str, preferred: f32, status: SignalStatus) -> f32 {
    fitted_readout_size(box_width - 2.0 * BOX_TEXT_INSET, text, preferred, status)
}

/// Fits the text that [`status_paint::readout_box`] will paint.
fn fitted_readout_size(width: f32, text: &str, preferred: f32, status: SignalStatus) -> f32 {
    let chars = if status.shows_value() {
        text.chars().count()
    } else {
        3
    };
    let advance = nominal_text_width(preferred, chars);
    if advance <= width {
        return preferred;
    }
    preferred * width / advance
}

/// Geometry of a pointed tape readout: the rectangular body spans
/// `far_x`..`near_x`, the tip at `tip_x` points toward the tape, and
/// the value is anchored at `text_x`, no larger than `preferred_size`.
pub(super) struct PointedBox {
    far_x: f32,
    near_x: f32,
    tip_x: f32,
    pub(super) text_x: f32,
    preferred_size: f32,
}

/// Airspeed readout: body at the panel's left edge, tip pointing right.
const IAS_READOUT: PointedBox = PointedBox {
    far_x: 2.0,
    near_x: 75.0,
    tip_x: 90.0,
    text_x: 40.0,
    preferred_size: 28.0,
};

/// Altitude readout: body at the panel's right edge, tip pointing left.
const ALT_READOUT: PointedBox = PointedBox {
    far_x: 478.0,
    near_x: 405.0,
    tip_x: 390.0,
    text_x: 442.0,
    preferred_size: 26.0,
};

/// Pointed value readout beside a tape. The run size shrinks
/// deterministically from `preferred_size` until the run's nominal ink
/// (the scene text-metrics contract every backend honors) fits the box
/// body, so a wide value — "10300", "-1030" — renders smaller, never
/// outside the box: an overflowing readout is silent display
/// corruption (DISP-02), which the box must make impossible for the
/// signal's whole representable range.
fn pointed_readout(
    scene: &mut SceneWriter<'_>,
    group: u8,
    sig: Sig<f32>,
    text: &str,
    geo: &PointedBox,
) -> Result<(), SceneError> {
    pointed_box(scene, sig, geo)?;
    // The dash path stays unclaimed on purpose: the claim rule covers
    // every visible run, and dashes ARE the honest degraded display.
    let shown = if sig.status.shows_value() {
        text
    } else {
        "---"
    };
    let size = fitted_text_size(geo, shown.chars().count());
    if sig.status.shows_value() {
        scene.text_attributed(group, geo.text_x, 180.0, size, Anchor::CENTER, shown)?;
    } else {
        scene.text(geo.text_x, 180.0, size, Anchor::CENTER, shown)?;
    }
    Ok(())
}

/// The pointed box frame and the value ink color shared by the tape
/// readouts: white for a shown value, red for the dashes.
fn pointed_box(
    scene: &mut SceneWriter<'_>,
    sig: Sig<f32>,
    geo: &PointedBox,
) -> Result<(), SceneError> {
    scene.fill_color(palette::BOX_BG)?;
    let border = status_paint::status_accent(sig.status).unwrap_or(palette::WHITE);
    scene.stroke(border, 2.0)?;
    scene.polygon(
        PaintMode::FillStroke,
        &[
            [geo.far_x, READOUT_TOP],
            [geo.near_x, READOUT_TOP],
            [geo.near_x, 168.0],
            [geo.tip_x, 180.0],
            [geo.near_x, 192.0],
            [geo.near_x, READOUT_BOTTOM],
            [geo.far_x, READOUT_BOTTOM],
        ],
    )?;
    scene.fill_color(if sig.status.shows_value() {
        palette::WHITE
    } else {
        safety::FAILURE_RED
    })?;
    Ok(())
}

/// Largest run size, capped at the box's preferred size, whose nominal
/// extents stay inside the box body from the box's text anchor: a
/// center anchor overhangs half the anchor width leftward and the ink
/// width minus that half rightward, and both extents scale linearly
/// with size, so the cap is a pure ratio.
pub(super) fn fitted_text_size(geo: &PointedBox, chars: usize) -> f32 {
    let body_left = geo.far_x.min(geo.near_x);
    let body_right = geo.far_x.max(geo.near_x);
    let width = nominal_text_width(geo.preferred_size, chars);
    let ink = nominal_text_ink_width(geo.preferred_size, chars);
    let left_need = width / 2.0;
    let right_need = ink - width / 2.0;
    let mut scale = 1.0f32;
    if left_need > geo.text_x - body_left {
        scale = scale.min((geo.text_x - body_left) / left_need);
    }
    if right_need > body_right - geo.text_x {
        scale = scale.min((body_right - geo.text_x) / right_need);
    }
    geo.preferred_size * scale.max(0.0)
}

/// Largest size, capped at the box's preferred size, whose whole
/// advance row stays inside the box body. The drum clips each column to
/// its own advance cell, so the row's advance extent is what must fit,
/// not just its ink: a window overhanging the body would stop being the
/// containment proof the digits behind it rely on.
pub(super) fn fitted_row_size(geo: &PointedBox, chars: usize) -> f32 {
    let body_left = geo.far_x.min(geo.near_x);
    let body_right = geo.far_x.max(geo.near_x);
    let half = nominal_text_width(geo.preferred_size, chars) / 2.0;
    let mut scale = 1.0f32;
    if half > geo.text_x - body_left {
        scale = scale.min((geo.text_x - body_left) / half);
    }
    if half > body_right - geo.text_x {
        scale = scale.min((body_right - geo.text_x) / half);
    }
    geo.preferred_size * scale.max(0.0)
}
