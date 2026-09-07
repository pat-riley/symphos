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

- Selectable microphone and system-output sources through PipeWire
- Linear and logarithmic frequency spectra
- Stereo waveform, scrolling spectrogram, peak and RMS meters
- Dominant frequency and musical note detection
- Spectral centroid, rolloff, flatness, zero-crossing rate, and crest factor
- Experimental BPM estimation with confidence reporting
- FFT size, window function, smoothing, gain, frequency range, decay, channel,
  and frame-rate controls
- Raw FFT-bin inspection and fullscreen mode
- GPU rendering with `wgpu` on native Wayland
- Colors derived from the active Omarchy theme

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
git clone <repository-url>
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
