# Workflow and output performance — 2026-10-21

Measured on an Apple M4 Pro, 24 GB, macOS, Metal, using the optimized development
test profile (`opt-level=1`, dependencies `opt-level=2`). These measurements are
not a promise that every scene or quality setting sustains 120 FPS. The running
development app and normal desktop workload were present during testing.

## CPU geometry changes

Resonance now computes radial/trigonometric samples once per mesh and stores
primitive indices instead of repeatedly copying vertices while sorting. Surface
triangles share vertices. Geometry/color regression comparisons retain the old
renderer as a test-only reference.

| Resonance workload | Previous CPU mesh time | Current CPU mesh time |
| --- | ---: | ---: |
| Membranes, 9 layers, detail 28 | 2.259 ms | 0.853 ms |
| Membranes, 16 layers, detail 48 | 5.507 ms | 1.928 ms |
| Wireframe, 16 layers, detail 48 | 1.837 ms | 0.844 ms |
| Contours, 16 layers, detail 48 | 1.442 ms | 0.620 ms |
| Points, 16 layers, detail 48 | 3.685 ms | 2.168 ms |

Paused Sphere and Cylinder meshes are reused while settings, theme, dimensions,
history, and virtual time are unchanged. One hundred requests for a dense
128 × 256 mesh took 254.1 ms rebuilding, versus 2.6 microseconds total retrieving
the cached `Arc<Mesh>` after one initial build. This excludes tessellation and
GPU drawing; it is not a whole-frame speedup of that magnitude.

## 1080p output paths

The hardware test renders all 16 default components into the recording texture
at 1920 × 1080 with a 120 FPS target. Thirty warmup frames precede 60 measured
frames. Each measurement includes CPU preparation, tessellation, GPU submission,
waiting for completion, and BGRA readback. The encoder and window presentation
are excluded. Spectrum history is prepopulated with two seconds of audio.

| Component | Median | 95th percentile |
| --- | ---: | ---: |
| 3D Waterfall | 2.484 ms | 10.464 ms |
| Frequency spectrum | 1.115 ms | 2.599 ms |
| Waveform | 1.625 ms | 3.145 ms |
| Spectrogram | 1.273 ms | 2.727 ms |
| Stereometer | 1.257 ms | 2.673 ms |
| Oscilloscope | 1.839 ms | 3.108 ms |
| Veroni | 2.689 ms | 3.809 ms |
| Strata | 2.854 ms | 4.074 ms |
| Flux | 4.699 ms | 6.033 ms |
| 3D Sphere | 2.194 ms | 3.588 ms |
| 3D Cylinder | 2.106 ms | 3.380 ms |
| 3D Rings | 1.606 ms | 3.253 ms |
| 3D Resonance | 2.939 ms | 4.330 ms |
| Orbital | 4.228 ms | 5.438 ms |
| MilkDrop presentation | 2.912 ms | 4.521 ms |
| Cymatics | 2.224 ms | 3.834 ms |

Waterfall had a 35.447 ms worst sample in this run, despite its low median. That
outlier and its p95 exceed the 8.33 ms budget and must not be hidden by quoting
only average FPS. MilkDrop renders asynchronously on its native worker; this row
measures presenting its latest image, not the rate of distinct preset frames.

## Cadence and diagnostics

The global display preference offers up to 120 FPS. An absolute deadline pacer
compensates event-loop wakeup latency instead of adding it to every frame.
Input-triggered early frames do not reset the cadence; missed deadlines are
skipped rather than causing a burst of catch-up frames. In the current live
Waterfall workspace, the displayed rate moved from roughly 110–116 FPS to
119–120 FPS after this change. This is a short observation, not an isolated
long-duration benchmark.

The independent signal worker also subtracts processing time from its 120 Hz
budget. MilkDrop's `fps` preset variable now follows the active display/recording
target; a native equation test verifies switching 60 to 120 to 30 in one engine.

**Help > Performance** reports a bounded 600-frame window of intervals, median,
p95, worst interval, budget overruns, UI CPU cost, and current per-component CPU
preparation. It adds no persistent workspace panel and never lowers fidelity.
GPU execution is explicitly excluded from those UI CPU counters.

## Recording validation

- Native H.264 + AAC stereo encoding and odd-width BGRA row alignment passed.
- All 16 visualizers rendered to an exact 322 × 182 texture without GPU validation
  errors; the nonstandard width exercises readback padding.
- End-to-end GPU/readback/encoder tests captured 60 of 60 frames at 1080 × 1920
  and 120 of 120 frames at 1280 × 720, with zero queue losses. `ffprobe` verified
  the dimensions, frame counts, one-second duration, and 48 kHz stereo AAC.
- A manual File-menu recording captured approximately 30 seconds of the running
  Waterfall with incoming audio at 1920 × 1080. Capture reported zero missed
  frames/audio samples. Decoded video was inspected and audio was non-silent.
  This exposed live timestamp jitter, subsequently corrected by stamping video
  on the requested frame grid. A GPU regression test verifies the live scheduler
  at 60 and 120 FPS with late wakeups and skipped frames, including the hidden-pane
  fallback's protection against duplicate frames.
- The Cymatics GPU test passed modal response, audio reaction, edge recycling,
  and paused-state checks after adding configurable grain lifecycle parameters.

Reproduce hardware probes individually, outside ordinary CI:

```sh
MACOSX_DEPLOYMENT_TARGET=14.2 cargo test --offline --locked --features development benchmark_resonance_mesh -- --ignored --nocapture
MACOSX_DEPLOYMENT_TARGET=14.2 cargo test --offline --locked --features development benchmark_paused_spectral_surface -- --ignored --nocapture
MACOSX_DEPLOYMENT_TARGET=14.2 cargo test --offline --locked --features development benchmark_all_output_paths_at_120_fps -- --ignored --nocapture
MACOSX_DEPLOYMENT_TARGET=14.2 cargo test --offline --locked --features development output_to_movie_preserves_audio_and_60_or_120_fps_timestamps -- --ignored --nocapture
MACOSX_DEPLOYMENT_TARGET=14.2 cargo test --offline --locked --features development native_preset_fps_tracks_output_target -- --ignored --nocapture
```
