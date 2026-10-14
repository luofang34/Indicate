# Cookbook: feed a simulator or a vehicle

A feeder turns a source's bytes into an `AircraftState` the host can
admit, without trusting the source. This page lists the steps; the
[X-Plane adapter](source-xplane.md) is the worked example.

## Steps

1. **Decode the wire in the shell.** The feeder crate takes no protocol
   types. Turn each packet into plain numbers and a `RawStamp`: role,
   integrity class, source id, incarnation, epoch, sequence, acquisition
   time, and clock domain. A simulator's data carries
   `ROLE_SIMULATION_TRUTH` on `CLOCK_SIMULATION`; a vehicle's estimate
   carries `ROLE_OPERATIONAL_ESTIMATE` on its own clock.
2. **Synthesize what the wire lacks, honestly.** A source without a
   sequence gets a wrapping counter you advance per publication. A
   source without an acquisition clock gets the simulation clock, and a
   repeated clock is refused as a time regression, which is the right
   outcome for a paused simulator.
3. **Offer attitude and kinematics to the avionics gate.** Configure
   `AvionicsIngress` with the role, pin the source and incarnation, and
   call `ingest` with the host's millisecond clock. The gate refuses
   replays, reorders, clock changes, and an untrusted host clock, and
   counts each refusal.
4. **Map the admitted lanes once.** `aircraft_state::apply_avionics`
   writes attitude, kinematics, quality, the authorized valid flags, and
   coherence. Do not copy the gate's generation into the snapshot; the
   host owns that counter.
5. **Build the other groups from their own datarefs.** Age each group
   from its oldest constituent's receipt, leave it absent until every
   constituent has arrived, and convert units once, into SI, radians,
   and NED.
6. **Admit through the host.** `Host::admit_snapshot` stamps the
   generation; `Host::paint` resolves, steps the alerts, and draws on
   the paint clock whether or not a packet arrived.
7. **Carry provenance to the label.** `AvionicsIngress::provenance`
   says whether the data is simulation. The shell paints the simulation
   label from it ([`AIR-FLAG-007`](requirements.md#air-flag-007)).
8. **Route intents back.** An admitted request comes back from the host
   as `Routed::ToSource`; translate it to the source's own write and
   wait for the echo. Never show the requested value.

## What to test without a socket

Encode and decode the packet layouts byte for byte. Feed a full
publication and resolve the state through the instrument model; assert
`Valid`. Feed a partial one; assert `Missing`. Repeat the source clock;
assert the truth lane goes `Stale` then `Failed`. Pass a NaN host clock;
assert nothing is fresh.

## Requirements this answers to

[`AIR-IN-008`](requirements.md#air-in-008) for identity and ordering
evidence, [`AIR-IN-009`](requirements.md#air-in-009) for per-signal
validity, [`AIR-UNAV-004`](requirements.md#air-unav-004) for what
happens when time or coherence is lost.
