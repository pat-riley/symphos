use std::time::{Duration, Instant};

use eframe::egui::{
    self, Align, Color32, FontId, Layout, Pos2, Rect, RichText, Sense, Stroke, Vec2,
};

use crate::analysis::{AnalysisFrame, ChannelMode, FFT_SIZES, WindowFunction};
use crate::audio::{AudioEngine, AudioEvent, AudioSource, AudioStatus};
use crate::theme::AppTheme;

const SPECTROGRAM_WIDTH: usize = 180;
const SPECTROGRAM_HEIGHT: usize = 96;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SpectrumScale {
    Logarithmic,
    Linear,
}

impl SpectrumScale {
    const ALL: [Self; 2] = [Self::Logarithmic, Self::Linear];

    const fn label(self) -> &'static str {
        match self {
            Self::Logarithmic => "Logarithmic",
            Self::Linear => "Linear",
        }
    }
}

pub struct SymphosApp {
    engine: AudioEngine,
    sources: Vec<AudioSource>,
    selected_node: Option<String>,
    status: AudioStatus,
    theme: AppTheme,
    spectrogram_pixels: Vec<Color32>,
    spectrogram_cursor: usize,
    spectrogram_texture: Option<egui::TextureHandle>,
    last_sequence: u64,
    fullscreen: bool,
    show_inspector: bool,
    spectrum_scale: SpectrumScale,
    display_fps: f32,
    last_frame: Instant,
}

impl SymphosApp {
    pub fn new(context: &eframe::CreationContext<'_>) -> Self {
        let theme = AppTheme::load_omarchy();
        apply_style(&context.egui_ctx, &theme);
        let engine = AudioEngine::start();
        engine.refresh_sources();
        Self {
            engine,
            sources: Vec::new(),
            selected_node: None,
            status: AudioStatus::Idle,
            theme,
            spectrogram_pixels: vec![Color32::BLACK; SPECTROGRAM_WIDTH * SPECTROGRAM_HEIGHT],
            spectrogram_cursor: 0,
            spectrogram_texture: None,
            last_sequence: 0,
            fullscreen: false,
            show_inspector: false,
            spectrum_scale: SpectrumScale::Logarithmic,
            display_fps: 0.0,
            last_frame: Instant::now(),
        }
    }

    fn process_audio_events(&mut self) {
        let events: Vec<_> = self.engine.drain_events().collect();
        for event in events {
            match event {
                AudioEvent::Sources(sources) => {
                    self.sources = sources;
                }
                AudioEvent::Status(status) => self.status = status,
            }
        }
    }

    fn update_spectrogram(&mut self, context: &egui::Context, frame: &AnalysisFrame) {
        if frame.sequence == self.last_sequence || frame.log_bands.is_empty() {
            return;
        }
        self.last_sequence = frame.sequence;
        let column = self.spectrogram_cursor;
        for row in 0..SPECTROGRAM_HEIGHT {
            let band_index = SPECTROGRAM_HEIGHT - 1 - row;
            let db = frame
                .log_bands
                .get(band_index)
                .map_or(-120.0, |band| band.dbfs);
            self.spectrogram_pixels[row * SPECTROGRAM_WIDTH + column] = heat_color(db, &self.theme);
        }
        self.spectrogram_cursor = (self.spectrogram_cursor + 1) % SPECTROGRAM_WIDTH;
        let image = egui::ColorImage::new(
            [SPECTROGRAM_WIDTH, SPECTROGRAM_HEIGHT],
            self.spectrogram_pixels.clone(),
        );
        match &mut self.spectrogram_texture {
            Some(texture) => texture.set(image, egui::TextureOptions::LINEAR),
            None => {
                self.spectrogram_texture = Some(context.load_texture(
                    "symphos-spectrogram",
                    image,
                    egui::TextureOptions::LINEAR,
                ));
            }
        }
    }

