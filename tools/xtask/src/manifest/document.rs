//! The manifest document: every value this revision pins, in one fixed
//! key order.
//!
//! Each value is read from the definition that owns it and, where the
//! tree can recompute it, checked against the pin before it is written
//! down. A manifest generated over a stale pin would launder a
//! disagreement into a machine-readable claim, so a mismatch stops the
//! generator instead of producing a file.
//!
//! Entries keep the order their source declares. Panel order is itself
//! pinned — it is composition order, and the scene digest covers it —
//! so sorting here would hide a change the digest already refuses.

use std::path::Path;

use indicate_instrument_glyphs::PANEL_GLYPHS;
use indicate_instrument_panels::{BUILTIN_CRITICALITY_BANDS, BUILTIN_PANELS, BUILTIN_SCENE_DIGEST};
use indicate_instrument_registry::{
    Registry, composition_digest, scene_digest, validate_composition,
};
use indicate_instrument_scene::{MAX_SCENE_BYTES, SCENE_FORMAT_VERSION};
use instrument_bench::{BENCH_COMPOSITION, BENCH_COMPOSITION_DIGEST};

use crate::error::XtaskError;
use crate::manifest::corpus::{self, CORPUS_PATH};
use crate::manifest::json;
use crate::manifest::state_abi;

/// The document shape. A consumer's check keys off this: a reader
/// written against version 1 is told it is reading something else
/// rather than silently missing a value that moved.
const MANIFEST_VERSION: u32 = 1;

/// The whole manifest as a UTF-8 JSON document ending in a newline.
pub fn render(root: &Path) -> Result<String, XtaskError> {
    let registry =
        Registry::new(BUILTIN_PANELS).map_err(|source| XtaskError::Registry { source })?;
    let mut scratch = vec![0u8; MAX_SCENE_BYTES];
    let mut lines = vec!["{".to_string()];
    header(&mut lines, root)?;
    lines.push(format!(
        "  \"compositionDigest\": {},",
        json::string(&scene_pin(&registry, &mut scratch)?)
    ));
    screen_block(&mut lines, &registry, &mut scratch)?;
    baselines_block(&mut lines, &registry)?;
    lines.push(format!(
        "  \"glyphPackHash\": {},",
        json::string(&glyph_pin()?)
    ));
    bands_block(&mut lines)?;
    lines.push("}".to_string());
    let mut out = lines.join("\n");
    out.push('\n');
    Ok(out)
}

fn header(lines: &mut Vec<String>, root: &Path) -> Result<(), XtaskError> {
    let abi = state_abi::read(root)?;
    let corpus = corpus::read(root)?;
    lines.push(format!("  \"manifestVersion\": {MANIFEST_VERSION},"));
    lines.push("  \"generatedBy\": \"cargo xtask gen-release-manifest\",".to_string());
    lines.push("  \"simOnly\": \"SIM / NOT FOR FLIGHT — simulation use only\",".to_string());
    lines.push(format!(
        "  \"stateAbi\": {{ \"module\": {}, \"version\": {} }},",
        json::string(&abi.module),
        abi.version
    ));
    lines.push(format!("  \"sceneFormatVersion\": {SCENE_FORMAT_VERSION},"));
    lines.push("  \"corpus\": {".to_string());
    lines.push(format!("    \"path\": {},", json::string(CORPUS_PATH)));
    lines.push(format!("    \"version\": {},", corpus.version));
    lines.push(format!("    \"sha256\": {}", json::string(&corpus.sha256)));
    lines.push("  },".to_string());
    Ok(())
}

/// The scene digest over the shipped panel set, checked against the pin
/// the panel crate declares.
fn scene_pin(registry: &Registry, scratch: &mut [u8]) -> Result<String, XtaskError> {
    let computed = json::hex(
        &scene_digest(registry, scratch).map_err(|source| XtaskError::Digest { source })?,
    );
    if computed != BUILTIN_SCENE_DIGEST {
        return Err(XtaskError::PinMismatch {
            value: "the composition digest",
            computed,
            declared: BUILTIN_SCENE_DIGEST.to_string(),
        });
    }
    Ok(computed)
}

