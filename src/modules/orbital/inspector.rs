//! Orbital inspector organization. Field IDs and numeric bounds live in the settings catalog.
use super::{
    Orbital, OrbitalSettings, camera, choice, properties_combo, property_row, settings_panel,
};
use crate::{help::HoverHelp, modules::settings_group_with_default};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Extra {
    None,
    Population,
    ParticleShape,
    ParticleColor,
    Paths,
    Arrangement,
    OrbStyle,
    OrbColor,
    OrbPalette,
    Field,
    CameraBasic,
    CameraAdvanced,
    Guides,
    Looks,
}
struct Group {
    title: &'static str,
    numeric: &'static str,
    open: bool,
    extra: Extra,
}
const fn group(title: &'static str, numeric: &'static str, open: bool, extra: Extra) -> Group {
    Group {
        title,
        numeric,
        open,
        extra,
    }
}
const PARTICLES: &[Group] = &[
    group("Population & quality", "", true, Extra::Population),
    group("Size & shape", "Particle size", true, Extra::ParticleShape),
    group("Appearance", "Particle appearance", true, Extra::None),
    group("Color", "Particle color", false, Extra::ParticleColor),
    group("Orientation", "Particle orientation", false, Extra::None),
    group("Flow & motion", "Particle motion", false, Extra::None),
    group("Flocking", "Flocking", false, Extra::None),
    group("Turbulence", "Turbulence", false, Extra::None),
    group("Cloud boundaries", "Cloud boundaries", false, Extra::None),
    group("Paths & trails", "Paths and trails", false, Extra::Paths),
];
const ORBS: &[Group] = &[
    group(
        "Arrangement & motion",
        "Orb arrangement",
        true,
        Extra::Arrangement,
    ),
    group("Size & shape", "Orb size", true, Extra::None),
    group("Appearance", "Orb appearance", true, Extra::OrbStyle),
    group("Color", "Orb color", false, Extra::OrbColor),
    group(
        "Palette adjustments",
        "Orb palette",
        false,
        Extra::OrbPalette,
    ),
    group("Lighting", "Orb lighting", false, Extra::None),
    group("Surface details", "Orb surface", false, Extra::None),
    group("Size pulsing", "Orb pulse", false, Extra::None),
    group(
        "Attraction & orbit",
        "Attraction and orbit",
        false,
        Extra::None,
    ),
    group("Surface field", "Surface field", false, Extra::Field),
    group("Lens & depth", "Lens and depth", false, Extra::None),
    group("Corona & rays", "Corona and rays", false, Extra::None),
];
const RESPONSE: &[Group] = &[
    group("Input smoothing", "Audio smoothing", true, Extra::None),
    group(
        "Movement & light",
        "Audio movement and light",
        true,
        Extra::None,
    ),
    group("Orb expansion", "Audio orb size", true, Extra::None),
    group("Transient flares", "Transient flares", true, Extra::None),
];
const CAMERA: &[Group] = &[
    group("Basic", "Camera basic", true, Extra::CameraBasic),
    group("Advanced", "Camera advanced", false, Extra::CameraAdvanced),
    group("Spatial guides", "Camera guides", true, Extra::Guides),
];
const RENDERING: &[Group] = &[
    group("Starting looks", "", true, Extra::Looks),
    group(
        "Overall light & rendering",
        "Scene rendering",
        true,
        Extra::None,
    ),
    group("Simulation clock", "Simulation clock", true, Extra::None),
];
const CATEGORIES: &[(&str, &[Group])] = &[
    ("Particles", PARTICLES),
    ("Orbs", ORBS),
    ("Signal Response", RESPONSE),
    ("Camera", CAMERA),
    ("Rendering", RENDERING),
];
const PALETTES: &[&str] = &[
    "Spectral",
    "Ember",
    "Glacier",
    "Aurora",
    "Candy",
    "Gold",
    "Monochrome",
    "Custom gradient",
];

/// Old saved category indices map to the nearest consolidated category.
pub(crate) fn legacy_section(index: usize) -> usize {
    match index {
        1 | 6 | 8 | 9 | 10 => 1,
        3 => 2,
        5 => 3,
        _ => 0,
    }
}

