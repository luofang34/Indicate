//! Composed-frame evidence: three pinned REN-03-style hashes, and the
//! show-through property that makes overlap safe.
#![allow(clippy::expect_used, clippy::panic)]

use indicate_alerts::AlertOutput;
use indicate_instrument_panels::{BUILTIN_CRITICALITY_BANDS, PFD_DESCRIPTOR};
use indicate_instrument_registry::{
    BackgroundCapability, CompositionDescriptor, ConfigBlob, CriticalityBands, DesignFrame,
    GroupSet, PanelCriticality, PanelDescriptor, PanelDrawError, Region, Registry, Slot,
    validate_composition,
};
use indicate_instrument_scene::{LayerId, MAX_SCENE_BYTES, PaintMode, Rgba8, SceneWriter};
use indicate_instrument_state::{FreshnessPolicy, PanelData, resolve};
use sha2::{Digest, Sha256};

use std::string::String;
use std::vec;
use std::vec::Vec;

mod obscuration;

use super::{CompositionInputs, render_composition};
use crate::{FrameId, FramebufferDims, RasterError, RenderStatus};

/// Composed-frame hashes pinned from a byte-reproducible render, in the
/// same discipline as the per-panel `raster_baselines` (REN-03): `libm`
/// plus IEEE-754 `f32` make them identical across the supported CI
/// architectures, so a mismatch is a determinism regression rather than
/// a value to re-pin casually.
///
/// Each covers a different composition shape, because placement, opaque
/// overlap, and overlay show-through fail in different ways.
const SIDE_BY_SIDE_HASH: &str = "d5d06ac7ca6945971ad2c2052b4f785d88b1a041d29a43798bd6f8163d52658b";
const OPAQUE_INSET_HASH: &str = "521729352acb1e7bd5bf6db8b6471e46491844b97f8f6525b2827a5d2fd1de08";
const OVERLAY_HASH: &str = "d9ec53b6527d6e5a93d45a3acf0c1cb01deb0e1405b3411f463860eaa20ea45a";

pub(super) const PANEL_FRAME: DesignFrame = DesignFrame {
    width: 480.0,
    height: 360.0,
};

/// The inset and overlay both occupy this rect on the PFD: the strip
/// above the PFD's measured criticality band, which begins at y 38 and
/// runs to the bottom of the alert stack at y 352. That leaves the top
/// band of the frame, clear also of the VSI readout strip at x 440.
///
/// The rect is deliberately small. Once a panel's warnings are measured
/// with alerts fed, a PFD-sized panel has very little surface a slot
/// may sit on at all, and that is the floor working rather than a
/// fixture inconvenience. `validate_composition` proves the reading
/// rather than this comment — see
/// [`the_fixtures_are_admissible_compositions`].
const INSET_RECT: Region = Region {
    x: 140.0,
    y: 4.0,
    width: 200.0,
    height: 32.0,
};

const INSET_FRAME: DesignFrame = DesignFrame {
    width: 200.0,
    height: 32.0,
};

fn no_config(config: &ConfigBlob<'_>) -> Result<(), PanelDrawError> {
    config.require_schema(&[])?;
    Ok(())
}

/// An `Opaque` inset: it owns its background band with a full-frame
/// fill, so it occludes everything under its rect. The declaration is
/// honest by construction here, which is what admission proves for a
/// shipped panel.
fn draw_inset(
    _data: &PanelData,
    config: &ConfigBlob<'_>,
    _alerts: Option<&AlertOutput>,
    frame: DesignFrame,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    no_config(config)?;
    scene.begin_layer(LayerId::Background)?;
    scene.save()?;
    scene.fill_color(Rgba8::rgb(16, 24, 48))?;
    scene.rect(PaintMode::Fill, 0.0, 0.0, frame.width, frame.height)?;
    scene.restore()?;
    scene.end_layer(LayerId::Background)?;
    marker_band(scene)?;
    Ok(())
}

/// A `NotUsed` overlay: it opens no background band at all, so every
/// pixel of its rect that its own symbology does not paint is left
/// showing whatever lies beneath.
fn draw_overlay(
    _data: &PanelData,
    config: &ConfigBlob<'_>,
    _alerts: Option<&AlertOutput>,
    _frame: DesignFrame,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    no_config(config)?;
    marker_band(scene)?;
    Ok(())
}

