# X-Plane source adapter

`tools/indicate-source-xplane` feeds a running X-Plane into the
reference host over the simulator's UDP data interface, and writes
admitted operator intents back as datarefs. It is a **SIM / NOT FOR
FLIGHT** engineering port ([`AIR-BAS-001`](requirements.md#air-bas-001)).
The simulator is an untrusted source like any other: its data is
admitted, aged, and labelled, never trusted because it is a simulator.

## What was not verified

The dataref names, their units, and the packet layouts below are taken
from X-Plane's documented UDP interface. They were not checked against
a running simulator when this adapter was written. A mismatch is a
table edit in `datarefs.rs`, not a design change; the wire tests pin
the layouts this adapter expects, so a change there is deliberate.

## Shape

| Stage | Where | Rule |
|---|---|---|
| Subscribe | `wire::encode_rref_request` | One `RREF` request per dataref at 20 Hz; the index is the dataref's position in `Dataref::ALL`. |
| Decode | `wire::decode_rref_reply` | A reply that is not `RREF` or ends inside a record is refused whole. A record for an index this build never subscribed is counted and skipped; a non-finite value is skipped. |
| Truth lane | `snapshot::XPlaneSource` | Attitude and kinematics are offered to the feeder's avionics gate in the simulation-truth role, stamped on the simulation clock with a synthesized wrapping sequence ([`AIR-IN-008`](requirements.md#air-in-008)). A paused simulator repeats its clock; the gate refuses the repeat as a time regression and the groups age into `Stale` and then `Failed`. |
| Other groups | `snapshot::XPlaneSource` | Air data, altitude, heading, variation, wind, and selections are built from their own datarefs, aged from the oldest constituent's receipt, and absent until every constituent has arrived. |
| Host | the binary | One snapshot admitted per tick, one frame painted per tick on a 50 ms clock whether or not a packet arrived, the failure page when a frame is refused. |
| Intents | `intents::dref_for` | A barometric setting is written in inches of mercury, a magnetic heading bug in degrees, a barometric altitude selection in feet. A request in another reference is not silently converted and is not written. Confirmation is the echo through the selections group, never the write. |

Provenance survives: the truth lane reports the simulation role and
clock, and the shell owes the simulation label from it
([`AIR-FLAG-007`](requirements.md#air-flag-007)). This adapter logs the
provenance; the shipped panels do not yet paint the label.

## Conversions

| From the simulator | Into the state |
|---|---|
| `theta`, `phi`, `psi` degrees | Body-to-NED quaternion through `Quat::from_euler` |
| `P`, `Q`, `R` degrees per second | Body rates, radians per second |
| `local_x`, `local_y`, `local_z` (east, up, south) | NED position: north is `-z`, east is `x`, down is `-y` |
| `indicated_airspeed` knots | Meters per second |
| `barometer_setting` inches of mercury | Hectopascals |
| `altitude_ft_pilot` feet | Barometric-indicated altitude, meters, origin `BARO_ORIGIN` |
| `magpsi` degrees | Magnetic heading, radians |
| `psi` minus `magpsi` | Magnetic variation, east positive; the simulator's own variation dataref is not read, so no sign convention is assumed |
| `wind_heading_deg_mag`, `wind_speed_kts` | Wind from-direction in true through the reported variation, speed in meters per second |
| `heading_mag`, `altitude` (autopilot) | Selections: heading bug against the magnetic reference, altitude in the barometric class |

## Running it

```sh
cargo run -p indicate-source-xplane -- --xplane 127.0.0.1:49000 --out target/xplane-frames --frames 200
```

Every twentieth frame is written as a PPM file through the reference
rasterizer. `--set-baro 1013.2` requests one barometric setting through
the intent gate once packets are flowing and writes the dataref; the
log then shows the request pending until the simulator echoes it.

## Verification

`cargo test -p indicate-source-xplane` pins the packet layouts, the
index round trip, the unit and frame conversions, the state mapping
resolved through the instrument model, the paused-simulator behaviour,
the untrusted-clock behaviour, and the intent-to-dataref mapping. No
test opens a socket.
