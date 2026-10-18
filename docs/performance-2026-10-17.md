# Rendering and macro performance — 2026-10-17

Measured on macOS arm64 using the release profile. These are CPU microbenchmarks,
not GPU timings or whole-app FPS measurements. Both reference and optimized paths
run sequentially in the same test executable with deterministic inputs.

## Changes

- Macro rendering skips settings copies/conversions when no enabled binding has
  an available signal sample. Active bindings apply and restore in two settings
  round-trips total, rather than one per individual field. Only bound fields are
  restored, so camera gestures and unrelated changes made during rendering survive.
- Macro knobs and inspector indicators share a settings snapshot for numeric
  lookups instead of serializing the entire component for every displayed value.
- Sphere meshes compute camera trigonometry once per frame, latitude trigonometry
  and taper once per row, and frequency interpolation indices once per column.
  Displaced and resting geometry share these calculations. Cell depths are
  computed once before sorting instead of repeatedly inside the comparator.

## Measurements

| Workload | Reference | Optimized |
| --- | ---: | ---: |
| Twelve sphere macro updates and manual-value restoration | 0.107 ms | 0.008 ms |
| Sphere mesh, 64 × 128, wrapped spectrum | 0.580 ms | 0.451 ms |
| Sphere mesh, 64 × 128, pole ripples | 0.548 ms | 0.414 ms |
| Sphere mesh, 128 × 256, wrapped spectrum | 2.730 ms | 2.227 ms |
| Sphere mesh, 128 × 256, pole ripples | 2.616 ms | 2.007 ms |

The macro test runs 5,000 apply/restore cycles; each sphere case runs 100 mesh
builds after warmup. Macro timing excludes visualization rendering and knob UI.
Sphere timing excludes egui tessellation, GPU submission, and presentation.
Results vary with machine load and do not establish an end-to-end speedup.

Reproduce with:

```sh
MACOSX_DEPLOYMENT_TARGET=14.2 cargo test --offline --release benchmark_macro_updates -- --ignored --nocapture --test-threads=1
MACOSX_DEPLOYMENT_TARGET=14.2 cargo test --offline --release benchmark_sphere_mesh -- --ignored --nocapture --test-threads=1
```

Regression checks compare all six sphere styles in both motion modes against the
previous renderer, including rolled, stretched, panned, twisted, and pole-on
views. Vertex positions agree within 0.001 screen units; colors, UVs, indices, and
vertex counts match exactly. Macro tests compare batched and individual updates,
integer/nested/invalid fields, missing and bypassed sources, and restoration
without overwriting unrelated camera edits. Existing real-input, persistence,
signal, and modulation tests also pass.

Validation: formatting, Clippy with warnings denied for normal and development
builds, 255 normal tests, 267 development tests, and four supervisor tests passed.
Ten opt-in tests are ignored by each default suite, including timing probes.

## Further candidates

Profile an active Flux session before changing its particle simulation. If GPU
submission or mesh upload dominates, GPU particle updates and instanced drawing
may offer larger gains than additional CPU tuning. Sphere mesh caching for paused
panes is another candidate, provided invalidation covers camera gestures, theme,
viewport, modulation, and frozen capture time as thoroughly as Waterfall's cache.
