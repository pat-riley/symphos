# Experimental signal input routing

This experiment lives on `codex/signal-input-routing`, branched from GitHub
`main` after Particle Waves and Spectral Bloom merged in PR #71.

## Try it

1. Open **Signals** in the footer. **Modulation** keeps the existing channel mixer.
2. Select **Audio inputs**, then **+ Audio input**. Name it, select Stereo,
   Left, Right, Mid or Side, and adjust gain, high-pass, low-pass and noise gate.
   The live spectrum and level show its output.
3. Open **Sketch connections**. Select a sketch and choose the named input.
   Several sketches can share it; each receives its waveform, spectrum and
   measurements. Main audio retains the existing direct connection.
4. Adjust that sketch's frequency, envelope and response controls below its
   connection. These are the same saved parameters previously in its inspector.
   Gain/filtering on a named input is shared; sketch response is independent.
5. Choose **Unassigned** to disable built-in audio response while keeping LFO,
   MIDI and separate modulation mappings available. Disable an audio input to
   silence all its consumers, or delete it to disconnect them.

The visualizer inspector has an **Audio & modulation…** shortcut to its Signals
connection. Signal Response, Frequency Range, channel routing and legacy
modulation sections appear in Signals. Appearance, geometry, motion physics,
camera and scientific-model settings remain in the inspector. Cymatics'
physical damping ratio stays with plate Geometry.

Modulation channels still produce control values/events. In a channel's Source
inspector, select a named audio input to run its detector on the same processed
audio used by a sketch. An input can therefore feed both a waterfall's spectrum
and a bass-envelope detector. LFO and MIDI sources remain independent of audio.

## Ownership and compatibility

- The master capture remains the single device/application source selected in
  Settings. Named inputs are processing branches of that capture, not additional
  simultaneous hardware capture devices.
- Inputs are created manually, up to eight per scene. Adding or changing a
  visualizer creates none. Deleting one disconnects consumers rather than
  substituting the master or creating a replacement.
- Existing scenes default to Main audio with no new input channels and retain
  their local response parameters. Routes and named input settings save with
  scenes/projects, workspace recovery and Undo/Redo.
- View presets retain input descriptions and reuse matching existing inputs.
  Missing or differently configured inputs remain disconnected. Component-only
  presets continue to edit component settings without changing its connection.
- The frame-rate preference remains global. Recording renders the connected
  visualizer feed; the recording soundtrack remains the captured master audio,
  as in the existing live-recording implementation. Named-input processing does
  not generate or play audio.

## Runtime

The existing capture callback and analysis worker remain independent of the UI.
Analysis snapshots now carry their analysis configuration. On the existing
Signals worker, each named input processes a fresh snapshot at most once, then
publishes shared immutable waveform/spectrum/measurement data. An unchanged
capture reuses its prior processed result; bypass shares the original buffers.

Processed stereo PCM receives channel routing, gain and optional second-order
Butterworth high/low-pass filters, followed by an optional RMS noise gate. Its
FFT and measurements are recalculated together using the main analysis window,
FFT size and settings. Filters replay the bounded half-second waveform as
pre-roll; they are visualization inputs, not a sample-continuous audio-output
effects engine. The gate is a simple hard gate, without a dedicated gate envelope.

Each input keeps a bounded thirty-second spectrum archive (at most 901 rows),
so connected history visualizers catch up after occlusion. Source/processing
changes clear incompatible histories. Input edits preserve particle simulation
and camera state. Renaming an input does not reset its processing or history.
Rendering and recording consume the same connection; no input DSP runs on the
capture callback or render thread.

Native input-processing timing is available through the ignored
`benchmark_input_processing` test. Tests cover filter/level behavior across PCM
and FFT, stereo cancellation, mute/gate, caching, source deletion, scene and
preset compatibility, and worker operation without any UI.

On Apple M4 Pro / macOS 26.6.1, a single high/low-pass stereo input at 48 kHz
and a 4096-point FFT measured 0.193 ms per fresh snapshot over 300 updates.
Reusing an unchanged capture measured 0.110 microseconds per worker lookup.
This excludes main capture/analysis, rendering and recording; higher sample
rates, larger FFTs and additional processed inputs increase the worker cost.
