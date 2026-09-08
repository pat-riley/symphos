<p align="center">
  <img src="assets/io.github.riley.Symphos.svg" width="112" alt="Symphos icon">
</p>

# Symphos

Symphos is a native, GPU-rendered audio analyzer and visualizer for Omarchy.
It captures microphones and system output through PipeWire, displays the signal
in real time, and is being built toward a visual scene editor where audio
signals can control shapes, particles, effects, and shaders.

Symphos is currently a proof of concept. The analyzer works, but scene creation
and visual parameter mapping are still on the roadmap.

## Current features

- Compact top-right PipeWire source selector, defaulting to system-output speakers
- Four resizable panes with interchangeable waterfall, spectrum, waveform,
  and spectrogram modules
- A live 3D waterfall with Surface, Lines, Y Lines, Wireframe, Dots, Stems, and Bars render styles,
  fully hideable guides, rotation, zoom, and 0.1–30 second history (2 seconds by default)
- Independent waterfall X/Y lengths (0.25×–10×, default 1× each) without changing frequency range or history duration
- Independent frequency range, linear/log scale, time/frequency smoothing, gain, decay, and
  intensity controls for each frequency module
- Optional musical-note axis labels and adjustable A4 tuning reference
- Mode-specific appearance controls, heatmap contrast, eight isometric views, and camera auto-orbit
- Spectrum peak hold, stereo waveform time/amplitude controls, and color palettes
- Module-only settings sidebar with icon tabs that follows the selected pane
- Interactive camera orientation gizmos in the sidebar and waterfall viewport
- Toggleable bottom-left Info View with hover descriptions instead of popup tooltips
- Shared FFT size, window function, analysis channel, and frame-rate controls
- Bottom navigation with FFT/rate controls, a stereo/mix icon, live levels, FPS, and sample rate
- Raw FFT-bin inspection, detailed diagnostics, and fullscreen mode
- GPU rendering with `wgpu` on native Wayland
- Colors derived from the active Omarchy theme

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
Use the dropdown in any pane header to choose its module. Drag the divider below
the main pane or between the lower panes to resize them. The small fullscreen
icon in each pane header fills the workspace with that pane; the inward-corners
restore icon or Escape returns to the dashboard.

Drag the waterfall to orbit fully around either axis, including over and under
the grid. Right-drag, middle-drag, or Shift + left-drag pans the view. Rotation
keeps the view's scale stable. Scroll to zoom (0.5×–10×, also adjustable under
Camera) and double-click to reset the
camera, including pan. **Center view** under Camera resets only the pan.
The Camera panel has an XYZ orientation gizmo, with a compact version in the
waterfall's upper-right corner. Drag either gizmo to orbit, click an axis to
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
The icon rail and settings share one continuous sidebar surface, without a gap
or separate rounded panels. The chevron collapses or opens settings.
The rail stays visible when collapsed; clicking a tab opens its section.
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
Only the selected style's parameters are shown directly below the dropdown:
surface mesh/base walls; line thickness and time-trace spacing; Y-line thickness
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
bottom-right meters; detailed status remains in Shared options. When settings
are open, Info View docks underneath them; otherwise it
reserves space below the dashboard. It never covers a visualization.
The waterfall's icon tabs are ordered **Geometry**, **Frequency Range**,
**Signal Response**, **Appearance**, then **Camera** at the bottom. Geometry is
the default tab and includes history duration, so the waterfall no longer needs
a separate Time & History tab. Other modules show only relevant tabs; the
spectrogram retains its Time & History tab. One section is visible at a time; the selected tab
and scroll position are remembered separately for each pane/module/tab during the session. Waveform
has Time Window, Amplitude, and Appearance tabs; channel display is a global setting.

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
- **Waveform:** 1–250 ms windows independent of FFT size, with full-rate capture
  and peak-preserving display reduction. Optional rising/falling threshold trigger
  aligns repeating signals (left channel in stereo, selected channel in single
  view), falling back to the latest window when no crossing exists. Auto scale uses
  a common, bounded gain for both channels; manual amplitude remains available.
  Line/Filled area styles have thickness/opacity controls, plus separate centerline,
  grid, and label switches. Trigger and auto scale default off; window remains 40 ms.
- **Spectrogram:** 0.1–30 seconds of visible history (8 seconds by default), newest
  at right or bottom, smooth or pixelated rendering, 64–1024 time cells and 24–96
  frequency cells. Lower frequency display resolution preserves the loudest band
  in each cell. Display detail does not increase FFT resolution or the 30 Hz history
  capture rate. Palettes, contrast, levels, grid, and labels are independently
  adjustable without clearing history.

The bottom bar keeps **FFT** and **Rate** visible. The overlapping-circles icon
switches all waveform panes and meters between separate **L/R stereo** and a
**single mixed view**, without changing frequency analysis or clearing history.
The shared-options icon holds the FFT window, frequency-analysis channel,
single-view channel (mix/left/right), diagnostics, capture details, and theme info.
Level bars show RMS with peak markers and peak dBFS readouts. Mixed levels are
measured from the summed signal, including phase cancellation. Waveform time
windows are independent of the FFT capture window. The source selector
and a compact settings icon remain at the top right; there is no top-left menu button.

The top-right **settings icon**, matched to the source picker's height, opens
pane-size reset, fullscreen (F11), and the FFT inspector with
the detailed analyzer metrics and experimental tempo estimate. Pane assignments,
sizes, module settings, and Info View visibility currently last for the session.

## Requirements

Symphos currently targets Linux desktops using PipeWire. Development is focused
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

## Build and run

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

This installs the binary and desktop metadata under `~/.local`. Remove that
installation with:

```sh
./scripts/uninstall-dev.sh
```

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
Arch packaging.

## Contributing

Symphos is early software and its scene model is still taking shape. Bug
reports, performance measurements, analyzer validation, and focused pull
requests are welcome. Read [CONTRIBUTING.md](CONTRIBUTING.md) before starting a
larger change.

## License

Symphos is available under the [MIT License](LICENSE).
