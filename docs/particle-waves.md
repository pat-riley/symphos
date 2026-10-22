# Particle Waves

Add **Particle Waves** from the **+** component browser or **Create** menu.
Incoming FFT measurements form luminous 3D layers; no audio is generated.
The initial view uses four blue/lime/white ribbons with gentle drift and depth
of field. The global frame-rate preference applies, including 120 FPS.

## Shape and layers

**Geometry** selects Ribbons, Rings, Spiral, or Tunnel. Frequency runs along
the ribbon or circumference, and recent audio history extends along its depth
(or radius for rings). Spectrum height comes directly from the measured level
within the selected dB range. Set Resonant ripple and Drift strength to zero
for an attached spectral surface without artistic motion.

Use one to eight layers. Shared settings apply initially; choose a layer to
change its position, rotation, tilt or scale. Enable its override to set its
own palette and particle size, density, opacity, and motion multiplier. Layer
selection is editing context; it does not reset the other layers. Transform
and override numeric controls can be mapped independently to scene macros or
MIDI, using stable paths such as `layers.3.size`.

**Signal Response** controls frequency bounds, logarithmic/linear spacing,
display gain, smoothing, decay and visible dB limits. History is retained as
raw spectral frames and reprocessed when the frequency settings change.
History is captured at up to 30 Hz, interpolated continuously for rendering,
and remains separate from the global display FPS. Actual capture gaps are
left empty rather than inventing spectrum data.

## Particle motion and appearance

**Flow & Motion** controls audio-driven displacement away from the spectral
surface. Drifting fraction keeps a stable population attached while a separate
subset can scatter (25% by default). Drift strength and Audio scatter spread
grains on louder input;
Return strength pulls them back and Motion damping reduces overshoot.
The motion envelope has its own attack, release and threshold. Frequency
filtering applies to both the main shape and the scatter envelope. After
input stops, agitation fades and particles return toward the surface.

Resonant ripple adds illustrated traveling waves proportional to local
spectral level. Dominant incoming pitch influences their spacing. This is an
artistic response inspired by vibrating surfaces, not a calibrated physical
simulation. It does not replace the underlying measured spectrum.

**Particles** offers 8,192–131,072 total particles, shared among the layers,
plus density, radius, variation, opacity, softness, and disc/square/diamond/ring
sprites. Quality stays explicit; the renderer does not silently reduce it.

**Appearance** contains a three-color palette with particle, frequency, level,
or layer color drivers. Gradient blend ranges from distinct colored grains
to smooth color transitions. Glow and brightness affect rendering only.
Depth of field has focus distance, defocus strength and a maximum bokeh radius.
Focus distance is measured from the camera; matching Camera distance focuses
around the scene center. Blurred light is spread across the sprite to limit
fogging. Particle and blur sizes scale with output resolution.

## Navigation and integration

Drag to rotate freely, including roll. Shift/right/middle-drag pans; scroll
zooms; double-click resets the camera. **Camera** also exposes auto-rotation,
rotation speed/direction, lens, distance, orthographic projection and optional
XYZ grid guides. Manual rotation stops auto-rotation.

Settings participate in scene/project saving, presets, Undo/Redo, macros,
Signals and numeric MIDI Learn. Adding this component creates no signal
channels. Per-pane pause freezes history, particle movement, illustrated ripple
phase and auto-rotation while allowing camera inspection. Live recording uses
the selected output resolution rather than the interactive preview size cap.

## Rendering and verification

A dedicated module in `symphos-visuals` runs the particle spring/damping
simulation on the GPU, with substeps no longer than 1/120 second. Particle
state remains on the GPU. Each instance owns its history, particle buffers and
target; pipelines are shared by its renderer. Uploads are ordered before their
own compute pass so batched callbacks cannot overwrite another frame's data.
The simulation uses elapsed time, caps large resume gaps, and does no work in
the audio capture callback.

Automated checks cover measured peak position, silence, time-based envelopes,
pause, layer parameter addressing, old sixteen-component project migration,
settings round trips, bounds, WGSL validation and shader ABI. A native Metal
probe checks audio-driven displacement, spring return, pause/resize stability,
60/120 FPS integration, all four modes, visible pixels and GPU validation.

On an Apple M4 Pro, a 32,768-particle 1280×720 GPU probe across all four
modes measured 0.526 ms median and 2.439 ms p95 for submission plus GPU wait.
This excludes host UI, audio analysis, encoding and presentation, and is not
a guarantee of 120 FPS for every scene. Large bokeh discs, high particle counts,
multiple components and high-resolution recording increase rendering cost.
