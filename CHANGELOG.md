# Changelog

All notable changes to Symphos will be documented here. The project follows
[Semantic Versioning](https://semver.org/) once stable releases begin.

## [Unreleased]

### Added

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
