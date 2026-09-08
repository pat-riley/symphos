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

The UI coalesces source-discovery events and selects a system output on startup,
preferring nodes with "speaker" in their description or name. It can upgrade
an automatic fallback when speakers arrive later, but never selects microphones
automatically or overrides an explicit session choice. No system default device
is changed; selection only targets the analyzer's capture stream. The navbar's
compact top-right selector preserves full device names in its dropdown/Info View.

Each dashboard pane owns independent waterfall, spectrum, waveform, and
spectrogram state in `src/modules/`. Selecting a pane directs the compact
sidebar to its active module. Pane assignment and sizing live in the UI;
capture and analysis remain independent of layout and rendering.

Sidebar controls use a shared collapsible-panel helper. Section IDs are scoped
by pane and module, separating disclosure state while switching modules.
Frequency bounds, signal response, and appearance controls are separate groups;
shared audio analysis and meters remain outside the module-specific groups.
The waterfall's sidebar and viewport orientation gizmos operate on the same
camera state and share its projection math. Their Z-up axes map frequency to X,
time to Y, and level to Z. Axis clicks align or flip the view without changing
pan, zoom, or history. Gizmo dragging retains unrestricted orbit.
Gizmos retain invisible endpoint hit targets with colored arms and plain labels,
without endpoint discs. `help::HoverHelp` preserves descriptions at control call
sites and sends hovered/dragged response text to the docked Info View instead of
popup tooltips. Each frame clears the help selection; smaller, specific controls
take precedence over enclosing regions. The panel is drawn after controls so
descriptions update in the same frame, and its reserved layout never overlays
the dashboard. Help visibility is independent of the settings sidebar.

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
views from below the grid. Panning offsets the projected camera center in pane
coordinates, independently of zoom and rotation, and scales with pane resizing.
The waterfall defaults to two seconds and
supports 0.1–30 seconds, with independent surface-grid and floor-grid toggles.
Independent 0.25×–10× X (frequency) and Y (time) length multipliers under Geometry
scale the surface, lines, wireframe, floor grid, and axis labels together.
Both default to 1×. A shared geometry transform maps the user-facing Y axis to
the renderer's Z axis. The camera's bounding sphere accounts for both lengths
while staying stable during orbit and height edits. Wireframe uses colored
segments across both frequency and time, skips missing-history gaps, and does
not fill faces. These geometry settings never alter or clear retained history.
Y Lines emits only time-axis connections within each frequency band, with no
cross-frequency edges, and skips missing-history gaps just like Wireframe.
Dots emits small screen-space discs for valid frequency/time samples only,
sorted back-to-front in a single mesh, with no edges between samples.
The shared Heatmap palette interpolates seven blue-to-red color stops using
the same normalized level as waterfall height. It is independent of desktop
theme colors and camera/geometry settings; the module's dB floor and ceiling
define its signal range. It also applies to the spectrogram without changing
capture or retained history.
Slider and scroll zoom share a 0.5×–10× range; close-ups are clipped to the pane
and existing pan gestures allow navigation without changing geometry or history.
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
