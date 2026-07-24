//! Failure classification for one video stream's lifecycle: which errors
//! cost a frame, which cost the stream, and which retire the writer.

use wtransport::error::{ConnectionError, StreamOpeningError, StreamWriteError};

/// Preserves the concrete cause behind a connection-fatal stream failure.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(super) enum FatalKind {
    /// The connection has been dropped.
    #[error("not connected")]
    NotConnected,
    /// A QUIC protocol error.
    #[error("QUIC protocol error")]
    QuicProto,
    /// The uni-stream open request itself failed.
    #[error("uni stream request failed: {0}")]
    OpenRequest(#[source] ConnectionError),
}

/// Separates frame-local loss from connection-fatal loss.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(super) enum StreamError {
    /// The peer stopped or refused this stream alone (`Stopped`,
    /// `Refused`): a peer-attributed one-frame loss; the connection and
    /// every other source are unaffected.
    #[error("peer stopped or refused the stream at {phase}")]
    PeerStop {
        /// The phase that surfaced it (`open`, `write`, `finish`).
        phase: &'static str,
        /// The peer's application error code, when it carried one.
        code: Option<u64>,
    },
    /// The stream was already closed locally (`Closed`): a frame-local
    /// anomaly, recoverable like a peer stop but NOT a peer stop/refusal.
    #[error("stream already closed locally at {phase}")]
    LocalClose {
        /// The phase that surfaced it.
        phase: &'static str,
    },
    /// Connection-level loss or a protocol failure: the writer must
    /// retire — no further frame can be delivered on this connection.
    #[error("connection-fatal at {phase}: {kind}")]
    ConnectionFatal {
        /// The phase that surfaced it.
        phase: &'static str,
        /// The preserved underlying kind (its own `#[source]` chain).
        #[source]
        kind: FatalKind,
    },
}

pub(super) fn classify_write(error: &StreamWriteError, phase: &'static str) -> StreamError {
    match error {
        StreamWriteError::Stopped(code) => StreamError::PeerStop {
            phase,
            code: Some(code.into_inner()),
        },
        StreamWriteError::Closed => StreamError::LocalClose { phase },
        StreamWriteError::NotConnected => StreamError::ConnectionFatal {
            phase,
            kind: FatalKind::NotConnected,
        },
        StreamWriteError::QuicProto => StreamError::ConnectionFatal {
            phase,
            kind: FatalKind::QuicProto,
        },
    }
}

/// Classifies a refused stream as frame-local and a lost connection as fatal.
pub(super) fn classify_open(error: &StreamOpeningError) -> StreamError {
    match error {
        StreamOpeningError::Refused => StreamError::PeerStop {
            phase: "open",
            code: None,
        },
        StreamOpeningError::NotConnected => StreamError::ConnectionFatal {
            phase: "open",
            kind: FatalKind::NotConnected,
        },
    }
}

/// Classifies an open-request [`ConnectionError`] (the first `open_uni`
/// await): always connection-fatal, but the concrete cause is retained
/// rather than discarded to a static string.
pub(super) fn classify_open_request(error: ConnectionError) -> StreamError {
    StreamError::ConnectionFatal {
        phase: "open",
        kind: FatalKind::OpenRequest(error),
    }
}