    fn source_selector(&mut self, ui: &mut egui::Ui) {
        let selected_label = self
            .sources
            .iter()
            .find(|source| Some(&source.node_name) == self.selected_node.as_ref())
            .map(|source| format!("{} · {}", source.kind.label(), source.display_name))
            .unwrap_or_else(|| {
                if self.sources.is_empty() {
                    "Looking for PipeWire sources…".into()
                } else {
                    "Select an audio source…".into()
                }
            });
        egui::ComboBox::from_id_salt("audio-source")
            .width(310.0)
            .selected_text(selected_label)
            .show_ui(ui, |ui| {
                for source in &self.sources {
                    let selected = Some(&source.node_name) == self.selected_node.as_ref();
                    let label = format!("{}  {}", source.kind.label(), source.display_name);
                    if ui.selectable_label(selected, label).clicked() {
                        self.selected_node = Some(source.node_name.clone());
                        self.engine.select_source(source.clone());
                    }
                }
            });
    }

    fn controls(&mut self, ui: &mut egui::Ui, frame: &AnalysisFrame) {
        ui.set_width(280.0);
        ui.spacing_mut().slider_width = 180.0;
        ui.label(RichText::new("ANALYSIS").small().color(self.theme.muted));
        ui.add_space(8.0);
        stereo_meters(ui, frame, &self.theme);
        ui.add_space(12.0);
        let mut settings = self
            .engine
            .analysis
            .settings
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();

        control_label(
            ui,
            "FFT size",
            "Higher values improve frequency resolution and add latency.",
            self.theme.muted,
        );
        egui::ComboBox::from_id_salt("fft-size")
            .width(ui.available_width())
            .selected_text(format!("{} samples", settings.fft_size))
            .show_ui(ui, |ui| {
                for size in FFT_SIZES {
                    ui.selectable_value(&mut settings.fft_size, size, format!("{size} samples"));
                }
            });
        ui.add_space(10.0);

        control_label(
            ui,
            "Spectrum scale",
            "Switch between perceptual logarithmic bands and linear FFT frequency spacing.",
            self.theme.muted,
        );
        egui::ComboBox::from_id_salt("spectrum-scale")
            .width(ui.available_width())
            .selected_text(self.spectrum_scale.label())
            .show_ui(ui, |ui| {
                for scale in SpectrumScale::ALL {
                    ui.selectable_value(&mut self.spectrum_scale, scale, scale.label());
                }
            });
        ui.add_space(10.0);

        control_label(
            ui,
            "Window",
            "Controls FFT leakage and frequency separation.",
            self.theme.muted,
        );
        egui::ComboBox::from_id_salt("window")
            .width(ui.available_width())
            .selected_text(settings.window.label())
            .show_ui(ui, |ui| {
                for window in WindowFunction::ALL {
                    ui.selectable_value(&mut settings.window, window, window.label());
                }
            });
        ui.add_space(10.0);

        control_label(
            ui,
            "Channel",
            "Signal used for spectrum-derived metrics.",
            self.theme.muted,
        );
        egui::ComboBox::from_id_salt("channel")
            .width(ui.available_width())
            .selected_text(settings.channel_mode.label())
            .show_ui(ui, |ui| {
                for channel in ChannelMode::ALL {
                    ui.selectable_value(&mut settings.channel_mode, channel, channel.label());
                }
            });
        ui.add_space(10.0);

        slider(ui, &mut settings.smoothing, 0.0..=0.98, "Smoothing", "");
        slider(
            ui,
            &mut settings.gain_db,
            -24.0..=36.0,
            "Display gain",
            " dB",
        );
        slider(
            ui,
            &mut settings.decay_db_per_second,
            6.0..=96.0,
            "Decay",
            " dB/s",
        );
        ui.add_space(8.0);
        control_label(
            ui,
            "Frequency range",
            "Log-spectrum and metric analysis bounds.",
            self.theme.muted,
        );
        ui.label("Minimum frequency");
        ui.add(
            egui::Slider::new(&mut settings.min_frequency, 10.0..=500.0)
                .logarithmic(true)
                .suffix(" Hz"),
        );
        ui.label("Maximum frequency");
        ui.add(
            egui::Slider::new(&mut settings.max_frequency, 2_000.0..=24_000.0)
                .logarithmic(true)
                .suffix(" Hz"),
        );
        if settings.min_frequency >= settings.max_frequency {
            settings.max_frequency = (settings.min_frequency * 2.0).min(24_000.0);
        }
        ui.add_space(8.0);
        control_label(
            ui,
            "Frame rate",
            "Target analyzer snapshots and UI refreshes per second.",
            self.theme.muted,
        );
        egui::ComboBox::from_id_salt("analysis-fps")
            .width(ui.available_width())
            .selected_text(format!("{} Hz", settings.analysis_fps))
            .show_ui(ui, |ui| {
                for rate in [30, 60, 90, 120] {
                    ui.selectable_value(&mut settings.analysis_fps, rate, format!("{rate} Hz"));
                }
            });

        *self
            .engine
            .analysis
            .settings
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = settings;

        ui.add_space(18.0);
        ui.separator();
        ui.add_space(10.0);
        if ui.button("Refresh audio sources").clicked() {
            self.engine.refresh_sources();
        }
        if ui
            .button(if self.fullscreen {
                "Exit fullscreen  F11"
            } else {
                "Fullscreen  F11"
            })
            .clicked()
        {
            self.toggle_fullscreen(ui.ctx());
        }
        ui.checkbox(&mut self.show_inspector, "Raw FFT-bin inspector");
        ui.add_space(16.0);
        ui.vertical(|ui| {
            ui.label(
                RichText::new(format!("Omarchy theme · {}", self.theme.name))
                    .small()
                    .color(self.theme.muted),
            );
        });
    }

