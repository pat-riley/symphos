// Explicit settings-only snapshots: no samples, histories, textures, or clocks.
// Clone is deliberate so this schema can grow owned preset fields later.
macro_rules! module_settings {
    ($module:ident, $settings:ident, { $($field:ident : $ty:ty => $($path:ident).+),* $(,)? }) => {
        #[derive(Clone, Debug, PartialEq)]
        pub(crate) struct $settings { $( $field: $ty, )* }
        impl $module {
            #[allow(clippy::clone_on_copy)]
            pub(crate) fn settings_snapshot(&self) -> $settings {
                $settings { $( $field: self.$($path).+.clone(), )* }
            }
            #[allow(clippy::clone_on_copy)]
            pub(crate) fn apply_settings(&mut self, settings: &$settings) {
                $( self.$($path).+ = settings.$field.clone(); )*
            }
        }
    };
}

mod camera_gizmo;
mod frequency;
mod spectrogram;
mod spectrum;
mod waterfall;
mod waveform;

use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, FontId, Pos2, Rect};

use crate::help::HoverHelp;
use crate::icons::Icon;
use crate::{analysis::AnalysisFrame, theme::AppTheme};
use spectrogram::Spectrogram;
use spectrum::Spectrum;
use waterfall::Waterfall;
use waveform::Waveform;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModuleKind {
    Waterfall,
    Spectrum,
    Waveform,
    Spectrogram,
}

impl ModuleKind {
    pub const ALL: [Self; 4] = [
        Self::Waterfall,
        Self::Spectrum,
        Self::Waveform,
        Self::Spectrogram,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Waterfall => "3D Waterfall",
            Self::Spectrum => "Frequency spectrum",
            Self::Waveform => "Waveform",
            Self::Spectrogram => "Spectrogram",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Waterfall => {
                "A 3D frequency history: X is frequency, Y is time, and Z is signal level. Drag to orbit; right-drag or Shift-drag to pan."
            }
            Self::Spectrum => {
                "Current signal level by frequency. Hover over a band to inspect its frequency and level."
            }
            Self::Waveform => {
                "Audio amplitude over a short time window. Use the bottom-bar stereo/mix icon to switch between separate channels and a combined signal."
            }
            Self::Spectrogram => {
                "Frequency over time, with color showing signal level. Newest audio appears on the right or bottom, depending on orientation."
            }
        }
    }
}

pub struct ModulePane {
    pub kind: ModuleKind,
    waterfall: Waterfall,
    spectrum: Spectrum,
    waveform: Waveform,
    spectrogram: Spectrogram,
    active_sections: [usize; 4],
    frozen: Option<FrozenFrame>,
    paused_duration: Duration,
    pub last_capture: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ModuleSettings {
    Waterfall(waterfall::WaterfallSettings),
    Spectrum(spectrum::SpectrumSettings),
    Waveform(waveform::WaveformSettings),
    Spectrogram(spectrogram::SpectrogramSettings),
}

impl ModuleSettings {
    pub fn kind(&self) -> ModuleKind {
        match self {
            Self::Waterfall(_) => ModuleKind::Waterfall,
            Self::Spectrum(_) => ModuleKind::Spectrum,
            Self::Waveform(_) => ModuleKind::Waveform,
            Self::Spectrogram(_) => ModuleKind::Spectrogram,
        }
    }
}

struct FrozenFrame {
    frame: AnalysisFrame,
    started: Instant,
    time: Instant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsSection {
    Camera,
    History,
    Geometry,
    Frequency,
    Response,
    Appearance,
    TimeWindow,
    Amplitude,
}

impl SettingsSection {
    pub fn title(self) -> &'static str {
        match self {
            Self::Camera => "Camera",
            Self::History => "Time & History",
            Self::Geometry => "Geometry",
            Self::Frequency => "Frequency Range",
            Self::Response => "Signal Response",
            Self::Appearance => "Appearance",
            Self::TimeWindow => "Time Window",
            Self::Amplitude => "Amplitude",
        }
    }
    pub fn icon(self) -> Icon {
        match self {
            Self::Camera => Icon::Camera,
            Self::History | Self::TimeWindow => Icon::Time,
            Self::Geometry => Icon::Geometry,
            Self::Frequency => Icon::Frequency,
            Self::Response => Icon::Response,
            Self::Appearance => Icon::Appearance,
            Self::Amplitude => Icon::Waveform,
        }
    }
}

impl ModulePane {
    pub fn copy_settings(&self) -> ModuleSettings {
        match self.kind {
            ModuleKind::Waterfall => ModuleSettings::Waterfall(self.waterfall.settings_snapshot()),
            ModuleKind::Spectrum => ModuleSettings::Spectrum(self.spectrum.settings_snapshot()),
            ModuleKind::Waveform => ModuleSettings::Waveform(self.waveform.settings_snapshot()),
            ModuleKind::Spectrogram => {
                ModuleSettings::Spectrogram(self.spectrogram.settings_snapshot())
            }
        }
    }

