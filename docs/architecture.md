# Architecture

## Runtime lanes

1. **Capture lane** (PipeWire on Linux, Core Audio taps on macOS) negotiates float audio and copies interleaved
   samples into a fixed-size SPSC ring. The callback never locks, allocates,
   logs, or performs FFT work.
2. **Analysis lane** drains the ring, maintains overlapping windows, computes
   reusable real FFT plans, derives metrics, and atomically publishes an
   immutable `AnalysisFrame`.
3. **UI/GPU lane** reads the newest snapshot and renders it. It never waits for
   the audio or analysis lane and may safely skip intermediate frames.

Dropping stale samples or visual frames is preferable to blocking a real-time
audio callback.

Live recording adds a bounded encoder lane on macOS. The analysis worker taps
raw stereo PCM before analysis gain; GPU readbacks feed a three-frame queue.
Neither capture callbacks nor ordinary UI rendering wait for compression.
The selected component renders once at its requested output resolution and that
texture also supplies the workspace preview. See [live recording](live-recording.md).

Creative undo stores bounded scene/settings transactions and reuses surviving
component runtimes when restoring them. Shared parameter descriptors supply
macro dragging and direct MIDI Learn; bindings use stable component identities
and are saved with scenes. See [workflow foundations](workflow-foundations.md).

On the experimental input-routing branch, Signals also owns named audio
processing branches. The Signals worker processes fresh PCM snapshots into
shared waveform, spectrum and measurement feeds, and archives their frequency
history. Components select Main audio, a named input or Unassigned; modulation
detectors can select those same inputs. The visual inspector delegates input
response controls to the Signals dock. No input processing runs in capture or
render callbacks. See [signal input routing](signal-input-routing.md) for the
experiment's behavior, compatibility and current limits.

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

Waveform arrays contain chronological, full-rate samples from a half-second
analysis-lane ring, independent of FFT size. Capacity is at least the maximum
FFT size and capped at 192,000 stereo samples (half a second at 384 kHz).
Rate changes reset this ring. The callback and SPSC buffer remain unchanged.
Waveform display windows are capped at 250 ms, leaving trigger-search headroom;
per-pixel extrema reduction preserves brief transients and their time order.
Stereo trigger alignment uses the left channel, and automatic display gain is
shared between lanes to preserve relative levels. Translucent fills split at
zero crossings instead of emitting self-intersecting quads.

## Visualization modules

The UI coalesces source-discovery events and selects a system output on startup,
preferring nodes with "speaker" in their description or name. It can upgrade
an automatic fallback when speakers arrive later, but never selects microphones
automatically or overrides an explicit session choice. No system default device
is changed; selection only targets the analyzer's capture stream. The navbar's
compact top-right selector preserves full device names in its dropdown/Info View.

Each dashboard pane owns independent waterfall, spectrum, waveform,
spectrogram, and stereometer state in `src/modules/`. Selecting a pane directs
the right-hand inspector to its active module without opening it automatically.
The inspector reserves space on the right and resizes from its left edge. A pane’s
eye button opens it; its header X closes it. Parameter sections are independent
collapsible headers in one scrollable surface that meets the navbar and window edges. Pane assignment and sizing live in
the UI; capture and analysis remain independent of layout and rendering.
The component browser and inspector slide over 220 ms using egui's frame clock
and symmetric easing. Their contents retain full width and clip at the window
edge, while the dashboard and footer reserve the animated visible width. Resize
grips activate after opening; persisted widths and open states remain independent
of the transition, including when reversing it mid-slide.

