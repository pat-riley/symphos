# Workspace layouts and presets

Symphos opens with the default 3D waterfall above three analyzer panes on its
first launch. Later launches restore the last workspace, including its component
choices, settings, dividers, sidebar, and every project scene. Choose **Default view**
in **Scene → Visual preset library…** to apply the original layout and components
to the current scene.

## Component inspector

The component inspector docks on the right and starts open on first launch.
Each pane’s top-right eye button selects that pane and opens its inspector; **I**
opens the selected pane’s inspector, and **N** toggles it. The far-right navbar
button also toggles the inspector and highlights when it is open. The inspector header X
closes it. Selecting a visualization never opens a hidden inspector automatically.

The inspector keeps the component identity and **Driven by** source pinned above
horizontal **Geometry**, **Appearance**, and **Camera** tabs. Flat visualizers omit
Camera. Geometry contains structure, population, motion and simulation controls;
Appearance contains rendering, colors, materials, lighting and feedback. Audio
processing remains in Signals. Each visualizer remembers its selected tab and
scroll position. The tab-row menu contains preset, copy/paste and reset actions.
All 3D cameras offer graphical view buttons and an optional XYZ navigator, plus
axis/custom-axis orbit, back-and-forth sweeps, wobble and auto-pan. Manual camera
navigation stops motion at the visible view; pausing holds it in place.

The pinned header shows the component icon, name, visualization type, panel
number, live/paused status, and capture sample rate. Numeric controls use aligned
label/value columns on a flat panel surface with slim accent indicators.
Drag fields, Shift-drag for fine adjustment, or click the value for exact entry.
The **Appearance** tab contains the shared [gradient and swatch library](component-colors.md).

The inspector surface meets the navbar, right window edge, and bottom window
edge without outer padding. A gap on its left separates it from the plots.
Opening it resizes the visualization area. Drag its left edge to change width, or
double-click that edge to reset. Visibility and width restore with the last
workspace. Info View remains independent at the bottom left.

## Arrange panes

Panes stay tiled inside the application. Drag a divider to resize its neighboring
sections, or double-click it to make that split equal. The layout preserves usable
pane sizes when space permits and adapts its proportions to smaller windows.

Click a pane's header **X** to close it. Its sibling pane or section expands into
the freed space; unrelated split proportions stay intact. Closing every pane
shows **Add a component**. The workspace supports up to 16 open panes.

Drag the dotted grip at the left of a pane header to rearrange it. Drop near
another pane's left, right, top, or bottom edge to split that pane and move the
existing component there. Drop in its center to swap the two pane positions.
Settings and histories travel with the moved component. Escape cancels a drag.

Each header has separate eye and fullscreen buttons. The eye opens the inspector.
Fullscreen fills the app window with that panel and hides the navigation, inspector,
and footer. Escape or the restore icon returns to the previous workspace. A brief
“Press Esc to exit fullscreen” hint fades away after three seconds. F toggles panel
fullscreen; F11 toggles native application fullscreen. Compact panes hide pause
and the drag grip first, keeping fullscreen, inspect, and close accessible.

## Add components

**Add New** in the top-left navigation opens a dropdown of every component, grouped
under Instruments and Visualizers. Selecting one adds a pane beside the selected
pane. New empty workspaces also offer a component browser for dragging components
to pane edges or centers.
The browser docks flush on the left with a compact header and one list of components.
Each single-line row shows an icon, name, and six-dot drag grip, with full-width
dividers. Descriptions remain available on hover; search and category tabs are
currently omitted. Small **Scientific instruments** and **Visualizers** headings
separate measurement components from artistic ones.

Veroni, Strata, and Flux share the [Audio mappings inspector](audio-modulation.md)
for selecting frequency bands, routing them to specific movements or appearance,
and editing graphical attack/decay/sustain/release envelopes.

- Click a component row to add that visualization beside the selected pane.
- Drag a row to a pane edge to add a new split there.
- Drag a row to a pane center to replace its visualization.

