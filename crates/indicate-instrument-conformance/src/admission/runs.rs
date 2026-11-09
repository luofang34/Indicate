//! The run scanner: every text run of a scene as a design-space ink
//! rectangle with the provenance claim that prefixes it.

use indicate_instrument_glyphs::typeface::{FaceTable, TypefaceError};
use indicate_instrument_scene::{Cmd, SceneCmds};

use super::TextRun;
use super::geometry::{Gs, track_state};
use super::lettering::run_ink;

/// Why the run scanner refused a scene.
pub(super) enum RunsDefect {
    Decode,
    MisplacedClaim,
    /// The face bound to `slot` cannot letter a run.
    Face {
        slot: u8,
        source: TypefaceError,
    },
}

/// Decodes every text run into a design-space ink rectangle in the face
/// `faces` binds to its slot, tracking the transform, clip and face
/// state the way a backend would, and pairing each run with the
/// provenance claim prefixing it. A claim not immediately consumed by a
/// text run is refused — stacked, dangling, or shape-interposed claims
/// are structurally malformed.
pub(super) fn collect_runs(
    scene: &[u8],
    faces: &FaceTable<'_>,
) -> Result<Vec<TextRun>, RunsDefect> {
    let cmds = SceneCmds::new(scene).map_err(|_| RunsDefect::Decode)?;
    let mut runs = Vec::new();
    let mut pending: Option<u8> = None;
    let mut stack = vec![Gs::DEFAULT];
    for cmd in cmds {
        if pending.is_some() && !matches!(cmd, Ok(Cmd::Text { .. } | Cmd::TextArc(_))) {
            return Err(RunsDefect::MisplacedClaim);
        }
        let cmd = cmd.map_err(|_| RunsDefect::Decode)?;
        if let Cmd::Attribute { group } = cmd {
            pending = Some(group);
            continue;
        }
        track_state(&mut stack, &cmd);
        let gs = stack.last().copied().ok_or(RunsDefect::Decode)?;
        // An arc run is a text run for every rule: its claim, its
        // numerals, its visibility, and printed-legend invariance.
        let ink = run_ink(&gs.ctm, &cmd, gs.font, faces).map_err(|source| RunsDefect::Face {
            slot: gs.font,
            source,
        })?;
        if let Some((rect, text)) = ink {
            runs.push(TextRun {
                visible: gs.clip.is_none_or(|clip| rect.intersects(&clip)),
                rect,
                text: text.to_string(),
                attribution: pending.take(),
                clip: gs.clip,
            });
        }
    }
    if pending.is_some() {
        return Err(RunsDefect::MisplacedClaim);
    }
    Ok(runs)
}
