# Roadmap

Symphos is evolving from an analyzer into a visual instrument: users will build
scenes from visual elements, connect audio signals to their properties, tune
the response, and perform or share the result.

## 1. Validate the analyzer foundation

- Exercise microphone and system-output capture across device changes.
- Add sustained capture, silence, source-switch, and format-change tests.
- Measure capture-to-display latency, dropped samples, CPU use, GPU frame time,
  and memory growth.
- Improve onset and tempo accuracy with confidence and insufficient-data states.
- Define named low, mid, and high bands alongside arbitrary frequency bands.

Exit condition: analysis signals remain stable during a 30-minute session and
the app reports enough timing data to diagnose a missed frame or audio sample.

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
  and a particle emitter.
- Give elements transform, color, opacity, blend, and element-specific fields.
- Save human-readable, versioned scene files and load them safely.
- Add undo/redo and autosave before the editor becomes complex.

Exit condition: users can create a circle, edit it, save the scene, restart the
app, and recover the same result.

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

## 6. Effects and shader scenes

- Add trails, blur, bloom, feedback, distortion, color grading, and compositing.
- Expose a stable GPU input block containing time, resolution, audio signals,
  beat state, and selected FFT data.
- Prototype WGSL scene shaders and native Rust elements against the same scene
  contract.
- Report shader compilation errors in the editor and preserve the last valid
  render.
- Decide on a plugin ABI only after several built-in scenes expose its actual
  requirements.

Exit condition: a custom shader can be added, mapped to audio signals, saved in
a scene, and recovered gracefully when compilation fails.

## 7. Performance, sharing, and distribution

- Add performance presets for Apple Silicon/Asahi and conventional GPUs.
- Create deterministic demo audio and visual regression captures.
- Package scenes with their assets and import untrusted packages safely.
- Add scene thumbnails, metadata, and exportable bundles.
- Produce reproducible Arch Linux packages and signed GitHub releases.
- Document scene compatibility and migrations before declaring 1.0.
