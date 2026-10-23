use crate::help::HoverHelp;
use egui::Ui;

macro_rules! numeric {
    ($name:ident { $($field:ident: $default:expr, $min:expr, $max:expr, $section:literal, $label:literal, $help:literal;)* }) => {
        impl $name {
            pub fn numeric_controls(&mut self, ui: &mut Ui, section: &str, prefix: &str) {
                $(if !(section == "Camera" && stringify!($field) == "orbit_speed" && super::super::visual_inspector(ui).is_some()) && (section == $section || (section == "Bloom drift" && $section == "Drift and return" && stringify!($field) != "drift_fraction") || (section == "Bloom layout" && $section == "Layout" && !matches!(stringify!($field), "radius" | "turns" | "twist"))) {
                    ui.add(crate::parameter::Parameter::new(&mut self.$field, $min..=$max, $default)
                        .key(&format!("{prefix}{}", stringify!($field))).text($label)).help_text($help);
                })*
            }
            fn sanitize_numbers(&mut self) {
                $(self.$field = if self.$field.is_finite() {self.$field.clamp($min, $max)} else {$default};)*
            }
        }
    }
}
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub(crate) struct Layer {
    pub enabled: bool,
    pub custom: bool,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub rotation: f32,
    pub tilt: f32,
    pub scale: f32,
    pub size: f32,
    pub density: f32,
    pub speed: f32,
    pub opacity: f32,
    pub color_a: [u8; 3],
    pub color_b: [u8; 3],
    pub color_c: [u8; 3],
}
impl Default for Layer {
    fn default() -> Self {
        Self {
            enabled: true,
            custom: false,
            x: 0.0,
            y: 0.0,
            z: 0.0,
            rotation: 0.0,
            tilt: 0.0,
            scale: 1.0,
            size: 1.0,
            density: 1.0,
            speed: 1.0,
            opacity: 1.0,
            color_a: [61, 119, 255],
            color_b: [136, 255, 32],
            color_c: [235, 255, 255],
        }
    }
}
numeric!(Layer {
    x:0.0,-6.0,6.0,"Transform","Position X","Move this layer along world X. Its source spectrum remains unchanged.";
    y:0.0,-6.0,6.0,"Transform","Position Y","Move this layer vertically in addition to the shared layer spacing.";
    z:0.0,-6.0,6.0,"Transform","Position Z","Move this layer toward or away from the center along world Z.";
    rotation:0.0,-180.0,180.0,"Transform","Rotation °","Rotate this layer around its vertical axis, independently of the camera.";
    tilt:0.0,-180.0,180.0,"Transform","Tilt °","Tilt this layer around its horizontal axis. Camera orbit remains independent.";
    scale:1.0,0.1,3.0,"Transform","Scale","Scale the whole layer including its audio displacement and particle drift.";
    size:1.0,0.1,4.0,"Override","Particle size ×","Multiply the shared particle radius for this layer. Depth of field still applies.";
    density:1.0,0.0,1.0,"Override","Density","Keep this fraction of the layer's particles, using a stable pattern that does not flicker.";
    speed:1.0,0.0,3.0,"Override","Motion speed ×","Multiplier for this layer's particle drift and illustrated resonance phase. Does not change the time or frequency represented by its spectrum history.";
    opacity:1.0,0.0,1.0,"Override","Opacity","Fade this layer independently. Zero makes it invisible while retaining its settings.";
});

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub(crate) struct Settings {
    pub show_gizmo: bool,
    pub camera_motion: super::super::camera_motion::Settings,
    pub bloom: Bloom,
    pub frequency: super::super::frequency::FrequencySettings,
    pub layers: [Layer; 8],
    pub layer_count: u32,
    pub mode: u32,
    pub quality: u32,
    pub shape: u32,
    pub color_mode: u32,
    pub orthographic: bool,
    pub auto_orbit: bool,
    pub guides: bool,
    pub depth_of_field: bool,
    pub color_a: [u8; 3],
    pub color_b: [u8; 3],
    pub color_c: [u8; 3],
    pub width: f32,
    pub depth: f32,
    pub height: f32,
    pub spacing: f32,
    pub history_seconds: f32,
    pub radius: f32,
    pub turns: f32,
    pub twist: f32,
    pub size: f32,
    pub size_variation: f32,
    pub density: f32,
    pub opacity: f32,
    pub softness: f32,
    pub glow: f32,
    pub brightness: f32,
    pub color_spread: f32,
    pub color_blend: f32,
    pub color_offset: f32,
    pub saturation: f32,
    pub drift: f32,
    pub drift_fraction: f32,
    pub return_strength: f32,
    pub drag: f32,
    pub turbulence: f32,
    pub motion_speed: f32,
    pub audio_scatter: f32,
    pub threshold: f32,
    pub attack_ms: f32,
    pub release_ms: f32,
    pub ripple: f32,
    pub ripple_scale: f32,
    pub ripple_speed: f32,
    pub audio_size: f32,
    pub audio_light: f32,
    pub focus_distance: f32,
    pub aperture: f32,
    pub max_blur: f32,
    pub yaw: f32,
    pub elevation: f32,
    pub roll: f32,
    pub zoom: f32,
    pub pan_x: f32,
    pub pan_y: f32,
    pub camera_distance: f32,
    pub field_of_view: f32,
    pub orbit_speed: f32,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            camera_motion: super::super::camera_motion::Settings::y_up_default(),
            show_gizmo: false,
            bloom: Bloom::default(),
            frequency: super::super::frequency::FrequencySettings {
                floor: -72.0,
                ceiling: -6.0,
                smoothing: 0.45,
                frequency_smoothing: 1,
                ..Default::default()
            },
            layers: std::array::from_fn(|_| Layer::default()),
            layer_count: 4,
            mode: 0,
            quality: 1,
            shape: 0,
            color_mode: 0,
            orthographic: false,
            auto_orbit: false,
            guides: false,
            depth_of_field: true,
            color_a: [61, 119, 255],
            color_b: [136, 255, 32],
            color_c: [235, 255, 255],
            width: 5.5,
            depth: 5.0,
            height: 1.6,
            spacing: 0.6,
            history_seconds: 3.0,
            radius: 1.5,
            turns: 2.0,
            twist: 0.15,
            size: 1.6,
            size_variation: 0.7,
            density: 0.85,
            opacity: 0.85,
            softness: 0.35,
            glow: 0.45,
            brightness: 0.85,
            color_spread: 1.0,
            color_blend: 0.2,
            color_offset: 0.0,
            saturation: 1.0,
            drift: 0.3,
            drift_fraction: 0.25,
            return_strength: 9.0,
            drag: 4.0,
            turbulence: 1.3,
            motion_speed: 1.0,
            audio_scatter: 1.5,
            threshold: 0.18,
            attack_ms: 25.0,
            release_ms: 260.0,
            ripple: 0.15,
            ripple_scale: 3.0,
            ripple_speed: 1.0,
            audio_size: 0.4,
            audio_light: 0.6,
            focus_distance: 8.0,
            aperture: 0.7,
            max_blur: 24.0,
            yaw: -0.55,
            elevation: 0.48,
            roll: -0.12,
            zoom: 1.3,
            pan_x: 0.0,
            pan_y: 0.0,
            camera_distance: 8.5,
            field_of_view: 53.13,
            orbit_speed: 0.14,
        }
    }
}
numeric!(Settings {
    width:5.5,1.0,12.0,"Layout","Width","Frequency span of the ribbons in world units. Low frequencies start at the left end before layer rotation.";
    depth:5.0,0.1,12.0,"Layout","History depth","Physical depth occupied by the selected history duration. For rings this becomes radial width; for spirals and tunnels it extends along their axis.";
    height:1.6,0.0,6.0,"Layout","Spectrum height","Displacement produced by the measured frequency level. Zero flattens the frequency profile; it does not mute particle motion.";
    spacing:0.6,0.0,3.0,"Layout","Layer spacing","Distance between neighboring layers. Layers share the same real spectrum and history, with independent transforms and optional styles.";
    history_seconds:3.0,0.25,12.0,"Layout","History seconds","Seconds of incoming spectrum spread from the front to the back. Older sound moves away continuously; this duration is independent of display FPS.";
    radius:1.5,0.2,5.0,"Layout","Radius","Base radius for ring, spiral and tunnel arrangements. Spectrum level displaces the radius in tunnel mode.";
    turns:2.0,0.25,6.0,"Layout","Spiral turns","Number of turns along the frequency axis in Spiral mode. Each position still corresponds to a measured frequency.";
    twist:0.15,-2.0,2.0,"Layout","Twist","Bend the arrangement along its history axis. This changes the placement of the measured spectrum without substituting a generated wave.";
    size:1.6,0.3,8.0,"Particle size","Radius px","In-focus particle radius at the focus plane, measured in a output with a 1080-pixel short side. Scales with recording resolution and perspective.";
    size_variation:0.7,0.0,1.0,"Particle size","Size variation","Stable differences between individual particles. Zero gives equal-size grains; higher values create scattered highlights.";
    density:0.85,0.0,1.0,"Particle size","Density","Visible fraction of the available particles. Lower this for airy dotted ribbons. Increase Particle budget to add detail without changing size.";
    opacity:0.85,0.0,1.0,"Particle size","Opacity","Shared particle visibility. Additive light lets overlapping particles accumulate brightness.";
    softness:0.35,0.0,1.0,"Particle size","Edge softness","Round the edge of each particle from a crisp point to a soft luminous dot. Camera defocus adds further softness.";
    glow:0.45,0.0,2.0,"Light","Glow","Add a broad luminous halo around each particle. Dense layers can become brighter where halos overlap.";
    brightness:0.85,0.0,4.0,"Light","Brightness","Overall emitted light. Has no effect on the incoming audio or particle forces.";
    color_spread:1.0,0.0,1.0,"Palette","Color spread","Portion of the three-color gradient used by particles. Lower values concentrate the first color.";
    color_blend:0.2,0.0,1.0,"Palette","Gradient blend","Blend the three palette colors smoothly or keep distinct colored particles. Low values preserve separate blue, lime and white lights; high values create a continuous gradient.";
    color_offset:0.0,0.0,1.0,"Palette","Color offset","Shift which part of the palette is sampled by the color driver.";
    saturation:1.0,0.0,2.0,"Palette","Saturation","Zero creates monochrome particles; one uses the chosen colors; above one intensifies their saturation.";
    drift:0.3,0.0,3.0,"Drift and return","Drift strength","Audio-driven force that lets grains leave their spectral surface. Zero keeps a precisely attached spectrum. Silence stops new agitation and the return spring gathers particles back.";
    drift_fraction:0.25,0.0,1.0,"Drift and return","Drifting fraction","Fraction of particles allowed to leave the spectral surface. The remaining particles stay attached so the measured ribbons remain easy to read. Zero attaches every grain; one turns the whole layer into a drifting cloud.";
    return_strength:9.0,0.5,30.0,"Drift and return","Return strength","Spring pulling each particle back to its moving frequency position. Higher values maintain crisp ribbons; lower values allow a loose cloud.";
    drag:4.0,0.5,15.0,"Drift and return","Motion damping","Remove particle momentum smoothly. Higher damping calms overshoot and helps particles settle without snapping.";
    turbulence:1.3,0.1,6.0,"Drift and return","Flow detail","Spatial scale of the smooth swirling force field. Small values produce broad currents; high values produce finer agitation.";
    motion_speed:1.0,0.0,3.0,"Drift and return","Flow speed","How fast the drifting force field evolves. Zero holds the field while particles can still respond and return to the surface.";
    audio_scatter:1.5,0.0,6.0,"Audio dynamics","Audio scatter","Extra drift on stronger input. The measured spectrum always sets the main shape; this controls only how far grains leave it.";
    threshold:0.18,0.0,0.95,"Audio dynamics","Scatter threshold","Normalized measured level above which scattering begins. Raise it so only louder notes break grains away.";
    attack_ms:25.0,1.0,500.0,"Audio dynamics","Motion attack ms","How quickly the particle-motion envelope responds to an incoming peak. Does not smooth or reshape the frequency spectrum itself.";
    release_ms:260.0,10.0,3000.0,"Audio dynamics","Motion release ms","How long audio-driven agitation persists after a peak. Short release makes the return spring respond quickly to musical gaps.";
    ripple:0.15,0.0,2.0,"Resonant motion","Resonant ripple","Add traveling ripples proportional to the measured local spectral level. Inspired by vibrating plates: the ripple disappears with the sound. Zero leaves a direct frequency-height surface.";
    ripple_scale:3.0,0.25,12.0,"Resonant motion","Ripple detail","Number of illustrated wavefronts across each ribbon. This is a visual response, not a calibrated physical plate simulation.";
    ripple_speed:1.0,0.0,5.0,"Resonant motion","Ripple speed","Slow-motion phase rate of the audio-driven ripples. Incoming dominant frequency also changes their spacing.";
    audio_size:0.4,0.0,3.0,"Audio dynamics","Audio size","How strongly local frequency level enlarges its particles. Zero keeps size independent of audio.";
    audio_light:0.6,0.0,3.0,"Audio dynamics","Audio light","How strongly local spectral level brightens its particles. Zero keeps their appearance constant while shape and motion still respond.";
    focus_distance:8.0,0.5,25.0,"Focus","Focus distance","Camera-space distance where particles are sharp. Match it to Camera distance to focus around the center, or move it nearer for sharp foreground particles.";
    aperture:0.7,0.0,3.0,"Focus","Defocus strength","How quickly particles outside the focus plane become soft bokeh circles. Zero keeps all depths sharp. Light is spread over the blurred area to avoid a bright fog.";
    max_blur:24.0,1.0,80.0,"Focus","Maximum blur px","Limit the bokeh radius at 1080-pixel output short side. Larger values create the oversized foreground lights in the reference and cost more GPU fill time.";
    yaw:-0.55,-std::f32::consts::PI,std::f32::consts::PI,"Camera","Yaw","Horizontal camera angle. Drag the canvas for unrestricted trackball rotation, including roll.";
    elevation:0.48,-std::f32::consts::PI,std::f32::consts::PI,"Camera","Elevation","Vertical camera angle. You can rotate above, below, or completely around the particle layers.";
    roll:-0.12,-std::f32::consts::PI,std::f32::consts::PI,"Camera","Roll","Rotate around the viewing axis independently of the particle geometry.";
    zoom:1.3,0.25,4.0,"Camera","Zoom","Magnification. Scroll over the view to zoom without changing its focus distance.";
    pan_x:0.0,-2.0,2.0,"Camera","Pan X","Horizontal camera framing. Shift-drag or right-drag over the view to pan.";
    pan_y:0.0,-2.0,2.0,"Camera","Pan Y","Vertical camera framing. Panning does not move the particle layers or affect the audio.";
    camera_distance:8.5,2.0,25.0,"Camera","Camera distance","Distance from the camera to the origin. Move closer for stronger perspective and large foreground particles.";
    field_of_view:53.13,20.0,100.0,"Camera","Field of view °","Lens angle. Wide views exaggerate depth; narrow views compress perspective.";
    orbit_speed:0.14,-1.0,1.0,"Camera","Auto-rotate speed","Radians per second while Auto-rotate is enabled. Negative values reverse direction. Manual rotation stops automatic rotation.";
});
impl Settings {
    pub fn sanitize(&mut self) {
        self.sanitize_numbers();
        self.bloom.sanitize_numbers();
        self.layer_count = self.layer_count.clamp(1, 8);
        self.mode = self.mode.min(3);
        self.quality = self.quality.min(3);
        self.shape = self.shape.min(3);
        self.color_mode = self.color_mode.min(3);
        for layer in &mut self.layers {
            layer.sanitize_numbers();
        }
        // Saved files and modulation must never send invalid values into the GPU.
        let f = &mut self.frequency;
        for (value, default, min, max) in [
            (&mut f.min_hz, 20.0, 1.0, 20000.0),
            (&mut f.max_hz, 20000.0, 2.0, 192000.0),
            (&mut f.floor, -72.0, -140.0, 0.0),
            (&mut f.ceiling, -6.0, -24.0, 48.0),
            (&mut f.gain, 0.0, -96.0, 96.0),
            (&mut f.smoothing, 0.45, 0.0, 0.98),
            (&mut f.decay, 36.0, 0.0, 240.0),
        ] {
            *value = if value.is_finite() {
                value.clamp(min, max)
            } else {
                default
            };
        }
        f.max_hz = f.max_hz.max(f.min_hz + 1.0);
        f.ceiling = f.ceiling.max(f.floor + 1.0);
        f.frequency_smoothing = f.frequency_smoothing.min(8);
    }
}