Per-pane freeze holds a cloned analysis snapshot and a fixed virtual timestamp.
Resume accumulates the paused duration into that pane's clock offset, so old
history does not expire while paused or jump forward on resume. Other panes and
capture are unaffected; skipped wall-clock time is intentionally not recorded
in the paused pane. Capture/module resets discard freeze state and clock offset.
Module-settings resets retain capture/history and global waveform channel state.
The stereometer consumes the same full-rate half-second stereo snapshot as the
waveform. Its visible-window-only filters split samples at 200 Hz and 2 kHz for
RGB balance, overlaid band plots, and phase correlation; they never run in the
capture callback or change shared FFT analysis. Linear mode maps full-scale
Left/Right samples one-to-one into rotated Mid/Side space, Scaled mode expands
quiet radii inside the same diamond, and Lissajous plots unrotated channels.
The oscilloscope also consumes that snapshot, routing Left, Right, Mid, or Side
locally. Pitch follow downsamples a bounded recent window, removes DC, searches
30 Hz–2 kHz with normalized autocorrelation, refines the strongest early local
peak, and aligns the selected cycle count to a rising zero crossing. Threshold
trigger and free-run modes use an FFT-independent manual timebase. Previous
windows can be overlaid as afterglow without retaining another audio history;
pitch-locked windows share the detected period and therefore align. Its RGB and
multi-band modes reuse display-only 200 Hz and 2 kHz crossover behavior. None of
this processing runs in the capture callback or changes shared analysis.
Spectrogram raster caching keys on retained rows, pixel-scale time movement,
display processing, palette, and theme; paused or empty views do not repeatedly
rebuild/upload the same texture. Display resolution never changes analysis rate.
Frequency-cell reduction takes the loudest constituent band. Spectrum peak
hold uses per-band deadlines and elapsed-time dB release, independent of paint
rate; reprocessing a paused spectrum applies the new gain/range immediately.

Visualizer inspectors share the Photoshop-inspired horizontal Geometry,
Appearance and Camera tabs, compact theme-matched dropdowns, unstriped property
rows and a top-right component-actions menu. Identity and audio routing stay
pinned above the tabs while only the parameter body scrolls. Flat visualizers
omit Camera. Existing section IDs remain readable and map onto these categories;
parameter keys and the audio-processing inspector in Signals are unchanged.
Geometry includes structure, population and simulation/flow; Appearance includes
materials, particle rendering, color, light and feedback. Orbital's particle and
orb groups retain explicit prefixes. No extra visualizer tabs are needed.

Scene backgrounds are saved once in `SceneState`/`Session`, with a runtime copy
on each visible pane for workspace and recording rendering. A defaulted, omitted
black value keeps legacy scene fingerprints and documents compatible. The first
Appearance group edits the scene value. Gradients use a shared cached texture;
editing the background does not reset audio history, particle simulations, or
feedback buffers. GPU particle outputs use premultiplied alpha over that canvas;
Milkdrop retains the opaque output authored by its preset.

All 3D visualizers share the camera motion math in `modules/camera_motion.rs`:
world/custom axes, continuous or eased sweeps, reversal, wobble and independent
framing pan. Waterfall preserves its existing serialized `motion` recipe; other
visualizers save a defaulted `camera_motion` recipe with their existing camera
fields. `modules/camera.rs` adapts radians/degrees and Y-up/Z-up conventions,
applies transient render poses, and restores the exact saved values after drawing.
Manual navigation cancels motion and commits the displayed pose; pause holds the
same virtual timestamp. Camera animation therefore does not dirty presets or
consume Undo history. CPU and GPU renderers retain their lenses and simulation
state. All 3D views expose the same graphical axis navigator and six views;
Flux and Strata also support free trackball rotation, pan and optional XYZ grids.
Signals retain their separate processing rail.
3D Rings wraps signed time-domain samples around each orbit. Its Geometry tab
owns visual displacement, waveform visibility/detail, outline interpolation, and
level-driven expansion. Visibility uses a bounded, shared stereo display boost
with a quiet floor; the signed detail curve retains zero crossings. A separate
per-channel RMS envelope supplies fast attack and configurable exponential
release, using the pane's virtual time so pause and 30/60/120 Hz rendering agree.
These controls never alter the input audio or other sketches. Source gain and
automatic input scaling remain in Signals. Resting guides stay unexpanded and
camera framing uses configured bounds, avoiding camera pumping as music changes.
Each pane/module/tab also has an independent scroll-area identity, avoiding
scroll offsets carrying across unrelated sections. Appearance uses grouped
render-style, color/level, and viewport-guide controls; Geometry groups
dimensions, history duration, and time-slice detail. Render styles have vector icons and retain
independent per-style settings when switching.
The render-style picker fills
the inspector's right-hand control column; its compact menu uses 176-point rows
with compact rows and flush selection styling. Its scroll-area height is
sized for the full seven-item list, while viewport constraints permit scrolling
only when the list cannot fit; both above/below placement and constrained-height
scrolling have interaction coverage.