/// The fixture screen the bench composes, its slots, and the digest
/// over them — the shape a consumer reproduces, not only the value.
fn screen_block(
    lines: &mut Vec<String>,
    registry: &Registry,
    scratch: &mut [u8],
) -> Result<(), XtaskError> {
    validate_composition(registry, &BENCH_COMPOSITION, &BUILTIN_CRITICALITY_BANDS)
        .map_err(|source| XtaskError::Composition { source })?;
    let computed = json::hex(
        &composition_digest(registry, &BENCH_COMPOSITION, scratch)
            .map_err(|source| XtaskError::Digest { source })?,
    );
    if computed != BENCH_COMPOSITION_DIGEST {
        return Err(XtaskError::PinMismatch {
            value: "the screen-composition digest",
            computed,
            declared: BENCH_COMPOSITION_DIGEST.to_string(),
        });
    }
    lines.push("  \"screenComposition\": {".to_string());
    lines.push("    \"fixture\": \"tools/instrument-bench\",".to_string());
    lines.push(format!(
        "    \"screen\": {{ {} }},",
        json::frame_fields(
            BENCH_COMPOSITION.screen.width,
            BENCH_COMPOSITION.screen.height
        )?
    ));
    lines.push("    \"slots\": [".to_string());
    let mut slots = Vec::new();
    for slot in BENCH_COMPOSITION.slots {
        slots.push(format!(
            "      {{ \"panel\": {}, \"x\": {}, \"y\": {}, {} }}",
            json::string(slot.panel),
            json::number(slot.rect.x, "a slot x")?,
            json::number(slot.rect.y, "a slot y")?,
            json::frame_fields(slot.rect.width, slot.rect.height)?
        ));
    }
    lines.push(slots.join(",\n"));
    lines.push("    ],".to_string());
    lines.push(format!("    \"digest\": {}", json::string(&computed)));
    lines.push("  },".to_string());
    Ok(())
}

/// The reference rasterizer's pinned frame hash per panel × canonical
/// frame, as each descriptor declares it.
fn baselines_block(lines: &mut Vec<String>, registry: &Registry) -> Result<(), XtaskError> {
    lines.push("  \"rasterBaselines\": [".to_string());
    let mut entries = Vec::new();
    for panel in registry.panels() {
        for (frame, hash) in panel.raster_baselines {
            entries.push(format!(
                "    {{ \"panel\": {}, \"frame\": {{ {} }}, \"sha256\": {} }}",
                json::string(panel.id),
                json::frame_fields(frame.width, frame.height)?,
                json::string(hash)
            ));
        }
    }
    lines.push(entries.join(",\n"));
    lines.push("  ],".to_string());
    Ok(())
}

/// The measured criticality band per panel × frame. An unwitnessed band
/// is `null`, never an empty rectangle: a shell that reads zero where
/// the tree recorded "nothing was seen" would licence obscuring it.
fn bands_block(lines: &mut Vec<String>) -> Result<(), XtaskError> {
    lines.push("  \"criticalityBands\": [".to_string());
    let mut entries = Vec::new();
    for entry in BUILTIN_CRITICALITY_BANDS.panels {
        let band = match entry.band {
            Some(region) => format!(
                "{{ \"x\": {}, \"y\": {}, {} }}",
                json::number(region.x, "a criticality band x")?,
                json::number(region.y, "a criticality band y")?,
                json::frame_fields(region.width, region.height)?
            ),
            None => "null".to_string(),
        };
        entries.push(format!(
            "    {{ \"panel\": {}, \"frame\": {{ {} }}, \"band\": {band} }}",
            json::string(entry.panel),
            json::frame_fields(entry.frame.width, entry.frame.height)?
        ));
    }
    lines.push(entries.join(",\n"));
    lines.push("  ]".to_string());
    Ok(())
}

/// The glyph pack's recorded content hash, checked against the hash the
/// live glyph data produces.
fn glyph_pin() -> Result<String, XtaskError> {
    let recorded = json::hex(&PANEL_GLYPHS.recorded_hash());
    let live = json::hex(&PANEL_GLYPHS.content_hash());
    if recorded != live {
        return Err(XtaskError::PinMismatch {
            value: "the glyph pack content hash",
            computed: live,
            declared: recorded,
        });
    }
    Ok(recorded)
}
