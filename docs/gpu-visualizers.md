# Native GPU visualizers

`crates/symphos-visuals` is an internal Rust/wgpu library independent of egui,
Symphos settings, audio devices, or project files. Orbital is its first working
sketch: a GPU orbital/flocking hybrid inspired by the kinds of collective motion
in Robert Hodgin's Magnetosphere work. The shaders and implementation are original;
this is not a port of Magnetosphere or Cinder.

## Responsibilities

- The app owns audio capture, analysis, Signals, scene macros, inspector selection,
  project persistence, and the display clock.
- `src/modules/orbital.rs` owns versionable, settings-only state and its parameter
  catalog. Its stable field IDs drive inspector editing and existing macro bindings.
  Continuous fields share bounds, defaults and help in one declaration. Particle
  quality and attractor count are discrete controls and not modulation targets.
- `src/visuals.rs` adapts the sketch to egui-wgpu prepare/paint callbacks. It shares
  compiled pipelines through renderer resources. Each pane owns an independent
  GPU instance; deleting a pane releases it, and switching away releases its state.
- The library owns GPU buffers, spatial binning, simulation, instanced drawing,
  per-instance render targets, HDR history, bloom and final tone mapping. It never
  captures audio or performs interactive CPU particle readback.

The library is deliberately an internal API, not yet a dynamic plugin SDK.
Existing scientific/CPU components remain supported without being rewritten.
A future external renderer can feed a texture through a separate adapter; this
version does not introduce a C++ ABI, OpenGL interop, or MilkDrop support. Do not
force an external engine to implement Orbital's simulation or shader structure.

## Pipeline

1. Clear a 16³ spatial grid.
2. Bin particle positions and velocities into fixed-point atomic sums.
3. Compute 27-cell neighborhood averages, cohesion, alignment, separation, orbital
   forces, core repulsion, turbulence and a soft spherical boundary.
4. Write updated particles into the other storage buffer (no in-place read/write).
5. Decay the previous HDR frame, then draw velocity-aligned particle quads and
   analytically shaded sphere impostors. This is an additive artistic renderer;
   it does not provide general mesh occlusion, physical collisions or shadow maps.
6. Blur a quarter-resolution target horizontally and vertically, then composite
   bloom and tone-map into the host's viewport/scissor.

Work is bounded O(particles × 27 cells), not all-pairs flocking. Fixed-point bounds
keep grid sums below signed 32-bit capacity at the maximum particle count. Shader
pipelines are compiled lazily once per device. Particle buffers are allocated only
on initialization/quality change; resizing replaces textures, not particles.
Camera changes clear screen-space trails so old views do not smear into the new one.
Pause retains simulation/history; resizing and camera interaction remain supported.

## Particle art direction

Particles → Particle form offers Soft glow, Disc, Square, Diamond, Ring, Spark and velocity-aligned Arrow
silhouettes, size variation, aspect, edge softness, ring thickness, rotation/spin,
orientation variation, opacity and brightness variation. Geometry remains instanced
quads: larger sprites and long streaks increase fill cost without adding mesh data.

Particles → Color offers Spectral, Ember, Glacier, Aurora, Candy, Gold,
Monochrome and a Custom gradient with two saved color swatches. Color can follow
particle identity, speed, distance, height or nearest attractor. Spread, offset,
saturation and cycling are continuous macro targets. The non-spectral gradients
ping-pong at their ends when animated; Monochrome is neutral white. Attractor
brightness is independent, including zero to hide spheres without removing forces.
All 111 continuous parameters can be assigned to scene macros. Shapes, palette,
color driver and swatches are saved inspector choices. Existing projects default
to the original spectral soft-glow look. Appearance edits also redraw paused views.

## Orb art direction

The Orbs inspector groups contain:

- Form: nine analytic styles (Shaded sphere, Unlit disc, Outline, Wireframe,
  Latitude bands, Plasma, Soft glow, Lensing core, Radiant star), visual size, aspect, stable size variation,
  pulse depth/rate and audio-driven size. The Geometry radius controls the legacy force core; the optional surface field
  follows the rendered size and transient expansion.
- Color: Match particles, Own palette, Solid color or Per-orb colors. Own palette
  has eight palettes including two custom swatches, offset, spread, cycling and
  a pole-to-pole surface gradient. Each of five attractors retains its individual
  swatch even when the count is reduced. Orb saturation is separate.
- Material: brightness, opacity, ambient/directional lighting, specular strength,
  roughness, rim strength/focus and light azimuth/elevation. Light is in camera
  space; analytic Core occlusion optionally hides particles behind the surface.
