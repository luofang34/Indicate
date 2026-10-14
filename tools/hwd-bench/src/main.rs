//! Generates an offline, interactive HWD gallery from the production instrument scene.
mod cases;
mod clearance;
mod review;
mod scene;

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("HWD text clearance: {detail}")]
    Clearance { detail: String },
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
    let mut gallery = review::Gallery::default();
    let json = cases::all()
        .iter()
        .map(|case| gallery.encode_case(case))
        .collect::<Result<Vec<_>, _>>()?
        .join(",");
    let page = include_str!("gallery.html")
        .replace("__SCRIPT__", include_str!("gallery.js"))
        .replace("__CASES__", &json)
        .replace("__SHAPES__", &gallery.shapes.join(","))
        .replace("__PROFILES__", &review::profile_names())
        .replace("__ZONES__", &review::zones())
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

#[cfg(test)]
mod cross_set_tests;
#[cfg(test)]
mod tests;
