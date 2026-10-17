# Implementation prompt: musical signal sources and visualizer mapping

Use this document as the implementation request for the next model. It captures
our agreed direction, current code, first milestone, and acceptance criteria.

## Start condition and Git workflow

The user wants the current work reviewed and merged before implementing this
feature. The prerequisite is the PR from `codex/visualizer-modulation` into
`pat-riley/symphos:main`, containing visualizer modulation, Flux performance
macros, compact inspectors, color libraries, GUI themes, and this prompt.

Before changing implementation code:

1. Confirm that prerequisite PR is merged, or obtain the user's explicit
   confirmation and verify that the remote main contains its changes.
2. Inspect repository instructions and Git status. Preserve unrelated user work;
   do not discard, blindly stash, or reset it.
3. Fetch and update main, then create a fresh branch, such as
   `codex/musical-signal-sources`, based on the merged main.
4. Read the current implementation and documents listed below. State your first
   implementation slice, then carry it through code, checks, and native UI
   verification. Do not start a second thread or change the user's model.
5. If the prerequisite is still open, stop implementation and explain that the
   user explicitly asked for the merge first. Planning and read-only inspection
   can continue. Do not merge the PR on the user's behalf.

Later user replies override this document. Do not repeatedly ask questions
already answered in the conversation.

## Product intent

Symphos is a native Rust/egui audio analyzer and artistic visualizer, currently
bespoke for its creator. It should grow through useful abstractions and reusable
projects without becoming a general-purpose programming environment.

The app needs two working spaces:

- **Signals**: obtain inputs, extract musically useful information, shape it,
  and inspect named channels in a mixer-like view.
- **Visualize**: arrange components, choose which signals drive their behavior,
  and perform with each visualizer's bespoke macro panel.

Settings handles global capture/device configuration. A signal-strip fader
controls visual influence; this milestone does not mix or play audible audio.

A source in the user's language is a configurable musical driver: kick
activity, vocal intensity, melody movement, hits, or impact. Its underlying
input and extraction method may vary. A full system mix requires detection or
separation; an isolated Ableton track needs less extraction; an OSC stream that
already supplies the intended control can bypass audio analysis.

Keep the product interface concrete: choose a source template, configure it,
watch its output, and assign it. No node canvas, scripting, arbitrary graphs,
or audio-effects insert chains are required.

## Confirmed direction versus provisional defaults

Confirmed:

- Start with the app's existing system-audio capability and preserve macOS and
  Omarchy Linux behavior.
- Signals should support configurable extraction rather than only fixed
  bass/mid/high frequency bands.
- Input adapters should accommodate future Ableton audio/data, MIDI, OSC, and
  file-backed inputs without rebuilding the visualizers.
- Frequency selection, input gain, detector settings, smoothing, and envelope
  shaping primarily belong to a reusable signal.
- Components select signals and map them to their own parameters/macros.
- Scientific instruments must retain accurate raw measurement paths.
- Each artistic visualizer retains its own high-level performance interface.
- Save/reopen creative setups is a future goal to address in this milestone's
  persistence design.

Three final questions were presented before the handoff. Check for the user's
answers. Until revised, the proposed defaults are:

1. First milestone implements current audio plus lightweight detectors;
   Ableton/MIDI/OSC connections and stem separation are later integrations.
2. A project saves inputs' logical references, signals, mappings, components,
   layout, palette data, and base macro values together. GUI themes and physical
   device preferences stay separate.
3. Disconnected inputs fade their influence to zero, then resume on reconnection.
   Define and document the fade and freshness timing; do not apply a sudden jump
   or hold stale control data indefinitely.

These are proposals, not claims that the user explicitly approved each detail.
Routine implementation choices may follow these defaults after the merge unless
new user guidance changes them.

## Baseline to preserve

Read:

- `src/audio.rs`, `src/audio/macos.rs`, `src/audio/linux.rs`, and
  `src/audio/demo.rs`: capture providers and development audio.
