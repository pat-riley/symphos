# Audio mappings for visualizers

> The Signals workspace now owns shared extraction and envelopes. See
> [Signals and projects](signal-sources.md) for the current workflow. This document
> also describes the retained legacy route math and Strata local band bank.

Veroni, Strata, and Flux share an **Audio mappings** section at the top of their
inspectors. Scientific instruments continue to show their measured signals with
their existing controls. The component browser groups these two families with
small headings; mixed workspaces still use one capture and analysis stream.

## Make a mapping

1. Open a visualizer's inspector with its eye button.
2. Select an existing mapping, or use **Add mapping** to choose a destination.
3. Choose **Frequency band**, **Full mix**, or **Stereo balance** as its source.
4. For a frequency band, drag the two bright edges over the live spectrum, or
   enter exact low/high frequencies. The axis is logarithmic, from 20 Hz to 20 kHz.
5. Choose a response mode and shape its envelope. Adjust **Amount** to control
   how strongly it changes the destination; negative amounts reduce it.

Each mapping has an enable checkbox and a remove button. Multiple mappings can
feed the same destination; their outputs add. There are at most 24 mappings per
component. All changes are local to that component instance.

**Ambient motion** sets animation between sounds. **Audio amount** scales all
of the component's mappings. Set Audio amount to zero to hear the same audio
while seeing only ambient animation. Set both to zero to stop the simulation
clock; movement mappings can wake a zero-ambient Flux cloud when Audio amount
is above zero. Camera auto-orbit is controlled separately. Pause freezes the full pane,
including envelopes and automatic rotation, while manual camera gestures remain
available.

## Response and envelope

| Mode | Behavior |
| --- | --- |
| Level follower | Tracks continuous band energy using Attack and Release smoothing. Decay and Sustain are disabled. |
| Gated ADSR | Opens above the trigger threshold, attacks to 1, decays to Sustain, holds while the input stays above the gate, then releases to zero. A small hysteresis prevents gate chatter. |
| Transient ADSR | A rising-energy event triggers the envelope for the configured Hold duration. Sustained input does not repeatedly retrigger it. This is onset detection, not tempo synchronization. |

Drag **A**, **D**, and **R** horizontally to change their durations. Drag **S**
vertically to change its level. Numeric fields provide exact timing in
milliseconds. The horizontal live line shows the current envelope value; the
Input and Envelope meters show detection and shaping separately. For an automatic
Strata mapping, the meters show the strongest layer voice.

Under **Sensitivity & output range**:

- **Floor dB** rejects low-level input. **Gain dB** adjusts the detector input.
- **Curve** changes the normalized input response.
- **Output min/max** remap the envelope before Amount is applied. A nonzero
  minimum intentionally produces an offset even in silence.
- **Adaptive sensitivity**, above the mapping list, follows recent per-band peaks
  with slow recovery. It never lowers the reference below −42 dB or bypasses the
  noise floor, so it does not amplify digital silence.

These normalized controls are artistic modulation signals, not calibrated
measurement readouts. The frequency spectrum is unweighted raw FFT band power;
its resolution is limited by the selected FFT size and sample rate. Very narrow
bands can contain no FFT bins. Full mix uses the mixed signal's RMS. Stereo
balance uses signed left/right RMS balance weighted by mix level.

## Destinations

| Visualizer | Destinations |
| --- | --- |
| Flux | Motion speed, expansion, swirling flow, turbulence, brightness, particle/core size, horizontal steering |
| Veroni | Motion speed, cellular expansion, swirling flow, fine turbulence, brightness, point size, horizontal steering |
| Strata | Motion speed, layer bend, layer ripples, layer shimmer, brightness, dust/grain size |

Motion and appearance mappings are independent. A brightness-only mapping does
not alter the particles' trajectories or the cells' geometry. Geometry and
appearance settings remain the base values; modulation never overwrites them.
Output sums and the resulting geometry are bounded, including negative amounts.

The default Flux and Veroni maps use bass for expansion, mids for swirl, highs
for turbulence, broad energy for brightness and speed, and stereo for steering.
Size is separate. You can make a kick-only burst by adding an Expansion mapping,
selecting a low band, choosing Transient ADSR, and setting a fast attack and a
longer release.

Strata's default layer mappings each divide 20 Hz–20 kHz into equal logarithmic
ranges, one per layer. Bottom layers listen to low frequencies and top layers to
high frequencies. **All layers · auto bands** divides the *selected* range, so
changing the layer count or range redistributes it automatically. Each layer has
an independent envelope. Choose a numbered layer to send the entire selected
range to that layer alone. Full-mix/stereo sources feed the same source to every
selected layer instead of splitting frequencies. Brightness is a separate global
mapping and can illuminate the entire sculpture.

## Presets and migration

Mappings, bands, amounts, envelopes, ambient motion, and sensitivity settings save
with component presets, full-view presets, and the last workspace. Layout-only
presets still contain no component configuration. Runtime envelopes, audio
features, particle positions, and animation clocks are never serialized.

Older visualizer settings acquire editable default routes on load. Legacy
sensitivity, noise floor, attack/release, and destination influence values seed
those routes. Geometry, palettes, camera settings, and workspace layout survive.
The new modulation behavior is not a pixel-for-pixel replay of the old hardcoded
responses. New components default to reduced ambient motion; migrated components
retain full ambient motion for continuity.

## Implementation boundaries

`analysis.rs` creates one immutable `AudioFeatures` snapshot on the analysis
worker, shared by all panes through `Arc`. Prefix-summed FFT power supports exact
band queries without rescanning the spectrum for each mapping. Capture callbacks
and OS audio backends are unchanged.

`modulation.rs` contains source extraction, bounded route settings, independent
per-route/per-layer envelopes, and output accumulation. `modulation/ui.rs` draws
the native spectrum and envelope editors. Artistic renderers consume outputs;
scientific renderers do not. A future plugin or capture adapter can publish the
same analysis contract without changing the mapping UI or envelope engine.
This milestone does not add per-app capture, multiple simultaneous audio devices,
MIDI, external sidechains, or a DAW plugin.

Route identity is stable across insertion/deletion. Source/range/target/mode
changes reset the affected envelope, layer-count changes rebuild automatic
voices, and capture epoch or FFT-format changes clear incompatible state.
Paused panes use frozen frames and clocks. Flux retains its bounded 120 Hz
simulation; its cached audio features are reused across simulation steps.

Automated tests cover frequency boundaries, quiet high bands after loud bass,
ADSR stages/release/retriggering, silence, rate independence, source resets,
layer redistribution, routing isolation, migration, settings round trips,
compact inspector layout, and dragging both frequency and envelope handles.
Native Mac checks do not substitute for a real Omarchy audio/GPU session.
