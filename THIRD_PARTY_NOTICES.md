# Third-party notices

Symphos's original code is MIT licensed. Its MilkDrop-compatible renderer uses
projectM, copyright the projectM team and contributors, under LGPL-2.1-or-later.
The complete vendored library source, local modifications, license text, and
attribution are included under `vendor/projectm/`. Dependency license texts are
collected in `vendor/projectm/licenses/` and copied into macOS app bundles
and the Linux user-local installation. The evaluator, GLM, GLAD,
HLSL parser, and image decoder retain their respective licenses there.

This version links projectM statically. Distributing binaries must also satisfy
the LGPL's source/relinking requirements: provide the corresponding Symphos and
vendored projectM source and build instructions so recipients can modify the
library and relink the executable. Do not present the entire combined binary as
MIT-only. See `docs/milkdrop.md` for reproducible build instructions.

The three bundled `.milk` presets in `assets/milkdrop/` are original Symphos
fixtures under the repository's MIT license. External preset packs are not
bundled or relicensed; their authors retain their rights and attribution.
MilkDrop is credited to Ryan Geiss. This integration uses projectM's compatible
implementation; it is not the original Winamp plug-in or an official MilkDrop release.