/// Settings for the sculptural spectrum profile; ignored by Particle Waves.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub(crate) struct Bloom {
    pub columns: f32,
    pub bend: f32,
    pub curl: f32,
    pub fall: f32,
    pub cloud: f32,
    pub roughness: f32,
    pub strand_width: f32,
    pub cloud_size: f32,
    pub cloud_light: f32,
    pub idle_light: f32,
    pub feedback_enabled: bool,
    pub persistence: f32,
    pub expansion: f32,
    pub rotation: f32,
    pub warp: f32,
    pub shift_x: f32,
    pub shift_y: f32,
    pub chroma: f32,
}
impl Default for Bloom {
    fn default() -> Self {
        Self {
            columns: 96.0,
            bend: 1.1,
            curl: 0.45,
            fall: 1.0,
            cloud: 0.22,
            roughness: 0.35,
            strand_width: 0.65,
            cloud_size: 2.3,
            cloud_light: 1.6,
            idle_light: 0.015,
            feedback_enabled: true,
            persistence: 0.35,
            expansion: 0.06,
            rotation: 0.025,
            warp: 0.1,
            shift_x: 0.0,
            shift_y: 0.025,
            chroma: 0.5,
        }
    }
}
numeric!(Bloom {
    columns:96.0,16.0,128.0,"Sculpture","Frequency columns","Number of discrete measured frequency columns. Fewer columns make individual bars clearer; higher counts form a fine luminous curtain.";
    bend:1.1,-4.0,4.0,"Sculpture","Strand bend","Sweep the lower portion of each bar sideways. The top remains a direct reading of its measured frequency level.";
    curl:0.45,0.0,3.0,"Sculpture","Curl","Fold the lower strands into rolling lobes. Local spectral energy controls their movement; silent columns do not develop artificial peaks.";
    fall:1.0,0.0,4.0,"Sculpture","Tail length","Extend the flowing tail below the frequency columns. Higher values create a cascading sculpture.";
    cloud:0.22,0.0,0.8,"Sculpture","Dissolve to particles","Fraction of strands replaced by free luminous grains around the lower folds. Zero renders only connected filaments.";
    roughness:0.35,0.0,2.0,"Sculpture","Particle spread","Spread dissolved grains around the folds. The audio-driven drift controls add motion on top of this stable distribution.";
    strand_width:0.65,0.1,3.0,"Sculpture","Strand width","Width of connected filaments relative to particle radius. Thin strands preserve the multicolored frequency bars.";
    cloud_size:2.3,0.2,6.0,"Sculpture","Cloud particle size ×","Size multiplier for the dissolved grains only. Raise this for luminous clusters beneath the fine spectrum filaments.";
    cloud_light:1.6,0.0,4.0,"Sculpture","Cloud brightness ×","Light emitted by dissolved grains relative to the connected strands. Shape and measured frequency levels remain unchanged.";
    idle_light:0.015,0.0,1.0,"Sculpture","Idle light","Light remaining in silent frequency columns. Zero fades the sculpture away without audio; a small value leaves a faint resting silhouette.";
    persistence:0.35,0.03,3.0,"Feedback","Echo lifetime s","Time for old rendered light to fade to about 37 percent. This is real image feedback, measured in seconds independently of display FPS. Disable feedback for the clean live sculpture.";
    expansion:0.06,-0.5,0.5,"Feedback","Echo expansion","Scale old light away from the center per second. Negative values pull echoes inward. Small values create delicate receding copies.";
    rotation:0.025,-0.8,0.8,"Feedback","Echo rotation","Rotate the previous image in radians per second. This affects lingering light rather than the 3D camera or current spectrum.";
    warp:0.1,0.0,2.0,"Feedback","Echo distortion","Audio-sensitive flowing distortion applied to old light. Zero leaves clean repeated shapes; high values create liquid trails.";
    shift_x:0.0,-0.5,0.5,"Feedback","Echo drift X","Horizontal screen-space drift of old light per second. The current frequency columns remain fixed in world space.";
    shift_y:0.025,-0.5,0.5,"Feedback","Echo drift Y","Vertical screen-space drift of old light per second. Positive values move echoes downward.";
    chroma:0.5,0.0,4.0,"Feedback","Color separation","Separate red and blue samples in the feedback image for iridescent edges. Does not alter the input spectrum.";
});
impl Settings {
    pub fn spectral_bloom() -> Self {
        Self {
            layer_count: 3,
            layers: std::array::from_fn(|i| Layer {
                x: i as f32 * 0.2,
                y: -(i as f32) * 0.55,
                z: i as f32 * 0.2,
                scale: (1.0 - i as f32 * 0.1).max(0.4),
                ..Layer::default()
            }),
            width: 5.0,
            depth: 2.8,
            height: 4.5,
            spacing: 0.2,
            size: 1.15,
            size_variation: 0.4,
            density: 1.0,
            opacity: 0.7,
            brightness: 1.1,
            color_a: [72, 175, 255],
            color_b: [255, 124, 156],
            color_c: [244, 243, 213],
            color_mode: 1,
            color_blend: 0.9,
            glow: 0.35,
            drift: 0.15,
            drift_fraction: 0.22,
            ripple: 0.0,
            aperture: 0.12,
            yaw: -0.5,
            elevation: 0.25,
            roll: 0.0,
            zoom: 1.1,
            ..Self::default()
        }
    }
}
