# Live reload development

Run the development app and its file watcher with one command from this checkout:

```sh
./scripts/dev.sh
```

The first launch uses generated stereo audio. Later launches restore the saved
audio mode. The separate **Symphos Dev** app uses the same navigation as the
regular app, uses the development Cargo profile, and keeps the normal app's installation and runtime separate.
Rust, Python 3, and the usual platform build dependencies are required; no extra
watcher package is needed. On Mac, the supervisor creates and locally signs
`target/dev/Symphos Dev.app`. On Linux it runs `target/dev/symphos` in the current
graphical session.

## Edit and save

Rust/native source changes are debounced, compiled, and packaged while the
current app stays open. A successful build closes the supervised dev process,
replaces its executable, and relaunches it. A failed compile or packaging step
keeps the last working app open and reports the failure in the watcher terminal and `target/dev/build.log`.
The supervisor controls only the child it launched. Ctrl+C stops that development
session. One supervisor can run per checkout.

Code edits use rebuild and relaunch. Palette and UI-scale edits apply to the
running app without a restart. Rust function hot-patching is not implemented.
Incremental build times depend on the changes and the machine; small source
changes were observed in roughly 2–8 seconds on the development Mac after warming
the build cache. Initial builds and dependency changes take longer.

The saved development workspace includes all component settings in each
pane, pane assignments, selected pane, inspector visibility and width, divider
positions, shared analysis and meter controls, audio mode, and window size and
position. It deliberately excludes samples, histories, frozen frames, textures,
and meter peaks. Panel fullscreen and disclosure/scroll state remain transient.
State is checkpointed during use and on graceful exit. An
invalid/incompatible workspace starts with defaults and retains the rejected
file for inspection. The regular app also restores its workspace, using its
platform configuration directory. Dev state and presets remain isolated under
`target/dev`; see [workspace layouts and presets](workspace-layouts.md).

Useful command options:

```sh
./scripts/dev.sh --demo-audio    # Force generated audio on each relaunch
./scripts/dev.sh --system-audio  # Force real capture on each relaunch
./scripts/dev.sh --once          # Build and run without watching
./scripts/dev.sh --build-only    # Package without launching
./scripts/dev.sh --check         # Check normal and development builds
```

Generated audio does not play sound or open an audio device. It drives the real
analysis pipeline with pulsing bass and independent stereo tones. Run
`./scripts/dev.sh --system-audio` for real capture, or `--demo-audio` for generated
audio. Rebuilding an ad-hoc signed Mac app may require
renewed system-audio permission; generated audio avoids that dependency while
working on visuals. The normal audio backend remains the path for real device
and performance validation.

## Live palette and scale

Edit `target/dev/theme.json` while the dev app runs. For example:

```json
{
  "name": "My development palette",
  "accent": "#f59e0b",
  "selection": "#a86210",
  "ui_scale": 1.1
}
```

Omitted fields use the built-in dev defaults. Colors use `#RRGGBB`; `ui_scale`
accepts 0.6–2.0. The app checks this file every 200 ms. Invalid JSON, unknown
keys, or invalid values keep the last working theme and record an
entry in the regular issue log. See `assets/development/theme.json` for the
complete palette.

These overrides apply to the dev app. To retain an approved starting palette
in Git, copy the values into the tracked development template. Changes intended
for the normal app should be implemented in its theme system separately.

## Development files and diagnostics

| File under `target/dev` | Purpose |
| --- | --- |
| `theme.json` | Live palette and UI scale |
| `workspace.json` | Versioned settings-only checkpoint, including tiled layout |
| `presets.json` | User layout, full-view, and component preset library |
| `workspace.rejected.json` | Most recent invalid checkpoint retained for recovery |
| `status.json` | Latest supervisor build status |
| `control.json` | Supervisor's graceful-close request |
| `build.log` | Latest compilation output |
| `app.log` | Development app stdout/stderr |

These generated files are ignored by Git. `cargo clean` removes the target
folder, including this development state; copy useful theme values
elsewhere first. The ordinary issue log remains available inside the app.

## Verification

Mac normal and development builds pass formatting, Clippy, and tests. The
normal suite passes 154 tests and the development suite passes 161 tests; both
retain five ignored manual timing probes (including Veroni, Strata, and Flux). Four Python supervisor tests
cover source watching, successful publishing, atomic control writes, and failed
builds retaining the current app.

A live watcher check intentionally introduced a compilation error, verified the
running build was retained, restored the source, and verified that the successful
relaunch restored pane/global settings. A live palette check changed the accent
and selection colors to amber while the build status/generation stayed unchanged.
The generated signal was exercised in the running app.

The Veroni timing probe can be run with:

```sh
cargo test --features development modules::veroni::tests::render_timing -- --ignored --nocapture
```

On the development Mac it measured approximately 0.49 ms/frame at 128 cells and
1.13 ms/frame at 240 cells for UI drawing and tessellation at 1600×900 points.
These are local CPU measurements, not end-to-end GPU frame times or Linux results.

Strata's corresponding probe is
`cargo test --features development modules::strata::tests::render_timing -- --ignored --nocapture`.
After adding per-layer frequency mapping, it measured 0.94 ms/frame at default settings and 5.14 ms/frame
at maximum layers, grid detail, dust, and grain counts. It includes geometry,
depth sorting, UI drawing, and tessellation; GPU presentation is excluded.

Flux's corresponding probe is
`cargo test --features development modules::flux::tests::render_timing -- --ignored --nocapture`.
On the same Mac, it measured 2.12 ms/frame at the default 6,000 particles and
9.34 ms/frame at 20,000 particles. It includes fixed-step simulation, depth
sorting, UI drawing, and tessellation; GPU presentation is excluded.

CI includes the development feature and supervisor tests on Mac and Linux.
A real Omarchy session is still required for desktop-device and graphics
regression evidence; container checks cannot provide that hardware coverage.
