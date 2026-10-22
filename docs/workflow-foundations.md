# Editing, controller mapping, and signal shaping

## Creative undo

**Edit > Undo / Redo** and Command-Z / Command-Shift-Z restore creative edits:
component parameters, scene changes, signal settings, macro assignments, and
controller mappings. A continuous drag is one edit; nearby typing is grouped.
Text fields retain their native local undo behavior while focused.

The in-memory journal retains up to 64 transactions and is cleared on New/Open
Project. It is separate from crash recovery and project saving. Device selection,
application preferences, pane selection, and inspector navigation are not
creative transactions. Automatic camera rotation does not generate edits.
Restoring a parameter preserves surviving components' simulation and capture
state. Deleting and recreating a component necessarily recreates that runtime.

## MIDI Learn

Enable the desired input under **Settings > MIDI**. Right-click a numeric
component parameter or an assigned macro and choose **Learn MIDI controller**.
Move an absolute knob, fader, or pad. The next supported message from an enabled
input binds that component parameter; it does not create a signal channel.

Mappings are saved with the scene, address components by stable identity, and
appear under Settings > MIDI. Mapped inspector fields display a small **M**.
Right-click again to relearn or remove the mapping. A controller can address
several parameters. Each parameter has one direct controller binding.

This MVP maps normalized absolute values into the parameter's ordinary range,
including logarithmic and integer behavior. Pitch bend maps its center to the
middle of the parameter range. A released note returns gate/velocity mappings to
zero. A controller that has not sent a new value does not continuously overwrite
a manual edit. Relative encoders, pickup/soft takeover, scene launch bindings, and
MIDI feedback to hardware remain future work.

Learning from a macro currently binds its assigned parameter. Replacing that
macro's target does not move an existing direct parameter binding. For a MIDI
source that should pass through normalization, smoothing, and modulation strength,
create a MIDI signal channel instead and connect it to the macro.

## Visual signal processing

Signal processing follows:

`Input > Detector/envelope > Input range > Curve > Influence > Smoothing > Mappings`

The Output inspector shows raw, normalized, and final values plus a transfer
curve. For a detector moving only between 0.50 and 0.55, enable input-range
normalization and set those limits, or use **Learn range · 5 s** while the audio
plays. The range expands to 0–1 before influence and final smoothing. Learning
finishes even while the editor is hidden and retains a result message, including
the measured limits or an explanation when no changing live input was detected.

The optional curve stage adds exponent, dead zone, stepped output, and inversion.
It defaults to bypass, preserving existing scenes. Bipolar sources retain their
center and sign. Smoothing follows shaping and influence, so quantized or noisy
input can still produce a continuous final control signal. Each channel remains
independent; changing one shared source affects all its consumers.

Use **Open signal processor** in the channel context menu or Output inspector for
a resizable floating editor. The gray trace shows detector/envelope output before
normalization; the channel-colored trace shows final output, both on the same
scale over two seconds. Source, detector, response, output, and mapping controls
are the same controls as the inspector. Inapplicable stages disappear for MIDI
and LFO inputs. This is a built-in control-signal editor, not a VST/audio plug-in
host, and it can remain open while the Signals dock is hidden.

## Inspector and development UI

Component preset/reset/copy/paste actions are grouped under **Component actions**
instead of competing with the parameter hierarchy. Parameter addresses and ranges
now come from the same descriptor used by inspector dragging, macros, and MIDI.

Development-mode builds expose **Dev > Design System**. The gallery uses the
current theme and actual shared controls, and includes iconography plus explicit
placeholders for brand, app icon, splash screen, and release checks. It is absent
from normal application mode and is not a separate user-facing settings surface.
