# Compact inspector and component colors

Open a component's inspector with its eye button, **I**, or the navbar inspector
button. The fixed icon rail selects one configuration tab at a time: audio
mappings, geometry, motion, particles, appearance, camera, and component-specific
analysis tabs. Hover icons for their names. The active tab title identifies the current controls.

The inspector uses 22-point property rows, 12-point labels, aligned numeric
fields, subtle alternating row shading, compact disclosure groups, and full-width
rules. The header and tab rail remain fixed while properties scroll. Tab
selection is saved with the pane; scroll and disclosure positions are independent
for each pane, module, and tab during the session. The performance bar's Inspect
action selects the relevant tab.

## Palette browser

Choose the palette icon (**Appearance**), then open **Palette**. There are twelve
factory gradient previews: Spectrum, Ocean, Ember, Heatmap, Viridis, Magma,
Inferno, Plasma, Aurora, Ice, Electric lime, and Pearl. **Default colors** follows
the automatic platform/development palette independently of the GUI skin. Existing Solar, Ion, Kiln, and Glacier presets remain compatible.
The browser also lists your saved palettes. The strip below the picker previews
the current gradient.

These palettes style components independently of the application skin in
Settings → Theme & Colors. Veroni maps multi-color gradients across sites;
Strata maps them to membrane activity; Flux moves through their colors. Scientific
components retain their interpretation controls: waveform Color map maps signal
level, while Static selects lane colors. Stereometer and oscilloscope palettes
apply in Static mode; RGB and multiband modes keep their frequency colors.
The compact Viridis/Magma/Inferno/Plasma gradients use five color anchors.

## Edit and save

Expand **Edit gradient & swatches**:

1. Select a numbered stop. Its color button opens the RGB/HSV color picker,
   including hex entry; the row also shows its hex color.
2. Add/remove stops (2–8), reverse the gradient, or turn the selected stop into a
   solid color. Stop spacing is uniform. **Cyclic gradient** joins the last stop
   back to the first; Flux's cyclic presets retain their original wrap behavior.
3. Enter a name and **Save palette**. Reusing a name offers **Update palette**.
4. Give a selected color a name and **Save selected color** to create a swatch.
   Click a swatch to apply it to the selected gradient stop.

Use **Manage saved palettes** to apply or delete a library entry. Right-click a
swatch to see its name or delete it. Deleting a saved palette does not change
components already using its colors. The library supports 64 palettes and 64
swatches, with names up to 40 characters. Edits apply immediately; no capture or
history reset is needed.

## Storage and portability

A versioned `colors.json` file lives beside the workspace and visualization
presets: `target/dev/colors.json` for live development, and the normal platform
configuration directory for the regular app. Dev and regular libraries are
separate. Writes replace the file atomically. Invalid libraries are retained as
`colors.rejected.json`; failures show a notice instead of silently losing data.

Custom palettes embed their stops and cyclic setting directly in component
snapshots. Copy/paste, full-view presets, component presets, and restored
workspaces therefore carry their colors without requiring the same library on
another computer. Library names are labels for reusable color data, rather than
external references. Older snapshots retain their colors and default newly added
scope palette fields to Theme.