An overlay previews the target split or replacement during dragging. Adding and
rearranging components never starts another audio capture device; each open
component uses the existing shared analysis stream. Components hidden temporarily
by pane expansion keep ingesting capture. Closed components release their
histories.

## Veroni visualizer

**Veroni** is under **New component**, or choose **Veroni flow** in the
preset browser for a single-panel view. It uses moving Voronoi sites to create
rounded cells, pale outlines, luminous points, and fine connections inspired by
the reference image. The points define the geometry, so their movement actually
reshapes the cells.

The right inspector has four sections, including editable audio mappings:

| Section | Controls |
| --- | --- |
| Geometry | 12–240 cells, repeatable pattern seed, clustering, corner softness |
| Flow & Motion | Speed, drift, turbulence, swirl, breathing |
| Audio mappings | Frequency bands, movement/appearance destinations, level/gated/transient response, graphical ADSR |
| Appearance | Shared gradients and custom swatches, cell outlines/tint, edge width/opacity/response, point size/response, glow, connections and reach |

Default mappings use bass for cell expansion, mids for swirling flow, and highs
for fine turbulence. Brightness and point size have independent mappings. Open
**Audio mappings** to change any source band, destination, amount, or response.
For a calmer result, reduce Ambient motion and Flow speed, then increase the
selected route's Release. Audio amount zero leaves only ambient flow.
**Flow speed = 0** stops the flow clock while audio can still deform the cells;
use the pane's **Pause** button to hold the entire visual still. Colors include
Electric lime (default), Glacial blue, Ember, and the current theme accent.

Settings save in component presets, full-view presets, and the last workspace.
Animation time, current energy envelopes, and point positions are transient.
Older six-component workspaces and full-view presets acquire default Veroni
settings without changing their existing layout or component settings.

## Strata visualizer

**Strata** is under **New component**, or choose **Strata chamber** in the
preset browser. It turns audio into a suspended sculpture: ivory-to-coral wire
membranes, drifting luminous dust, and slowly tumbling faceted grains inside a
faint volume frame. The reference's layered mesh and granular structure inspire
the design; it is an artistic energy-driven field, not a physical filter model.

Its inspector controls are organized into six sections:

| Section | Controls |
| --- | --- |
| Geometry | 2–12 layers, mesh detail, width, depth, stack height, sag, edge tension |
| Flow & Motion | Motion speed, ripple frequency, turbulence, layer twist |
| Audio mappings | Automatic frequency bands or individual layer targets, bend/ripple/shimmer, brightness, graphical ADSR |
| Particles | Dust count, wire grains, repeatable seed, lift, spread, grain radius, dust size, trails |
| Appearance | Shared gradients and custom swatches (legacy Kiln/Glacier/Pearl/theme presets retained), wire width, mesh/particle opacity, glow, membrane/frame/hint toggles |
| Camera | Rotation, elevation, zoom, auto orbit, orbit speed |

The default layer mappings distribute 20 Hz–20 kHz logarithmically from bass at
the bottom to treble at the top. Changing the count automatically redistributes
the bands. In **Audio mappings**, choose All layers to subdivide a custom range,
or a numbered layer to target just that membrane. Each layer has its own envelope
and motion clock. Bend, ripple, and shimmer are separate destinations. The
spectrum editor shows the current subdivisions; full-mix and stereo sources
can also drive the layers together.

A separate brightness mapping illuminates the sculpture. Mesh highlights move
toward the palette's warm color as the layer's deformation and energy rise.
Each layer stays within its own vertical space, even at maximum response.
Particles fade before wrapping at the top or bottom of the volume.

Drag inside the scene to orbit (this stops auto orbit at its current angle).
Scroll to zoom; double-click to reset the view. **Pause** freezes the wave clock,
particles, and automatic camera rotation together. Manual camera controls remain
available while paused. Auto orbit runs independently of **Motion speed**; set
**Audio amount** to zero for ambient motion alone. Presets contain settings only;
animation clocks and captured audio are never serialized.