`global_bar` owns the stereo/single-view display state, renders visible FFT/rate
controls, an advanced shared-options menu, and compact meters. Help is at the
far left. The UI passes the global channel view to every waveform pane, including
currently hidden ones, without resetting frequency histories. FFT/window/analysis
channel edits retain their existing history-reset behavior; display mode and rate
edits do not reset history. The analysis lane accumulates actual mixed-sample
RMS/peak alongside stereo levels, so cancellation is represented correctly;
the real-time capture callback is unchanged. Capture status remains available
through a colored indicator and the shared menu instead of the Live badge.
The waterfall's sidebar and viewport orientation gizmos operate on the same
camera state and share its projection math. Their Z-up axes map frequency to X,
time to Y, and level to Z. Axis clicks align or flip the view without changing
pan, zoom, or history. Gizmo dragging retains unrestricted orbit.
Named presets explicitly select all six orthographic sides without toggling.
Eight isometric presets use ±35.264° elevation and 45° corner increments for
equal axis foreshortening. Waterfall camera motion stores a starting view and a
motion recipe separately from runtime angles/phases, so animation cannot create
undo entries or dirty a preset. It integrates elapsed visible-frame time with
f64 phases and caps long resume gaps. Rotation supports the fixed frequency X,
time Y and level Z axes or a normalized custom direction. Rodrigues rotation of
the viewing direction and continuous Euler-branch selection keep the gizmos,
surface ordering and camera synchronized, including pole crossings. Continuous
orbits or sinusoidal bounded sweeps combine with optional figure-eight yaw/pitch
wobble. Speed/reverse changes preserve phase; zero orbit speed allows wobble
alone. The mesh cache includes rendered camera angles independently of saved
settings. Stopping or manually moving the camera captures the visible pose;
named views/reset stop animation. Legacy settings default to the original
level-axis orbit with no wobble. Capture timing and history are unaffected.
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
Optional frequency smoothing applies an edge-normalized triangular kernel
over linear power in neighboring display bands before the temporal response.
Radius zero preserves existing behavior exactly; radii 1–8 soften spectral
shape without altering the FFT. Retained histories replay this setting from
raw magnitudes. Musical-note labels use nearest equal-tempered notes with a
per-module A4 reference; label/tuning edits never reprocess history and do not
constitute pitch or key detection.

Waterfall and spectrogram histories retain at most 30 seconds / 901 snapshots
per active module, sampled at no more than 30 Hz. Monotonic timestamps set the
time axis independently of UI refresh rate. A bounded analysis-lane archive
collects these snapshots even when the window is occluded or another pane is
expanded. Every assigned history module consumes newly captured snapshots,
independently of painting; a returning window catches up from the latest 30 seconds.
Raw magnitudes use shared immutable arrays, so the archive and independent
module histories do not duplicate FFT buffers. Source/analysis resets advance a
capture epoch, clear queued old-source samples, and reject obsolete packets.
Manual pane pause skips archive ingestion and preserves its virtual clock.
Raw FFT magnitudes are retained
alongside display bands so frequency range, scale, gain, smoothing, and decay
changes reprocess existing history without a reset. The maximum FFT size uses
about 29 MiB of magnitude storage per history. Stale snapshots leave gaps;
capture format changes clear incompatible history.
Changing sources or shared FFT/window/channel settings clears pane histories.

Waterfall meshes are cached behind a shared immutable handle; unchanged paused
views reuse geometry rather than re-projecting/sorting/copying it. The key
includes sampled capture sequences, settings, camera, viewport rectangle, and
theme. Navigation, new data, style edits, expiry, and resizing invalidate it.
Cheap guides and interactive gizmos still draw normally. An ignored benchmark
compares cached requests with the identical uncached mesh builder, not total
GPU frame time.