    fn toggle_fullscreen(&mut self, context: &egui::Context) {
        self.fullscreen = !self.fullscreen;
        context.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.fullscreen));
    }
}

impl eframe::App for SymphosApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.process_audio_events();
        if ui.input(|input| input.key_pressed(egui::Key::F11)) {
            self.toggle_fullscreen(ui.ctx());
        }

        let now = Instant::now();
        let elapsed = now
            .duration_since(self.last_frame)
            .as_secs_f32()
            .max(1.0e-4);
        self.last_frame = now;
        self.display_fps += (elapsed.recip() - self.display_fps) * 0.08;
        let snapshot = self.engine.analysis.snapshot.load_full();
        self.update_spectrogram(ui.ctx(), &snapshot);

        egui::Frame::new()
            .fill(self.theme.background)
            .inner_margin(egui::Margin::same(18))
            .show(ui, |ui| {
                ui.set_min_size(ui.available_size());
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("SYMPHOS")
                            .size(22.0)
                            .strong()
                            .color(self.theme.foreground),
                    );
                    ui.label(RichText::new("ANALYZER").small().color(self.theme.accent));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("{:>3.0} FPS", self.display_fps))
                                .monospace()
                                .small()
                                .color(self.theme.muted),
                        );
                        status_badge(ui, &self.status, &self.theme);
                    });
                });
                ui.add_space(10.0);
                self.source_selector(ui);
                ui.add_space(14.0);
                ui.separator();
                ui.add_space(14.0);

                ui.horizontal_top(|ui| {
                    egui::Frame::new()
                        .fill(self.theme.panel)
                        .corner_radius(10)
                        .inner_margin(egui::Margin::same(14))
                        .show(ui, |ui| {
                            ui.with_layout(Layout::top_down(Align::LEFT), |ui| {
                                ui.set_width(296.0);
                                egui::ScrollArea::vertical()
                                    .id_salt("analysis-controls")
                                    .max_height(ui.available_height())
                                    .auto_shrink([false, false])
                                    .show(ui, |ui| self.controls(ui, &snapshot));
                            });
                        });
                    ui.add_space(12.0);
                    egui::ScrollArea::vertical()
                        .id_salt("analyzer-content")
                        .show(ui, |ui| {
                            ui.with_layout(Layout::top_down(Align::LEFT), |ui| {
                                spectrum_panel(ui, &snapshot, self.spectrum_scale, &self.theme);
                                ui.add_space(10.0);
                                ui.columns(2, |columns| {
                                    waveform_panel(&mut columns[0], &snapshot, &self.theme);
                                    self.spectrogram_panel(&mut columns[1], &snapshot);
                                });
                                ui.add_space(10.0);
                                metrics_panel(ui, &snapshot, &self.theme);
                                if self.show_inspector {
                                    ui.add_space(10.0);
                                    raw_inspector(ui, &snapshot, &self.theme);
                                }
                            });
                        });
                });
            });

        let target_rate = self
            .engine
            .analysis
            .settings
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .analysis_fps;
        ui.ctx()
            .request_repaint_after(Duration::from_secs_f32(1.0 / target_rate.max(1) as f32));
    }
}

