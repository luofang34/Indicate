//! Per-(client, source) video frame writer: ONE long-lived host-initiated
//! uni stream per source carrying length-delimited frames, each write under
//! a deadline so a stalled consumer costs a frame (and its stream) rather
//! than wedging its source permanently.
//!
//! Multiplexing rather than a stream per frame is load-bearing for
//! interoperability: a receiver that never returns the connection-level
//! flow-control window consumed by CLOSED streams exhausts its whole session
//! budget after a fixed volume of video and stops consuming forever, while
//! bytes on a live stream are credited normally.

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use pilotage_session::ClientKey;
use tokio::sync::mpsc;
use tokio::time::Instant;
use tracing::{error, warn};
use wtransport::stream::OpeningUniStream;
use wtransport::{Connection, SendStream, VarInt};

use super::budget::PressureSignals;
use super::{EncodedFrame, now_ns};
use crate::runtime::stream_tag::{
    FOURCC_MJPEG, VIDEO_RECORD_PREFIX_LEN, VIDEO_STREAM_V3, frame_video_payload_v2,
};

use classify::{FatalKind, StreamError, classify_open, classify_open_request, classify_write};
use reaper::OpenReapers;

mod classify;
mod reaper;

/// Longest a single frame's uni-stream write may take before the stream
/// is reset and the writer moves on. A client that stops consuming one
/// stream (per-stream flow control fills) would otherwise park the write
/// forever: the capacity-1 handoff then never frees, every later frame is
/// dropped-to-latest, and that source is dead for that client until
/// reconnect — a wedged stream must cost one frame, not the source.
/// Generous against transient congestion (a healthy frame completes in
/// milliseconds on the deployment link).
const FRAME_WRITE_DEADLINE: Duration = Duration::from_secs(2);

/// A peer that withholds uni-stream credit for this long is starving the
/// connection, not merely delaying one frame. This stage allocates no stream,
/// so cancellation is safe and later frames may retry loudly.
const STREAM_CREDIT_STARVATION_BOUND: Duration = Duration::from_secs(30);

/// Application error code carried on the RESET_STREAM of a
/// deadline-exceeded frame. Informational to the peer, which discards the
/// partial frame regardless of the code.
const STALL_RESET_CODE: u32 = 1;

/// Video body streams yield to reliable session traffic on the same QUIC
/// connection. DATAGRAM frames already precede stream frames in Quinn's packet
/// assembly, while this negative priority keeps bulk video behind control.
const VIDEO_STREAM_PRIORITY: i32 = -10;

/// One per-frame outbound stream. `write_all`/`finish` are the clean send
/// path; `reset` is the explicit RESET_STREAM a deadline-exceeded frame
/// needs. Dropping a wtransport/Quinn `SendStream` attempts a graceful
/// FIN, not a reset — a stalled peer never drains it, so truncated
/// streams would linger and eventually exhaust its stream allowance.
/// Resetting the retained stream frees its slot immediately. Send-bounded
/// because the writer runs as a spawned task on a multi-threaded runtime.
trait FrameStream {
    /// Assigns the stream's transport scheduler priority.
    fn set_priority(&self, priority: i32);
    /// Writes the whole buffer, awaiting flow-control credit.
    fn write_all(&mut self, buf: &[u8]) -> impl Future<Output = Result<(), StreamError>> + Send;
    /// Finishes the stream cleanly (graceful FIN).
    fn finish(&mut self) -> impl Future<Output = Result<(), StreamError>> + Send;
    /// Resets the stream (RESET_STREAM); a no-op on an already-closed one.
    fn reset(&mut self);
}

/// Opens per-frame outbound streams on a connection.
trait FrameChannel {
    /// The stream this channel opens.
    type Stream: FrameStream + Send + 'static;
    /// The allocated stream while its WebTransport header is being flushed.
    type Opening: Send + 'static;
    /// Waits for stream credit, returning only after allocation.
    fn request_open(&self) -> impl Future<Output = Result<Self::Opening, StreamError>> + Send;
    /// Flushes the WebTransport stream header for an allocated stream.
    fn finish_open(
        opening: Self::Opening,
    ) -> impl Future<Output = Result<Self::Stream, StreamError>> + Send + 'static;
}