For a quieter composition, reduce Dust count and Wire grains, lower Mesh opacity,
and use a long Release. For a dense architectural look, increase Mesh detail;
more layers and finer grids cost more to render. The default seven layers and
24 grid divisions keep the structure readable without filling every gap.

Strata uses the existing analysis stream and native renderer on Mac and Linux.
It adds no audio devices or platform dependencies. Existing six- and seven-module
workspaces and full-view presets acquire its default settings automatically,
including those containing customized Veroni components.

## Flux particle cloud

**Flux** is under **New component**, or select **Flux currents** from the
preset browser. It creates a 3D cloud of thousands of colored points, drawn into
a dispersed, moving volume around attraction points. It starts at 6,000 particles
and supports 200–20,000. The reference's multicolored filaments and luminous cores
inspire the cloud; forces, damping, and turbulence animate individual particles.

The inspector includes six sections:

| Section | Controls |
| --- | --- |
| Geometry | Particle count, 1–8 attraction points, seed, cloud spread, depth stretch |
| Flow & Motion | Simulation speed, orbit magnetism, attraction, swirl, turbulence, damping, diffusion, core movement, vertical pull |
| Audio mappings | Frequency bands, force/brightness/size/steering destinations, level/gated/transient response, graphical ADSR |
| Particles | Point size, size variation, opacity, trails, glow, sparkle, core visibility and size |
| Appearance | Shared gradients and custom swatches (legacy Spectrum/Solar/Ion/theme presets retained), color by current/velocity/audio band/depth, saturation, depth fade, navigation hints |
| Camera | Rotation, elevation, roll, zoom, perspective, auto orbit, orbit speed |

Flux's [performance bar](flux-performance.md) docks below the selected pane's
workspace: four faders and eight knobs cover pacing, expansion, musical accents,
orbital shape, and appearance. Press **P** to show/hide it; in panel fullscreen,
P brings the controls back. **Inspect** reveals the detailed right-hand controls
for the last-touched macro. The formation menu provides Cloud, Rings, Helix,
Knot, Vortex, and Petals; **Burst** launches a manual accent.

**Orbit magnetism** in **Flow & Motion** blends between a cloud and orbital bands.
It defaults to **0**: every particle follows its own moving path through a 3D
volume, so the initial cloud does not settle into thin rings. Raise it toward
**1** to gather particles onto shared orbital bands; intermediate values retain
thicker, looser formations. Lowering it disperses the same particles again,
without reseeding or teleporting. Existing presets without this setting also
start at zero.

Audio expansion increases the extent of both formations. Swirl changes their
circulation, and turbulence disturbs their paths; a tightly magnetized ring keeps
a smaller amount of turbulence so its band remains readable. **Attraction** sets
how closely particles follow those paths. **Auto orbit** remains a separate
camera setting.

Default mappings send bass to expansion, mids to swirl, highs to turbulence,
and stereo balance to horizontal steering. Brightness, particle size, and motion
speed are separate destinations. Change the bands and envelopes in **Audio
mappings**; add a Transient ADSR mapping for onset-driven bursts. Level follower
smooths continuous energy with attack/release. Audio amount zero leaves only
ambient movement; camera orbit remains independently controlled.

Drag to orbit; **Shift-drag** to roll; scroll to zoom; double-click to reset the
view. Manual dragging stops auto orbit at the current angle. Perspective zero
uses orthographic projection. **Pause** freezes the particles, response, and
automatic camera together, while still allowing manual rotation of the frozen
cloud. Simulation speed zero stops particles but leaves automatic camera motion
and audio-driven brightness active.

