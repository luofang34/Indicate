# Cookbook: see pixels

Two shells in this repository put a frame on disk with no simulator
and no display hardware.

## Every shipped panel, every canonical state

```sh
cargo run --locked -p instrument-bench -- --out target/frames
```

The bench composes the shipped registry, reproduces the pinned scene
digest and screen-composition digest, runs the admission harness, and
rasterizes every panel at every canonical state and frame through the
reference rasterizer. It writes one PPM file per case under the
directory you name. A nonzero exit is a conformance failure.

## The head-worn set in a browser

```sh
cargo run --release -p hwd-bench -- target/hwd-gallery.html
```

Open the file. Pick a case, move the head sliders, or start the sweep.

## A live loop with no simulator attached

```sh
cargo run -p indicate-source-xplane -- --frames 100 --out target/xp
```

With no X-Plane listening, the adapter still subscribes, ticks its
paint clock 100 times, and writes every twentieth PFD frame. Every
group is `Missing`, so the frames show dashes and flags: the display
never invents a value. Point `--xplane` at a running simulator to see
data.

## Read a PPM

Any image viewer opens a PPM. On macOS, Preview does; on Linux,
`display` or a browser extension does. The frames are 480 by 360 for
the shipped panels.
