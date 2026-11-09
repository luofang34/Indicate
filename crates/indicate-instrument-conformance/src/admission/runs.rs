//! The run scanner: every text run of a scene as a design-space ink
//! rectangle with the provenance claim that prefixes it.

use indicate_instrument_scene::{Cmd, SceneCmds};

use super::TextRun;
use super::geometry::{Ctm, Rect, arc_run_rect, text_rect};

/// Why the run scanner refused a scene.
pub(super) enum RunsDefect {
    Decode,
    MisplacedClaim,
}

/// Decodes every text run into a design-space ink rectangle, tracking
/// the transform and clip state the way a backend would, and pairing
/// each run with the provenance claim prefixing it. A claim not
/// immediately consumed by a text run is refused — stacked, dangling,
/// or shape-interposed claims are structurally malformed.
pub(super) fn collect_runs(scene: &[u8]) -> Result<Vec<TextRun>, RunsDefect> {
    let cmds = SceneCmds::new(scene).map_err(|_| RunsDefect::Decode)?;
    let mut runs = Vec::new();
    let mut pending: Option<u8> = None;
    let mut stack = vec![(Ctm::IDENTITY, None::<Rect>)];
    for cmd in cmds {
        if pending.is_some() && !matches!(cmd, Ok(Cmd::Text { .. } | Cmd::TextArc(_))) {
            return Err(RunsDefect::MisplacedClaim);
        }
        match cmd {
            Ok(Cmd::Attribute { group }) => pending = Some(group),
            Ok(run @ (Cmd::Text { .. } | Cmd::TextArc(_))) => {
                let (ctm, clip) = stack.last().copied().ok_or(RunsDefect::Decode)?;
                let (rect, text) = run_extent(&ctm, &run);
                runs.push(TextRun {
                    visible: clip.is_none_or(|clip| rect.intersects(&clip)),
                    rect,
                    text: text.to_string(),
                    attribution: pending.take(),
                    clip,
                });
            }
            Ok(Cmd::Save) => stack.push(stack.last().copied().ok_or(RunsDefect::Decode)?),
            Ok(Cmd::Restore) => {
                stack.pop();
                if stack.is_empty() {
                    stack.push((Ctm::IDENTITY, None));
                }
            }
            Ok(Cmd::Translate { x, y }) => {
                if let Some((ctm, _)) = stack.last_mut() {
                    ctm.translate(x, y);
                }
            }
            Ok(Cmd::Rotate { radians }) => {
                if let Some((ctm, _)) = stack.last_mut() {
                    ctm.rotate(radians);
                }
            }
            Ok(Cmd::ClipRect { x, y, w, h }) => {
                if let Some((ctm, clip)) = stack.last_mut() {
                    let mapped = ctm.map_rect(&Rect {
                        min_x: x,
                        min_y: y,
                        max_x: x + w,
                        max_y: y + h,
                    });
                    *clip = Some(match clip {
                        None => mapped,
                        Some(previous) => previous.intersect(&mapped),
                    });
                }
            }
            Ok(_) => {}
            Err(_) => return Err(RunsDefect::Decode),
        }
    }
    if pending.is_some() {
        return Err(RunsDefect::MisplacedClaim);
    }
    Ok(runs)
}

/// A text run's design-space ink rectangle and its text. An arc run is
/// a text run for every rule: its claim, its numerals, its visibility,
/// and printed-legend invariance.
fn run_extent<'a>(ctm: &Ctm, cmd: &Cmd<'a>) -> (Rect, &'a str) {
    match *cmd {
        Cmd::Text {
            x,
            y,
            size,
            anchor,
            text,
        } => {
            let local = text_rect(x, y, size, anchor.h, anchor.v, text.chars().count());
            (ctm.map_rect(&local), text)
        }
        Cmd::TextArc(arc) => (arc_run_rect(ctm, &arc), arc.text),
        _ => (
            Rect {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 0.0,
                max_y: 0.0,
            },
            "",
        ),
    }
}