For orbital streams, raise Orbit magnetism, then increase Swirl and Trails and add some
amount on the Swirling flow mapping. For a soft nebula, lower Attraction, raise Diffusion, and increase
Depth fade. Lower particle count and Glow when using several visualizer panes.
Changing the seed or attraction-point count reseeds the cloud; adding particles
preserves those already present. Presets save settings only, not particle
positions, velocities, animation time, or captured audio.

Simulation uses bounded 120 Hz steps and limits catch-up after hidden/stalled
frames. Forces and velocities are constrained to keep even extreme settings
finite. Rendering sorts particles by depth and batches them into a native mesh.
The same shared analysis stream feeds Flux on Mac and Linux, with no new audio
capture or platform dependencies. Existing workspaces and full-view presets
from the six-, seven-, and eight-component versions retain their settings and
receive default Flux settings.

## Scenes and reusable presets

The top bar selects complete project scenes in both Signals and Visualize. Each
scene owns its signal channels, component configurations, layout, mappings, and
macros. Previous/next recalls scenes immediately; the + button duplicates the
current setup. The scene menu also offers a new empty scene, rename, move
earlier/later, and delete. Switching scenes retains edits, and File → Save Project
saves every scene. See [projects and scenes](signal-sources.md#projects-and-presets).

Use **Scene → Visual preset library…** for existing full-view, layout, and component
presets. **Component presets…** in the inspector opens that library in the component
category. Within the library, click the preset name to search by category; layout
thumbnails preview the arrangement. An asterisk marks edits to the loaded preset.

Click **Save**, enter a name, and choose a preset type:

| Type | Saved content | Applying it |
| --- | --- | --- |
| Layout only | Pane count, split arrangement, and divider proportions | Keeps existing component choices and settings in visual order |
| Full view | Layout plus component choices and their settings | Restores the saved component configuration |
| Component | Settings for the selected visualization | Applies to a selected pane of the same type without changing the layout |

Layout-only presets have no module configuration in their saved data. If the new
layout needs fewer panes, extra components become hidden slots with their
settings retained. A later larger layout reuses those slots. If more slots are
needed, Symphos adds default frequency-spectrum components. Adding a fresh
component from the library may reuse a hidden slot with fresh settings.

Full views and component presets leave shared FFT settings, audio source,
capture permissions, and routing unchanged. They do not contain audio samples,
captured histories, frozen frames, or textures.

User presets have a **···** button in the browser for rename and delete. Factory
presets remain available. Deleting a user preset leaves the current workspace
intact. Names must be unique within their category.

## Saved files

The regular app saves `workspace.json` and `presets.json` under:

- macOS: `~/Library/Application Support/Symphos/`
- Linux: `$XDG_CONFIG_HOME/symphos/`, or `~/.config/symphos/` when unset

The live reload app uses `target/dev/` instead, so its workspace and preset
library are separate. Settings are checkpointed during use and on graceful exit;
the last workspace also survives rebuilds. `cargo clean` removes dev state.

Workspace schema version 2 migrates the earlier four-pane development workspace.
The user preset library has its own versioned schema. Invalid layouts, duplicate
pane IDs, oversized collections, and unsafe allocation settings are rejected.
Rejected files are preserved beside the originals as `workspace.rejected.json`
or `presets.rejected.json` before falling back to defaults.

## Validation

Tests cover split collapse, move/swap operations, nonoverlapping geometry, legacy
workspace migration, settings-only serialization, component preservation during
layout changes, preset-library recovery, and last-session restoration. Egui
pointer-interaction tests exercise library drag/add, center replacement, pane
movement, and closure through the empty-workspace state.

The Mac scratch app was checked for fresh-launch defaults, component addition,
close-and-fill, saving a layout-only preset, preset navigation, and restoration
after replacing its executable. Mac and Ubuntu 24.04 Linux checks passed formatting, Clippy, normal/development
tests, and locked release builds. The Mac bundle also passed local signature
verification. Real Omarchy graphics and device validation remains a separate hardware
check; the PipeWire and Core Audio capture implementations are unchanged by this
workspace feature.
