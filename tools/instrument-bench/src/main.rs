//! Standalone bench shell (ADR-0029): the third, deliberately unalike
//! shell. It composes the same registry the web shell consumes,
//! reproduces the cross-shell scene digest and the screen-composition
//! digest against their pinned values, runs the admission harness, and
//! rasterizes every panel × canonical state × canonical frame —
//! optionally writing PPM frames — with no host and no protocol. A
//! nonzero exit is a conformance failure, never a partial pass.
//!
//! Identity is reported before judgement: a refusal from the admission
//! harness still leaves the reader knowing which contract this shell
//! was running.

mod output;

use std::io::Write;

use output::print_line;
use std::path::PathBuf;

use indicate_instrument_conformance::{AdmissionError, admit};
use indicate_instrument_panels::{BUILTIN_CRITICALITY_BANDS, BUILTIN_PANELS, BUILTIN_SCENE_DIGEST};
use indicate_instrument_raster::{FrameId, FramebufferDims, RenderStatus, render};
use indicate_instrument_registry::{
    CANONICAL_STATES, CompositionDescriptor, CompositionError, DesignFrame, EMPTY_CONFIG,
    PanelDrawError, Region, Registry, RegistryError, Slot, composition_digest, scene_digest,
    validate_composition,
};
use indicate_instrument_scene::{MAX_SCENE_BYTES, SceneWriter};
use indicate_instrument_state::{FreshnessPolicy, resolve};

#[derive(Debug, thiserror::Error)]
enum BenchError {
    /// The shipped composition failed registry validation.
    #[error("registry composition refused: {0}")]
    Compose(#[from] RegistryError),
    /// The admission harness refused a panel.
    #[error("admission refused: {0}")]
    Admission(#[from] AdmissionError),
    /// This shell renders a different contract than the pin.
    #[error("scene digest {got} does not match the pinned {want}")]
    DigestMismatch {
        /// What this shell computed.
        got: String,
        /// The cross-shell pin.
        want: &'static str,
    },
    /// The fixture screen composition was refused.
    #[error("screen composition refused: {0}")]
    Composition(#[from] CompositionError),
    /// This shell lays the fixture screen out differently than the pin.
    #[error("screen-composition digest {got} does not match the pinned {want}")]
    CompositionDigestMismatch {
        /// What this shell computed.
        got: String,
        /// The cross-shell pin.
        want: &'static str,
    },
    /// A panel refused to draw a corpus state.
    #[error("panel {panel} failed to draw {state}: {source}")]
    Draw {
        /// The refusing panel.
        panel: &'static str,
        /// The corpus state.
        state: &'static str,
        /// The panel's reason.
        #[source]
        source: PanelDrawError,
    },
    /// The digest's scratch buffer cannot hold a scene.
    #[error("digest scratch buffer of {len} bytes is too small")]
    DigestScratch {
        /// The offending buffer length.
        len: usize,
    },
    /// The reference rasterizer refused a validated scene.
    #[error("raster failed for {panel} × {state}")]
    Raster {
        /// The panel whose scene failed.
        panel: &'static str,
        /// The corpus state.
        state: &'static str,
    },
    /// A PPM frame could not be written.
    #[error("writing {} failed", path.display())]
    Io {
        /// The destination that failed.
        path: PathBuf,
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },
}

/// The logical screen the fixture composition lays out on: two panel
/// frames wide and two tall.
const BENCH_SCREEN: DesignFrame = DesignFrame {
    width: 960.0,
    height: 720.0,
};

const fn tile(panel: &'static str, x: f32, y: f32) -> Slot {
    Slot {
        panel,
        rect: Region {
            x,
            y,
            width: 480.0,
            height: 360.0,
        },
        occludes: &[],
    }
}

/// The fixture screen: the three shipped panels tiled, each at the one
/// frame it declares. It overlaps nothing, so what it exercises here is
/// placement, the frame rule, and the digest; the occlusion and
/// dead-slot rules are covered by the registry's own must-fail
/// fixtures, which need panels shaped to break them.
const BENCH_COMPOSITION: CompositionDescriptor = CompositionDescriptor {
    screen: BENCH_SCREEN,
    slots: &[
        tile("pfd", 0.0, 0.0),
        tile("hsi", 480.0, 0.0),
        tile("monitor", 0.0, 360.0),
    ],
};

/// The pinned screen-composition digest over [`BENCH_COMPOSITION`]:
/// every shell composing this screen from this registry reproduces it.
const BENCH_COMPOSITION_DIGEST: &str =
    "071bd35c2e5884d7376a6a3e6ee5fa391148c74b51604d2024dea565190f688a";

fn main() -> Result<(), BenchError> {
    let out_dir = parse_out_dir();
    let registry = Registry::new(BUILTIN_PANELS)?;
    let mut scratch = vec![0u8; MAX_SCENE_BYTES];

    let digest = check_scene_digest(&registry, &mut scratch)?;
    print_line(&format!("scene digest: {digest} (matches pin)"));
    let composed = check_composition(&registry, &mut scratch)?;
    print_line(&format!(
        "screen-composition digest: {composed} (matches pin)"
    ));

    let report = admit(&registry)?;
    print_line(&format!(
        "admission: {} cases pass, {} counted warnings",
        report.cases,
        report.warnings.len()
    ));

    let mut rasterized = 0usize;
    for panel in registry.panels() {
        for state in CANONICAL_STATES {
            for frame in panel.canonical_frames {
                let pixels = rasterize(panel, state.id, (state.build)(), *frame, &mut scratch)?;
                rasterized += 1;
                if let Some(dir) = &out_dir {
                    write_ppm(dir, panel.id, state.id, *frame, &pixels)?;
                }
            }
        }
    }
    print_line(&format!(
        "rasterized {rasterized} panel x state x canonical frame renders"
    ));
    Ok(())
}

fn check_scene_digest(registry: &Registry, scratch: &mut [u8]) -> Result<String, BenchError> {
    let digest = hex(scene_digest(registry, scratch).map_err(digest_error)?);
    if digest != BUILTIN_SCENE_DIGEST {
        return Err(BenchError::DigestMismatch {
            got: digest,
            want: BUILTIN_SCENE_DIGEST,
        });
    }
    Ok(digest)
}

fn check_composition(registry: &Registry, scratch: &mut [u8]) -> Result<String, BenchError> {
    validate_composition(registry, &BENCH_COMPOSITION, &BUILTIN_CRITICALITY_BANDS)?;
    let digest =
        hex(composition_digest(registry, &BENCH_COMPOSITION, scratch).map_err(digest_error)?);
    if digest != BENCH_COMPOSITION_DIGEST {
        return Err(BenchError::CompositionDigestMismatch {
            got: digest,
            want: BENCH_COMPOSITION_DIGEST,
        });
    }
    Ok(digest)
}

fn digest_error(error: indicate_instrument_registry::DigestError) -> BenchError {
    match error {
        indicate_instrument_registry::DigestError::Draw {
            panel,
            state,
            source,
        } => BenchError::Draw {
            panel,
            state,
            source,
        },
        indicate_instrument_registry::DigestError::Scratch { len } => {
            BenchError::DigestScratch { len }
        }
    }
}

fn parse_out_dir() -> Option<PathBuf> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--out" {
            return args.next().map(PathBuf::from);
        }
    }
    None
}

