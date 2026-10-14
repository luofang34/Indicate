# Operator intent contract

This contract defines the return path from a human controller to the
system: the `indicate-intent` crate (INT-01). It grants no control
credit. A simulator adapter turns an admitted request into its own
write; nothing in this family commands an aircraft.

## Boundary

An operator input device (a bezel key, a knob panel, a touch surface)
produces a `Request`: an `IntentStamp` and an `Intent`. The stamp
carries the device identity, a wrapping per-device sequence, and the
device's own clock. The intent is one typed selection or display
action. The shell owns the device, its decoding, and its transport.
The crate owns admission and the echo rule, once, for every shell.

Crew selections reach the display only through the selections group
([`AIR-IN-007`](requirements.md#air-in-007)). A request does not write
that group.

## The echo rule

An intent is a request, never a value. The display paints only what
the source reports back. `EchoMonitor` holds each admitted selection
request and compares it with the state on every frame:

| Status | Meaning | Shell obligation |
|---|---|---|
| `Pending` | Requested, not yet echoed, inside the confirmation window | Show the state's value; may mark the selection as pending |
| `Confirmed` | The state reports the requested value | None; reported once |
| `Overdue` | Requested, not echoed, past the window | Annunciate that the request was not honoured; never show the requested value in its place |
| `Unmeasurable` | A request is outstanding and the host clock cannot age it | Treat as overdue |

Identity fields (heading reference, altitude class, origin, geoid
model, navigation source) must match exactly. Numbers match within a
small tolerance, because the wire carries `f32` and a source may round
a setting to its own resolution. A newer request of the same kind
replaces the outstanding one, and the old value echoing back does not
confirm it.

## Admission

`IntentGate` admits requests from one pinned device. It refuses, with
a typed `IntentFault`, a request from another device, a sequence that
is not newer in wrapping serial order
([`AIR-IN-008`](requirements.md#air-in-008)), a host clock reading it
cannot trust, a value that is not finite or outside its bounds, and a
reference the state cannot name (`Unknown`). A refused request reaches
no source, no manager, and no display, and it does not advance the
gate.

| Intent | Bounds |
|---|---|
| Barometric setting | 800 to 1100 hPa |
| Heading bug | 0 to 2π radians, against a named reference |
| Selected altitude | −1000 to 30 000 m, against a named class |
| Navigation source | A named source |

Alert acknowledgements are routed by the host to the central alert
manager ([`AIR-IN-012`](requirements.md#air-in-012)); page selection and
declutter are routed to the display. None of them reaches a source.

## Verification

`cargo test -p indicate-intent` covers device pinning, wrapping serial
order, the clock rule, every bound and unknown reference, confirmation
by echo, overdue silence, replacement by a newer request, and the
unmeasurable case. The crate is `no_std` and is compiled for the
bare-metal target in CI.

## Not yet in this contract

- A wire encoding for requests. Shells pass Rust values; a byte
  encoding shared with non-Rust shells is a later, versioned addition.
- A descriptor field declaring which intents a panel accepts, so that
  admission can check a panel with a softkey region declares them.
- A simulator adapter. The X-Plane source and the reference host are
  where an admitted request becomes a dataref write.
