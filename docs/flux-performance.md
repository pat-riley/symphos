# Flux performance macros

Select a Flux pane to reveal its performance bar below the visualization. The
right inspector still contains the detailed geometry, appearance, and audio
mappings. The bar controls the selected Flux instance, not every pane.

Click **Flux macros** or **Hide** to collapse the bar; click **Show Flux macros**
to expand it. **P** also toggles it. Twelve controls fit in one row on wider
workspaces and two rows on narrower ones, leaving the visualization usable.
In panel fullscreen the bar starts hidden; press P to bring it up for performance.
**Inspect** opens the right sidebar, exits panel fullscreen if necessary, and
reveals the relevant section for the last-touched macro. For a macro with an
audio mapping, it selects that destination's route.

## Play the controls

The four vertical faders are levels and pacing controls. The eight rotary knobs
shape motion and appearance. Drag upward to increase, hold Shift for fine
adjustments, or double-click to restore that control's default. The percentage
field supports exact entry. Focused controls also accept arrow keys.

| Control | Type | Performance effect |
| --- | --- | --- |
| Brightness | Fader | Master visual light level. Zero blacks out particles and cores; captured audio is unaffected. |
| Speed | Fader | Base travel rate, with music adding movement. Zero settles the particle clock to a stop; illumination can still react. |
| Expansion | Fader | Resting extent of the formation, leaving room for music-driven expansion. |
| Impact | Fader | Rising musical energy adds brief outward impulses, expansion, illumination, longer trails, and momentum. Zero removes these extra accents while normal mappings remain active. |
| Swirl | Knob | Circulation speed and direction. The midpoint is neutral; lower values reverse circulation. |
| Turbulence | Knob | Smooth travel to disturbed, detailed paths, combined with the mapped high-frequency response. |
| Magnetism | Knob | Independent 3D cloud paths to tightly held orbital formations. |
| Shape | Knob | Continuous morph through Rings, Helix, Knot, Vortex, and Petals. |
| Depth | Knob | Compress or stretch the displayed third dimension. |
| Trails | Knob | Particle streak length, with short extra extensions from Impact. |
| Glow | Knob | Halo around points and cores. |
| Color drift | Knob | Palette circulation, independent of instantaneous audio-driven brightness. |

Percentages represent the control's safe range, not the current audio-driven
result. Thin accent meters show the mapped music influence on the related
controls; Impact's meter shows its actual transient envelope. The manual settings
remain visible while audio adds motion underneath. Values glide over about
65 ms to make fast mouse changes—and future controller changes—less abrupt.
Very small residual differences snap to the target so zero speed and brightness
actually reach zero.

## Drop and formation gestures

Try keeping Speed and Impact moderate during a verse, then raising Impact,
Expansion, and Brightness together for a drop. Increase Trails for longer
filaments, or briefly lower Magnetism to release a tightly held formation into a
cloud. **Burst** launches a manual one-shot accent using the current Impact
setting. It also works without waiting for an audio onset; Impact zero makes it
visually neutral.

The formation menu beside the panel number provides quick starting points:

- **Cloud** releases magnetism while keeping the same particles.
- **Rings** uses orbital paths in alternating planes.
- **Helix** distributes particles along a cylindrical corkscrew.
- **Knot** traces a torus-knot-like path with intertwined loops.
- **Vortex** forms a widening spiral funnel.
- **Petals** creates a lobed orbit with motion through three dimensions.

Choosing an orbital form sets Shape to its anchor and Magnetism to 90%. Shape
can then morph between those anchors; increase Magnetism for tighter paths or
reduce it for diffuse formations. Audio expansion acts on every formation.
These changes move the existing particles through bounded forces, without
reseeding or teleporting them. Auto orbit in the inspector rotates the camera
and is independent of the orbital formation.

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
response listens to. Master Audio amount zero removes audio-driven accents;
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

Automated coverage includes the twelve controls and their saved values, drag
interaction, narrow/wide bar layouts, sustained versus transient response,
smoothing/zero recovery, distinct and continuous orbital paths, extreme live
audio and repeated bursts across all five formations, plot/sidebar/help docking,
brightness-only isolation, and pause/frame-rate behavior.