pub(super) fn controls(ui: &mut egui::Ui, orbital: &mut Orbital) {
    for &(title, groups) in CATEGORIES {
        settings_panel(ui, title, |ui| {
            for g in groups {
                if g.extra == Extra::OrbPalette && orbital.settings.orb_color_mode != 1 {
                    continue;
                }
                let label = if super::super::visual_inspector(ui).is_some()
                    && matches!(title, "Particles" | "Orbs")
                {
                    format!("{title} · {}", g.title)
                } else {
                    g.title.to_owned()
                };
                settings_group_with_default(ui, &label, g.open, |ui| {
                    if g.extra == Extra::Population {
                        population(ui, orbital);
                    } else {
                        extras(ui, &mut orbital.settings, g.extra);
                    }
                    if g.extra == Extra::Field {
                        ui.add_enabled_ui(orbital.settings.orb_field_enabled, |ui| {
                            orbital.settings.controls(ui, g.numeric)
                        });
                    } else if !g.numeric.is_empty() {
                        orbital.settings.controls(ui, g.numeric);
                    }
                });
            }
        });
    }
}
fn population(ui: &mut egui::Ui, orbital: &mut Orbital) {
    property_row(ui, "Particle count", |ui| {
        properties_combo("orbital-quality",ui.available_width(),4)
            .selected_text(format!("{}",symphos_visuals::particle_count(orbital.settings.quality)))
            .show_ui(ui,|ui| {
                for (i,name) in ["16,384 · Low","32,768 · Balanced","65,536 · High","131,072 · Ultra"].iter().enumerate() {
                    ui.selectable_value(&mut orbital.settings.quality,i as u32,*name);
                }
            }).response.help_text("Per-component particle count and render quality. Changing it restarts the simulation. Start with Balanced for multiple panes. Target display FPS is in Settings → Display & Layout.");
    });
    if ui.button("Restart particles").help_text("Reset the particle simulation and clear trails while retaining controls and macro mappings.").clicked() {orbital.clear();}
}
fn extras(ui: &mut egui::Ui, s: &mut OrbitalSettings, extra: Extra) {
    match extra {
        Extra::ParticleShape => choice(
            ui,
            "Shape",
            &mut s.particle_shape,
            &[
                "Soft glow",
                "Disc",
                "Square",
                "Diamond",
                "Ring",
                "Spark",
                "Arrow",
            ],
            "Particle silhouette. Arrow follows velocity and ignores decorative rotation/spin. Paths & trails contains velocity stretching and recorded trajectories.",
        ),
        Extra::ParticleColor => {
            choice(
                ui,
                "Palette",
                &mut s.color_palette,
                PALETTES,
                "Particle color family. Orbs can follow this palette or use independent colors in Orbs → Color.",
            );
            choice(
                ui,
                "Color by",
                &mut s.color_mode,
                &[
                    "Particle identity",
                    "Speed",
                    "Distance from center",
                    "Height",
                    "Nearest attractor",
                ],
                "Choose what samples the palette. Identity stays fixed per particle; other modes reveal live motion and structure.",
            );
            if s.color_palette == 7 {
                swatch(
                    ui,
                    "Start color",
                    &mut s.color_start,
                    "Start of the particle gradient.",
                );
                swatch(
                    ui,
                    "End color",
                    &mut s.color_end,
                    "End of the particle gradient.",
                );
            }
        }
        Extra::Paths => {
            ui.checkbox(&mut s.show_paths,"Show particle paths").help_text("Record recent 3D trajectories for a sampled set of particles. Paths reproject when the camera moves and pause with the simulation. Motion streak stretches particle heads; Trail half-life is screen-space light persistence, a separate effect.");
        }
        Extra::Arrangement => {
            property_row(ui, "Attractors", |ui| {
                ui.add(egui::DragValue::new(&mut s.attractors).range(1..=5)).help_text("One to five attractor orbs. Orb spacing and motion place their centers; Attraction & orbit sets how they steer particles.");
            });
        }
        Extra::OrbStyle => choice(
            ui,
            "Render style",
            &mut s.orb_style,
            &[
                "Shaded sphere",
                "Unlit disc",
                "Outline",
                "Wireframe",
                "Latitude bands",
                "Plasma",
                "Soft glow",
                "Lensing core",
                "Radiant star",
            ],
            "Analytic orb surface style. Lighting, Surface details, Lens & depth and Corona & rays refine the look. Attraction and collisions are configured separately.",
        ),
        Extra::OrbColor => {
            choice(
                ui,
                "Color source",
                &mut s.orb_color_mode,
                &[
                    "Match particles",
                    "Own palette",
                    "Solid color",
                    "Per-orb colors",
                ],
                "Follow the particle palette, use an independent palette, or choose one color for all orbs or a separate color per orb.",
            );
            if s.orb_color_mode == 1 {
                choice(
                    ui,
                    "Orb palette",
                    &mut s.orb_palette,
                    PALETTES,
                    "Independent orb palette. Palette adjustments appears below in this mode.",
                );
            }
            if s.orb_color_mode == 2 || (s.orb_color_mode == 1 && s.orb_palette == 7) {
                swatch(
                    ui,
                    "Orb color / start",
                    &mut s.orb_color_start,
                    "Solid orb color or start of its independent gradient.",
                );
            }
            if s.orb_color_mode == 1 && s.orb_palette == 7 {
                swatch(
                    ui,
                    "Orb end color",
                    &mut s.orb_color_end,
                    "End of the independent orb gradient.",
                );
            }
            if s.orb_color_mode == 3 {
                for i in 0..s.attractors as usize {
                    swatch(
                        ui,
                        &format!("Orb {} color", i + 1),
                        &mut s.orb_colors[i],
                        "This orb's color is retained if the attractor count is reduced and restored.",
                    );
                }
            }
        }
        Extra::Field => {
            ui.checkbox(&mut s.orb_field_enabled,"Enable surface field").help_text("Bend particles around each orb with an adjustable repulsive and circulating field. Off preserves the original center attraction and repulsion.");
            ui.add_enabled_ui(s.orb_field_enabled,|ui| {ui.checkbox(&mut s.orb_solid,"Solid surface").help_text("Keep particle centers outside the orb plus clearance, sliding along its moving surface. Off leaves a permeable soft field. Visibility is independently controlled by Core occlusion in Lens & depth. Avoid deeply overlapping large orbs for clean flow.");});
        }
        Extra::CameraBasic => camera::basic_controls(ui, s),
        Extra::CameraAdvanced => camera::advanced_controls(ui, s),
        Extra::Guides => camera::guide_controls(ui, s),
        Extra::Looks => {
            if ui.button("Lensing swarm").help_text("Apply a dark-core violet/cyan starting look, including particle appearance and orb surface flow. Retains camera, quality, paths and macro assignments. Signal mappings remain active.").clicked() {s.apply_look(false);}
            if ui.button("Solar ribbons").help_text("Apply warm radiant orbs, coronas, golden particles and surface flow. Retains camera, quality, paths and macro assignments. Signal mappings remain active.").clicked() {s.apply_look(true);}
        }
        Extra::None | Extra::Population | Extra::OrbPalette => {}
    }
}
fn swatch(ui: &mut egui::Ui, title: &str, color: &mut [u8; 3], help: &str) {
    property_row(ui, title, |ui| {
        ui.color_edit_button_srgb(color).help_text(help);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn each_numeric_parameter_has_one_group_and_audio_has_one_home() {
        let mut homes = std::collections::HashMap::new();
        for &(category, groups) in CATEGORIES {
            for g in groups {
                if !g.numeric.is_empty() {
                    assert!(
                        homes.insert(g.numeric, category).is_none(),
                        "duplicate group {}",
                        g.numeric
                    );
                }
            }
        }
        let fields = OrbitalSettings::inspector_fields();
        assert_eq!(fields.len(), 111);
        let mut seen = std::collections::HashSet::new();
        for &(field, group) in &fields {
            assert!(seen.insert(field), "duplicate field {field}");
            let category = homes
                .get(group)
                .unwrap_or_else(|| panic!("missing inspector group for {field}: {group}"));
            if matches!(
                field,
                "response_ms"
                    | "audio_force"
                    | "audio_light"
                    | "orb_audio_size"
                    | "orb_burst"
                    | "orb_burst_decay"
                    | "orb_burst_sensitivity"
                    | "orb_burst_size"
                    | "orb_burst_drive"
            ) {
                assert_eq!(
                    *category, "Signal Response",
                    "{field} must have one reactive home"
                );
            }
        }
        for group in homes.keys() {
            assert!(
                fields.iter().any(|(_, g)| g == group),
                "empty group {group}"
            );
        }
        let particles = PARTICLES
            .iter()
            .filter(|g| g.extra != Extra::Population)
            .count();
        assert!(
            particles > 5 && ORBS.len() > 5,
            "details live in disclosures, not separate tabs"
        );
    }
}
