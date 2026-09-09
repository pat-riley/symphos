use std::time::Instant;

use eframe::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};

use super::{
    ModuleRenderState, Palette,
    frequency::{BANDS, FrequencySettings, History},
    label, mix, settings_panel,
};
use crate::help::HoverHelp;
use crate::{analysis::AnalysisFrame, theme::AppTheme};

pub struct Spectrogram {
    history: History,
    seconds: f32,
    palette: Palette,
    contrast: f32,
    vertical: bool,
    smooth: bool,
    time_pixels: usize,
    frequency_pixels: usize,
    grid: bool,
    labels: bool,
    texture: Option<egui::TextureHandle>,
    raster_key: Option<RasterKey>,
    pinned: Option<SpectrogramPin>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct SpectrogramPin {
    frequency_hz: f32,
    age_seconds: f32,
    dbfs: Option<f32>,
}

#[derive(PartialEq)]
struct RasterKey {
    newest: Option<(u64, Instant)>,
    oldest: Option<Instant>,
    time_cell: u64,
    seconds: f32,
    time_pixels: usize,
    frequency_pixels: usize,
    vertical: bool,
    smooth: bool,
    palette: Palette,
    contrast: f32,
    settings: FrequencySettings,
    theme: AppTheme,
}

module_settings!(Spectrogram, SpectrogramSettings, {
frequency: super::frequency::FrequencySettings => history.data.settings,
seconds: f32 => seconds,
palette: Palette => palette,
contrast: f32 => contrast,
vertical: bool => vertical,
smooth: bool => smooth,
time_pixels: usize => time_pixels,
frequency_pixels: usize => frequency_pixels,
grid: bool => grid,
labels: bool => labels,
});

impl Default for Spectrogram {
    fn default() -> Self {
        Self {
            history: History::default(),
            seconds: 8.0,
            palette: Palette::default(),
            contrast: 1.0,
            vertical: false,
            smooth: true,
            time_pixels: 240,
            frequency_pixels: BANDS,
            grid: false,
            labels: true,
            texture: None,
            raster_key: None,
            pinned: None,
        }
    }
}

impl Spectrogram {
    pub fn ingest(&mut self, frame: &crate::capture_history::SpectralFrame, now: Instant) {
        self.history.ingest(frame, now);
    }
    #[cfg(test)]
    pub fn history_len(&self) -> usize {
        self.history.rows.len()
    }
    pub fn clear(&mut self) {
        self.history.clear();
        self.texture = None;
        self.raster_key = None;
        self.pinned = None;
    }

    pub fn clear_cursor(&mut self) {
        self.pinned = None;
    }

    fn measurement_at(&self, x: f32, y: f32, now: Instant, sample_rate: u32) -> SpectrogramPin {
        let x = x.clamp(0.0, 1.0);
        let y = y.clamp(0.0, 1.0);
        let (frequency, time) = if self.vertical { (x, y) } else { (1.0 - y, x) };
        let age_seconds = self.seconds * (1.0 - time);
        let frequency_pixels = self.frequency_pixels.clamp(1, BANDS);
        let bin = ((frequency * frequency_pixels as f32) as usize).min(frequency_pixels - 1);
        let dbfs = self.history.sample(now, age_seconds).map(|levels| {
            levels[bin * BANDS / frequency_pixels..(bin + 1) * BANDS / frequency_pixels]
                .iter()
                .copied()
                .fold(-120.0_f32, f32::max)
        });
        SpectrogramPin {
            frequency_hz: self.history.data.settings.frequency(frequency, sample_rate),
            age_seconds,
            dbfs,
        }
    }

    pub fn reset_settings(&mut self) {
        let mut history = std::mem::take(&mut self.history);
        history.data.settings = Default::default();
        *self = Self {
            history,
            ..Self::default()
        };
    }