/// The symbology both fixtures paint: one opaque bar well inside the
/// rect, leaving the rest of the band untouched.
fn marker_band(scene: &mut SceneWriter<'_>) -> Result<(), PanelDrawError> {
    scene.begin_layer(LayerId::Tapes)?;
    scene.save()?;
    scene.fill_color(Rgba8::rgb(255, 208, 0))?;
    scene.rect(PaintMode::Fill, 12.0, 8.0, 80.0, 16.0)?;
    scene.restore()?;
    scene.end_layer(LayerId::Tapes)?;
    Ok(())
}

const fn fixture(
    id: &'static str,
    background: BackgroundCapability,
    draw: indicate_instrument_registry::DrawFn,
) -> PanelDescriptor {
    PanelDescriptor {
        id,
        title: "Fixture",
        required_layers: 1 << 2, // Tapes
        required_groups: GroupSet::EMPTY,
        frame_min: INSET_FRAME,
        frame_max: INSET_FRAME,
        frame_step: (1.0, 1.0),
        aspect_min: 6.2,
        aspect_max: 6.3,
        canonical_frames: &[INSET_FRAME],
        background,
        config_schema: &[],
        group_regions: &[],
        extreme_states: &[],
        raster_baselines: &[],
        draw,
    }
}

static PFD_AND_INSET: [PanelDescriptor; 2] = [
    PFD_DESCRIPTOR,
    fixture("inset", BackgroundCapability::Opaque, draw_inset),
];

static PFD_AND_OVERLAY: [PanelDescriptor; 2] = [
    PFD_DESCRIPTOR,
    fixture("overlay", BackgroundCapability::NotUsed, draw_overlay),
];

/// The PFD's pinned criticality band, restated so the fixture bands are
/// one `'static` slice; [`the_fixture_band_matches_the_pin`] refuses a
/// drift between this and `BUILTIN_CRITICALITY_BANDS`.
const PFD_BAND: PanelCriticality = PanelCriticality {
    panel: "pfd",
    frame: PANEL_FRAME,
    band: Some(Region {
        x: 6.0,
        y: 38.0,
        width: 468.0,
        height: 314.0,
    }),
};

const fn quiet(panel: &'static str) -> PanelCriticality {
    PanelCriticality {
        panel,
        frame: INSET_FRAME,
        band: None,
    }
}

const INSET_BANDS: CriticalityBands = CriticalityBands {
    panels: &[PFD_BAND, quiet("inset")],
};

const OVERLAY_BANDS: CriticalityBands = CriticalityBands {
    panels: &[PFD_BAND, quiet("overlay")],
};

const fn tile(panel: &'static str, x: f32) -> Slot {
    Slot {
        panel,
        rect: Region {
            x,
            y: 0.0,
            width: 480.0,
            height: 360.0,
        },
        occludes: &[],
    }
}

const SIDE_BY_SIDE: CompositionDescriptor = CompositionDescriptor {
    screen: DesignFrame {
        width: 960.0,
        height: 360.0,
    },
    slots: &[tile("pfd", 0.0), tile("hsi", 480.0)],
};

/// The PFD alone, and then the PFD with something above it: the two
/// differ by exactly one slot, which is what lets the show-through
/// property compare them pixel for pixel.
const PFD_ALONE: CompositionDescriptor = CompositionDescriptor {
    screen: PANEL_FRAME,
    slots: &[tile("pfd", 0.0)],
};

const OPAQUE_INSET: CompositionDescriptor = CompositionDescriptor {
    screen: PANEL_FRAME,
    slots: &[
        tile("pfd", 0.0),
        Slot {
            panel: "inset",
            rect: INSET_RECT,
            occludes: &[],
        },
    ],
};

const OVERLAY: CompositionDescriptor = CompositionDescriptor {
    screen: PANEL_FRAME,
    slots: &[
        tile("pfd", 0.0),
        Slot {
            panel: "overlay",
            rect: INSET_RECT,
            occludes: &[],
        },
    ],
};

/// The shared canonical "typical" state, the same fixture the per-panel
/// frame hashes and the scene digest draw.
pub(super) fn typical() -> PanelData {
    resolve(
        &indicate_instrument_registry::states::typical(),
        &FreshnessPolicy::default(),
    )
}