impl SymphosApp {
    fn spectrogram_panel(&self, ui: &mut egui::Ui, frame: &AnalysisFrame) {
        panel(
            ui,
            "SPECTROGRAM",
            "Frequency over time",
            &self.theme,
            |ui| {
                let size = Vec2::new(ui.available_width(), 176.0);
                let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
                let painter = ui.painter_at(rect);
                painter.rect_filled(rect, 5.0, self.theme.background);
                if let Some(texture) = &self.spectrogram_texture {
                    let cursor = self.spectrogram_cursor as f32 / SPECTROGRAM_WIDTH as f32;
                    if cursor <= f32::EPSILON {
                        painter.image(
                            texture.id(),
                            rect,
                            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                            Color32::WHITE,
                        );
                    } else {
                        let split = rect.left() + rect.width() * (1.0 - cursor);
                        painter.image(
                            texture.id(),
                            Rect::from_min_max(rect.min, Pos2::new(split, rect.bottom())),
                            Rect::from_min_max(Pos2::new(cursor, 0.0), Pos2::new(1.0, 1.0)),
                            Color32::WHITE,
                        );
                        painter.image(
                            texture.id(),
                            Rect::from_min_max(Pos2::new(split, rect.top()), rect.max),
                            Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(cursor, 1.0)),
                            Color32::WHITE,
                        );
                    }
                }
                axis_label(
                    &painter,
                    rect.left_top() + Vec2::new(6.0, 5.0),
                    format_frequency(frame.log_bands.last().map_or(20_000.0, |b| b.center_hz)),
                    self.theme.muted,
                );
                axis_label(
                    &painter,
                    rect.left_bottom() + Vec2::new(6.0, -17.0),
                    format_frequency(frame.log_bands.first().map_or(20.0, |b| b.center_hz)),
                    self.theme.muted,
                );
            },
        );
    }
}

fn apply_style(context: &egui::Context, theme: &AppTheme) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = theme.background;
    visuals.window_fill = theme.panel;
    visuals.extreme_bg_color = theme.background;
    visuals.faint_bg_color = theme.card;
    visuals.selection.bg_fill = theme.accent;
    visuals.selection.stroke = Stroke::new(1.0, theme.foreground);
    visuals.widgets.noninteractive.bg_fill = theme.card;
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, theme.foreground);
    visuals.widgets.inactive.bg_fill = theme.card;
    visuals.widgets.hovered.bg_fill = mix(theme.card, theme.accent, 0.18);
    visuals.widgets.active.bg_fill = mix(theme.card, theme.accent, 0.28);
    context.set_visuals(visuals);
    context.style_mut_of(egui::Theme::Dark, |style| {
        style.spacing.item_spacing = Vec2::new(10.0, 9.0);
        style.spacing.button_padding = Vec2::new(12.0, 8.0);
        style.spacing.interact_size.y = 32.0;
        style
            .text_styles
            .insert(egui::TextStyle::Body, FontId::proportional(15.0));
        style
            .text_styles
            .insert(egui::TextStyle::Button, FontId::proportional(15.0));
        style
            .text_styles
            .insert(egui::TextStyle::Small, FontId::proportional(13.0));
    });
}

