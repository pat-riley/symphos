use super::*;
macro_rules! settings {
 ($($field:ident: $default:expr, $min:expr, $max:expr, $section:literal, $label:literal, $help:literal;)*) => {
 #[derive(Clone,Debug,PartialEq,serde::Serialize,serde::Deserialize)]
 #[serde(default)]
 pub(crate) struct CymaticsSettings {
 $(pub $field:f32,)*
 pub mode:u32,pub shape:u32,pub edge:u32,pub center_support:bool,pub material:u32,
 pub quality:u32,pub artistic:bool,pub auto_sensitivity:bool,pub recycle_edges:bool,pub emitters:u32,pub color_mode:u32,pub soft_grains:bool,
 pub color_a:[u8;3],pub color_b:[u8;3],pub plate_color:[u8;3],pub orthographic:bool,pub guides:bool,
 }
 impl Default for CymaticsSettings { fn default()->Self { Self { $($field:$default,)*mode:0,shape:0,edge:0,center_support:false,material:0,quality:1,artistic:false,auto_sensitivity:true,recycle_edges:true,emitters:3,color_mode:0,soft_grains:false,color_a:[238,219,171],color_b:[255,252,239],plate_color:[20,24,30],orthographic:false,guides:false } } }
 impl CymaticsSettings {
 pub fn sanitize(&mut self){$(self.$field=if self.$field.is_finite(){self.$field.clamp($min,$max)}else{$default};)*self.mode=self.mode.min(3);self.shape=self.shape.min(2);self.edge=self.edge.min(2);self.material=self.material.min(2);self.quality=self.quality.min(3);self.emitters=self.emitters.clamp(1,6);self.color_mode=self.color_mode.min(1);self.high_hz=self.high_hz.max(self.low_hz+1.0);}
 #[cfg(test)]
 pub fn specs()->Vec<crate::macros::ParameterSpec>{vec![$(crate::macros::ParameterSpec{field:stringify!($field).into(),label:$label.into(),min:$min as f64,max:$max as f64,safe_min:$min as f64,safe_max:$max as f64,default:$default as f64,integer:false,logarithmic:false,scale:1.0},)*]}
 pub fn numeric_controls(&mut self,ui:&mut egui::Ui,section:&str){$(if section==$section {ui.add(crate::parameter::Parameter::new(&mut self.$field,$min..=$max,$default).key(stringify!($field)).text($label)).help_text($help);})*}
 }
 };
}
settings! {
 width_cm: 40.0, 10.0, 100.0, "Geometry", "Plate width cm", "Physical width, or diameter for a circular plate. Doubling it lowers all natural frequencies by a factor of four. The camera framing stays constant.";
 aspect: 1.4, 0.5, 2.0, "Geometry", "Rectangle aspect", "Height divided by width. Used only for rectangular plates. Changes resonance spacing and breaks square-plate degeneracy. Recalculates modes in the background.";
 thickness_mm: 1.0, 0.2, 5.0, "Geometry", "Thickness mm", "Thin-plate thickness. Doubling thickness doubles natural frequencies. The rendered plane is deliberately thin; this value controls bending stiffness.";
 poisson: 0.33, 0.15, 0.45, "Geometry", "Poisson ratio", "Transverse contraction of the plate material. Affects bending stiffness and free-edge mode shapes. Aluminum is approximately 0.33; steel 0.30.";
 excite_x: 0.23, -0.95, 0.95, "Geometry", "Excitation X", "Position where incoming audio applies an idealized point force. A mode cannot be excited at its node. Coordinates run from -1 to +1 across the plate.";
 excite_y: 0.17, -0.95, 0.95, "Geometry", "Excitation Y", "Point-force position along the other plate axis. On a circular plate, positions outside the rim are projected inside. No support or exciter is drawn.";
 damping: 0.035, 0.005, 0.3, "Signal Response", "Damping ratio", "Resonance bandwidth. Small values emphasize narrow, persistent resonances; high values broaden and soften them. Natural frequencies still come from the physical plate.";
 gain: 40.0, 0.0, 240.0, "Signal Response", "Excitation gain", "Scale incoming audio excitation. With Automatic visual sensitivity, sets the visible vibration level while preserving the modal pattern. Switch automatic sensitivity off for the original relative physical amplitudes. Silence stays silent.";
 low_hz: 20.0, 10.0, 4000.0, "Signal Response", "Low frequency Hz", "Ignore incoming spectral peaks below this frequency. Together with High frequency, isolates which part of the music excites the plate.";
 high_hz: 4000.0, 40.0, 20000.0, "Signal Response", "High frequency Hz", "Ignore incoming peaks above this frequency. Only the modeled low-order resonances are represented; a very high cutoff does not add more plate modes.";
 gate_db: -65.0, -100.0, -20.0, "Signal Response", "Noise gate dB", "Spectral peaks quieter than this threshold do not excite the plate. Raise it if microphone noise keeps the sand moving.";
 attack_ms: 65.0, 5.0, 1000.0, "Signal Response", "Attack ms", "How quickly the displayed vibration energy follows new audio. Sand still needs time to migrate into the new pattern.";
 release_ms: 350.0, 10.0, 3000.0, "Signal Response", "Release ms", "How quickly vibration fades after audio stops. Grains lose momentum and remain where they settled; patterns do not animate without input.";
 migration: 0.65, 0.0, 4.0, "Particles", "Settling strength", "Effective motion toward low-vibration areas, approximating sand collecting at nodal lines. This is a transport model, not a simulation of individual grain collisions.";
 friction: 7.0, 0.5, 20.0, "Particles", "Friction", "How quickly grains lose horizontal momentum. Higher values settle calmly; lower values allow longer drifting paths.";
 agitation: 0.06, 0.0, 0.5, "Particles", "Vibration agitation", "Small random kicks in vibrating regions keep grains mobile until they find a node. Quieter areas receive less agitation. Zero removes stochastic motion.";
 grain_size: 1.2, 0.4, 6.0, "Particles", "Grain size px", "Rendered grain radius in target pixels. Smaller grains reveal delicate patterns; larger grains make a denser image without adding particles.";
 size_variation: 0.5, 0.0, 1.0, "Particles", "Size variation", "Stable per-grain size variation. Does not flicker or reset the simulation.";
 grain_opacity: 0.9, 0.0, 1.0, "Particles", "Grain opacity", "Visibility of sand grains. The plate remains visible when grains are transparent.";
 glow: 0.25, 0.0, 3.0, "Appearance", "Grain glow", "Brighten the centers of grains. This is a visual effect; it does not change vibration or settling.";
 plate_opacity: 1.0, 0.0, 1.0, "Appearance", "Plate opacity", "Visibility of the plane. Zero hides the plate in Sand mode while retaining its particles and physical boundary.";
 brightness: 1.0, 0.1, 3.0, "Appearance", "Brightness", "Overall plate and particle brightness. Has no effect on measured audio or physical motion.";
 color_spread: 0.7, 0.0, 1.0, "Appearance", "Color spread", "How much of the two-color gradient is used. Zero uses a single color; one spans the full gradient.";
 color_offset: 0.0, 0.0, 1.0, "Appearance", "Color offset", "Shift the sampled range of the gradient toward its second color.";
 contrast: 1.0, 0.2, 4.0, "Appearance", "Color contrast", "Shape the gradient distribution. Higher values concentrate the first color and preserve bright highlights.";
 displacement: 0.12, 0.0, 0.6, "Appearance", "Surface height", "Visual magnification of the wave surface. Sand remains on a flat plane. The displacement is illustrative, not a calibrated measurement in meters.";
 display_rate: 0.45, 0.0, 3.0, "Appearance", "Wave motion Hz", "Slow-motion illustration of wave phase in Wave surface and Interference modes. Zero holds the illustrated phase; incoming audio still determines the pattern and amplitude.";
 node_width: 0.075, 0.005, 0.3, "Appearance", "Nodal line width", "Thickness of bright low-vibration regions in Nodal patterns mode. Does not change the underlying resonances.";
 contours: 10.0, 0.0, 40.0, "Appearance", "Contour density", "Number of decorative equal-amplitude bands in Nodal patterns mode. Zero leaves only the nodal highlight.";
 ambient: 0.35, 0.0, 1.0, "Appearance", "Ambient light", "Base illumination of shaded wave surfaces, including regions turned away from the directional light.";
 diffuse: 0.8, 0.0, 2.0, "Appearance", "Directional light", "Shading strength from the light direction. Helps reveal small ripples when the plane is tilted.";
 specular: 0.3, 0.0, 2.0, "Appearance", "Specular highlight", "Brightness of reflections on the shader surface. Raise it for a polished metallic or liquid appearance.";
 roughness: 0.45, 0.05, 1.0, "Appearance", "Roughness", "High values spread highlights broadly; low values make small sharp reflections.";
 light_angle: -35.0, -180.0, 180.0, "Appearance", "Light azimuth", "Direction of the light around the plane, in degrees.";
 light_height: 50.0, 5.0, 85.0, "Appearance", "Light elevation", "Height of the light above the plane, in degrees.";
 turbulence: 0.3, 0.0, 3.0, "Flow & Motion", "Artistic turbulence", "Extra swirling grain motion. Only active when Artistic motion is enabled; zero keeps the nodal settling behavior.";
 emitter_spacing: 0.55, 0.0, 0.9, "Flow & Motion", "Emitter spacing", "Distance of virtual wave emitters from the plate center in Interference mode. No physical emitter is drawn.";
 emitter_phase: 0.25, 0.0, 1.0, "Flow & Motion", "Emitter phase spread", "Phase offset between neighboring emitters, in cycles. Changes constructive and destructive interference.";
 wave_scale: 1.0, 0.2, 4.0, "Flow & Motion", "Interference scale", "Artistic multiplier for wave number in Interference mode. Base wave number follows incoming pitch and thin-plate dispersion.";
 yaw: 0.0, -std::f32::consts::PI, std::f32::consts::PI, "Camera", "Yaw", "Horizontal camera orientation. Drag anywhere on the plane for free trackball rotation.";
 elevation: -0.95, -std::f32::consts::PI, std::f32::consts::PI, "Camera", "Elevation", "Camera tilt. Top view looks straight down onto the plate.";
 roll: 0.0, -std::f32::consts::PI, std::f32::consts::PI, "Camera", "Roll", "Rotation about the viewing axis. Trackball dragging supports unrestricted rotation.";
 zoom: 1.0, 0.25, 4.0, "Camera", "Zoom", "Camera magnification. Scroll over the component to adjust it.";
 pan_x: 0.0, -2.0, 2.0, "Camera", "Pan X", "Horizontal framing offset. Shift-drag or right-drag to pan.";
 pan_y: 0.0, -2.0, 2.0, "Camera", "Pan Y", "Vertical framing offset. Does not move the physical plate.";
 camera_distance: 4.0, 2.0, 10.0, "Camera", "Camera distance", "Distance from the camera to the plate. Works with perspective and orthographic views.";
}
