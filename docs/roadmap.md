# Roadmap

Symphos is evolving from an analyzer into a visual instrument: users will build
scenes from visual elements, connect audio signals to their properties, tune
the response, and perform or share the result.

Live progress is tracked in the
[GitHub roadmap issue](https://github.com/pat-riley/symphos/issues/19).

Symphos is a standalone application first. The
[desktop and plugin plan](cross-platform-plan.md) retains future VST3/DAW
integration research, but plugin implementation and framework prototypes are
deferred while standalone audio responsiveness and usability improve.
The [landing page brief](landing-page-brief.md) covers the separate public site.

## Current priorities — 25 October 2026

Projects/scenes, scene and component presets, Undo/Redo, macros, signal insert
chains, editable color libraries, a local macOS build, and Linux/macOS CI already
exist. Their presence does not establish release readiness or completion of the
full layered scene-composition model described below. The longer-term areas
should be read against this inventory rather than as a list of unstarted work.

### 1. Audio responsiveness and understandable Signals

- Improve transient, envelope, level and beat response before adding more
  visualizers or starting plugin development.
- Share capture and reusable fast/detailed analysis between consumers. Keep
  FFT resolution and device choices in application settings; do not require
  users to tune an FFT to create a responsive signal.
- Give each signal its own relevant frequency range, sensitivity, threshold,
  normalization and attack/release controls. Expose controls appropriate to
  that signal's purpose instead of showing every processor option at once.
- Use the chosen guided workflow: select a signal type, adjust its response,
  and preview the result. Put insert chains under Advanced. Explain what is
  detected, how the response is shaped, and which visual parameters it controls.
- Make input, detected value, final response and connected visual destination
  easy to compare live. Existing insert input/output monitors should make
  changes perceptible without requiring users to understand the chain first.
- Keep advanced processing available, preserve existing chains, stable signal
  identities and mappings, and avoid silently rewriting saved custom chains.
- Separate a signal's detection/response from the amount and range applied at
  each visual mapping. One shared signal can drive multiple destinations with
  different strengths.
- Clearly explain that processing affects visualization inputs/control values;
  it does not change the sound played through the speakers.
- Establish deterministic test audio for impulses, bass hits, sustained tones,
  varied-level music and silence. Measure response timing, false triggers,
  silence recovery and consistency across render rates. Distinguish FFT-window
  duration from measured capture-to-visible-response latency.

Exit condition: create a bass-driven visual response through understandable
signal controls, see the result of each adjustment, and retain useful behavior
across quiet/loud recordings without reconstructing an insert chain manually.
Existing projects and custom insert chains retain their behavior.

### 2. Performance and reliability alongside audio work

- Track capture drops, analysis cost, slow frames and sustained memory growth
  with representative scenes and repeatable workloads.
- Exercise device/source changes, silence, sleep/wake, recording and scene
  switching in sustained sessions; preserve the independent capture lane.
- Validate realistic 60/120 FPS workloads and report tail latency and overruns
  alongside average FPS. Publish measured limits before promising budgets.

### 3. Export workspace after the audio-response milestone

- Explore an **Export** dock tab beside **Signals** and **Macros**.
- Put canvas/output size, aspect ratio and export controls in that workspace;
  keep output resolution independent of the editor window size.
- Begin from the existing live single-component recording implementation,
  then define full-scene/workspace framing and export behavior explicitly.
- Decide live recording versus offline audio-file rendering, audio routing,
  frame rate, format, progress and cancellation behavior before implementation.

Export is a follow-up design direction, not part of the initial responsiveness
work. Existing recording remains available while the workspace is designed.

### 4. VST3 and other DAW integration — deferred

Keep VST3 on the roadmap as a future way to use Symphos inside a DAW. Do not
start a plugin shell, framework migration or embedded-editor prototype in the
current milestone. Revisit the embedded-plugin versus standalone-sender choice
after the standalone workflow and audio response are reliable. Preserve the
existing independent audio/analysis/render boundaries in the meantime.

The complete [analyzer feature backlog](analyzer-feature-backlog.md) records
the current parity inventory, shipped waveform work, proposed metering modules,
capture workflows, workspace features, and platform integrations. The
longer-term areas below describe the broader direction; the current priorities
above determine near-term delivery order:

- **Analyzer foundation:** independent component sources, Mid/Side routing,
  reference spectra, measurement cursors, Mel/piano views, and sharper
  spectrogram modes.
- **Analysis and metering:** shipped stereo-image/Lissajous and phase-correlation
  views and a pitch-following scope, followed by calibrated VU and
  standards-based loudness.
- **Projects and workspaces:** persistent module presets, user color maps,
  detached components, saved layouts, transparency, and desktop-bar behavior.
- **Capture and distribution:** audio recording, retrospective clips, DAW
  transport/audio bridges, plug-in evaluation, cross-platform builds, and
  platform video-output integrations such as Syphon.

## 1. Validate the analyzer foundation

- Exercise microphone and system-output capture across device changes.
- Allow each analyzer or visual component window to select its own audio input
  instead of relying on one application-wide source.
- Investigate application and virtual audio sources, including per-application
  streams, loopback devices, virtual cables, and DAW routing. Defer plugin
  protocols until the standalone routing requirements are understood.
- Add sustained capture, silence, source-switch, and format-change tests.
- Measure capture-to-display latency, dropped samples, CPU use, GPU frame time,
  and memory growth.
- Improve onset and tempo accuracy with confidence and insufficient-data states.
- Automatically detect the musical key of a song from accumulated pitch-class
  evidence, with major/minor estimates, confidence, and an insufficient-audio state.
- Define named low, mid, and high bands alongside arbitrary frequency bands.
- Extend independent Left, Right, Mid, and Side routing from waveform panes to
  the other analyzer and metering components.
- Potential improvement: capture a reference spectrum as a faint overlay while
  live analysis continues, for comparing sounds or EQ changes. Evaluate range
  alignment and clearly distinguish the reference from peak hold.
- Potential improvement: an optional measurement cursor with pinned frequency/
  note, level, and time readouts, especially for frozen spectrogram inspection.
  Keep it distinct from Info View and avoid obscuring the visualization.

Exit condition: analysis signals remain stable during a 30-minute session and
the app reports enough timing data to diagnose a missed frame or audio sample;
multiple components can use different sources without blocking one another.

## 2. Build the modulation engine

A renderer-independent signal system, typed insert chains and signal mappings
are implemented. Improve their responsiveness and interaction model as described
in the current priorities, then expand the available musical information.

Signal sources:

- Peak, RMS, stereo balance, and waveform samples
- Named or custom frequency-band energy
- Dominant frequency, pitch class, centroid, rolloff, and flatness
- Onset events, beat phase, tempo, and silence state
- Time, scene age, and user-controlled values

Every continuous signal can pass through normalization, threshold, input and
output ranges, inversion, curve shaping, quantization, attack, release, and
hold. Event signals can trigger pulses, toggles, counters, and envelopes.

Exit condition: a custom bass-band signal can drive a test value smoothly and
predictably across quiet and loud material.

## 3. Establish the scene model

- Add a GPU canvas with resolution and aspect-ratio controls.
- Represent a scene as ordered layers with stable IDs.
- Start with circles, rectangles, lines, gradients, waveforms, spectrum bars,
  and a particle emitter, then expand into additional modes such as spheres and
  other 2D and 3D primitives.
- Give elements transform, color, opacity, blend, and element-specific fields.
- Save human-readable, versioned project files with a Symphos-specific file
  extension and load them safely. A project should contain its scene,
  component layout, routing, mappings, and references to required assets.
- Extend the shipped individual-component presets into the enclosing scene
  project workflow. Analyzer presets already save independently of layouts.
- Shipped: preset saves suggest a fresh, editable three-word name and add a new
  active library entry; subsequent creative edits display an `(edited)` marker.
- Save reusable user-authored color maps and reference-spectrum definitions.
- Add undo/redo and autosave before the editor becomes complex.

Exit condition: users can create a circle, edit it, save the project, restart
the app, and recover the same scene, window configuration, routing, and
mappings; an individual component preset can be reused in another project.

## 4. Add audio-to-visual mappings

The first complete authoring workflow is:

1. Add a circle.
2. Create or select a 30–150 Hz bass signal.
3. Map that signal to the circle radius.
4. Set the input range, output range, curve, attack, and release.
5. Preview the incoming signal and mapped value live.
6. Save, reload, and play the scene fullscreen.

Mappings should be reusable and allow multiple sources to combine through add,
multiply, minimum, maximum, or blend operations. Visual properties retain a
base value so modulation can be relative or absolute.

Exit condition: the circle-and-bass workflow works without editing a file or
writing code.

## 5. Shape the application workspaces

- **Analyze:** inspect audio and create named signals.
- **Create:** canvas, layers, element properties, and mappings.
- **Perform:** fullscreen output, scene switching, and a compact set of live
  controls.

Add scene templates and a scene preset library after scene files and mappings
have stable versioning. Analyzer workspace and component presets are available
in the current app.

- Extend the shipped tiled workspace, resizing, and pane expansion into
  multi-display workflows when required. Detached/floating panes are outside the
  current workspace design.
- Evaluate transparent component backgrounds and a Wayland desktop-bar mode
  after detached windows have stable sizing and input behavior.
- Extend the shipped named tiled layouts and last-session restoration with
  display-aware placement when multi-display workflows arrive.
- Rework the UI around the Analyze, Create, and Perform workflows once their
  core tasks and window model are validated.

Exit condition: users can arrange a workspace for editing or performance, save
it, and restore a usable layout even when the available displays have changed.

## 6. Effects and shader scenes

- Add trails, blur, bloom, feedback, distortion, color grading, and compositing.
- Expose a stable GPU input block containing time, resolution, audio signals,
  beat state, and selected FFT data.
- Prototype WGSL scene shaders and native Rust elements against the same scene
  contract.
- Add a composition mode where users combine built-in primitives, 3D geometry,
  shaders, effects, and audio mappings into a single reusable visual scene.
- Define camera, lighting, material, depth, and transform behavior before
  expanding the 3D primitive library.
- Report shader compilation errors in the editor and preserve the last valid
  render.
- Decide on a plugin ABI only after several built-in scenes expose its actual
  requirements.

Exit condition: built-in primitives and 3D geometry can be composed with a
custom shader, mapped to audio signals, saved in a project, and recovered
gracefully when shader compilation fails.

## 7. Performance, sharing, and distribution

- Adopt the staged review, testing, and compatibility gates described in the
  [development process](development-process.md) as contributors and releases
  make each stage worthwhile.
- Add performance presets for Apple Silicon/Asahi and conventional GPUs.
- Define frame-time, capture latency, memory, and CPU/GPU budgets for each
  supported workload and track them with repeatable benchmarks.
- Support higher and uncapped render rates where hardware and displays allow,
  without tying audio analysis correctness to render frequency.
- Add adaptive quality controls for expensive effects and scenes so missed
  frame budgets degrade visuals predictably instead of disrupting audio.
- Create deterministic demo audio and visual regression captures.
- Package scenes with their assets and import untrusted packages safely.
- Add scene thumbnails, metadata, and exportable bundles.
- Add bounded source recording and retrospective 10/60-second audio clips
  without moving file I/O onto the realtime capture path.
- Deferred: evaluate DAW sender/transport bridges and VST3, CLAP or Audio Unit
  integration after the standalone audio-response and usability milestones.
- Produce reproducible Arch Linux packages and signed GitHub releases.
- Separate audio capture and theme integration behind platform interfaces, then
  add macOS Core Audio and Windows WASAPI backends, including supported forms
  of system-output capture.
- Add macOS and Windows CI, packaging, signing/notarization, permissions, and
  downloadable releases.
- Evaluate platform-specific video output, including Syphon on macOS, after the
  renderer and window abstractions are portable.
- Document scene compatibility and migrations before declaring 1.0.

Exit condition: representative scenes meet their published performance budgets
at common refresh rates, and users can install a tested release on Linux,
macOS, or Windows.

## 8. Product identity and public presence

- Design a distinctive logo and derive the application icon, wordmark, and
  social/repository artwork from it.
- Launch a public website with downloads, feature examples, compatibility
  information, and links to the project and documentation.
- Build user documentation for installation, audio routing, analysis,
  authoring, performance, project files, presets, and troubleshooting.
- Build contributor documentation for architecture, platform backends, the
  scene format, shaders, testing, profiling, and release processes.

Exit condition: a new user can discover Symphos, install the correct build,
route an audio source, create or open a project, and troubleshoot common setup
problems without repository-specific knowledge.
