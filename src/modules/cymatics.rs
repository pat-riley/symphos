//! Audio-driven Chladni plates with a reduced physical model and artistic views.
mod gpu;
mod response;
mod settings;
use super::{properties_combo, property_row, settings_group, settings_panel, trackball};
use crate::{analysis::AnalysisFrame, help::HoverHelp, theme::AppTheme};
use egui::{Rect, Sense};
use settings::CymaticsSettings;
pub(crate) use settings::CymaticsSettings as Settings;
use std::{
    sync::{Arc, mpsc},
    time::Instant,
};
use symphos_visuals::{
    cymatics::{GRID, Plate, PlateSpec},
    cymatics_gpu::CymaticsFrame,
};

pub struct Cymatics {
    settings: CymaticsSettings,
    plate: Option<Plate>,
    pending: Option<mpsc::Receiver<Plate>>,
    gpu: gpu::View,
    last: Option<Instant>,
    time: f32,
    field: Arc<Vec<[f32; 4]>>,
    reset: bool,
    seed: u32,
    drive: DriveStatus,
}
#[derive(Clone, Copy)]
struct DriveStatus {
    input_db: f32,
    peak_hz: f32,
    peaks: usize,
    vibration: f32,
}
impl Default for DriveStatus {
    fn default() -> Self {
        Self {
            input_db: -120.0,
            peak_hz: 0.0,
            peaks: 0,
            vibration: 0.0,
        }
    }
}
impl Default for Cymatics {
    fn default() -> Self {
        Self {
            settings: Default::default(),
            plate: None,
            pending: None,
            gpu: Default::default(),
            last: None,
            time: 0.0,
            field: Arc::new(vec![[0.0; 4]; GRID * GRID]),
            reset: true,
            seed: 1,
            drive: DriveStatus::default(),
        }
    }
}
fn choice(ui: &mut egui::Ui, name: &str, value: &mut u32, options: &[&str], help: &str) {
    property_row(ui, name, |ui| {
        properties_combo(name, ui.available_width(), options.len())
            .selected_text(options[*value as usize])
            .show_ui(ui, |ui| {
                for (i, label) in options.iter().enumerate() {
                    ui.selectable_value(value, i as u32, *label);
                }
            })
            .response
            .help_text(help);
    });
}
impl Cymatics {
    pub fn settings_snapshot(&self) -> CymaticsSettings {
        self.settings.clone()
    }
    pub fn apply_settings(&mut self, s: &CymaticsSettings) {
        self.settings = s.clone();
        self.settings.sanitize();
    }
    pub fn clear(&mut self) {
        self.gpu = Default::default();
        self.last = None;
        self.time = 0.0;
        self.field = Arc::new(vec![[0.0; 4]; GRID * GRID]);
        self.reset = true;
        self.drive = DriveStatus::default();
    }
    pub fn release_if_inactive(&mut self) {
        if self.last.is_some() {
            self.clear();
        }
    }
    pub fn reset_settings(&mut self) {
        *self = Self::default();
    }
    fn spec(&self) -> PlateSpec {
        PlateSpec {
            circle: self.settings.shape == 2,
            aspect: if self.settings.shape == 1 {
                f64::from(self.settings.aspect)
            } else {
                1.0
            },
            edge: self.settings.edge,
            center: self.settings.center_support,
            poisson: f64::from(self.settings.poisson),
        }
    }
    fn update_plate(&mut self) {
        let desired = self.spec();
        if let Some(receiver) = &self.pending {
            match receiver.try_recv() {
                Ok(plate) => {
                    self.pending = None;
                    // Preserve settled grains while physics controls are mapped or
                    // edited. Only a different boundary outline needs a new population.
                    if self
                        .plate
                        .as_ref()
                        .is_none_or(|old| old.spec.circle != plate.spec.circle)
                    {
                        self.field = Arc::new(vec![[0.0; 4]; GRID * GRID]);
                        self.reset = true;
                    }
                    self.plate = Some(plate);
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.pending = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if self.pending.is_none() && self.plate.as_ref().is_none_or(|p| p.spec != desired) {
            let (tx, rx) = mpsc::sync_channel(1);
            self.pending = Some(rx);
            std::thread::spawn(move || {
                let _ = tx.send(Plate::solve(desired));
            });
        }
    }
    pub fn controls(&mut self, ui: &mut egui::Ui) {
        self.settings.sanitize();
        let s = &mut self.settings;
        settings_panel(ui, "Geometry", |ui| {
            settings_group(ui, "Plate and mode", |ui| {
                choice(
                    ui,
                    "Mode",
                    &mut s.mode,
                    &["Sand", "Wave surface", "Nodal patterns", "Interference"],
                    "All modes react only to incoming audio. Sand uses approximate grain transport over a physical plate response; Wave surface and Nodal patterns visualize that response. Interference is an artistic traveling-wave view.",
                );
                choice(
                    ui,
                    "Plate shape",
                    &mut s.shape,
                    &["Square", "Rectangle", "Circle"],
                    "Choose the plate boundary. Square and rectangle with simply supported edges use exact analytical modes. Other configurations use a reduced thin-plate numerical model.",
                );
                choice(
                    ui,
                    "Edge constraint",
                    &mut s.edge,
                    &["Simply supported", "Clamped", "Free"],
                    "Simply supported fixes edge displacement while allowing rotation. Clamped fixes displacement and slope. Free edges have no applied bending restraint. This changes the resonances, not just the outline.",
                );
                ui.checkbox(&mut s.center_support,"Center support").help_text("Enforce zero displacement at one idealized center point, without drawing hardware. This is a point support, not a finite clamp. Free-edge rigid-body modes are omitted.");
                choice(
                    ui,
                    "Material",
                    &mut s.material,
                    &["Aluminum", "Steel", "Brass"],
                    "Sets Young’s modulus and density to representative values: aluminum 69 GPa / 2700 kg/m³; steel 200 / 7850; brass 110 / 8500. Adjust Poisson ratio separately. Real alloys and mounts differ.",
                );
            });
            settings_group(ui, "Dimensions and excitation", |ui| {
                s.numeric_controls(ui, "Geometry")
            });
            ui.label("Thin-plate modes · approximate sand transport").help_text("Uses linear Kirchhoff–Love plate theory. Exact simply supported rectangular modes; polynomial Rayleigh–Ritz approximation elsewhere. Only the lowest 24 bending modes are retained. Audio is reduced to six dominant spectral peaks. No grain collisions, air flow, gravity tilt, or physical transducer calibration.");
        });
        settings_panel(ui, "Particles", |ui| {
            choice(
                ui,
                "Particle count",
                &mut s.quality,
                &["8,192", "32,768", "65,536", "131,072"],
                "GPU grain count. Higher counts resolve fine lines but cost more GPU time. The global display FPS remains unchanged. Changing count scatters a fresh population.",
            );
            ui.checkbox(&mut s.soft_grains, "Soft luminous grains")
                .help_text(
                    "Use soft glowing sprites instead of crisp sand grains. Appearance only.",
                );
            ui.checkbox(&mut s.recycle_edges, "Recycle grains at edges").help_text("Return grains that reach the outer edge to a scattered area near the center, with no leftover velocity. Enabled by default for continuous audio-driven flow. Turn off for natural settling at the boundary. Recycling stops when paused or the vibration has faded.");
            settings_group(ui, "Grain appearance", |ui| {
                s.numeric_controls(ui, "Particles")
            });
            settings_group(ui, "Motion", |ui| s.numeric_controls(ui, "Motion"));
            settings_group(ui, "Lifecycle", |ui| s.numeric_controls(ui, "Lifecycle"));
            if ui.button("Scatter grains").help_text("Redistribute grains across the current plate with zero velocity. Keeps plate settings, audio response, camera, and mappings.").clicked(){self.reset=true;self.seed=self.seed.wrapping_add(1);}
        });
        settings_panel(ui, "Signal Response", |ui| {
            ui.checkbox(&mut s.auto_sensitivity,"Automatic visual sensitivity").help_text("Normalize the displayed vibration pattern to the incoming audio level. Makes normal music and quieter mid/high frequencies visibly move grains while preserving nodes and resonance shapes. Changes absolute visual amplitudes; switch off to inspect the original relative physical response. Silence stays still.");
            s.numeric_controls(ui, "Signal Response");
            settings_group(ui, "Live excitation", |ui| {
                property_row(ui, "Input level", |ui| {
                    ui.label(format!("{:.1} dBFS",self.drive.input_db)).help_text("Strongest current FFT bin before the Cymatics frequency filter and noise gate. If this stays near -120 dBFS, check the audio input in Settings.");
                });
                property_row(ui, "Accepted peaks", |ui| {
                    ui.label(format!("{}",self.drive.peaks)).help_text("Number of incoming spectral peaks currently driving the plate, after the frequency range and noise gate. Zero means the filter or gate is excluding the current audio, or the input is silent.");
                });
                property_row(ui, "Strongest drive", |ui| {
                    ui.label(if self.drive.peaks>0{format!("{:.1} Hz",self.drive.peak_hz)}else{"—".into()}).help_text("Frequency of the strongest accepted spectral peak. It is measured from incoming audio; no tone is generated.");
                });
                property_row(ui, "Vibration level", |ui| {
                    ui.label(format!("{:.3}",self.drive.vibration)).help_text("RMS display vibration after excitation gain, automatic sensitivity and attack/release. This is a relative visual level, not meters. Zero means the grains currently receive no vibration.");
                });
                if self.drive.input_db > s.gate_db && self.drive.peaks == 0 {
                    ui.colored_label(
                        ui.visuals().warn_fg_color,
                        "Audio is present, but the frequency filter excludes it.",
                    );
                }
            });
            if let Some(plate) = &self.plate {
                let hz = response::frequencies(plate, s);
                ui.label(format!("Modeled resonances: {:.1}–{:.1} Hz",hz[0],hz[hz.len()-1])).help_text("Range of the 24 modeled bending resonances. Incoming frequencies outside this range can drive off-resonant response but cannot excite unmodeled higher modes. Narrow-band audio makes the clearest Chladni figures.");
                if s.low_hz > hz[hz.len() - 1] || s.high_hz < hz[0] {
                    ui.colored_label(ui.visuals().warn_fg_color,"The input range misses the modeled resonances. Lower the cutoff or adjust plate dimensions for clearer patterns.");
                }
            }
            ui.label("Input: main audio · no generated sound").help_text("Uses the existing captured input spectrum. Signals may modulate inspector parameters through scene macros; Cymatics does not create source channels or produce audio.");
        });
        settings_panel(ui, "Appearance", |ui| {
            settings_group(ui, "Palette", |ui| {
                property_row(ui, "Starting palette", |ui| {
                    properties_combo("cymatics-palette",ui.available_width(),5).selected_text("Choose…").show_ui(ui,|ui|{
                for (name,a,b) in [("Natural sand",[238,219,171],[255,252,239]),("Salt",[180,190,200],[255,255,255]),("Thermal",[35,35,160],[255,190,30]),("Ocean",[0,70,170],[40,255,210]),("Neon",[80,20,255],[255,30,150])]{if ui.selectable_label(false,name).clicked(){s.color_a=a;s.color_b=b;ui.close();}}
            }).response.help_text("Set gradient endpoints without changing physical settings, camera or macro bindings.");
                });
                choice(
                    ui,
                    "Color driver",
                    &mut s.color_mode,
                    &["Grain variation / energy", "Vibration / signed wave"],
                    "For sand, choose stable per-grain variation or local vibration. For surfaces, choose vibration energy or signed displacement.",
                );
                for (name, color) in [
                    ("First color", &mut s.color_a),
                    ("Second color", &mut s.color_b),
                    ("Plate color", &mut s.plate_color),
                ] {
                    property_row(ui, name, |ui| {
                        ui.color_edit_button_srgb(color)
                            .help_text("Edit the saved color. Changes rendering only.");
                    });
                }
            });
            settings_group(ui, "Surface, grain and light", |ui| {
                s.numeric_controls(ui, "Appearance")
            });
        });
        settings_panel(ui, "Flow & Motion", |ui| {
            ui.checkbox(&mut s.artistic,"Artistic motion").help_text("Add turbulence to sand migration. Disabled by default to preserve the physical nodal pattern. Shader displacement, colors and lighting are always visual styling.");
            ui.add(crate::parameter::Parameter::new(&mut s.emitters,1..=6,3).key("emitters").text("Interference emitters")).help_text("Number of virtual emitters in Interference mode. All use incoming dominant pitch and amplitude, with configurable spatial and phase offsets.");
            s.numeric_controls(ui, "Flow & Motion");
        });
        settings_panel(ui, "Camera", |ui| {
            ui.checkbox(&mut s.orthographic, "Orthographic").help_text(
                "Remove perspective size changes. Useful with Top view to inspect nodal patterns.",
            );
            ui.checkbox(&mut s.guides,"XYZ axes").help_text("Show orientation axes through the origin. These are visual guides; rotating the camera does not tilt the physical plate under gravity.");
            ui.horizontal(|ui|{
            if ui.button("Top view").help_text("Look straight down at the flat plane. Reset rotation, roll and framing while retaining zoom.").clicked(){s.yaw=0.0;s.elevation = -std::f32::consts::FRAC_PI_2;s.roll=0.0;s.pan_x=0.0;s.pan_y=0.0;}
            if ui.button("Reset camera").help_text("Restore the initial angled camera and centered framing. Does not scatter grains.").clicked(){reset_camera(s);}
        });
            s.numeric_controls(ui, "Camera");
        });
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
        self.update_plate();
        let dt = self.last.map_or(0.0, |last| {
            now.saturating_duration_since(last).as_secs_f32().min(0.1)
        });
        self.last = Some(now);
        self.time += dt;
        let s = &mut self.settings;
        let response=ui.interact(rect,ui.id().with("cymatics-camera"),Sense::click_and_drag()).help_text("Cymatics · incoming audio drives plate resonances. Drag to rotate freely; right/Shift-drag to pan; scroll to zoom; double-click resets the camera. A narrow-band input gives clearer nodal patterns than dense music.");
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
            }
        }
        if response.hovered() {
            s.zoom =
                (s.zoom * (ui.input(|i| i.smooth_scroll_delta.y) * 0.002).exp()).clamp(0.25, 4.0);
        }
        if response.double_clicked() {
            reset_camera(s);
        }
        let Some(plate) = &self.plate else {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Preparing plate modes…",
                egui::FontId::proportional(14.0),
                theme.muted,
            );
            ui.ctx().request_repaint();
            return;
        };
        if dt > 0.0 {
            let peaks = response::peaks(audio, s, live);
            let amplitude = if live {
                audio
                    .bins
                    .iter()
                    .map(|b| b.magnitude)
                    .filter(|v| v.is_finite())
                    .fold(0.0_f32, f32::max)
            } else {
                0.0
            };
            self.drive.input_db = 20.0 * amplitude.max(1e-6).log10();
            self.drive.peaks = peaks.len();
            self.drive.peak_hz = peaks.first().map_or(0.0, |p| p.0);
            let target = response::field(plate, s, &peaks, self.time);
            response::smooth_field(
                Arc::make_mut(&mut self.field).as_mut_slice(),
                &target,
                s,
                dt,
            );
        }
        self.drive.vibration =
            (self.field.iter().map(|v| v[0]).sum::<f32>() / (GRID * GRID) as f32).sqrt();
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
        let az = s.light_angle.to_radians();
        let el = s.light_height.to_radians();
        let frame = CymaticsFrame {
            clock: [
                dt,
                self.time,
                if self.reset { 1.0 } else { 0.0 },
                self.seed as f32,
            ],
            geometry: [
                if plate.spec.circle { 1.0 } else { 0.0 },
                plate.spec.aspect as f32,
                s.mode as f32,
                if s.mode == 0 { 0.0 } else { s.displacement },
            ],
            motion: [
                s.migration,
                s.friction,
                s.agitation,
                if s.artistic { s.turbulence } else { 0.0 },
            ],
            grains: [s.grain_size, s.size_variation, s.grain_opacity, s.glow],
            recycling: [
                if s.recycle_edges { 1.0 } else { 0.0 },
                s.recycle_threshold,
                s.spawn_radius.min(s.recycle_threshold * 0.95),
                s.refresh_rate,
            ],
            lifecycle: [s.max_speed, 0.0, 0.0, 0.0],
            surface: [s.plate_opacity, s.brightness, s.node_width, s.contours],
            light: [s.ambient, s.diffuse, s.specular, s.roughness],
            direction: [
                az.cos() * el.cos(),
                el.sin(),
                az.sin() * el.cos(),
                if s.soft_grains { 1.0 } else { 0.0 },
            ],
            view: [rect.aspect_ratio(), s.zoom, s.pan_x, s.pan_y],
            axis_x: axes[0],
            axis_y: axes[1],
            axis_z: axes[2],
            camera: [
                s.camera_distance,
                2.4,
                if s.orthographic { 1.0 } else { 0.0 },
                self.field.iter().map(|v| v[0]).sum::<f32>() / (GRID * GRID) as f32,
            ],
            color_a: color(s.color_a),
            color_b: color(s.color_b),
            plate_color: color(s.plate_color),
            color: [
                s.color_mode as f32,
                s.color_spread,
                s.color_offset,
                s.contrast,
            ],
            output: [0.0; 4],
        };
        self.gpu.show(
            ui,
            rect,
            frame,
            self.field.clone(),
            [8192, 32768, 65536, 131072][s.quality as usize],
        );
        self.reset = false;
        if s.guides {
            draw_guides(ui, rect, s, axes);
        }
        if self.pending.is_some() {
            ui.painter().text(
                rect.left_bottom() + egui::vec2(12.0, -12.0),
                egui::Align2::LEFT_BOTTOM,
                "Updating plate modes…",
                egui::FontId::proportional(12.0),
                theme.muted,
            );
        }
    }
}
fn reset_camera(s: &mut CymaticsSettings) {
    s.yaw = 0.0;
    s.elevation = -0.95;
    s.roll = 0.0;
    s.zoom = 1.0;
    s.pan_x = 0.0;
    s.pan_y = 0.0;
    s.camera_distance = 4.0;
}
fn draw_guides(ui: &egui::Ui, rect: Rect, s: &CymaticsSettings, axes: [[f32; 4]; 3]) {
    let project = |p: [f32; 3]| {
        let q: [f32; 3] = std::array::from_fn(|r| (0..3).map(|c| axes[c][r] * p[c]).sum());
        let depth = if s.orthographic {
            s.camera_distance
        } else {
            s.camera_distance + q[2]
        };
        let scale = 2.4 * s.zoom * rect.aspect_ratio().min(1.0) / depth;
        rect.center()
            + egui::vec2(
                (q[0] * scale / rect.aspect_ratio() + s.pan_x) * rect.width() * 0.5,
                (q[1] * scale - s.pan_y) * rect.height() * 0.5,
            )
    };
    let painter = ui.painter_at(rect);
    for (axis, name, color) in [
        (0, "X", egui::Color32::LIGHT_RED),
        (1, "Y", egui::Color32::LIGHT_GREEN),
        (2, "Z", egui::Color32::LIGHT_BLUE),
    ] {
        let mut p = [0.0; 3];
        p[axis] = 1.3;
        let end = project(p);
        painter.line_segment([project([0.0; 3]), end], egui::Stroke::new(1.0, color));
        painter.text(
            end,
            egui::Align2::LEFT_BOTTOM,
            name,
            egui::FontId::proportional(11.0),
            color,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::{ModuleKind, ModulePane, ModuleSettings, PaneState};
    #[test]
    fn parameters_presets_and_old_workspaces_roundtrip() {
        let mut settings = ModuleSettings::Cymatics(Box::default());
        for spec in CymaticsSettings::specs() {
            assert!(spec.valid());
            let v = spec.min + (spec.max - spec.min) * 0.37;
            crate::macros::set_many(&mut settings, &[(spec.field.clone(), v)]);
            assert!(
                (serde_json::to_value(&settings).unwrap()["Cymatics"][&spec.field]
                    .as_f64()
                    .unwrap()
                    - v)
                    .abs()
                    < 0.002
            );
        }
        let mut pane = ModulePane::new(ModuleKind::Cymatics);
        assert!(pane.paste_settings(&settings));
        let snapshot = pane.snapshot();
        let decoded: PaneState =
            serde_json::from_slice(&serde_json::to_vec(&snapshot).unwrap()).unwrap();
        let mut restored = ModulePane::new(ModuleKind::Spectrum);
        restored.restore(&decoded);
        assert_eq!(snapshot, restored.snapshot());
        let mut old_cymatics = serde_json::to_value(CymaticsSettings::default()).unwrap();
        old_cymatics
            .as_object_mut()
            .unwrap()
            .remove("auto_sensitivity");
        old_cymatics
            .as_object_mut()
            .unwrap()
            .remove("recycle_edges");
        old_cymatics["gain"] = 120.0.into();
        let upgraded: CymaticsSettings = serde_json::from_value(old_cymatics).unwrap();
        assert!(upgraded.auto_sensitivity);
        assert!(upgraded.recycle_edges);
        let disabled = CymaticsSettings {
            recycle_edges: false,
            ..Default::default()
        };
        let restored: CymaticsSettings =
            serde_json::from_value(serde_json::to_value(disabled).unwrap()).unwrap();
        assert!(!restored.recycle_edges);
        assert_eq!(upgraded.gain, 120.0);
        let old = ModulePane::new(ModuleKind::Milkdrop).snapshot();
        let mut json = serde_json::to_value(&old).unwrap();
        json["sections"].as_array_mut().unwrap().truncate(15);
        json["macros"]["banks"].as_array_mut().unwrap().truncate(15);
        json["settings"]
            .as_array_mut()
            .unwrap()
            .retain(|v| v.get("Cymatics").is_none());
        let upgraded: PaneState = serde_json::from_value(json).unwrap();
        assert_eq!(old, upgraded);
    }
    #[test]
    fn hostile_settings_cannot_escape_gpu_bounds() {
        let mut s = CymaticsSettings {
            shape: u32::MAX,
            mode: u32::MAX,
            quality: u32::MAX,
            emitters: 0,
            gain: f32::INFINITY,
            friction: f32::NAN,
            aspect: -10.0,
            ..Default::default()
        };
        s.sanitize();
        assert_eq!(s.mode, 3);
        assert_eq!(s.quality, 3);
        assert_eq!(s.emitters, 1);
        assert!(s.gain.is_finite() && s.friction.is_finite());
        assert_eq!(s.aspect, 0.5);
    }
}
