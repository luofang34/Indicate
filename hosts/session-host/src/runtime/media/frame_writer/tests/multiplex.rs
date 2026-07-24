//! The multiplexing invariant: one stream carries a source's whole frame
//! sequence.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use super::{MockChannel, Open, Tally, Write, drain_test_frames, queue};

/// THE Safari regression guard: many frames must ride ONE stream.
///
/// A receiver that never returns the connection-level flow-control window
/// consumed by CLOSED streams (WebKit) exhausts its session budget after a
/// fixed volume of video and stops consuming forever. Opening a stream per
/// frame is therefore not an implementation detail but a defect: this pins
/// the invariant that a healthy source opens exactly one stream no matter
/// how many frames flow, and finishes it once at the end.
#[tokio::test(start_paused = true)]
async fn many_frames_ride_a_single_stream() {
    const FRAMES: usize = 64;
    let mut rx = queue(FRAMES).await;
    let tally = Arc::new(Tally::default());
    let channel = MockChannel {
        script: (0..FRAMES).map(|_| Open::Ready(Write::Ok)).collect(),
        tally: tally.clone(),
    };

    drain_test_frames(&channel, &mut rx).await;

    assert_eq!(
        tally.opened.load(Ordering::SeqCst),
        1,
        "{FRAMES} frames must open exactly ONE stream: a stream per frame \
         leaks the peer's connection window and wedges the session"
    );
    assert_eq!(
        tally.reset.load(Ordering::SeqCst),
        0,
        "a healthy run resets nothing"
    );
    assert_eq!(
        tally.finished.load(Ordering::SeqCst),
        1,
        "the source's stream is finished once, when the writer exits"
    );
}