- Surface: grid/band/plasma density, line/outline width, rotation, spin and tilt.
  Patterns follow world orientation as the camera moves. Fractional density blends
  whole wave counts to avoid a longitude seam. Pattern controls apply to patterned
  styles; unlit, outline and soft glow intentionally bypass material lighting.

New settings preserve the previous shaded, particle-colored appearance by default.
All continuous orb controls support the same macro workflow as particle controls.
Numeric field IDs remain stable when inspector groups change.

## Particle paths, camera and motion

Paths stores world-space positions for up to 512 evenly sampled particle IDs in a
fixed 128-sample GPU ring buffer (1 MiB per component). Sampling is at most 30 times
per simulation second; timestamps make duration/fading correct at slower rates.
Particles → Paths & trails exposes enable, 16–512 tracer count, 0.1–4 second duration, width,
opacity and age fade. Paths are independent of particle/orb opacity and use the
current particle palette. They reproject when the camera moves, including while
paused. Disabling clears path feedback; enabling begins a fresh history. No GPU
history is serialized or read back in the interactive app. Compute and drawing use
separate resource layouts so the writable history never enters a read-only render
pass as a conflicting binding.

Camera has Perspective/Orthographic projection, lens angle, distance, six axis view
presets, centering/reset, free rotation/pan/zoom and auto-orbit. The viewport includes
a clickable XYZ navigator and optional unit grids on XZ, XY, YZ or all three planes.
Y is up. Guides use the same projection equations as GPU particles and paths. They
are translucent overlays, not depth-tested geometry. Legacy inspector indices migrate to their consolidated categories.

Particles → Flow & motion, Flocking, Turbulence and Cloud boundaries offer approximate neighborhood reach, rear awareness,
personal space, turning agility, drag, minimum speed, center pull, plane confinement,
vertical drift, turbulence scale/evolution, boundary return, orbit-axis spread and
simulation speed. The neighborhood remains a bounded 27-cell aggregate search,
not an all-pairs boid simulation. A distant safety enclosure and capped acceleration
keep extreme settings bounded. Arrow glyphs use pixel-space projected velocity;
spin and rotation offsets are ignored in this mode so direction remains meaningful.

Every continuous parameter supplies a longer Info View description with visual
consequences, practical tips, related settings, range/default and macro instructions.

## Performance and saving

The default is 32,768 particles; Low/High/Ultra use 16,384/65,536/131,072.
Retina targets are capped at 1920 pixels on their longest side. Low renders at half
physical resolution; other levels use 75% before the cap. Quality is saved with
component settings. Display target is a local Session preference (60 FPS default)
under Settings → Display & Layout. It is excluded from scene/project state and
independent of FFT analysis cadence. It is a scheduling target, not a guarantee:
GPU load, display refresh, input and other panes still affect measured frame rate.

Simulation integrates elapsed time with a 1/30-second wall-time stall cap, scaled
by Simulation speed (0–2); the shader limits each integration step to 1/15 second. Main-audio RMS
response is smoothed and fades on disconnect; simulation continues without input.
Set Audio force and Audio light to zero to use only manual controls / mapped signals.
Signals are never created automatically. Use the existing workflow: drag an
inspector parameter to a scene macro, then choose a signal and modulation depth.

Projects save configuration and mappings, not GPU buffers, transient trails or
particle positions. Loading or restarting creates a fresh simulation. Legacy
component settings, inspector sections and macro banks migrate by appending Orbital.

## Validation

```sh
cargo test --workspace --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
# Needs a real Metal/Vulkan/DX12 adapter (outside restrictive GPU sandboxes):
cargo test -p symphos-visuals orbital_gpu_validation_and_timing -- --ignored --nocapture
```

The regular suite validates WGSL without a GPU, parameter field roundtrips, macro
addressing, bounds and older-project migration. The explicit GPU test compiles all
pipelines, simulates and renders each quality, checks actual pixels, exercises
paused resize, soaks the default simulation for a minute and checks finite state / cloud volume, and reports synchronized submission + GPU timing. This excludes UI
layout, window presentation, other components and external display overhead.
The development supervisor watches `crates/` including WGSL changes.

## Optional orb surface field

Orbs → Surface field adds an opt-in surface interaction layer without changing
legacy scenes. Surface clearance expands a spherical obstacle around the rendered
orb; visual scale, per-orb size variation, pulse and audio expansion all contribute.
A stretched silhouette uses a conservative surrounding sphere. Extreme obstacle
radii are limited to eight world units. Field reach, repulsion, falloff and signed
surface circulation control a soft force outside that boundary. These five numeric
controls are scene-persisted and macro-addressable.

