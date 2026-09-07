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

- Microphone and system-output selection in the top navigation through PipeWire
- Four resizable panes with interchangeable waterfall, spectrum, waveform,
  and spectrogram modules
- A live 3D waterfall with surface or spectrum-line rendering, independent surface
  and floor grids, rotation, zoom, and 0.1–30 second history (2 seconds by default)
- Independent frequency range, linear/log scale, smoothing, gain, decay, and
  intensity controls for each frequency module
- Spectrum peak hold, stereo waveform time/amplitude controls, and color palettes
- Compact, collapsible settings sidebar that follows the selected pane
- Shared FFT size, window function, analysis channel, and frame-rate controls
- Bottom status strip with capture status, FPS, sample rate, and peak level
- Raw FFT-bin inspection, detailed diagnostics, and fullscreen mode
- GPU rendering with `wgpu` on native Wayland
- Colors derived from the active Omarchy theme

## Using the dashboard

Select an audio source in the navbar, then click a pane to edit its settings.
Use the dropdown in any pane header to choose its module. Drag the divider below
the main pane or between the lower panes to resize them. **Expand** fills the
workspace with one pane; **Restore** or Escape returns to the dashboard.

Drag the waterfall to orbit fully around either axis, including over and under
the grid. Rotation keeps the view's scale stable. Scroll to zoom and double-click to
reset the camera. History is live only; there is no recording or playback.
Waterfall frequency, gain, smoothing, and decay controls reprocess the retained
history immediately. Height stretches the surface above its fixed floor.
The menu button at the upper left collapses or opens the settings sidebar.
Shared audio settings are under **Audio analysis** in that sidebar. Waveform
time windows are limited to the current FFT capture window.

**View** includes pane-size reset, fullscreen (F11), and the FFT inspector with
the detailed analyzer metrics and experimental tempo estimate. Pane assignments,
sizes, and module settings currently last for the session.

## Requirements

Symphos currently targets Linux desktops using PipeWire. Development is focused
on Omarchy and tested on ARM64 Apple Silicon with the Asahi graphics stack.

You need:

- Rust 1.95 or newer
- PipeWire development headers
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
