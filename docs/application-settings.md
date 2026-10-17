# Application Settings

Open Settings with the top navigation gear or **Cmd+,** on macOS / **Ctrl+,** on
Linux. The footer settings button opens the same window at **Analysis & Meters**.
Drag the Settings title bar to move the window; its position is constrained to
the app viewport. Close with the X, Escape, or a click outside. Settings blocks
workspace interactions while open, but capture and visualization continue.
Escape first dismisses an open dropdown, then Settings; a fullscreen pane stays
fullscreen until a subsequent Escape.

Changes apply immediately. Categories and window position are remembered while
the app is running. Theme and zoom preferences are checkpointed with the last
workspace and restored on launch, including development rebuilds. The window
itself starts closed.

| Category | Controls |
| --- | --- |
| Display & Layout | Interface zoom (60–200%), default zoom, application fullscreen, component inspector, Info View, balance pane sizes |
| Theme & Colors | App skin previews and selection |
| Audio | Capture source, connection status, format, signal level, retry capture, platform capture guidance |
| Analysis & Meters | FFT size/window/channel, update rate, meter layout/channel, reset peak and clip hold, dropped samples, FFT diagnostics |
| Help & About | Keyboard shortcuts, issue log, version/build, platform, current theme |

## App themes

**Automatic**, **Symphos Dark**, **Studio charcoal**, **Live dark**, **Graphite**,
**Midnight**, **Plum**, and **Ember** are available. Studio charcoal draws on
Resolve's cool charcoal workspace and restrained blue highlights; Live dark uses
Ableton-inspired flat gray surfaces and amber controls. Each skin sets the GUI's
surfaces, navigation, text, control accents, selection colors, dialogs, and
performance bar. These are dark skins. The operating system controls the native
window title bar.

GUI skins do not recolor visualizations or change their chosen palettes, camera,
parameters, capture history, or frozen state. Visual backdrops and default signal
colors continue to use the automatic platform/development palette. Choosing a
component gradient or swatch is a separate action in its Appearance tab.
**Edit selected component colors** opens that tab; see the
[component color library](component-colors.md).

Automatic preserves existing platform behavior: Omarchy's active palette on
Linux, the default Symphos palette on Mac, and `target/dev/theme.json` for the
supervised development build. Automatic continues to refresh; explicit skins
remain selected even if the platform/development palette changes.

App appearance belongs to workspace preferences, independently of layout,
full-view, and component presets. Loading a visualization preset does not change
the app skin or interface zoom. Existing workspaces without appearance fields
retain Automatic and their previous default scale.

## Analysis and audio

Changing FFT size, window, or analysis channel clears capture histories and resets
module analysis state, preserving the existing interpretation rules. Changing the
analysis update rate, meter layout/channel, theme, or zoom does not clear histories.
Retry capture starts a new capture epoch. Selecting a source uses the same backend
as the top navigation selector.

Quick FFT size, update rate, and stereo-meter controls remain in the footer. The
right-hand component inspector still handles each component's visual and audio
mapping parameters; Settings handles shared application preferences.
