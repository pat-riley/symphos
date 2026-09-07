use std::time::Instant;

use eframe::egui::{self, Color32, Pos2, Rect, Vec2};

use super::{
    Palette,
    frequency::{BANDS, History},
    frequency_label, label,
};
use crate::{analysis::AnalysisFrame, theme::AppTheme};

pub struct Spectrogram {
    history: History,
    seconds: f32,
    palette: Palette,
    texture: Option<egui::TextureHandle>,
}

impl Default for Spectrogram {
    fn default() -> Self {
        Self {
            history: History::default(),
            seconds: 8.0,
            palette: Palette::default(),
            texture: None,
        }
    }
}

impl Spectrogram {
    pub fn clear(&mut self) {
        self.history.clear();
        self.texture = None;
    }

    pub fn controls(&mut self, ui: &mut egui::Ui) {
        ui.add(egui::Slider::new(&mut self.seconds, 2.0..=30.0).text("History s"));
        self.palette.controls(ui);
        ui.separator();
        self.history.data.settings.controls(ui);
    }

    pub fn draw(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        frame: &AnalysisFrame,
        theme: &AppTheme,
        now: Instant,
        live: bool,
    ) {
        self.history.update(frame, now, live);
        let width = 240;
        let mut pixels = vec![theme.background; width * BANDS];
        let settings = &self.history.data.settings;
        for x in 0..width {
            let age = self.seconds * (1.0 - x as f32 / (width - 1) as f32);
            if let Some(levels) = self.history.sample(now, age) {
                for (band, db) in levels.iter().enumerate() {
                    pixels[(BANDS - band - 1) * width + x] =
                        self.palette.color(settings.intensity(*db), theme);
                }
            }
        }
        let image = egui::ColorImage::new([width, BANDS], pixels);
        match &mut self.texture {
            Some(texture) => texture.set(image, egui::TextureOptions::LINEAR),
            None => {
                self.texture = Some(ui.ctx().load_texture(
                    format!("spectrogram-{:?}", ui.id()),
                    image,
                    egui::TextureOptions::LINEAR,
                ))
            }
        }
        let painter = ui.painter_at(rect);
        let plot = Rect::from_min_max(
            rect.min + Vec2::new(36.0, 12.0),
            rect.max - Vec2::new(12.0, 24.0),
        );
        if let Some(texture) = &self.texture {
            painter.image(
                texture.id(),
                plot,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        for step in 0..=2 {
            let t = step as f32 / 2.0;
            label(
                &painter,
                Pos2::new(rect.left() + 3.0, plot.bottom() - t * plot.height() - 5.0),
                frequency_label(settings.frequency(t, frame.sample_rate)),
                theme.muted,
            );
        }
        label(
            &painter,
            plot.left_bottom() + Vec2::new(0.0, 7.0),
            format!("−{:.0} s", self.seconds),
            theme.muted,
        );
        label(
            &painter,
            plot.right_bottom() + Vec2::new(-22.0, 7.0),
            "now",
            theme.muted,
        );
    }
}
