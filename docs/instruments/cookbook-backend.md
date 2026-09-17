# Cookbook: write a backend

A backend interprets the scene IR and puts pixels on a surface. The
[backend contract](backend-contract.md) is binding; this page is the
order of work.

## Steps

1. **Decode the stream.** A scene is a version byte, then commands as
   `[opcode u8][payload_len u16][payload]`. Skip an unknown ordinary
   opcode and count it; refuse an unknown layer id. The
   [scene-layer protocol](scene-layer-protocol.md) has the table.
2. **Run the structural gate before anything is visible.**
   `validate_layers` in `indicate-instrument-scene` is the one
   authoritative gate; a port in another language re-checks framing as
   defence in depth.
3. **Interpret every opcode.** Translate and rotate compose in command
   order from an identity, unit-scale start. There is no scale opcode
   and there will not be one: map the whole frame to the viewport once.
   Clip is a rectangle. `Attribute` paints nothing.
4. **Render text from the glyph pack.** Load the manifest from
   `indicate-instrument-glyphs`, verify its hash, and paint each set
   cell as a quad. A character without a glyph is a typed failure, not
   a substitution ([glyph-pack.md](glyph-pack.md)).
5. **Replay the conformance corpus.** Pin `corpusVersion` and
   `corpusSha256` from
   `crates/indicate-instrument-scene/corpus/scene-conformance-corpus.json`
   in your own CI and compare your decode outcomes, gate verdicts, and
   method trace with each golden entry. The reference rasterizer is
   bit-exact; other backends are trace-conformant with documented
   tolerances ([renderer-verification.md](renderer-verification.md)).
6. **Keep the two disciplines.** Build native path objects once per
   identity and replay them; repaint on the display clock, never on
   data arrival.
7. **Report health.** Feed the shell's display reasons from the
   [display-reason registry](display-reason-registry.md) so a retained
   last-good image is detected
   ([`AIR-IN-013`](requirements.md#air-in-013)).
8. **Compose.** Paint slots in index order, clip to the slot rect,
   translate to the slot origin, and replay the validated scene; the
   registry's composition validation ran before first paint.

## Where the reference lives

`indicate-instrument-raster` is the reference: RGBA8, straight alpha,
pixel-centre coverage, no anti-aliasing, Q8.8 snapping at the transform
boundary. Read its crate documentation before choosing a tolerance.
