//! Work-budget accounting the bench relies on when it prices a frame.
//!
//! The raster crate's own suite is bound by recorded evidence, so the
//! accounting rule is pinned from the shell side: a composite is priced
//! only when a pixel is written.

#![allow(clippy::expect_used, clippy::panic)]

use indicate_instrument_raster::{FrameId, FramebufferDims, RenderStatus, render};
use indicate_instrument_scene::{LayerId, PaintMode, Rgba8, SceneWriter};

const W: u32 = 8;
const H: u32 = 4;

fn full_frame_fill(alpha: u8) -> Vec<u8> {
    let mut scratch = [0u8; 256];
    let mut writer = SceneWriter::new(&mut scratch).expect("buffer");
    writer.begin_layer(LayerId::Background).expect("layer");
    writer
        .fill_color(Rgba8::rgba(0, 0, 0, alpha))
        .expect("color");
    writer
        .rect(PaintMode::Fill, 0.0, 0.0, W as f32, H as f32)
        .expect("rect");
    writer.end_layer(LayerId::Background).expect("layer");
    let used = writer.finish();
    scratch[..used].to_vec()
}

fn composites_of(scene: &[u8]) -> u64 {
    let mut framebuffer = vec![0u8; (W * H * 4) as usize];
    let report = render(
        scene,
        &mut framebuffer,
        FramebufferDims::tight(W, H),
        FrameId::default(),
    )
    .expect("renders");
    assert_eq!(report.status, RenderStatus::Painted);
    report.work.composites
}

#[test]
fn a_fully_transparent_fill_prices_no_composites() {
    assert_eq!(composites_of(&full_frame_fill(0)), 0);
}

#[test]
fn an_opaque_full_frame_fill_prices_one_composite_per_pixel() {
    assert_eq!(composites_of(&full_frame_fill(255)), u64::from(W * H));
}