fn panel(
    ui: &mut egui::Ui,
    title: &str,
    subtitle: &str,
    theme: &AppTheme,
    body: impl FnOnce(&mut egui::Ui),
) {
    egui::Frame::new()
        .fill(theme.panel)
        .corner_radius(10)
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(title)
                        .small()
                        .strong()
                        .color(theme.foreground),
                );
                ui.label(RichText::new(subtitle).small().color(theme.muted));
            });
            ui.add_space(8.0);
            body(ui);
        });
}

fn spectrum_panel(
    ui: &mut egui::Ui,
    frame: &AnalysisFrame,
    scale: SpectrumScale,
    theme: &AppTheme,
) {
    panel(
        ui,
        "FREQUENCY SPECTRUM",
        match scale {
            SpectrumScale::Logarithmic => "Logarithmic · dBFS",
            SpectrumScale::Linear => "Linear · dBFS",
        },
        theme,
        |ui| {
            let size = Vec2::new(ui.available_width(), 268.0);
            let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 5.0, theme.background);
            for db in [-120.0, -90.0, -60.0, -30.0, 0.0] {
                let y = remap(db, -120.0, 0.0, rect.bottom(), rect.top());
                painter.line_segment(
                    [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
                    Stroke::new(1.0, mix(theme.muted, theme.background, 0.6)),
                );
                axis_label(
                    &painter,
                    Pos2::new(rect.left() + 5.0, y + 3.0),
                    format!("{db:.0}"),
                    theme.muted,
                );
            }
            let points = spectrum_points(frame, scale, rect.width());
            let count = points.len().max(1);
            let gap = 1.5;
            let width = (rect.width() / count as f32 - gap).max(1.0);
            for (index, (_, db)) in points.iter().enumerate() {
                let x = rect.left() + index as f32 * rect.width() / count as f32;
                let top = remap(
                    db.clamp(-120.0, 0.0),
                    -120.0,
                    0.0,
                    rect.bottom(),
                    rect.top(),
                );
                let intensity = ((db + 90.0) / 90.0).clamp(0.0, 1.0);
                let color = mix(theme.accent_alt, theme.accent, intensity);
                painter.rect_filled(
                    Rect::from_min_max(
                        Pos2::new(x, top),
                        Pos2::new((x + width).min(rect.right()), rect.bottom()),
                    ),
                    1.5,
                    color,
                );
            }
            match scale {
                SpectrumScale::Logarithmic => {
                    for frequency in [
                        20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10_000.0, 20_000.0,
                    ] {
                        let x = log_x(frequency, frame, rect);
                        if rect.contains(Pos2::new(x, rect.center().y)) {
                            axis_label(
                                &painter,
                                Pos2::new(x + 3.0, rect.bottom() - 17.0),
                                format_frequency(frequency),
                                theme.muted,
                            );
                        }
                    }
                }
                SpectrumScale::Linear => {
                    let maximum = points.last().map_or(20_000.0, |point| point.0);
                    for step in 0..=4 {
                        let frequency = maximum * step as f32 / 4.0;
                        let x = rect.left() + rect.width() * step as f32 / 4.0;
                        axis_label(
                            &painter,
                            Pos2::new(x + 3.0, rect.bottom() - 17.0),
                            format_frequency(frequency),
                            theme.muted,
                        );
                    }
                }
            }
            if let Some(pointer) = response.hover_pos() {
                let index = (((pointer.x - rect.left()) / rect.width()) * count as f32)
                    .floor()
                    .clamp(0.0, count.saturating_sub(1) as f32)
                    as usize;
                if let Some((frequency, db)) = points.get(index) {
                    let text = format!("{}  {:+.1} dBFS", format_frequency(*frequency), db);
                    painter.text(
                        pointer + Vec2::new(10.0, -12.0),
                        egui::Align2::LEFT_BOTTOM,
                        text,
                        FontId::monospace(11.0),
                        theme.foreground,
                    );
                }
            }
        },
    );
}