Waterfall Appearance can use a gradient or a portable image material. Surface
mode interpolates image UVs across the frequency/time terrain on the existing
GPU mesh; Frequency × level mapping uses signal height instead of age. Both
axes can be mirrored. Base walls/floor inherit the material, while a two-row
white atlas gutter keeps mesh guides independent of image colors. Other render
styles sample the same image coordinates for their traces, points and bars.
The supplied starter landscape is bundled; custom PNG/JPEG/BMP files import via
the native macOS picker (Zenity on Linux). Import decoding, EXIF orientation,
resizing and PNG encoding run on a worker, bounded to 32 MB input, 8192 pixels
per source edge, 256 MB decode allocation, and 1024 pixels per stored edge.
Saved settings embed the normalized PNG rather than a machine-specific path;
decoded pixels and the GPU handle are never serialized. An additive
`image_removed` setting distinguishes an intentionally empty slot from the
legacy default starter. Removing an image cancels pending imports and releases
its GPU handle; replacing one retains the previous image on import failure.
An empty slot renders gradient colors until an explicit upload or starter restore.
Snapshots share an
immutable asset and reuse the upload until the image changes. Numeric parameter
documents omit the binary asset and restore it after modulation, avoiding image
encoding/decoding or payload copies on the per-frame control path. Selecting a
material never clears or reprocesses captured audio history.

The waterfall projects a bounded frequency/time surface on the UI lane and
submits one mesh to egui's existing GPU renderer. Surface cells are ordered
far-to-near by their ground-plane footprints rather than average 3D face depth,
which can incorrectly let tall peaks cover nearer valleys at isometric angles.
Each non-planar cell's triangles are ordered by the viewing ray's crossing of
their shared diagonal. Mesh edges travel with their owning triangle; facing
base walls follow the cell top and away-facing walls precede it. Regression
tests compare painter ordering against per-sample nearest interpolated depth
at all eight isometric corners, with walls on/off and stretched X/Y lengths.
Surface mode closes its perimeter
down to the fixed floor; line mode draws separate frequency traces. Camera
framing reserves the entire height range so changing height does not shift the
floor or clip peaks at default zoom. A fixed bounding sphere keeps scale and the
orbit center stable through full horizontal and vertical rotations, including
views from below the grid. Panning offsets the projected camera center in pane
coordinates, independently of zoom and rotation, and scales with pane resizing.
The waterfall defaults to two seconds and
supports 0.1–30 seconds, with independent surface-grid and floor-grid toggles.
A master guide switch also suppresses all axis lines, labels, hints, and the
viewport gizmo without overwriting the individual visibility preferences.
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
Stems adds floor-anchored, color-graded pins and point caps. Style-specific
spacing decimates rendered traces/points only; it never changes stored bands
or timestamps. Dot diameter and line thickness are screen-space controls.
Exact end-on line projections render finite square caps. Base walls can be
disabled separately from the surface; all modes leave missing history blank.
Bars adds solid, floor-anchored cuboids. Frequency groups use their highest
processed level, and time spacing selects every Nth retained slice without
changing history. Width/depth percentages keep footprints within their slots
and the overall X/Y extents. Cuboids are ordered by disjoint ground footprints,
not height, and only their three camera-facing faces are emitted, including
bottom caps for views from below. Shaded sides use the existing palette and
contrast mapping. Tests verify grouping, spacing, dimensions, gaps, and
nearest-depth visibility across all eight isometric corners.
The shared Heatmap palette interpolates seven blue-to-red color stops using
the same normalized level as waterfall height. It is independent of desktop
theme colors and camera/geometry settings; the module's dB floor and ceiling
define its signal range. It also applies to the spectrogram without changing
capture or retained history.
Heatmap contrast applies a symmetric power curve to color coordinates only,
preserving cool/warm endpoints and the midpoint. It defaults to 1 and is
independent per waterfall/spectrogram; other palettes and height are untouched.
Slider and scroll zoom share a 0.5×–10× range; close-ups are clipped to the pane
and existing pan gestures allow navigation without changing geometry or history.
History length, height, time-slice detail, palette, and camera controls are local
to the waterfall. Hidden panes do not render, but continue accumulating history
from the analysis-side archive. No audio callback work was added.

