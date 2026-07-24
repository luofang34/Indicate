//! Graceful stream-finish bounds.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use super::{MockChannel, Open, Tally, Write, drain_test_frames, queue};

#[tokio::test(start_paused = true)]
async fn a_stalled_finish_is_bounded_and_reset() {
    let mut rx = queue(1).await;
    let tally = Arc::new(Tally::default());
    let channel = MockChannel {
        script: vec![Open::Ready(Write::FinishStall)],
        tally: tally.clone(),
    };
    let outer_bound = super::super::STREAM_FINISH_DEADLINE + Duration::from_secs(1);

    let completed = tokio::time::timeout(outer_bound, drain_test_frames(&channel, &mut rx)).await;

    assert!(
        completed.is_ok(),
        "writer shutdown must complete within its finish deadline"
    );
    assert_eq!(
        tally.finished.load(Ordering::SeqCst),
        1,
        "graceful finish is attempted once"
    );
    assert_eq!(
        tally.reset.load(Ordering::SeqCst),
        1,
        "a finish that misses its deadline is reset exactly once"
    );
}
