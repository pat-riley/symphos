//! Native GPU orbital / flocking sketch with analytic cores, lensing and coronas.
use super::{properties_combo, property_row, settings_panel, trackball};
use crate::{
    analysis::AnalysisFrame, help::HoverHelp, parameter::Parameter, theme::AppTheme,
    visuals::GpuView,
};
use egui::{Rect, Sense};
use std::time::Instant;
use symphos_visuals::Frame;
mod camera;
mod help;
pub(super) mod inspector;

// Stable field IDs are shared by persistence, inspector and macro drag payloads.
// One declaration sets bounds/defaults/help and prevents inspector/shader drift.
macro_rules! parameters {
    ($($field:ident: $default:expr, $min:expr, $max:expr, $section:literal, $label:literal, $help:literal;)*) => {
        #[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
        #[serde(default)]
        pub(crate) struct OrbitalSettings {
            $(pub $field:f32,)*
            pub quality:u32,
            pub attractors:u32,
            pub particle_shape:u32,
            pub color_palette:u32,
            pub color_mode:u32,
            pub color_start:[u8;3],
            pub color_end:[u8;3],
            pub orb_style:u32,
            pub orb_color_mode:u32,
            pub orb_palette:u32,
            pub orb_color_start:[u8;3],
            pub orb_color_end:[u8;3],
            pub orb_colors:[[u8;3];5],
            pub orb_field_enabled:bool,
            pub orb_solid:bool,
            pub show_paths:bool,
            pub show_grid:bool,
            pub show_axes:bool,
            pub show_gizmo:bool,
            pub grid_plane:u32,
            pub projection:u32,
            pub auto_orbit:bool,
        }
        impl Default for OrbitalSettings {
            fn default()->Self { Self {$($field:$default,)*quality:1,attractors:3,particle_shape:0,color_palette:0,color_mode:0,color_start:[20,220,255],color_end:[255,40,150],orb_style:0,orb_color_mode:0,orb_palette:2,orb_color_start:[20,220,255],orb_color_end:[255,40,150],orb_colors:[[40,200,255],[255,70,140],[255,190,40],[120,255,90],[150,90,255]],orb_field_enabled:false,orb_solid:true,show_paths:false,show_grid:true,show_axes:true,show_gizmo:true,grid_plane:0,projection:0,auto_orbit:false} }
        }
        impl OrbitalSettings {
            fn sanitize(&mut self) {
                $(self.$field=if self.$field.is_finite(){self.$field.clamp($min,$max)}else{$default};)*
                self.quality=self.quality.min(3); self.attractors=self.attractors.clamp(1,5);
                self.particle_shape=self.particle_shape.min(6);
                self.color_palette=self.color_palette.min(7);
                self.color_mode=self.color_mode.min(4);
                self.orb_style=self.orb_style.min(8);
                self.orb_color_mode=self.orb_color_mode.min(3);
                self.orb_palette=self.orb_palette.min(7);
                self.grid_plane=self.grid_plane.min(3); self.projection=self.projection.min(1);
            }
            fn controls(&mut self,ui:&mut egui::Ui,section:&str) {
                $(if section==$section {
                    ui.add(Parameter::new(&mut self.$field,$min..=$max,$default).key(stringify!($field)).text($label)).help_text_with(|| help::parameter(stringify!($field),$label,$help,$section,[$min as f32,$max as f32,$default as f32]));
                })*
            }
            #[cfg(test)]
            fn inspector_fields()->Vec<(&'static str, &'static str)> {vec![$((stringify!($field),$section),)*]}
            #[cfg(test)]
            fn specs()->Vec<crate::macros::ParameterSpec> {
                vec![$(crate::macros::ParameterSpec {field:stringify!($field).into(),label:$label.into(),min:$min as f64,max:$max as f64,safe_min:$min as f64,safe_max:$max as f64,default:$default as f64,integer:false,logarithmic:false,scale:1.0},)*]
            }
        }
    }
}
parameters! {
    cohesion: 0.8, 0.0, 4.0, "Flocking", "Cohesion", "Pull nearby particles toward one another, gathering them into groups. Higher values make tighter schools; lower values let streams spread out.";
    alignment: 1.1, 0.0, 4.0, "Flocking", "Alignment", "Encourage nearby particles to travel in the same direction. Higher values create organized currents; lower values let individual directions differ.";
    separation: 0.8, 0.0, 4.0, "Flocking", "Separation", "Push crowded groups apart so particles have room to move. This estimates local crowding rather than testing collisions between every pair of particles.";
    speed: 1.8, 0.1, 6.0, "Particle motion", "Speed limit", "Maximum particle speed in world units per second. Motion uses elapsed time, independent of display FPS.";
    attraction: 1.0, 0.0, 5.0, "Attraction and orbit", "Attraction", "Pull particles toward the moving attractor orbs.";
    orbit: 1.8, -4.0, 4.0, "Attraction and orbit", "Orbital force", "Tangential force around each orb's tilted axis. Negative reverses circulation.";
    repulsion: 3.5, 0.0, 8.0, "Attraction and orbit", "Core repulsion", "Push particles away from orb centers, opening luminous cavities.";
    noise: 0.45, 0.0, 3.0, "Turbulence", "Turbulence", "Smooth time-varying forces that break up regular orbits.";
    spacing: 1.3, 0.0, 3.0, "Orb arrangement", "Orb spacing", "Distance of attractor centers from the scene origin.";
    radius: 0.18, 0.03, 0.7, "Orb size", "Orb radius", "Rendered orb radius and radius of its repulsive core.";
    motion: 0.18, -1.0, 1.0, "Orb arrangement", "Orb motion", "Angular speed of the attractors; zero holds their arrangement in place.";
    boundary: 4.5, 2.0, 8.0, "Cloud boundaries", "Cloud radius", "Soft spherical boundary and spatial-grid extent. Larger values spread the cloud.";
    size: 1.5, 0.5, 12.0, "Particle size", "Particle size", "Particle width in render-target pixels. Lower sizes reduce GPU fill cost.";
    streak: 0.9, 0.0, 4.0, "Paths and trails", "Motion streak", "Stretch moving particles along velocity to reveal flow; stationary particles remain visible.";
    brightness: 0.7, 0.0, 3.0, "Scene rendering", "Overall brightness", "Particle and orb radiance before HDR bloom and tone mapping.";
    hue: 0.05, 0.0, 1.0, "Particle color", "Color offset", "Shift the sampled position within the selected palette. With zero color spread, this selects one color for all particles.";
    size_variation: 0.0, 0.0, 1.0, "Particle size", "Size variation", "Stable per-particle size variation, from uniform to a mixture of tiny and large particles. Does not flicker or reset the flock.";
    aspect: 1.0, 0.25, 4.0, "Particle size", "Aspect ratio", "Stretch each particle's shape along its local horizontal axis, independently of velocity streaking.";
    softness: 0.5, 0.0, 1.0, "Particle size", "Edge softness", "Zero produces crisp, antialiased edges; one produces a soft luminous center. Applies to all particle shapes.";
    ring_width: 0.3, 0.05, 1.0, "Particle size", "Ring thickness", "Thickness of the Ring shape. One fills the ring into a disc.";
    rotation: 0.0, -180.0, 180.0, "Particle orientation", "Rotation °", "Rotate particle shapes relative to their direction of travel.";
    spin: 0.0, -180.0, 180.0, "Particle orientation", "Spin °/s", "Continuous particle-shape rotation. Negative values reverse the direction.";
    rotation_variation: 0.0, 0.0, 1.0, "Particle orientation", "Rotation variation", "Give each particle a stable random orientation offset, up to a full turn.";
    opacity: 1.0, 0.0, 1.0, "Particle appearance", "Particle opacity", "Scale particle light contribution. Zero hides particles while retaining the attractor orbs. Additive rendering does not occlude particles behind others.";
    brightness_variation: 0.0, 0.0, 1.0, "Particle appearance", "Brightness variation", "Stable per-particle brightness variation. Increase it to break up uniform luminous bands.";
    color_spread: 0.35, 0.0, 1.0, "Particle color", "Color spread", "How much of the palette is used by the chosen color driver. Zero makes particles a single color; one uses the full palette.";
    saturation: 1.0, 0.0, 2.0, "Particle color", "Saturation", "Zero is grayscale, one preserves the palette, and higher values intensify its colors.";
    color_cycle: 0.0, -0.5, 0.5, "Particle color", "Color cycle /s", "Animate the palette position over time without changing particle motion. Zero holds the palette still.";
    orb_brightness: 1.0, 0.0, 3.0, "Orb appearance", "Orb brightness", "Independent brightness of the attractor spheres. Zero hides the spheres while their forces remain active.";
    lighting: 1.0, 0.0, 3.0, "Orb lighting", "Orb lighting", "Directional shading on analytic sphere impostors, with a luminous rim.";
    orb_scale: 1.0, 0.1, 5.0, "Orb size", "Visual size", "Scale the rendered orbs. The optional surface field follows this size; legacy Core repulsion still uses Orb radius.";
    orb_aspect: 1.0, 0.25, 3.0, "Orb size", "Aspect ratio", "Horizontal stretch of the orb silhouette in the view plane. Appearance only; the force field stays spherical.";
    orb_size_variation: 0.0, 0.0, 1.0, "Orb size", "Size variation", "Give the orbs different stable sizes. The optional surface field follows each orb’s size.";
    orb_pulse: 0.0, 0.0, 0.8, "Orb pulse", "Pulse depth", "Rhythmically expand and contract the visual radius. Zero disables pulsing.";
    orb_pulse_rate: 0.5, 0.0, 4.0, "Orb pulse", "Pulse rate Hz", "Cycles per second for orb size pulsing. Each orb has an offset phase.";
    orb_audio_size: 0.4, 0.0, 2.0, "Audio orb size", "Audio size", "Expand the orb radius with smoothed main-audio energy. The optional surface field follows this expansion.";
    orb_opacity: 1.0, 0.0, 1.0, "Orb appearance", "Orb opacity", "Scale the orb's additive light contribution. Zero hides it without disabling attraction. These are luminous surfaces, not opaque occluding meshes.";
    orb_ambient: 0.5, 0.0, 2.0, "Orb lighting", "Ambient light", "Base surface illumination for shaded, wireframe, bands and plasma styles.";
    orb_specular: 0.0, 0.0, 4.0, "Orb lighting", "Highlight strength", "Add a directional specular highlight to shaded and patterned surfaces.";
    orb_roughness: 0.4, 0.05, 1.0, "Orb lighting", "Roughness", "Low values make tight highlights; high values make broad highlights. Requires nonzero Highlight strength.";
    orb_rim: 3.0, 0.0, 6.0, "Orb lighting", "Rim light", "Light around the edges of shaded and patterned spheres.";
    orb_rim_power: 3.0, 0.5, 12.0, "Orb lighting", "Rim focus", "Higher values concentrate rim light nearer the edge.";
    orb_light_azimuth: -26.565, -180.0, 180.0, "Orb lighting", "Light azimuth °", "Horizontal direction of the orb's light in camera space.";
    orb_light_elevation: 28.22, -89.0, 89.0, "Orb lighting", "Light elevation °", "Vertical direction of the orb's light in camera space.";
    orb_detail: 12.0, 2.0, 48.0, "Orb surface", "Surface density", "Density of wireframe lines, latitude bands or plasma waves. Fractional values blend adjacent whole densities to avoid a seam.";
    orb_line_width: 0.12, 0.02, 0.5, "Orb surface", "Line thickness", "Width of wireframe lines, latitude bands and the outline rim.";
    orb_surface_rotation: 0.0, -180.0, 180.0, "Orb surface", "Surface rotation °", "Rotate patterns around the orb's vertical axis, independently of its orbit.";
    orb_surface_spin: 0.0, -180.0, 180.0, "Orb surface", "Surface spin °/s", "Animate pattern rotation. Negative reverses it; zero holds the pattern still.";
    orb_surface_tilt: 0.0, -180.0, 180.0, "Orb surface", "Surface tilt °", "Tilt the pattern axis. Especially useful for latitude bands and wireframe grids.";
    orb_hue: 0.0, 0.0, 1.0, "Orb palette", "Orb color offset", "Choose a position in the independent orb palette. With zero spread and zero surface gradient, all orbs use this color.";
    orb_color_spread: 1.0, 0.0, 1.0, "Orb palette", "Orb color spread", "Spread the orbs across the independent palette. Used in Own palette mode.";
    orb_saturation: 1.0, 0.0, 2.0, "Orb color", "Orb saturation", "Adjust the orb colors independently. Zero produces grayscale.";
    orb_color_cycle: 0.0, -0.5, 0.5, "Orb palette", "Orb color cycle /s", "Animate the independent palette position. Used in Own palette mode.";
    orb_surface_gradient: 0.0, 0.0, 1.0, "Orb palette", "Surface gradient", "Blend from one palette color per orb to a pole-to-pole surface gradient. Used in Own palette mode.";
    bloom: 0.7, 0.0, 3.0, "Scene rendering", "Bloom", "Quarter-resolution separable HDR glow around bright particles and orbs.";
    exposure: 1.0, 0.1, 3.0, "Scene rendering", "Exposure", "Tone-mapping exposure. Reduce it to retain detail in dense clouds.";
    trails: 0.09, 0.0, 0.8, "Paths and trails", "Trail half-life s", "Screen-space feedback half-life in seconds. Zero clears every frame; long trails accumulate light.";
    audio_force: 0.65, 0.0, 2.0, "Audio movement and light", "Audio force", "Built-in main-audio RMS response adds force and repulsion. Set to zero for exclusively manual or macro-driven motion.";
    audio_light: 0.7, 0.0, 2.0, "Audio movement and light", "Audio light", "Built-in main-audio response brightens particles and orbs. Signals can also drive any exposed continuous parameter through scene macros.";
    response_ms: 120.0, 0.0, 1000.0, "Audio smoothing", "Response ms", "Smooth main-audio energy; disconnected input fades to zero. Flocking continues without audio.";
    neighbor_radius: 1.75, 0.5, 1.75, "Flocking", "Neighborhood reach", "How far a particle looks for flockmates, in spatial-grid cells. Small values form independent local streams; large values coordinate more of the flock. This is an approximate neighborhood, not an exact pairwise search.";
    rear_awareness: 1.0, 0.0, 1.0, "Flocking", "Rear awareness", "How much a particle responds to neighbors behind it. One sees in every direction; zero mostly follows what is ahead, like a school of fish.";
    separation_radius: 0.6, 0.1, 2.0, "Flocking", "Personal space", "The distance over which nearby groups push each other apart. Increase it for a looser flock, or reduce it for tightly packed streams. Separation controls how strongly they push.";
    agility: 40.0, 1.0, 60.0, "Particle motion", "Turning agility", "Limit how quickly forces can change a particle's velocity. Low values create broad sweeping turns; high values let particles react sharply to their neighbors and orbs.";
    drag: 0.12, 0.0, 3.0, "Particle motion", "Drag", "How quickly momentum is lost. Low drag lets particles coast; high drag damps motion and can make them settle unless forces keep pushing.";
    minimum_speed: 0.0, 0.0, 2.0, "Particle motion", "Minimum speed", "Keep particles moving instead of letting them stall. Zero allows a rest state. The effective minimum never exceeds Speed limit.";
    center_pull: 0.0, 0.0, 3.0, "Cloud boundaries", "Center pull", "Pull the entire flock toward the world origin, independently of the moving orbs. Small amounts gather wide clouds; large amounts compress the flock.";
    plane_confinement: 0.0, 0.0, 5.0, "Cloud boundaries", "Flatten to XZ", "Pull particles toward the horizontal XZ plane. Zero keeps a full 3D cloud; stronger values produce disc-like swarms. The XYZ guides show this plane.";
    lift: 0.0, -2.0, 2.0, "Particle motion", "Vertical drift", "Apply a steady push along the Y axis. Positive values rise; negative values sink. The cloud boundary still pulls particles back into the scene.";
    noise_scale: 1.0, 0.1, 4.0, "Turbulence", "Turbulence scale", "Frequency of the turbulence field. Low values make broad bends; high values create small, busy eddies. Turbulence controls the strength.";
    noise_speed: 1.0, 0.0, 3.0, "Turbulence", "Turbulence evolution", "How quickly the turbulence pattern changes over time. Zero freezes the field, but particles can still move through it.";
    boundary_strength: 3.0, 0.0, 10.0, "Cloud boundaries", "Boundary return", "How strongly particles return when they approach the Cloud radius. Low values allow excursions; high values keep a tight enclosure. A distant safety boundary prevents numerical escape.";
    orbit_tilt: 1.0, 0.0, 2.0, "Attraction and orbit", "Orbit axis spread", "Tilt the circulation axes of the orbs in different directions. Zero aligns them vertically; larger values create crossing 3D currents.";
    simulation_speed: 1.0, 0.0, 2.0, "Simulation clock", "Simulation speed", "Scale the time used by particle motion, orb motion and animated surfaces. One is normal speed; zero holds time still. Camera navigation remains available.";
    orb_particle_light: 0.0, 0.0, 4.0, "Orb lighting", "Field illumination", "Make particles glow as they approach an orb's surface, revealing the currents wrapping around it. Distant particles keep their base brightness. Increase for a luminous skin of moving dust; reduce Particle opacity if the rest of the cloud is too bright. Bursts also flare this nearby light.";
    orb_core_darkness: 0.0, 0.0, 1.0, "Lens and depth", "Core occlusion", "Hide particles and paths behind the orb surface. One gives a dark, solid silhouette with foreground particles still visible; zero keeps the translucent additive look. This changes visibility, independently of Solid surface collision.";
    orb_lens: 0.0, -1.0, 1.0, "Lens and depth", "Lens warp", "Bend the rendered light around each orb, like a stylized gravitational lens. Positive gathers light toward the rim; negative pushes it outward. This screen-space effect changes appearance, not particle trajectories.";
    orb_lens_reach: 2.5, 1.1, 6.0, "Lens and depth", "Lens reach", "Outer radius of the optical distortion, measured in orb radii. Small values make a tight glass-like edge; large values bend a wider area of the swarm.";
    orb_dispersion: 0.0, 0.0, 1.0, "Lens and depth", "Color dispersion", "Separate red and blue slightly in the lens distortion, producing fine colored fringes. Requires Lens warp. Keep it subtle to preserve fine particle detail.";
    orb_corona: 0.0, 0.0, 4.0, "Corona and rays", "Corona brightness", "Add a luminous atmosphere outside each orb. Uses that orb’s selected color and fades into surrounding space. The core can remain dark while the edge glows.";
    orb_corona_width: 0.18, 0.02, 1.0, "Corona and rays", "Corona width", "Thickness of the glow outside the surface, measured in orb radii. A narrow halo makes a crisp luminous rim; a wide halo gives a diffuse solar atmosphere.";
    orb_rays: 0.0, 0.0, 4.0, "Corona and rays", "Radial rays", "Fine irregular shafts of light radiating from each orb. Increase for energetic starbursts; zero keeps a clean halo. Burst strength makes the rays flare on audio attacks.";
    orb_ray_length: 2.0, 0.1, 5.0, "Corona and rays", "Ray length", "How far rays extend beyond the orb surface, measured in orb radii. Long rays add drama but can crowd the scene; balance with Radial rays brightness.";
    orb_ray_density: 48.0, 8.0, 128.0, "Corona and rays", "Ray density", "Number of fine radial strands around each orb. Higher values make a delicate corona; lower values produce broader spokes. Fractional values blend neighboring densities smoothly.";
    orb_corona_spin: 8.0, -90.0, 90.0, "Corona and rays", "Corona drift °/s", "Slowly rotate the irregular ray pattern. Each orb has its own phase. Zero freezes the pattern; negative reverses the drift. This does not rotate the camera or change particle forces.";
    orb_burst: 0.0, 0.0, 4.0, "Transient flares", "Burst strength", "Brighten the rim, halo and rays on sharp increases in main-audio energy. This is transient detection, not a tempo clock. Zero disables automatic bursts; use Burst drive for your own signal mapping.";
    orb_burst_decay: 350.0, 40.0, 2000.0, "Transient flares", "Burst release ms", "How quickly a burst fades after an audio attack. Short releases feel percussive; long releases give slow breathing flares. Does not change the main audio smoothing control.";
    orb_burst_sensitivity: 1.5, 0.25, 4.0, "Transient flares", "Burst sensitivity", "How readily sharp increases in audio energy trigger a flare. Increase for quieter attacks; reduce if the corona flickers too frequently. Sustained steady sound should settle rather than repeatedly fire.";
    orb_burst_size: 0.2, 0.0, 1.0, "Transient flares", "Burst expansion", "Expand orbs during a burst, as a fraction of their resting radius. Zero changes light only. The optional surface field follows the changing radius so the swarm moves around the expanded orb.";
    orb_burst_drive: 0.0, 0.0, 1.0, "Transient flares", "Burst drive", "Manual flare envelope. Map a kick, MIDI pad, LFO or other Signal here to control the flash directly. This combines with automatic bursts and works even when no main audio is connected.";
    field_clearance: 0.08, 0.0, 1.5, "Surface field", "Surface clearance", "Extra space in world units around each visible orb. The field follows visual size, size variation, pulse and audio expansion. Stretched orbs use a surrounding sphere. The protective radius is capped at eight world units for extreme visual scales. Increase clearance to leave an empty halo around the surface.";
    field_reach: 0.8, 0.05, 4.0, "Surface field", "Field reach", "How far beyond the protected surface particles begin turning away. A broad field makes gentle sweeping detours; a narrow field produces close surface-hugging flow.";
    field_repulsion: 12.0, 0.0, 40.0, "Surface field", "Field repulsion", "Outward push within the surface field. Raise it to resist attraction before particles reach the orb. Zero removes the soft push; Solid surface can still keep particle centers outside.";
    field_flow: 2.0, -8.0, 8.0, "Surface field", "Surface circulation", "Additional sideways motion near the orb, guiding particles around its surface. Positive follows Orbital force's positive direction; negative reverses it. Zero lets particles slide in their incoming direction.";
    field_falloff: 1.5, 0.25, 4.0, "Surface field", "Field falloff", "Shape the field's fade toward its outer edge. Low values spread the push and circulation throughout the field; high values concentrate them near the surface.";
    path_count: 128.0, 16.0, 512.0, "Paths and trails", "Traced particles", "How many evenly distributed particles leave a visible path. Start near 64 to inspect individual routes; more paths show the overall flow but can become crowded. Rounded to a whole number.";
    path_length: 2.0, 0.1, 4.0, "Paths and trails", "Path duration s", "How much recent simulation time each path displays, up to four seconds. New paths grow as samples arrive; this does not extend the screen-space glow trail.";
    path_width: 1.0, 0.5, 5.0, "Paths and trails", "Path width px", "Thickness of the trajectory lines in the GPU render target. Thin paths work well for many tracers; thick paths make a few routes stand out.";
    path_opacity: 0.5, 0.0, 1.0, "Paths and trails", "Path opacity", "Brightness contribution of path lines. Lower it to keep the particle heads dominant. The lines are additive, so overlapping routes become brighter.";
    path_fade: 1.5, 0.0, 4.0, "Paths and trails", "Path age fade", "Fade older parts of each path. Zero keeps the entire path equally bright; larger values emphasize the recent portion near the particle.";
    camera_distance: 8.5, 3.0, 24.0, "Camera basic", "Camera distance", "Distance from the camera to the world origin. Increase it to see more of the scene; reduce it to move closer. In perspective, this changes the relative size of near and far objects.";
    field_of_view: 53.1301, 25.0, 100.0, "Camera advanced", "Field of view °", "Width of the perspective lens. Low angles compress depth like a telephoto lens; high angles exaggerate depth. Orthographic projection ignores this setting.";
    camera_orbit_speed: 10.0, -60.0, 60.0, "Camera advanced", "Camera orbit °/s", "Speed of automatic camera rotation around Y. Enable Auto-orbit to use it; dragging the camera or choosing a view stops it.";
    grid_extent: 5.0, 1.0, 10.0, "Camera guides", "Grid extent", "Half-width of the reference grid in world units. Five means it extends from -5 to +5 around the origin. This is a visual ruler, not a particle boundary.";
    grid_opacity: 0.22, 0.02, 0.7, "Camera guides", "Grid opacity", "Visibility of the reference grid. Keep it subtle for performance visuals or raise it when studying the spatial layout.";
    yaw: -0.3, -std::f32::consts::PI, std::f32::consts::PI, "Camera advanced", "Yaw", "Horizontal camera rotation. Drag the canvas to rotate freely.";
    elevation: 0.45, -std::f32::consts::PI, std::f32::consts::PI, "Camera advanced", "Elevation", "Vertical camera rotation.";
    roll: 0.0, -std::f32::consts::PI, std::f32::consts::PI, "Camera advanced", "Roll", "Camera roll around its viewing axis.";
    zoom: 1.0, 0.25, 3.0, "Camera basic", "Zoom", "Camera magnification; also controlled by the scroll wheel.";
    pan_x: 0.0, -2.0, 2.0, "Camera advanced", "Pan X", "Horizontal framing offset; Shift-drag to pan.";
    pan_y: 0.0, -2.0, 2.0, "Camera advanced", "Pan Y", "Vertical framing offset; Shift-drag to pan.";
}
#[derive(Default)]
pub struct Orbital {
    settings: OrbitalSettings,
    gpu: GpuView,
    last: Option<Instant>,
    time: f32,
    energy: f32,
    burst: BurstEnvelope,
}
impl Orbital {
    pub fn settings_snapshot(&self) -> OrbitalSettings {
        self.settings.clone()
    }
    pub fn apply_settings(&mut self, s: &OrbitalSettings) {
        self.settings = s.clone();
        self.settings.sanitize();
    }
    pub fn clear(&mut self) {
        self.gpu = GpuView::default();
        self.last = None;
        self.time = 0.0;
        self.energy = 0.0;
        self.burst = BurstEnvelope::default();
    }
    pub fn release_if_inactive(&mut self) {
        if self.last.is_some() {
            self.clear();
        }
    }
    pub fn reset_settings(&mut self) {
        *self = Self::default();
    }
    pub fn controls(&mut self, ui: &mut egui::Ui) {
        inspector::controls(ui, self);
        self.settings.sanitize();
    }
    pub fn draw(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        frame: &AnalysisFrame,
        theme: &AppTheme,
        now: Instant,
        live: bool,
    ) {
        self.settings.sanitize();
        let wall_dt = self.last.map_or(1.0 / 60.0, |last| {
            now.saturating_duration_since(last)
                .as_secs_f32()
                .min(1.0 / 30.0)
        });
        self.last = Some(now);
        let dt = wall_dt * self.settings.simulation_speed;
        self.time += dt;
        if self.settings.auto_orbit {
            self.settings.yaw = wrap_angle(
                self.settings.yaw + wall_dt * self.settings.camera_orbit_speed.to_radians(),
            );
        }
        let target = if live {
            (frame.mix_rms * 4.0).clamp(0.0, 1.0)
        } else {
            0.0
        };
        self.energy += (target - self.energy)
            * (1.0 - (-dt / (self.settings.response_ms * 0.001).max(0.02)).exp());
        let burst = self.burst.update(
            frame,
            live,
            dt,
            self.settings.orb_burst_decay,
            self.settings.orb_burst_sensitivity,
        );
        let s = &mut self.settings;
        let gizmo_rect = camera::gizmo_rect(rect);
        let over_gizmo = s.show_gizmo
            && ui.input(|i| {
                i.pointer
                    .hover_pos()
                    .is_some_and(|p| gizmo_rect.contains(p))
            });
        let response=ui.interact(rect,ui.id().with("orbital-camera"),Sense::click_and_drag()).help_text("Orbital · GPU flocking\nDrag to rotate, Shift/right-drag to pan, scroll to zoom, double-click to reset the camera. Expose inspector controls as scene macros to drive them from Signals.");
        if response.dragged() && !over_gizmo {
            s.auto_orbit = false;
            let delta = ui.input(|i| i.pointer.delta());
            if response.dragged_by(egui::PointerButton::Secondary)
                || response.dragged_by(egui::PointerButton::Middle)
                || ui.input(|i| i.modifiers.shift)
            {
                s.pan_x += delta.x / rect.width() * 2.0;
                s.pan_y -= delta.y / rect.height() * 2.0;
            } else if let Some(p) = response.interact_pointer_pos() {
                trackball::drag(
                    &mut s.yaw,
                    &mut s.elevation,
                    &mut s.roll,
                    p - delta,
                    p,
                    rect.center(),
                    rect.width().min(rect.height()) * 0.4,
                );
            }
        }
        if response.hovered() && !over_gizmo {
            s.zoom =
                (s.zoom * (ui.input(|i| i.smooth_scroll_delta.y) * 0.002).exp()).clamp(0.25, 3.0);
        }
        if response.double_clicked() && !over_gizmo {
            camera::reset(s);
        }
        s.sanitize();
        let axes = trackball::orientation_axes(s.yaw, s.elevation, s.roll);
        let axis = |i: usize| {
            [
                axes[i][0] as f32,
                -axes[i][1] as f32,
                axes[i][2] as f32,
                0.0,
            ]
        };
        self.gpu.show(
            ui,
            rect,
            Frame {
                clock: [dt, self.time, 0.0, 0.0],
                flock: [s.cohesion, s.alignment, s.separation, s.speed],
                forces: [s.attraction, s.orbit, s.repulsion, s.noise],
                orbs: [s.attractors as f32, s.spacing, s.radius, s.motion],
                look: [s.size, s.streak, s.brightness, s.hue],
                light: [s.lighting, s.bloom, s.exposure, s.trails],
                view: [rect.aspect_ratio(), s.zoom, s.pan_x, s.pan_y],
                axis_x: axis(0),
                axis_y: axis(1),
                axis_z: axis(2),
                audio: [self.energy, s.audio_force, s.audio_light, s.boundary],
                output: [0.0; 4],
                camera: [
                    s.camera_distance,
                    1.0 / (s.field_of_view.to_radians() * 0.5).tan(),
                    s.projection as f32,
                    0.0,
                ],
                behavior: [
                    s.neighbor_radius,
                    s.rear_awareness,
                    s.separation_radius,
                    s.agility,
                ],
                dynamics: [s.drag, s.minimum_speed, s.center_pull, s.plane_confinement],
                field: [s.lift, s.noise_scale, s.noise_speed, s.boundary_strength],
                circulation: [s.orbit_tilt, 0.0, 0.0, 0.0],
                orb_field: [
                    s.orb_field_enabled as u8 as f32,
                    s.field_clearance,
                    s.field_reach,
                    s.field_repulsion,
                ],
                orb_barrier: [
                    s.field_flow,
                    s.field_falloff,
                    s.orb_solid as u8 as f32,
                    s.orb_particle_light,
                ],
                orb_optics: [
                    s.orb_core_darkness,
                    s.orb_lens,
                    s.orb_lens_reach,
                    s.orb_dispersion,
                ],
                orb_corona: [
                    s.orb_corona,
                    s.orb_corona_width,
                    s.orb_rays,
                    s.orb_ray_length,
                ],
                orb_flare: [
                    s.orb_ray_density,
                    s.orb_corona_spin.to_radians(),
                    (burst * s.orb_burst).max(s.orb_burst_drive),
                    s.orb_burst_size,
                ],
                paths: [
                    if s.show_paths { 1.0 } else { 0.0 },
                    s.path_count.round(),
                    s.path_length,
                    s.path_width,
                ],
                path_style: [s.path_opacity, s.path_fade, 0.0, 0.0],
                path_clock: [0.0; 4],
                orb_style: [s.orb_style as f32, s.orb_scale, s.orb_aspect, s.orb_opacity],
                orb_shape: [
                    s.orb_size_variation,
                    s.orb_pulse,
                    s.orb_pulse_rate,
                    s.orb_audio_size,
                ],
                orb_material: [s.orb_ambient, s.orb_specular, s.orb_roughness, s.orb_rim],
                orb_surface: [
                    s.orb_rim_power,
                    s.orb_detail,
                    s.orb_line_width,
                    s.orb_surface_rotation.to_radians(),
                ],
                orb_light: [
                    s.orb_light_azimuth.to_radians(),
                    s.orb_light_elevation.to_radians(),
                    s.orb_surface_spin.to_radians(),
                    s.orb_surface_tilt.to_radians(),
                ],
                orb_color: [
                    s.orb_color_mode as f32,
                    s.orb_palette as f32,
                    s.orb_hue,
                    s.orb_color_spread,
                ],
                orb_tint: [
                    s.orb_saturation,
                    s.orb_color_cycle,
                    s.orb_surface_gradient,
                    0.0,
                ],
                orb_start: rgba(s.orb_color_start),
                orb_end: rgba(s.orb_color_end),
                orb_colors: s.orb_colors.map(rgba),
                particle_style: [
                    s.particle_shape as f32,
                    s.size_variation,
                    s.aspect,
                    s.softness,
                ],
                particle_detail: [
                    s.ring_width,
                    s.rotation.to_radians(),
                    s.spin.to_radians(),
                    s.rotation_variation,
                ],
                particle_color: [
                    s.color_palette as f32,
                    s.color_mode as f32,
                    s.color_spread,
                    s.saturation,
                ],
                particle_light: [
                    s.opacity,
                    s.brightness_variation,
                    s.color_cycle,
                    s.orb_brightness,
                ],
                color_start: [
                    s.color_start[0] as f32 / 255.0,
                    s.color_start[1] as f32 / 255.0,
                    s.color_start[2] as f32 / 255.0,
                    0.0,
                ],
                color_end: [
                    s.color_end[0] as f32 / 255.0,
                    s.color_end[1] as f32 / 255.0,
                    s.color_end[2] as f32 / 255.0,
                    0.0,
                ],
            },
            symphos_visuals::particle_count(s.quality),
            if s.quality == 0 { 0.5 } else { 0.75 },
        );
        camera::draw_guides(ui, rect, s, theme);
        if s.show_gizmo {
            camera::gizmo(ui, gizmo_rect, s);
        }
    }
}
#[cfg(test)]
mod tests;