impl FrameStream for SendStream {
    fn set_priority(&self, priority: i32) {
        SendStream::set_priority(self, priority);
    }

    async fn write_all(&mut self, buf: &[u8]) -> Result<(), StreamError> {
        SendStream::write_all(self, buf)
            .await
            .map_err(|e| classify_write(&e, "write"))
    }

    async fn finish(&mut self) -> Result<(), StreamError> {
        SendStream::finish(self)
            .await
            .map_err(|e| classify_write(&e, "finish"))
    }

    fn reset(&mut self) {
        // Best-effort: a stream already finished or reset returns
        // ClosedStream, which is nothing to act on here.
        SendStream::reset(self, VarInt::from_u32(STALL_RESET_CODE)).ok();
    }
}

impl FrameChannel for Connection {
    type Stream = SendStream;
    type Opening = OpeningUniStream;

    async fn request_open(&self) -> Result<OpeningUniStream, StreamError> {
        self.open_uni().await.map_err(classify_open_request)
    }

    async fn finish_open(opening: OpeningUniStream) -> Result<SendStream, StreamError> {
        opening.await.map_err(|e| classify_open(&e))
    }
}

/// Per-(client, source) writer: receives the latest encoded frame and writes
/// it as one host-initiated uni stream tagged [`VIDEO_FRAME_V2`], one stream
/// per frame (ADR-0005, ADR-0020), leading with the frame's capture identity.
/// Exits when the handoff channel closes (client deregistered or media task
/// shutting down).
pub(super) async fn client_writer(
    client: ClientKey,
    source_id: u8,
    connection: Connection,
    mut frames: mpsc::Receiver<EncodedFrame>,
    pressure: Arc<PressureSignals>,
    start: Instant,
) {
    drain_frames(client, source_id, &connection, &mut frames, pressure, start).await;
}

/// What one frame's delivery attempt produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeadlinePhase {
    CreditWait,
    HeaderFlush,
    Write,
}

impl DeadlinePhase {
    fn as_str(self) -> &'static str {
        match self {
            Self::CreditWait => "open_credit",
            Self::HeaderFlush => "open_header",
            Self::Write => "write",
        }
    }
}

#[derive(Debug)]
enum FrameOutcome {
    /// Written and finished cleanly.
    Sent,
    /// The frame exceeded its deadline. `reset` is true when the stream
    /// was immediately reset. `reaper_owned` is true when an allocated
    /// stream is still opening under detached ownership.
    Stalled {
        phase: DeadlinePhase,
        reset: bool,
        reaper_owned: bool,
    },
    /// The peer stopped or refused this stream alone; the connection and
    /// other sources are healthy — one-frame loss, keep writing.
    PeerStop {
        /// The phase (`open`, `write`, `finish`) that surfaced it.
        phase: &'static str,
        /// The peer's application error code, when it carried one.
        code: Option<u64>,
    },
    /// The stream was already closed locally — a frame-local anomaly,
    /// recoverable, but not a peer stop/refusal.
    LocalClose {
        /// The phase that surfaced it.
        phase: &'static str,
    },
    /// Connection-level loss or protocol failure; retire the writer.
    ConnectionFatal {
        /// The phase that surfaced it.
        phase: &'static str,
        /// The preserved underlying kind.
        kind: FatalKind,
    },
}

/// The running per-writer loss counters, kept distinguishable so logs and
/// metrics separate deadline stalls, peer-local stop/refusal, and
/// local closes from the one connection-fatal event that ends the writer.
#[derive(Default)]
struct LossCounters {
    stalls: u64,
    peer_drops: u64,
    local_closes: u64,
}