fn waveform_panel(ui: &mut egui::Ui, frame: &AnalysisFrame, theme: &AppTheme) {
    panel(ui, "WAVEFORM", "Left + right", theme, |ui| {
        let size = Vec2::new(ui.available_width(), 176.0);
        let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 5.0, theme.background);
        painter.line_segment(
            [
                Pos2::new(rect.left(), rect.center().y),
                Pos2::new(rect.right(), rect.center().y),
            ],
            Stroke::new(1.0, mix(theme.muted, theme.background, 0.5)),
        );
        draw_wave(&painter, rect, &frame.waveform_left, theme.accent, 0.46);
        draw_wave(
            &painter,
            rect,
            &frame.waveform_right,
            theme.accent_alt,
            0.46,
        );
        axis_label(
            &painter,
            rect.left_top() + Vec2::new(6.0, 5.0),
            "L",
            theme.accent,
        );
        axis_label(
            &painter,
            rect.left_top() + Vec2::new(22.0, 5.0),
            "R",
            theme.accent_alt,
        );
    });
}

fn stereo_meters(ui: &mut egui::Ui, frame: &AnalysisFrame, theme: &AppTheme) {
    ui.label(RichText::new("STEREO LEVEL").small().color(theme.muted));
    for (label, rms, peak, color) in [
        ("L", frame.rms[0], frame.peak[0], theme.accent),
        ("R", frame.rms[1], frame.peak[1], theme.accent_alt),
    ] {
        ui.horizontal(|ui| {
            ui.label(RichText::new(label).monospace().small().color(color));
            let (rect, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 7.0), Sense::hover());
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 3.5, theme.background);
            let rms_fraction = ((amplitude_db(rms) + 60.0) / 60.0).clamp(0.0, 1.0);
            painter.rect_filled(
                Rect::from_min_max(
                    rect.min,
                    Pos2::new(rect.left() + rect.width() * rms_fraction, rect.bottom()),
                ),
                3.5,
                color,
            );
            let peak_fraction = ((amplitude_db(peak) + 60.0) / 60.0).clamp(0.0, 1.0);
            let peak_x = rect.left() + rect.width() * peak_fraction;
            painter.line_segment(
                [
                    Pos2::new(peak_x, rect.top() - 1.0),
                    Pos2::new(peak_x, rect.bottom() + 1.0),
                ],
                Stroke::new(1.0, theme.foreground),
            );
        });
    }
}

fn metrics_panel(ui: &mut egui::Ui, frame: &AnalysisFrame, theme: &AppTheme) {
    let values = [
        (
            "DOMINANT",
            format_frequency(frame.dominant_frequency_hz),
            frame.dominant_note.clone(),
        ),
        (
            "LEVEL L",
            format!("{:.1} dB", amplitude_db(frame.rms[0])),
            format!("peak {:.1}", amplitude_db(frame.peak[0])),
        ),
        (
            "LEVEL R",
            format!("{:.1} dB", amplitude_db(frame.rms[1])),
            format!("peak {:.1}", amplitude_db(frame.peak[1])),
        ),
        (
            "CENTROID",
            format_frequency(frame.spectral_centroid_hz),
            "spectral center".into(),
        ),
        (
            "ROLLOFF 85%",
            format_frequency(frame.spectral_rolloff_hz),
            "energy boundary".into(),
        ),
        (
            "FLATNESS",
            format!("{:.3}", frame.spectral_flatness),
            "tone ↔ noise".into(),
        ),
        (
            "CREST",
            format!("{:.1} dB", frame.crest_factor_db),
            "peak / RMS".into(),
        ),
        (
            "TEMPO",
            if frame.bpm > 0.0 {
                format!("{:.0} BPM", frame.bpm)
            } else {
                "Learning…".into()
            },
            format!("{:.0}% confidence", frame.bpm_confidence * 100.0),
        ),
        (
            "ZERO CROSSING",
            format!("{:.3}", frame.zero_crossing_rate),
            "crossings / sample".into(),
        ),
        (
            "RESOLUTION",
            format!("{:.2} Hz", frame.sample_rate as f32 / frame.fft_size as f32),
            "per FFT bin".into(),
        ),
    ];
    ui.columns(4, |columns| {
        for (index, (label, value, detail)) in values.iter().enumerate() {
            metric_card(&mut columns[index % 4], label, value, detail, theme);
            if index % 4 == 3 && index + 1 < values.len() {
                for column in columns.iter_mut() {
                    column.add_space(8.0);
                }
            }
        }
    });
    ui.add_space(7.0);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!(
                "{} Hz · {} ch · FFT {} · {:.1} ms window",
                frame.sample_rate, frame.channels, frame.fft_size, frame.latency_ms
            ))
            .monospace()
            .small()
            .color(theme.muted),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let color = if frame.dropped_samples == 0 {
                theme.muted
            } else {
                theme.warning
            };
            ui.label(
                RichText::new(format!("{} dropped samples", frame.dropped_samples))
                    .monospace()
                    .small()
                    .color(color),
            );
        });
    });
}