    pub fn controls(&mut self, ui: &mut egui::Ui) {
        settings_panel(ui, "Time & History", |ui| {
            super::settings_group(ui, "Duration", |ui| {
                ui.add(crate::parameter::Parameter::new(&mut self.seconds, 0.1..=30.0, 8.0).logarithmic(true).text("History s"))
                .help_text("Visible history duration. Retains up to 30 seconds so changing the view does not discard recent audio.");
            });
        });
        settings_panel(ui, "Frequency Range", |ui| {
            self.history.data.settings.range_controls(ui)
        });
        settings_panel(ui, "Signal Response", |ui| {
            self.history.data.settings.response_controls(ui)
        });
        settings_panel(ui, "Appearance", |ui| {
            super::settings_group(ui, "Layout & detail", |ui| {
                super::properties_combo("orientation", 176.0, 2)
                .selected_text(if self.vertical { "Newest at bottom" } else { "Newest at right" })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.vertical, false, "Newest at right").help_text("Time runs left to right; frequency runs low to high from bottom to top.");
                    ui.selectable_value(&mut self.vertical, true, "Newest at bottom").help_text("Time runs top to bottom; frequency runs low to high from left to right.");
                }).response.help_text("Change scrolling orientation without clearing captured history.");
                ui.checkbox(&mut self.smooth, "Smooth pixels")
                .help_text("Interpolate between display cells. Turn off for sharp, pixelated cells. Does not change FFT or frequency smoothing.");
                ui.add(crate::parameter::Parameter::new(&mut self.time_pixels, 64..=1024, 240).text("Time cells"))
                .help_text("Display resolution along time. More cells produce a finer raster, not a higher capture rate; history is sampled up to 30 times per second.");
                ui.add(crate::parameter::Parameter::new(&mut self.frequency_pixels, 24..=BANDS, BANDS).text("Freq. cells"))
                .help_text("Displayed frequency rows or columns. Fewer cells group bands using their loudest level to preserve peaks. FFT resolution is unchanged.");
            });
            super::settings_group(ui, "Color & level", |ui| {
                self.palette.controls(ui);
                self.palette.contrast_controls(ui, &mut self.contrast);
                self.history.data.settings.level_controls(ui);
            });
            super::settings_group(ui, "Guides", |ui| {
                ui.checkbox(&mut self.grid, "Grid")
                    .help_text("Overlay time and frequency reference divisions.");
                ui.checkbox(&mut self.labels, "Axis labels").help_text(
                "Show frequency and time labels. Note names are available under Frequency Range.",
            );
                if ui
                    .add_enabled(
                        self.pinned.is_some(),
                        egui::Button::new("Clear pinned measurement"),
                    )
                    .help_text("Remove the pane-local measurement crosshair.")
                    .clicked()
                {
                    self.pinned = None;
                }
            });
        });
    }

    fn image(&self, now: Instant, theme: &AppTheme) -> egui::ColorImage {
        let times = self.time_pixels.clamp(64, 1024);
        let bands = self.frequency_pixels.clamp(24, BANDS);
        let [width, height] = if self.vertical {
            [bands, times]
        } else {
            [times, bands]
        };
        let mut pixels = vec![theme.background; width * height];
        for time in 0..times {
            let age = self.seconds * (1.0 - time as f32 / (times - 1) as f32);
            if let Some(levels) = self.history.sample(now, age) {
                for band in 0..bands {
                    let start = band * BANDS / bands;
                    let end = (band + 1) * BANDS / bands;
                    let db = levels[start..end]
                        .iter()
                        .copied()
                        .fold(-120.0_f32, f32::max);
                    let color = self.palette.color_with_contrast(
                        self.history.data.settings.intensity(db),
                        self.contrast,
                        theme,
                    );
                    let (x, y) = if self.vertical {
                        (band, time)
                    } else {
                        (time, bands - band - 1)
                    };
                    pixels[y * width + x] = color;
                }
            }
        }
        egui::ColorImage::new([width, height], pixels)
    }

    pub fn draw(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        frame: &AnalysisFrame,
        theme: &AppTheme,
        now: Instant,
        render_state: ModuleRenderState,
    ) {
        let ModuleRenderState { live, frozen } = render_state;
        self.history.update(frame, now, live);
        let key = RasterKey {
            newest: self.history.rows.back().map(|row| (row.sequence, row.time)),
            oldest: self.history.rows.front().map(|row| row.time),
            time_cell: self.history.rows.back().map_or(0, |row| {
                (now.saturating_duration_since(row.time).as_secs_f64()
                    * self.time_pixels.saturating_sub(1) as f64
                    / f64::from(self.seconds)) as u64
            }),
            seconds: self.seconds,
            time_pixels: self.time_pixels,
            frequency_pixels: self.frequency_pixels,
            vertical: self.vertical,
            smooth: self.smooth,
            palette: self.palette,
            contrast: self.contrast,
            settings: self.history.data.settings.clone(),
            theme: theme.clone(),
        };
        if self.raster_key.as_ref() != Some(&key) {
            let image = self.image(now, theme);
            let options = if self.smooth {
                egui::TextureOptions::LINEAR
            } else {
                egui::TextureOptions::NEAREST
            };
            match &mut self.texture {
                Some(texture) => texture.set(image, options),
                None => {
                    self.texture = Some(ui.ctx().load_texture(
                        format!("spectrogram-{:?}", ui.id()),
                        image,
                        options,
                    ))
                }
            }
            self.raster_key = Some(key);
        }
        let painter = ui.painter_at(rect);
        let plot = Rect::from_min_max(
            rect.min + Vec2::new(if self.labels { 44.0 } else { 10.0 }, 12.0),
            rect.max - Vec2::new(12.0, if self.labels { 24.0 } else { 10.0 }),
        );
        if !plot.is_positive() {
            return;
        }
        if let Some(texture) = &self.texture {
            painter.image(
                texture.id(),
                plot,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        if self.grid {
            let stroke = Stroke::new(0.5, mix(theme.background, theme.foreground, 0.45));
            for t in [0.25, 0.5, 0.75] {
                let x = plot.left() + t * plot.width();
                let y = plot.top() + t * plot.height();
                painter.line_segment(
                    [Pos2::new(x, plot.top()), Pos2::new(x, plot.bottom())],
                    stroke,
                );
                painter.line_segment(
                    [Pos2::new(plot.left(), y), Pos2::new(plot.right(), y)],
                    stroke,
                );
            }
        }
        if self.labels {
            for step in 0..=2 {
                let t = step as f32 / 2.0;
                let position = if self.vertical {
                    Pos2::new(plot.left() + t * (plot.width() - 25.0), plot.bottom() + 7.0)
                } else {
                    Pos2::new(rect.left() + 3.0, plot.bottom() - t * plot.height() - 5.0)
                };
                label(
                    &painter,
                    position,
                    self.history.data.settings.axis_label(t, frame.sample_rate),
                    theme.muted,
                );
            }
            let (old, newest) = if self.vertical {
                (
                    Pos2::new(rect.left() + 2.0, plot.top()),
                    Pos2::new(rect.left() + 2.0, plot.bottom() - 12.0),
                )
            } else {
                (
                    plot.left_bottom() + Vec2::new(0.0, 7.0),
                    plot.right_bottom() + Vec2::new(-22.0, 7.0),
                )
            };
            label(&painter, old, format!("−{:.1}s", self.seconds), theme.muted);
            label(&painter, newest, "now", theme.muted);
        }
        if frozen && let Some(pin) = self.pinned {
            let frequency = self
                .history
                .data
                .settings
                .fraction_for_frequency(pin.frequency_hz, frame.sample_rate);
            let time = (1.0 - pin.age_seconds / self.seconds.max(0.001)).clamp(0.0, 1.0);
            let (x, y) = if self.vertical {
                (
                    plot.left() + frequency * plot.width(),
                    plot.top() + time * plot.height(),
                )
            } else {
                (
                    plot.left() + time * plot.width(),
                    plot.bottom() - frequency * plot.height(),
                )
            };
            let stroke = Stroke::new(1.0, theme.warning);
            painter.line_segment(
                [Pos2::new(x, plot.top()), Pos2::new(x, plot.bottom())],
                stroke,
            );
            painter.line_segment(
                [Pos2::new(plot.left(), y), Pos2::new(plot.right(), y)],
                stroke,
            );
            label(
                &painter,
                Pos2::new(
                    (x + 5.0).min(plot.right() - 105.0),
                    (y - 16.0).max(plot.top()),
                ),
                format!(
                    "{} · −{:.2}s · {}",
                    self.history.data.settings.label_frequency(pin.frequency_hz),
                    pin.age_seconds,
                    pin.dbfs
                        .map_or_else(|| "No audio".into(), |db| format!("{db:.1} dBFS"))
                ),
                theme.warning,
            );
        }
        let response = ui.interact(
            plot,
            ui.id().with("spectrogram-measure"),
            egui::Sense::click(),
        );
        if frozen
            && response.clicked()
            && let Some(pointer) = response.interact_pointer_pos()
        {
            let x = ((pointer.x - plot.left()) / plot.width()).clamp(0.0, 1.0);
            let y = ((pointer.y - plot.top()) / plot.height()).clamp(0.0, 1.0);
            self.pinned = Some(self.measurement_at(x, y, now, frame.sample_rate));
        }
        if let Some(pointer) = response.hover_pos() {
            let x = ((pointer.x - plot.left()) / plot.width()).clamp(0.0, 1.0);
            let y = ((pointer.y - plot.top()) / plot.height()).clamp(0.0, 1.0);
            let measurement = self.measurement_at(x, y, now, frame.sample_rate);
            let action = if frozen { " · click to pin" } else { "" };
            response.help_text(format!(
                "{} · −{:.2} s · {}{action}",
                self.history
                    .data
                    .settings
                    .label_frequency(measurement.frequency_hz),
                measurement.age_seconds,
                measurement
                    .dbfs
                    .map_or_else(|| "No captured audio".into(), |db| format!("{db:.1} dBFS"))
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::frequency::HistoryRow;
    use super::*;

    #[test]
    fn copying_appearance_leaves_destination_history_intact() {
        let source = Spectrogram {
            seconds: 2.0,
            vertical: true,
            palette: Palette::Heatmap,
            ..Spectrogram::default()
        };
        let mut target = Spectrogram::default();
        target.history.rows.push_back(HistoryRow {
            time: Instant::now(),
            levels: [-25.0; BANDS],
            magnitudes: vec![0.1].into(),
            sequence: 9,
        });
        target.apply_settings(&source.settings_snapshot());
        assert_eq!(target.seconds, 2.0);
        assert!(target.vertical);
        assert_eq!(target.history.rows.len(), 1);
        assert_eq!(target.history.rows[0].sequence, 9);
    }

    #[test]
    fn paused_raster_is_cached_but_style_changes_upload_without_clearing_history() {
        let context = egui::Context::default();
        let mut spectrogram = Spectrogram::default();
        let now = Instant::now();
        spectrogram.history.rows.push_back(HistoryRow {
            time: now,
            levels: [-30.0; BANDS],
            magnitudes: vec![].into(),
            sequence: 1,
        });
        let render = |spectrogram: &mut Spectrogram| {
            let mut output = context.run_ui(egui::RawInput::default(), |ui| {
                spectrogram.draw(
                    ui,
                    Rect::from_min_size(Pos2::ZERO, egui::vec2(500.0, 300.0)),
                    &AnalysisFrame::default(),
                    &AppTheme::default(),
                    now,
                    ModuleRenderState {
                        live: false,
                        frozen: false,
                    },
                );
            });
            let updates = output.textures_delta.set.len();
            output.textures_delta.clear();
            updates
        };
        assert!(render(&mut spectrogram) > 0);
        assert_eq!(render(&mut spectrogram), 0);
        spectrogram.grid = true;
        spectrogram.labels = false;
        assert_eq!(render(&mut spectrogram), 0);
        spectrogram.vertical = true;
        spectrogram.smooth = false;
        assert_eq!(render(&mut spectrogram), 1);
        assert_eq!(spectrogram.history.rows.len(), 1);
        assert_eq!(render(&mut spectrogram), 0);
        spectrogram.palette = Palette::Heatmap;
        spectrogram.contrast = 3.0;
        assert_eq!(render(&mut spectrogram), 1);
    }

    #[test]
    fn orientation_and_resolution_preserve_peak_positions_and_history() {
        let now = Instant::now();
        let theme = AppTheme::default();
        let mut spectrogram = Spectrogram {
            frequency_pixels: 24,
            time_pixels: 64,
            seconds: 0.1,
            ..Spectrogram::default()
        };
        let mut levels = [-90.0; BANDS];
        levels[0] = 0.0;
        spectrogram.history.rows.push_back(HistoryRow {
            time: now,
            levels,
            magnitudes: vec![].into(),
            sequence: 1,
        });
        let horizontal = spectrogram.image(now, &theme);
        assert_eq!(horizontal.size, [64, 24]);
        let loud = spectrogram.palette.color(1.0, &theme);
        assert_eq!(horizontal.pixels[23 * 64 + 63], loud);
        spectrogram.vertical = true;
        let vertical = spectrogram.image(now, &theme);
        assert_eq!(vertical.size, [24, 64]);
        assert_eq!(vertical.pixels[63 * 24], loud);
        assert_ne!(vertical.pixels[63 * 24 + 23], loud);
        assert_eq!(spectrogram.history.rows.len(), 1);
        spectrogram.reset_settings();
        assert_eq!(spectrogram.history.rows.len(), 1);
    }

    #[test]
    fn display_resolution_is_bounded_and_empty_history_has_no_fake_signal() {
        let spectrogram = Spectrogram {
            time_pixels: usize::MAX,
            frequency_pixels: usize::MAX,
            ..Spectrogram::default()
        };
        let theme = AppTheme::default();
        let image = spectrogram.image(Instant::now(), &theme);
        assert_eq!(image.size, [1024, BANDS]);
        assert!(image.pixels.iter().all(|p| *p == theme.background));
    }

    #[test]
    fn pinned_measurements_map_consistently_in_both_orientations_and_clear() {
        let now = Instant::now();
        let mut spectrogram = Spectrogram::default();
        spectrogram.history.rows.push_back(HistoryRow {
            time: now,
            levels: [-24.0; BANDS],
            magnitudes: vec![].into(),
            sequence: 1,
        });

        let horizontal = spectrogram.measurement_at(1.0, 0.5, now, 48_000);
        spectrogram.vertical = true;
        let vertical = spectrogram.measurement_at(0.5, 1.0, now, 48_000);
        assert!((horizontal.frequency_hz - vertical.frequency_hz).abs() < 0.01);
        assert_eq!(horizontal.age_seconds, 0.0);
        assert_eq!(horizontal.dbfs, Some(-24.0));
        assert_eq!(horizontal, vertical);

        spectrogram.pinned = Some(horizontal);
        spectrogram.clear_cursor();
        assert!(spectrogram.pinned.is_none());
    }
}
