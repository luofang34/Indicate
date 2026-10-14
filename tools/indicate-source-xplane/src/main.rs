//! `indicate-source-xplane`: subscribe to a running X-Plane over UDP,
//! drive the reference host on a paint clock, and write PFD frames.
//!
//! The socket, the clock, and the files live here; everything judged
//! lives in the library. The paint clock ticks whether or not a packet
//! arrived, so a simulator that stops publishing is shown as stale data
//! on a live display, never as a frozen frame.

use std::net::{SocketAddr, UdpSocket};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use indicate_host::{Host, HostConfig, HostError, failure_page};
use indicate_instrument_descriptor::EMPTY_CONFIG;
use indicate_instrument_panels::{BUILTIN_FRAME, PFD_DESCRIPTOR};
use indicate_instrument_raster::{FrameId, FramebufferDims, RasterError, render};
use indicate_intent::{Intent, IntentStamp, Request};
use indicate_source_xplane::intents::dref_for;
use indicate_source_xplane::snapshot::SourceFault;
use indicate_source_xplane::wire::{
    DREF_WRITE_LEN, RREF_REQUEST_LEN, encode_dref_write, encode_rref_request,
};
use indicate_source_xplane::{Dataref, SUBSCRIPTION_HZ, XPlaneSource};

const USAGE: &str = "\
indicate-source-xplane [--xplane <addr>] [--out <dir>] [--frames <n>] [--set-baro <hpa>]

  --xplane <addr>   X-Plane's UDP address (default 127.0.0.1:49000)
  --out <dir>       write every 20th PFD frame as a PPM file
  --frames <n>      stop after n paint ticks (default: run until killed)
  --set-baro <hpa>  once connected, request this barometric setting
                    through the intent gate and write it as a dataref";

const TICK: Duration = Duration::from_millis(50);
const SCENE_BYTES: usize = 65_536;
const FRAMES_PER_FILE: u64 = 20;

