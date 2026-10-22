# Cymatics

Cymatics is an incoming-audio visualizer. It never generates or routes audio out.
Use **+ → Cymatics** or **Create → Component → Cymatics**. The inspector offers:

- **Geometry:** Sand, Wave surface, Nodal patterns, or Interference; square,
  rectangular, or circular plates; simply supported, clamped, or free edges;
  optional invisible point support at the center; aluminum, steel, or brass;
  plate dimensions, Poisson ratio, and excitation position.
- **Particles:** 8K–131K grains (32K default), migration, friction, agitation,
  size, size variation, opacity, soft sprites, edge recycling, and Scatter grains.
  **Recycle grains at edges** defaults on: grains in the outer 4% of the plate
  return to a random point within its central 35%, with velocity cleared. This
  artistic option keeps incoming audio producing continuous flow instead of a
  pile at the rim. Disable it for natural boundary settling. It respects pause
  and stops recycling when vibration has faded.
- **Signal Response:** input gain, frequency range, noise gate, resonance damping,
  attack/release, automatic visual sensitivity, live input/drive/vibration readings,
  and the range of modeled resonances.
- **Appearance:** five starting palettes, editable endpoint and plate colors,
  color drivers, contrast, opacity, glow, surface height, illustrative phase rate,
  nodal contours, and directional lighting/material controls.
- **Flow & Motion:** optional artistic sand turbulence and interference emitter
  count, spacing, phase spread, and wave-number scaling.
- **Camera:** free trackball rotation, pan, zoom, perspective/orthographic views,
  top-view/reset actions, and optional XYZ guides. Camera movement does not
  physically tilt the plate under gravity.

Numeric inspector labels can be dragged to scene macros and mapped to existing
Signals. Settings, colors and mappings persist through the existing scene,
project and component-preset systems. Particle positions, wave fields, GPU
resources and clocks are runtime state and are not serialized. No source
channels are created automatically.

## Physical model and its limits

The bending model follows linear isotropic Kirchhoff–Love thin-plate theory:

`D = E h³ / (12 (1 − ν²))`, `D ∇⁴w + ρh ẅ = force`.

For a simply supported rectangle of width `a` and height `b`, the exact modes are
`sin(mπx/a) sin(nπy/b)` and their frequencies are
`f_mn = (π/2) sqrt(D/(ρh)) ((m/a)² + (n/b)²)`.

Other configurations use a degree-eight polynomial Rayleigh–Ritz basis and
midpoint quadrature over the plate. The trial functions enforce displacement
and slope constraints on supported/clamped boundaries. Free-edge conditions
are natural conditions of the bending-energy functional. A center support
projects the trial space onto zero center displacement. Rigid-body modes are
excluded. Disk quadrature approximates the circular boundary; higher modes
are less accurate than low modes. This is a reduced model, not a converged FEM
solver or a calibrated measurement instrument.

The lowest 24 bending modes are retained. Up to six separated spectral peaks
from the existing audio analysis drive complex damped modal responses, weighted
by mode shape at the excitation point and generalized modal mass. Modes driven
at the same frequency interfere coherently; energies from different driving
frequencies are summed. Audio FFT magnitudes do not preserve original phase
relationships between separate frequencies. Frequency resolution is limited by
the global audio FFT setting. The excitation gain represents an unspecified
transducer; visual height is not displacement in meters.

**Automatic visual sensitivity** is enabled by default. It applies one display
scale to the whole modal response, targeting a vibration level proportional to
accepted input amplitude and excitation gain. This makes mid/high-frequency and
quieter audio visible despite small off-resonant physical displacement. Spatial
nodes, modal interference, and energy ratios across the plate are preserved;
absolute resonance-amplitude comparisons are not. Disable it for the original
relative physical amplitudes. Gain is bounded; silent or gated input is never
normalized into motion. Live excitation readings distinguish silent input,
filter/gate rejection, and a weak vibration response. The inspector warns when
the frequency filter misses the modeled resonance range.

Grains migrate down the gradient of time-averaged vibration energy with drag
and vibration-dependent agitation. This approximates collection at nodal lines;
it does not model grain collisions, vertical impacts, pile height, air flow,
or sand changing plate mass. Grain motion is fixed-step bounded (at most 1/120 s
per step). Natural settling is the default; turbulence is opt-in. After silence,
the response decays and grains stop rather than continuously generating patterns.

Wave surface and Nodal patterns show the same modal response. Wave motion is
an explicit slow-motion phase illustration, not a real-time measurement of
hundreds of cycles per second. Interference is an artistic radial-wave mode:
incoming dominant pitch sets bending-wave dispersion, with configurable emitter
positions and phase. It is not an additional physical boundary-value solution.

Steady narrow-band input makes clearer Chladni figures than rapidly changing,
broadband music. Plate dimensions and thickness tune the resonances to the
incoming sound. Moving excitation onto a node suppresses that mode.

References:
- Arthur W. Leissa, **Vibration of Plates**, NASA SP-160 (1969):
  https://ntrs.nasa.gov/citations/19700009156
- COMSOL, **How Do Chladni Plates Make It Possible to Visualize Sound?**:
  https://www.comsol.com/blogs/how-do-chladni-plates-make-it-possible-to-visualize-sound

## Performance and validation

Display FPS remains a global device preference (Settings → Display). Scene
switches do not change it. Audio analysis rate is a separate existing scene
analysis setting. Cymatics uses elapsed time and bounded simulation steps;
changing display FPS does not intentionally change settling speed. Long stalls
are capped at 100 ms rather than attempting unbounded catch-up.

Plate modes solve on one background worker per active component. GPU pipelines
are shared; each pane owns its grains, field and render targets. Default grain
count is 32,768; render targets are capped at 1920 pixels on the longest edge.
Higher counts, multiple components, Retina fill rate and CPU audio response all
contribute to actual frame time, so 60 FPS is a target rather than a guarantee.

Tests cover analytical frequency scaling and nodes, agreement with Leissa's
clamped-square/circle fundamental frequencies within 2.5%, center support,
finite modes, resonant/off-resonant drive, rejection at a node, quiet PCM through the production FFT and modal field, sensitivity preserving nodes and input-level differences, silence/gating,
frame-rate-independent envelopes, serialization/macro addressing, legacy schema
migration, WGSL validation, and real-GPU settling, boundaries, all four rendering
modes, resizing, and preserving the initial grain scatter when multiple updates share a
GPU submission. Frame data uploads are encoded beside their own dispatches to
prevent a later update from overwriting an earlier scatter command. The GPU
test runs explicitly with:

```sh
cargo test -p symphos-visuals cymatics_gpu_settles -- --ignored --nocapture
```
