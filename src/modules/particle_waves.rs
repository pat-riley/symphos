//! Measured spectrum history arranged as layers of freely drifting GPU particles.
mod gpu;
mod settings;
#[cfg(test)]
mod tests;
use super::{
    frequency::{BANDS, History},
    properties_combo, property_row, settings_group, settings_panel, trackball,
};
use crate::{analysis::AnalysisFrame, help::HoverHelp, theme::AppTheme};
use egui::{Rect, Sense};
pub(crate) use settings::Settings;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use symphos_visuals::particle_waves::{Frame, Layer, ROWS};

pub struct ParticleWaves {
    settings: Settings,
    sculptural: bool,
    history: History,
    gpu: gpu::View,
    field: Arc<Vec<f32>>,
    last: Option<Instant>,
    time: f32,
    envelope: f32,
    selected_layer: usize,
}
impl Default for ParticleWaves {
    fn default() -> Self {
        Self {
            settings: Settings::default(),
            sculptural: false,
            history: History::default(),
            gpu: gpu::View::default(),
            field: Arc::new(vec![0.0; BANDS * ROWS]),
            last: None,
            time: 0.0,
            envelope: 0.0,
            selected_layer: 0,
        }
    }
}
fn choice(ui: &mut egui::Ui, label: &str, value: &mut u32, options: &[&str], help: &str) {
    property_row(ui, label, |ui| {
        properties_combo(label, ui.available_width(), options.len())
            .selected_text(options[*value as usize])
            .show_ui(ui, |ui| {
                for (i, name) in options.iter().enumerate() {
                    ui.selectable_value(value, i as u32, *name);
                }
            })
            .response
            .help_text(help);
    });
}
fn colors(ui: &mut egui::Ui, a: &mut [u8; 3], b: &mut [u8; 3], c: &mut [u8; 3]) {
    for (label, color) in [
        ("First color", a),
        ("Middle color", b),
        ("Highlight color", c),
    ] {
        property_row(ui, label, |ui| {
            ui.color_edit_button_srgb(color).help_text("Saved palette color. Color driver determines where particles sample this three-color gradient.");
        });
    }
}
impl ParticleWaves {
    pub fn spectral_bloom() -> Self {
        Self {
            settings: Settings::spectral_bloom(),
            sculptural: true,
            ..Self::default()
        }
    }
    pub fn settings_snapshot(&self) -> Settings {
        self.settings.clone()
    }
    pub fn apply_settings(&mut self, s: &Settings) {
        self.settings = s.clone();
        self.settings.sanitize();
    }
    pub fn reset_settings(&mut self) {
        self.gpu = Default::default();
        self.settings = if self.sculptural {
            Settings::spectral_bloom()
        } else {
            Settings::default()
        };
    }
    pub fn clear(&mut self) {
        self.history.clear();
        self.gpu = Default::default();
        self.field = Arc::new(vec![0.0; BANDS * ROWS]);
        self.last = None;
        self.time = 0.0;
        self.envelope = 0.0;
    }
    pub fn release_if_inactive(&mut self) {
        if self.last.is_some() {
            self.clear();
        }
    }
    pub fn ingest(&mut self, frame: &crate::capture_history::SpectralFrame, now: Instant) {
        self.history.data.settings = self.settings.frequency.clone();
        self.history.ingest(frame, now);
    }
    pub fn controls(&mut self, ui: &mut egui::Ui) {
        self.settings.sanitize();
        let s = &mut self.settings;
        settings_panel(ui, "Geometry", |ui| {
            settings_group(ui, "Arrangement", |ui| {
                if !self.sculptural {
                    choice(
                        ui,
                        "Mode",
                        &mut s.mode,
                        &["Ribbons", "Rings", "Spiral", "Tunnel"],
                        "All arrangements show the measured spectrum. Along each ribbon or circumference is frequency; depth or radial extent is recent audio history. Geometry changes preserve the input history.",
                    );
                }
                ui.add(crate::parameter::Parameter::new(&mut s.layer_count,1..=8,4).key("layer_count").text("Layers")).help_text("One to eight independently configurable layers sharing the incoming spectrum. The particle budget is divided across active layers.");
                s.numeric_controls(
                    ui,
                    if self.sculptural {
                        "Bloom layout"
                    } else {
                        "Layout"
                    },
                    "",
                );
                if self.sculptural {
                    s.bloom.numeric_controls(ui, "Sculpture", "bloom.");
                }
            });
            settings_group(ui, "Individual layers", |ui| {
                self.selected_layer = self.selected_layer.min(s.layer_count as usize - 1);
                property_row(ui, "Edit layer", |ui| {
                    properties_combo("particle-wave-layer",ui.available_width(),s.layer_count as usize).selected_text(format!("Layer {}",self.selected_layer+1)).show_ui(ui,|ui| {for i in 0..s.layer_count as usize{ui.selectable_value(&mut self.selected_layer,i,format!("Layer {}",i+1));}}).response.help_text("Choose the layer whose transform and appearance you want to edit. Selecting a layer does not change its settings.");
                });
                let layer = &mut s.layers[self.selected_layer];
                let prefix = format!("layers.{}.", self.selected_layer);
                ui.checkbox(&mut layer.enabled, "Visible")
                    .help_text("Hide this layer while keeping its transform and style for later.");
                layer.numeric_controls(ui, "Transform", &prefix);
                ui.checkbox(&mut layer.custom,"Override shared appearance and motion").help_text("Use this layer's palette, size, density, opacity and motion multipliers. Uncheck to use the shared settings; your overrides are retained.");
                if layer.custom {
                    layer.numeric_controls(ui, "Override", &prefix);
                    colors(
                        ui,
                        &mut layer.color_a,
                        &mut layer.color_b,
                        &mut layer.color_c,
                    );
                }
                if ui.button("Reset layer").help_text("Restore only this layer's transform and overrides. Shared geometry and other layers are unaffected.").clicked(){*layer=Default::default();}
            });
        });
        settings_panel(ui, "Particles", |ui| {
            settings_group(ui, "Particle rendering", |ui| {
                choice(
                    ui,
                    "Particle budget",
                    &mut s.quality,
                    &["8,192", "32,768", "65,536", "131,072"],
                    "Total GPU particle budget shared across all layers. More particles resolve finer dots but increase GPU work. No automatic quality reduction occurs at high frame-rate targets.",
                );
                choice(
                    ui,
                    "Shape",
                    &mut s.shape,
                    &["Disc", "Square", "Diamond", "Ring"],
                    "Sprite shape before optical defocus. Disc gives soft round lights like the reference. Squares and diamonds reveal the individual samples.",
                );
                s.numeric_controls(ui, "Particle size", "");
            });
        });
        settings_panel(ui, "Signal Response", |ui| {
            settings_group(ui, "Frequency range", |ui| {
                ui.checkbox(&mut s.frequency.logarithmic,"Logarithmic frequency").help_text("Give bass and treble comparable space. Uncheck for uniform spacing in Hz. Retained history is reprocessed from its measured FFT magnitudes.");
                ui.add(crate::parameter::Parameter::new(&mut s.frequency.min_hz,10.0..=2000.0,20.0).key("frequency.min_hz").logarithmic(true).text("Low Hz")).help_text("Lowest frequency represented by the ribbon. Frequencies outside this range do not drive particle scattering.");
                ui.add(crate::parameter::Parameter::new(&mut s.frequency.max_hz,2000.0..=24000.0,20000.0).key("frequency.max_hz").logarithmic(true).text("High Hz")).help_text("Highest visible frequency, bounded by the source Nyquist limit. Does not change captured audio.");
            });
            s.frequency.response_controls(ui);
            settings_group(ui, "Visible level range", |ui| {
                s.frequency.level_controls(ui);
            });
            settings_group(ui, "Motion and light response", |ui| {
                s.numeric_controls(ui, "Audio dynamics", "");
            });
        });
        settings_panel(ui, "Flow & Motion", |ui| {
            settings_group(ui, "Drift and return", |ui| {
                s.numeric_controls(
                    ui,
                    if self.sculptural {
                        "Bloom drift"
                    } else {
                        "Drift and return"
                    },
                    "",
                );
            });
            if !self.sculptural {
                settings_group(ui, "Audio-driven resonance", |ui| {
                    s.numeric_controls(ui, "Resonant motion", "");
                });
            }
        });
        if self.sculptural {
            settings_panel(ui, "Feedback", |ui| {
                ui.checkbox(&mut s.bloom.feedback_enabled, "Image feedback").help_text("Accumulate and transform previous rendered light. Disable for clean spectrum strands. Feedback is cleared when the viewport is resized or this component is reset.");
                s.bloom.numeric_controls(ui, "Feedback", "bloom.");
            });
        }
        settings_panel(ui, "Appearance", |ui| {
            settings_group(ui, "Palette", |ui| {
                choice(
                    ui,
                    "Color driver",
                    &mut s.color_mode,
                    &["Particle variation", "Frequency", "Spectral level", "Layer"],
                    "Choose what samples the palette. Particle variation keeps a stable mix of blue, green and white dots; Frequency orders colors along the spectrum; Level follows incoming intensity; Layer separates stacked sheets.",
                );
                colors(ui, &mut s.color_a, &mut s.color_b, &mut s.color_c);
                s.numeric_controls(ui, "Palette", "");
            });
            settings_group(ui, "Light", |ui| {
                s.numeric_controls(ui, "Light", "");
            });
            settings_group(ui, "Depth of field", |ui| {
                ui.checkbox(&mut s.depth_of_field,"Depth of field").help_text("Keep particles near the focus distance crisp and spread nearer or farther particles into soft bokeh discs. Disable for equal sharpness throughout the scene.");
                s.numeric_controls(ui, "Focus", "");
            });
        });
        settings_panel(ui, "Camera", |ui| {
            ui.checkbox(&mut s.auto_orbit,"Auto-rotate").help_text("Orbit continuously around the layers at the chosen speed. Manual rotation stops auto-rotate; panning and zoom retain it.");
            ui.checkbox(&mut s.orthographic,"Orthographic").help_text("Remove perspective size changes while retaining 3D rotation. Depth of field can still show the focus plane.");
            ui.checkbox(&mut s.guides,"XYZ reference grid").help_text("Show the world-space ground grid and colored X/Y/Z axes. It follows the same camera as the particles and is hidden by default.");
            ui.horizontal(|ui| {
                if ui.button("Front").help_text("Look directly across X and Y, centered on the layers. Stops auto-rotate.").clicked(){s.yaw=0.0;s.elevation=0.0;s.roll=0.0;s.auto_orbit=false;}
                if ui.button("Top").help_text("Look down at the XZ plane. Stops auto-rotate.").clicked(){s.yaw=0.0;s.elevation = -std::f32::consts::FRAC_PI_2;s.roll=0.0;s.auto_orbit=false;}
                if ui.button("Reset camera").help_text("Restore the angled view and framing without changing the spectrum, focus settings or particles.").clicked(){reset_camera(s, self.sculptural);}
            });
            s.numeric_controls(ui, "Camera", "");
        });
    }
    fn advance(&mut self, audio: &AnalysisFrame, now: Instant, live: bool) -> f32 {
        let dt = self.last.map_or(0.0, |last| {
            now.saturating_duration_since(last).as_secs_f32().min(0.1)
        });
        self.last = Some(now);
        self.time += dt;
        self.history.data.settings = self.settings.frequency.clone();
        self.history.update(audio, now, live);
        let f = &self.settings.frequency;
        let target = if live {
            audio
                .bins
                .iter()
                .filter(|b| b.frequency_hz >= f.min_hz && b.frequency_hz <= f.max_hz)
                .map(|b| f.intensity(20.0 * b.magnitude.max(1e-7).log10() + f.gain))
                .fold(0.0_f32, f32::max)
        } else {
            0.0
        };
        let tau = if target > self.envelope {
            self.settings.attack_ms
        } else {
            self.settings.release_ms
        } * 0.001;
        self.envelope += (target - self.envelope) * (1.0 - (-dt / tau).exp());
        let field = Arc::make_mut(&mut self.field);
        for row in 0..ROWS {
            let age = row as f32 / (ROWS - 1) as f32 * self.settings.history_seconds;
            let target_time = now.checked_sub(Duration::from_secs_f32(age)).unwrap_or(now);
            let index = self.history.rows.partition_point(|r| r.time <= target_time);
            let a = self.history.rows.get(index.saturating_sub(1));
            let b = self.history.rows.get(index);
            let dest = &mut field[row * BANDS..(row + 1) * BANDS];
            dest.fill(0.0);
            if let Some(a) = a {
                let distance = if a.time > target_time {
                    a.time.duration_since(target_time)
                } else {
                    target_time.duration_since(a.time)
                }
                .as_secs_f32();
                if distance < 0.12 {
                    let mix = b
                        .map_or(0.0, |b| {
                            target_time.saturating_duration_since(a.time).as_secs_f32()
                                / b.time.duration_since(a.time).as_secs_f32().max(0.001)
                        })
                        .clamp(0.0, 1.0);
                    for (i, out) in dest.iter_mut().enumerate() {
                        let av = f.intensity(a.levels[i]);
                        *out = av + (b.map_or(av, |b| f.intensity(b.levels[i])) - av) * mix;
                    }
                }
            }
        }
        dt
    }
    pub fn draw(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        audio: &AnalysisFrame,
        theme: &AppTheme,
        now: Instant,
        live: bool,
    ) {
        self.settings.sanitize();
        let dt = self.advance(audio, now, live);
        let s = &mut self.settings;
        if s.auto_orbit {
            s.yaw = (s.yaw + dt * s.orbit_speed + std::f32::consts::PI)
                .rem_euclid(std::f32::consts::TAU)
                - std::f32::consts::PI;
        }
        let response=ui.interact(rect,ui.id().with("particle-waves-camera"),Sense::click_and_drag()).help_text("Spectral particles · frequency shapes luminous 3D layers. Drag to rotate freely, Shift/right-drag to pan, scroll to zoom, double-click to reset. Stronger audio scatters grains; quieter passages draw them back to their spectral surface.");
        if response.dragged() {
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
                s.auto_orbit = false;
            }
        }
        if response.hovered() {
            s.zoom =
                (s.zoom * (ui.input(|i| i.smooth_scroll_delta.y) * 0.002).exp()).clamp(0.25, 4.0);
        }
        if response.double_clicked() {
            reset_camera(s, self.sculptural);
        }
        let axes = trackball::orientation_axes(s.yaw, s.elevation, s.roll)
            .map(|v| [v[0] as f32, v[1] as f32, v[2] as f32, 0.0]);
        let color = |c: [u8; 3]| {
            [
                c[0] as f32 / 255.0,
                c[1] as f32 / 255.0,
                c[2] as f32 / 255.0,
                1.0,
            ]
        };
        let frame = Frame {
            clock: [dt, self.time, 0.0, s.drift_fraction],
            geometry: [s.width, s.depth, s.height, s.layer_count as f32],
            layout: if self.sculptural {
                [
                    s.spacing,
                    s.bloom.cloud_size,
                    s.bloom.cloud_light,
                    s.bloom.idle_light,
                ]
            } else {
                [s.spacing, s.radius, s.turns, s.mode as f32]
            },
            motion: [s.drift, s.return_strength, s.drag, s.turbulence],
            response: [self.envelope, s.threshold, s.audio_scatter, s.motion_speed],
            ripple: [
                s.ripple,
                s.ripple_scale
                    * (0.8 + (audio.dominant_frequency_hz.max(20.0) / 1000.0).min(2.0) * 0.2),
                s.ripple_speed,
                s.twist,
            ],
            look: [s.size, s.size_variation, s.opacity, s.density],
            optics: [
                s.focus_distance,
                if s.depth_of_field { s.aperture } else { 0.0 },
                s.max_blur,
                s.shape as f32,
            ],
            color: [
                s.color_mode as f32,
                s.color_spread,
                s.color_offset,
                s.saturation,
            ],
            light: [s.glow, s.brightness, s.audio_size, s.audio_light],
            view: [rect.aspect_ratio(), s.zoom, s.pan_x, s.pan_y],
            camera: [
                s.camera_distance,
                s.field_of_view.to_radians(),
                f32::from(s.orthographic),
                s.softness,
            ],
            axis_x: axes[0],
            axis_y: axes[1],
            axis_z: axes[2],
            output: [0.0; 4],
            sculpture: if self.sculptural {
                [1.0, s.bloom.columns, s.bloom.bend, s.bloom.curl]
            } else {
                [0.0; 4]
            },
            fragments: [
                s.bloom.cloud,
                s.bloom.roughness,
                s.bloom.strand_width,
                s.bloom.fall,
            ],
            feedback: if self.sculptural && s.bloom.feedback_enabled {
                [
                    s.bloom.persistence,
                    s.bloom.expansion,
                    s.bloom.rotation,
                    s.bloom.warp,
                ]
            } else {
                [0.0; 4]
            },
            feedback_shift: [s.bloom.shift_x, s.bloom.shift_y, s.bloom.chroma, 0.0],
            layers: std::array::from_fn(|i| {
                let l = &s.layers[i];
                Layer {
                    transform: [l.x, l.y, l.z, l.scale],
                    rotation: [
                        l.rotation.to_radians(),
                        l.tilt.to_radians(),
                        s.color_blend,
                        f32::from(l.enabled),
                    ],
                    style: if l.custom {
                        [l.size, l.density, l.speed, l.opacity]
                    } else {
                        [1.0; 4]
                    },
                    a: color(if l.custom { l.color_a } else { s.color_a }),
                    b: color(if l.custom { l.color_b } else { s.color_b }),
                    c: color(if l.custom { l.color_c } else { s.color_c }),
                }
            }),
        };
        self.gpu.show(
            ui,
            rect,
            frame,
            self.field.clone(),
            [8192, 32768, 65536, 131072][s.quality as usize],
        );
        if s.guides {
            draw_guides(ui, rect, s, axes, theme);
        }
    }
}
fn reset_camera(s: &mut Settings, sculptural: bool) {
    let d = if sculptural {
        Settings::spectral_bloom()
    } else {
        Settings::default()
    };
    s.yaw = d.yaw;
    s.elevation = d.elevation;
    s.roll = d.roll;
    s.zoom = d.zoom;
    s.pan_x = 0.0;
    s.pan_y = 0.0;
    s.camera_distance = d.camera_distance;
    s.field_of_view = d.field_of_view;
    s.auto_orbit = false;
}
fn draw_guides(ui: &egui::Ui, rect: Rect, s: &Settings, axes: [[f32; 4]; 3], theme: &AppTheme) {
    let project = |point: [f32; 3]| -> Option<egui::Pos2> {
        let p: [f32; 3] = std::array::from_fn(|r| (0..3).map(|c| axes[c][r] * point[c]).sum());
        let depth = s.camera_distance + p[2];
        if depth < 0.25 {
            return None;
        }
        let scale = 1.0
            / (s.field_of_view.to_radians() * 0.5).tan()
            / if s.orthographic {
                s.camera_distance
            } else {
                depth
            }
            * s.zoom
            * rect.aspect_ratio().min(1.0);
        Some(
            rect.center()
                + egui::vec2(
                    p[0] * scale / rect.aspect_ratio() + s.pan_x,
                    p[1] * scale - s.pan_y,
                ) * rect.size()
                    * 0.5,
        )
    };
    let painter = ui.painter_at(rect);
    let line = |a, b, color, width| {
        if let (Some(a), Some(b)) = (project(a), project(b)) {
            painter.line_segment([a, b], egui::Stroke::new(width, color));
        }
    };
    for i in -5..=5 {
        line(
            [i as f32, 0.0, -5.0],
            [i as f32, 0.0, 5.0],
            theme.foreground.gamma_multiply(0.12),
            0.7,
        );
        line(
            [-5.0, 0.0, i as f32],
            [5.0, 0.0, i as f32],
            theme.foreground.gamma_multiply(0.12),
            0.7,
        );
    }
    for (i, label, c) in [
        (0, "X", egui::Color32::from_rgb(235, 100, 110)),
        (1, "Y", egui::Color32::from_rgb(110, 210, 130)),
        (2, "Z", egui::Color32::from_rgb(100, 165, 245)),
    ] {
        let mut p = [0.0; 3];
        p[i] = 3.0;
        line([0.0; 3], p, c, 1.2);
        if let Some(p) = project(p) {
            painter.text(
                p,
                egui::Align2::LEFT_BOTTOM,
                label,
                egui::FontId::proportional(12.0),
                c,
            );
        }
    }
}
