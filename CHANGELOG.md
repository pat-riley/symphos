# Changelog

All notable changes to Symphos will be documented here. The project follows
[Semantic Versioning](https://semver.org/) once stable releases begin.

## [Unreleased]

### Changed

- Constrain visualization dropdown labels so narrow pane headers keep pause and
  expand controls unobstructed.

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

- Right-align property labels against their numeric fields for consistent scanning.
- Keep the horizontal resize cursor while hovering or dragging numeric fields;
  show the text cursor only during inline editing.
- Use transparent collapsible properties headings separated by thin lines,
  keeping darker backgrounds on editable controls.
- Replace narrow module sliders and separate value boxes with wide filled fields,
  centered editable values, and relative dragging inspired by Blender's controls.
- Restyle module properties with compact recessed controls, aligned numeric
  labels, and collapsible section headers inspired by Blender's Properties editor.
- Show each properties tab's sections directly, with independent collapsible
  groups and no redundant dropdown that hides the entire tab.
- Use a side-profile film camera icon for the camera properties tab.
- Remove the properties close button's background, using an accent-colored X
  on hover and press.
- Simplify the settings toolbar to unboxed icons, using the accent color for
  the open tab and hover feedback instead of a side marker or separators.

### Added

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
