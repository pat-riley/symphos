use eframe::egui::{self, Color32, FontId, Pos2, Rect, Stroke, Vec2};

use super::{label, mix, properties_combo, settings_panel};
use crate::help::HoverHelp;
use crate::{analysis::AnalysisFrame, theme::AppTheme};

const LOW_CROSSOVER_HZ: f32 = 200.0;
const HIGH_CROSSOVER_HZ: f32 = 2_000.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum DisplayMode {
    Scaled,
    Linear,
    Lissajous,
}

impl DisplayMode {
    const ALL: [Self; 3] = [Self::Scaled, Self::Linear, Self::Lissajous];

    const fn label(self) -> &'static str {
        match self {
            Self::Scaled => "Scaled",
            Self::Linear => "Linear",
            Self::Lissajous => "Lissajous",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum ColorMode {
    Static,
    Rgb,
    MultiBand,
}

impl ColorMode {
    const ALL: [Self; 3] = [Self::Static, Self::Rgb, Self::MultiBand];

    const fn label(self) -> &'static str {
        match self {
            Self::Static => "Static",
            Self::Rgb => "RGB",
            Self::MultiBand => "Multi-band",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum CorrelationMode {
    SingleBand,
    MultiBand,
}

impl CorrelationMode {
    const ALL: [Self; 2] = [Self::SingleBand, Self::MultiBand];

    const fn label(self) -> &'static str {
        match self {
            Self::SingleBand => "Single-band",
            Self::MultiBand => "Multi-band",
        }
    }
}

pub struct Stereometer {
    display_mode: DisplayMode,
    color_mode: ColorMode,
    correlation_mode: CorrelationMode,
    window_ms: f32,
    dot_size: f32,
    intensity: f32,
    guides: bool,
    labels: bool,
    show_correlation: bool,
    show_balance: bool,
}

module_settings!(Stereometer, StereometerSettings, {
display_mode: DisplayMode => display_mode,
color_mode: ColorMode => color_mode,
correlation_mode: CorrelationMode => correlation_mode,
window_ms: f32 => window_ms,
dot_size: f32 => dot_size,
intensity: f32 => intensity,
guides: bool => guides,
labels: bool => labels,
show_correlation: bool => show_correlation,
show_balance: bool => show_balance,
});

impl Default for Stereometer {
    fn default() -> Self {
        Self {
            display_mode: DisplayMode::Linear,
            color_mode: ColorMode::Rgb,
            correlation_mode: CorrelationMode::MultiBand,
            window_ms: 120.0,
            dot_size: 2.2,
            intensity: 0.9,
            guides: true,
            labels: true,
            show_correlation: true,
            show_balance: true,
        }
    }
}

impl Stereometer {
    pub(super) const FACTORY_PRESETS: [&'static str; 3] =
        ["Balanced Stereo", "Lissajous Focus", "Multiband Phase"];

    pub(super) fn factory_settings(index: usize) -> Option<StereometerSettings> {
        let mut module = Self::default();
        match index {
            0 => {}
            1 => {
                module.display_mode = DisplayMode::Lissajous;
                module.color_mode = ColorMode::Static;
                module.correlation_mode = CorrelationMode::SingleBand;
                module.window_ms = 80.0;
            }
            2 => {
                module.display_mode = DisplayMode::Linear;
                module.color_mode = ColorMode::MultiBand;
                module.correlation_mode = CorrelationMode::MultiBand;
                module.window_ms = 180.0;
            }
            _ => return None,
        }
        Some(module.settings_snapshot())
    }

    pub fn clear(&mut self) {}

    pub fn reset_settings(&mut self) {
        *self = Self::default();
    }

    pub fn controls(&mut self, ui: &mut egui::Ui) {
        settings_panel(ui, "Display", |ui| {
            super::settings_group(ui, "Vectorscope", |ui| {
                properties_combo("stereometer-display-mode", 176.0, DisplayMode::ALL.len())
                    .selected_text(self.display_mode.label())
                    .show_ui(ui, |ui| {
                        for mode in DisplayMode::ALL {
                            ui.selectable_value(&mut self.display_mode, mode, mode.label())
                                .help_text(match mode {
                                    DisplayMode::Scaled => "Rotate stereo into Mid/Side space and expand quiet samples while preserving their direction.",
                                    DisplayMode::Linear => "Map incoming stereo amplitude one-to-one after rotating it into Mid/Side space.",
                                    DisplayMode::Lissajous => "Plot Left horizontally against Right vertically without the Mid/Side rotation.",
                                });
                        }
                    })
                    .response
                    .help_text("Choose the vectorscope projection. Linear is the conventional rotated stereo monitor.");
                ui.add(
                    crate::parameter::Parameter::new(&mut self.window_ms, 20.0..=250.0, 120.0)
                        .bounds(5.0..=500.0)
                        .logarithmic(true)
                        .text("Persistence ms"),
                )
                .help_text("Amount of recent full-rate stereo audio retained in the dot cloud. This does not change analysis rate or capture history.");
            });
            super::settings_group(ui, "Guides", |ui| {
                ui.checkbox(&mut self.guides, "Projection guides")
                    .help_text(
                        "Show the full-scale boundary, center axes, and half-scale reference.",
                    );
                ui.checkbox(&mut self.labels, "Axis labels")
                    .help_text("Label Mid/Side or Left/Right polarity and the correlation scale.");
                ui.checkbox(&mut self.show_balance, "Balance indicator")
                    .help_text("Show the RMS balance of the visible window from Left through center to Right.");
            });
        });

        settings_panel(ui, "Appearance", |ui| {
            super::settings_group(ui, "Color", |ui| {
                properties_combo("stereometer-color-mode", 176.0, ColorMode::ALL.len())
                    .selected_text(self.color_mode.label())
                    .show_ui(ui, |ui| {
                        for mode in ColorMode::ALL {
                            ui.selectable_value(&mut self.color_mode, mode, mode.label())
                                .help_text(match mode {
                                    ColorMode::Static => "Use the current theme accent for every sample.",
                                    ColorMode::Rgb => "Blend red, green, and blue from each sample's low-, mid-, and high-frequency balance.",
                                    ColorMode::MultiBand => "Overlay separate low, mid, and high vectorscopes in red, green, and blue.",
                                });
                        }
                    })
                    .response
                    .help_text("Choose a solid trace, frequency-balanced RGB dots, or three overlaid band views.");
            });
            super::settings_group(ui, "Dots", |ui| {
                ui.add(
                    crate::parameter::Parameter::new(&mut self.dot_size, 1.0..=5.0, 2.2)
                        .bounds(0.5..=12.0)
                        .text("Size px"),
                )
                .help_text("Screen-space size of each plotted audio sample.");
                ui.add(
                    crate::parameter::Parameter::new(&mut self.intensity, 0.1..=1.0, 0.9)
                        .bounds(0.01..=1.0)
                        .text("Intensity"),
                )
                .help_text(
                    "Maximum dot opacity. Older samples fade within the persistence window.",
                );
            });
        });

        settings_panel(ui, "Correlation", |ui| {
            super::settings_group(ui, "Meter", |ui| {
                ui.checkbox(&mut self.show_correlation, "Correlation meter")
                    .help_text("Show phase correlation from −1 (opposite polarity) through 0 to +1 (mono-compatible).");
                ui.add_enabled_ui(self.show_correlation, |ui| {
                    properties_combo(
                        "stereometer-correlation-mode",
                        176.0,
                        CorrelationMode::ALL.len(),
                    )
                    .selected_text(self.correlation_mode.label())
                    .show_ui(ui, |ui| {
                        for mode in CorrelationMode::ALL {
                            ui.selectable_value(&mut self.correlation_mode, mode, mode.label())
                                .help_text(match mode {
                                    CorrelationMode::SingleBand => "Show one overall Left/Right correlation marker.",
                                    CorrelationMode::MultiBand => "Show low, mid, and high correlation separately, plus the overall marker.",
                                });
                        }
                    })
                    .response
                    .help_text("Choose overall correlation only or the overall and three frequency bands together.");
                });
            });
            super::settings_group(ui, "Bands", |ui| {
                ui.label("Low  < 200 Hz");
                ui.label("Mid  200 Hz – 2 kHz");
                ui.label("High > 2 kHz");
                ui.small("The same display-only crossover filters drive RGB color, multi-band dots, and correlation.")
                    .help_text("Band splitting runs only for the visible Stereometer window and never changes captured audio or shared FFT analysis.");
            });
        });
    }

    pub fn draw(
        &self,
        ui: &mut egui::Ui,
        rect: Rect,
        frame: &AnalysisFrame,
        theme: &AppTheme,
        live: bool,
    ) {
        let painter = ui.painter_at(rect);
        let outer = rect.shrink(10.0);
        if !outer.is_positive() {
            return;
        }

        let meter_width = if self.show_correlation {
            (outer.width() * 0.17).clamp(42.0, 72.0)
        } else {
            0.0
        };
        let meter_gap = if self.show_correlation { 15.0 } else { 0.0 };
        let scope_area = Rect::from_min_max(
            outer.min,
            Pos2::new(outer.right() - meter_width - meter_gap, outer.bottom()),
        );
        let balance_height = if self.show_balance { 23.0 } else { 0.0 };
        let scope_side = scope_area
            .width()
            .min(scope_area.height() - balance_height)
            .max(0.0);
        let scope_center = Pos2::new(
            scope_area.center().x,
            scope_area.top() + (scope_area.height() - balance_height) * 0.5,
        );
        let scope = Rect::from_center_size(scope_center, Vec2::splat(scope_side));
        if !scope.is_positive() {
            return;
        }

        if self.guides {
            paint_scope_guides(&painter, scope, self.display_mode, theme);
        }
        if self.labels {
            paint_scope_labels(&painter, scope, self.display_mode, theme);
        }

        let len = frame.waveform_left.len().min(frame.waveform_right.len());
        let count = ((self.window_ms * frame.sample_rate.max(1) as f32 / 1_000.0).ceil() as usize)
            .clamp(2, len.max(2))
            .min(len);
        let start = len.saturating_sub(count);
        let samples = if count >= 2 {
            split_stereo_bands(
                &frame.waveform_left[start..len],
                &frame.waveform_right[start..len],
                frame.sample_rate,
            )
        } else {
            Vec::new()
        };

        if live && samples.len() >= 2 {
            match self.color_mode {
                ColorMode::Static => paint_samples(
                    &painter,
                    scope,
                    &samples,
                    None,
                    self.display_mode,
                    self.dot_size,
                    self.intensity,
                    theme,
                ),
                ColorMode::Rgb => paint_samples(
                    &painter,
                    scope,
                    &samples,
                    Some(usize::MAX),
                    self.display_mode,
                    self.dot_size,
                    self.intensity,
                    theme,
                ),
                ColorMode::MultiBand => {
                    for band in 0..3 {
                        paint_samples(
                            &painter,
                            scope,
                            &samples,
                            Some(band),
                            self.display_mode,
                            self.dot_size,
                            self.intensity,
                            theme,
                        );
                    }
                }
            }
        }

        if self.show_balance {
            paint_balance(
                &painter,
                scope,
                live.then(|| stereo_balance(&samples)),
                self.labels,
                theme,
            );
        }
        if self.show_correlation {
            let meter = Rect::from_min_max(
                Pos2::new(scope_area.right() + meter_gap, outer.top() + 8.0),
                Pos2::new(outer.right(), outer.bottom() - 8.0),
            );
            paint_correlation_meter(
                &painter,
                meter,
                live.then(|| correlations(&samples)),
                self.correlation_mode,
                self.labels,
                theme,
            );
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct StereoBands {
    full: [f32; 2],
    components: [[f32; 2]; 3],
    envelopes: [f32; 3],
}

fn split_stereo_bands(left: &[f32], right: &[f32], sample_rate: u32) -> Vec<StereoBands> {
    let rate = sample_rate.max(1) as f32;
    let coefficient = |frequency: f32| 1.0 - (-std::f32::consts::TAU * frequency / rate).exp();
    let low_coefficient = coefficient(LOW_CROSSOVER_HZ);
    let mid_coefficient = coefficient(HIGH_CROSSOVER_HZ);
    let release = (-1.0 / (rate * 0.012)).exp();
    let mut low_pass = [0.0_f32; 2];
    let mut mid_pass = [0.0_f32; 2];
    let mut envelopes = [0.0_f32; 3];

    left.iter()
        .zip(right)
        .map(|(&left, &right)| {
            let full = [left, right];
            for channel in 0..2 {
                low_pass[channel] += (full[channel] - low_pass[channel]) * low_coefficient;
                mid_pass[channel] += (full[channel] - mid_pass[channel]) * mid_coefficient;
            }
            let components = [
                low_pass,
                [mid_pass[0] - low_pass[0], mid_pass[1] - low_pass[1]],
                [full[0] - mid_pass[0], full[1] - mid_pass[1]],
            ];
            for band in 0..3 {
                let level = components[band][0].abs().max(components[band][1].abs());
                envelopes[band] = level.max(envelopes[band] * release);
            }
            StereoBands {
                full,
                components,
                envelopes,
            }
        })
        .collect()
}

fn scope_coordinates(mode: DisplayMode, left: f32, right: f32) -> Vec2 {
    let mut point = match mode {
        DisplayMode::Lissajous => Vec2::new(left, -right),
        DisplayMode::Linear | DisplayMode::Scaled => {
            Vec2::new((right - left) * 0.5, -(left + right) * 0.5)
        }
    };
    point.x = point.x.clamp(-1.0, 1.0);
    point.y = point.y.clamp(-1.0, 1.0);
    if mode == DisplayMode::Scaled {
        let radius = point.length();
        if radius > 1.0e-7 {
            point *= radius.sqrt() / radius;
            let diamond_extent = point.x.abs() + point.y.abs();
            if diamond_extent > 1.0 {
                point /= diamond_extent;
            }
        }
    }
    point
}

fn to_screen(scope: Rect, point: Vec2) -> Pos2 {
    scope.center()
        + Vec2::new(
            point.x * scope.width() * 0.5,
            point.y * scope.height() * 0.5,
        )
}

fn band_colors() -> [Color32; 3] {
    [
        Color32::from_rgb(245, 74, 70),
        Color32::from_rgb(106, 224, 88),
        Color32::from_rgb(76, 126, 255),
    ]
}

fn rgb_color(envelopes: [f32; 3], theme: &AppTheme) -> Color32 {
    let sum = envelopes.iter().sum::<f32>();
    if sum <= 1.0e-7 {
        return theme.muted;
    }
    let colors = band_colors();
    let weighted = |component: fn(&Color32) -> u8| {
        envelopes
            .iter()
            .zip(colors.iter())
            .map(|(&weight, color)| weight * component(color) as f32)
            .sum::<f32>()
            / sum
    };
    Color32::from_rgb(
        weighted(Color32::r).round() as u8,
        weighted(Color32::g).round() as u8,
        weighted(Color32::b).round() as u8,
    )
}

#[allow(clippy::too_many_arguments)]
fn paint_samples(
    painter: &egui::Painter,
    scope: Rect,
    samples: &[StereoBands],
    color_or_band: Option<usize>,
    display_mode: DisplayMode,
    dot_size: f32,
    intensity: f32,
    theme: &AppTheme,
) {
    let multi_band = color_or_band.is_some_and(|value| value < 3);
    let point_budget = if multi_band { 900 } else { 2_400 };
    let stride = samples.len().div_ceil(point_budget).max(1);
    let radius = dot_size * 0.5;
    let mut mesh = egui::Mesh::default();
    for (index, sample) in samples.iter().enumerate().step_by(stride) {
        let stereo = match color_or_band {
            Some(band) if band < 3 => sample.components[band],
            _ => sample.full,
        };
        let point = to_screen(scope, scope_coordinates(display_mode, stereo[0], stereo[1]));
        let base_color = match color_or_band {
            None => theme.accent,
            Some(band) if band < 3 => band_colors()[band],
            Some(_) => rgb_color(sample.envelopes, theme),
        };
        let age = (index + 1) as f32 / samples.len() as f32;
        let color = base_color.gamma_multiply(intensity * (0.12 + 0.88 * age.powf(1.6)));
        let base = mesh.vertices.len() as u32;
        for corner in [
            Vec2::new(-radius, -radius),
            Vec2::new(radius, -radius),
            Vec2::new(radius, radius),
            Vec2::new(-radius, radius),
        ] {
            mesh.colored_vertex(point + corner, color);
        }
        mesh.add_triangle(base, base + 1, base + 2);
        mesh.add_triangle(base, base + 2, base + 3);
    }
    painter.add(egui::Shape::mesh(mesh));
}

fn paint_scope_guides(painter: &egui::Painter, scope: Rect, mode: DisplayMode, theme: &AppTheme) {
    let strong = Stroke::new(1.0, mix(theme.background, theme.muted, 0.72));
    let faint = Stroke::new(0.7, mix(theme.background, theme.muted, 0.42));
    let center = scope.center();
    painter.line_segment(
        [
            Pos2::new(scope.left(), center.y),
            Pos2::new(scope.right(), center.y),
        ],
        faint,
    );
    painter.line_segment(
        [
            Pos2::new(center.x, scope.top()),
            Pos2::new(center.x, scope.bottom()),
        ],
        faint,
    );
    if mode == DisplayMode::Lissajous {
        painter.rect_stroke(scope, 0.0, strong, egui::StrokeKind::Inside);
        painter.rect_stroke(
            scope.shrink(scope.width() * 0.25),
            0.0,
            faint,
            egui::StrokeKind::Inside,
        );
    } else {
        let diamond = [
            scope.center_top(),
            scope.right_center(),
            scope.center_bottom(),
            scope.left_center(),
            scope.center_top(),
        ];
        painter.add(egui::Shape::line(diamond.to_vec(), strong));
        let inner = scope.shrink(scope.width() * 0.25);
        painter.add(egui::Shape::line(
            vec![
                inner.center_top(),
                inner.right_center(),
                inner.center_bottom(),
                inner.left_center(),
                inner.center_top(),
            ],
            faint,
        ));
    }
}

fn paint_scope_labels(painter: &egui::Painter, scope: Rect, mode: DisplayMode, theme: &AppTheme) {
    let [top, right, bottom, left] = if mode == DisplayMode::Lissajous {
        ["R+", "L+", "R−", "L−"]
    } else {
        ["+M", "+S", "−M", "−S"]
    };
    painter.text(
        scope.center_top() + Vec2::new(0.0, 3.0),
        egui::Align2::CENTER_TOP,
        top,
        FontId::monospace(9.0),
        theme.muted,
    );
    painter.text(
        scope.right_center() - Vec2::new(3.0, 0.0),
        egui::Align2::RIGHT_CENTER,
        right,
        FontId::monospace(9.0),
        theme.muted,
    );
    painter.text(
        scope.center_bottom() - Vec2::new(0.0, 3.0),
        egui::Align2::CENTER_BOTTOM,
        bottom,
        FontId::monospace(9.0),
        theme.muted,
    );
    painter.text(
        scope.left_center() + Vec2::new(3.0, 0.0),
        egui::Align2::LEFT_CENTER,
        left,
        FontId::monospace(9.0),
        theme.muted,
    );
}

fn correlation<I>(samples: I) -> f32
where
    I: Iterator<Item = [f32; 2]>,
{
    let (cross, left, right) = samples.fold((0.0_f64, 0.0_f64, 0.0_f64), |sum, sample| {
        let l = f64::from(sample[0]);
        let r = f64::from(sample[1]);
        (sum.0 + l * r, sum.1 + l * l, sum.2 + r * r)
    });
    let denominator = (left * right).sqrt();
    if denominator <= 1.0e-15 {
        0.0
    } else {
        (cross / denominator).clamp(-1.0, 1.0) as f32
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct Correlations {
    overall: f32,
    bands: [f32; 3],
}

fn correlations(samples: &[StereoBands]) -> Correlations {
    Correlations {
        overall: correlation(samples.iter().map(|sample| sample.full)),
        bands: std::array::from_fn(|band| {
            correlation(samples.iter().map(|sample| sample.components[band]))
        }),
    }
}

fn stereo_balance(samples: &[StereoBands]) -> f32 {
    let (left, right) = samples.iter().fold((0.0_f64, 0.0_f64), |sum, sample| {
        (
            sum.0 + f64::from(sample.full[0]).powi(2),
            sum.1 + f64::from(sample.full[1]).powi(2),
        )
    });
    let left = left.sqrt();
    let right = right.sqrt();
    if left + right <= 1.0e-12 {
        0.0
    } else {
        ((right - left) / (right + left)).clamp(-1.0, 1.0) as f32
    }
}

fn paint_balance(
    painter: &egui::Painter,
    scope: Rect,
    balance: Option<f32>,
    labels: bool,
    theme: &AppTheme,
) {
    let y = scope.bottom() + 13.0;
    let left = scope.left() + 5.0;
    let right = scope.right() - 5.0;
    let guide = Stroke::new(1.0, mix(theme.background, theme.muted, 0.65));
    painter.line_segment([Pos2::new(left, y), Pos2::new(right, y)], guide);
    painter.line_segment(
        [
            Pos2::new((left + right) * 0.5, y - 3.0),
            Pos2::new((left + right) * 0.5, y + 3.0),
        ],
        guide,
    );
    if let Some(balance) = balance {
        let x = egui::lerp(left..=right, (balance + 1.0) * 0.5);
        painter.circle_filled(Pos2::new(x, y), 3.0, theme.accent);
    }
    if labels {
        label(painter, Pos2::new(left, y + 3.0), "L", theme.muted);
        painter.text(
            Pos2::new(right, y + 3.0),
            egui::Align2::RIGHT_TOP,
            "R",
            FontId::monospace(10.0),
            theme.muted,
        );
    }
}

fn paint_correlation_meter(
    painter: &egui::Painter,
    rect: Rect,
    values: Option<Correlations>,
    mode: CorrelationMode,
    labels: bool,
    theme: &AppTheme,
) {
    if !rect.is_positive() {
        return;
    }
    painter.rect_filled(rect, 2.0, theme.card.gamma_multiply(0.55));
    painter.rect_stroke(
        rect,
        2.0,
        Stroke::new(0.8, mix(theme.background, theme.muted, 0.55)),
        egui::StrokeKind::Inside,
    );
    let marker_y = |value: f32| {
        egui::lerp(
            rect.bottom()..=rect.top(),
            (value.clamp(-1.0, 1.0) + 1.0) * 0.5,
        )
    };
    for value in [-1.0, 0.0, 1.0] {
        let y = marker_y(value);
        painter.line_segment(
            [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
            Stroke::new(
                if value == 0.0 { 1.0 } else { 0.7 },
                mix(
                    theme.background,
                    theme.muted,
                    if value == 0.0 { 0.68 } else { 0.42 },
                ),
            ),
        );
        if labels {
            painter.text(
                Pos2::new(rect.left() - 4.0, y),
                egui::Align2::RIGHT_CENTER,
                if value > 0.0 {
                    "+1"
                } else if value < 0.0 {
                    "−1"
                } else {
                    "0"
                },
                FontId::monospace(9.0),
                theme.muted,
            );
        }
    }
    let Some(values) = values else {
        return;
    };
    let overall_y = marker_y(values.overall);
    painter.rect_filled(
        Rect::from_min_max(
            Pos2::new(rect.left() + 2.0, overall_y - 1.0),
            Pos2::new(rect.right() - 2.0, overall_y + 1.0),
        ),
        0.5,
        theme.foreground,
    );
    if mode == CorrelationMode::SingleBand {
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(rect.left() + 4.0, overall_y - 2.5),
                Pos2::new(rect.right() - 4.0, overall_y + 2.5),
            ),
            1.0,
            theme.accent,
        );
        return;
    }
    let colors = band_colors();
    let column = (rect.width() - 6.0) / 3.0;
    for (band, color) in colors.into_iter().enumerate() {
        let left = rect.left() + 2.0 + band as f32 * column;
        let right = left + column - 2.0;
        let y = marker_y(values.bands[band]);
        painter.rect_filled(
            Rect::from_min_max(Pos2::new(left, y - 2.5), Pos2::new(right, y + 2.5)),
            1.0,
            color,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tones(left_phase: f32, right_phase: f32) -> Vec<StereoBands> {
        let rate = 48_000.0;
        let mut left = Vec::new();
        let mut right = Vec::new();
        for index in 0..4_800 {
            let phase = std::f32::consts::TAU * 440.0 * index as f32 / rate;
            left.push((phase + left_phase).sin());
            right.push((phase + right_phase).sin());
        }
        split_stereo_bands(&left, &right, rate as u32)
    }

    #[test]
    fn linear_projection_is_rotated_and_scaled_expands_quiet_samples() {
        assert_eq!(
            scope_coordinates(DisplayMode::Linear, 1.0, 1.0),
            Vec2::new(0.0, -1.0)
        );
        assert_eq!(
            scope_coordinates(DisplayMode::Linear, 1.0, -1.0),
            Vec2::new(-1.0, 0.0)
        );
        assert_eq!(
            scope_coordinates(DisplayMode::Lissajous, 0.25, -0.5),
            Vec2::new(0.25, 0.5)
        );
        let linear = scope_coordinates(DisplayMode::Linear, 0.01, 0.01).length();
        let scaled = scope_coordinates(DisplayMode::Scaled, 0.01, 0.01).length();
        assert!(scaled > linear);
        assert!(scaled <= 1.0);
    }

    #[test]
    fn correlation_reports_in_phase_opposite_phase_and_silence() {
        let in_phase = tones(0.0, 0.0);
        let opposite = tones(0.0, std::f32::consts::PI);
        assert!(correlations(&in_phase).overall > 0.999);
        assert!(correlations(&opposite).overall < -0.999);
        assert_eq!(correlation(std::iter::repeat_n([0.0, 0.0], 64)), 0.0);
        let hard_left = std::iter::repeat_n(
            StereoBands {
                full: [1.0, 0.0],
                ..StereoBands::default()
            },
            64,
        )
        .collect::<Vec<_>>();
        assert_eq!(stereo_balance(&hard_left), -1.0);
    }

    #[test]
    fn band_filters_separate_low_and_high_tones() {
        let rate = 48_000.0;
        let tone = |frequency: f32| {
            (0..9_600)
                .map(|index| (std::f32::consts::TAU * frequency * index as f32 / rate).sin())
                .collect::<Vec<_>>()
        };
        let low = tone(80.0);
        let high = tone(8_000.0);
        let low = split_stereo_bands(&low, &low, rate as u32);
        let high = split_stereo_bands(&high, &high, rate as u32);
        let energy = |samples: &[StereoBands], band: usize| {
            samples[samples.len() / 2..]
                .iter()
                .map(|sample| sample.components[band][0].powi(2))
                .sum::<f32>()
        };
        assert!(energy(&low, 0) > energy(&low, 2));
        assert!(energy(&high, 2) > energy(&high, 0));
    }

    #[test]
    fn all_modes_render_finite_geometry_and_settings_copy_without_runtime_data() {
        let context = egui::Context::default();
        let mut frame = AnalysisFrame {
            sequence: 1,
            waveform_left: (0..4_800)
                .map(|index| (index as f32 * 0.031).sin() * 0.8)
                .collect(),
            waveform_right: (0..4_800)
                .map(|index| (index as f32 * 0.037).sin() * 0.6)
                .collect(),
            ..AnalysisFrame::default()
        };
        for display_mode in DisplayMode::ALL {
            for color_mode in ColorMode::ALL {
                let mut meter = Stereometer {
                    display_mode,
                    color_mode,
                    ..Stereometer::default()
                };
                let settings = meter.settings_snapshot();
                meter.reset_settings();
                meter.apply_settings(&settings);
                assert_eq!(meter.settings_snapshot(), settings);
                let mut output = context.run_ui(egui::RawInput::default(), |ui| {
                    meter.draw(
                        ui,
                        Rect::from_min_size(Pos2::ZERO, Vec2::new(520.0, 340.0)),
                        &frame,
                        &AppTheme::default(),
                        true,
                    );
                });
                output.textures_delta.clear();
                let primitives = context.tessellate(output.shapes, output.pixels_per_point);
                let vertices: Vec<_> = primitives
                    .iter()
                    .flat_map(|primitive| match &primitive.primitive {
                        egui::epaint::Primitive::Mesh(mesh) => mesh.vertices.as_slice(),
                        _ => &[],
                    })
                    .collect();
                assert!(vertices.len() > 100);
                assert!(vertices.iter().all(|vertex| vertex.pos.is_finite()));
            }
        }
        frame.waveform_left.clear();
        frame.waveform_right.clear();
        let meter = Stereometer::default();
        let mut output = context.run_ui(egui::RawInput::default(), |ui| {
            meter.draw(
                ui,
                Rect::from_min_size(Pos2::ZERO, Vec2::new(120.0, 80.0)),
                &frame,
                &AppTheme::default(),
                false,
            );
        });
        output.textures_delta.clear();
    }
}
