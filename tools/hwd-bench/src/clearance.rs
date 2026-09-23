//! Text reservations include glyph advance, anchors, and contrast halos.
use crate::Error;
use indicate_instrument_glyphs::{ADVANCE, CELL_H};
use indicate_instrument_scene::{Cmd, HAlign, SceneCmds, VAlign};

pub(super) fn check(bytes: &[u8]) -> Result<(), Error> {
    let mut bounds: Vec<(&str, [f32; 4])> = Vec::new();
    for command in SceneCmds::new(bytes)? {
        let Cmd::Text {
            x,
            y,
            size,
            anchor,
            text,
        } = command?
        else {
            continue;
        };
        let width = text.chars().count() as f32 * f32::from(ADVANCE) * size / CELL_H as f32;
        let left = x - match anchor.h {
            HAlign::Left => 0.0,
            HAlign::Center => width / 2.0,
            HAlign::Right => width,
        };
        let top = y - match anchor.v {
            VAlign::Top => 0.0,
            VAlign::Middle => size / 2.0,
            _ => size,
        };
        // Two pixels cover the contrast halo and minimum separation on each edge.
        let rect = [left - 2.0, top - 2.0, left + width + 2.0, top + size + 2.0];
        let host = indicate_instrument_hmd::HOST_CONTEXT_ZONE;
        if rect[0] < host.x + host.width
            && rect[2] > host.x
            && rect[1] < host.y + host.height
            && rect[3] > host.y
        {
            return Err(Error::Clearance {
                detail: format!("{text:?} enters host context reservation"),
            });
        }
        if rect[0] < 0.0 || rect[1] < 0.0 || rect[2] > 1200.0 || rect[3] > 600.0 {
            return Err(Error::Clearance {
                detail: format!("{text:?} leaves frame: {rect:?}"),
            });
        }
        for (other, prior) in &bounds {
            if rect[0] < prior[2] && rect[2] > prior[0] && rect[1] < prior[3] && rect[3] > prior[1]
            {
                return Err(Error::Clearance {
                    detail: format!("{text:?} {rect:?} intersects {other:?} {prior:?}"),
                });
            }
        }
        bounds.push((text, rect));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