fn rasterize(
    panel: &'static indicate_instrument_registry::PanelDescriptor,
    state_id: &'static str,
    state: indicate_instrument_state::AircraftState,
    frame: DesignFrame,
    scratch: &mut [u8],
) -> Result<Vec<u8>, BenchError> {
    let data = resolve(&state, &FreshnessPolicy::default());
    let mut writer = SceneWriter::new(scratch).map_err(|_| BenchError::Raster {
        panel: panel.id,
        state: state_id,
    })?;
    (panel.draw)(&data, &EMPTY_CONFIG, None, frame, &mut writer).map_err(|source| {
        BenchError::Draw {
            panel: panel.id,
            state: state_id,
            source,
        }
    })?;
    let used = writer.finish();
    let (w, h) = (frame.width as u32, frame.height as u32);
    let mut framebuffer = vec![0u8; (w * h * 4) as usize];
    let report = render(
        scratch.get(..used).unwrap_or(&[]),
        &mut framebuffer,
        FramebufferDims::tight(w, h),
        FrameId::default(),
    )
    .map_err(|_| BenchError::Raster {
        panel: panel.id,
        state: state_id,
    })?;
    if report.status != RenderStatus::Painted {
        return Err(BenchError::Raster {
            panel: panel.id,
            state: state_id,
        });
    }
    Ok(framebuffer)
}

fn write_ppm(
    dir: &PathBuf,
    panel_id: &str,
    state_id: &str,
    frame: DesignFrame,
    rgba: &[u8],
) -> Result<(), BenchError> {
    let (w, h) = (frame.width as usize, frame.height as usize);
    // The frame is in the filename: a panel with several canonical
    // frames writes one image per size, and they must not overwrite
    // each other.
    let path = dir.join(format!("{panel_id}-{state_id}-{w}x{h}.ppm"));
    let io = |source| BenchError::Io {
        path: path.clone(),
        source,
    };
    std::fs::create_dir_all(dir).map_err(io)?;
    let mut out = Vec::new();
    out.extend_from_slice(format!("P6\n{w} {h}\n255\n").as_bytes());
    for pixel in rgba.chunks_exact(4) {
        out.extend_from_slice(&pixel[..3]);
    }
    let mut file = std::fs::File::create(&path).map_err(io)?;
    file.write_all(&out).map_err(io)?;
    print_line(&format!("wrote {}", path.display()));
    Ok(())
}

fn hex(digest: [u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(64);
    for byte in digest {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}
