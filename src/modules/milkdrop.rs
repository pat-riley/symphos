//! Native Symphos component hosting a MilkDrop-compatible preset engine.
use super::{settings_group_with_default, settings_panel};
use crate::{
    analysis::AnalysisFrame,
    help::HoverHelp,
    milkdrop::{
        preset::{self, Preset},
        runtime::{self, Request, Worker},
    },
    parameter::Parameter,
};
use serde::{Deserialize, Serialize};
use std::{sync::Arc, time::Instant};
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct MilkdropSettings {
    pub zoom: f32,
    pub rotation: f32,
    pub warp: f32,
    pub feedback: f32,
    pub brightness: f32,
    pub pan_x: f32,
    pub pan_y: f32,
    pub center_x: f32,
    pub center_y: f32,
    pub stretch_x: f32,
    pub stretch_y: f32,
    pub wave_alpha: f32,
    pub wave_red: f32,
    pub wave_green: f32,
    pub wave_blue: f32,
    pub wave_x: f32,
    pub wave_y: f32,
    pub wave_shape: f32,
    pub echo_alpha: f32,
    pub echo_zoom: f32,
    pub outer_size: f32,
    pub outer_alpha: f32,
    pub inner_size: f32,
    pub inner_alpha: f32,
    pub motion_alpha: f32,
    pub motion_length: f32,
    pub audio_gain: f32,
    pub sensitivity: f32,
    pub speed: f32,
    pub transition: f32,
    pub duration: f32,
    pub preset: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document: Option<Arc<Preset>>,
    pub autoplay: bool,
    pub shuffle: bool,
    pub favorites_only: bool,
    pub quality: u32,
    pub mesh: u32,
}
impl Default for MilkdropSettings {
    fn default() -> Self {
        Self {
            zoom: 0.0,
            rotation: 0.0,
            warp: 0.0,
            feedback: 0.0,
            brightness: 0.0,
            pan_x: 0.0,
            pan_y: 0.0,
            center_x: 0.0,
            center_y: 0.0,
            stretch_x: 0.0,
            stretch_y: 0.0,
            wave_alpha: 0.0,
            wave_red: 0.0,
            wave_green: 0.0,
            wave_blue: 0.0,
            wave_x: 0.0,
            wave_y: 0.0,
            wave_shape: 0.0,
            echo_alpha: 0.0,
            echo_zoom: 0.0,
            outer_size: 0.0,
            outer_alpha: 0.0,
            inner_size: 0.0,
            inner_alpha: 0.0,
            motion_alpha: 0.0,
            motion_length: 0.0,
            audio_gain: 1.0,
            sensitivity: 1.0,
            speed: 1.0,
            transition: 2.0,
            duration: 30.0,
            preset: 0,
            document: None,
            autoplay: false,
            shuffle: false,
            favorites_only: false,
            quality: 1,
            mesh: 48,
        }
    }
}
impl MilkdropSettings {
    fn sanitize(&mut self) {
        self.zoom = if self.zoom.is_finite() {
            self.zoom.clamp(-0.15, 0.15)
        } else {
            0.0
        };
        self.rotation = if self.rotation.is_finite() {
            self.rotation.clamp(-0.2, 0.2)
        } else {
            0.0
        };
        self.warp = if self.warp.is_finite() {
            self.warp.clamp(-2.0, 2.0)
        } else {
            0.0
        };
        self.feedback = if self.feedback.is_finite() {
            self.feedback.clamp(-0.25, 0.05)
        } else {
            0.0
        };
        self.brightness = if self.brightness.is_finite() {
            self.brightness.clamp(-2.0, 2.0)
        } else {
            0.0
        };
        self.pan_x = if self.pan_x.is_finite() {
            self.pan_x.clamp(-0.1, 0.1)
        } else {
            0.0
        };
        self.pan_y = if self.pan_y.is_finite() {
            self.pan_y.clamp(-0.1, 0.1)
        } else {
            0.0
        };
        self.center_x = if self.center_x.is_finite() {
            self.center_x.clamp(-0.5, 0.5)
        } else {
            0.0
        };
        self.center_y = if self.center_y.is_finite() {
            self.center_y.clamp(-0.5, 0.5)
        } else {
            0.0
        };
        self.stretch_x = if self.stretch_x.is_finite() {
            self.stretch_x.clamp(-0.3, 0.3)
        } else {
            0.0
        };
        self.stretch_y = if self.stretch_y.is_finite() {
            self.stretch_y.clamp(-0.3, 0.3)
        } else {
            0.0
        };
        self.wave_alpha = if self.wave_alpha.is_finite() {
            self.wave_alpha.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        self.wave_red = if self.wave_red.is_finite() {
            self.wave_red.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        self.wave_green = if self.wave_green.is_finite() {
            self.wave_green.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        self.wave_blue = if self.wave_blue.is_finite() {
            self.wave_blue.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        self.wave_x = if self.wave_x.is_finite() {
            self.wave_x.clamp(-0.5, 0.5)
        } else {
            0.0
        };
        self.wave_y = if self.wave_y.is_finite() {
            self.wave_y.clamp(-0.5, 0.5)
        } else {
            0.0
        };
        self.wave_shape = if self.wave_shape.is_finite() {
            self.wave_shape.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        self.echo_alpha = if self.echo_alpha.is_finite() {
            self.echo_alpha.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        self.echo_zoom = if self.echo_zoom.is_finite() {
            self.echo_zoom.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        self.outer_size = if self.outer_size.is_finite() {
            self.outer_size.clamp(-0.1, 0.1)
        } else {
            0.0
        };
        self.outer_alpha = if self.outer_alpha.is_finite() {
            self.outer_alpha.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        self.inner_size = if self.inner_size.is_finite() {
            self.inner_size.clamp(-0.1, 0.1)
        } else {
            0.0
        };
        self.inner_alpha = if self.inner_alpha.is_finite() {
            self.inner_alpha.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        self.motion_alpha = if self.motion_alpha.is_finite() {
            self.motion_alpha.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        self.motion_length = if self.motion_length.is_finite() {
            self.motion_length.clamp(-2.0, 2.0)
        } else {
            0.0
        };
        self.audio_gain = if self.audio_gain.is_finite() {
            self.audio_gain.clamp(0.0, 4.0)
        } else {
            1.0
        };
        self.sensitivity = if self.sensitivity.is_finite() {
            self.sensitivity.clamp(0.0, 3.0)
        } else {
            1.0
        };
        self.speed = if self.speed.is_finite() {
            self.speed.clamp(0.0, 3.0)
        } else {
            1.0
        };
        self.transition = if self.transition.is_finite() {
            self.transition.clamp(0.0, 10.0)
        } else {
            2.0
        };
        self.duration = if self.duration.is_finite() {
            self.duration.clamp(5.0, 300.0)
        } else {
            30.0
        };
        self.quality = self.quality.min(3);
        self.mesh = self.mesh.clamp(16, 128);
    }
    fn controls(&mut self, ui: &mut egui::Ui, section: &str) {
        if section == "Flow & Motion" {
            ui.add(Parameter::new(&mut self.zoom,-0.15..=0.15,0.0).key("zoom").text("Zoom")).help_text("Multiply the preset’s animated zoom by an exponential adjustment. Small positive values travel inward; negative values travel outward. Zero preserves the preset. Drag this parameter to a scene macro to control it with Signals.");
        }
        if section == "Flow & Motion" {
            ui.add(Parameter::new(&mut self.rotation,-0.2..=0.2,0.0).key("rotation").text("Rotation")).help_text("Add rotation in radians per feedback frame. Positive and negative values turn in opposite directions. Drag this parameter to a scene macro to control it with Signals.");
        }
        if section == "Flow & Motion" {
            ui.add(Parameter::new(&mut self.warp,-2.0..=2.0,0.0).key("warp").text("Warp")).help_text("Scale the preset’s warp intensity. Zero preserves it. Presets with custom shaders may implement their own warp. Drag this parameter to a scene macro to control it with Signals.");
        }
        if section == "Appearance" {
            ui.add(Parameter::new(&mut self.feedback,-0.25..=0.05,0.0).key("feedback").text("Feedback")).help_text("Add to the preset’s decay value. Positive values keep old frames longer; negative values clear trails faster. Clamped to 0–1. Drag this parameter to a scene macro to control it with Signals.");
        }
        if section == "Appearance" {
            ui.add(Parameter::new(&mut self.brightness,-2.0..=2.0,0.0).key("brightness").text("Brightness")).help_text("Scale the preset gamma control. Zero preserves the original. Custom composite shaders may ignore gamma. Drag this parameter to a scene macro to control it with Signals.");
        }
        if section == "Flow & Motion details" {
            ui.add(Parameter::new(&mut self.pan_x,-0.1..=0.1,0.0).key("pan_x").text("Flow X")).help_text("Add horizontal movement to the feedback image. Drag this parameter to a scene macro to control it with Signals.");
        }
        if section == "Flow & Motion details" {
            ui.add(Parameter::new(&mut self.pan_y,-0.1..=0.1,0.0).key("pan_y").text("Flow Y")).help_text("Add vertical movement to the feedback image. Drag this parameter to a scene macro to control it with Signals.");
        }
        if section == "Flow & Motion details" {
            ui.add(Parameter::new(&mut self.center_x,-0.5..=0.5,0.0).key("center_x").text("Center X")).help_text("Move the center of zoom and rotation horizontally. Drag this parameter to a scene macro to control it with Signals.");
        }
        if section == "Flow & Motion details" {
            ui.add(Parameter::new(&mut self.center_y,-0.5..=0.5,0.0).key("center_y").text("Center Y")).help_text("Move the center of zoom and rotation vertically. Drag this parameter to a scene macro to control it with Signals.");
        }
        if section == "Flow & Motion details" {
            ui.add(Parameter::new(&mut self.stretch_x,-0.3..=0.3,0.0).key("stretch_x").text("Stretch X")).help_text("Scale the preset’s horizontal stretch exponentially. Drag this parameter to a scene macro to control it with Signals.");
        }
        if section == "Flow & Motion details" {
            ui.add(Parameter::new(&mut self.stretch_y,-0.3..=0.3,0.0).key("stretch_y").text("Stretch Y")).help_text("Scale the preset’s vertical stretch exponentially. Drag this parameter to a scene macro to control it with Signals.");
        }
        if section == "Appearance" {
            ui.add(Parameter::new(&mut self.wave_alpha,-1.0..=1.0,0.0).key("wave_alpha").text("Wave opacity")).help_text("Add opacity to the built-in waveform. Custom waves have their own saved parameters in the preset editor. Drag this parameter to a scene macro to control it with Signals.");
        }
        if section == "Appearance" {
            ui.add(Parameter::new(&mut self.wave_red,-1.0..=1.0,0.0).key("wave_red").text("Wave red")).help_text("Add to the animated red component of the built-in waveform. Drag this parameter to a scene macro to control it with Signals.");
        }
        if section == "Appearance" {
            ui.add(Parameter::new(&mut self.wave_green,-1.0..=1.0,0.0).key("wave_green").text("Wave green")).help_text("Add to the animated green component of the built-in waveform. Drag this parameter to a scene macro to control it with Signals.");
        }
        if section == "Appearance" {
            ui.add(Parameter::new(&mut self.wave_blue,-1.0..=1.0,0.0).key("wave_blue").text("Wave blue")).help_text("Add to the animated blue component of the built-in waveform. Drag this parameter to a scene macro to control it with Signals.");
        }
        if section == "Appearance details" {
            ui.add(Parameter::new(&mut self.wave_x,-0.5..=0.5,0.0).key("wave_x").text("Wave X")).help_text("Offset the built-in waveform position horizontally. Drag this parameter to a scene macro to control it with Signals.");
        }
        if section == "Appearance details" {
            ui.add(Parameter::new(&mut self.wave_y,-0.5..=0.5,0.0).key("wave_y").text("Wave Y")).help_text("Offset the built-in waveform position vertically. Drag this parameter to a scene macro to control it with Signals.");
        }
        if section == "Appearance details" {
            ui.add(Parameter::new(&mut self.wave_shape,-1.0..=1.0,0.0).key("wave_shape").text("Wave shape")).help_text("Adjust the built-in waveform mystery parameter. Its effect depends on the preset’s wave mode. Drag this parameter to a scene macro to control it with Signals.");
        }
        if section == "Appearance details" {
            ui.add(Parameter::new(&mut self.echo_alpha,-1.0..=1.0,0.0).key("echo_alpha").text("Echo opacity")).help_text("Add to video echo opacity; positive values make repeated imagery stronger. Drag this parameter to a scene macro to control it with Signals.");
        }
        if section == "Appearance details" {
            ui.add(Parameter::new(&mut self.echo_zoom,-1.0..=1.0,0.0).key("echo_zoom").text("Echo zoom")).help_text("Scale the zoom of video echo independently of the main warp. Drag this parameter to a scene macro to control it with Signals.");
        }
        if section == "Appearance details" {
            ui.add(Parameter::new(&mut self.outer_size,-0.1..=0.1,0.0).key("outer_size").text("Outer width")).help_text("Adjust the preset’s outer border width. Drag this parameter to a scene macro to control it with Signals.");
        }
        if section == "Appearance details" {
            ui.add(Parameter::new(&mut self.outer_alpha,-1.0..=1.0,0.0).key("outer_alpha").text("Outer opacity")).help_text("Adjust the preset’s outer border opacity. Drag this parameter to a scene macro to control it with Signals.");
        }
        if section == "Appearance details" {
            ui.add(Parameter::new(&mut self.inner_size,-0.1..=0.1,0.0).key("inner_size").text("Inner width")).help_text("Adjust the preset’s inner border width. Drag this parameter to a scene macro to control it with Signals.");
        }
        if section == "Appearance details" {
            ui.add(Parameter::new(&mut self.inner_alpha,-1.0..=1.0,0.0).key("inner_alpha").text("Inner opacity")).help_text("Adjust the preset’s inner border opacity. Drag this parameter to a scene macro to control it with Signals.");
        }
        if section == "Appearance details" {
            ui.add(Parameter::new(&mut self.motion_alpha,-1.0..=1.0,0.0).key("motion_alpha").text("Vector opacity")).help_text("Adjust the visibility of the preset’s motion vectors. Drag this parameter to a scene macro to control it with Signals.");
        }
        if section == "Appearance details" {
            ui.add(Parameter::new(&mut self.motion_length,-2.0..=2.0,0.0).key("motion_length").text("Vector length")).help_text("Scale the length of the preset’s motion vectors exponentially. Drag this parameter to a scene macro to control it with Signals.");
        }
        if section == "Signal Response" {
            ui.add(Parameter::new(&mut self.audio_gain,0.0..=4.0,1.0).key("audio_gain").text("Audio gain")).help_text("Scale incoming stereo audio before MilkDrop’s waveform and bass/mid/treble analysis. Zero disables its native audio response; Signals can still drive inspector adjustments. Drag this parameter to a scene macro to control it with Signals.");
        }
        if section == "Signal Response" {
            ui.add(Parameter::new(&mut self.sensitivity,0.0..=3.0,1.0).key("sensitivity").text("Beat sensitivity")).help_text("Scale the normalized bass, mid and treble values used by preset equations and shaders. One preserves the original; zero removes this band response without muting the waveform. Drag this parameter to a scene macro to control it with Signals.");
        }
        if section == "Rendering" {
            ui.add(Parameter::new(&mut self.speed,0.0..=3.0,1.0).key("speed").text("Speed")).help_text("Scale preset time. Zero freezes time-based animation; feedback still renders. Use the component Pause button to freeze the whole image. Drag this parameter to a scene macro to control it with Signals.");
        }
        if section == "Presets" {
            ui.add(Parameter::new(&mut self.transition,0.0..=10.0,2.0).key("transition").text("Transition (s)")).help_text("Time to blend into the next preset. Zero switches immediately. Drag this parameter to a scene macro to control it with Signals.");
        }
        if section == "Presets" {
            ui.add(Parameter::new(&mut self.duration,5.0..=300.0,30.0).key("duration").text("Duration (s)")).help_text("Seconds before automatic advance when Auto play is enabled. Drag this parameter to a scene macro to control it with Signals.");
        }
    }
    fn export_preset(&self, p: &Preset) -> Preset {
        let mut p = p.clone();
        let values = self.adjustments();
        let mut code = String::new();
        if values[0] != 0. {
            let v = values[0];
            code.push_str(&format!("zoom=zoom*exp({v});"));
        }
        if values[1] != 0. {
            let v = values[1];
            code.push_str(&format!("rot=rot+({v});"));
        }
        if values[2] != 0. {
            let v = values[2];
            code.push_str(&format!("warp=warp*exp({v});"));
        }
        if values[3] != 0. {
            let v = values[3];
            code.push_str(&format!("decay=min(1,max(0,decay+({v})));"));
        }
        if values[4] != 0. {
            let v = values[4];
            code.push_str(&format!("gamma=gamma*exp({v});"));
        }
        if values[5] != 0. {
            let v = values[5];
            code.push_str(&format!("dx=dx+({v});"));
        }
        if values[6] != 0. {
            let v = values[6];
            code.push_str(&format!("dy=dy+({v});"));
        }
        if values[7] != 0. {
            let v = values[7];
            code.push_str(&format!("cx=cx+({v});"));
        }
        if values[8] != 0. {
            let v = values[8];
            code.push_str(&format!("cy=cy+({v});"));
        }
        if values[9] != 0. {
            let v = values[9];
            code.push_str(&format!("sx=sx*exp({v});"));
        }
        if values[10] != 0. {
            let v = values[10];
            code.push_str(&format!("sy=sy*exp({v});"));
        }
        if values[11] != 0. {
            let v = values[11];
            code.push_str(&format!("wave_a=min(1,max(0,wave_a+({v})));"));
        }
        if values[12] != 0. {
            let v = values[12];
            code.push_str(&format!("wave_r=min(1,max(0,wave_r+({v})));"));
        }
        if values[13] != 0. {
            let v = values[13];
            code.push_str(&format!("wave_g=min(1,max(0,wave_g+({v})));"));
        }
        if values[14] != 0. {
            let v = values[14];
            code.push_str(&format!("wave_b=min(1,max(0,wave_b+({v})));"));
        }
        if values[15] != 0. {
            let v = values[15];
            code.push_str(&format!("wave_x=wave_x+({v});"));
        }
        if values[16] != 0. {
            let v = values[16];
            code.push_str(&format!("wave_y=wave_y+({v});"));
        }
        if values[17] != 0. {
            let v = values[17];
            code.push_str(&format!("wave_mystery=wave_mystery+({v});"));
        }
        if values[18] != 0. {
            let v = values[18];
            code.push_str(&format!("echo_alpha=min(1,max(0,echo_alpha+({v})));"));
        }
        if values[19] != 0. {
            let v = values[19];
            code.push_str(&format!("echo_zoom=echo_zoom*exp({v});"));
        }
        if values[20] != 0. {
            let v = values[20];
            code.push_str(&format!("ob_size=min(0.5,max(0,ob_size+({v})));"));
        }
        if values[21] != 0. {
            let v = values[21];
            code.push_str(&format!("ob_a=min(1,max(0,ob_a+({v})));"));
        }
        if values[22] != 0. {
            let v = values[22];
            code.push_str(&format!("ib_size=min(0.5,max(0,ib_size+({v})));"));
        }
        if values[23] != 0. {
            let v = values[23];
            code.push_str(&format!("ib_a=min(1,max(0,ib_a+({v})));"));
        }
        if values[24] != 0. {
            let v = values[24];
            code.push_str(&format!("mv_a=min(1,max(0,mv_a+({v})));"));
        }
        if values[25] != 0. {
            let v = values[25];
            code.push_str(&format!("mv_l=mv_l*exp({v});"));
        }
        if !code.is_empty() {
            // MilkDrop reads contiguous, case-insensitive equation chunks.
            // Add to the last executable chunk without awakening ignored chunks
            // after a numbering gap, or colliding with an uppercase key.
            let indices: std::collections::BTreeSet<usize> = p
                .source
                .lines()
                .filter_map(|l| {
                    l.trim()
                        .to_ascii_lowercase()
                        .strip_prefix("per_frame_")?
                        .split_once('=')?
                        .0
                        .trim()
                        .parse()
                        .ok()
                })
                .collect();
            let next = (1..).find(|i| !indices.contains(i)).unwrap();
            if next == 1 {
                p.source = p
                    .source
                    .lines()
                    .map(|line| {
                        if line.trim().to_ascii_lowercase().starts_with("per_frame_")
                            && line.split_once('=').is_some_and(|(k, _)| {
                                k.trim()
                                    .to_ascii_lowercase()
                                    .strip_prefix("per_frame_")
                                    .is_some_and(|n| n.parse::<usize>().is_ok())
                            })
                        {
                            format!("// Previously non-executing chunk: {line}")
                        } else {
                            line.to_owned()
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                p.source.push_str(&format!("\nper_frame_1=;{code}\n"));
            } else if indices.range(next..).next().is_none() {
                p.source.push_str(&format!("\nper_frame_{next}=;{code}\n"));
            } else {
                let key = format!("per_frame_{}", next - 1);
                let newline = if p.source.contains("\r\n") {
                    "\r\n"
                } else {
                    "\n"
                };
                p.source = p
                    .source
                    .lines()
                    .map(|line| {
                        if line
                            .split_once('=')
                            .is_some_and(|(k, _)| k.trim().eq_ignore_ascii_case(&key))
                        {
                            // Insert before comments so both the code and annotation survive.
                            let end = [line.find("//"), line.find("\\\\")]
                                .into_iter()
                                .flatten()
                                .min()
                                .unwrap_or(line.len());
                            format!("{};{code}{}", &line[..end], &line[end..])
                        } else {
                            line.to_owned()
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(newline)
                    + newline;
            }
        }
        p
    }
    fn adjustments(&self) -> [f32; 32] {
        let mut values = [0.; 32];
        values[0] = self.zoom;
        values[1] = self.rotation;
        values[2] = self.warp;
        values[3] = self.feedback;
        values[4] = self.brightness;
        values[5] = self.pan_x;
        values[6] = self.pan_y;
        values[7] = self.center_x;
        values[8] = self.center_y;
        values[9] = self.stretch_x;
        values[10] = self.stretch_y;
        values[11] = self.wave_alpha;
        values[12] = self.wave_red;
        values[13] = self.wave_green;
        values[14] = self.wave_blue;
        values[15] = self.wave_x;
        values[16] = self.wave_y;
        values[17] = self.wave_shape;
        values[18] = self.echo_alpha;
        values[19] = self.echo_zoom;
        values[20] = self.outer_size;
        values[21] = self.outer_alpha;
        values[22] = self.inner_size;
        values[23] = self.inner_alpha;
        values[24] = self.motion_alpha;
        values[25] = self.motion_length;
        values
    }
}
pub(crate) struct Milkdrop {
    pub presets: Vec<Arc<Preset>>,
    settings: MilkdropSettings,
    worker: Option<Worker>,
    texture: Option<egui::TextureHandle>,
    texture_context: Option<egui::Context>,
    last: Option<Instant>,
    time: f64,
    elapsed: f32,
    restart: u64,
    error: String,
    search: String,
    scalar_filter: String,
    draft: Option<Preset>,
    milliseconds: f32,
    warning: String,
}
impl Default for Milkdrop {
    fn default() -> Self {
        Self {
            presets: preset::builtins().into_iter().map(Arc::new).collect(),
            settings: MilkdropSettings::default(),
            worker: None,
            texture: None,
            texture_context: None,
            last: None,
            time: 0.,
            elapsed: 0.,
            restart: 0,
            error: String::new(),
            search: String::new(),
            scalar_filter: String::new(),
            draft: None,
            milliseconds: 0.,
            warning: String::new(),
        }
    }
}
impl Milkdrop {
    fn remaining_library_bytes(&self) -> usize {
        preset::MAX_LIBRARY_BYTES
            .saturating_sub(self.presets.iter().map(|p| p.storage_bytes()).sum())
    }
    fn add_preset(&mut self, p: Preset) {
        if self.presets.len() >= preset::MAX_PRESETS
            || p.storage_bytes() > self.remaining_library_bytes()
        {
            self.error = "Preset library is full (2048 presets or 24 MB). Remove presets before importing more.".into();
            return;
        }
        self.presets.push(Arc::new(p));
        self.select(self.presets.len() - 1);
    }
    pub fn settings_snapshot(&self) -> MilkdropSettings {
        self.settings.clone()
    }
    pub fn apply_settings(&mut self, s: &MilkdropSettings) {
        if self.settings.preset != s.preset {
            self.elapsed = 0.;
            self.draft = None;
        }
        self.settings = s.clone();
        if let Some(document) = self.settings.document.take() {
            if document.validate().is_ok() {
                self.settings.preset =
                    if let Some(index) = self.presets.iter().position(|p| **p == *document) {
                        index
                    } else if self.presets.len() >= preset::MAX_PRESETS
                        || document.storage_bytes() > self.remaining_library_bytes()
                    {
                        self.error = "Preset library is full (2048 presets or 24 MB)".into();
                        self.settings.preset.min(self.presets.len() - 1)
                    } else {
                        self.presets.push(document);
                        self.presets.len() - 1
                    };
            } else {
                self.error = "Invalid preset in copied component settings".into();
            }
        }
        self.settings.sanitize();
    }
    pub fn export_settings(&self) -> MilkdropSettings {
        let mut s = self.settings.clone();
        s.document = self.presets.get(s.preset).cloned();
        s
    }
    pub fn clear(&mut self) {
        self.worker = None;
        self.texture = None;
        self.last = None;
        self.time = 0.;
        self.elapsed = 0.;
        self.error.clear();
        self.warning.clear();
    }
    pub fn release_if_inactive(&mut self) {
        if self.worker.is_some() {
            self.clear();
        }
    }
    pub fn reset_settings(&mut self) {
        self.settings = MilkdropSettings::default();
        self.draft = None;
        self.clear();
    }
    fn select(&mut self, i: usize) {
        if i < self.presets.len() {
            self.settings.preset = i;
            self.elapsed = 0.;
            self.draft = None;
            self.error.clear();
            self.warning.clear();
        }
    }
    fn advance(&mut self, back: bool) {
        let choices = self
            .presets
            .iter()
            .enumerate()
            .filter(|(_, p)| !self.settings.favorites_only || p.favorite)
            .map(|(i, _)| i)
            .collect::<Vec<_>>();
        if choices.is_empty() {
            return;
        }
        let current = choices
            .iter()
            .position(|i| *i == self.settings.preset)
            .unwrap_or(0);
        let next = if self.settings.shuffle && choices.len() > 1 {
            let n = (self.time.to_bits().wrapping_mul(6364136223846793005) >> 32) as usize;
            (current + 1 + n % (choices.len() - 1)) % choices.len()
        } else if back {
            (current + choices.len() - 1) % choices.len()
        } else {
            (current + 1) % choices.len()
        };
        self.select(choices[next]);
    }
    pub fn controls(&mut self, ui: &mut egui::Ui) {
        self.settings.sanitize();
        self.settings.preset = self
            .settings
            .preset
            .min(self.presets.len().saturating_sub(1));
        settings_panel(ui, "Presets", |ui| {
            ui.horizontal(|ui| {
                ui.menu_button("Import…",|ui| {
                    if ui.add_enabled(self.presets.len()<preset::MAX_PRESETS,egui::Button::new("Preset file…")).clicked() {
                        ui.close();
                        if let Some(path)=preset::choose(false,"",false) {
                            match Preset::import(&path) {
                                Ok(p)=>self.add_preset(p),
                                Err(e)=>self.error=e,
                            }
                        }
                    }
                    if ui.add_enabled(self.presets.len()<preset::MAX_PRESETS,egui::Button::new("Preset folder…")).clicked() {
                        ui.close();
                        if let Some(dir)=preset::choose(false,"",true) {
                            match preset::import_folder(&dir,preset::MAX_PRESETS-self.presets.len(),self.remaining_library_bytes()) {
                                Ok((presets,errors))=>{
                                    let first=self.presets.len();self.presets.extend(presets.into_iter().map(Arc::new));self.select(first);
                                    self.error=errors.join("\n");
                                },Err(e)=>self.error=e,
                            }
                        }
                    }
                }).response.help_text("Import .milk files or a folder. Equations, shaders and adjacent textures are preserved inside your project.");
                if ui.button("Export…").help_text("Export the current preset with live visual adjustments baked into per-frame equations, plus its textures. Audio gain, playback and quality remain Symphos settings.").clicked()
                    && let Some(p)=self.presets.get(self.settings.preset)
                    && let Some(path)=preset::choose(true,&format!("{}.milk",p.name),false) {
                    self.error=self.settings.export_preset(p).export(&path).err().unwrap_or_default();
                }
            });
            ui.text_edit_singleline(&mut self.search)
                .help_text("Search this component’s preset library.");
            let selected = self
                .presets
                .get(self.settings.preset)
                .map_or("No preset", |p| p.name.as_str());
            let mut selection = self.settings.preset;
            crate::dropdown::ComboBox::from_id_salt("milkdrop-preset")
                .selected_text(selected)
                .width(ui.available_width())
                .show_ui(ui, |ui| {
                    for (i, p) in self.presets.iter().enumerate() {
                        if p.name.to_lowercase().contains(&self.search.to_lowercase())
                            && (!self.settings.favorites_only || p.favorite)
                        {
                            ui.selectable_value(&mut selection, i, &p.name);
                        }
                    }
                });
            if selection != self.settings.preset {
                self.select(selection);
            }
            ui.horizontal(|ui| {
                if ui.button("Previous").clicked() {
                    self.advance(true);
                }
                if ui.button("Next").clicked() {
                    self.advance(false);
                }
                if ui.button("Restart").clicked() {
                    self.restart += 1;
                    self.elapsed = 0.;
                    self.error.clear();
                    self.warning.clear();
                }
            });
            if let Some(p) = self.presets.get_mut(self.settings.preset) {
                let mut favorite = p.favorite;
                if ui.checkbox(&mut favorite, "Favorite").changed() {
                    Arc::make_mut(p).favorite = favorite;
                }
            }
            ui.checkbox(&mut self.settings.autoplay,"Auto play").help_text("Advance through this component’s presets. Turn off to lock the current preset. Does not switch Symphos scenes.");
            ui.checkbox(&mut self.settings.shuffle, "Shuffle");
            ui.checkbox(&mut self.settings.favorites_only, "Favorites only");
            self.settings.controls(ui, "Presets");
            settings_group_with_default(ui, "Preset assets & editing", false, |ui| {
                let remaining_bytes = self.remaining_library_bytes();
                if ui.button("Import texture folder…").clicked()
                    && let Some(dir) = preset::choose(false, "", true)
                    && let Some(p) = self.presets.get_mut(self.settings.preset)
                {
                    let mut copy = (**p).clone();
                    match copy.import_textures(&dir) {
                        Ok(()) if copy.storage_bytes() <= p.storage_bytes() + remaining_bytes => {
                            *p = Arc::new(copy)
                        }
                        Ok(()) => {
                            self.error =
                                "Textures would exceed the 24 MB embedded library limit.".into()
                        }
                        Err(e) => self.error = e,
                    }
                }
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            self.presets.len() < preset::MAX_PRESETS,
                            egui::Button::new("Duplicate preset"),
                        )
                        .clicked()
                        && let Some(p) = self.presets.get(self.settings.preset)
                    {
                        let mut p = (**p).clone();
                        while p.name.len() > 235 {
                            p.name.pop();
                        }
                        p.name.push_str(" copy");
                        self.add_preset(p);
                    }
                    if ui
                        .add_enabled(self.presets.len() > 1, egui::Button::new("Remove"))
                        .clicked()
                    {
                        self.presets
                            .remove(self.settings.preset.min(self.presets.len() - 1));
                        self.select(self.settings.preset.min(self.presets.len() - 1));
                        self.restart += 1;
                    }
                });
                if self.draft.is_none() && ui.button("Edit saved preset values").clicked() {
                    self.draft = self
                        .presets
                        .get(self.settings.preset)
                        .map(|p| (**p).clone());
                }
                if let Some(draft) = &mut self.draft {
                    ui.label("Saved values are starting points; preset equations can animate them. Apply restarts the preset. Use performance adjustments for live changes.");
                    ui.text_edit_singleline(&mut draft.name);
                    ui.text_edit_singleline(&mut self.scalar_filter).help_text(
                        "Filter saved parameter names. Includes custom shapes and waves.",
                    );
                    for (key, mut value) in draft.scalars() {
                        if key
                            .to_lowercase()
                            .contains(&self.scalar_filter.to_lowercase())
                        {
                            ui.push_id(&key, |ui| {
                                ui.horizontal(|ui| {
                                    ui.add_sized(
                                        [(ui.available_width() - 90.).max(60.), 20.],
                                        egui::Label::new(&key).truncate(),
                                    )
                                    .on_hover_text(&key);
                                    if ui
                                        .add(egui::DragValue::new(&mut value).speed(0.01))
                                        .changed()
                                    {
                                        draft.set_scalar(&key, value);
                                    }
                                });
                            });
                        }
                    }
                    let mut apply = false;
                    let mut cancel = false;
                    ui.horizontal(|ui| {
                        apply = ui.button("Apply edits").clicked();
                        cancel = ui.button("Cancel edits").clicked();
                    });
                    if apply {
                        let p = self.draft.as_ref().unwrap();
                        match p.validate() {
                            Ok(()) => {
                                self.presets[self.settings.preset] = Arc::new(p.clone());
                                self.draft = None;
                                self.restart += 1;
                                self.error.clear();
                                self.warning.clear();
                            }
                            Err(e) => self.error = e,
                        }
                    }
                    if cancel {
                        self.draft = None;
                    }
                }
            });
        });
        for category in ["Flow & Motion", "Appearance", "Signal Response"] {
            settings_panel(ui, category, |ui| {
                if category == "Signal Response" {
                    ui.label("Audio gain controls the waveform input. Beat sensitivity scales the bands used by preset equations and shaders. Both default to 1.");
                } else {
                    ui.label("Adjustments preserve the preset at zero. Custom shaders may bypass individual built-in controls.");
                }
                settings_group_with_default(ui, "Common controls", true, |ui| {
                    self.settings.controls(ui, category);
                });
                if category != "Signal Response" {
                    settings_group_with_default(ui, "Detailed controls", false, |ui| {
                        self.settings.controls(ui, &format!("{category} details"));
                    });
                }
            });
        }
        settings_panel(ui, "Rendering", |ui| {
            self.settings.controls(ui, "Rendering");
            crate::dropdown::ComboBox::from_id_salt("milkdrop-quality").selected_text(["640 px","960 px","1280 px","1920 px"][self.settings.quality as usize]).show_ui(ui,|ui|{for (i,label) in ["640 px","960 px","1280 px","1920 px"].iter().enumerate(){ui.selectable_value(&mut self.settings.quality,i as u32,*label);}}).response.help_text("Maximum output width or height. Lower resolutions reduce OpenGL-to-wgpu transfer cost. Independent of display FPS.");
            ui.add(Parameter::new(&mut self.settings.mesh,16..=128,48).text("Warp mesh detail")).help_text("Resolution of the per-vertex equation grid. Higher values improve detailed distortion but require more CPU work.");
            ui.label(format!(
                "Renderer {:.1} ms · projectM 4.2",
                self.milliseconds
            ));
        });
        if !self.warning.is_empty() {
            ui.colored_label(ui.visuals().warn_fg_color, &self.warning);
            ui.label("Import the preset’s texture folder in Presets → Preset assets & editing.");
        }
        if !self.error.is_empty() {
            ui.colored_label(ui.visuals().error_fg_color, &self.error);
        }
    }
    pub fn draw(
        &mut self,
        ui: &mut egui::Ui,
        rect: egui::Rect,
        frame: &AnalysisFrame,
        now: Instant,
        live: bool,
        frozen: bool,
    ) {
        if self.texture_context.as_ref() != Some(ui.ctx()) {
            self.texture = None;
            self.texture_context = Some(ui.ctx().clone());
        }
        self.settings.sanitize();
        if self.presets.is_empty() {
            self.presets = preset::builtins().into_iter().map(Arc::new).collect();
        }
        self.settings.preset = self.settings.preset.min(self.presets.len() - 1);
        let dt = self.last.map_or(0., |last| {
            now.saturating_duration_since(last).as_secs_f32().min(0.1)
        });
        self.last = Some(now);
        if !frozen {
            self.time += f64::from(dt * self.settings.speed);
            self.elapsed += dt;
            if self.settings.autoplay && self.elapsed >= self.settings.duration {
                self.advance(false);
            }
        }
        if self.worker.is_none() {
            self.worker = Some(Worker::new(ui.ctx().clone()));
        }
        let worker = self.worker.as_ref().unwrap();
        if let Some(output) = worker
            .output
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
            && output
                .preset
                .as_ref()
                .is_none_or(|p| Arc::ptr_eq(p, &self.presets[self.settings.preset]))
        {
            self.milliseconds = output.milliseconds;
            self.warning = output.warning;
            match output.image {
                Ok(image) => {
                    if let Some(texture) = &mut self.texture {
                        texture.set(image, egui::TextureOptions::LINEAR);
                    } else {
                        self.texture = Some(ui.ctx().load_texture(
                            "milkdrop",
                            image,
                            egui::TextureOptions::LINEAR,
                        ));
                    }
                }
                Err(e) => self.error = e,
            }
        }
        if !frozen || self.texture.is_none() {
            let cap = if ui
                .data(|d| d.get_temp::<bool>(egui::Id::new("recording-output")))
                .unwrap_or(false)
            {
                4096.0
            } else {
                [640., 960., 1280., 1920.][self.settings.quality as usize]
            };
            let physical = rect.size() * ui.ctx().pixels_per_point();
            let scale = (cap / physical.max_elem().max(1.)).min(1.);
            let size = [
                (physical.x * scale).round().max(16.) as u32,
                (physical.y * scale).round().max(16.) as u32,
            ];
            worker.submit(Request {
                preset: self.presets[self.settings.preset].clone(),
                size,
                time: self.time,
                fps: ui
                    .data(|d| d.get_temp::<u32>(egui::Id::new("visual-target-fps")))
                    .unwrap_or(60),
                pcm: runtime::audio_window(frame, live, self.settings.audio_gain),
                controls: self.settings.adjustments(),
                sensitivity: self.settings.sensitivity,
                mesh: self.settings.mesh,
                transition: self.settings.transition,
                restart: self.restart,
            });
        }
        if let Some(texture) = &self.texture {
            ui.painter().image(
                texture.id(),
                rect,
                egui::Rect::from_min_max(egui::pos2(0., 1.), egui::pos2(1., 0.)),
                egui::Color32::WHITE,
            );
        } else {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                if self.error.is_empty() {
                    "Loading MilkDrop…"
                } else {
                    &self.error
                },
                egui::FontId::proportional(14.),
                ui.visuals().text_color(),
            );
        }
        if !self.error.is_empty() && self.texture.is_some() {
            ui.painter().text(
                rect.left_top() + egui::vec2(10., 10.),
                egui::Align2::LEFT_TOP,
                "Preset error — see inspector",
                egui::FontId::proportional(12.),
                ui.visuals().error_fg_color,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::{ModuleKind, ModulePane, ModuleSettings, PaneState};
    #[test]
    fn milkdrop_inspector_drag_assigns_scene_macro_without_changing_the_preset() {
        use crate::macros::{CONTEXT, ParameterContext, ParameterDrag};
        use crate::theme::AppTheme;
        use egui::Rect;
        let ctx = egui::Context::default();
        let theme = AppTheme::default();
        let mut panes = [ModulePane::new(ModuleKind::Milkdrop)];
        panes[0].select_section(crate::modules::SettingsSection::Flow);
        let base = panes[0].copy_settings();
        let mut macros = crate::macros::scene::SceneMacros::default();
        let mut render = |events| {
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1400.0, 900.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    macros.bar(
                        ui,
                        Rect::from_min_size(egui::pos2(0.0, 650.0), egui::vec2(1400.0, 220.0)),
                        &theme,
                        "milkdrop-scene",
                        &mut panes,
                    );
                    let bounds =
                        Rect::from_min_size(egui::pos2(1040.0, 0.0), egui::vec2(360.0, 640.0));
                    let mut inspector = ui.new_child(egui::UiBuilder::new().max_rect(bounds));
                    inspector.set_clip_rect(bounds);
                    inspector
                        .data_mut(|d| d.insert_temp(egui::Id::new("inspector-bounds"), bounds));
                    panes[0].inspector_controls(
                        &mut inspector,
                        &crate::analysis::AnalysisFrame::default(),
                    );
                    ui.data_mut(|d| {
                        d.remove::<Rect>(egui::Id::new("inspector-bounds"));
                        d.remove::<ParameterContext>(egui::Id::new(CONTEXT));
                    });
                },
            );
            out.textures_delta.clear();
            out
        };
        let out = render(vec![]);
        let origin = out
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.job.text == "Zoom" => {
                    Some(t.pos + t.galley.size() * 0.5)
                }
                _ => None,
            })
            .expect("zoom inspector label missing");
        let target = egui::pos2(55.0, 720.0);
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            pressed,
            button: egui::PointerButton::Primary,
            modifiers: egui::Modifiers::NONE,
        };
        render(vec![egui::Event::PointerMoved(origin)]);
        render(vec![button(origin, true)]);
        render(vec![egui::Event::PointerMoved(
            origin + egui::vec2(-16.0, 0.0),
        )]);
        assert_eq!(
            egui::DragAndDrop::payload::<ParameterDrag>(&ctx)
                .unwrap()
                .spec
                .field,
            "zoom"
        );
        render(vec![egui::Event::PointerMoved(target)]);
        render(vec![button(target, false)]);
        render(vec![]);
        let slot = &macros.slots[0];
        assert_eq!(slot.parameter.as_ref().unwrap().field, "zoom");
        assert_eq!(slot.target.as_ref().unwrap().kind, ModuleKind::Milkdrop);
        assert_eq!(slot.target.as_ref().unwrap().pane, panes[0].instance_id());
        assert_eq!(panes[0].copy_settings(), base);
    }
    #[test]
    fn fourteen_component_project_migrates_without_creating_signals_or_macros() {
        let original = ModulePane::new(ModuleKind::Orbital).snapshot();
        let mut json = serde_json::to_value(&original).unwrap();
        json["settings"]
            .as_array_mut()
            .unwrap()
            .retain(|s| s.get("Milkdrop").is_none());
        json["sections"].as_array_mut().unwrap().truncate(14);
        json["macros"]["banks"].as_array_mut().unwrap().truncate(14);
        json.as_object_mut().unwrap().remove("milkdrop_presets");
        let decoded: PaneState = serde_json::from_value(json).unwrap();
        let mut pane = ModulePane::new(ModuleKind::Spectrum);
        pane.restore(&decoded);
        assert_eq!(pane.snapshot(), original);
        assert!(
            pane.macros
                .banks
                .iter()
                .flatten()
                .all(|s| s.parameter.is_none() && s.signal.is_none())
        );
    }
    #[test]
    fn scene_snapshot_preserves_presets_assets_and_adjustments() {
        let mut pane = ModulePane::new(ModuleKind::Milkdrop);
        let mut preset = preset::builtins().remove(2);
        preset.name = "My shader".into();
        preset.favorite = true;
        preset
            .textures
            .insert("sample.png".into(), "00010203".into());
        pane.milkdrop.presets.push(Arc::new(preset));
        pane.milkdrop.settings.preset = 3;
        pane.milkdrop.settings.rotation = 0.03;
        let saved = pane.snapshot();
        let json = serde_json::to_vec(&saved).unwrap();
        let restored: PaneState = serde_json::from_slice(&json).unwrap();
        let mut other = ModulePane::new(ModuleKind::Spectrum);
        other.restore(&restored);
        assert_eq!(other.snapshot(), saved);
        assert_eq!(other.milkdrop.presets[3].name, "My shader");
        assert!(other.milkdrop.worker.is_none());
    }
    #[test]
    fn clipboard_transfers_preset_without_putting_assets_in_live_macro_settings() {
        let mut source = ModulePane::new(ModuleKind::Milkdrop);
        let mut p = preset::builtins().remove(0);
        p.name = "Custom source".into();
        source.milkdrop.presets.push(Arc::new(p));
        source.milkdrop.settings.preset = 3;
        let mut target = ModulePane::new(ModuleKind::Milkdrop);
        assert!(target.paste_settings(&source.export_settings()));
        assert_eq!(
            target.milkdrop.presets[target.milkdrop.settings.preset].name,
            "Custom source"
        );
        let json = serde_json::to_string(&target.copy_settings()).unwrap();
        assert!(!json.contains("source"));
        assert!(!json.contains("textures"));
    }
    #[test]
    fn live_adjustments_are_assignable_and_do_not_restart_the_preset() {
        let mut pane = ModulePane::new(ModuleKind::Milkdrop);
        let current = pane.milkdrop.presets[0].clone();
        let mut settings = pane.copy_settings();
        assert!(crate::macros::set(&mut settings, "rotation", 0.07));
        pane.apply_macro_settings(&settings);
        assert_eq!(pane.milkdrop.settings.rotation, 0.07);
        assert!(Arc::ptr_eq(&pane.milkdrop.presets[0], &current));
        assert_eq!(pane.milkdrop.restart, 0);
    }
    #[test]
    fn exporting_adjustments_retains_original_shaders_and_default_is_lossless() {
        let p = preset::builtins().remove(2);
        let defaults = MilkdropSettings::default();
        assert_eq!(defaults.export_preset(&p), p);
        let settings = MilkdropSettings {
            zoom: 0.04,
            rotation: -0.02,
            wave_alpha: -0.3,
            ..defaults
        };
        let export = settings.export_preset(&p);
        assert!(export.source.starts_with(&p.source));
        assert!(export.source.contains("zoom=zoom*exp(0.04)"));
        assert!(export.source.contains("rot=rot+(-0.02)"));
        assert!(export.source.contains("per_frame_3="));
        assert!(export.validate().is_ok());
    }
    #[test]
    fn export_handles_case_insensitive_keys_and_ignored_sparse_equations() {
        let settings = MilkdropSettings {
            zoom: 0.1,
            ..Default::default()
        };
        let mut p = preset::builtins().remove(0);
        p.source = "[preset00]\nPER_FRAME_1=zoom=1; // keep me\nper_frame_3=zoom=99;\n".into();
        let output = settings.export_preset(&p);
        assert!(
            output
                .source
                .contains("PER_FRAME_1=zoom=1; ;zoom=zoom*exp(0.1);// keep me")
        );
        assert!(!output.source.contains("per_frame_2="));
        p.source = "[preset00]\nper_frame_2=zoom=99;\n".into();
        let output = settings.export_preset(&p);
        assert!(
            output
                .source
                .contains("// Previously non-executing chunk: per_frame_2=zoom=99;")
        );
        assert!(output.source.contains("\nper_frame_1=;zoom=zoom*exp(0.1);"));
    }
    #[test]
    fn manual_preset_selection_and_favorites_playlist_are_component_local() {
        let mut m = Milkdrop::default();
        let other = Milkdrop::default();
        m.select(2);
        m.elapsed = 8.;
        m.advance(false);
        assert_eq!(m.settings.preset, 0);
        assert_eq!(m.elapsed, 0.);
        Arc::make_mut(&mut m.presets[1]).favorite = true;
        m.settings.favorites_only = true;
        m.advance(false);
        assert_eq!(m.settings.preset, 1);
        assert_eq!(other.settings.preset, 0);
        assert!(!other.presets[1].favorite);
    }
    #[test]
    fn untrusted_settings_are_bounded_before_native_rendering() {
        let mut m = Milkdrop::default();
        let s = MilkdropSettings {
            quality: u32::MAX,
            mesh: u32::MAX,
            zoom: f32::NAN,
            audio_gain: 1000.,
            ..Default::default()
        };
        m.apply_settings(&s);
        assert_eq!(m.settings.quality, 3);
        assert_eq!(m.settings.mesh, 128);
        assert_eq!(m.settings.zoom, 0.);
        assert_eq!(m.settings.audio_gain, 4.);
    }
    #[test]
    fn inspector_sections_register_common_and_detailed_parameter_controls() {
        let mut pane = ModulePane::new(ModuleKind::Milkdrop);
        let context = egui::Context::default();
        for section in pane.sections().to_vec() {
            pane.select_section(section);
            let mut output = context.run_ui(egui::RawInput::default(), |ui| {
                ui.set_max_width(300.);
                pane.inspector_controls(ui, &AnalysisFrame::default());
            });
            output.textures_delta.clear();
        }
        assert!(pane.milkdrop.worker.is_none());
        assert!(matches!(pane.copy_settings(), ModuleSettings::Milkdrop(_)));
    }
}
