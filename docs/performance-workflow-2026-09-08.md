# Parameter workflow and performance verification — 2026-09-08

## Environment

- Linux aarch64, Apple M1 Pro (G13S C0).
- Vulkan Honeykrisp, Mesa 26.1.8; 120 Hz display (the app's default target Rate is 60).
- System output: MacBook Pro J314 Speakers (`audio_effect.j314-convolver`),
  48,000 Hz, stereo, F32LE. Both native builds reached Live.

## Repeatable CPU microbenchmarks

Run independently of other builds/workloads:

```sh
cargo test --release benchmark_ -- --ignored --nocapture --test-threads=1
```

These compare the original algorithm or identical uncached builder with the
optimized path, using deterministic synthetic data. They are not GPU timings
or claims about whole-app FPS. They are opt-in measurements, not CI timing gates.

| Workload | Before | After |
| --- | ---: | ---: |
| 200 diagnostic summary builds, FFT 16,384 / 96 bands | 106.064 ms, full scan per band | 1.540 ms, ordered pass |
| 100 unchanged waterfall mesh requests, 72 slices / 96 bands | 46.946 ms, rebuild | 0.208 ms, warm cache |

Band outputs are also compared exactly against the original implementation
across multiple sample rates, FFT sizes, and frequency ranges. Cache regressions
check reuse and invalidation for data, camera, style, palette, viewport position,
and expired history. Interactive guides remain outside the expensive mesh cache.

## Native smoke comparison

Sequential eight-second default-dashboard runs, measured with Bash `time` and
stopped with `timeout --signal=TERM` (expected exit 124):

| Build | Wall | User CPU | System CPU |
| --- | ---: | ---: | ---: |
| Previous installed module build (`f94d8f4`) | 8.016 s | 3.567 s | 4.646 s |
| Updated workflow/capture/cache/analysis build | 8.005 s | 3.463 s | 4.601 s |

Single short runs include startup and uncontrolled desktop/audio conditions;
the updated run also enabled startup adapter diagnostics. This is startup/live
capture evidence, not a statistically meaningful end-to-end performance claim.

## Capture and input safety

- The analysis-side frequency archive retains at most 30 seconds / 901 samples,
  independently of UI painting. Hidden panes consume it, and returning windows
  catch up. Only explicit per-pane Pause intentionally skips history.
- Archive and module histories share immutable magnitude arrays. At maximum FFT
  size, one 901-row magnitude set is about 28 MiB. Paused panes can retain older
  sets; each history remains bounded. No allocation or locking was added to the
  real-time PipeWire callback.
- Source/analysis resets advance a capture epoch, reject old packets, and reset
  analysis buffers. Tests exercise a real analysis worker without a renderer,
  archive expiry, hidden-pane catch-up, and manual-pause isolation.
- Small FFTs no longer force transforms above the requested Rate. Hop length is
  sample rate divided by target Rate, regardless of FFT size. Every sample still
  enters the waveform capture ring; FFT windows may overlap or have gaps.
- BPM receives every onset sample, but runs autocorrelation roughly twice per
  second. It remains an experimental diagnostic, not a reliable beat clock.
- Typed values require finite numbers, safe bounds, and integers where required.
  Invalid values preserve the prior setting and appear in the issue log. Custom
  numeric changes are logged; reports preview active settings/capture metadata.
- Shortcuts ignore active/pending text entry, menus/modals, and held-key repeats.
  Copy/Paste contains settings only, with explicit module-type checks.

No preset files, saved workspaces, or startup restoration were introduced.
