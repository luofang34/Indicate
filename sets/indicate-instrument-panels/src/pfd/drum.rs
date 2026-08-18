//! The altitude readout's rolling-digit drum.
//!
//! The pointed box shows the altitude as digit columns: the final digit
//! pair steps in 20 ft faces (`00/20/40/60/80`) and scrolls through a
//! clipped window, and the hundreds column rolls only while the pair
//! crosses its 80→00 boundary, so vertical rate reads in the readout
//! itself — the drum's whole reason for being.
//!
//! The drum is a pure function of the altitude value: every position
//! derives from `value mod step`, never from a clock. Two backends
//! handed the same snapshot place every digit identically, which is
//! what keeps the raster baselines and the cross-shell digest meaning
//! anything.

use indicate_instrument_scene::{Anchor, SceneError, SceneWriter, nominal_text_width};
use indicate_instrument_symbology::fmt_label;

use super::tapes::{PointedBox, fitted_row_size};

/// Feet per face step of the final digit pair.
const STEP_FT: f32 = 20.0;
/// Feet per pair cycle: the pair wraps 80→00 at each hundred.
const PAIR_CYCLE_FT: f32 = 100.0;
/// Text line of the pointed readout box interior.
const CENTER_Y: f32 = 180.0;
/// The pair faces in drum order.
const PAIRS: [&str; 5] = ["00", "20", "40", "60", "80"];
/// Digit faces for the rolling hundreds column.
const DIGITS: [&str; 10] = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];

/// The drum decomposition of one altitude value: the pure value →
/// position map the panel-purity model requires.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Drum {
    /// A negative altitude prefixes the columns with `-`.
    pub negative: bool,
    /// `floor(|ft| / 1000)`: the fixed leading digits; 0 suppresses
    /// them so a sub-1,000 ft value does not read with a leading zero.
    pub leading: i64,
    /// The hundreds column's current digit.
    pub hundreds: u8,
    /// The hundreds column's roll toward the next digit: 0 while
    /// parked, moving only through the pair's 80→00 step.
    pub hundreds_roll: f32,
    /// Whether the hundreds column is drawn at all: a parked leading
    /// zero is suppressed so 80 ft never reads as "080".
    pub hundreds_drawn: bool,
    /// The pair's current face as an index into [`PAIRS`].
    pub pair: usize,
    /// The pair's scroll fraction toward the next face.
    pub pair_frac: f32,
}

/// Decomposes an altitude into drum positions. Pure: the same value
/// decomposes identically on every call and on every backend.
pub(super) fn drum_of(value_ft: f32) -> Drum {
    // Sign plus magnitude: the drum rolls the magnitude, so the
    // negative-altitude path is the same columns behind a `-`.
    let a = value_ft.abs();
    let n = a / STEP_FT;
    let base = libm::floorf(n);
    let n1 = a / PAIR_CYCLE_FT;
    let hbase = libm::floorf(n1);
    // The hundreds column rolls in lockstep with the pair's last face
    // (80→00), so the pair's 9→0 boundary carries one column up.
    let hundreds_roll = ((n1 - hbase - 0.8) * 5.0).clamp(0.0, 1.0);
    Drum {
        negative: value_ft < 0.0,
        leading: libm::floorf(a / 1000.0) as i64,
        hundreds: (hbase as i64).rem_euclid(10) as u8,
        hundreds_roll,
        hundreds_drawn: hbase > 0.0 || hundreds_roll > 0.0,
        pair: (base as i64).rem_euclid(5) as usize,
        pair_frac: n - base,
    }
}

/// Draws the drum into the pointed box's body for a shown value. Every
/// run carries `group`'s claim — a rolling digit is still a numeral,
/// and the claim rule's totality covers even clipped-out runs. The
/// column count feeds the row fit, so a wide value shrinks instead of
/// overflowing the box (DISP-02).
pub(super) fn draw(
    scene: &mut SceneWriter<'_>,
    group: u8,
    value_ft: f32,
    geo: &PointedBox,
) -> Result<(), SceneError> {
    let drum = drum_of(value_ft);
    let prefix = match (drum.negative, drum.leading) {
        (true, l) if l > 0 => fmt_label!(24, "-{l}"),
        (true, _) => fmt_label!(24, "-"),
        (false, l) if l > 0 => fmt_label!(24, "{l}"),
        (false, _) => fmt_label!(24, ""),
    };
    let fixed = prefix.as_str().chars().count();
    let cols = fixed + usize::from(drum.hundreds_drawn) + 2;
    let size = fitted_row_size(geo, cols);
    // Left edge of the column row: the columns together occupy the
    // centered extents one run of the same width would.
    let mut x = geo.text_x - nominal_text_width(size, cols) / 2.0;
    if fixed > 0 {
        scene.text_attributed(
            group,
            x,
            CENTER_Y,
            size,
            Anchor::MIDDLE_LEFT,
            prefix.as_str(),
        )?;
        x += nominal_text_width(size, fixed);
    }
    if drum.hundreds_drawn {
        let d = drum.hundreds as usize;
        x = roll_window(
            scene,
            group,
            x,
            size,
            Faces {
                cols: 1,
                cur: DIGITS[d],
                next: DIGITS[(d + 1) % 10],
                roll: drum.hundreds_roll,
            },
        )?;
    }
    roll_window(
        scene,
        group,
        x,
        size,
        Faces {
            cols: 2,
            cur: PAIRS[drum.pair],
            next: PAIRS[(drum.pair + 1) % 5],
            roll: drum.pair_frac,
        },
    )?;
    Ok(())
}

/// What one rolling column shows: its width in glyph columns, the face
/// on the text line, the face arriving beneath it, and how far the
/// strip has scrolled between the two.
struct Faces<'a> {
    cols: usize,
    cur: &'a str,
    next: &'a str,
    roll: f32,
}

/// One clipped digit window: the current face centered on the text line
/// and the next one pitch below, the strip scrolled up by `roll` of a
/// pitch as the value climbs. Returns the next column's left edge.
fn roll_window(
    scene: &mut SceneWriter<'_>,
    group: u8,
    x: f32,
    size: f32,
    faces: Faces<'_>,
) -> Result<f32, SceneError> {
    let w = nominal_text_width(size, faces.cols);
    scene.save()?;
    scene.clip_rect(x, CENTER_Y - size / 2.0, w, size)?;
    let cx = x + w / 2.0;
    scene.text_attributed(
        group,
        cx,
        CENTER_Y - faces.roll * size,
        size,
        Anchor::CENTER,
        faces.cur,
    )?;
    scene.text_attributed(
        group,
        cx,
        CENTER_Y + (1.0 - faces.roll) * size,
        size,
        Anchor::CENTER,
        faces.next,
    )?;
    scene.restore()?;
    Ok(x + w)
}
