# Roadmap

Symphos is evolving from an analyzer into a visual instrument: users will build
scenes from visual elements, connect audio signals to their properties, tune
the response, and perform or share the result.

Live progress is tracked in the
[GitHub roadmap issue](https://github.com/pat-riley/symphos/issues/19).

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

Exit condition: analysis signals remain stable during a 30-minute session and
the app reports enough timing data to diagnose a missed frame or audio sample;
multiple components can use different sources without blocking one another.

## 2. Build the modulation engine

Create a renderer-independent signal system that converts raw analysis data
into values useful for visuals.

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
- Save and load presets for individual component windows independently of the
  enclosing project.
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

Add templates and a small preset library only after scene files and mappings
have stable versioning.

- Support multiple component windows with intentional docking, tiling,
  detaching, resizing, fullscreen, and multi-display behavior.
- Save named workspace layouts and restore them reliably across display and
  resolution changes.
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
- Produce reproducible Arch Linux packages and signed GitHub releases.
- Separate audio capture and theme integration behind platform interfaces, then
  add macOS Core Audio and Windows WASAPI backends, including supported forms
  of system-output capture.
- Add macOS and Windows CI, packaging, signing/notarization, permissions, and
  downloadable releases.
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
