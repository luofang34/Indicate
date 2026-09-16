//! Generates an offline, interactive HWD gallery from the production instrument scene.
mod cases;
mod scene;

use indicate_instrument_hmd::{HMD_DESCRIPTOR, HMD_GLANCE_DESCRIPTOR, directions, view_reference};
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("drawing HWD case: {0}")]
    Draw(#[from] indicate_instrument_descriptor::PanelDrawError),
    #[error("decoding HWD scene: {0}")]
    Decode(#[from] indicate_instrument_scene::DecodeError),
    #[error("looking up HWD glyph: {0}")]
    Glyph(#[from] indicate_instrument_glyphs::GlyphError),
    #[error("gallery backend does not support {command}")]
    Unsupported { command: String },
    #[error("writing {}: {source}", path.display())]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

fn main() -> Result<(), Error> {
    let path = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/hwd-gallery.html"));
    let json = cases::all()
        .iter()
        .map(encode_case)
        .collect::<Result<Vec<_>, _>>()?
        .join(",");
    let page = include_str!("gallery.html")
        .replace("__CASES__", &json)
        .replace(
            "__ENTRY__",
            &indicate_instrument_hmd::COMPACT_ENTRY_DEGREES.to_string(),
        )
        .replace(
            "__EXIT__",
            &indicate_instrument_hmd::COMPACT_EXIT_DEGREES.to_string(),
        );
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).map_err(|source| Error::Write {
            path: parent.into(),
            source,
        })?;
    }
    std::fs::write(&path, page).map_err(|source| Error::Write { path, source })
}

fn encode_case(case: &cases::Case) -> Result<String, Error> {
    let forward = scene::encode(&case.data, case.alerts.as_ref(), &HMD_DESCRIPTOR)?;
    let compact = scene::encode(&case.data, case.alerts.as_ref(), &HMD_GLANCE_DESCRIPTOR)?;
    let frame = view_reference(&case.data);
    let angular = directions(&case.data);
    let lines = angular.strokes[..angular.length as usize]
        .iter()
        .map(|line| format!("[{:?},{:?}]", line.a, line.b))
        .collect::<Vec<_>>()
        .join(",");
    Ok(format!(
        "{{\"name\":{:?},\"note\":{:?},\"head\":{:?},\"background\":{:?},\"forward\":{forward},\"compact\":{compact},\"basis\":[{:?},{:?},{:?}],\"kind\":{},\"recovery\":{},\"world\":[{lines}]}}",
        case.name,
        case.note,
        case.head,
        case.background,
        frame.right,
        frame.up,
        frame.forward,
        frame.kind,
        case.data.presentation.unusual
    ))
}

#[cfg(test)]
mod cross_set_tests;
#[cfg(test)]
mod tests;
