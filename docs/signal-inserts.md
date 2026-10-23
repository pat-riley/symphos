# Signal channels and inserts

Create **Audio**, **LFO** or **MIDI** from the Signals mixer's right-click menu or
Create → Signal Channel. Detectors are inserts, not channel types. Channels are
created only by an explicit add, duplicate or paste action.

## Main audio and a bass-driven brightness macro

1. Add an **Audio** channel and name it **System audio**. Leave its insert list
   empty for an untouched waveform and spectrum.
2. Select the waterfall. Choose that channel in **Driven by** at the top of its
   visual inspector.
3. Add another **Audio** channel and name it **Bass**.
4. Click **+ Insert → Bass energy**. Its opinionated defaults measure 30–180 Hz
   with adaptive level scaling and a fast attack / slower release.
5. Add **Smooth**, **Envelope**, **Normalize** or **Curve** after the detector as
   needed. Click an insert to see its controls and live input/output traces in
   the inspector. Normalize's **Use recent input range** maps the last two seconds
   of observed input onto 0–1; enter exact limits for a narrower range.
6. Assign brightness to a scene macro, then choose **Bass** as its modulation
   source. Keep the waterfall's **Driven by** connection on the first channel.

The two channels share the master capture but have independent processing.
Changing a bass-control curve never changes the first channel's spectrum or
speaker output. Bass energy measures a band without changing the audio feed;
add **Frequency filter** before the detector if a sketch should also receive
band-limited waveform/spectrum data.

## Data types and order

An audio channel starts with audio. Audio gain, frequency filters, channel
selection and an audio gate process waveform data. A detector converts that
branch to a control curve; further inserts shape the curve. The most recent
audio before the detector remains available as the channel's main visualizer
feed. With no detector, an Audio channel is not offered as a modulation source.

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