/// Drains the handoff channel, delivering one frame per stream. Stream-credit
/// waits have an allocation-free starvation bound; header flush and writing
/// share a per-frame deadline. Frame-local loss keeps the writer alive, while
/// connection-level loss retires it.
async fn drain_frames<C: FrameChannel>(
    client: ClientKey,
    source_id: u8,
    channel: &C,
    frames: &mut mpsc::Receiver<EncodedFrame>,
    pressure: Arc<PressureSignals>,
    start: Instant,
) {
    let mut counters = LossCounters::default();
    let mut reapers = OpenReapers::new(client, source_id, Arc::clone(&pressure));
    // One stream carries every frame for this source: a receiver that leaks
    // the connection window of CLOSED streams would otherwise wedge after a
    // fixed volume of video.
    let mut stream: Option<C::Stream> = None;
    while let Some(frame) = frames.recv().await {
        // Stamp publication at the moment of write, distinct from the receive
        // stamp taken at dequeue, so a consumer can separate host queueing
        // latency from the capture-to-receipt gap.
        let published_at_ns = now_ns(start);
        let Some(body) = frame_video_payload_v2(
            source_id,
            &frame.capture,
            frame.received_at_ns,
            published_at_ns,
            FOURCC_MJPEG,
            &frame.jpeg,
        ) else {
            // A frame larger than u32::MAX cannot be length-prefixed; skip it
            // without failing the writer (no real camera frame reaches this).
            error!("video frame exceeds u32 length prefix; skipping");
            continue;
        };
        let outcome = deliver_frame(
            &mut stream,
            channel,
            &mut reapers,
            STREAM_CREDIT_STARVATION_BOUND,
            FRAME_WRITE_DEADLINE,
            VIDEO_STREAM_V3,
            &body,
        )
        .await;
        if !record_outcome(client, source_id, outcome, &mut counters, &pressure) {
            return;
        }
    }
    // Deregistration or shutdown: end the source's stream cleanly so the
    // client sees a close rather than a reset it would treat as loss.
    if let Some(mut stream) = stream {
        stream.finish().await.ok();
    }
}

/// Folds one frame's outcome into the counters and logs it, returning
/// `false` when the outcome is connection-fatal and the writer must retire.
fn record_outcome(
    client: ClientKey,
    source_id: u8,
    outcome: FrameOutcome,
    counters: &mut LossCounters,
    pressure: &PressureSignals,
) -> bool {
    match outcome {
        FrameOutcome::Sent => {}
        FrameOutcome::Stalled {
            phase,
            reset,
            reaper_owned,
        } => {
            pressure.record_deadline_stall();
            counters.stalls = counters.stalls.wrapping_add(1);
            warn!(
                client = client.as_u64(),
                source_id,
                phase = phase.as_str(),
                total_stalls = counters.stalls,
                stream_reset = reset,
                reaper_owned,
                "video frame exceeded its deadline; continuing with the next frame"
            );
        }
        FrameOutcome::PeerStop { phase, code } => {
            counters.peer_drops = counters.peer_drops.wrapping_add(1);
            warn!(
                client = client.as_u64(),
                source_id,
                phase,
                peer_code = code,
                total_peer_drops = counters.peer_drops,
                "peer stopped or refused this video stream; the connection is healthy, \
                 continuing with the next frame"
            );
        }
        FrameOutcome::LocalClose { phase } => {
            counters.local_closes = counters.local_closes.wrapping_add(1);
            warn!(
                client = client.as_u64(),
                source_id,
                phase,
                total_local_closes = counters.local_closes,
                "video stream was already closed locally; frame-local loss, continuing \
                 with the next frame"
            );
        }
        FrameOutcome::ConnectionFatal { phase, kind } => {
            // Connection-level loss retires the writer; a distinguishable
            // fatal record, not a debug line. `%source` logs the preserved
            // cause (timeout, application-close reason/code, H3, QUIC), not a
            // static string. The connection task's own teardown deregisters
            // this client.
            warn!(
                client = client.as_u64(),
                source_id,
                phase,
                source = %kind,
                total_connection_failures = 1_u64,
                total_stalls = counters.stalls,
                total_peer_drops = counters.peer_drops,
                total_local_closes = counters.local_closes,
                "video writer stopping: connection-level failure"
            );
            return false;
        }
    }
    true
}

/// Writes one frame onto the source's long-lived stream, opening that stream
/// first when there is none. A write that fails or outlives its deadline
/// resets the stream and clears the slot, so the next frame starts a fresh
/// one and the loss costs a frame rather than the source.
async fn deliver_frame<C: FrameChannel>(
    slot: &mut Option<C::Stream>,
    channel: &C,
    reapers: &mut OpenReapers,
    credit_bound: Duration,
    frame_budget: Duration,
    tag: u8,
    body: &[u8],
) -> FrameOutcome {
    let stream = match slot {
        Some(stream) => stream,
        None => match open_frame_stream(channel, reapers, credit_bound, frame_budget, tag).await {
            Ok(opened) => slot.insert(opened),
            Err(outcome) => return outcome,
        },
    };
    let deadline = Instant::now() + frame_budget;
    match tokio::time::timeout_at(deadline, write_record(stream, body)).await {
        Ok(Ok(())) => FrameOutcome::Sent,
        Ok(Err(error)) => {
            reset_slot(slot);
            error.into_outcome()
        }
        Err(_elapsed) => {
            reset_slot(slot);
            FrameOutcome::Stalled {
                phase: DeadlinePhase::Write,
                reset: true,
                reaper_owned: false,
            }
        }
    }
}

