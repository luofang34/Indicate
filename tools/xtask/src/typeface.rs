//! Typeface generation (`gen-typeface`): compiles the faces Indicate
//! ships as test data from their source definitions into the canonical
//! bytes a backend loads.
//!
//! The faces live outside the kernel crates, under
//! `testdata/typefaces/<id>/`, each beside a record of its provenance
//! and licence. A drift test regenerates the bytes and compares them
//! with the committed file, so the source and the data cannot part.

use indicate_instrument_glyphs::typeface::{FaceDef, encode_face, encoded_len};

use crate::error::XtaskError;
use crate::output::print_line;
use crate::workspace::repo_root;

pub mod us_ms33558;

#[cfg(test)]
mod tests;

/// Every face this repository ships as test data.
pub const FACES: [&FaceDef<'static>; 1] = [&us_ms33558::FACE];

/// The canonical bytes of `face`.
pub fn bytes_of(face: &FaceDef<'_>) -> Result<Vec<u8>, XtaskError> {
    let mut buf = vec![0u8; encoded_len(face)];
    let n = encode_face(face, &mut buf).map_err(|error| XtaskError::Usage {
        message: format!("typeface {} does not encode: {error}", face.id),
    })?;
    buf.truncate(n);
    Ok(buf)
}

/// Where `face`'s bytes are committed, relative to the repository root.
pub fn relative_path(face: &FaceDef<'_>) -> String {
    format!("testdata/typefaces/{0}/{0}.idtf", face.id)
}

/// Writes every face's bytes, printing each path and byte count.
pub fn run() -> Result<(), XtaskError> {
    for face in FACES {
        let path = repo_root().join(relative_path(face));
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|source| XtaskError::Io {
                context: "creating testdata/typefaces",
                source,
            })?;
        }
        let bytes = bytes_of(face)?;
        std::fs::write(&path, &bytes).map_err(|source| XtaskError::Io {
            context: "writing a typeface",
            source,
        })?;
        print_line(&format!("wrote {} ({} bytes)", path.display(), bytes.len()));
    }
    Ok(())
}