    pub fn paste_settings(&mut self, settings: &ModuleSettings) -> bool {
        if settings.kind() != self.kind {
            return false;
        }
        match settings {
            ModuleSettings::Waterfall(settings) => self.waterfall.apply_settings(settings),
            ModuleSettings::Spectrum(settings) => self.spectrum.apply_settings(settings),
            ModuleSettings::Waveform(settings) => self.waveform.apply_settings(settings),
            ModuleSettings::Spectrogram(settings) => self.spectrogram.apply_settings(settings),
        }
        true
    }

    pub fn new(kind: ModuleKind) -> Self {
        Self {
            kind,
            waterfall: Waterfall::default(),
            spectrum: Spectrum::default(),
            waveform: Waveform::default(),
            spectrogram: Spectrogram::default(),
            active_sections: [0; 4],
            frozen: None,
            paused_duration: Duration::ZERO,
            last_capture: 0,
        }
    }

    pub fn reset(&mut self) {
        self.last_capture = 0;
        self.frozen = None;
        self.paused_duration = Duration::ZERO;
        self.waterfall.clear();
        self.spectrum.clear();
        self.spectrogram.clear();
    }

    pub fn is_frozen(&self) -> bool {
        self.frozen.is_some()
    }

    pub fn toggle_freeze(&mut self, frame: &AnalysisFrame, now: Instant) {
        if let Some(frozen) = self.frozen.take() {
            self.paused_duration += now.saturating_duration_since(frozen.started);
            self.last_capture = frame.sequence;
        } else {
            self.frozen = Some(FrozenFrame {
                frame: frame.clone(),
                started: now,
                time: now - self.paused_duration,
            });
        }
    }

    fn reset_settings(&mut self) {
        match self.kind {
            ModuleKind::Waterfall => self.waterfall.reset_settings(),
            ModuleKind::Spectrum => self.spectrum.reset_settings(),
            ModuleKind::Waveform => self.waveform.reset_settings(),
            ModuleKind::Spectrogram => self.spectrogram.reset_settings(),
        }
    }

    pub fn controls(&mut self, ui: &mut egui::Ui, _frame: &AnalysisFrame) {
        ui.spacing_mut().slider_width = ui.spacing().slider_width.min(78.0);
        let section = self.active_section();
        ui.push_id(self.kind.label(), |ui| {
            ui.data_mut(|data| data.insert_temp(ui.id().with("active-module-section"), section));
            match self.kind {
                ModuleKind::Waterfall => self.waterfall.controls(ui),
                ModuleKind::Spectrum => self.spectrum.controls(ui),
                ModuleKind::Waveform => self.waveform.controls(ui),
                ModuleKind::Spectrogram => self.spectrogram.controls(ui),
            }
            ui.add_space(12.0);
            ui.separator();
            if ui.small_button("Reset module settings")
                .help_text("Restore all settings for this module in this pane only. Retains captured history and leaves other panes, audio, and global settings unchanged.")
                .clicked() {
                self.reset_settings();
            }
        });
    }

    pub fn sections(&self) -> &'static [SettingsSection] {
        use SettingsSection::*;
        match self.kind {
            ModuleKind::Waterfall => &[Geometry, Frequency, Response, Appearance, Camera],
            ModuleKind::Spectrum => &[Frequency, Response, Appearance],
            ModuleKind::Spectrogram => &[History, Frequency, Response, Appearance],
            ModuleKind::Waveform => &[TimeWindow, Amplitude, Appearance],
        }
    }

    pub fn active_section(&self) -> SettingsSection {
        self.sections()[self.active_sections[self.kind as usize]]
    }

    pub fn select_section(&mut self, section: SettingsSection) {
        if let Some(index) = self
            .sections()
            .iter()
            .position(|candidate| *candidate == section)
        {
            self.active_sections[self.kind as usize] = index;
        }
    }

    pub fn set_channel_view(&mut self, stereo: bool, channel: crate::analysis::ChannelMode) {
        self.waveform.set_channel_view(stereo, channel);
    }