fn metric_card(ui: &mut egui::Ui, label: &str, value: &str, detail: &str, theme: &AppTheme) {
    egui::Frame::new()
        .fill(theme.card)
        .corner_radius(8)
        .inner_margin(egui::Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(RichText::new(label).small().color(theme.muted));
            ui.label(
                RichText::new(value)
                    .size(18.0)
                    .monospace()
                    .color(theme.foreground),
            );
            ui.label(RichText::new(detail).small().color(theme.accent_alt));
        });
}

fn raw_inspector(ui: &mut egui::Ui, frame: &AnalysisFrame, theme: &AppTheme) {
    panel(
        ui,
        "RAW FFT BINS",
        "Linear frequency resolution",
        theme,
        |ui| {
            egui::ScrollArea::vertical()
                .max_height(260.0)
                .show(ui, |ui| {
                    egui::Grid::new("raw-bins")
                        .striped(true)
                        .num_columns(4)
                        .show(ui, |ui| {
                            ui.strong("Bin");
                            ui.strong("Frequency");
                            ui.strong("Magnitude");
                            ui.strong("dBFS");
                            ui.end_row();
                            for (index, bin) in frame.bins.iter().enumerate() {
                                ui.monospace(index.to_string());
                                ui.monospace(format!("{:.2} Hz", bin.frequency_hz));
                                ui.monospace(format!("{:.7}", bin.magnitude));
                                ui.monospace(format!("{:.2}", bin.dbfs));
                                ui.end_row();
                            }
                        });
                });
        },
    );
}

fn status_badge(ui: &mut egui::Ui, status: &AudioStatus, theme: &AppTheme) {
    let color = match status {
        AudioStatus::Streaming => theme.accent,
        AudioStatus::Connecting | AudioStatus::Paused => theme.warning,
        AudioStatus::Error(_) => theme.error,
        AudioStatus::Idle => theme.muted,
    };
    egui::Frame::new()
        .fill(mix(theme.background, color, 0.16))
        .corner_radius(12)
        .inner_margin(egui::Margin::symmetric(9, 4))
        .show(ui, |ui| {
            ui.label(
                RichText::new(format!("●  {}", status.label()))
                    .small()
                    .color(color),
            );
        });
}

fn control_label(ui: &mut egui::Ui, label: &str, hover: &str, color: Color32) {
    ui.label(RichText::new(label).small().color(color))
        .on_hover_text(hover);
}

fn slider(
    ui: &mut egui::Ui,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    label: &str,
    suffix: &str,
) {
    ui.label(label);
    ui.add(egui::Slider::new(value, range).suffix(suffix));
    ui.add_space(6.0);
}