fn choice(ui: &mut egui::Ui, label: &str, value: &mut u32, choices: &[&str], help: &str) {
    property_row(ui, label, |ui| {
        properties_combo(label, ui.available_width(), choices.len())
            .selected_text(choices[(*value as usize).min(choices.len() - 1)])
            .show_ui(ui, |ui| {
                for (i, name) in choices.iter().enumerate() {
                    ui.selectable_value(value, i as u32, *name);
                }
            })
            .response
            .help_text(help);
    });
}

fn rgba(color: [u8; 3]) -> [f32; 4] {
    [
        color[0] as f32 / 255.0,
        color[1] as f32 / 255.0,
        color[2] as f32 / 255.0,
        0.0,
    ]
}

fn wrap_angle(v: f32) -> f32 {
    (v + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

impl OrbitalSettings {
    fn apply_look(&mut self, solar: bool) {
        self.orb_style = if solar { 8 } else { 7 };
        self.orb_color_mode = 1;
        self.orb_palette = if solar { 5 } else { 4 };
        self.orb_color_spread = 0.75;
        self.orb_surface_gradient = 0.2;
        self.orb_scale = 2.8;
        self.orb_aspect = 1.0;
        self.orb_size_variation = 0.3;
        self.orb_ambient = 0.025;
        self.orb_rim = 2.5;
        self.orb_rim_power = 5.0;
        self.orb_opacity = 1.0;
        self.orb_brightness = 1.0;
        self.orb_core_darkness = if solar { 0.8 } else { 1.0 };
        self.orb_particle_light = 2.5;
        self.opacity = 0.45;
        self.orb_lens = if solar { 0.12 } else { 0.5 };
        self.orb_lens_reach = 2.8;
        self.orb_dispersion = 0.12;
        self.orb_corona = if solar { 1.0 } else { 0.55 };
        self.orb_corona_width = if solar { 0.3 } else { 0.12 };
        self.orb_rays = if solar { 0.65 } else { 0.3 };
        self.orb_ray_length = if solar { 2.5 } else { 1.7 };
        self.orb_ray_density = 64.0;
        self.orb_corona_spin = 6.0;
        self.orb_burst = 1.5;
        self.orb_burst_size = 0.2;
        self.orb_field_enabled = true;
        self.orb_solid = true;
        self.field_clearance = 0.05;
        self.field_reach = 1.2;
        self.field_repulsion = 18.0;
        self.field_flow = 3.0;
        self.particle_shape = 0;
        self.size = 0.9;
        self.size_variation = 0.5;
        self.streak = if solar { 1.8 } else { 0.8 };
        self.color_palette = if solar { 5 } else { 4 };
        self.color_mode = 4;
        self.color_spread = 0.75;
        self.brightness = 0.7;
        self.bloom = 0.65;
        self.trails = 0.06;
        self.attraction = 1.2;
        self.orbit = 2.3;
        self.cohesion = 0.45;
        self.alignment = 1.4;
        self.noise = 0.35;
    }
}
#[derive(Default)]
struct BurstEnvelope {
    value: f32,
    baseline: f32,
    cooldown: f32,
    last: Option<(u64, u64)>,
}
impl BurstEnvelope {
    fn update(
        &mut self,
        frame: &AnalysisFrame,
        live: bool,
        dt: f32,
        release_ms: f32,
        sensitivity: f32,
    ) -> f32 {
        if dt <= 0.0 {
            return self.value;
        }
        self.value *= (-dt / (release_ms * 0.001).max(0.04)).exp();
        self.cooldown = (self.cooldown - dt).max(0.0);
        if !live {
            self.last = None;
            self.baseline = 0.0;
            return self.value;
        }
        let id = (frame.capture_epoch, frame.sequence);
        if self.last != Some(id) {
            let level = if frame.mix_rms.is_finite() {
                frame.mix_rms.max(0.0)
            } else {
                0.0
            };
            // Prime a new capture with its current level, avoiding a connection flash.
            if self.last.is_none_or(|last| last.0 != id.0) {
                self.baseline = level;
            }
            let onset =
                ((level - self.baseline) * sensitivity / (self.baseline + 0.025)).clamp(0.0, 1.0);
            if level > 0.008 && onset > 0.25 && self.cooldown <= 0.0 {
                self.value = self.value.max(onset);
                self.cooldown = 0.12;
            }
            self.baseline += (level - self.baseline) * (1.0 - (-dt / 0.22).exp());
            self.last = Some(id);
        }
        self.value
    }
}
