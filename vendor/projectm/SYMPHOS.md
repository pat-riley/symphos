# Vendored projectM

Source: https://github.com/projectM-visualizer/projectm
Revision: `349b7cc2db312d47cef6be24c6856ced6643a385` (4.2.0 development snapshot).
Evaluator submodule: https://github.com/projectM-visualizer/projectm-eval
Revision: `22fb0cfd8f2dfbcd2b68f2443e7f44e19b32c09a`.

The source, bundled dependencies, generated evaluator parser, and license files
are checked in. Builds do not download dependencies. Upstream screenshots,
website documentation, Git metadata, and standalone tests are omitted.

Local modifications:

- `ProjectM.hpp`, `ProjectM.cpp`, `Renderer/RenderContext.hpp`: per-instance
  32-float control extension, copied into both presets during a transition; retain
  the outgoing texture manager while its preset finishes blending. Apply the
  previously unused beat sensitivity to normalized audio bands (default 1.0).
- `MilkdropPreset/MilkdropPreset.cpp`: bounded post-equation visual adjustments,
  plus restoration of the preset framebuffer after blur texture allocation for
  offscreen rendering. Zero adjustments bypass clamping so HDR/out-of-range
  values authored in presets remain intact.
- `MilkdropPreset/Shaders/PresetWarpVertexShaderGlsl330.vert`: reproduce CPU
  `powf` behavior for negative zoom with integer exponents (undefined in GLSL).
- `MilkdropPreset/PerPixelMesh.cpp`, `FinalComposite.cpp`: propagate shader
  compilation errors instead of silently replacing failed shaders with defaults.

- `MilkdropPreset/PresetFileParser.cpp`: preserve shader newlines while joining
  equation chunks after stripping comments, matching original MilkDrop behavior.
- `MilkdropPreset/MilkdropShader.cpp`, `vendor/hlslparser`: retain compiler
  diagnostics; anchor sampler declaration cleanup; handle assignment casts,
  flattened vector arrays, global array initialization and integer multiplication.
- `Renderer/TextureManager.cpp`: missing random textures produce a named fallback
  and a warning instead of an undefined sampler that breaks shader compilation.
- `licenses/`: collected bundled-dependency notices, including GLM 1.0.3 and
  GLAD 2.0.8 license texts from their official release sources. GLAD's generated
  loader selects CC0-1.0 plus Apache-2.0; GLM and stb use their MIT option.

The host/context bridge and CMake integration live in `src/milkdrop/`.
The upstream C API is unchanged. These extensions are specific to this pinned
source, not a promise of compatibility with an arbitrary installed projectM.

projectM retains its LGPL-2.1-or-later licensing. See COPYING, LICENSE.txt,
AUTHORS.txt, and the respective license files in vendor/. Symphos's original
host code is MIT. Local changes to projectM remain under projectM's license.