## Frequency analysis

- Legacy diagnostic log bands use an ordered single pass over FFT bins, preserving
  the original boundaries and maxima without rescanning all bins for every band.
- The target Rate determines the analysis hop independently of FFT size; small
  FFTs no longer force excess transforms just to maintain an overlap fraction.
  Capture and waveform history still receive every sample. Large hops can leave
  gaps between FFT windows; overlapping windows occur when the hop is smaller.
- Experimental BPM autocorrelation runs approximately twice per second while
  every frame's onset flux is retained. Rate/format changes reset its timing state.
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

## Platform capture boundary

`src/audio.rs` defines source/status/event types and selects `audio/linux.rs`
or `audio/macos.rs` at compile time. The Linux implementation retains its
PipeWire callback, selection rules, native features, and command loop. Source
fields still use the original `global_id`/`node_name` names for compatibility;
Mac uses a stable `coreaudio:system` identifier. Generalizing these names can
follow when more Mac sources or another backend require it.

On Mac, a control worker owns a private Core Audio tap and aggregate device.
`macos_capture.m` handles native lifetimes and exposes a small C ABI. Its IOProc
forwards borrowed Float32 pointers and strides to Rust; Rust copies into the
bounded SPSC ring, counts drops, and sanitizes nonfinite values. No FFT, logging,
allocation, mutex, or Objective-C messaging occurs in the sample callback.

Each reconnect stops the previous native session and allocates a separate ring
and analysis runtime. The new runtime shares the existing analysis controls,
but never inherits samples or history from a previous format. The UI adopts
that runtime via a control event. Tap format/default-output changes request a
reconnect; a stalled stream shows a waiting state and can be retried manually.
Shutdown stops the IOProc and unregisters property listeners before releasing
the callback context. If Core Audio teardown fails, the context is deliberately
retained and an error is logged rather than freeing memory still held by the OS.

Omarchy theme lookup runs only on Linux. Mac uses Symphos Dark and writes bounded
diagnostics under `~/Library/Logs/Symphos`. See [Mac development](macos-development.md)
for current coverage and unresolved device/permission scenarios.

## Tiled workspace and preset state

Scene preset saves prefill a fresh, editable three-word name and append a new
library entry atomically. Only a successful save adopts that name in the scene
bar. Optional scene metadata retains its preset identity and a stable creative
state fingerprint, so subsequent changes show `(edited)` and Undo can return to
the clean state. Inspector navigation, dock visibility and automatic camera
motion are excluded. Fingerprints are checked only during edit transactions,
at most ten times per second while dragging; idle rendering does no extra work.
Preset fingerprints serialize as fixed-width hexadecimal metadata. Legacy u64
fingerprints remain readable. This prevents opaque identifiers from failing
the bounded numeric-parameter checks used by workspace recovery.

`workspace.rs` models pane placement as a binary split tree with stable component
slot IDs. Closing a leaf promotes its sibling; moves keep the same component
instance and its settings. Open panes ingest the shared capture stream even while
another pane is expanded. Closed slots retain settings and discard histories.

Layout-only presets contain canonical geometry without module configuration.
Full-view presets add explicit settings-only pane snapshots. Component presets
apply a matching module snapshot without changing geometry or shared analysis.
`workspace_ui.rs` provides the component library, drag payloads, preset browser,
and save/manage dialogs; pane mutations occur after dashboard rendering.
`dropdown::ComboBox` applies the shared panel-derived trigger/popup colors,
compact text and flush menu rows through a scoped style. Visualizers, Signals,
MIDI, settings and recording use this same wrapper without changing unrelated
buttons or fields.

Versioned session and preset JSON files are stored separately and replaced
atomically. Collection, tree, and allocation bounds are checked before restoring
state. Normal app data uses the platform configuration directory; live reload
data remains under `target/dev`. Session version 2 migrates the earlier four-pane
development checkpoint. See [workspace layouts and presets](workspace-layouts.md)
for the user-facing behavior and file locations.
