# Workspace layouts and presets

Symphos opens with the default 3D waterfall above three analyzer panes on its
first launch. Later launches restore the last workspace, including its component
choices, settings, dividers, sidebar, and selected preset. Choose **Default view**
in the top preset browser whenever you want the original view again.

## Component inspector

The component inspector docks on the right and starts open on first launch.
Each pane’s top-right eye button selects that pane and opens its inspector; **I**
opens the selected pane’s inspector, and **N** toggles it. The far-right navbar
button also toggles the inspector and highlights when it is open. The inspector header X
closes it. Selecting a visualization never opens a hidden inspector automatically.

All sections appear in one scrollable panel with collapsible disclosure headers,
open by default. Each pane/module retains its own disclosure and scroll state
throughout the session. There is no separate section-icon rail. Major sections have full-width divider
lines and disclosure headers; quieter labels group related controls within them.
The pinned header shows the component icon, name, visualization type, panel
number, live/paused status, and capture sample rate. Numeric fields use full-width
tracks below their labels, with a slim accent indicator and exact value entry.

The inspector surface meets the navbar, right window edge, and bottom window
edge without outer padding. A gap on its left separates it from the plots.
Opening it resizes the visualization area. Drag its left edge to change width, or
double-click that edge to reset. Visibility and width restore with the last
workspace. Info View remains independent at the bottom left.

## Arrange panes

Panes stay tiled inside the application. Drag a divider to resize its neighboring
sections, or double-click it to make that split equal. The layout preserves usable
pane sizes when space permits and adapts its proportions to smaller windows.

Click a pane's header **X** to close it. Its sibling pane or section expands into
the freed space; unrelated split proportions stay intact. Closing every pane
shows **Add a component**. The workspace supports up to 16 open panes.

Drag the dotted grip at the left of a pane header to rearrange it. Drop near
another pane's left, right, top, or bottom edge to split that pane and move the
existing component there. Drop in its center to swap the two pane positions.
Settings and histories travel with the moved component. Escape cancels a drag.

Each header has separate eye and fullscreen buttons. The eye opens the inspector.
Fullscreen fills the app window with that panel and hides the navigation, inspector,
and footer. Escape or the restore icon returns to the previous workspace. A brief
“Press Esc to exit fullscreen” hint fades away after three seconds. F toggles panel
fullscreen; F11 toggles native application fullscreen. Compact panes hide pause
and the drag grip first, keeping fullscreen, inspect, and close accessible.

## Add components

Open **New component** in the top bar. The browser docks flush on the left, using the inspector’s header, field styling,
component icons, and full-width dividers. It has text search and Frequency,
Time, and Stereo categories, with room for additional components in future builds.

- Click a library card to add that visualization beside the selected pane.
- Drag a card to a pane edge to add a new split there.
- Drag a card to a pane center to replace its visualization.

An overlay previews the target split or replacement during dragging. Adding and
rearranging components never starts another audio capture device; each open
component uses the existing shared analysis stream. Components hidden temporarily
by pane expansion keep ingesting capture. Closed components release their
histories.

## Browse and save presets

The top bar shows the selected preset's name and previous/next arrows. Click the
name to search presets by category. Layout thumbnails preview the arrangement.
An asterisk indicates changes since the selected preset was loaded.

Click **Save**, enter a name, and choose a preset type:

| Type | Saved content | Applying it |
| --- | --- | --- |
| Layout only | Pane count, split arrangement, and divider proportions | Keeps existing component choices and settings in visual order |
| Full view | Layout plus component choices and their settings | Restores the saved component configuration |
| Component | Settings for the selected visualization | Applies to a selected pane of the same type without changing the layout |

Layout-only presets have no module configuration in their saved data. If the new
layout needs fewer panes, extra components become hidden slots with their
settings retained. A later larger layout reuses those slots. If more slots are
needed, Symphos adds default frequency-spectrum components. Adding a fresh
component from the library may reuse a hidden slot with fresh settings.

Full views and component presets leave shared FFT settings, audio source,
capture permissions, and routing unchanged. They do not contain audio samples,
captured histories, frozen frames, or textures.

User presets have a **···** button in the browser for rename and delete. Factory
presets remain available. Deleting a user preset leaves the current workspace
intact. Names must be unique within their category.

## Saved files

The regular app saves `workspace.json` and `presets.json` under:

- macOS: `~/Library/Application Support/Symphos/`
- Linux: `$XDG_CONFIG_HOME/symphos/`, or `~/.config/symphos/` when unset

The live reload app uses `target/dev/` instead, so its workspace and preset
library are separate. Settings are checkpointed during use and on graceful exit;
the last workspace also survives rebuilds. `cargo clean` removes dev state.

Workspace schema version 2 migrates the earlier four-pane development workspace.
The user preset library has its own versioned schema. Invalid layouts, duplicate
pane IDs, oversized collections, and unsafe allocation settings are rejected.
Rejected files are preserved beside the originals as `workspace.rejected.json`
or `presets.rejected.json` before falling back to defaults.

## Validation

Tests cover split collapse, move/swap operations, nonoverlapping geometry, legacy
workspace migration, settings-only serialization, component preservation during
layout changes, preset-library recovery, and last-session restoration. Egui
pointer-interaction tests exercise library drag/add, center replacement, pane
movement, and closure through the empty-workspace state.

The Mac scratch app was checked for fresh-launch defaults, component addition,
close-and-fill, saving a layout-only preset, preset navigation, and restoration
after replacing its executable. Mac and Ubuntu 24.04 Linux checks passed formatting, Clippy, normal/development
tests, and locked release builds. The Mac bundle also passed local signature
verification. Real Omarchy graphics and device validation remains a separate hardware
check; the PipeWire and Core Audio capture implementations are unchanged by this
workspace feature.
