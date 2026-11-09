//! The moving parts, each in its own transform, then the fixed parts in
//! front of them, and the failure flag.
//!
//! A pointer is its outline filled under a `TRANSLATE` to its pivot and
//! a `ROTATE` by its pose angle, inside its own save and restore. A
//! drum wheel is its digit strip under a `TRANSLATE` by its offset,
//! inside the window's clip. A card is its shapes under a `TRANSLATE`
//! to its pivot, a `ROTATE` and a `TRANSLATE` along its own axis; a tape
//! is its strip under a `TRANSLATE` inside its window's clip. So the 2D
//! scene moves each part exactly as the physical build does: by one or
//! two numbers per part.
//!
//! Order, back to front: drum windows, cards, tapes, pointers by stack
//! height, then the fixed overlays by stack height.

use indicate_instrument_descriptor::PanelDrawError;
use indicate_instrument_scene::{Anchor, PaintMode, SceneWriter};
use indicate_instrument_symbology::safety;

use crate::face::Placement;
use crate::outline::pointer_outline;
use crate::pose::{DialPose, WindowPose};
use crate::spec::{AnalogDialSpec, Lighting, OffFlag, WindowSpec};

mod parts;

const DIGITS: [&str; 10] = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];

/// Width of a printed separator between wheels, as a share of a wheel.
pub(crate) const SEPARATOR_SHARE: f32 = 0.4;
/// Distance between adjacent digits on a wheel strip, as a share of the
/// aperture height: less than one, so the neighbours just show.
pub(crate) const DIGIT_PITCH_SHARE: f32 = 0.9;

/// Draws every drum window, then every visible pointer in ascending
/// stack height, into the band the caller has open.
pub fn draw_moving_parts(
    spec: &AnalogDialSpec<'_>,
    pose: &DialPose,
    lighting: Lighting,
    place: &Placement,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    for (window, window_pose) in spec.windows.iter().zip(&pose.windows) {
        draw_window(spec, window, window_pose, lighting, place, scene)?;
    }
    parts::draw_cards(spec, pose, lighting, place, scene)?;
    parts::draw_tapes(spec, pose, lighting, place, scene)?;
    let mut order = [0usize; crate::spec::MAX_POINTERS];
    for (k, slot) in order.iter_mut().enumerate() {
        *slot = k;
    }
    let count = spec.pointers.len().min(order.len());
    let height = |k: usize| {
        spec.pointers
            .get(k)
            .map_or(0.0, |pointer| pointer.stack_height_mm)
    };
    if let Some(live) = order.get_mut(..count) {
        live.sort_unstable_by(|a, b| height(*a).total_cmp(&height(*b)));
        for k in live.iter() {
            if let (Some(pointer), Some(p)) = (spec.pointers.get(*k), pose.pointers.get(*k))
                && p.visible
            {
                let [x, y] = place.at(pointer.pivot);
                let outline = pointer_outline(&pointer.style).scaled(place.scale);
                scene.save()?;
                scene.translate(x, y)?;
                scene.rotate(p.angle_rad)?;
                scene.fill_color(pointer.style.paint.under(lighting))?;
                scene.polygon(PaintMode::Fill, outline.points())?;
                if pointer.style.hub_radius > 0.0 {
                    scene.circle(
                        PaintMode::Fill,
                        0.0,
                        0.0,
                        pointer.style.hub_radius * place.scale,
                    )?;
                }
                scene.restore()?;
            }
        }
    }
    parts::draw_overlays(spec, lighting, place, scene)
}

fn draw_window(
    spec: &AnalogDialSpec<'_>,
    window: &WindowSpec<'_>,
    pose: &WindowPose,
    lighting: Lighting,
    place: &Placement,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let [ax, ay, aw, ah] = window.aperture;
    let [x, y] = place.at([ax, ay]);
    let (w, h) = (aw * place.scale, ah * place.scale);
    scene.fill_color(window.drum)?;
    scene.rect(PaintMode::Fill, x, y, w, h)?;
    if !pose.live {
        // No digits: a digit on a drum with no value would claim one.
        return Ok(());
    }
    let group = spec
        .channel(window.channel)
        .map_or(0, |channel| channel.input.group().to_u8());
    let wheels = usize::from(window.wheels).max(1);
    let gap = if window.separator.is_some() {
        SEPARATOR_SHARE
    } else {
        0.0
    };
    let wheel_w = w / (wheels as f32 + gap);
    let pitch = h * DIGIT_PITCH_SHARE;
    let size = window.digit_size * place.scale;
    scene.save()?;
    scene.clip_rect(x, y, w, h)?;
    scene.fill_color(window.paint.under(lighting))?;
    let mut right = x + w;
    for (k, position) in pose.wheels.iter().enumerate().take(wheels) {
        let cx = right - wheel_w * 0.5;
        let base = libm::floorf(*position);
        scene.save()?;
        scene.translate(cx, y + h * 0.5 - (position - base) * pitch)?;
        // Only the digit in the window and its neighbours above and
        // below: the rest of the strip is behind the face.
        for step in -1..=1_i32 {
            let digit = (base as i32 + step).rem_euclid(10);
            let text = DIGITS.get(digit as usize).copied().unwrap_or("0");
            scene.text_attributed(group, 0.0, step as f32 * pitch, size, Anchor::CENTER, text)?;
        }
        scene.restore()?;
        right -= wheel_w;
        if let Some((after, mark)) = window.separator
            && usize::from(after) == k + 1
        {
            let sep_w = wheel_w * gap;
            scene.text(right - sep_w * 0.5, y + h * 0.5, size, Anchor::CENTER, mark)?;
            right -= sep_w;
        }
    }
    scene.restore()?;
    Ok(())
}

/// Draws `flag` into the band the caller has open: a red tab with its
/// legend, in the face where the instrument shows it.
pub fn draw_flag(
    flag: &OffFlag<'_>,
    place: &Placement,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let [x, y] = place.at(flag.at);
    let size = flag.size * place.scale;
    let (w, h) = (
        size * 0.75 * flag.text.chars().count() as f32 + size,
        size * 1.6,
    );
    scene.fill_color(safety::FAILURE_RED)?;
    scene.rect(PaintMode::Fill, x - w * 0.5, y - h * 0.5, w, h)?;
    scene.fill_color(safety::ANNUNCIATION_WHITE)?;
    scene.text(x, y, size, Anchor::CENTER, flag.text)?;
    Ok(())
}
