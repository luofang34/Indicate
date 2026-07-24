// One long-lived uni stream's body reader, factored out of main.js so the
// kind-tag peel and the LIVE authority-envelope dispatch can be tested against
// real ReadableStreams (the core of `readOneUniStream`).
//
// Every incoming uni stream leads with a one-byte kind tag (0x01
// reliable session events, 0x02/0x03 one video frame). The event stream is
// long-lived and NEVER closes during a session, so its envelopes must be
// decoded and dispatched AS THEY COMPLETE — buffering to close would strand a
// recovery acknowledgement forever. A video stream is one frame in its whole
// body, rendered from the returned tail at close.

import { drainAuthorityEnvelopes } from "./authority-stream.js";
import { streamCancellationReason } from "./stream-cancellation.js";
/** Kind tag of a LONG-LIVED per-source video stream: the tag once, then a
 *  sequence of length-delimited frame bodies. One stream per source keeps the
 *  connection's flow-control window circulating — an engine that never returns
 *  the window consumed by CLOSED streams (WebKit) wedges permanently after a
 *  fixed volume of stream-per-frame video. */
export const STREAM_KIND_VIDEO_STREAM = 0x04;

/** Byte count prefixing each frame record inside a video stream. */
export const VIDEO_RECORD_PREFIX_LEN = 4;

/**
 * Hands every COMPLETE `[u32 BE length][body]` record in `buf` to
 * `onRecord`, returning the leftover partial tail for the next chunk. A
 * record whose length has arrived but whose body has not is left intact:
 * length-delimited framing means a short read is never a parse failure.
 */
export async function drainVideoRecords(buf, onRecord) {
  for (;;) {
    if (buf.length < VIDEO_RECORD_PREFIX_LEN) return buf;
    const view = new DataView(buf.buffer, buf.byteOffset, VIDEO_RECORD_PREFIX_LEN);
    const length = view.getUint32(0, false);
    const end = VIDEO_RECORD_PREFIX_LEN + length;
    if (buf.length < end) return buf;
    // Copy the record out: the caller keeps it across awaits while later
    // chunks reuse the underlying buffer.
    await onRecord(buf.slice(VIDEO_RECORD_PREFIX_LEN, end));
    buf = buf.subarray(end);
  }
}

/** Appends `incoming` after `existing`. */
function appendBytes(existing, incoming) {
  const out = new Uint8Array(existing.length + incoming.length);
  out.set(existing, 0);
  out.set(incoming, existing.length);
  return out;
}

/**
 * Reads one uni stream to close from `reader`: peels the leading one-byte kind
 * tag, and for `authorityKind` decodes and dispatches every COMPLETE envelope
 * live through `onAuthorityEnvelope`. Returns `{ kind, tail, aborted }` — the
 * caller renders a video body from `tail` at close (authority has already
 * dispatched incrementally). `shouldContinue()`, when supplied, aborts the read
 * on session teardown and reports `aborted: true` so the caller skips its
 * close-time work. The `authorityKind` callback receives every typed envelope
 * carried on the reliable session-events stream.
 *
 * @param reader a `ReadableStreamDefaultReader` over the stream's bytes
 * @param cb `{ authorityKind, decode, onAuthorityEnvelope, shouldContinue }`
 */
export async function readUniStream(
  reader,
  {
    authorityKind,
    decode,
    onAuthorityEnvelope,
    videoStreamKind,
    onVideoRecord,
    shouldContinue,
    onCancelFailure,
  },
) {
  async function cancel(kind, cause = null) {
    const reason = streamCancellationReason(kind, cause);
    try {
      await reader.cancel(reason);
    } catch (error) {
      onCancelFailure?.(error, reason);
    }
  }

  let buf = new Uint8Array(0);
  let kind = null;
  try {
    for (;;) {
      const { value, done } = await reader.read();
      if (shouldContinue && !shouldContinue()) {
        await cancel("stream-abandoned");
        return { kind, tail: buf, aborted: true };
      }
      if (value) buf = appendBytes(buf, value);
      // The one-byte kind tag leads the stream; peel it once it arrives.
      if (kind === null && buf.length >= 1) {
        kind = buf[0];
        buf = buf.subarray(1);
      }
      // Authority is long-lived: dispatch every complete envelope live so a
      // recovery ack is acted on immediately, never buffered until a close that
      // never comes.
      if (kind === authorityKind) {
        buf = drainAuthorityEnvelopes(buf, decode, onAuthorityEnvelope);
      }
      // A video stream is long-lived too: every complete record is one frame
      // and must paint as it arrives, never buffered to a close that only
      // comes at session end.
      if (kind === videoStreamKind) {
        buf = await drainVideoRecords(buf, onVideoRecord);
      }
      if (done) break;
    }
  } catch (error) {
    await cancel("stream-read-failed", error);
    throw error;
  }
  return { kind, tail: buf, aborted: false };
}
