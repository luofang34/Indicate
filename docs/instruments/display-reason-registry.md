# Display-reason registry

This document is the cross-shell contract for display reasons. A display
reason is a machine-readable statement of why a display is not showing
current, valid panel imagery. The Rust shell, the Swift shell, and the
JavaScript shell each report display reasons for the renderer-health
monitor input ([`AIR-IN-013`](requirements.md#air-in-013)). One registry
defines the reasons for all three shells. A cross-shell comparison of
display health then compares states, not vocabularies.

This document is a deterministic engineering contract for simulator and
embedded shells. It does not claim certification credit for a shell.

## The registry

The authoritative enumeration is the `DisplayFault` enum in
`indicate-alerts` (`crates/indicate-alerts/src/condition.rs`). Each
reason has a stable one-byte code. The alert manager packs the code into
a stable two-byte `AlertId` as `0x0500 | code`, where `0x05` is the
display family selector. A shell may use the code or the `AlertId` as
the cross-shell identity of a reason.

| Code | Reason | Identity | Class | Meaning |
|---:|---|---|---|---|
| 1 | `RendererStalled` | `0x0501` | Warning | The renderer stopped making progress. |
| 2 | `FrameGenerationLost` | `0x0502` | Warning | Frame generation stopped advancing. |
| 3 | `CommandBufferCorrupt` | `0x0503` | Warning | The draw-command buffer failed its integrity check. |
| 4 | `BackendLost` | `0x0504` | Caution | The rendering backend was lost. |
| 5 | `RetainedImage` | `0x0505` | Warning | A retained last-good image is suspected on the output path. |

The table lists every reason in ascending code order. `DisplayFault::ALL`
carries the same list in the same order. The registry doc tests in
`crates/indicate-alerts/src/condition/tests.rs` parse this table and fail
the build when the table and the code disagree, so a registry change must
arrive with its row. The section holds this one table and no other.

Annunciation labels such as `DSP STALL` are shell presentation. They are
not part of the registry.

## Append-only rule

The registry is append-only, like the scene opcode vocabulary
(ADR-0017). A new reason takes the next free code. Codes are never
renumbered, reused, or removed. An older shell can then degrade
gracefully on a reason it does not know.

## Unknown reasons on an older shell

A shell that receives a reason code it does not know must map the code
to a generic display-not-current presentation at warning level. The
shell must never map an unknown reason to a healthy display. This rule
is fail-closed, consistent with the Unknown-sentinel convention
(VAL-01): an unrecognized wire value selects the safe outcome, never a
benign one.

The Rust side decodes fail-closed. `DisplayFault::from_code` returns
`None` for a code outside the registry, and `class_of` returns `None`
for the identity that code packs into. `None` is not a healthy state: a
consumer that cannot name the reason must still treat the display as not
current.

## Swift and JavaScript mirrors

The Swift and JavaScript mirrors live in their downstream repositories.
Each mirror must declare the same reasons with the same codes as the
table above. A mirror must not declare a reason that this registry does
not define. Each downstream repository must carry a drift check that
compares its mirror against this table, under the same discipline as the
scene-conformance corpus pins: a registry change here reddens the pinned
consumer at its next pin advance, and that red is the sync mechanism
working.
