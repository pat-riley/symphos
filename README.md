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
- A live 3D waterfall with surface, frequency-line, time-line, wireframe, or dot rendering, independent surface
  and floor grids, rotation, zoom, and 0.1–30 second history (2 seconds by default)
- Independent waterfall X/Y lengths (0.25×–10×, default 1× each) without changing frequency range or history duration
- Independent frequency range, linear/log scale, smoothing, gain, decay, and
  intensity controls for each frequency module
- Spectrum peak hold, stereo waveform time/amplitude controls, and color palettes
- Settings sidebar with collapsible categories that follows the selected pane
- Interactive camera orientation gizmos in the sidebar and waterfall viewport
- Toggleable bottom-left Info View with hover descriptions instead of popup tooltips
- Shared FFT size, window function, analysis channel, and frame-rate controls
- Bottom status strip with capture status, FPS, sample rate, and peak level
- Raw FFT-bin inspection, detailed diagnostics, and fullscreen mode
- GPU rendering with `wgpu` on native Wayland
- Colors derived from the active Omarchy theme

## Using the dashboard

Symphos automatically selects system-output speakers on startup (or another
system output if speakers are unavailable; it never automatically selects a
microphone). The compact selector at the top right lets you choose any source;
the dropdown retains full names and source types. Manual choices are kept for
the session and are not replaced when other devices appear.
Click a pane to edit its settings.
Use the dropdown in any pane header to choose its module. Drag the divider below
the main pane or between the lower panes to resize them. **Expand** fills the
workspace with one pane; **Restore** or Escape returns to the dashboard.

Drag the waterfall to orbit fully around either axis, including over and under
the grid. Right-drag, middle-drag, or Shift + left-drag pans the view. Rotation
keeps the view's scale stable. Scroll to zoom (0.5×–10×, also adjustable under
Camera) and double-click to reset the
camera, including pan. **Center view** under Camera resets only the pan.
The Camera panel has an XYZ orientation gizmo, with a compact version in the
waterfall's upper-right corner. Drag either gizmo to orbit, click an axis to
align, and click it again to flip to the opposite side. Front, Side, and Top
buttons provide the same view shortcuts. X represents frequency, Y time, and
Z level; angle fields use degrees.
The gizmos use colored lines and plain axis labels without endpoint circles;
axis clicking and dragging still work.
History is live only; there is no recording or playback.
Waterfall frequency, gain, smoothing, and decay controls reprocess the retained
history immediately. Height stretches the surface above its fixed floor.
The menu button at the upper left collapses or opens the settings sidebar.
Under **Time & History**, **History s** sets the displayed time window.
Under **Geometry**, **Length X** stretches the frequency axis and **Length Y**
stretches the time axis independently. Both default to 1× with a 0.25×–10× range.
The floor grid and labels follow both lengths, and framing accommodates the geometry.
**Geometry** offers Surface, Lines, Y Lines, Wireframe, and Dots. Lines draws traces
across frequency; Y Lines draws traces along time, one per frequency band.
Wireframe connects both directions without filling the faces. Dots shows separate
colored points for each frequency/time sample with no connecting edges.
Neither length nor mode changes
clear the audio history.
Under **Appearance**, choose **Heatmap** for a full blue → cyan → green →
yellow → orange → red gradient. Cooler colors represent quieter levels and
warmer colors louder peaks, using the same signal level as waterfall height.
**Floor dB** and **Ceiling dB** set the mapped range. Heatmap works in every
waterfall geometry mode and is also available in the spectrogram.
Click **? Help** at the bottom right to show or hide **Info View** at the bottom
left. Hover over a control or visualization for a short description. Popup
tooltips are disabled even when Info View is hidden; existing descriptions are
retained in the shared help system. Capture status remains next to Help in the
footer. When settings are open, Info View docks underneath them; otherwise it
reserves space below the dashboard. It never covers a visualization.
Controls are grouped by purpose: **Camera**, **Time & History**, **Geometry**,
**Frequency Range**, **Signal Response**, and **Appearance**, with only relevant
sections shown for each module. Several sections can stay open together; their
open/closed state is kept separately for each pane and module during the session.
Shared audio settings are under **Audio Analysis · Shared**. Waveform
time windows are limited to the current FFT capture window.

**View** includes pane-size reset, fullscreen (F11), and the FFT inspector with
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