- `src/analysis.rs`, `src/capture_history.rs`: measured frames, epochs, and
  histories that continue independently of visible panes.
- `src/modulation.rs` and `src/modulation/ui.rs`: current source extraction,
  per-route response state, graphical spectrum/ADSR editors, target accumulation.
- `src/modules/flux.rs` and `src/modules/flux/performance.rs`: bounded particle
  simulation, cloud/orbit behavior, Impact accents, and twelve stable macros.
- `src/modules/strata.rs` and `src/modules/veroni.rs`: artistic consumers.
- `src/modules/mod.rs`: settings snapshots, pane pause state, compact icon tabs.
- `src/app.rs`, `src/app/settings.rs`, `src/workspace_ui.rs`, `src/workspace.rs`:
  application composition, preferences, presets, and session validation.
- `src/colors.rs` and `src/theme.rs`: component gradients versus GUI skins.
- `docs/audio-modulation.md`, `docs/flux-performance.md`,
  `docs/application-settings.md`, `docs/component-colors.md`,
  `docs/workspace-layouts.md`, and `docs/realtime-development.md`.

Current mappings bundle extraction, shaping, and destination into each route.
An immutable, shared `AudioFeatures` snapshot already supports band queries
without rescanning the FFT for every route. Reuse this work deliberately.

Preserve tiled panes, resizing, close-and-fill, pane fullscreen/Escape, preset
navigation, scientific/visualizer families, live reload, inspector icon tabs,
color presets, and the separate GUI skin system. Keep the original/default
visualization arrangement and existing projects usable after migration.

## Domain model and signal path

Separate these concepts in code even when the interface uses familiar source
and channel terminology:

1. **Input/provider**: supplies raw audio, control values, events, or transport
   information. A connection configuration is different from a musical signal.
2. **Extractor**: obtains meaningful information from an input. Examples are
   band energy, onset strength, stem level, or an already supplied control value.
3. **Response**: calibration/normalization, gate or sensitivity, gain, follower
   smoothing, trigger hold/refractory timing, and appropriate envelope shaping.
4. **Named signal**: reusable identity and observable output, such as
   `Bass hits` or `Overall intensity`.
5. **Mapping**: connects a signal to a component/macro with amount, inversion,
   limits, and the relevant target semantics.
6. **Performance macro**: component-specific base or influence control. It
   remains playable while musical modulation continues underneath.

Example:

`System playback → low-band transient detector → trigger envelope → Kick activity
→ Flux Impact / Veroni expansion / Strata brightness`

The same `Kick activity` signal could later use an isolated kick track or
explicit OSC trigger. Its mappings should survive replacing the extraction
method when the output contract remains compatible.

### Output contracts

Preserve types and meaning. At minimum distinguish:

- Unipolar continuous intensity, normally 0–1.
- Bipolar controls, such as signed stereo balance.
- Discrete events carrying timestamp and strength.
- Raw audio/spectral measurement frames for scientific instruments.

Allow future pitch/note and transport/beat data without pretending all inputs
are audio or silently collapsing every type into one float. Most visual macros
can use normalized controls, but event timing, polarity, units, and freshness
must remain available where needed.

A raw audio frame is not interchangeable with a normalized control channel.
Scientific components must not accept MIDI/OSC scalar data as if it were a
measured spectrum. Show only compatible mapping choices and explain unavailable
ones plainly.

Use stable IDs for inputs, signals, component instances, and destinations.
Renaming/reordering a channel must not break links. Deleting one must not silently
redirect a mapping to a different signal with the same name or list index.

## First milestone

Deliver a functioning vertical slice using the existing capture stream. Do not
open new device, network, or DAW connections in this milestone unless the user
explicitly expands scope.

Provide ready-to-use signal recipes:

- **Overall intensity**: full-mix level with adjustable response.
- **Band energy**: user-selected frequency range and level following.
- **Bass hits**: configurable low-frequency transient/onset detection with
  threshold, retrigger spacing, and optional pulse envelope.
