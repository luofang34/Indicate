# Assurance profiles

The repository carries two kinds of code with two kinds of obligation.
This page names them so a contributor knows which battery a change
must pass, and a reviewer knows which one to demand. It changes no
gate; it states which gates apply to what.

## Kernel profile

Applies to `crates/` and to the shipped flight set in
`sets/indicate-instrument-panels`.

- The full local battery in `CONTRIBUTING.md`, including the evidence
  gate, the recorded-count freshness check, and the release-marker and
  manifest guards.
- A change to a recorded test source re-records its evidence, and a
  branch that moves a configuration digest merges with a merge commit.
- A change that moves one of the five contract values or adds a public
  API consumers call is cut as a release.
- Every new behaviour cites a requirement, and the identifier registry
  defines every work package the code names.

## Community set profile

Applies to a set crate under `sets/` other than the shipped flight set,
and to shells under `tools/`.

- `cargo fmt`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test`, `cargo doc` with missing-docs denied, and the structure
  check.
- Admission in the crate's own test, with the out-of-frame ink count
  pinned.
- A set document with the generated descriptor block and at least one
  cited requirement.
- The bare-metal closure for a `no_std` set.
- No evidence-graph binding and no recorded-count record, unless the
  set is promoted to the kernel profile by adding it to the shipped
  registry.

## Why two

The kernel profile is shaped like the lifecycle evidence a formal
assurance program keeps, and it costs what such evidence costs. A set that a
simulator user writes for one aircraft does not need that evidence to
be useful and would not be written if it did. The community profile
keeps the properties that make a set safe to compose, which admission
proves, and leaves the properties that make a set part of a safety
baseline to the moment it joins one.

## Promotion

A community set joins the kernel profile when it is added to the
shipped registry. That change moves the composition digest and the
panel set, so it is cut as a release, and from then on the full
battery applies.