/// Resets and discards the current stream so the next frame opens a fresh one.
fn reset_slot<S: FrameStream>(slot: &mut Option<S>) {
    if let Some(mut stream) = slot.take() {
        stream.reset();
    }
}

/// Opens one source's video stream and writes its leading kind tag. The
/// stream-credit wait has its own allocation-free bound; once allocated, the
/// header flush runs under the frame budget and an expired flush transfers
/// its still-live future to the reaper.
async fn open_frame_stream<C: FrameChannel>(
    channel: &C,
    reapers: &mut OpenReapers,
    credit_bound: Duration,
    frame_budget: Duration,
    tag: u8,
) -> Result<C::Stream, FrameOutcome> {
    let Some(reaper_permit) = reapers.reserve().await else {
        error!("video open reaper semaphore closed; retiring writer");
        return Err(FrameOutcome::ConnectionFatal {
            phase: "open_header",
            kind: FatalKind::NotConnected,
        });
    };
    let opening = match tokio::time::timeout(credit_bound, channel.request_open()).await {
        Ok(Ok(opening)) => opening,
        Ok(Err(error)) => return Err(error.into_outcome()),
        Err(_elapsed) => {
            return Err(FrameOutcome::Stalled {
                phase: DeadlinePhase::CreditWait,
                reset: false,
                reaper_owned: false,
            });
        }
    };
    let deadline = Instant::now() + frame_budget;
    let mut completion = Box::pin(C::finish_open(opening));
    let mut stream = match tokio::time::timeout_at(deadline, &mut completion).await {
        Ok(Ok(stream)) => stream,
        Ok(Err(error)) => return Err(error.into_outcome()),
        Err(_elapsed) => {
            reapers.own(completion, reaper_permit);
            return Err(FrameOutcome::Stalled {
                phase: DeadlinePhase::HeaderFlush,
                reset: false,
                reaper_owned: true,
            });
        }
    };
    drop(reaper_permit);
    stream.set_priority(VIDEO_STREAM_PRIORITY);
    match tokio::time::timeout_at(deadline, stream.write_all(&[tag])).await {
        Ok(Ok(())) => Ok(stream),
        Ok(Err(error)) => {
            stream.reset();
            Err(error.into_outcome())
        }
        Err(_elapsed) => {
            stream.reset();
            Err(FrameOutcome::Stalled {
                phase: DeadlinePhase::HeaderFlush,
                reset: true,
                reaper_owned: false,
            })
        }
    }
}

impl StreamError {
    /// Maps a stream error to its frame outcome, preserving the peer-stop,
    /// local-close, and connection-fatal classification.
    fn into_outcome(self) -> FrameOutcome {
        match self {
            StreamError::PeerStop { phase, code } => FrameOutcome::PeerStop { phase, code },
            StreamError::LocalClose { phase } => FrameOutcome::LocalClose { phase },
            StreamError::ConnectionFatal { phase, kind } => {
                FrameOutcome::ConnectionFatal { phase, kind }
            }
        }
    }
}

/// Writes one length-delimited frame record into the source's live stream.
/// The stream is NOT finished: it carries every later frame too.
async fn write_record<S: FrameStream>(stream: &mut S, body: &[u8]) -> Result<(), StreamError> {
    let len = u32::try_from(body.len()).unwrap_or(u32::MAX);
    // Typed against the constant so the wire contract and the prefix width
    // cannot drift apart.
    let prefix: [u8; VIDEO_RECORD_PREFIX_LEN] = len.to_be_bytes();
    stream.write_all(&prefix).await?;
    stream.write_all(body).await
}

#[cfg(test)]
mod classification_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod transport_tests;