- **Spectral change**: a clearly defined measure of frame-to-frame spectral
  change, with tunable sensitivity/response.
- **Stereo balance**: preserve the existing signed control and level gating.

Keep sustained level and hit events distinct. Follower mode uses attack/release;
full ADSR applies to gated/triggered response modes rather than displaying
inactive, misleading controls on every input.

Do not label a frequency filter as true vocal, melody, or kick isolation.
Bass hits is an estimate from a mixed recording. Distinguish provenance in the
UI: estimated from mix, supplied isolated audio, or direct external control.
Only expose confidence percentages if an actual algorithm supplies meaningful,
validated confidence. Do not fabricate confidence from signal amplitude.

Actual stem separation, robust polyphonic melody extraction, instrument
classification, beat-grid synchronization, and song-section/drop recognition
need separate evaluation. They may have latency and processing costs; the
architecture should leave room for measured latency, delayed results, and
file preprocessing without promising real-time feasibility now.

## Signals workspace UX

Add top-level **Signals** and **Visualize** navigation without losing the current
workspace state when switching. Use the existing skin, typography, compact
controls, hover help, and reusable spectrum/envelope widgets.

Signals should resemble a compact Logic-style mixer in organization:

- Named, reorderable strips with type/provenance icon and connection status.
- Input and extraction-method selection.
- Output activity/trigger meter, enable/mute, and an influence/trim fader.
- Add, duplicate, rename, and delete actions.
- A right-hand inspector for the selected signal's detailed configuration.
- A visible list or count of linked component/macro destinations.

Use horizontal scrolling or another bounded layout for more strips. Keep
meters and strip controls legible at the app's minimum window size and saved
UI scales. Channel configuration should remain available while signals run.

Selecting Band energy or Bass hits reveals frequency selection and relevant
detection settings. Direct control inputs later reveal address/channel/range
settings instead. Avoid exposing irrelevant FFT or ADSR controls.

Show input activity and shaped output separately so the user can diagnose
whether the issue is capture, extraction, shaping, or mapping. Show inactive,
stale, missing, and disconnected states explicitly.

Global source/device selection in Settings remains the single configuration
for the current capture input. A strip chooses a logical available input; it
must not unexpectedly replace the app's physical capture source. Keep any
navbar device shortcut synchronized with Settings or make it open that page.

No audible panning, audio sends/buses, recording mixer, or playback output is
implied by the strip design. Label normalized visual influence accurately.

## Visualize workspace and mappings

Keep the current component arrangement and right inspector. Replace repeated
source extraction controls with a named signal picker and live activity preview,
then destination, amount/depth, inversion, and output limits.

Provide **Edit signal** to jump to the selected channel configuration and
**Duplicate for this component** when a different detector or envelope is needed.
Changing a shared signal should clearly identify which consumers it affects.
Do not hide a second full detector or stacked ADSR behind a mapping by default.
A future local override can use pre-shaped features explicitly rather than
accidentally applying the same envelope twice.

Make default components immediately useful with starter channels and mappings.
Users should not have to build an entire signal setup before seeing movement.

Map signals to stable bespoke macro/parameter IDs. Flux's catalog already offers
normalized getter/setter ranges and twelve stable IDs. Support its useful macro
set intentionally, including Impact's event behavior; do not merely rename the
existing seven modulation targets and claim all twelve macros are routable.
Retain meaningful targets for Veroni and Strata without forcing identical macro
panels on every visualizer.

Manual controls and musical modulation must coexist. Keep the saved/manual base
value distinct from the runtime effective value. Define additive modulation,
amount scaling, polarity, clamping, and event dispatch explicitly. Do not write
live modulation back into settings or make knobs fight incoming data. Display
base value and live influence where useful. Preserve manual Burst and current
pause behavior.

