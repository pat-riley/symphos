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
