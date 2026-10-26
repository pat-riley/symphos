# Signal channels and inserts

Signals open with a guided setup. Right-click the Signals mixer and choose
**Add channel → Loudness**, **Bass Energy**, **Band Energy**, **Bass Hits**,
**Spectral Change**, **Stereo Balance** or **Beat**. Choose **Audio only** for an
untouched waveform/spectrum feed; **LFO** and **MIDI** remain available.
Channels are created only by an explicit add, duplicate or paste action.

The strip's type dropdown changes what the signal detects. **Edit response…**
opens its inspector: Source, Detect, Response, Live response and Mapped
parameters. Audio-only strips show **Set up signal…** until a detector is selected.
The source is the capture configured in Settings; these controls do not change
speaker playback. Loudness is an RMS-based artistic control, not calibrated LUFS.

## Main audio and a bass-driven brightness macro

1. Keep the permanent **System audio** source available for the waterfall's
   waveform/spectrum, or choose an existing audio channel in **Driven by**.
2. Add **Bass Energy** from the Signals menu. It immediately produces a control
   curve; there is no detector insert to create manually.
3. Click **Edit response…**. Adjust its frequency range, sensitivity, noise
   floor, attack/release and **Auto level** in the inspector.
4. Compare the gray detected signal and colored final output in **Live response**.
   The final output includes extra processing and the Visual influence setting.
5. Assign brightness to a scene macro and choose this signal as its modulation
   source. **Mapped parameters** lists its current parameter destinations.

**Bass Energy** follows sustained low-frequency strength. **Bass Hits** produces
pulses on sudden rises. **Spectral Change** follows changing textures and attacks,
while **Beat** estimates rhythmic timing or provides a manual tempo clock.
Each type explains its behavior and exposes the relevant response controls.

The guided controls edit the existing saved detector in place. Viewing a saved
signal changes nothing; choosing its current type retains all adjustments.
Choosing a different type changes that detector's settings while keeping the
channel ID, custom name, routes, preceding audio processing and following control
processors. Returning a detector channel to Audio only requires the explicit
removal action under Advanced, so dependent processors are never silently lost.

## Advanced insert chains

Expand **Advanced · Insert chain** in the signal inspector to add, inspect,
reorder, bypass or remove processors. These are the same processors edited by
the guided controls; there is no second processing engine or project format.
Custom chains remain intact and reopening them does not reconstruct a recipe.

Detectors already include adaptive scaling and attack/release. Add Smooth,
Envelope, Normalize or Curve only when another response stage is useful. Click
an insert for its detailed input/output monitor. Normalize's **Use recent input
range** maps the last two seconds of observed input onto 0–1; enter exact limits
for a narrower range. Filters process the visualization's waveform/spectrum;
control processors change modulation values. None changes speaker output.

## Data types and order

An audio channel starts with audio. Audio gain, frequency filters, channel
selection and an audio gate process waveform data. A detector converts that
branch to a control curve; further inserts shape the curve. The most recent
audio before the detector remains available as the channel's main visualizer
feed. An Audio channel without a detector has no modulation curve yet. The
macro source picker offers it as **<channel name> · add Overall intensity**;
selecting that action appends the detector and connects its control output.
Opening the picker alone does not modify the channel. Existing detector, LFO,
and MIDI control sources remain directly selectable. A full insert chain must
have room made for the detector in Signals first.

LFOs and decoded MIDI messages start as control values and offer control
processors directly. Audio filters and audio detectors cannot be inserted there.
The existing MIDI message selection, device/channel filters, sustain handling,
Learn and message-specific previews remain available. MIDI inserts act on the
normalized control value, not on a generated MIDI or audio output stream.

Drag insert slots to change their order, or use their right-click menu. Invalid
data-type transitions are rejected. Bypass preserves a processor's position.
Click an insert and press Delete or Backspace to remove it; click the channel
header first to delete the whole channel. Insert and channel context menus are
separate, and typing in an editor never deletes the selected channel.
Bypassing a detector silences its control conversion while preserving audio.
Removing a detector also removes its dependent control inserts, as the menu
states; Edit → Undo can restore the edit. A chain supports up to sixteen inserts.

## Included processors

- Audio: gain, a combined frequency filter, separate high/low-pass filters,
  stereo/left/right/mid/side routing and an RMS noise gate.
- Detectors: overall intensity, bass energy, arbitrary band energy, bass hits,
  positive spectral change, signed stereo balance and beat detection.
- Control: fixed or adaptive normalization, attack/release envelope, time-based
  smoothing, gain, hysteretic threshold, curve/dead zone, invert and steps.

Beat detection starts with adaptive low-band onsets, retrigger protection and
robust median interval timing. It exposes a beat pulse or 0–1 phase, with tempo
and confidence readouts. Automatic phase requires consistent recent beats;
silence and irregular material do not create a confident clock. Manual BPM is
available when the audio does not provide clear rhythmic evidence. Tempo can
still follow subdivisions or miss weak beats; this is a live musical detector,
not a guaranteed tempo/transcription service.

## Persistence and runtime

Older detector channels migrate in place to Audio channels with matching
inserts. Their IDs, names, colors, order, response settings and macro bindings
are retained. Existing LFO/MIDI normalization, curve shaping and smoothing also
become inserts. Audio-only gain/filter/gate settings migrate into audio inserts.
Migration is idempotent and does not add extra mixer channels. Scenes, projects,
recovery, undo/redo, duplication and preset dependency descriptions retain chains.
A visual preset reuses a matching existing source; it never creates sources.

The independent Signals worker executes the chain, never the capture callback
or UI render loop. Bypass branches share immutable capture arrays. PCM inserts
reuse the existing second-order Butterworth/filter and FFT analysis path, with
bounded half-second pre-roll; they are visualization processors rather than a
sample-continuous audio effects output engine. Smoothing/envelopes use elapsed
time rather than UI frames. Each insert publishes a bounded two-second scope;
runtime detector state, tempo estimates and scopes are not saved in projects.
Live input/insert gain edits preserve the archive and apply to incoming audio
without restarting history visualizers. Source, filter, routing and chain
structure changes invalidate incompatible history; editing a later control
processor leaves the audio feed/history intact.

On the development Mac (Apple M4 Pro, 48 kHz / 4096-point FFT), the explicit
worker benchmark measured **0.171 ms per fresh snapshot** for two audio sources
with frequency filter, bass energy, normalization and smoothing on the second
branch. This measures the Signals worker only, not total UI/GPU frame time.
