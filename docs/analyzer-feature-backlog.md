# Analyzer feature backlog

This inventory records analyzer, metering, capture, and workspace capabilities
that are useful reference points for Symphos. It was initially assembled from
the public MiniMeters feature set, but it describes Symphos outcomes rather than
promising interface or implementation compatibility with another product.

Status meanings:

- **Shipped** — present in the current source tree.
- **Planned** — accepted direction and assigned to a roadmap phase.
- **Evaluate** — useful, but its design or fit still needs investigation.

## Waveform

| Capability | Status | Symphos direction |
| --- | --- | --- |
| One or two independent waveform channels | **Shipped** | Each pane selects one signal or any pair independently. |
| Left, Right, Mid, and Side signals | **Shipped** | Mid is `(L + R) / 2`; Side is `(L - R) / 2`. |
| Static channel colors | **Shipped** | Theme accent colors distinguish lanes. |
| Multi-band waveform colors | **Shipped** | Trace color follows display-only low, mid, and high energy. |
| Level color map | **Shipped** | The selected palette maps to instantaneous waveform amplitude. |
| Multi-band peak history | **Shipped** | Low-, mid-, and high-band peak envelopes can be overlaid behind the trace. |
| Adjustable waveform speed | **Shipped** | The across-pane duration controls travel speed independently of FFT size. |
| Scrolling and static loop modes | **Shipped** | Scroll keeps now at the right; Loop overwrites fixed positions at a wrapping write head. |
| DAW timecode overlay | **Planned** | Add after a DAW transport/audio bridge establishes a reliable clock source. |

## Metering and analysis

| Capability | Status | Symphos direction |
| --- | --- | --- |
| Calibrated VU meter | **Planned** | Add selectable calibration, VU ballistics, and peak/clip indicators. |
| Standards-based loudness | **Planned** | Add LUFS momentary and short-term alongside configurable fast/slow RMS. |
| Stereo vectorscope | **Shipped** | Scaled, one-to-one rotated linear, and Lissajous views include static, RGB, and overlaid low/mid/high rendering. |
| Phase correlation | **Shipped** | Overall and separate low/mid/high correlation are available with an RMS balance indicator. |
| Pitch-following oscilloscope | **Shipped** | Local autocorrelation pitch locking, threshold trigger, free run, cycle controls, component routing, and afterglow are available. |
| Per-module Mid/Side spectrum analysis | **Planned** | Extend independent channel routing beyond waveform panes. |
| Reference spectrum / target curve | **Planned** | Capture a live reference and load supported audio files for comparison overlays. |
| Mel frequency scale | **Evaluate** | Compare it with the existing linear, logarithmic, and note-label views. |
| Piano-key spectrogram overlay | **Evaluate** | Add if it improves pitch reading beyond note axis labels without obscuring history. |
| Multiple spectrogram analysis modes | **Evaluate** | Compare classic STFT rendering with reassigned/sharper time-frequency modes. |
| Measurement cursor | **Planned** | Pin frequency/note, level, and time readouts on frozen frequency views. |

## Capture and DAW interoperability

| Capability | Status | Symphos direction |
| --- | --- | --- |
| Record captured audio | **Planned** | Record the selected source without blocking the realtime capture callback. |
| Recover the previous 10 or 60 seconds | **Planned** | Maintain a bounded audio ring distinct from the existing spectral history. |
| DAW audio-sender bridge | **Evaluate** | Investigate a small sender before committing to full plug-in versions. |
| VST3, CLAP, and Audio Unit meters | **Evaluate** | Revisit after standalone routing and the component contract stabilize. |
| Per-component audio sources | **Planned** | Replace the current one-source-per-application restriction. |
| Application and virtual sources | **Planned** | Support PipeWire application streams, loopbacks, and virtual cables where available. |
| Syphon window output on macOS | **Evaluate** | Consider with the future macOS renderer and capture backend. |

## Workspace, presentation, and persistence

| Capability | Status | Symphos direction |
| --- | --- | --- |
| Saved module presets | **Planned** | Persist individual component settings in a versioned format. |
| Saved workspace layouts | **Planned** | Restore pane assignments, routing, window positions, and display-aware layouts. |
| Detached/pop-out components | **Planned** | Support docking, tiling, detaching, fullscreen, and multi-display placement. |
| Horizontal desktop bar / stick mode | **Evaluate** | Prototype after detached windows work reliably on Wayland. |
| Transparent visualization backgrounds | **Evaluate** | Add explicit transparency controls without compromising text and guide contrast. |
| User-authored color maps | **Planned** | Save reusable gradients rather than limiting modules to built-in palettes. |
| Windows and macOS builds | **Planned** | Add platform audio backends, packaging, CI, signing, and output-capture support. |

## Delivery order

1. Finish waveform validation and independent component routing.
2. Add loudness and calibrated VU modules; stereo-image and phase-correlation
   are shipped.
3. Add persistent component presets and reference-spectrum workflows.
4. Add bounded audio recording and retrospective clips.
5. Establish detached workspaces and a DAW bridge before evaluating full meter plug-ins.
6. Expand platform and video-output integrations after the core contracts stabilize.

## Reference

- [MiniMeters modules and platform feature table](https://minimeters.app/),
  reviewed September 9, 2026.
