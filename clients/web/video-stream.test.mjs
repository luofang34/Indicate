// Framing contract for the long-lived per-source video stream.
//
// One stream per source carries every frame as `[u32 BE length][body]`. The
// reader must paint each record AS IT COMPLETES — the stream only closes at
// session end, so buffering to close would freeze the picture forever — and
// must carry a partial tail across chunk boundaries, because a length-
// delimited record split across TCP/QUIC reads is normal, not corruption.
//
// Run: node clients/web/video-stream.test.mjs

import assert from "node:assert/strict";

import {
  STREAM_KIND_VIDEO_STREAM,
  VIDEO_RECORD_PREFIX_LEN,
  drainVideoRecords,
  readUniStream,
} from "./uni-stream.js";

function record(body) {
  const out = new Uint8Array(VIDEO_RECORD_PREFIX_LEN + body.length);
  new DataView(out.buffer).setUint32(0, body.length, false);
  out.set(body, VIDEO_RECORD_PREFIX_LEN);
  return out;
}

const concat = (...parts) => {
  const total = parts.reduce((sum, part) => sum + part.length, 0);
  const out = new Uint8Array(total);
  let at = 0;
  for (const part of parts) {
    out.set(part, at);
    at += part.length;
  }
  return out;
};

// ---- complete records drain; a partial tail is preserved -------------------
{
  const seen = [];
  const two = concat(record(Uint8Array.of(1, 2, 3)), record(Uint8Array.of(9)));
  const tail = await drainVideoRecords(two, (body) => seen.push([...body]));
  assert.deepEqual(seen, [[1, 2, 3], [9]], "both complete records drain");
  assert.equal(tail.length, 0, "nothing is left over");

  // A record whose length arrived but whose body has not: keep it whole.
  const partial = concat(record(Uint8Array.of(4, 5, 6)).subarray(0, 5));
  const kept = await drainVideoRecords(partial, () => assert.fail("incomplete must not drain"));
  assert.equal(kept.length, 5, "the partial record is carried forward");

  // A prefix shorter than the length field itself is also carried.
  const stub = Uint8Array.of(0, 0);
  assert.equal(
    (await drainVideoRecords(stub, () => assert.fail("no record yet"))).length,
    2,
    "a truncated length prefix waits for more bytes",
  );
}

// ---- the reader paints records live, across chunk boundaries ---------------
{
  const body = Uint8Array.of(7, 7, 7, 7, 7);
  const framed = concat(Uint8Array.of(STREAM_KIND_VIDEO_STREAM), record(body), record(body));
  // Split mid-record so a naive reader that waits for a boundary fails.
  const chunks = [framed.subarray(0, 4), framed.subarray(4, 11), framed.subarray(11)];
  let index = 0;
  const painted = [];
  let closed = false;
  const reader = {
    read: async () => {
      if (index >= chunks.length) {
        // The stream stays OPEN after the records, exactly like a live
        // session: the reader must have painted without waiting for this.
        assert.equal(painted.length, 2, "both frames painted before any close");
        closed = true;
        return { value: undefined, done: true };
      }
      const value = chunks[index];
      index += 1;
      return { value, done: false };
    },
    cancel: async () => {},
  };
  const { kind, aborted } = await readUniStream(reader, {
    authorityKind: 0x01,
    decode: () => null,
    onAuthorityEnvelope: () => {},
    videoStreamKind: STREAM_KIND_VIDEO_STREAM,
    onVideoRecord: (frame) => painted.push([...frame]),
    shouldContinue: () => true,
  });
  assert.equal(kind, STREAM_KIND_VIDEO_STREAM, "the kind tag is peeled once");
  assert.equal(aborted, false);
  assert.deepEqual(painted, [[...body], [...body]], "each record paints once, in order");
  assert.ok(closed, "the reader consumed the whole stream");
}

// ---- a record body is COPIED, not aliased into the read buffer -------------
{
  const seen = [];
  const buffer = concat(record(Uint8Array.of(1, 1)), record(Uint8Array.of(2, 2)));
  await drainVideoRecords(buffer, (body) => seen.push(body));
  // Mutating the source afterwards must not corrupt an already-handed frame:
  // the renderer holds the body across awaits while later chunks arrive.
  buffer.fill(0xff);
  assert.deepEqual([...seen[0]], [1, 1], "the first record survives buffer reuse");
  assert.deepEqual([...seen[1]], [2, 2], "the second record survives buffer reuse");
}

console.log("video stream framing contract passed");
