# MilkDrop in Symphos

Component browser (top-left plus button) → MilkDrop creates an independent projectM-backed component. Its preset
library, current selection, favorites, playback options, and parameter adjustments
belong to that component in the scene. No signal channels are created automatically.

## Engine choice

The original MilkDrop 2.25c source targets Windows, Winamp and DirectX 9. A direct
port would require replacing its platform layer, graphics backend, and HLSL
compilation. projectM already implements MilkDrop equations, custom waves/shapes,
feedback, warp/composite shaders, noise/blur textures, audio response, and blending.
It is therefore used as a separate native engine, not translated into Orbital's
particle simulation or rewritten as a subset of MilkDrop.

The original code was inspected alongside projectM before choosing this path:
https://sourceforge.net/projects/milkdrop2/files/
https://github.com/projectM-visualizer/projectm
https://www.geisswerks.com/milkdrop/milkdrop_preset_authoring.html

The pinned revision and narrow local extensions are documented in
`vendor/projectm/SYMPHOS.md`. This is compatible playback with known limits, not a
claim of pixel-identical behavior for every historical DirectX preset.

## Inspector and performance

- **Presets:** import a file or folder, search, select, favorite, previous/next,
  shuffle, auto play/lock, duration and transition time, restart, duplicate/remove,
  export, and texture import.
- **Flow & Motion:** common zoom, rotation and warp adjustments, with detailed
  pan, center and stretch controls underneath.
- **Appearance:** feedback, brightness, waveform opacity/color; detailed wave
  position/shape, video echo, borders and motion vectors.
- **Signal Response:** audio gain (zero disables native audio input) and beat
  sensitivity (a multiplier on normalized bands, independent of waveform gain).
  MilkDrop receives stereo PCM resampled from Symphos's full-rate
  capture history to a 44.1 kHz reference window. Disconnected input is silence.
- **Rendering:** animation speed, resolution cap, warp mesh detail and measured
  native render time. The existing global display target remains authoritative.

Continuous controls can be dragged onto scene macros and mapped to Signals,
including audio detectors, LFO and MIDI. Visual adjustments default to zero and
are applied after preset equations, before rendering; they do not recompile a
shader or reset feedback on every modulation frame. Custom shaders can ignore
built-in variables, so not every control affects every preset.

The advanced saved-value editor edits numeric literals, including custom shape
and wave parameters. Apply restarts the selected preset. Equation and shader
lines are preserved; they can animate or override saved starting values. A source
code editor is deferred: export/edit externally/import for equation or shader edits.

Export bakes the current visual adjustments into a final per-frame equation and
writes the preset's textures beside it. Audio gain, animation speed, resolution,
and playlist options are host settings and remain in the Symphos project.

## Portable data

Preset source and texture data are embedded in project/scene state; an external
file path is not needed to reopen a project. Libraries are separated from numeric
module settings so macro updates never serialize the image assets. Component
copy/paste and component presets explicitly include the selected preset document.

Import supports up to 2048 presets per component, 2 MB per preset source and 8 MB
of textures per preset. Import stops at 24 MB of embedded source/assets per
component; the existing 64 MB project limit still applies across scenes. Texture
assets are kept as validated filenames and hex data. Export refuses to overwrite
an existing texture with different data. Existing original preset source files are
not changed by editing within Symphos. No downloaded preset pack is bundled.

Missing textures must be supplied via Import texture folder; projectM's treatment
of missing texture samplers may differ from the original engine. Shader failures
are surfaced in the inspector instead of silently substituting a fallback shader.

## Rendering and lifetime

Each component has a worker thread with a private OpenGL context (CGL on macOS,
EGL on Linux). The application still renders its interface through wgpu/Metal.
The worker renders into an offscreen framebuffer and reads bounded RGBA output;
the UI uploads the most recent completed image. This is a compatibility-first
CPU transfer, not zero-copy OpenGL/Metal interop. Default output is capped at
960 pixels along the longer edge; 640/1280/1920 are available.

Queues are bounded, completed output replaces older output, and preset compilation
never runs in the UI or audio callback. Removing/switching away from a component
closes its worker queue; GL resources are destroyed on their owner thread. Runtime
buffers, feedback and clocks are not saved. Pause holds the completed image.

## Build and validation

Install CMake 3.21+ and a C++17 compiler. On macOS use `brew install cmake` plus
Xcode command-line tools. Linux also needs EGL headers/libraries (`libegl1-mesa-dev`
on Ubuntu). `CMAKE` can override the executable. Homebrew's usual install paths
are detected. The pinned source builds offline through Cargo; no global projectM
installation is needed.

```sh
cargo fmt --all -- --check
MACOSX_DEPLOYMENT_TARGET=14.2 cargo clippy --workspace --all-targets --locked -- -D warnings
MACOSX_DEPLOYMENT_TARGET=14.2 cargo test --workspace --all-targets --locked
MACOSX_DEPLOYMENT_TARGET=14.2 cargo build --release --locked
```

A real GPU test renders bundled presets, exercises controls and optionally runs
an external preset corpus. Its timings cover offscreen native rendering/readback,
not full-app FPS or audio capture latency:

```sh
cargo test milkdrop::runtime::tests::native_presets_render_and_live_controls_change_pixels -- --ignored --nocapture
# Optional environment variables:
# SYMPHOS_MILKDROP_TEST_PRESETS=/path/to/preset/folder
# SYMPHOS_MILKDROP_TEST_TEXTURES=/path/to/texture/folder
# SYMPHOS_MILKDROP_PREVIEW=/tmp/milkdrop.ppm
```

Non-GPU tests cover malformed input, asset traversal rejection, import/export,
project migration, portable texture persistence, clipboard transfer, macro edits,
playlist behavior and inspector layout.

### Validation on Apple M4 Pro (2026-10-21)

The external `presets-milkdrop-original` corpus (552 presets), with its separate
texture pack, and the three bundled presets loaded and rendered at least 30 frames
each at 640 × 360 with no equation parser, shader compiler, or OpenGL errors.
Presets initially dark were additionally exercised with longer startup, incoming
feedback and pulsed audio. All 555 produced visible output across these checks.
Feedback-only presets can need an incoming image; volume-gated presets can remain
dark under steady audio. These are reasons to try a transition or varied music
before assuming the engine failed. This
exercise establishes execution compatibility, not pixel matching against Windows
MilkDrop, exhaustive preset behavior, or freedom from every historical shader bug.
The downloaded corpus/texture pack are test inputs only and are not redistributed.

The native lifecycle test runs two independent GL workers, live opacity and beat-sensitivity changes,
negative-zoom feedback, failed-shader recovery, transitions and landscape/portrait
resizes. Native frame
render/readback times measured roughly 1–4 ms at the tested resolutions; this is
not a guaranteed full-app frame rate. The actual app was also exercised with demo
audio, built-in presets, native import dialogs and an imported original preset.
Linux build configuration is included in CI; local GPU validation was on macOS.
