# Architecture

## Runtime lanes

1. **PipeWire lane** negotiates native float audio and copies interleaved
   samples into a fixed-size SPSC ring. The callback never locks, allocates,
   logs, or performs FFT work.
2. **Analysis lane** drains the ring, maintains overlapping windows, computes
   reusable real FFT plans, derives metrics, and atomically publishes an
   immutable `AnalysisFrame`.
3. **UI/GPU lane** reads the newest snapshot and renders it. It never waits for
   the audio or analysis lane and may safely skip intermediate frames.

Dropping stale samples or visual frames is preferable to blocking a real-time
audio callback.

## Analysis contract

Each published frame contains source format and timing metadata, stereo
waveforms, linear FFT bins, display-ready logarithmic bands, peak/RMS values,
dominant frequency, nearest equal-tempered note, spectral centroid, rolloff,
flatness, zero-crossing rate, dynamic range, and a conservative BPM estimate.

The analyzer displays both logarithmic and downsampled linear spectra. The raw
inspector always exposes every linear FFT bin with its exact frequency,
magnitude, and smoothed dBFS value.

The contract is deliberately renderer-agnostic so future scenes can consume
the same data through GPU buffers, shader uniforms, or native Rust modules.

## Visualization modules

Each dashboard pane owns independent waterfall, spectrum, waveform, and
spectrogram state in `src/modules/`. Selecting a pane directs the compact
sidebar to its active module. Pane assignment and sizing live in the UI;
capture and analysis remain independent of layout and rendering.

Frequency modules derive 96 linear or logarithmic display bands from raw FFT
magnitudes. Each applies its own gain, time-adjusted smoothing, decay, frequency
range, and dB range; shared FFT/window/channel controls still select the signal
and transform used by all panes. This avoids applying the analysis frame's
pre-smoothed display bins a second time.

Waterfall and spectrogram histories retain at most 30 seconds / 901 snapshots
per active module, sampled at no more than 30 Hz. Monotonic timestamps set the
time axis independently of UI refresh rate. Raw FFT magnitudes are retained
alongside display bands so frequency range, scale, gain, smoothing, and decay
changes reprocess existing history without a reset. The maximum FFT size uses
about 29 MiB of magnitude storage per history. Stale snapshots leave gaps;
capture format changes clear incompatible history.
Changing sources or shared FFT/window/channel settings clears pane histories.

The waterfall projects a bounded frequency/time surface on the UI lane and
submits one mesh to egui's existing GPU renderer. Cells and grid edges are
sorted back-to-front for camera rotation. Surface mode closes its perimeter
down to the fixed floor; line mode draws separate frequency traces. Camera
framing reserves the entire height range so changing height does not shift the
floor or clip peaks at default zoom. A fixed bounding sphere keeps scale and the
orbit center stable through full horizontal and vertical rotations, including
views from below the grid. The waterfall defaults to two seconds and
supports 0.1–30 seconds, with independent surface-grid and floor-grid toggles.
History length, height, time-slice detail, palette, and camera controls are local
to the waterfall. Hidden panes
do not render or accumulate new history. No audio callback work was added.

## Frequency analysis

- Selectable power-of-two FFT sizes from 512 through 16,384 samples.
- Hann, Hamming, Blackman-Harris, and rectangular windows.
- Configurable overlap/hop size and exponential attack/release smoothing.
- Linear bins remain available for inspection; logarithmic bands drive the
  default spectrum display.
- Decibels use full scale (`dBFS`) with a bounded numerical floor.

## Failure model

PipeWire disconnects, disappearing devices, format changes, and ring overflow
are status events. They do not panic the process. Source selection rebuilds the
capture stream off the real-time callback.
