use super::*;
use crate::{
    modules::{camera_gizmo::Axis, properties_combo, property_row, settings_group, settings_panel},
    parameter::Parameter,
};
impl Resonance {
    pub fn controls(&mut self, ui: &mut egui::Ui) {
        let s = &mut self.settings;
        macro_rules! p {
            ($field:ident, $range:expr, $default:expr, $name:expr, $help:expr, $ui:ident) => {
                $ui.add(
                    Parameter::new(&mut s.$field, $range, $default)
                        .key(stringify!($field))
                        .text($name),
                )
                .help_text($help);
            };
        }
        settings_panel(ui, "Geometry", |ui| {
            settings_group(ui, "Spectrum response", |ui| {
                property_row(ui, "Map spectrum", |ui| {
                    properties_combo("lotus-spectrum-layout", ui.available_width(), 3)
                        .selected_text(s.spectrum_layout.label()).show_ui(ui, |ui| {
                            for mode in [SpectrumLayout::Layers, SpectrumLayout::PoleToPole, SpectrumLayout::AroundPetals] {
                                ui.selectable_value(&mut s.spectrum_layout, mode, mode.label());
                            }
                        }).response.help_text("By layer assigns contiguous bass-to-treble bands from the first layer to the last, retaining each petal's silhouette. Layer flow carries each assigned band along with its shell. Pole to pole follows frequencies along the shell; Around petals mirrors the spectrum around each petal. These are artistic mappings, not a calibrated spectrum plot.");
                });
                p!(
                    spectrum_depth,
                    0.0..=1.0,
                    0.35,
                    "Spectrum movement",
                    "Direct frequency-driven expansion of the petals, independent of the slow ripple animation. Zero restores the original base geometry's motion; higher values reveal changes in individual frequency bands.",
                    ui
                );
                p!(
                    spectrum_visibility,
                    0.0..=1.0,
                    0.7,
                    "Spectrum visibility",
                    "A bounded display boost reveals quiet frequency differences. Zero keeps their relative size; one gives the strongest boost. Near-silence is not amplified, and this never alters the shared signal.",
                    ui
                );
                p!(
                    attack_accent,
                    0.0..=1.0,
                    0.15,
                    "Attack accent",
                    "Briefly expand the flower on rising audio levels, then return as the sound sustains. Fast/slow envelope comparison follows attacks rather than estimating beats; zero disables it.",
                    ui
                );
            });
            settings_group(ui, "Audio motion", |ui| {
                p!(
                    audio_expand,
                    0.0..=1.5,
                    0.22,
                    "Audio expansion",
                    "Overall input level expands the layered shape. Zero disables direct expansion; scene macros can also drive geometry.",
                    ui
                );
                p!(
                    audio_ripple,
                    0.0..=0.5,
                    0.12,
                    "Audio ripples",
                    "The selected frequency range bends the shell from pole to pole. Zero disables the animated ripple response; Spectrum movement separately follows the frequency levels directly.",
                    ui
                );
                p!(
                    response_ms,
                    0.0..=1000.0,
                    90.0,
                    "Motion smoothing ms",
                    "Visual interpolation time for spectrum and level changes. This does not filter the shared audio signal. Attack accents retain a fast onset; disconnected input fades while ambient motion continues.",
                    ui
                );
            });
            settings_group(ui, "Starting form", |ui| {
                property_row(ui, "Form", |ui| {
                    properties_combo("resonance-form",ui.available_width(),3).selected_text("Choose a starting form…").show_ui(ui,|ui| {
                        for (i,name) in ["Radiation petals","Solar waves","Bloom"].iter().enumerate() {
                            if ui.selectable_label(false,*name).clicked() {
                                s.layers=if i==1 {12} else {9}; s.petals=if i==2 {5} else {2};
                                s.radius=0.28; s.spacing=0.13; s.waist=0.18; s.sharpness=0.7;
                                s.flattening=if i==1 {0.65} else {0.24}; s.layer_depth=if i==2 {0.7} else {0.3};
                                s.twist=if i==1 {16.0} else if i==2 {8.0} else {4.0};
                                s.opening=if i==2 {0.55} else {1.0};
                                s.palette=if i==1 {Palette::Ember} else if i==2 {Palette::Aurora} else {ResonanceSettings::default().palette};
                                ui.close();
                            }
                        }
                    }).response.help_text("Choose layered radiation petals, warm twisting solar waves, or an open flower. This changes geometry and palette; capture, camera, motion, and macro assignments are retained.");
                });
            });
            settings_group(ui, "Layers", |ui| {
                p!(
                    layers,
                    1..=16,
                    9,
                    "Layers",
                    "Number of overlapping wavefront shells. Each layer has its own size, twist and palette position.",
                    ui
                );
                p!(
                    radius,
                    0.05..=2.0,
                    0.28,
                    "Inner radius",
                    "Size of the innermost shell.",
                    ui
                );
                p!(
                    spacing,
                    0.01..=0.4,
                    0.13,
                    "Layer spacing",
                    "Radial distance between neighboring shells.",
                    ui
                );
                p!(
                    layer_depth,
                    -3.0..=3.0,
                    0.3,
                    "Depth spread",
                    "Offset the layers along their central axis. Zero nests them around a common center.",
                    ui
                );
                p!(
                    twist,
                    -90.0..=90.0,
                    4.0,
                    "Layer twist °",
                    "Angular offset between neighboring layers.",
                    ui
                );
            });
            settings_group(ui, "Petal shape", |ui| {
                p!(
                    petals,
                    1..=8,
                    2,
                    "Petals",
                    "Number of lobes around each shell. Two creates the paired radiation shape in the reference.",
                    ui
                );
                p!(
                    waist,
                    0.02..=1.0,
                    0.18,
                    "Waist",
                    "Width of the valleys between petals. One produces a round shell.",
                    ui
                );
                p!(
                    sharpness,
                    0.2..=4.0,
                    0.7,
                    "Petal sharpness",
                    "Lower values create broad rounded lobes; higher values narrow the petals.",
                    ui
                );
                p!(
                    flattening,
                    0.03..=1.5,
                    0.24,
                    "Shell depth",
                    "Depth relative to the radial size. Low values make thin petal membranes; high values form volumes.",
                    ui
                );
                p!(
                    opening,
                    0.1..=1.0,
                    1.0,
                    "Shell coverage",
                    "One closes the shell from pole to pole. Reduce it to open the layers into bowls or flower petals.",
                    ui
                );
                p!(
                    detail,
                    12..=48,
                    28,
                    "Surface detail",
                    "Mesh resolution. Dense layer stacks automatically cap triangle count for predictable performance.",
                    ui
                );
            });
        });
        settings_panel(ui, "Flow & Motion", |ui| {
            settings_group(ui, "Wave motion", |ui| {
                p!(
                    flow,
                    -0.5..=0.5,
                    0.045,
                    "Layer flow Hz",
                    "Expanding or contracting shell cycle. Layers fade in at birth and fade out at the boundary. Zero keeps shells in place.",
                    ui
                );
                p!(
                    spin,
                    -90.0..=90.0,
                    5.0,
                    "Spin °/s",
                    "Rotate the petal forms around their central axis.",
                    ui
                );
                p!(
                    ripple_speed,
                    -3.0..=3.0,
                    0.18,
                    "Ripple speed Hz",
                    "Speed and direction of waves moving over each shell.",
                    ui
                );
                p!(
                    ripple_amount,
                    0.0..=0.5,
                    0.035,
                    "Ripple depth",
                    "Resting wave displacement, present even in silence.",
                    ui
                );
                p!(
                    ripple_count,
                    0.0..=12.0,
                    3.0,
                    "Ripple count",
                    "Number of wave undulations from pole to pole.",
                    ui
                );
                p!(
                    phase_spread,
                    0.0..=1.0,
                    0.16,
                    "Layer phase",
                    "Stagger ripples across shells for a traveling wavefront.",
                    ui
                );
            });
            ui.checkbox(&mut s.alternate, "Alternate spin")
                .help_text("Neighboring shells rotate in opposite directions.");
        });
        settings_panel(ui, "Frequency Range", |ui| {
            s.frequency.range_controls(ui);
        });
        settings_panel(ui, "Signal Response", |ui| {
            s.frequency.response_controls(ui);
            settings_group(ui, "Display range", |ui| {
                s.frequency.level_controls(ui);
            });
        });
        settings_panel(ui, "Appearance", |ui| {
            settings_group(ui, "Audio lighting", |ui| {
                p!(
                    audio_light,
                    0.0..=1.0,
                    0.35,
                    "Audio brightness",
                    "Overall input level brightens the membranes. Zero keeps the appearance independent of audio.",
                    ui
                );
                p!(
                    spectrum_light,
                    0.0..=2.0,
                    0.55,
                    "Spectrum light",
                    "Brighten each petal region with its mapped frequency band. Zero disables local spectral lighting; overall Audio brightness remains independent.",
                    ui
                );
            });

            settings_group(ui, "Surface", |ui| {
                property_row(ui, "Draw as", |ui| {
                    properties_combo("resonance-style",ui.available_width(),4).selected_text(match s.style {Style::Membranes=>"Membranes",Style::Wireframe=>"Wireframe",Style::Contours=>"Contours",Style::Points=>"Points"}).show_ui(ui,|ui| {
                    for (style,name) in [(Style::Membranes,"Membranes"),(Style::Wireframe,"Wireframe"),(Style::Contours,"Contours"),(Style::Points,"Points")] {ui.selectable_value(&mut s.style,style,name);}
                }).response.help_text("Translucent petal surfaces, a connected wire grid, isolated latitude contours, or a point cloud.");
                });
            });
            settings_group(ui, "Light & transparency", |ui| {
                p!(
                    opacity,
                    0.0..=1.0,
                    0.12,
                    "Opacity",
                    "Per-shell transparency. Lower values reveal nested layers.",
                    ui
                );
                p!(
                    rim,
                    0.0..=1.0,
                    0.5,
                    "Rim light",
                    "Strength of the luminous edges where the surface turns away from the camera.",
                    ui
                );
                p!(
                    brightness,
                    0.0..=3.0,
                    1.3,
                    "Brightness",
                    "Overall component brightness.",
                    ui
                );
                p!(
                    wire_opacity,
                    0.0..=1.0,
                    0.0,
                    "Grid overlay",
                    "Overlay latitude and meridian lines on membrane surfaces.",
                    ui
                );
                p!(
                    line_width,
                    0.25..=4.0,
                    0.7,
                    "Line width px",
                    "Width of wireframe, contour and cutaway rim lines.",
                    ui
                );
                p!(
                    grid_stride,
                    1..=12,
                    4,
                    "Grid spacing",
                    "Draw every Nth grid line; higher values produce fewer lines.",
                    ui
                );
                p!(
                    point_size,
                    0.5..=5.0,
                    1.4,
                    "Point size px",
                    "Dot radius in point-cloud mode.",
                    ui
                );
            });
            settings_group(ui, "Color", |ui| {
                s.palette.controls(ui);
                property_row(ui, "Color by", |ui| {
                    properties_combo("resonance-color",ui.available_width(),4).selected_text(match s.color_by {ColorBy::Layer=>"Layer",ColorBy::Petal=>"Petal",ColorBy::Energy=>"Audio energy",ColorBy::Position=>"Pole to pole"}).show_ui(ui,|ui| {
                    for (mode,name) in [(ColorBy::Layer,"Layer"),(ColorBy::Petal,"Petal"),(ColorBy::Energy,"Audio energy"),(ColorBy::Position,"Pole to pole")] {ui.selectable_value(&mut s.color_by,mode,name);}
                }).response.help_text("Spread the palette across shells, around each petal, over audio levels, or from pole to pole.");
                });
                p!(
                    color_shift,
                    -1.0..=1.0,
                    0.0,
                    "Color offset",
                    "Move the palette sampling position; endpoints clamp to the first or last color.",
                    ui
                );
                p!(
                    contrast,
                    0.25..=4.0,
                    1.0,
                    "Color contrast",
                    "Bias the distribution of the selected gradient.",
                    ui
                );
                ui.checkbox(&mut s.reverse, "Reverse colors")
                    .help_text("Reverse the palette direction.");
                if !crate::modules::flat_inspector(ui) {
                    ui.checkbox(&mut s.black_canvas, "Black canvas")
                        .help_text("Use a black canvas independently of the GUI theme.");
                }
            });
            settings_group(ui, "Guides", |ui| {
                ui.checkbox(&mut s.show_gizmo, "Navigation gizmo");
            });
        });
        settings_panel(ui, "Camera", |ui| {
            settings_group(ui, "View presets", |ui| {
                ui.horizontal_wrapped(|ui| {
                    for (name, axis, positive) in [
                        ("Front", Axis::Y, false),
                        ("Right", Axis::X, true),
                        ("Top", Axis::Z, true),
                        ("Back", Axis::Y, true),
                    ] {
                        if ui.button(name).clicked() {
                            (s.yaw, s.elevation) = axis.angles(positive);
                            s.roll = 0.0;
                        }
                    }
                });
            });
            settings_group(ui, "Transform", |ui| {
                for (key, name, angle, default) in [
                    ("yaw", "Rotation °", &mut s.yaw, -0.45_f32),
                    ("elevation", "Elevation °", &mut s.elevation, 0.3),
                    ("roll", "Roll °", &mut s.roll, 0.35),
                ] {
                    let mut degrees = angle.to_degrees();
                    if ui
                        .add(
                            Parameter::new(&mut degrees, -180.0..=180.0, default.to_degrees())
                                .key(key)
                                .raw_scale(std::f64::consts::PI / 180.0)
                                .text(name),
                        )
                        .changed()
                    {
                        *angle = degrees.to_radians();
                    }
                }
                p!(
                    zoom,
                    0.25..=8.0,
                    1.0,
                    "Zoom",
                    "Scroll over the component to zoom. Drag rotates freely and right/Shift-drag pans.",
                    ui
                );
                for (key, name, value) in [
                    ("pan.x", "Pan X", &mut s.pan.x),
                    ("pan.y", "Pan Y", &mut s.pan.y),
                ] {
                    ui.add(
                        Parameter::new(value, -2.0..=2.0, 0.0)
                            .bounds(-4.0..=4.0)
                            .key(key)
                            .text(name),
                    );
                }
                if ui.button("Reset camera").clicked() {
                    s.auto_orbit = false;
                    s.yaw = -0.45;
                    s.elevation = 0.3;
                    s.roll = 0.35;
                    s.zoom = 1.0;
                    s.pan = Vec2::ZERO;
                }
            });
        });
    }
}
