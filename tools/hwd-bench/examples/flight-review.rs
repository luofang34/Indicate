//! Renders identical flight inputs through each production display for visual review.

use indicate_instrument_descriptor::{
    DesignFrame, DrawContext, EMPTY_CONFIG, PanelDescriptor, RenderSpace, SceneTarget,
};
use indicate_instrument_hmd::{HMD_DESCRIPTOR, HMD_GLANCE_DESCRIPTOR};
use indicate_instrument_panels::{AUTOFLIGHT_DESCRIPTOR, PFD_DESCRIPTOR};
use indicate_instrument_raster::{FrameId, FramebufferDims, RenderStatus, render};
use indicate_instrument_scene::{
    SceneWriter,
    angular::{AngularSceneWriter, MAX_ANGULAR_SCENE_BYTES},
};
use indicate_instrument_state::{FreshnessPolicy, abi::v8::fixtures, resolve};
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("drawing review scene: {0}")]
    Draw(#[from] indicate_instrument_descriptor::PanelDrawError),
    #[error("encoding review scene: {0:?}")]
    Scene(indicate_instrument_scene::SceneError),
    #[error("rasterizing panel {panel}: {detail}")]
    Raster { panel: &'static str, detail: String },
    #[error("writing {}: {source}", path.display())]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

fn main() -> Result<(), Error> {
    let directory = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/flight-review"));
    std::fs::create_dir_all(&directory).map_err(|source| Error::Write {
        path: directory.clone(),
        source,
    })?;
    for panel in [
        PFD_DESCRIPTOR,
        AUTOFLIGHT_DESCRIPTOR,
        HMD_DESCRIPTOR,
        HMD_GLANCE_DESCRIPTOR,
    ] {
        export(&directory, &panel)?;
    }
    Ok(())
}

fn export(directory: &Path, panel: &PanelDescriptor) -> Result<(), Error> {
    let data = resolve(&fixtures::extended(), &FreshnessPolicy::default());
    let ctx = DrawContext::new(&data).with_config(EMPTY_CONFIG);
    let mut scene = [0; 65536];
    let mut ang_buf = vec![0u8; MAX_ANGULAR_SCENE_BYTES];
    let mut writer = SceneWriter::new(&mut scene).map_err(Error::Scene)?;
    let frame: DesignFrame = match panel.space {
        RenderSpace::Planar2D { frame_min, .. } => {
            (panel.draw)(
                &ctx,
                SceneTarget::Planar2D {
                    frame: frame_min,
                    writer: &mut writer,
                },
            )?;
            frame_min
        }
        RenderSpace::AngularSphere {
            overlay_frame: Some(frame),
        } => {
            let mut ang_writer = AngularSceneWriter::new(&mut ang_buf).map_err(Error::Scene)?;
            (panel.draw)(
                &ctx,
                SceneTarget::AngularSphere {
                    angular: &mut ang_writer,
                    overlay: Some((frame, &mut writer)),
                },
            )?;
            let _ang_used = ang_writer.finish();
            frame
        }
        _ => {
            return Err(Error::Raster {
                panel: panel.id,
                detail: "no 2D frame".into(),
            });
        }
    };
    let length = writer.finish();
    let (width, height) = (frame.width as u32, frame.height as u32);
    let mut pixels = vec![0; (width * height * 4) as usize];
    let report = render(
        &scene[..length],
        &mut pixels,
        FramebufferDims::tight(width, height),
        FrameId::default(),
    )
    .map_err(|error| Error::Raster {
        panel: panel.id,
        detail: format!("{error:?}"),
    })?;
    if report.status != RenderStatus::Painted {
        return Err(Error::Raster {
            panel: panel.id,
            detail: format!("{:?}", report.status),
        });
    }
    let mut ppm = format!("P6\n{width} {height}\n255\n").into_bytes();
    for pixel in pixels.as_chunks::<4>().0 {
        ppm.extend_from_slice(&pixel[..3]);
    }
    let path = directory.join(format!("{}.ppm", panel.id));
    std::fs::write(&path, ppm).map_err(|source| Error::Write { path, source })
}
