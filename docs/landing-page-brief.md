# Symphos landing page brief

Create a separate public website that lets musicians see Symphos in motion,
understand where its audio comes from, and install the right build. Start the
mockup after capturing representative application visuals; publish download
claims only when the corresponding release is tested.

## Message and visual direction

Working headline: **See the shape of your sound.**

Current description: “A real-time audio analyzer and visualizer, built for Linux.”
Once the Mac release is ready, evolve the description to explain that users can
visualize computer playback or a selected input. Add DAW track analysis when
the plugin ships. Avoid presenting the scene editor as an available feature.

Use Symphos's dark charcoal, warm foreground and green accent as a starting
point. Let a large real waterfall/spectrum recording carry the page. Keep
typography and layout spacious enough to contrast with the detailed app UI.
Preserve Symphos's identity rather than borrowing Aurora's artwork or branding.

## Page outline

| Section | Content | Visitor action |
| --- | --- | --- |
| Hero | Headline, one sentence, real application image or short looping demo | Download the recommended tested build; show all downloads |
| Choose a workflow | Computer playback, microphone/interface, DAW plugin | Understand audio setup before installing |
| Analyzer gallery | Waterfall, spectrum, waveform, spectrogram, stereometer and oscilloscope | Inspect real capabilities and customization |
| Compatibility | OS versions, CPU architectures, audio backends, plugin formats and tested hosts | Confirm that their setup is supported |
| Getting started | Short OS-specific steps; permission explanation; plugin installation | Reach the first working visual quickly |
| Project and support | Release notes, source, issue reporting, license and roadmap | Follow development or report a problem |

Illustrative layout for the later mockup:

```text
Symphos                              Features  Downloads  Docs

See the shape of your sound.
Short description of the released product.
[Download for your platform] [All downloads]

[Large real application preview with play/pause]

Computer audio       Audio inputs       Inside your DAW
[Availability and setup for each workflow]

[Analyzer gallery]   [Customization examples]

[Compatibility and download table]
[Three-step setup]   [Troubleshooting]

Source  Changelog  License  Report an issue
```

## Release and content requirements

Maintain a small release manifest containing version, artifact URL, checksum,
OS minimum, CPU architecture, plugin format and release-note link. Platform
detection can suggest a download but must leave all platforms visible. Mark
planned builds as planned, with no fake download buttons or implied support.
Keep standalone and plugin downloads distinct.

Capture one short demo using original or appropriately licensed audio, one
clean dashboard screenshot and a few focused module images. Include a poster
image and reduced-motion behavior; pause animation offscreen and provide a
visible play/pause control. Do not require visitor microphone permission or
autoplay sound to demonstrate the product.

Draft separate setup guides for Linux PipeWire, Mac system-audio permissions,
explicit audio inputs, and DAW installation. Explain that a plugin sees audio
routed to its track/bus, while a standalone system source captures playback.
Do not promise that every protected or exclusive audio path is capturable.

## Implementation sequence

1. Confirm name/domain and whether launch is free/open-source, paid or undecided.
   Domain availability and trademark checks remain future work.
2. Capture the app visuals and prepare a desktop/mobile mockup with the outline
   above. Use real feature names and current availability.
3. Build a small static site in a separate repository or deployment project,
   keeping website tooling out of the Rust application's build path.
4. Connect verified release artifacts and documentation. Start without accounts,
   payments or a newsletter unless those are actually needed.
5. Check responsive layout, keyboard navigation, contrast, reduced motion,
   download links, compatibility copy and page performance before publishing.

The domain and hosting provider can be chosen later. They do not block the
[desktop and plugin work](cross-platform-plan.md).