fn compose(registry: &Registry, composition: &CompositionDescriptor) -> Vec<u8> {
    let (w, h) = (
        composition.screen.width as u32,
        composition.screen.height as u32,
    );
    let mut pixels = vec![0u8; (w * h * 4) as usize];
    let mut scratch = vec![0u8; MAX_SCENE_BYTES];
    let data = typical();
    let mut inputs = CompositionInputs {
        data: &data,
        alerts: None,
        scratch: &mut scratch,
    };
    let report = render_composition(
        registry,
        composition,
        &mut inputs,
        &mut pixels,
        FramebufferDims::tight(w, h),
        FrameId::default(),
    )
    .expect("the fixture composition renders");
    assert_eq!(report.status, RenderStatus::Painted);
    pixels
}

fn sha_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for byte in digest {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

/// Renders twice and asserts the frame is bit-reproducible before
/// comparing it to the pin, so a mismatch tells the reader whether the
/// renderer became non-deterministic or merely changed.
fn pinned(registry: &Registry, composition: &CompositionDescriptor, want: &str) -> Vec<u8> {
    let first = compose(registry, composition);
    let second = compose(registry, composition);
    assert_eq!(first, second, "a composed frame is bit-reproducible");
    assert_eq!(sha_hex(&first), want);
    first
}

fn builtin() -> Registry {
    Registry::new(indicate_instrument_panels::BUILTIN_PANELS).expect("shipped panels compose")
}

#[test]
fn the_fixture_band_matches_the_pin() {
    let pinned = BUILTIN_CRITICALITY_BANDS
        .entry("pfd", PANEL_FRAME)
        .expect("the PFD pins a band at its canonical frame");
    assert_eq!(*pinned, PFD_BAND);
}

/// Every fixture below is a composition the registry admits. The inset
/// and overlay clear the PFD's criticality band and every readout
/// region it declares, so neither needs an `occludes` entry — and that
/// is asserted here rather than reasoned about in a comment.
#[test]
fn the_fixtures_are_admissible_compositions() {
    validate_composition(&builtin(), &SIDE_BY_SIDE, &BUILTIN_CRITICALITY_BANDS)
        .expect("side by side");
    validate_composition(&builtin(), &PFD_ALONE, &BUILTIN_CRITICALITY_BANDS).expect("pfd alone");
    let inset = Registry::new(&PFD_AND_INSET).expect("composes");
    validate_composition(&inset, &OPAQUE_INSET, &INSET_BANDS).expect("opaque inset");
    let overlay = Registry::new(&PFD_AND_OVERLAY).expect("composes");
    validate_composition(&overlay, &OVERLAY, &OVERLAY_BANDS).expect("notused overlay");
}

#[test]
fn side_by_side_composed_frame_is_reproducible_and_pinned() {
    pinned(&builtin(), &SIDE_BY_SIDE, SIDE_BY_SIDE_HASH);
}

#[test]
fn opaque_inset_composed_frame_is_reproducible_and_pinned() {
    let registry = Registry::new(&PFD_AND_INSET).expect("composes");
    pinned(&registry, &OPAQUE_INSET, OPAQUE_INSET_HASH);
}

#[test]
fn overlay_composed_frame_is_reproducible_and_pinned() {
    let registry = Registry::new(&PFD_AND_OVERLAY).expect("composes");
    pinned(&registry, &OVERLAY, OVERLAY_HASH);
}

/// An opaque slot replaces what is under it: every pixel of the inset's
/// rect differs from the PFD-alone frame in at least the background,
/// so the inset is not merely painting over transparent gaps.
#[test]
fn an_opaque_inset_replaces_the_panel_beneath_it() {
    let base = compose(&builtin(), &PFD_ALONE);
    let registry = Registry::new(&PFD_AND_INSET).expect("composes");
    let composed = compose(&registry, &OPAQUE_INSET);
    let mut differing = 0usize;
    for (x, y) in inset_pixels() {
        if pixel(&composed, x, y) != pixel(&base, x, y) {
            differing += 1;
        }
    }
    assert_eq!(
        differing,
        inset_pixels().count(),
        "an Opaque slot owns every pixel of its rect"
    );
}

/// The property that makes overlap safe: where a `NotUsed` overlay's
/// own scene paints nothing, the slot beneath shows through unchanged.
///
/// This is the strong form — every transparent pixel of the overlay's
/// scene, not a sampled one — because a hash proves reproducibility and
/// this proves correctness.
#[test]
fn a_notused_overlay_lets_the_panel_beneath_show_through() {
    let base = compose(&builtin(), &PFD_ALONE);
    let registry = Registry::new(&PFD_AND_OVERLAY).expect("composes");
    let composed = compose(&registry, &OVERLAY);
    let alone = overlay_scene_alone();

    let (mut transparent, mut painted) = (0usize, 0usize);
    for (x, y) in inset_pixels() {
        let local = (x - INSET_RECT.x as usize, y - INSET_RECT.y as usize);
        let own = pixel_at(&alone, local.0, local.1, INSET_FRAME.width as usize);
        if own[3] == 0 {
            transparent += 1;
            assert_eq!(
                pixel(&composed, x, y),
                pixel(&base, x, y),
                "the panel beneath shows through at ({x}, {y})"
            );
        } else {
            painted += 1;
            assert_eq!(
                pixel(&composed, x, y),
                own,
                "the overlay's own ink reaches the surface at ({x}, {y})"
            );
        }
    }
    // Neither half of the property may be vacuous: the overlay must
    // really paint somewhere and really leave gaps.
    assert!(transparent > 0, "the overlay covered its whole rect");
    assert!(painted > 0, "the overlay painted nothing at all");
}

/// `validate_composition` refuses an unregistered slot at init, so the
/// renderer meets one only when a composition was painted that was
/// never admitted. It fails typed and spoils, like every other
/// reference render — it does not panic and it does not leave a
/// plausible partial frame.
#[test]
fn an_unregistered_slot_fails_typed_and_spoils() {
    const STRANGER: CompositionDescriptor = CompositionDescriptor {
        screen: PANEL_FRAME,
        slots: &[tile("pfd", 0.0), tile("nowhere", 0.0)],
    };
    let registry = builtin();
    let (w, h) = (PANEL_FRAME.width as u32, PANEL_FRAME.height as u32);
    let mut pixels = vec![0u8; (w * h * 4) as usize];
    let mut scratch = vec![0u8; MAX_SCENE_BYTES];
    let data = typical();
    let mut inputs = CompositionInputs {
        data: &data,
        alerts: None,
        scratch: &mut scratch,
    };
    let error = render_composition(
        &registry,
        &STRANGER,
        &mut inputs,
        &mut pixels,
        FramebufferDims::tight(w, h),
        FrameId::default(),
    )
    .expect_err("the registry composes no panel called nowhere");
    assert_eq!(error, RasterError::SlotPanelMissing { panel: "nowhere" });
    // Against the frame the successful slot alone would have left, not
    // against a property the spoil pattern and the PFD's opaque ground
    // happen to share: alpha is 255 everywhere either way, so an
    // alpha test would pass whether or not the frame was spoiled.
    assert_ne!(
        pixels,
        compose(&builtin(), &PFD_ALONE),
        "no pixel of the partial composition survived"
    );
}

/// The overlay's scene alone on transparent black, at its own frame:
/// the ground truth for which of its pixels are its own ink.
fn overlay_scene_alone() -> Vec<u8> {
    let registry = Registry::new(&PFD_AND_OVERLAY).expect("composes");
    const ALONE: CompositionDescriptor = CompositionDescriptor {
        screen: INSET_FRAME,
        slots: &[Slot {
            panel: "overlay",
            rect: Region {
                x: 0.0,
                y: 0.0,
                width: INSET_FRAME.width,
                height: INSET_FRAME.height,
            },
            occludes: &[],
        }],
    };
    compose(&registry, &ALONE)
}

fn inset_pixels() -> impl Iterator<Item = (usize, usize)> {
    let x0 = INSET_RECT.x as usize;
    let y0 = INSET_RECT.y as usize;
    let (w, h) = (INSET_RECT.width as usize, INSET_RECT.height as usize);
    (y0..y0 + h).flat_map(move |y| (x0..x0 + w).map(move |x| (x, y)))
}

fn pixel(frame: &[u8], x: usize, y: usize) -> [u8; 4] {
    pixel_at(frame, x, y, PANEL_FRAME.width as usize)
}

fn pixel_at(frame: &[u8], x: usize, y: usize, width: usize) -> [u8; 4] {
    let at = (y * width + x) * 4;
    let px = frame.get(at..at + 4).expect("pixel inside the frame");
    [px[0], px[1], px[2], px[3]]
}