fn draw_wave(painter: &egui::Painter, rect: Rect, samples: &[f32], color: Color32, scale: f32) {
    if samples.len() < 2 {
        return;
    }
    let points: Vec<_> = samples
        .iter()
        .enumerate()
        .map(|(index, sample)| {
            Pos2::new(
                rect.left() + index as f32 * rect.width() / (samples.len() - 1) as f32,
                rect.center().y - sample.clamp(-1.0, 1.0) * rect.height() * scale,
            )
        })
        .collect();
    painter.add(egui::Shape::line(points, Stroke::new(1.35, color)));
}

fn axis_label(painter: &egui::Painter, position: Pos2, text: impl ToString, color: Color32) {
    painter.text(
        position,
        egui::Align2::LEFT_TOP,
        text,
        FontId::monospace(10.0),
        color,
    );
}

fn spectrum_points(
    frame: &AnalysisFrame,
    scale: SpectrumScale,
    available_width: f32,
) -> Vec<(f32, f32)> {
    match scale {
        SpectrumScale::Logarithmic => frame
            .log_bands
            .iter()
            .map(|band| (band.center_hz, band.dbfs))
            .collect(),
        SpectrumScale::Linear => {
            let minimum = frame.log_bands.first().map_or(20.0, |band| band.center_hz);
            let maximum = frame
                .log_bands
                .last()
                .map_or(frame.sample_rate as f32 * 0.5, |band| band.center_hz)
                .min(frame.sample_rate as f32 * 0.5);
            let active: Vec<_> = frame
                .bins
                .iter()
                .filter(|bin| bin.frequency_hz >= minimum && bin.frequency_hz <= maximum)
                .collect();
            if active.is_empty() {
                return Vec::new();
            }
            let group_count = ((available_width / 3.0) as usize)
                .clamp(32, 256)
                .min(active.len());
            (0..group_count)
                .map(|group| {
                    let start = group * active.len() / group_count;
                    let end = ((group + 1) * active.len() / group_count).max(start + 1);
                    let slice = &active[start..end.min(active.len())];
                    let frequency =
                        slice.iter().map(|bin| bin.frequency_hz).sum::<f32>() / slice.len() as f32;
                    let db = slice.iter().map(|bin| bin.dbfs).fold(-120.0_f32, f32::max);
                    (frequency, db)
                })
                .collect()
        }
    }
}

fn log_x(frequency: f32, frame: &AnalysisFrame, rect: Rect) -> f32 {
    let min = frame
        .log_bands
        .first()
        .map_or(20.0, |band| band.center_hz)
        .max(1.0);
    let max = frame
        .log_bands
        .last()
        .map_or(20_000.0, |band| band.center_hz)
        .max(min + 1.0);
    let normalized = ((frequency.max(min).ln() - min.ln()) / (max.ln() - min.ln())).clamp(0.0, 1.0);
    rect.left() + rect.width() * normalized
}

fn remap(value: f32, from_min: f32, from_max: f32, to_min: f32, to_max: f32) -> f32 {
    to_min + (value - from_min) / (from_max - from_min) * (to_max - to_min)
}

fn amplitude_db(amplitude: f32) -> f32 {
    20.0 * amplitude.max(1.0e-7).log10()
}

fn format_frequency(frequency: f32) -> String {
    if frequency >= 1000.0 {
        format!("{:.2} kHz", frequency / 1000.0)
    } else if frequency > 0.0 {
        format!("{frequency:.1} Hz")
    } else {
        "—".into()
    }
}

fn heat_color(db: f32, theme: &AppTheme) -> Color32 {
    let value = ((db + 100.0) / 100.0).clamp(0.0, 1.0);
    if value < 0.45 {
        mix(theme.background, theme.accent_alt, value / 0.45)
    } else if value < 0.82 {
        mix(theme.accent_alt, theme.accent, (value - 0.45) / 0.37)
    } else {
        mix(theme.accent, theme.foreground, (value - 0.82) / 0.18)
    }
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
