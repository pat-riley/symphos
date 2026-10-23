# Experimental signal input routing

This experiment lives on `codex/signal-input-routing`, branched from GitHub
`main` after Particle Waves and Spectral Bloom merged in PR #71.

## Try it

1. Open **Signals** with the levels icon in the top navigation. Audio, LFO, MIDI and detector channels share
   one mixer; there are no extra dock tabs.
2. Right-click empty mixer space and choose **Add channel → Audio**.
   A **System audio** channel appears with a live frequency-spectrum preview.
   Existing experimental audio inputs also appear here without creating copies.
3. Click its header to edit the audio channel in the inspector. Set gain,
   Stereo/Left/Right/Mid/Side, high-pass, low-pass and noise gate.
4. Under **Connected sketches**, enable the sketches that should receive its
   waveform, spectrum and measurements. Expand a connected sketch's response
   controls to tune its independent frequency/envelope behavior.
5. Audio channels use an **Input gain** knob and a spectrum; control channels
   retain their **Visual influence** knob and time-history curves. MIDI retains
   its message-specific preview. Rename, duplicate, reorder, color, mute, solo
   and delete audio channels using the same mixer interactions.
6. Choose **Unassigned** from a sketch's audio connection to disable built-in
   audio response while retaining LFO/MIDI modulation. Muting an audio channel
   silences its consumers. Audio solo acts among audio channels, independently
   of control-channel solo. Deleting disconnects consumers without substitution.

The visualizer inspector's **Driven by** selector chooses the captured system
audio, an existing audio channel, or Unassigned. It never creates a channel.
**Inspect audio signal…** selects the existing connected channel in Signals;
there is no separate sketch-audio inspector. Set frequency range, gain, smoothing
and response in the audio channel's **Connected sketches → Sketch response**.
Peak markers, reference traces, note labels, appearance, geometry, motion physics,
camera and scientific-model settings stay in the visualizer inspector. Cymatics'
physical damping ratio stays with plate Geometry.

Modulation channels still produce control values/events. In a detector channel's Source
inspector, select an Audio feed to run its detector on the same processed
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
changes clear incompatible histories, while live gain adjustments keep the
archive scrolling continuously. Input edits preserve particle simulation
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

Audio detectors and control processing now use the typed
[insert-chain workflow](signal-inserts.md). The earlier fixed detector selector
and standalone detector creation commands have been replaced by insert slots.
