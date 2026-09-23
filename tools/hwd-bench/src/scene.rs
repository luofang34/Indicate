//! Converts production scene commands to geometry without substituting host fonts.
use crate::Error;
use indicate_instrument_glyphs::{ADVANCE, CELL_H, CELL_W, PANEL_GLYPHS};
use indicate_instrument_hmd::{DisplayContext, InstrumentReference, draw_hwd, layer_reference};
use indicate_instrument_scene::{Anchor, Cmd, HAlign, Rgba8, SceneCmds, SceneWriter, VAlign};
use indicate_instrument_state::PanelData;

pub(super) fn encode(
    data: &PanelData,
    alerts: Option<&indicate_alerts::AlertOutput>,
    context: DisplayContext,
    compact: bool,
) -> Result<Vec<String>, Error> {
    let mut storage = [0; 8192];
    let mut writer = SceneWriter::new(&mut storage)
        .map_err(indicate_instrument_descriptor::PanelDrawError::from)?;
    draw_hwd(data, context, compact, alerts, &mut writer)?;
    let length = writer.finish();
    crate::clearance::check(&storage[..length])?;
    let mut shapes = Vec::new();
    let mut fill = Rgba8::rgba(255, 255, 255, 255);
    let mut stroke = fill;
    let mut width = 1.0;
    let mut paints = Vec::new();
    let mut reference = 0;
    for command in SceneCmds::new(&storage[..length])? {
        let start = shapes.len();
        match command? {
            Cmd::Save => paints.push((fill, stroke, width)),
            Cmd::Restore => {
                (fill, stroke, width) = paints.pop().ok_or_else(|| Error::Unsupported {
                    command: "unbalanced Restore".into(),
                })?;
            }
            Cmd::FillColor { color } => fill = color,
            Cmd::Stroke { color, width: w } => {
                stroke = color;
                width = w;
            }
            Cmd::BeginLayer { layer } => {
                reference = match layer_reference(layer, compact || data.presentation.unusual) {
                    InstrumentReference::Aircraft => 0,
                    InstrumentReference::HeadLevel => 1,
                    InstrumentReference::Head => 2,
                };
            }
            Cmd::EndLayer { .. } | Cmd::Attribute { .. } => {}
            command => geometry(command, &mut shapes, fill, stroke, width)?,
        }
        for shape in &mut shapes[start..] {
            shape.insert_str(1, &format!("\"reference\":{reference},"));
        }
    }
    Ok(shapes)
}

fn geometry(
    command: Cmd<'_>,
    shapes: &mut Vec<String>,
    fill: Rgba8,
    stroke: Rgba8,
    width: f32,
) -> Result<(), Error> {
    match command {
        Cmd::Line { x1, y1, x2, y2 } => {
            shapes.push(shape(&[[x1, y1], [x2, y2]], stroke, width, false))
        }
        Cmd::Polyline { points } => shapes.push(shape(
            &points.iter().collect::<Vec<_>>(),
            stroke,
            width,
            false,
        )),
        Cmd::Rect { mode, x, y, w, h } => {
            let points = [[x, y], [x + w, y], [x + w, y + h], [x, y + h], [x, y]];
            if mode != indicate_instrument_scene::PaintMode::Stroke {
                shapes.push(shape(&points, fill, 0.0, true));
            }
            if mode != indicate_instrument_scene::PaintMode::Fill {
                shapes.push(shape(&points, stroke, width, false));
            }
        }
        Cmd::Circle {
            mode: indicate_instrument_scene::PaintMode::Stroke,
            cx,
            cy,
            r,
        } => {
            let points = (0..=80)
                .map(|i| {
                    let a = i as f32 * core::f32::consts::TAU / 80.0;
                    [cx + r * a.cos(), cy + r * a.sin()]
                })
                .collect::<Vec<_>>();
            shapes.push(shape(&points, stroke, width, false));
        }
        Cmd::Text {
            x,
            y,
            size,
            anchor,
            text,
        } => glyphs(shapes, text, [x, y], size, anchor, fill)?,
        command => {
            return Err(Error::Unsupported {
                command: format!("{command:?}"),
            });
        }
    }
    Ok(())
}

fn shape(points: &[[f32; 2]], color: Rgba8, width: f32, fill: bool) -> String {
    format!(
        "{{\"points\":{points:?},\"color\":[{},{},{},{}],\"width\":{width},\"fill\":{fill}}}",
        color.r, color.g, color.b, color.a
    )
}

fn glyphs(
    shapes: &mut Vec<String>,
    text: &str,
    position: [f32; 2],
    size: f32,
    anchor: Anchor,
    color: Rgba8,
) -> Result<(), Error> {
    let scale = size / CELL_H as f32;
    let advance = f32::from(ADVANCE) * scale;
    let width = text.chars().count() as f32 * advance;
    let x = position[0]
        - match anchor.h {
            HAlign::Left => 0.0,
            HAlign::Center => width / 2.0,
            HAlign::Right => width,
        };
    let y = position[1]
        - match anchor.v {
            VAlign::Top => 0.0,
            VAlign::Middle => size / 2.0,
            _ => size,
        };
    let mut points = Vec::new();
    for (i, ch) in text.chars().enumerate() {
        let glyph = PANEL_GLYPHS.glyph(ch)?.glyph;
        for row in 0..CELL_H {
            for col in 0..CELL_W {
                if !glyph.pixel(col, row) {
                    continue;
                }
                let left = x + i as f32 * advance + col as f32 * scale;
                let top = y + row as f32 * scale;
                points.push([
                    [left, top],
                    [left + scale, top],
                    [left + scale, top + scale],
                    [left, top + scale],
                ]);
            }
        }
    }
    // A complete text run is one compound path, matching the host's glyph atlas geometry.
    shapes.push(format!(
        "{{\"glyphs\":{points:?},\"color\":[{},{},{},{}]}}",
        color.r, color.g, color.b, color.a
    ));
    Ok(())
}