    pub fn ingest(&mut self, frames: &[std::sync::Arc<crate::capture_history::SpectralFrame>]) {
        for frame in frames {
            if frame.sequence <= self.last_capture {
                continue;
            }
            self.last_capture = frame.sequence;
            if self.is_frozen() {
                continue;
            }
            let time = frame.time - self.paused_duration;
            match self.kind {
                ModuleKind::Waterfall => self.waterfall.ingest(frame, time),
                ModuleKind::Spectrogram => self.spectrogram.ingest(frame, time),
                _ => {}
            }
        }
    }

    pub fn draw(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        frame: &AnalysisFrame,
        theme: &AppTheme,
        live: bool,
    ) {
        let now = self.frozen.as_ref().map_or_else(
            || Instant::now() - self.paused_duration,
            |frozen| frozen.time,
        );
        let frame = self.frozen.as_ref().map_or(frame, |frozen| &frozen.frame);
        let live = live || self.frozen.is_some();
        ui.painter().rect_filled(rect, 4.0, theme.background);
        ui.interact(rect, ui.id().with("module-help"), egui::Sense::hover())
            .help_text(self.kind.description());
        match self.kind {
            ModuleKind::Waterfall => self.waterfall.draw(ui, rect, frame, theme, now, false),
            ModuleKind::Spectrum => self.spectrum.draw(ui, rect, frame, theme, now, live),
            ModuleKind::Waveform => self.waveform.draw(ui, rect, frame, theme, live),
            ModuleKind::Spectrogram => self.spectrogram.draw(ui, rect, frame, theme, now, false),
        }
        if self.frozen.is_some() {
            ui.painter().text(
                rect.right_top() + egui::vec2(-6.0, 4.0),
                egui::Align2::RIGHT_TOP,
                "PAUSED",
                FontId::monospace(10.0),
                theme.foreground,
            );
        }
        if !live {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Waiting for audio",
                FontId::proportional(14.0),
                theme.muted,
            );
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TraceStyle {
    Bars,
    Line,
    Filled,
}

// Popups otherwise inherit the application's roomier menu defaults. Keep every
// properties picker consistent and reserve enough height for its complete list.
fn properties_combo(id: &str, width: f32, rows: usize) -> egui::ComboBox {
    egui::ComboBox::from_id_salt(id)
        .width(width)
        .height(rows as f32 * 26.0)
        .truncate()
        .popup_style(egui::style::StyleModifier::new(|style| {
            style.spacing.item_spacing.y = 2.0;
            style.spacing.interact_size.y = 24.0;
            style.spacing.button_padding = egui::vec2(5.0, 2.0);
            style.override_font_id = Some(FontId::proportional(13.0));
        }))
}

impl TraceStyle {
    fn label(self) -> &'static str {
        match self {
            Self::Bars => "Bars",
            Self::Line => "Line",
            Self::Filled => "Filled area",
        }
    }

    fn controls(&mut self, ui: &mut egui::Ui, bars: bool) {
        properties_combo("trace-style", 176.0, 3)
            .selected_text(self.label())
            .show_ui(ui, |ui| {
                for style in [Self::Bars, Self::Line, Self::Filled] {
                    if style == Self::Bars && !bars {
                        continue;
                    }
                    let (rect, response) =
                        ui.allocate_exact_size(egui::vec2(176.0, 24.0), egui::Sense::click());
                    if response.hovered() || *self == style {
                        ui.painter()
                            .rect_filled(rect, 3.0, ui.visuals().selection.bg_fill);
                    }
                    let icon_rect = Rect::from_min_size(rect.min, egui::vec2(24.0, 24.0));
                    crate::icons::paint(
                        ui,
                        icon_rect,
                        match style {
                            Self::Bars => Icon::Bars,
                            Self::Line => Icon::Waveform,
                            Self::Filled => Icon::Surface,
                        },
                        ui.visuals().text_color(),
                    );
                    ui.painter().text(
                        rect.left_center() + egui::vec2(30.0, 0.0),
                        egui::Align2::LEFT_CENTER,
                        style.label(),
                        FontId::proportional(13.0),
                        ui.visuals().text_color(),
                    );
                    if response.clicked() {
                        *self = style;
                        ui.close();
                    }
                    response.help_text(
                        "Choose a trace style. Its relevant appearance controls are shown below.",
                    );
                }
            })
            .response
            .help_text("Choose how the signal is drawn; does not change analysis.");
    }
}

fn trace_mesh(points: &[Pos2], baseline: f32, color: Color32) -> egui::Mesh {
    let mut mesh = egui::Mesh::default();
    for pair in points.windows(2) {
        let base = mesh.vertices.len() as u32;
        // Split at zero crossings: an unsplit quad would self-intersect and
        // double-blend part of a translucent waveform fill.
        if (pair[0].y - baseline) * (pair[1].y - baseline) < 0.0 {
            let t = (baseline - pair[0].y) / (pair[1].y - pair[0].y);
            let crossing = Pos2::new(egui::lerp(pair[0].x..=pair[1].x, t), baseline);
            for p in [
                pair[0],
                crossing,
                Pos2::new(pair[0].x, baseline),
                pair[1],
                Pos2::new(pair[1].x, baseline),
            ] {
                mesh.colored_vertex(p, color);
            }
            mesh.add_triangle(base, base + 1, base + 2);
            mesh.add_triangle(base + 1, base + 3, base + 4);
            continue;
        }
        for p in [
            pair[0],
            pair[1],
            Pos2::new(pair[1].x, baseline),
            Pos2::new(pair[0].x, baseline),
        ] {
            mesh.colored_vertex(p, color);
        }
        mesh.add_triangle(base, base + 1, base + 2);
        mesh.add_triangle(base, base + 2, base + 3);
    }
    mesh
}

fn fill_trace(painter: &egui::Painter, points: &[Pos2], baseline: f32, color: Color32) {
    painter.add(egui::Shape::mesh(trace_mesh(points, baseline, color)));
}

pub(crate) fn settings_panel(ui: &mut egui::Ui, title: &str, body: impl FnOnce(&mut egui::Ui)) {
    let selected =
        ui.data(|data| data.get_temp::<SettingsSection>(ui.id().with("active-module-section")));
    if selected.is_some_and(|section| section.title() != title) {
        return;
    }
    // The toolbar selects the tab; only the groups inside it are collapsible.
    ui.push_id(title, body);
}

/// Compact disclosure rows, with state scoped to the pane, module, and section.
pub(crate) fn settings_group(ui: &mut egui::Ui, title: &str, body: impl FnOnce(&mut egui::Ui)) {
    ui.separator();
    ui.scope(|ui| {
        let widgets = ui.visuals().widgets.clone();
        let accent = ui.visuals().selection.bg_fill;
        let visuals = ui.visuals_mut();
        // egui uses this flag for full-row hit targets as well as painting.
        // Keep the hit target, but make every header state transparent.
        visuals.collapsing_header_frame = true;
        for widget in [
            &mut visuals.widgets.noninteractive,
            &mut visuals.widgets.inactive,
            &mut visuals.widgets.hovered,
            &mut visuals.widgets.active,
            &mut visuals.widgets.open,
        ] {
            widget.weak_bg_fill = Color32::TRANSPARENT;
            widget.bg_stroke = egui::Stroke::NONE;
        }
        visuals.widgets.hovered.fg_stroke.color = accent;
        visuals.widgets.active.fg_stroke.color = accent;
        egui::CollapsingHeader::new(title)
            .default_open(true)
            .show_background(false)
            .show_unindented(ui, |ui| {
                // Fields retain their normal recessed backgrounds.
                ui.visuals_mut().widgets = widgets;
                ui.visuals_mut().collapsing_header_frame = false;
                ui.add_space(2.0);
                body(ui);
                ui.add_space(4.0);
            });
    });
}

fn label(painter: &egui::Painter, position: Pos2, text: impl ToString, color: Color32) {
    painter.text(
        position,
        egui::Align2::LEFT_TOP,
        text,
        FontId::monospace(10.0),
        color,
    );
}

fn mix(a: Color32, b: Color32, amount: f32) -> Color32 {
    let channel = |left: u8, right: u8| {
        (left as f32 + (right as f32 - left as f32) * amount.clamp(0.0, 1.0)) as u8
    };
    Color32::from_rgb(
        channel(a.r(), b.r()),
        channel(a.g(), b.g()),
        channel(a.b(), b.b()),
    )
}

fn frequency_label(hz: f32) -> String {
    if hz >= 1000.0 {
        format!("{:.1}k", hz / 1000.0)
    } else {
        format!("{hz:.0}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_history_panes_catch_up_without_painting_and_manual_pause_still_skips_audio() {
        use std::sync::Arc;
        let mut panes = [
            ModulePane::new(ModuleKind::Spectrogram),
            ModulePane::new(ModuleKind::Waterfall),
        ];
        let start = Instant::now();
        let frames: Vec<_> = (0..240)
            .map(|i| {
                Arc::new(crate::capture_history::SpectralFrame {
                    sequence: i + 1,
                    time: start + Duration::from_millis(i * 34),
                    sample_rate: 48000,
                    fft_size: 512,
                    magnitudes: vec![0.1; 257].into(),
                })
            })
            .collect();
        // No UI frame or draw call occurs during these eight seconds of capture.
        for pane in &mut panes {
            pane.ingest(&frames);
        }
        assert_eq!(panes[0].spectrogram.history_len(), 240);
        assert_eq!(panes[1].waterfall.history_len(), 240);
        let paused_at = start + Duration::from_secs(9);
        panes[0].toggle_freeze(
            &AnalysisFrame {
                sequence: 240,
                ..Default::default()
            },
            paused_at,
        );
        let next = Arc::new(crate::capture_history::SpectralFrame {
            sequence: 241,
            time: paused_at + Duration::from_secs(1),
            sample_rate: 48000,
            fft_size: 512,
            magnitudes: vec![0.2; 257].into(),
        });
        for pane in &mut panes {
            pane.ingest(std::slice::from_ref(&next));
        }
        assert_eq!(panes[0].spectrogram.history_len(), 240);
        assert_eq!(panes[1].waterfall.history_len(), 241);
        panes[0].toggle_freeze(
            &AnalysisFrame {
                sequence: 241,
                ..Default::default()
            },
            paused_at + Duration::from_secs(1),
        );
        panes[0].ingest(&frames);
        assert_eq!(panes[0].spectrogram.history_len(), 240);
    }

    #[test]
    fn settings_clipboard_is_typed_and_does_not_copy_capture_or_pause_state() {
        for kind in ModuleKind::ALL {
            let mut source = ModulePane::new(kind);
            source.toggle_freeze(&AnalysisFrame::default(), Instant::now());
            let settings = source.copy_settings();
            assert_eq!(settings.kind(), kind);
            let mut target = ModulePane::new(kind);
            assert!(target.paste_settings(&settings));
            assert!(target.copy_settings() == settings);
            assert!(!target.is_frozen());
            assert_eq!(target.paused_duration, Duration::ZERO);
            target.kind = if kind == ModuleKind::Waveform {
                ModuleKind::Spectrum
            } else {
                ModuleKind::Waveform
            };
            let before = target.copy_settings();
            assert!(!target.paste_settings(&settings));
            assert!(target.copy_settings() == before);
        }
    }

    #[test]
    fn pause_captures_only_one_pane_and_resume_omits_paused_time() {
        let mut pane = ModulePane::new(ModuleKind::Waveform);
        let other = ModulePane::new(ModuleKind::Waveform);
        let now = Instant::now();
        let mut frame = AnalysisFrame {
            sequence: 42,
            ..AnalysisFrame::default()
        };
        pane.toggle_freeze(&frame, now);
        frame.sequence = 43;
        frame.waveform_left.fill(1.0);
        assert_eq!(pane.frozen.as_ref().unwrap().frame.sequence, 42);
        assert_eq!(pane.frozen.as_ref().unwrap().frame.waveform_left[0], 0.0);
        assert!(!other.is_frozen());
        pane.reset_settings();
        assert!(pane.is_frozen());
        let later = now + Duration::from_secs(90);
        pane.toggle_freeze(&frame, later);
        assert!(!pane.is_frozen());
        assert_eq!(later - pane.paused_duration, now);
        pane.toggle_freeze(&frame, later + Duration::from_secs(1));
        assert_eq!(
            pane.frozen.as_ref().unwrap().time,
            now + Duration::from_secs(1)
        );
        pane.reset();
        assert!(!pane.is_frozen());
        assert_eq!(pane.paused_duration, Duration::ZERO);
    }

    #[test]
    fn filled_waveform_splits_zero_crossings_without_overlapping_triangles() {
        let mesh = trace_mesh(
            &[Pos2::new(0.0, -1.0), Pos2::new(2.0, 1.0)],
            0.0,
            Color32::WHITE,
        );
        let area: f32 = mesh
            .indices
            .as_chunks::<3>()
            .0
            .iter()
            .map(|triangle| {
                let a = mesh.vertices[triangle[0] as usize].pos;
                let b = mesh.vertices[triangle[1] as usize].pos;
                let c = mesh.vertices[triangle[2] as usize].pos;
                ((b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)).abs() * 0.5
            })
            .sum();
        assert_eq!(area, 1.0);
        assert!(mesh.vertices.iter().any(|v| v.pos == Pos2::new(1.0, 0.0)));
    }

    #[test]
    fn frozen_waveform_ignores_new_frames_and_live_status() {
        let context = egui::Context::default();
        let mut pane = ModulePane::new(ModuleKind::Waveform);
        let mut frame = AnalysisFrame::default();
        pane.toggle_freeze(&frame, Instant::now());
        let render = |pane: &mut ModulePane, frame: &AnalysisFrame, live| {
            let mut output = context.run_ui(egui::RawInput::default(), |ui| {
                pane.draw(
                    ui,
                    Rect::from_min_size(Pos2::ZERO, egui::vec2(500.0, 300.0)),
                    frame,
                    &AppTheme::default(),
                    live,
                );
            });
            output.textures_delta.clear();
            context
                .tessellate(output.shapes, output.pixels_per_point)
                .into_iter()
                .flat_map(|p| match p.primitive {
                    egui::epaint::Primitive::Mesh(mesh) => mesh
                        .vertices
                        .into_iter()
                        .map(|v| (v.pos, v.color))
                        .collect(),
                    _ => vec![],
                })
                .collect::<Vec<_>>()
        };
        let before = render(&mut pane, &frame, true);
        frame.waveform_left.fill(1.0);
        frame.waveform_right.fill(-1.0);
        let after = render(&mut pane, &frame, false);
        assert_eq!(before, after);
    }

    #[test]
    fn module_controls_fit_the_compact_sidebar_and_reset_is_always_available() {
        for kind in ModuleKind::ALL {
            let context = egui::Context::default();
            let mut pane = ModulePane::new(kind);
            for &section in pane.sections() {
                pane.select_section(section);
                let mut output = context.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(
                            Pos2::ZERO,
                            egui::vec2(216.0, 1000.0),
                        )),
                        ..Default::default()
                    },
                    |ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                        ui.style_mut()
                            .text_styles
                            .insert(egui::TextStyle::Body, FontId::proportional(13.0));
                        ui.style_mut()
                            .text_styles
                            .insert(egui::TextStyle::Button, FontId::proportional(13.0));
                        pane.controls(ui, &AnalysisFrame::default());
                        assert!(
                            ui.min_rect().right() <= 216.0,
                            "{kind:?}/{section:?} width {}",
                            ui.min_rect().right()
                        );
                    },
                );
                output.textures_delta.clear();
                assert!(output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.job.text == "Reset module settings")));
                let tab_title_count = output.shapes.iter().filter(|shape| matches!(
                    &shape.shape, egui::Shape::Text(text) if text.galley.job.text == section.title()
                )).count();
                // Amplitude is also a numeric field label. No tab gets an
                // additional heading that could collapse all of its groups.
                assert_eq!(
                    tab_title_count,
                    usize::from(section == SettingsSection::Amplitude),
                    "{kind:?}/{section:?} must show its groups directly",
                );
            }
        }
    }

    #[test]
    fn property_groups_remember_collapsed_state_without_affecting_other_panes() {
        let context = egui::Context::default();
        for theme in [egui::Theme::Dark, egui::Theme::Light] {
            context.style_mut_of(theme, |style| style.animation_time = 0.0);
        }
        let mut pane = ModulePane::new(ModuleKind::Waterfall);
        let render = |pane: &mut ModulePane, pane_index, events| {
            let mut output = context.run_ui(
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |ui| {
                    ui.push_id(pane_index, |ui| {
                        pane.controls(ui, &AnalysisFrame::default())
                    });
                },
            );
            output.textures_delta.clear();
            output
        };
        let text_rect = |output: &egui::FullOutput, label: &str| {
            output.shapes.iter().find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.job.text == label => {
                    Some(Rect::from_min_size(text.pos, text.galley.size()))
                }
                _ => None,
            })
        };
        let output = render(&mut pane, 0, vec![]);
        assert!(text_rect(&output, "Geometry").is_none());
        let header = text_rect(&output, "Dimensions").unwrap().center();
        assert!(text_rect(&output, "Length X").is_some());
        for pressed in [true, false] {
            render(
                &mut pane,
                0,
                vec![
                    egui::Event::PointerMoved(header),
                    egui::Event::PointerButton {
                        pos: header,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
        let output = render(&mut pane, 0, vec![]);
        assert!(text_rect(&output, "Length X").is_none());
        assert!(text_rect(&output, "History s").is_some());
        assert!(text_rect(&output, "Reset module settings").is_some());
        pane.select_section(SettingsSection::Frequency);
        render(&mut pane, 0, vec![]);
        pane.select_section(SettingsSection::Geometry);
        assert!(text_rect(&render(&mut pane, 0, vec![]), "Length X").is_none());
        assert!(text_rect(&render(&mut pane, 1, vec![]), "Length X").is_some());
    }

    #[test]
    fn icon_tabs_show_one_section_and_preserve_per_module_and_pane_selection() {
        let context = egui::Context::default();
        let mut pane = ModulePane::new(ModuleKind::Waterfall);
        assert_eq!(pane.active_section(), SettingsSection::Geometry);
        assert_eq!(pane.sections().last(), Some(&SettingsSection::Camera));
        assert!(!pane.sections().contains(&SettingsSection::History));
        pane.select_section(SettingsSection::Camera);
        let render = |pane: &mut ModulePane| {
            let mut output = context.run_ui(egui::RawInput::default(), |ui| {
                pane.controls(ui, &AnalysisFrame::default());
            });
            output.textures_delta.clear();
            output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) => Some(text.galley.job.text.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        let labels = render(&mut pane);
        assert!(labels.iter().any(|label| label == "View presets"));
        assert!(!labels.iter().any(|label| label == "Camera"));
        assert!(!labels.iter().any(|label| label == "History s"));
        pane.select_section(SettingsSection::Geometry);
        let labels = render(&mut pane);
        assert!(labels.iter().any(|label| label == "Length X"));
        assert!(labels.iter().any(|label| label == "History s"));
        assert!(
            !labels
                .iter()
                .any(|label| label == "Camera" || label == "Auto-orbit")
        );
        pane.kind = ModuleKind::Spectrum;
        assert_eq!(pane.active_section(), SettingsSection::Frequency);
        pane.select_section(SettingsSection::Appearance);
        pane.kind = ModuleKind::Waterfall;
        assert_eq!(pane.active_section(), SettingsSection::Geometry);
        pane.kind = ModuleKind::Spectrum;
        assert_eq!(pane.active_section(), SettingsSection::Appearance);
        let other = ModulePane::new(ModuleKind::Spectrum);
        assert_eq!(other.active_section(), SettingsSection::Frequency);
        pane.kind = ModuleKind::Waveform;
        assert_eq!(
            pane.sections(),
            &[
                SettingsSection::TimeWindow,
                SettingsSection::Amplitude,
                SettingsSection::Appearance
            ]
        );
    }

    #[test]
    fn global_channel_view_reaches_every_waveform_pane() {
        let context = egui::Context::default();
        for kind in ModuleKind::ALL {
            let mut pane = ModulePane::new(kind);
            pane.set_channel_view(false, crate::analysis::ChannelMode::StereoMix);
            pane.kind = ModuleKind::Waveform;
            let mut output = context.run_ui(egui::RawInput::default(), |ui| {
                pane.draw(
                    ui,
                    Rect::from_min_size(Pos2::ZERO, egui::vec2(500.0, 300.0)),
                    &AnalysisFrame::default(),
                    &AppTheme::default(),
                    false,
                );
            });
            output.textures_delta.clear();
            assert!(output.shapes.iter().any(|shape| matches!(&shape.shape,
                egui::Shape::Text(text) if text.galley.job.text == "Stereo mix")));
            assert!(!output.shapes.iter().any(|shape| matches!(&shape.shape,
                egui::Shape::Text(text) if text.galley.job.text == "L" || text.galley.job.text == "R")));
        }
    }

    #[test]
    fn heatmap_spans_cool_to_warm_without_theme_tinting() {
        let theme = AppTheme::default();
        let palette = Palette::Heatmap;
        let quiet = palette.color(0.0, &theme);
        let middle = palette.color(0.5, &theme);
        let loud = palette.color(1.0, &theme);
        assert!(quiet.b() > quiet.r() && quiet.b() > quiet.g());
        assert!(middle.g() > middle.r() && middle.g() > middle.b());
        assert!(loud.r() > loud.g() && loud.r() > loud.b());
        assert_eq!(palette.color(-1.0, &theme), quiet);
        assert_eq!(palette.color(2.0, &theme), loud);
        let other_theme = AppTheme {
            background: Color32::WHITE,
            foreground: Color32::BLACK,
            ..theme.clone()
        };
        let mut colors = std::collections::HashSet::new();
        for step in 0..=255 {
            let level = step as f32 / 255.0;
            let color = palette.color(level, &theme);
            colors.insert(color.to_array());
            assert_eq!(color, palette.color(level, &other_theme));
            if step > 0 {
                let previous = palette.color((step - 1) as f32 / 255.0, &theme);
                for (a, b) in color.to_array().iter().zip(previous.to_array()) {
                    assert!(a.abs_diff(b) <= 6, "smooth gradient between color stops");
                }
            }
        }
        assert!(colors.len() > 250);
    }

    #[test]
    fn heatmap_contrast_preserves_endpoints_midpoint_and_other_palettes() {
        let theme = AppTheme::default();
        for contrast in [0.25, 1.0, 3.0] {
            for t in [0.0, 0.5, 1.0] {
                assert_eq!(
                    Palette::Heatmap.color_with_contrast(t, contrast, &theme),
                    Palette::Heatmap.color(t, &theme)
                );
            }
            for palette in [Palette::Theme, Palette::Ember, Palette::Ocean] {
                assert_eq!(
                    palette.color_with_contrast(0.3, contrast, &theme),
                    palette.color(0.3, &theme)
                );
            }
        }
        let cool = Palette::Heatmap.color_with_contrast(0.25, 3.0, &theme);
        let warm = Palette::Heatmap.color_with_contrast(0.75, 3.0, &theme);
        assert!(cool.b() > cool.g());
        assert!(warm.r() > warm.g());
        for step in 0..=100 {
            let t = step as f32 / 100.0;
            assert_eq!(
                Palette::Heatmap.color_with_contrast(t, 1.0, &theme),
                Palette::Heatmap.color(t, &theme)
            );
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Palette {
    #[default]
    Theme,
    Ember,
    Ocean,
    Heatmap,
}

impl Palette {
    fn contrast_controls(self, ui: &mut egui::Ui, contrast: &mut f32) {
        if self == Self::Heatmap {
            ui.add(crate::parameter::Parameter::new(contrast, 0.25..=3.0, 1.0).text("Contrast"))
                .help_text("1 is neutral. Higher contrast separates quiet blues from loud reds; lower contrast brings colors toward the middle. Changes color only, not height, levels, or retained audio.");
        }
    }

    fn color_with_contrast(self, value: f32, contrast: f32, theme: &AppTheme) -> Color32 {
        let value = if self == Self::Heatmap {
            let t = value.clamp(0.0, 1.0);
            let a = t.powf(contrast);
            a / (a + (1.0 - t).powf(contrast))
        } else {
            value
        };
        self.color(value, theme)
    }

    fn controls(&mut self, ui: &mut egui::Ui) {
        properties_combo("palette", 176.0, 4)
            .selected_text(self.label())
            .show_ui(ui, |ui| {
                for value in [Self::Theme, Self::Ember, Self::Ocean, Self::Heatmap] {
                    ui.selectable_value(self, value, value.label()).help_text(
                        if value == Self::Heatmap {
                            "Full-spectrum heatmap: cool blue and cyan for quiet levels, green through yellow and orange to red for loud peaks. Floor and Ceiling set the level range."
                        } else {
                            "Choose the colors used to represent quiet and loud signal levels."
                        },
                    );
                }
            })
            .response
            .help_text("Choose a signal color palette: current theme, Ember, Ocean, or a full-spectrum Heatmap from cool quiet levels to warm loud peaks.");
    }

    fn label(self) -> &'static str {
        match self {
            Self::Theme => "Theme colors",
            Self::Ember => "Ember",
            Self::Ocean => "Ocean",
            Self::Heatmap => "Heatmap",
        }
    }

    fn color(self, value: f32, theme: &AppTheme) -> Color32 {
        let (low, high) = match self {
            Self::Heatmap => {
                // Map the same normalized level used for waterfall height to a
                // full cool-to-warm spectrum, independent of the desktop theme.
                const STOPS: [Color32; 7] = [
                    Color32::from_rgb(32, 40, 160),
                    Color32::from_rgb(36, 100, 240),
                    Color32::from_rgb(0, 200, 230),
                    Color32::from_rgb(45, 205, 115),
                    Color32::from_rgb(245, 224, 55),
                    Color32::from_rgb(255, 140, 35),
                    Color32::from_rgb(240, 45, 35),
                ];
                let position = value.clamp(0.0, 1.0) * (STOPS.len() - 1) as f32;
                let index = (position as usize).min(STOPS.len() - 2);
                return mix(STOPS[index], STOPS[index + 1], position - index as f32);
            }
            Self::Theme => (theme.accent_alt, theme.accent),
            Self::Ember => (
                Color32::from_rgb(140, 44, 70),
                Color32::from_rgb(255, 181, 70),
            ),
            Self::Ocean => (
                Color32::from_rgb(45, 72, 172),
                Color32::from_rgb(80, 225, 214),
            ),
        };
        if value < 0.4 {
            mix(theme.background, low, value / 0.4)
        } else if value < 0.85 {
            mix(low, high, (value - 0.4) / 0.45)
        } else {
            mix(high, theme.foreground, (value - 0.85) / 0.15)
        }
    }
}