When multiple mappings share a destination, keep combination predictable and
bounded. An incompatible or missing source must not produce jumps, NaN values,
or an unintended constant offset.

## Scientific instruments and Strata layer bands

Waterfall, spectrum, waveform, spectrogram, stereometer, and oscilloscope continue
to use measured audio and their own display/analysis controls. A waterfall's
frequency display range stays in its inspector; artistic signal shaping must not
silently alter its data or calibration.

An isolated audio input could eventually supply a scientific component, but an
extracted scalar intensity cannot replace its waveform or FFT. Preserve this
separate measurement path.

Strata currently distributes logarithmic frequency ranges over its layer count,
with independent envelopes per layer. Preserve automatic redistribution when
layers change. Do not collapse the layer bank into one shared scalar. Support a
compatible band-bank contract or retain the existing automatic layer mapping
path until a correct migration is available; document that boundary honestly.
Different component layer counts must not mutate a shared signal's configuration
behind the user's back.

## Runtime, timing, and performance

Evaluate shared signals once on a consistent clock and publish immutable,
timestamped results that multiple components can consume. Keep extractor and
envelope state outside renderers. Source processing should continue when a pane
is hidden, a different tab is selected, or another component is paused.

Pausing a component freezes its consumed visual/control state while other
consumers continue. Resuming must follow a defined current-state policy without
replaying accumulated triggers or bursting due to a large elapsed time.

Source replacement, capture epoch changes, incompatible FFT changes, and method
changes should reset only the necessary state. Editing one signal must not clear
other channels or unrelated visualization histories. A global capture change
still obeys existing history/epoch rules.

Keep allocation, blocking I/O, UI work, and heavy computation out of capture
callbacks. Reuse cached FFT features and bounded histories. Establish budgets
for signal count and per-step work. Measure the added extraction cost and its
impact on the existing particle/geometry renderers before claiming performance.

Future providers may update at different rates. Define timestamps, freshness,
event IDs, and stale-data behavior now. A musical event must fire once per
connected consumer rather than once per draw call; delayed frames must not
create duplicate triggers. External normalized values still need declared
meaning, range validation, optional smoothing, and disconnection policy.

## Projects, presets, and migration

Use a versioned project format; a human-readable `.symphos.json` document is a
reasonable initial choice. Save the creative setup:

- Logical inputs and extraction configurations.
- Signal IDs, names, ordering, calibration, response, and influence values.
- Component instances, layouts, signal mappings, palettes, and base macro values.
- Necessary referenced asset metadata for future file-backed sources.

Keep GUI theme, UI zoom, machine-specific device preferences, and local recent
project paths separate. Use logical aliases such as system playback so opening
on another machine can resolve or request a binding instead of retaining a
foreign device ID and silently selecting the wrong source.

Exclude samples, captured histories, particle positions, active envelope state,
queued triggers, and transient meters. Never embed secrets or future network
credentials in portable project files.

Preserve layout-only, full-view, and component presets. A component preset with
signal dependencies must either carry the necessary signal definitions or
explicitly request compatible binding; no silent broken references. Define
import ID collision handling and the difference between project identity and
reusable signal recipes.

Migrate existing workspaces and mappings without discarding palettes, camera,
geometry, macros, or layouts. Initially create independent signal definitions
for existing routes where needed, preserving their separate settings. Do not
automatically merge identical-looking routes into shared state: changing one
would then unexpectedly change another. Sharing should be explicit.

Retain invalid/unsupported documents for recovery, validate IDs, references,
finite values and bounded arrays, and save atomically. Add save/open/reload
coverage, not only serialization tests. Keep the live development workspace and
libraries isolated from the regular app.

## Future integrations: prepare contracts, defer implementations

Separate adapter concerns from extraction and mapping so these can be added:

- Isolated audio tracks or stems from Ableton or another host.
- MIDI CC, notes, clocks, and controller assignments to bespoke macros.
- OSC values and triggers, with explicit addresses, ranges, and timestamp policy.
- Audio files, MIDI files, or recorded control streams with a transport clock.
- Optional stem separation or more sophisticated musical event detection.

