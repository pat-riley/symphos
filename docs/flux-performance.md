# Flux performance macros

Select a Flux pane to reveal its performance bar below the visualization. The
right inspector still contains the detailed geometry, appearance, and audio
mappings. The bar controls the selected Flux instance, not every pane.

The current control workflow is documented in
[Performance macros and inline modulation](performance-modulation.md). All
three artistic components now expose twelve knobs in one horizontally scrolling
row, source badges, draggable modulation rings, inline source editors, and
reaction presets. **Advanced** opens the right inspector. **Burst** and the
Cloud/Rings/Helix/Knot/Vortex/Petals formation menu remain available in Flux.

## Musical response

Continuous mapped energy shapes the body's movement and light. The additional
Impact detector compares fast and slow envelopes of the inputs routed to
Expansion and Turbulence, looking for rising-energy events. A threshold crossing
and refractory period distinguish new accents from sustained input. Its pulse
then decays over roughly 320 ms. This is onset emphasis; it does not classify
instruments, identify song sections, predict drops, or synchronize to a BPM grid.
A brightness-only route does not feed this motion detector.

The existing [audio mappings](audio-modulation.md) remain editable. Their source
bands, sensitivity, envelopes, and amounts still determine what each visual
response listens to. Mod amount zero removes audio-driven accents;
manual Burst remains available. Pause freezes all animation, macro smoothing,
and pulse clocks. A manual Burst queued while paused starts upon resuming.

## Research and design decisions

- [TouchDesigner Audio Analysis](https://derivative.ca/UserGuide/Palette%3AaudioAnalysis)
  exposes separate low/mid/high levels, kick/snare-like detection, and slow/fast
  spectral features. The useful principle here is to distinguish sustained
  energy from short events, instead of driving every visible behavior from one
  loudness value.
- [TouchDesigner audio workflow](https://derivative.ca/UserGuide/Creating_Audio_with_CHOPs)
  distinguishes envelope following, trigger envelopes, filtering, and smoothing.
  Flux packages these ideas into its existing mappings and a playable bar.
- [projectM](https://github.com/projectM-visualizer/projectm) separates PCM/FFT and
  beat analysis from preset-driven rendering. Flux likewise keeps its audio
  mapping engine separate from the formation and performance controls.
- [Apple Music visualizer documentation](https://support.apple.com/en-ph/guide/music/muse3aa39574/mac)
  describes effects responding to music type and playback state, but does not
  document the internal animation algorithm. The visual direction is an
  expressive, evolving scene with coherent motion and musical accents; this is
  not an implementation of Apple's visualizer.

These are design inputs, not evidence of a universal perceptual optimum. Default
amounts and timing will continue to benefit from listening tests across different
music and speakers/headphones.

## State and future MIDI

The macros edit the same underlying Flux settings used by its inspector, so
there are no competing copies of speed, trails, depth, or other base values.
Their values and the bar's visibility save with Flux presets and the workspace.
Pulse detectors, current slewed values, particle positions, color/flow clocks,
and queued Burst events never serialize.

The macro catalog has twelve stable string IDs and normalized 0–1 getter/setter
ranges. A future MIDI adapter can route CC values through those setters rather
than implementing a second control layer. Device selection, MIDI Learn, encoder
handling, and pickup/soft takeover are future work; this milestone does not open
MIDI ports.

Automated coverage includes the twelve controls and their saved values, separate knob/ring drag
interaction, a single-row bar, sustained versus transient response,
smoothing/zero recovery, distinct and continuous orbital paths, extreme live
audio and repeated bursts across all five formations, plot/sidebar/help docking,
brightness-only isolation, and pause/frame-rate behavior.
