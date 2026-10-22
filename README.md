<p align="center">
  <img src="assets/io.github.riley.Symphos.svg" width="112" alt="Symphos icon">
</p>

# Symphos

Symphos is a native, GPU-rendered audio analyzer and visualizer, developed for Omarchy
and now available as a local macOS developer build. On Linux it captures microphones
and system output through PipeWire; on macOS it captures system playback through
Core Audio taps. It displays the signal
in real time, and is being built toward a visual scene editor where audio
signals can control shapes, particles, effects, and shaders.

Symphos is currently a proof of concept with scientific analyzers, artistic
visualizers, editable audio mappings, and performance controls. Named musical
signal sources feed visual parameters, and projects organize complete setups into
scenes for quick recall of signals and visuals together.

## Current features

- Tiled workspace with resizable dividers, draggable pane headers, close-and-fill
  behavior, and a compact component library. The first launch uses the default
  waterfall-on-top view; later launches restore the last workspace.
- Shared top scene navigation with a full-screen **Library** for project scenes
  and reusable scene presets. Select, rename, duplicate, delete, search, and drag
  scenes into performance order. The Save icon captures a named preset with its
  signals, mappings, visuals, macros, and a thumbnail. File → Save Project keeps
  the entire scene collection.

- Spectrogram/waterfall history continues while panes are hidden or the app window
  is occluded. Returning views catch up from the latest 30 seconds of independently
  captured frequency history. Only explicit pane Pause stops that pane's history;
  this is a rolling analyzer buffer, not an audio recording feature.
- Copy/Paste settings in the module sidebar (Ctrl+Shift+C / Ctrl+Shift+V) applies
  a settings-only snapshot to a matching module. Audio/history, pause state, and
  global FFT and footer-meter choices stay outside the destination pane. The
  clipboard is session-only; reusable component presets are available in the
  inspector’s **Component presets…** menu.
- [Application Settings](docs/application-settings.md): movable, categorized window
  with eight GUI theme choices, including Resolve- and Ableton-inspired skins,
  and saved interface zoom. Open from the application menu or Ctrl+,
  (Cmd+, on Mac); drag its title bar to move, and press Escape to close.
- **Settings → Help & About → Keyboard shortcuts** opens the shortcut reference modal (Ctrl+K).
  Space pauses the selected pane; Shift+Space pauses all/resumes all; N toggles
  the sidebar; I opens the selected panel’s inspector; F toggles panel fullscreen; 1–4 select panes; F1 toggles Info View;
  F11 toggles fullscreen; Escape exits panel fullscreen or dismisses a modal/menu.
  Typing, key-repeat events, and open menus/modals do not trigger application actions.
- [Compact component inspector and color library](docs/component-colors.md) with
  icon tabs, aligned parameter rows, twelve gradient previews, editable color
  stops, and saved palettes/swatches shared across components.
- Numeric module controls: drag the filled field to adjust, Shift-drag for fine
  adjustment, double-click beside its number to reset, or click the number to type
  an exact value (including scientific notation). Focused fields also accept
  left/right arrows to adjust and Enter to start typing.
  Slider ranges are comfortable working ranges; selected geometry/appearance fields
  accept larger custom values within documented safe limits. Buffer sizes, history,
  percentages, and algorithm limits remain bounded. Rejected values retain the last
  valid setting and appear in **Settings → Help & About → Issue log**, with a copyable report.
  Diagnostics also append to `$XDG_STATE_HOME/symphos/issues.log` (fallback
  `~/.local/state/symphos/issues.log`, 1 MiB plus one rotated previous log).
  Reports include typed-value changes, rejected values, capture errors, panic
  messages, and a preview of the selected module/current capture configuration.
  Review before sharing; no automatic uploads.
- Audio source selection in Settings → Audio, defaulting to system-output speakers
- Resizable panes with interchangeable waterfall, sphere, cylinder, spectrum, waveform,
  spectrogram, stereometer, oscilloscope, Veroni, Strata, and Flux components