Do not assume Ableton Link provides all track audio, levels, notes, and automation.
Research the actual transport/API for the requested information when that
integration is authorized. Keep tempo synchronization, audio delivery, and
parameter/control delivery distinct. Do not add placeholder UI that pretends
unimplemented integrations are connected or usable.

## Suggested implementation slices

1. Audit current route state, pause semantics, macro IDs, and persistence.
   Document the chosen signal contracts and migration plan.
2. Introduce input references, named signal definitions, runtime snapshots, and
   cached processing using existing capture/features. Cover algorithms and timing.
3. Add the Signals workspace with starter recipes, working editors, meters,
   and channel management. Preserve the Visualize workspace when switching.
4. Connect named signals to visualizer targets/macros, including shared versus
   duplicated source workflows. Preserve scientific measurements and Strata banks.
5. Add complete project save/open and preset dependency handling. Verify migration
   against real existing schema fixtures and the saved development workspace.
6. Verify the live app, listening to different material and inspecting silence,
   sustained bass, percussive input, source changes, and source loss. Document
   usage, limitations, and measured performance.

Do not stop after a data model or static mixer mockup. Complete a usable first
milestone. If a larger integration changes scope, explain the concrete boundary
rather than silently adding network/DAW functionality.

## Verification and acceptance criteria

The implementation should demonstrate:

- Two components can use one named signal and react to the same shaped output.
- Editing a shared detector affects those consumers; mapping amount remains local.
- Duplicating a source gives independent configuration and preserves other links.
- Switching tabs, reordering/renaming strips, pause, hidden panes, and frame-rate
  changes do not change detector timing or produce duplicate hits.
- Synthetic tones fall in the intended bands; impulse/sustained/silent inputs
  exercise follower, gate, and transient behavior correctly.
- A band-energy detector never masquerades as reliable instrument separation.
- Missing/stale sources follow the documented policy and recover safely.
- Macros remain playable, bounded, and separate from saved base values.
- Scientific readings and Strata's automatic layer bands retain their contracts.
- Project save/open restores the creative setup; old workspaces and presets
  migrate without losing state or creating dangling references.
- GUI skins and visualization palettes remain separate; existing controls and
  live reload remain usable on Mac and Omarchy Linux.

Run `./scripts/dev.sh --check`, relevant signal/migration/integration tests, and
appropriate manual timing probes. Perform native UI checks for width/zoom,
mixer overflow, mappings, save/reload, and source status. The existing dev
supervisor supports rebuild/relaunch and generated stereo audio. Reuse a running
supervisor instead of starting a competing one.

CI covers macOS and Linux compilation/tests, but report any lack of a real
Omarchy PipeWire/GPU session. Do not claim hardware testing that did not occur.
Document sources for new algorithm or protocol choices and avoid relying on
unverified product capabilities.

## Research context

These sources motivated the abstraction; they are not commitments to adopt a
specific dependency or reproduce another application's implementation:

- [Ableton Max for Live modulators](https://www.ableton.com/en/live-manual/12/max-for-live-devices/):
  reusable modulators, multiple mapped destinations, and per-destination ranges.
- [Resolume parameter animation](https://resolume.com/support/en/parameter-animation):
  direct audio analysis with frequency selection and response shaping.
- [TouchDesigner audio analysis](https://derivative.ca/UserGuide/Palette%3AaudioAnalysis):
  distinct sustained, percussive, and spectral musical features.
- [librosa onset detection](https://librosa.org/doc/0.11.0/generated/librosa.onset.onset_strength.html):
  onset strength derived from spectral change, distinct from source separation.
- [Demucs research implementation](https://github.com/facebookresearch/demucs):
  learned music stem separation. The repository is archived; evaluate current
  maintenance, compute, latency, and licensing before selecting any implementation.
