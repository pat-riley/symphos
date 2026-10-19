//! Contextual descriptions for the docked Info View, not short tooltips.
pub(super) fn parameter(
    field: &str,
    label: &str,
    description: &str,
    section: &str,
    range: [f32; 3],
) -> String {
    let tip = match field {
        "field_clearance" | "field_reach" | "field_repulsion" | "field_flow" | "field_falloff" => {
            "Enable the surface field in Orbs → Surface field first. Start with Solid surface on, reach 0.8 and circulation 2. Increase reach for smoother detours and clearance for a wider halo. Solid surface prevents penetration even when attraction wins; turn it off for a soft, permeable field. Use particle paths to inspect the flow. Core occlusion in Orbs → Lens & depth controls whether particles behind an orb remain visible."
        }
        "orb_core_darkness" | "orb_lens" | "orb_lens_reach" | "orb_dispersion" => {
            "Start with Lensing swarm in Rendering → Starting looks. Core occlusion gives depth by hiding particles behind each orb while foreground particles remain visible. Lens warp bends the rendered image around that shape; Orbs → Surface field controls the actual movement. A little dispersion adds color at the rim. These are stylized optical effects rather than a physical relativity simulation."
        }
        "orb_corona" | "orb_corona_width" | "orb_rays" | "orb_ray_length" | "orb_ray_density"
        | "orb_corona_spin" => {
            "Keep the core dim and raise Corona brightness to separate a dark body from its atmosphere. Add Radial rays gradually; fine sparse rays read better than an evenly bright disc. Halo and rays use each orb's color. They extend beyond the surface without enlarging its collision boundary."
        }
        "orb_burst"
        | "orb_burst_decay"
        | "orb_burst_sensitivity"
        | "orb_burst_size"
        | "orb_burst_drive" => {
            "Automatic bursts detect sudden energy increases in the main audio. For precise control, map your own prepared Signal to Burst drive; it works with audio, MIDI or an LFO. Use a short release for kicks or a longer one for swelling light. Burst expansion also moves the surface field, pushing particles around the growing orb."
        }
        "cohesion" => {
            "Turn this up to gather particles into schools; turn it down to let separate streams drift apart. Balance it with Separation so the cloud does not bunch into a dense knot."
        }
        "alignment" => {
            "Increase this for coordinated, smooth traffic. Reduce it for independent, chaotic directions. Arrow particles make the difference easy to see."
        }
        "separation" | "separation_radius" => {
            "If the center becomes a bright clump, increase this gradually. Cohesion pulls groups together while separation pushes nearby groups apart; both can be active at once."
        }
        "orbit" | "orbit_tilt" => {
            "Try showing a few particle paths and hiding the orbs temporarily. You can see whether the current circles one axis or twists through several planes."
        }
        "attraction" | "repulsion" | "radius" => {
            "Attraction draws particles toward orb centers. Core repulsion keeps them away from the immediate center. The balance creates the cavity around each orb; changing visual size does not change that legacy cavity. For surface-following avoidance, enable the surface field and Solid surface under Orbs."
        }
        "noise" | "noise_scale" | "noise_speed" => {
            "Start with low turbulence to establish a recognizable orbit, then add it gradually. Strength sets how hard it pushes, scale sets the size of the bends, and evolution sets how quickly the field changes."
        }
        "speed" | "minimum_speed" | "agility" | "drag" => {
            "Use low agility for wide turns and high agility for quick reactions. Drag removes momentum; a minimum speed keeps the flock moving. The speed limit always remains the upper bound."
        }
        "hue" | "color_spread" | "saturation" | "color_cycle" => {
            "For one consistent particle color, set Color spread to zero and use Color offset to choose it. For a varied flock, increase spread. Keep saturation near one while choosing a palette."
        }
        "orb_hue" | "orb_color_spread" | "orb_color_cycle" | "orb_surface_gradient" => {
            "Select Own palette in the Orbs section to use this control. These settings do not recolor the particles. A surface gradient distributes the palette from one pole of each orb to the other."
        }
        "streak" | "rotation" | "spin" | "rotation_variation" => {
            "For arrows that report the true travel direction, use Arrow with zero rotation, spin and rotation variation. Arrow mode locks orientation to velocity; motion streak still controls its length."
        }
        "size" | "size_variation" | "aspect" | "softness" | "ring_width" => {
            "Start with a small size and little or no motion streak to see the individual shape. Large particles overlap more and can wash out the image, especially with bloom and long glow trails."
        }
        "brightness" | "exposure" | "bloom" | "trails" => {
            "Overall brightness, bloom and exposure live in Rendering and affect particles and orbs together. If everything becomes white, reduce brightness or exposure before changing the palette. Bloom spreads bright light; the glow trail retains old pixels. Stored particle paths are configured in Particles → Paths & trails."
        }
        "orb_roughness"
        | "orb_specular"
        | "orb_ambient"
        | "lighting"
        | "orb_rim"
        | "orb_rim_power"
        | "orb_light_azimuth"
        | "orb_light_elevation" => {
            "Use Shaded sphere, Wireframe, Latitude bands or Plasma to see lighting changes. Unlit disc, Outline and Soft glow deliberately ignore surface lighting. Roughness needs a nonzero highlight strength."
        }
        "orb_detail"
        | "orb_line_width"
        | "orb_surface_rotation"
        | "orb_surface_spin"
        | "orb_surface_tilt" => {
            "Use Wireframe, Latitude bands or Plasma to study the surface. Try a low density first so the lines are easy to see. Surface spin turns the pattern without moving the orb's attraction field."
        }
        _ => match section {
            "Particle motion"
            | "Flocking"
            | "Turbulence"
            | "Cloud boundaries"
            | "Attraction and orbit" => {
                "Change one force at a time. Show particle paths to inspect motion over time, or switch to Arrow particles to inspect the current direction. The grid helps distinguish a change in motion from camera movement."
            }
            "Camera basic" | "Camera advanced" | "Camera guides" => {
                "This changes your view or its guides, not the simulation. Front, Top and Side views are useful for checking depth; Reset camera restores a familiar perspective without resetting the particles."
            }
            "Paths and trails" => {
                "Paths are recorded in world space at up to 30 samples per simulation second. They remain in place when the camera turns. Only a subset is traced, so the cloud stays readable and the GPU cost stays bounded."
            }
            "Orb size" | "Orb appearance" | "Orb lighting" | "Orb color" => {
                "Orb appearance is independent of its force field. You can hide an orb and particles will still circulate around it. Brightness and opacity control emitted light; Core occlusion independently controls visibility behind the surface."
            }
            "Audio smoothing" | "Audio movement and light" | "Audio orb size" => {
                "These controls use the main audio input. Set their strength to zero if you want motion driven only by manual values or Signals mapped through macros. Missing audio fades away; the simulation can continue in silence."
            }
            "Orb arrangement" => {
                "Use the XYZ grid to judge distances. These settings arrange the force field in world space. Visual orb size and surface styles are in Orbs, while camera zoom changes only your view."
            }
            _ => {
                "Try small adjustments while keeping the other controls steady. Appearance settings are saved with the component and can be changed without restarting the particle simulation."
            }
        },
    };
    format!(
        "{label}\n{description}\n\n{tip}\n\nRange: {} to {}. Default: {}.\nDrag to adjust; use Shift for finer changes. Drag the parameter label onto a scene macro to control it from a Signal.",
        range[0], range[1], range[2]
    )
}
