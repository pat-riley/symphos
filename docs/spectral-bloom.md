# Spectral Bloom

Add **Spectral Bloom** from **+** or **Create → Component**. It starts with
three cascading spectrum curtains using the default visualizer palette in
Settings. Existing presets retain their saved colors. The reference image's
code/text is deliberately absent; this component visualizes the incoming audio.

## Shape

The top of each column follows the measured FFT level. Columns descend into
curved strands, then folds and a cloud of luminous grains. Frequency runs
left-to-right; the layer and strand depth sample recent measured history.
The folds are artistic transformations, not a physical acoustic simulation.

**Geometry** controls width, height, depth, layer spacing, history duration,
frequency-column count, strand bend, curl and tail length. **Dissolve to
particles** allocates a separate portion of the GPU budget to the cloud so
removing grains does not randomly break the remaining filaments. Cloud size
and brightness are independently adjustable. Individual layers have position,
rotation, tilt, scale and optional palette/appearance overrides.

**Signal Response** selects frequency bounds, level range, gain, smoothing and
decay. The motion envelope controls attack, release, threshold, scatter, size
and brightness response. The spectrum itself remains measured data even when
artistic motion is turned up. **Flow & Motion** controls drift, return and
damping of the dissolved particles; connected filaments remain anchored.

**Particles** sets total budget, grain shape, radius, variation, density,
softness and opacity. Density below one intentionally thins out both strands
and cloud. **Appearance** sets the palette, color driver, glow, brightness and
depth of field. Cloud grains receive optical defocus; connected filaments keep
their thin profile. Particle Waves' ring/ribbon arrangement and free-particle
fraction controls do not apply to this profile.

## Feedback

**Feedback** transforms the previous rendered image and blends it with new
light. Echo lifetime is in seconds; expansion, rotation and drift are rates
per second. Distortion adds audio-sensitive flow, and color separation offsets
red and blue samples. These affect the lingering image, not the measured
frequency positions. Disable Image feedback for a clean sculpture.

The renderer uses floating-point ping-pong textures so faint echoes decay
instead of sticking at an 8-bit rounding floor. Fresh light and decay are
normalized to elapsed time. Pause freezes the accumulated image. Editing the
camera or appearance while paused redraws a clean image; resizing clears
feedback rather than stretching old pixels. Resetting the component also
clears its accumulated feedback. A component owns its own feedback buffers;
other panes and their audio settings are unaffected.

## Camera and integration

Drag to rotate in 3D, Shift/right-drag to pan, scroll to zoom, or use the
Camera inspector. Auto-rotate, orthographic projection and an XYZ guide grid
are available. Numeric inspector controls support scene macros and MIDI Learn.
Settings save with scenes/projects and presets, including layer overrides and
feedback parameters. Adding this visualizer does not create signal channels.
The global FPS preference applies, including 120 FPS. Recording uses the
requested output dimensions.

## Implementation and checks

Spectral Bloom is a distinct registered component with an independent settings
bank and runtime. It shares spectrum-history sampling, camera controls and the
GPU particle renderer with Particle Waves. Its geometry branch creates connected
filaments plus a separate grain population; its extra GPU pass implements image
feedback. Audio capture and analysis code are unchanged.

Tests cover project migration from seventeen components, settings round trips,
parameter mapping, shader validation, and existing visualizer registration/UI
contracts. Native Metal tests check visible rendering, retained light after new
emission stops, decay to darkness, pause, resize and feedback disable. The
existing Particle Waves, Orbital and Cymatics GPU tests also pass with the shared
renderer changes. GPU timings exclude audio analysis, UI, presentation and
recording/encoding; complex scenes and large outputs can still exceed a frame's
budget.

## Gradient palettes

**Appearance → Palette** uses the shared factory/saved gradient browser and
**Edit gradient & swatches** editor, including 2–8 stops and cyclic gradients.
Individual layer overrides have the same picker. Color driver, spread, offset,
saturation and Gradient blend still control the mapping. Existing presets keep
their original three-color distribution until a palette is selected. Color edits
preserve particle motion and the incoming spectrum history.