- [Flux particle cloud](docs/workspace-layouts.md#flux-particle-cloud) with
  music-driven 3D currents, 200–20,000 particles, trails, and 42 controls including
  orbit, roll, perspective, and color mapping
- [Performance macros](docs/performance-modulation.md) for each scene with twelve assignable rotary controls across components,
  musical Impact bursts, and morphing Rings/Helix/Knot/Vortex/Petals formations
- [Shared visualizer audio mappings](docs/audio-modulation.md) with draggable
  frequency bands, independent movement/appearance destinations, graphical ADSR,
  ambient motion controls, and editable per-layer Strata response
- [Strata layered sculpture](docs/workspace-layouts.md#strata-visualizer) with
  rippling wire membranes, drifting dust, tumbling grains, an orbitable camera,
  and 39 controls for structure, motion, audio response, and appearance
- [Veroni cellular visualizer](docs/workspace-layouts.md#veroni-visualizer) with
  audio-reactive Voronoi cells, flowing lime points, and 30 controls for geometry,
  movement, audio response, and appearance
- A live 3D waterfall with Surface, Lines, Y Lines, Wireframe, Dots, Stems, and Bars render styles,
  fully hideable guides, rotation, zoom, and 0.1–30 second history (2 seconds by default)
- Independent waterfall X/Y lengths (0.25×–10×, default 1× each) without changing frequency range or history duration
- Independent frequency range, linear/log scale, time/frequency smoothing, gain, decay, and
  intensity controls for each frequency module
- Optional musical-note axis labels and adjustable A4 tuning reference
- Mode-specific appearance controls, heatmap contrast, eight isometric views, and camera auto-orbit
- Spectrum peak hold and waveform controls for independent L/R/Mid/Side lanes,
  scroll or loop motion, speed, static/multi-band/color-map rendering, multi-band
  peak history, triggering, time/amplitude, and color palettes
- Right-hand component inspector with a panel eye button, icon tabs, and collapsible groups
- Interactive camera orientation gizmos in the sidebar and waterfall viewport
- Toggleable bottom-left Info View with hover descriptions instead of popup tooltips
- Shared FFT size, window function, analysis channel, and frame-rate controls
- Bottom navigation switches Signals/Macros docks, with live levels, FPS, and sample rate
- Raw FFT-bin inspection, detailed diagnostics, and fullscreen mode
- GPU rendering with `wgpu` on native Wayland
- Colors derived from the active Omarchy theme

## Menus and common commands

On macOS, the menu bar exposes **File, Edit, Create, Scene, Playback, View,
Window, and Help**. Other desktops expose the same app commands under **Menu**.

- **Symphos → Settings…** opens preferences (Cmd+,). The **Preferences** submenu
  jumps directly to Audio, MIDI, Analysis & Meters, Display & Layout, or Theme & Colors.
- **File** creates, opens, and saves complete projects, opens the scene
  library, and closes the selected component. **Edit** provides text clipboard
  commands, compatible component settings copy/paste, and component reset.
- **Create** adds any component or detector/LFO/MIDI signal channel, creates an
  empty scene (Cmd+Shift+N), or duplicates the current scene (Cmd+Shift+D).
  Creating components never creates signal channels automatically.
- **Scene** navigates previous/next scenes (Cmd+Option+Left/Right), renames the
  current scene, or opens the existing delete confirmation.
- **Playback** pauses one/all components, clears captured history, and resets
  peak/clip hold. **View** toggles the component browser (Cmd+Shift+A), inspector,
  Macros (Cmd+B), Signals (Cmd+Shift+B), and Info View; balances pane sizes;
  controls fullscreen; and changes interface zoom (Cmd+= / Cmd+- / Cmd+0).
- **Window** uses the native Minimize, Zoom, and Bring All to Front commands.
  **Help** opens the shortcut reference, FFT diagnostics, issue log, and app information.

Checkmarks follow the live workspace. Unavailable actions are disabled, including
incompatible component paste and workspace changes while editing or in a modal.
Shortcuts above use Ctrl on other desktops (Alt in place of Option).
Device and interface preferences remain local; scene edits are saved in projects.

## Using the dashboard

Symphos automatically selects system-output speakers on startup (or another
system output if speakers are unavailable; it never automatically selects a
microphone). The compact selector at the top right lets you choose any source;
the picker stays a fixed width, while full names wrap inside a bounded, scrollable
dropdown. Choosing the current source does not restart capture. The list retains
source types and updates automatically as
devices are added or removed, without a refresh button. Manual choices are kept for
the session and are not replaced when other devices appear.
Click a pane to edit its settings.
Use the dropdown in any pane header to choose its module. Drag any divider to
resize its neighboring sections; double-click a divider to equalize that split.
Close a pane with its header X and its neighbor fills the space. Use
**New component** to search or browse visualizations. Click to add beside the
selected pane, or drag to an edge to split and to the center to replace. Drag a
pane's dotted header grip to move it, or onto another pane's center to swap them.
See [workspace layouts and presets](docs/workspace-layouts.md) for preset behavior.

The eye icon in each pane header selects that panel and opens its inspector on the
right. All parameters appear in one scrollable panel, grouped under collapsible
section headers. A separate fullscreen icon beside the eye fills the app window
with that panel. Escape or the restore icon returns to your workspace; a brief
exit hint fades away. F11 toggles native application fullscreen.

Drag the waterfall to orbit fully around either axis, including over and under
the grid. Right-drag, middle-drag, or Shift + left-drag pans the view. Rotation
keeps the view's scale stable. Scroll to zoom (0.5×–10×, also adjustable under
Camera) and double-click to reset the
camera, including pan. **Center view** under Camera resets only the pan.
The Camera panel has an XYZ orientation gizmo. An optional compact version in the
waterfall's upper-right corner is hidden by default; enable **Navigation gizmo**
under Appearance → Viewport guides to show it. Drag either gizmo to orbit, click an axis to
align, and click it again to flip to the opposite side. Explicit Front, Back,
Left, Right, Top, and Bottom buttons always snap to the named orthographic view.
**Snap isometric…** offers four corner views from above and four from below,
with equal foreshortening along all three axes. View presets preserve pan and zoom.
**Auto-orbit** rotates around the level axis at 1–30 degrees/second (default 10),
with an optional reverse direction. Manual camera adjustments, presets, and reset
stop auto-orbit. Its speed is independent of history duration and analysis rate.
X represents frequency, Y time, and Z level; angle fields use degrees.
The gizmos use colored lines and plain axis labels without endpoint circles;
axis clicking and dragging still work.
History is live only; there is no recording or playback.
Waterfall frequency, gain, smoothing, and decay controls reprocess the retained
history immediately. Under **Signal Response**, **Time smooth** softens changes
between frames, while **Freq. smooth** blends neighboring display bands (0 is off,
1–8 progressively softens jagged peaks). It does not change the FFT or audio.
Under **Frequency Range**, **Note labels** replaces Hz ticks with the nearest
equal-tempered note. **A4 Hz** changes the tuning reference (440 Hz by default).
These are axis references, not detected notes or key detection. Both controls also
work independently in the spectrum and spectrogram.
Height stretches the surface above its fixed floor.
The inspector runs flush from the navigation bar to the bottom and right edges of
the app, with a gap only beside the plots. Its header X closes it; any pane’s eye
button reopens it. Clicking a visualization updates selection without opening a
hidden inspector. Opening it resizes the visualization area.
Under **Geometry → Time & detail**, **History s** sets the waterfall's displayed time window.
Under **Geometry**, **Length X** stretches the frequency axis and **Length Y**
stretches the time axis independently. Both default to 1× with a 0.25×–10× range.
The floor grid and labels follow both lengths, and framing accommodates the geometry.
Under **Appearance** (the paintbrush tab), the compact icon-labelled **Render style**
dropdown offers Surface, Lines, Y Lines, Wireframe, Dots, Stems, and Bars. All seven
options fit without scrolling whenever screen space allows; scrolling remains
available when the viewport is too short. Lines draws traces
across frequency; Y Lines draws traces along time, one per frequency band.
Wireframe connects both directions without filling the faces. Dots shows separate
colored points for each frequency/time sample with no connecting edges. Stems
draws a forest of colored pins from the floor to each signal level. Bars draws
solid 3D columns, with shaded sides to make their volume readable from any angle.
**Base floor** fills the underside of Surface mode at zero height, independently
of **Base walls** and the reference **Floor grid**. It defaults off and is saved
with the component. Missing history stays open instead of being bridged.
Only the selected style's parameters are shown directly below the dropdown:
surface mesh/base walls/base floor; line thickness and time-trace spacing; Y-line thickness
and band spacing; wireframe thickness and mesh spacing; dot size and point spacing;
stem thickness and spacing; or bar width/depth, bands per bar, and time spacing.
Bars defaults to 75% width, 65% depth, four display bands per bar, and every third
time slice. Width/depth adjust the gaps between columns, while **Bands/bar**
uses the peak within each frequency group so narrow peaks are retained.
Bar height still uses Geometry's Height and the module's dB range. Spacing
means every Nth trace/sample, not audio
sampling rate. Each style remembers its own settings during the session.
Geometry groups Length X, Length Y, and Height under Dimensions, and History s
and Time slices under Time & detail.
Neither length nor mode changes clear the audio history.
**Viewport guides → Show guides** hides every reference overlay, including the
surface mesh, floor grid, axes, labels, navigation text, and viewport gizmo.
Individual switches control the floor grid, axes/labels, and navigation gizmo;
turning guides back on restores those preferences. Sidebar camera controls
remain available even with all guides hidden.
Under **Appearance**, choose **Heatmap** for a full blue → cyan → green →
yellow → orange → red gradient. Cooler colors represent quieter levels and
warmer colors louder peaks, using the same signal level as waterfall height.
**Floor dB** and **Ceiling dB** set the mapped range. Heatmap's **Contrast** control
is color-only: 1 is neutral, higher values separate cool and warm levels more,
and lower values bring colors toward the middle without changing height.
Heatmap works in every
waterfall geometry mode and is also available in the spectrogram.
Click **? Help** at the far bottom left to show or hide **Info View** at the bottom
left. Hover over a control or visualization for a short description. Popup
tooltips are disabled even when Info View is hidden; existing descriptions are
retained in the shared help system. A capture-status indicator sits beside the
bottom-right meters; detailed status is in Settings → Audio and Analysis & Meters. When settings
are open, Info View docks underneath them; otherwise it
reserves space below the dashboard. It never covers a visualization.
The waterfall's icon tabs are ordered **Geometry**, **Frequency Range**,
**Signal Response**, **Appearance**, then **Camera** at the bottom. Geometry is
the default tab and includes history duration, so the waterfall no longer needs
a separate Time & History tab. Other modules show only relevant tabs; the
spectrogram retains its Time & History tab. One section is visible at a time; the selected tab
and scroll position are remembered separately for each pane/module/tab during the session. Waveform
has Channels, Time Window, Amplitude, and Appearance tabs; its channel layout is
independent for every pane. Stereometer has Display, Appearance, and Correlation
tabs. Oscilloscope has Channels, Synchronization, Time Window, Amplitude, and
Appearance tabs.

Each pane has a **pause/resume** icon beside expand/restore. Pausing freezes that
pane's captured frame and history clock while capture and other panes continue.
Settings remain editable; paused time is omitted when the pane resumes. Changing
source, FFT format, or the pane's module clears the frozen capture. **Reset module
settings** restores only the selected module in that pane, preserving retained
history and leaving global settings and other panes alone.

- **Frequency spectrum:** Bars, Line, and Filled area styles with contextual bar
  gap, thickness, and opacity controls. Signal Response includes peak hold with
  a timed hold, independent dB/s falloff, indefinite hold, and peak reset. Appearance
  adds palettes, heatmap contrast, and independent grid/axis-label switches.
  Capture a live spectrum as a comparison overlay; frozen panes allow a frequency,
  note, and dBFS measurement to be pinned.
- **Waveform:** One or two independently selected Left, Right, Mid, or Side lanes.
  Adjustable 10–500 ms travel spans are independent of FFT size, with full-rate
  capture and peak-preserving display reduction. Optional rising/falling threshold
  trigger aligns repeating signals from the first lane, falling back to the latest
  window when no crossing exists. Scroll mode keeps new audio at the right edge;
  Loop mode advances a write head over a static trace. Static, low/mid/high
  multi-band, and level color-map modes are available, with an optional multi-band
  peak-history overlay. Auto scale uses a common, bounded gain for both lanes;
  manual amplitude remains available. Line/Filled area styles have
  thickness/opacity controls, plus separate centerline, grid, and label switches.
  Trigger, auto scale, peak history, and Loop default off; the travel span defaults
  to 250 ms.
- **Spectrogram:** 0.1–30 seconds of visible history (8 seconds by default), newest
  at right or bottom, smooth or pixelated rendering, 64–1024 time cells and 24–96
  frequency cells. Lower frequency display resolution preserves the loudest band
  in each cell. Display detail does not increase FFT resolution or the 30 Hz history
  capture rate. Palettes, contrast, levels, grid, and labels are independently
  adjustable without clearing history. Frozen panes allow a frequency, note,
  dBFS, and time measurement to be pinned in either orientation.
- **Stereometer:** Scaled, one-to-one rotated Linear, and unrotated Lissajous
  views of the recent full-rate stereo signal. Static color, frequency-balanced
  RGB dots, and separate red/green/blue low/mid/high overlays are available.
  The side meter shows overall phase correlation alone or alongside the three
  bands, from −1 through 0 to +1. Persistence, dot size/intensity, guides,
  labels, and the Left/Right RMS balance indicator are independent per pane.
  The display-only crossovers are 200 Hz and 2 kHz and do not alter capture or
  shared FFT analysis.
- **Oscilloscope:** A short, stabilized view for examining the repeating shape
  of Left, Right, Mid, or Side. Follow pitch estimates a local 30 Hz–2 kHz
  period and supports one or multiple detected cycles; Threshold trigger aligns
  a manual timebase to a selected level and slope; Free run shows the newest
  manual window. Auto/manual amplitude, static or frequency-balanced RGB color,
  separate low/mid/high overlays, configurable afterglow, centerline, grid, and
  status labels are independent per pane. Pitch detection and band splitting
  consume only the existing full-rate sample snapshot and do not alter capture
  or shared FFT analysis.

The bottom bar toggles **Signals**, **Macros**, and **Info View**. Signals and
Macros occupy one dock at a time, leaving the visualizations visible above.
**Settings → Analysis & Meters** contains FFT size, update rate, window,
frequency-analysis channel, stereo/single-meter mode, and single-meter channel.
These meter choices do not change waveform panes or captured history.
Level bars show RMS with live and retained peak markers. Left, Right, and Mix
maxima are held independently, clipping latches visibly, and both can be reset
from Settings → Analysis & Meters or by restarting capture. Mixed levels are measured from the
summed signal, including phase cancellation. Waveform time
windows are independent of the FFT capture window. The borderless plus button at the top left toggles the component browser;
the outline icon at the top right toggles the inspector. Audio source selection is in **Settings → Audio**.

**Symphos → Settings…** (Cmd+, on macOS; File → Settings… on other desktops) opens
pane-size balancing, fullscreen (F11), and the FFT inspector with
the detailed analyzer metrics and experimental tempo estimate. Pane assignments,
sizes, module settings, and Info View visibility restore automatically at the
next launch. Capture history and paused frames are not saved.

## Linux requirements

Symphos supports Linux desktops using PipeWire. Development is focused
on Omarchy and tested on ARM64 Apple Silicon with the Asahi graphics stack.

You need:

- Rust 1.95 or newer
- PipeWire 1.0 or newer and its development headers
- Wayland and XKB development headers
- A Vulkan, OpenGL, or other `wgpu`-supported graphics driver

On Arch Linux or Omarchy, install the native dependencies with:

```sh
omarchy pkg add base-devel pipewire libxkbcommon wayland
```

Install Rust with your distribution package or the official `rustup` installer.

## Build and run on Linux

Install Rust, a C++17 compiler, CMake 3.21+, and the PipeWire/EGL development
headers before building. MilkDrop’s pinned native dependencies are vendored.

```sh
git clone https://github.com/pat-riley/symphos.git
cd symphos
cargo run --release
```

The application needs access to your desktop session's PipeWire socket and GPU
render device. Run it from a normal graphical terminal.

To build and add Symphos to the Omarchy application menu for the current user:

```sh
./scripts/install-dev.sh
```

This checks formatting, runs lint and tests, builds the locked release, and
installs the verified binary and desktop metadata under `~/.local`. Run this
command after local UI changes to update the launcher, then close and reopen
Symphos. A running instance does not update itself.

Settings → Help & About shows the app version and a source build identifier,
with a **Copy build info** button. The identifier includes uncommitted source
changes, so it distinguishes local builds even before a commit. Compare it with
`symphos --version` when reporting issues or checking what the launcher runs.

The right-hand component inspector starts open on first launch and remembers
your last visibility choice afterward. Use a panel’s eye button or **I** to open it,
**N** to toggle it, and the header X to close it. Each parameter section has a
collapsible header. Drag the inspector's left edge to resize, or double-click to
restore its compact width. Width and visibility restore with the workspace;
disclosure states and each pane’s scroll position remain independent during the session.

Remove the installation with:

```sh
./scripts/uninstall-dev.sh
```

## Build and run on macOS

The initial Mac backend visualizes system playback on macOS 14.2 or newer,
without a virtual audio driver. Apple Silicon has been tested locally; Intel,
additional input devices, and public distribution are still pending validation.
Install Rust 1.95 or newer, Xcode command line tools, and CMake (`brew install cmake`), then run:

```sh
./scripts/build-macos.sh
open target/macos/Symphos.app
```

Allow system audio recording when macOS requests it. If capture needs to be
restarted, use **Settings → Audio → Retry audio capture**. Microphone/interface input
selection and DAW plugins are not part of this first Mac milestone. The build
is locally ad-hoc signed; public downloads will require release signing and
notarization. See [Mac setup and validation](docs/macos-development.md).

## Live reload development

Run `./scripts/dev.sh` for a separate **Symphos Dev** app with rebuild-on-save,
restored development settings, generated audio, and live theme editing. See [the rapid development guide](docs/realtime-development.md).

## Development checks

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo build --release
```

## Architecture and roadmap

The audio callback, signal analysis, and UI/GPU work run independently so a
slow visual frame cannot block real-time capture. See
[the architecture notes](docs/architecture.md) for the thread and data model.

The [project roadmap](docs/roadmap.md) describes the path from the current
analyzer to editable scenes, audio-to-visual mappings, effects, shaders, and
Arch packaging. The [analyzer feature backlog](docs/analyzer-feature-backlog.md)
tracks metering, capture, DAW, workspace, and platform gaps in more detail.

The [desktop and plugin plan](docs/cross-platform-plan.md) proposes macOS system
audio capture, preservation of Omarchy support, VST3/AU investigation, and later
Windows support. The first Mac system-audio developer build is implemented; plugins, Windows,
and public downloadable releases remain planned.
The [landing page brief](docs/landing-page-brief.md) outlines the separate website.

## Contributing

Symphos is early software and its scene model is still taking shape. Bug
reports, performance measurements, analyzer validation, and focused pull
requests are welcome. Read [CONTRIBUTING.md](CONTRIBUTING.md) before starting a
larger change.

## License

Symphos is available under the [MIT License](LICENSE).

### Signals and portable projects

Use **Signals** to prepare reusable intensity, band-energy, hit, spectral-change
and stereo controls in a resizable bottom dock while visuals remain visible.
Use **Signals** or **Macros** in the bottom toolbar to switch docks; only one is
shown at a time. Drag the Signals dock’s top edge to resize it. Clicking a channel
shows its right inspector; clicking a visual pane returns to component settings.
The top-left plus button opens the component browser. Click an instrument or
visualizer to add it, or drag one into the workspace. Drag the browser’s right edge
to resize it; double-click that edge to restore its compact width. The width is
remembered locally across restarts. FFT size, analysis
rate and mono/stereo meters live in **Settings → Analysis & Meters**. **File → Save Project** saves signals,
mappings, layouts and component settings for every scene together. The scene
selector remains in the top bar, and switching retains edits. Use **Duplicate** to
copy a setup, or **New scene** to start fresh. **Library**
opens a full-screen manager: select cards without changing the playing scene,
then use **Load scene** or double-click to recall one and close Library. Switch
between thumbnail grid and compact list views; the choice is remembered locally.
Right-click an item or use **More** for rename, duplicate, delete and ordering
commands. New and duplicated cards stay unloaded
until you choose them. Drag cards (with search cleared), or use Move earlier / later,
to change order. The toolbar Save icon opens a naming dialog for a reusable scene
preset. Loading one adds a scene to the project and keeps existing scenes intact.
Scene presets include all channels, mappings, macros, components, and analysis
settings; device bindings and GUI preferences stay local. Thumbnails capture the
active workspace when opening Library or saving; **Refresh preview** updates the
active scene. Older scenes show a placeholder until captured. The Scene presets
section also links to the existing visual/component preset collection.
File → New Project starts a fresh scene collection; older projects become one
initial scene. GUI and hardware preferences stay local. On macOS, **Settings → MIDI** enables
hardware and virtual MIDI inputs. MIDI signal channels support Learn, CC, note
velocity/gates, pitch bend, and aftertouch; DAW tracks can send to **Symphos Input**.
OSC, direct Ableton integration, and file inputs remain planned.
See [the Signals guide](docs/signal-sources.md#midi-inputs-macos).

### 3D Sphere

Add **3D Sphere** from the component library, or choose the **Spherical spectrum**
view preset. It wraps the same processed FFT history as 3D Waterfall onto a closed
sphere: frequency runs around the circumference, history flows from north to
south, and level pushes the surface outward. A mirrored frequency wrap closes
the seam; pole taper keeps both poles smooth. Reverse flow changes the direction.

The inspector includes shared frequency range, logarithmic/linear spacing, gain,
smoothing, decay, and dB limits, plus radius, displacement, three-axis stretch,
twist, longitude offset, spectrum repeats, history duration, and mesh resolution.
Choose wireframe, surface, rings, meridians, points, or radial spikes. Color by
frequency, level, or history with the shared palettes; adjust brightness, contrast,
opacity, rear visibility, and the resting grid independently.

Drag to orbit, Shift/right-drag to pan, scroll to zoom, and double-click to reset.
The Camera tab adds roll, axis views, and reversible auto-orbit. Numeric parameter
labels can be dragged onto the twelve empty macro slots, including scientific
processing controls. Project and view presets preserve settings and macro banks;
older projects gain a default Sphere bank without altering existing components.

Sphere’s **Geometry → Surface motion** picker also offers **Pole ripples**. Audio
energy and rising hits launch circular crests and troughs that travel from the top
pole toward the bottom pole. Adjust Travel speed, Ripple rate Hz, Damping, and
Hit emphasis; Displacement controls their depth. Reverse history flow starts
from the opposite pole. **Launch ripple** sends a manual wave, including in
silence. The numeric ripple controls can be assigned to macros.

Sphere rotation uses a free trackball: drag near the center to tilt, or around
the silhouette to roll. The pivot follows the sphere when panned, and rotation
remains free at the poles. The orientation gizmo reflects roll as well as yaw
and elevation. Shift/right-drag still pans; double-click restores the camera.


### 3D Cylinder

Add **3D Cylinder** from the component library or use the **Cylindrical spectrum**
view preset. Frequency wraps around the circumference, history travels down the
height, and audio level displaces the radius. **Geometry → Surface motion → Axial
ripples** sends audio-driven rings down the cylinder instead. Reverse flow starts
from the bottom; Launch ripple sends a manual wave even in silence.

Cylinder shares Sphere's frequency range and response processing, six rendering
styles, palettes, lighting, history resolution, twist, repeats, ripple settings,
free trackball rotation, pan/zoom, view presets, and auto-orbit. Its shape controls
add independent height, top/bottom radius multipliers, displacement falloff near
the rims, and optional end caps. Surface and wireframe caps follow the displaced
rims, so the cylinder can become a tapered column or an open tube. In Geometry,
uncheck **Show end caps** to remove the end faces and radial grid lines. Set
**Top radius ×** or **Bottom radius ×** to zero to taper that end to a true point;
audio displacement fades with the taper so the point stays closed.

Numeric inspector labels can be dragged onto its twelve initially empty macro
slots. Projects and presets preserve Cylinder settings and macro assignments;
older projects keep their existing components and receive a blank Cylinder bank.
No signal channels are created when adding or switching to Cylinder.

### 3D Rings

Add **3D Rings** from the component library or choose **Orbital waveforms** in the
view presets. Live time-domain audio bends closed lines into moving orbital
rings. Choose tilted orbital planes, concentric rings, or a vertical stack, with
one to twelve rings, adjustable spacing, inclination, spread, and ellipticity.

The waveform controls share Waveform's Left, Right, Mid, Side, and crossing
trigger behavior. Alternate stereo rings keep Left and Right aligned. Adjust the
capture window, repeats, seam blend, input gain, radial and sideways movement,
and temporal or line smoothing. Auto scale is optional; disabling it preserves
relative input amplitude. Orbit speed and trace spin set independent movement,
with optional alternating directions.

Appearance includes palettes, color by ring/amplitude/position, line width, glow,
audio-driven brightness, opacity, rear visibility, and resting orbit guides.
Free trackball rotation, roll, pan, zoom, and axis views work like the other 3D
primitives. Numeric controls can be dragged onto twelve initially empty macro
slots. Project and view presets save settings and assignments; adding Rings does
not create signal channels or alter existing components.

### 3D Resonance

Add **3D Resonance** for layered, petal-shaped wavefronts inspired by radiation
and merging-black-hole artwork. Start with **Radiation petals**, **Solar waves**,
or **Bloom** in Geometry. Adjust one to sixteen layers, one to eight petals,
spacing, depth, flattening, waist, sharpness, twist, and shell opening. This is
an artistic primitive rather than a physical simulation.

Flow controls layer expansion and recycling, with independent spin, traveling
ripples, phase spread, and optional alternating rotation. Audio can expand the
shells, displace their surfaces, and brighten them; frequency range, input gain,
and response smoothing control the reaction. Membranes, wireframe, contours,
and points share palettes and layer/petal/energy/position coloring, transparency,
rim light, and optional wire overlays. Detail is bounded to keep geometry costs
predictable as layers increase.

Free trackball rotation, roll, pan, zoom, and axis views match the other 3D
components. Numeric controls support scene macro assignment. Projects and
presets save the configuration; the **Resonance petals** view preset provides
another starting point. Existing projects gain default Resonance settings
without changing their components, macros, or signal channels.


### Orbital GPU visualizer

Add **Orbital** for a GPU orbital/flocking hybrid: collective particle currents,
moving attractor orbs, repulsive cores, turbulence, HDR trails, bloom and shaded
orb lighting. The inspector exposes 111 continuous parameters that can be dragged
onto scene macros and connected to Signals. Audio force/light provide optional
main-audio response; turn those down to use only manual controls and signal mappings.

Start with **Balanced (32,768 particles)**; the Particles inspector also offers
16,384, 65,536 and 131,072. Quality and appearance are saved per component. The
**60 FPS display target** is a local preference under **Settings → Display & Layout**,
independent of analysis rate and project contents. Drag the canvas to rotate,
Shift/right-drag to pan, and scroll to zoom. **Camera** adds orthographic/perspective
views, XYZ grids, axis presets and auto-orbit. **Particles → Paths & trails** traces recent world-space
trajectories for a configurable subset of particles. Choose **Arrow** in Particles
to show travel direction; **Particles → Flow & motion** provides neighborhood, turning, drag,
minimum-speed, drift and confinement controls. Hover a control with Info View open
for practical guidance, defaults and related controls.

For the richer orb treatments, open **Rendering → Starting looks** and choose **Lensing swarm** or
**Solar ribbons**. These starting looks combine dark cores or radiant surfaces,
asymmetric rims, lens warping, fine irregular corona rays and surface-following flow.
**Orbs → Corona & rays** exposes the atmosphere controls. **Signal Response** groups
audio-reactive motion, lighting, size and transient flares; map a
Signal to **Burst drive** for direct control. **Core occlusion** hides particles
and paths behind an orb while retaining foreground motion. Lens distortion is a
stylized screen effect, separate from the actual particle forces in **Orbs → Surface field**.

The new [GPU visualizer library](docs/gpu-visualizers.md) is independent of the UI
and capture engine. It provides the foundation for additional bespoke GPU sketches.
MilkDrop uses the separate compatibility engine below; Cinder integration remains
outside the current implementation.

### MilkDrop-compatible visualizer

**Component browser → MilkDrop** imports and edits `.milk` presets using a pinned native
projectM engine. Use inspector adjustments with scene macros and Signals, browse
favorites, blend between presets, and save preset source/textures with projects.
See [MilkDrop integration](docs/milkdrop.md) for controls, compatibility limits,
build dependencies and GPU validation. Builds now require CMake 3.21+ and a C++
compiler; Linux additionally requires EGL development libraries.

The bundled projectM library has its own LGPL license. See
[third-party notices](THIRD_PARTY_NOTICES.md), including source/relinking requirements.

### Cymatics

The Cymatics component adds incoming-audio Chladni sand, shaded wave surfaces, nodal patterns, and artistic interference on rotatable square, rectangular, or circular plates. Its inspector exposes plate physics, GPU grains, color/material controls, audio response, and camera navigation; numeric controls support scene macros and Signals. Sand transport is approximate, with natural settling by default. It follows the global display FPS preference and never generates sound. See [the model, controls, and validation notes](docs/cymatics.md).