/// Why the adapter stopped.
#[derive(Debug, thiserror::Error)]
enum SourceError {
    #[error("usage error: {message}\n{USAGE}")]
    Usage { message: String },
    #[error("I/O failure during {context}: {source}")]
    Io {
        context: &'static str,
        #[source]
        source: std::io::Error,
    },
    #[error("wire: {0}")]
    Wire(#[from] indicate_source_xplane::WireError),
    #[error("host: {0}")]
    Host(#[from] HostError),
    #[error("raster: {0}")]
    Raster(#[from] RasterError),
}

struct Args {
    xplane: SocketAddr,
    out: Option<PathBuf>,
    frames: Option<u64>,
    set_baro: Option<f32>,
}

fn main() -> ExitCode {
    tracing_subscriber::fmt::init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    match parse_args(&args).and_then(run) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!(%error, "indicate-source-xplane failed");
            ExitCode::FAILURE
        }
    }
}

fn parse_args(args: &[String]) -> Result<Args, SourceError> {
    let mut parsed = Args {
        xplane: "127.0.0.1:49000".parse().map_err(|_| SourceError::Usage {
            message: "default address did not parse".into(),
        })?,
        out: None,
        frames: None,
        set_baro: None,
    };
    let mut index = 0;
    while let Some(flag) = args.get(index) {
        let value = args.get(index + 1).ok_or_else(|| SourceError::Usage {
            message: format!("{flag} needs a value"),
        });
        match flag.as_str() {
            "--xplane" => {
                let text = value?;
                parsed.xplane = text.parse().map_err(|_| SourceError::Usage {
                    message: format!("--xplane {text:?} is not host:port"),
                })?;
            }
            "--out" => parsed.out = Some(PathBuf::from(value?)),
            "--frames" => {
                let text = value?;
                parsed.frames = Some(text.parse().map_err(|_| SourceError::Usage {
                    message: format!("--frames {text:?} is not a count"),
                })?);
            }
            "--set-baro" => {
                let text = value?;
                parsed.set_baro = Some(text.parse().map_err(|_| SourceError::Usage {
                    message: format!("--set-baro {text:?} is not a number"),
                })?);
            }
            other => {
                return Err(SourceError::Usage {
                    message: format!("unknown flag {other:?}"),
                });
            }
        }
        index += 2;
    }
    Ok(parsed)
}

fn run(args: Args) -> Result<(), SourceError> {
    let socket = UdpSocket::bind("0.0.0.0:0").map_err(|source| SourceError::Io {
        context: "binding the UDP socket",
        source,
    })?;
    socket
        .set_read_timeout(Some(Duration::from_millis(5)))
        .map_err(|source| SourceError::Io {
            context: "setting the socket timeout",
            source,
        })?;
    subscribe(&socket, args.xplane)?;
    tracing::info!(xplane = %args.xplane, "subscribed to X-Plane datarefs");

    let started = Instant::now();
    let mut source = XPlaneSource::new();
    let mut host = Host::new(
        HostConfig::default(),
        indicate_alerts::AlertProfile::simulator(),
    );
    let mut scene = vec![0u8; SCENE_BYTES];
    let mut ticks = 0u64;
    let mut baro_requested = false;
    loop {
        let now_ms = started.elapsed().as_secs_f64() * 1000.0;
        drain(&socket, &mut source, now_ms);
        if let Some(hpa) = args.set_baro
            && !baro_requested
            && source.counters().packets > 0
        {
            request_baro(&socket, args.xplane, &mut host, hpa, now_ms)?;
            baro_requested = true;
        }
        host.admit_snapshot(source.snapshot(now_ms), now_ms)?;
        tick(&mut host, &mut scene, now_ms, ticks, args.out.as_deref())?;
        ticks = ticks.wrapping_add(1);
        if args.frames.is_some_and(|limit| ticks >= limit) {
            return Ok(());
        }
        std::thread::sleep(TICK);
    }
}

fn subscribe(socket: &UdpSocket, xplane: SocketAddr) -> Result<(), SourceError> {
    let mut packet = [0u8; RREF_REQUEST_LEN];
    for dataref in Dataref::ALL {
        encode_rref_request(
            &mut packet,
            SUBSCRIPTION_HZ,
            dataref.index(),
            dataref.path(),
        )?;
        socket
            .send_to(&packet, xplane)
            .map_err(|source| SourceError::Io {
                context: "sending an RREF request",
                source,
            })?;
    }
    Ok(())
}

/// Applies every packet waiting on the socket. A refused packet is
/// logged and skipped; the paint clock does not wait for it.
fn drain(socket: &UdpSocket, source: &mut XPlaneSource, now_ms: f64) {
    let mut buf = [0u8; 2048];
    while let Ok((len, _)) = socket.recv_from(&mut buf) {
        match source.observe(&buf[..len], now_ms) {
            Ok(_) => {}
            Err(SourceFault::Wire(error)) => tracing::warn!(%error, "packet refused"),
            Err(SourceFault::InvalidClock) => tracing::error!("host clock refused"),
        }
    }
}

fn request_baro(
    socket: &UdpSocket,
    xplane: SocketAddr,
    host: &mut Host,
    hpa: f32,
    now_ms: f64,
) -> Result<(), SourceError> {
    let request = Request {
        stamp: IntentStamp {
            device_id: 1,
            sequence: 1,
            issued_at_ms: now_ms as u64,
        },
        intent: Intent::SelectBaro { hpa },
    };
    match host.request(&request, now_ms) {
        Ok(routed) => tracing::info!(?routed, "baro request admitted"),
        Err(refused) => {
            tracing::warn!(%refused, "baro request refused");
            return Ok(());
        }
    }
    let Some((dataref, value)) = dref_for(&request.intent) else {
        return Ok(());
    };
    let mut packet = [0u8; DREF_WRITE_LEN];
    encode_dref_write(&mut packet, value, dataref.path())?;
    socket
        .send_to(&packet, xplane)
        .map_err(|source| SourceError::Io {
            context: "sending a DREF write",
            source,
        })?;
    tracing::info!(
        dataref = dataref.path(),
        value,
        "dataref written; awaiting echo"
    );
    Ok(())
}

/// One paint-clock tick: a frame or the failure page, then optionally a
/// file.
fn tick(
    host: &mut Host,
    scene: &mut [u8],
    now_ms: f64,
    ticks: u64,
    out: Option<&std::path::Path>,
) -> Result<(), SourceError> {
    let scene_len = match host.paint(
        &PFD_DESCRIPTOR,
        &EMPTY_CONFIG,
        BUILTIN_FRAME,
        &[],
        scene,
        now_ms,
    ) {
        Ok(painted) => {
            if painted
                .alerts
                .active()
                .iter()
                .any(|alert| alert.class == indicate_alerts::AlertClass::Warning)
            {
                tracing::warn!(render = painted.render_generation, "warning active");
            }
            painted.scene_len
        }
        Err(HostError::InvalidClock) => return Err(HostError::InvalidClock.into()),
        Err(error) => {
            tracing::error!(%error, "paint refused; showing the failure page");
            failure_page(scene, BUILTIN_FRAME, "DISPLAY FAIL").map_err(HostError::Scene)?
        }
    };
    if let Some(dir) = out
        && ticks.is_multiple_of(FRAMES_PER_FILE)
    {
        write_frame(dir, ticks, &scene[..scene_len], host.render_generation())?;
    }
    Ok(())
}

fn write_frame(
    dir: &std::path::Path,
    ticks: u64,
    scene: &[u8],
    render_generation: u32,
) -> Result<(), SourceError> {
    let (w, h) = (BUILTIN_FRAME.width as u32, BUILTIN_FRAME.height as u32);
    let mut pixels = vec![0u8; (w * h * 4) as usize];
    render(
        scene,
        &mut pixels,
        FramebufferDims::tight(w, h),
        FrameId {
            frame_generation: 0,
            render_generation,
        },
    )?;
    let mut ppm = format!("P6\n{w} {h}\n255\n").into_bytes();
    for px in pixels.as_chunks::<4>().0 {
        ppm.extend_from_slice(&px[..3]);
    }
    std::fs::create_dir_all(dir).map_err(|source| SourceError::Io {
        context: "creating the output directory",
        source,
    })?;
    let path = dir.join(format!("pfd-{ticks:06}.ppm"));
    std::fs::write(&path, ppm).map_err(|source| SourceError::Io {
        context: "writing a frame",
        source,
    })?;
    tracing::info!(path = %path.display(), "frame written");
    Ok(())
}
