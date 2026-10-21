# Changelog

All notable changes to Symphos will be documented here. The project follows
[Semantic Versioning](https://semver.org/) once stable releases begin.

## [Unreleased]

### MilkDrop-compatible visualizer

- Add a native projectM-backed MilkDrop component with `.milk` file/folder import,
  shader/equation playback, editable saved values, portable textures, and export.
- Add live inspector adjustments for motion, feedback, waveform color, echo,
  borders and native audio response, assignable to scene macros and Signals.
- Add per-component favorites, shuffle, auto play, transitions and preset lock.
- Preserve older projects with a fifteenth component slot and empty macro bank.
- Build the pinned native library offline; CMake and Linux EGL are new build dependencies.

### Added

- Add a full-screen scene Library with project/preset sections, saved previews,
  search, explicit loading, scene creation, duplication, renaming, deletion, and
  drag-to-reorder in grid and list views, plus double-click recall and contextual
  actions. Remember the view preference locally. Replace the scene toolbar plus with a Save icon and naming
  dialog; reusable scene presets capture complete setups and load as new scenes.

- Expand the application menu with Edit, Create, Scene, Playback, View, Window,
  and Help; expose component/source creation, scene management, playback,
  dock visibility, zoom, diagnostics, and direct settings categories. Share a
  command catalog with the shortcut reference and non-macOS menu; reflect live
  checkmarks, selection compatibility, modal state, and native text editing.

- Add Orbital, a native GPU orbital/flocking visualizer with configurable particles,
  attractor orbs, surface avoidance fields, world-space paths, arrow glyphs,
  XYZ camera guides, lensing cores, coronas and transient flares. Expose 111
  scene-saved macro parameters in grouped Particles, Orbs, Signal Response,
  Camera and Rendering inspectors, preserving older projects and mappings.
- Add the internal Rust/wgpu `symphos-visuals` library and a local display frame-rate
  preference (60 FPS default) independent of audio analysis and project settings.

- Add Flux, a music-reactive 3D particle cloud with moving attraction points,
  bass bursts, swirling currents, trails, four palettes and color mappings,
  42 controls, and orbit/roll/zoom/perspective camera controls. Add the Flux
  currents preset and preserve customized settings from older workspaces.

- Add Strata, an audio-reactive layered wire sculpture inspired by suspended
  membranes and granular fields. Include drifting dust, tumbling wire grains,
  39 controls, four palettes, drag/scroll camera controls, automatic orbit, and
  the Strata chamber preset. Preserve customized Veroni and older workspaces.

- Add Veroni, an audio-reactive Voronoi visualizer with flowing cells, luminous
  points, and fine connections. Include 30 inspector controls for geometry,
  motion, audio response, and appearance; component browser integration and Veroni
  flow preset; pause/fullscreen and saved settings. Upgrade existing six-module
  workspaces and view presets without losing their configuration.

### Changed

- Make the component browser resizable from its right edge, with double-click reset
  and a locally remembered width, matching the inspector’s resizing behavior.

- Replace the Add New capsule/dropdown with a borderless plus button matching
  the inspector icon’s styling. Browse, click to add, or drag all components from the left drawer.

- Tighten the top bar to a 42-pixel height with equal 8-pixel outer padding,
  uniform 26-pixel controls, and a smaller logo aligned with the toolbar icons.

- Compact the New component browser into single-line icon/name rows with six-dot
  drag grips. Remove visible descriptions, search, and category tabs, and simplify its header.

- Map Strata membranes to separate logarithmic bands from bass at the bottom
  to treble at the top. Automatically redistribute bands when layer count
  changes, with independent response envelopes, motion, and highlights.

- Use the white Symphos logo as the top-left component browser toggle. Give its browser
  a flush left sidebar with shared inspector styling and component icon rows.

- Center the preset controls on the full navbar and reduce the audio selector
  width, while retaining full device names in its menu.

- Add a far-right navbar button to show or hide the component inspector, with
  an active state while it is open.

- Give visualization panes one continuous background, with header controls
  floating above the same surface as the visualization.

- Unify inspector styling with edge-to-edge section dividers, full-width
  disclosure rows, quieter group labels, and consistent numeric fields. Add a
  pinned component header with its icon, visualization type, panel, status, and
  sample rate.

- Add a separate eye button beside panel fullscreen that opens the selected
  panel’s inspector. Show all parameter sections in one scrollable panel with
  collapsible headers, flush to the navbar and right/bottom window edges.

- Constrain visualization dropdown labels so narrow pane headers keep pause and
  inspect controls unobstructed.

- Avoid allocating sample lists and cloning themes on waterfall mesh cache hits;
  borrow numeric labels and format dynamic help only for the hovered control.
- Keep axis navigator endpoints on the stack and skip painting offscreen camera
  controls.
- Use concise native numeric formatting for exact entry and clear stale input
  errors after a valid adjustment.
- Center and orbit the waterfall around the floor grid's midpoint, with default
  framing sized to keep the full height range visible while rotating.
- Give camera transforms full-width rows, enlarge the XYZ navigator, and use
  compact labeled cube shortcuts with active-view feedback.
- Standardize compact properties dropdowns and show all eight isometric views
  with aligned entries and automatic closing after selection.

- Place inspector parameter labels above full-width numeric fields.
- Keep the horizontal resize cursor while hovering or dragging numeric fields;
  show the text cursor only during inline editing.
- Replace narrow module sliders and separate value boxes with full-width fields,
  slim range indicators, centered editable values, and relative dragging.
- Restyle module properties with compact recessed controls, aligned numeric
  labels, and collapsible section headers inspired by Blender's Properties editor.
- Remove the properties close button's background, using an accent-colored X
  on hover and press.

- Soften selected pane outlines to a muted accent tint.
- Dock the component inspector on the right, with an inspect
  button and a resize handle on its left.
  Selecting a component keeps a hidden inspector closed.
- Match the dark Mac navigation surface to the native title bar, with contrasting
  dropdown fields and visible hover, selection, and border states.
- Give the top navigation a uniform full-width surface and clear lower edge;
  remove the development toolbar while keeping live reload and CLI audio modes.

### Added

- Tiled workspace manager with close-and-fill behavior, resizable nested splits,
  pane rearrangement, and a searchable component library with edge/center drops.
- Persistent last workspace and a top preset browser for layout-only, full-view,
  and matching-component presets, including save, rename, and delete.

- Opt-in development app with rebuild-on-save, last-working-build retention,
  settings-only workspace restoration, generated stereo audio, and live palette
  and UI-scale editing.

- Initial macOS system-playback capture through a private, nonmuting Core Audio
  tap, with fresh analysis sessions on reconnect and a manual capture retry.
- Local macOS `.app` packaging, audio permission metadata, Mac shortcut labels,
  platform log paths, and macOS CI alongside the pinned Linux PipeWire baseline.
  The Mac build is a developer preview; plugins and microphone/input selection
  on Mac are separate milestones.

- Retained peak markers and latched clipping for the footer's independent Left,
  Right, and Mix meters, with an explicit reset action.
- Live reference capture and comparison overlays in Spectrum panes.
- Pinned frequency, note, level, and time measurements in frozen Spectrum and
  Spectrogram panes.
- A dedicated Oscilloscope module with local pitch-period locking, threshold
  triggering, free run, single/multiple-cycle views, Left/Right/Mid/Side routing,
  auto/manual scaling, RGB and multi-band traces, and aligned afterglow overlays.
- A Stereometer module with scaled, one-to-one rotated linear, and Lissajous
  vectorscopes; static, frequency-balanced RGB, and overlaid low/mid/high color
  modes; overall or multi-band phase correlation; and an RMS balance indicator.
- Independent one/two-lane waveform routing for Left, Right, Mid, and Side;
  static, multi-band, and level color-map modes; multi-band peak-history
  overlays; adjustable travel speed; and scrolling/static-loop motion.
- A documented analyzer feature backlog covering metering, capture, DAW,
  workspace, presentation, and cross-platform work.
- Fine parameter dragging, individual double-click resets, exact numeric entry,
  bounded custom values, and copyable diagnostic reports with rotated local logs.
- Basic pane/navigation shortcuts and a keyboard-reference modal in Settings.
- Typed settings-only Copy/Paste between matching modules, without persistence.
- Renderer-independent frequency capture so hidden/occluded history views catch
  up without stopping collection; source epochs reject obsolete capture packets.
- Waterfall mesh caching, linear-time diagnostic band summaries, throttled BPM
  estimation, and FFT-independent target-rate scheduling.
- Regression tests and opt-in CPU benchmarks for the capture/rendering changes.
- Per-pane pause/resume and module-only settings reset.
- Spectrum Line/Filled area styles, contextual appearance controls, timed or
  indefinite peak hold with independent release, palettes, and guide visibility.
- FFT-independent waveform windows, threshold triggering, automatic scaling,
  filled traces, and separate centerline/grid/label controls.
- Spectrogram orientation, smooth/pixelated raster, time/frequency display
  resolution, guide visibility, and sub-second history windows.
- Native PipeWire microphone and system-output capture.
- GPU-rendered analyzer interface with waveform, spectrum, spectrogram, meters,
  derived signal metrics, and raw FFT-bin inspection.
- Omarchy theme integration, desktop launcher metadata, and scalable app icon.
- User-local development installer and uninstaller.