Solid surface uses a swept segment/sphere test in the moving orb's reference frame
plus penetration correction, preserving tangential motion rather than bouncing.
It can be disabled for a permeable field, or the whole field can be disabled for
original pass-through dynamics. Correction uses up to four bounded passes for
multiple orbs and exits early without contacts. Avoid deeply overlapping large
orbs: this is an artistic steering/avoidance model, not a rigid-body solver. The
surface collision does not control visibility; Core occlusion in Orbs → Lens & depth
independently hides particles behind the visible surface.

Metal tests cover fast particles crossing a complete orb in one step, disabled
and soft-field passage, and clearance around multiple moving/pulsing orbs.

## Lensing cores, corona and transient flares

A second review of the original [Magnetosphere](https://roberthodgin.com/project/magnetosphere)
and [Magnetosphere 2.0](https://roberthodgin.com/project/magnetosphere-2-0) case studies
informed the contrast, luminous rims, fine rays and transient-driven treatment.
These controls are an original stylized implementation, not a reproduction of
Hodgin's shaders or a claim of matching his particle counts or performance.

Rendering → Starting looks offers Lensing swarm and Solar ribbons. They set a
coordinated subset of orb/particle art direction and surface-flow controls while
retaining the camera, quality, paths and macro assignments. Lensing core and
Radiant star are also independent render-style choices in Orbs. Existing projects
retain their saved look; all added effects default to inactive.

Core occlusion analytically intersects the camera ray with each orb ellipsoid to
attenuate particles and paths behind its front surface while preserving foreground
particles. It works in perspective and orthographic views and between orb surfaces.
It is independent of the optional solid-surface motion boundary. The shared
`optics.wgsl` geometry includes visual scale, variation, pulse, audio expansion
and transient expansion, so rendering and the motion boundary agree. Orb appearance
is still an analytic impostor, not a general mesh renderer or shadow system.

The HDR composite adds bounded screen-space lens distortion, optional RGB dispersion,
an outer atmosphere and irregular curved radial rays. Corona intensity, thickness,
ray length/density and drift are independent parameters using each orb's color.
Optical effects are stylized; they do not solve physical gravitational lensing.
Field illumination brightens particles and paths near the surface to emphasize
wrapping currents; its strength also responds to the live burst.
Color dispersion costs extra texture samples only where an active lens bends light.
Visibility loops skip when occlusion is zero; corona/lens work skips when disabled.

Automatic bursts use fresh capture epochs/sequences and sudden RMS increases, with
an adaptive energy baseline, a short refractory interval and configurable sensitivity
and release. They do not estimate tempo. A new capture primes without flashing,
stale analysis frames cannot retrigger, and disconnect fades the envelope. Burst
drive is a separate continuous macro target for prepared Signals, including MIDI
or LFO sources. Bursts brighten orb/rim/corona light and can expand the actual orb.
All 16 added controls save with the scene and support macro assignments.

Tests cover default migration, preset preservation, transient lifecycle, all nine
orb styles, lens/dispersion/burst pixel changes, and explicit foreground/rear-particle
visibility in both camera projections on Metal. Set `SYMPHOS_GPU_PREVIEW_DIR` when
running the ignored GPU test to export optional PPM proof renders.

## Consolidated Orbital inspector

The inspector now has five categories:

- **Particles:** population/quality, size/shape, appearance, color, orientation,
  flow/motion, flocking, turbulence, cloud boundaries, and paths/trails.
- **Orbs:** arrangement/motion, size/shape, appearance, color/palette, lighting,
  surface details, size pulsing, attraction/orbit, surface field, lens/depth,
  and corona/rays.
- **Signal Response:** input smoothing, audio movement/light, audio orb expansion,
  and transient flares including the manual Signal-driven burst envelope.
- **Camera:** basic framing/navigation, advanced lens/orientation/auto-orbit,
  and spatial guides.
- **Rendering:** coordinated starting looks, shared HDR brightness/bloom/exposure,
  and the simulation clock shared by particles and orbs.

Groups reuse the shared inspector disclosure styling and retain their open state
while navigating. Basic groups start open; deeper controls start collapsed.
Palette adjustments appear only for the independent orb palette. Every numeric
parameter is assigned to exactly one group, with stable field IDs, ranges, defaults
and macro addressing. Audio-reactive controls have a single editing location.

Pane snapshots carry `orbital_inspector_version: 1` to distinguish the consolidated
tab indices from the previous eleven-tab layout. Missing/zero versions map old
Geometry/Orbs/Field/Optics/Corona tabs to Orbs, Flow/Appearance/Paths to Particles,
Response to Signal Response, and Camera to Camera. Newly saved indices roundtrip
without a second migration. The version changes only inspector navigation; values,
macro mappings and shader behavior are unchanged.
