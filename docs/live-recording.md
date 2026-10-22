# Live visualizer recording

Choose a component in the workspace, then **File > Record Visualizer**. Select
landscape 1080p, portrait 1080 × 1920, square, 4K, or custom dimensions, and a
30, 60, or 120 FPS target. Choose the destination and wait for **Recording
performance** before performing. **Stop & save** finalizes the movie. Closing
the recording window keeps capture running; reopen it from File to stop.

The first implementation records one selected component and the incoming stereo
audio to an H.264/AAC QuickTime movie on macOS. It does not record application
chrome. A component's own grid, labels, or camera guides remain part of that
component's image. Its camera can still be manipulated in the preview. Output
dimensions are independent of the workspace pane size; the workspace preview
is letterboxed to show the recording aspect ratio.

Custom dimensions must be even, between 64 and 4096 pixels per axis, with at most
4096 × 2160 total pixels. The GPU visualizers retain their selected quality
settings. Their normal preview-size cap does not constrain recording output.
The global display target remains a local preference; recording can temporarily
request a faster update cadence without changing the saved preference.

## Timing and audio

This is live capture, not an offline export guarantee. Video timestamps follow
the requested frame grid, while actual rendering uses the current live state.
If rendering or encoding falls behind, capture reports missed frames instead of
blocking the audio callback or silently lowering visual quality. Slower frames
remain visible until the next captured frame. Audio follows sample-count timing,
with real gaps retained when the source disconnects.

Audio is tapped on the analysis worker before FFT gain and smoothing. Signal
influence, modulation amount, and visualizer audio response do not change the
recorded audio level. The app still generates no audio. Choose the audio source
and sample rate before starting a recording. If the sample rate changes, capture
stops and saves the existing performance. No input produces a silent movie.

Capturing a different scene or deleting the recorded component ends that capture;
opening the Library or hiding its pane temporarily does not. Closing the app
stops capture and waits for finalization. An existing destination movie is only
replaced after its replacement has finalized successfully.

## Implementation

- `recording/render.rs` renders the component into a private egui context and a
  full-resolution BGRA texture, which is also reused for the workspace preview.
- Three reusable readback buffers and a bounded three-frame encoding queue keep
  memory and GPU work bounded. Readbacks are delivered in submission order even
  when GPU callbacks complete out of order.
- The analysis worker publishes stereo PCM through a bounded queue. Device
  callbacks perform no new allocation, locking, logging, or encoding.
- `recording/encoder.m` uses AVAssetWriter on a dedicated worker. The normal UI
  path never waits for compression. Preparation excludes encoder initialization
  and the first shader/texture setup from the performance clock.

Full-scene composition, offline audio-file rendering, lossless codecs, and Linux
encoding remain follow-up work. The offscreen output path is reusable groundwork
for scene composition and a later scene-wide post-processing stage.

## Verification

Ignored hardware tests exercise native stereo encoding, odd-row-stride texture
readback across all 16 components, 1080 × 1920 at 60 FPS, and 1280 × 720 at 120
FPS. See [the performance report](performance-2026-10-21.md) for measurements and
the distinction between output-path timings and whole-application frame rate.
