use egui::{Color32, Rect, Response, Sense, Stroke, TextureHandle, Ui, Vec2};

use super::formats::{self, Format};
use crate::{
    help::HoverHelp,
    icons::{self, Icon},
};

#[derive(Clone)]
pub(super) struct Editor {
    color: [u8; 3],
    alpha: u8,
    hsv: [f32; 3],
    format: Format,
    text: String,
    invalid: bool,
    field: Option<(f32, TextureHandle)>,
    opacity_texture: Option<([u8; 3], TextureHandle)>,
}

impl Editor {
    pub fn new(color: [u8; 3]) -> Self {
        Self {
            color,
            alpha: 255,
            hsv: formats::hsv(color),
            format: Format::Hex,
            text: Format::Hex.text(color),
            invalid: false,
            field: None,
            opacity_texture: None,
        }
    }
    pub fn rgb(&self) -> [u8; 3] {
        self.color
    }
    pub fn alpha(&self) -> u8 {
        self.alpha
    }
    pub fn sync_alpha(&mut self, alpha: u8) {
        if self.alpha != alpha {
            self.alpha = alpha;
            self.refresh_text();
        }
    }
    pub fn sync(&mut self, color: [u8; 3]) {
        if self.color != color {
            self.set_rgb(color);
            self.refresh_text();
        }
    }
    fn set_rgb(&mut self, color: [u8; 3]) {
        let mut hsv = formats::hsv(color);
        if hsv[1] == 0.0 {
            hsv[0] = self.hsv[0];
        }
        self.hsv = hsv;
        self.color = color;
    }
    fn refresh_text(&mut self) {
        self.text = self.format.text(self.color);
        if self.alpha < 255 {
            if self.format == Format::Css {
                self.text.pop();
                self.text
                    .push_str(&format!(" / {:.3})", self.alpha as f32 / 255.0));
            } else {
                self.text.push_str(&format!("{:02X}", self.alpha));
            }
        }
        self.invalid = false;
    }
    fn set_hsv(&mut self, hsv: [f32; 3]) {
        let hsv = hsv.map(|value| {
            if value.is_finite() {
                value.clamp(0.0, 1.0)
            } else {
                0.0
            }
        });
        self.hsv = hsv;
        self.color = formats::rgb(hsv);
        self.refresh_text();
    }

    pub fn controls(&mut self, ui: &mut Ui) -> Response {
        let width = width(ui.ctx());
        ui.set_width(width);
        ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
        ui.spacing_mut().interact_size.y = 26.0;
        self.saturation_value(ui, width);
        let sample = ui
            .horizontal(|ui| {
                let sample = ui
                    .add_enabled_ui(cfg!(target_os = "macos") || cfg!(test), |ui| {
                        icons::sized_button(
                            ui,
                            Icon::Eyedropper,
                            false,
                            26.0,
                            "Sample screen color",
                        )
                    })
                    .inner;
                sample.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Sample screen color")
                });
                self.hue_slider(ui, width - 34.0);
                sample
            })
            .inner;
        ui.horizontal(|ui| {
            ui.allocate_space(egui::vec2(26.0, 26.0));
            self.opacity_slider(ui, width - 34.0);
        });
        self.values(ui);
        sample
    }

    fn saturation_value(&mut self, ui: &mut Ui, width: f32) {
        let height = (ui.ctx().content_rect().height() - 190.0)
            .clamp(140.0, 260.0)
            .min(width);
        let (bounds, response) =
            ui.allocate_exact_size(Vec2::new(width, height), Sense::click_and_drag());
        let rect = bounds;
        let response = response.on_hover_cursor(egui::CursorIcon::Crosshair);
        if let Some(pos) = response.interact_pointer_pos() {
            self.set_hsv([
                self.hsv[0],
                ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0),
                (1.0 - (pos.y - rect.top()) / rect.height()).clamp(0.0, 1.0),
            ]);
        }
        if self
            .field
            .as_ref()
            .is_none_or(|(hue, _)| *hue != self.hsv[0])
        {
            const SIZE: usize = 64;
            let pixels = (0..SIZE * SIZE)
                .map(|i| {
                    let color = formats::rgb([
                        self.hsv[0],
                        (i % SIZE) as f32 / (SIZE - 1) as f32,
                        1.0 - (i / SIZE) as f32 / (SIZE - 1) as f32,
                    ]);
                    Color32::from_rgb(color[0], color[1], color[2])
                })
                .collect();
            let image = egui::ColorImage::new([SIZE, SIZE], pixels);
            if let Some((hue, texture)) = &mut self.field {
                texture.set(image, egui::TextureOptions::LINEAR);
                *hue = self.hsv[0];
            } else {
                self.field = Some((
                    self.hsv[0],
                    ui.ctx()
                        .load_texture("Color field", image, egui::TextureOptions::LINEAR),
                ));
            }
        }
        ui.painter().add(
            egui::epaint::RectShape::filled(rect, 6, Color32::WHITE).with_texture(
                self.field.as_ref().unwrap().1.id(),
                Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
            ),
        );
        ui.painter().rect_stroke(
            rect,
            6,
            Stroke::new(1.0, ui.visuals().text_color().gamma_multiply(0.15)),
            egui::StrokeKind::Inside,
        );
        self.handle(
            ui,
            egui::pos2(
                rect.left() + self.hsv[1] * rect.width(),
                rect.bottom() - self.hsv[2] * rect.height(),
            ),
            self.color,
        );
        response
            .help_text("Drag to choose saturation (left to right) and brightness (bottom to top).");
    }

    fn hue_slider(&mut self, ui: &mut Ui, width: f32) {
        let (bounds, response) =
            ui.allocate_exact_size(Vec2::new(width, 26.0), Sense::click_and_drag());
        let rect = bounds.shrink2(egui::vec2(0.0, 4.0));
        let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
        if let Some(pos) = response.interact_pointer_pos() {
            self.set_hsv([
                ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0),
                self.hsv[1],
                self.hsv[2],
            ]);
        }
        let key = egui::Id::new("color-picker-hue-strip");
        let texture = ui
            .ctx()
            .data(|d| d.get_temp::<TextureHandle>(key))
            .unwrap_or_else(|| {
                let pixels = (0..256)
                    .map(|i| {
                        let color = formats::rgb([i as f32 / 255.0, 1.0, 1.0]);
                        Color32::from_rgb(color[0], color[1], color[2])
                    })
                    .collect();
                let texture = ui.ctx().load_texture(
                    "Hue strip",
                    egui::ColorImage::new([256, 1], pixels),
                    egui::TextureOptions::LINEAR,
                );
                ui.ctx().data_mut(|d| d.insert_temp(key, texture.clone()));
                texture
            });
        ui.painter().add(
            egui::epaint::RectShape::filled(rect, 100, Color32::WHITE).with_texture(
                texture.id(),
                Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
            ),
        );
        self.handle(
            ui,
            egui::pos2(rect.left() + self.hsv[0] * rect.width(), rect.center().y),
            formats::rgb([self.hsv[0], 1.0, 1.0]),
        );
        response.help_text("Drag to choose the hue. Saturation and brightness stay unchanged.");
    }

    fn opacity_slider(&mut self, ui: &mut Ui, width: f32) {
        let (bounds, response) =
            ui.allocate_exact_size(Vec2::new(width, 26.0), Sense::click_and_drag());
        let rect = bounds.shrink2(egui::vec2(0.0, 4.0));
        if let Some(pos) = response.interact_pointer_pos() {
            self.alpha =
                (((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0) * 255.0).round() as u8;
            self.refresh_text();
        }
        if self
            .opacity_texture
            .as_ref()
            .is_none_or(|(color, _)| *color != self.color)
        {
            let [r, g, b] = self.color;
            let pixels = (0..128 * 8)
                .map(|i| {
                    let background =
                        Color32::from_gray(if ((i % 128) / 4 + (i / 128) / 4) % 2 == 0 {
                            72
                        } else {
                            104
                        });
                    background.lerp_to_gamma(Color32::from_rgb(r, g, b), (i % 128) as f32 / 127.0)
                })
                .collect();
            let image = egui::ColorImage::new([128, 8], pixels);
            if let Some((color, texture)) = &mut self.opacity_texture {
                texture.set(image, egui::TextureOptions::LINEAR);
                *color = self.color;
            } else {
                self.opacity_texture = Some((
                    self.color,
                    ui.ctx()
                        .load_texture("Color opacity", image, egui::TextureOptions::LINEAR),
                ));
            }
        }
        ui.painter().add(
            egui::epaint::RectShape::filled(rect, 100, Color32::WHITE).with_texture(
                self.opacity_texture.as_ref().unwrap().1.id(),
                Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
            ),
        );
        self.handle(
            ui,
            egui::pos2(
                rect.left() + self.alpha as f32 / 255.0 * rect.width(),
                rect.center().y,
            ),
            self.color,
        );
        response
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .help_text("Drag to set opacity: transparent on the left, opaque on the right.");
        #[cfg(test)]
        ui.ctx()
            .data_mut(|d| d.insert_temp(egui::Id::new("test-opacity-slider"), rect));
    }

    fn handle(&self, ui: &Ui, pos: egui::Pos2, [r, g, b]: [u8; 3]) {
        ui.painter()
            .circle_filled(pos, 9.5, Color32::from_black_alpha(80));
        ui.painter().circle_filled(pos, 8.0, Color32::WHITE);
        ui.painter()
            .circle_filled(pos, 5.5, Color32::from_rgb(r, g, b));
    }

    fn values(&mut self, ui: &mut Ui) {
        ui.horizontal_top(|ui| {
            let before = self.format;
            let menu = crate::dropdown::ComboBox::from_id_salt("color-format")
                .width(66.0)
                .control_height(26.0)
                .selected_text(self.format.label())
                .show_ui(ui, |ui| {
                    for format in Format::ALL {
                        ui.selectable_value(&mut self.format, format, format.label());
                    }
                });
            #[cfg(test)]
            ui.ctx().data_mut(|d| {
                d.insert_temp(egui::Id::new("test-format-control"), menu.response.rect)
            });
            let _ = menu;
            if self.format != before {
                self.refresh_text();
            }
            if matches!(self.format, Format::Hex | Format::Css) {
                let edit = egui::TextEdit::singleline(&mut self.text)
                    .char_limit(96)
                    .font(egui::FontId::monospace(11.0))
                    .vertical_align(egui::Align::Center)
                    .margin(egui::vec2(6.0, 0.0))
                    .desired_width((ui.available_width() - 62.0).max(50.0));
                let response = ui.add_sized([(ui.available_width() - 62.0).max(50.0), 26.0], edit);
                if response.changed() {
                    if let Some((color, alpha)) = self.format.parse_color(&self.text) {
                        self.set_rgb(color);
                        self.alpha = alpha;
                        self.invalid = false;
                    } else {
                        self.invalid = true;
                    }
                }
                if response.lost_focus() && !self.invalid {
                    self.refresh_text();
                }
                #[cfg(test)]
                ui.ctx().data_mut(|d| {
                    d.insert_temp(egui::Id::new("test-color-text-field"), response.rect)
                });
                if self.invalid {
                    ui.painter().rect_stroke(
                        response.rect,
                        3,
                        Stroke::new(1.0, ui.visuals().error_fg_color),
                        egui::StrokeKind::Inside,
                    );
                }
                response.help_text(if self.format == Format::Hex {
                    "Enter RGB or RGBA hex, such as FF8800, #F80, or FF880080."
                } else {
                    "Enter #hex, rgb(255 136 0 / 50%), or hsl(32deg 100% 50%)."
                });
            } else {
                self.components(ui);
            }
            let mut opacity = self.alpha as f32 / 255.0 * 100.0;
            if ui
                .add_sized(
                    [54.0, 26.0],
                    egui::DragValue::new(&mut opacity)
                        .range(0.0..=100.0)
                        .suffix("%")
                        .max_decimals(0),
                )
                .changed()
            {
                self.alpha = (opacity / 100.0 * 255.0).round() as u8;
                self.refresh_text();
            }
        });
        if self.invalid {
            ui.colored_label(ui.visuals().error_fg_color, "Enter a valid color.");
        }
    }

    fn components(&mut self, ui: &mut Ui) {
        let mut values = match self.format {
            Format::Rgb => self.color.map(|x| x as f32),
            Format::Hsl => {
                let [h, s, l] = formats::hsl(self.hsv);
                [h * 360.0, s * 100.0, l * 100.0]
            }
            _ => {
                let [h, s, v] = self.hsv;
                [h * 360.0, s * 100.0, v * 100.0]
            }
        };
        let labels = match self.format {
            Format::Rgb => ["R", "G", "B"],
            Format::Hsl => ["H", "S", "L"],
            _ => ["H", "S", "B"],
        };
        let width =
            ((ui.available_width() - 54.0 - 3.0 * ui.spacing().item_spacing.x) / 3.0).max(35.0);
        let mut changed = false;
        for (i, value) in values.iter_mut().enumerate() {
            ui.vertical(|ui| {
                ui.set_width(width);
                ui.spacing_mut().item_spacing.y = 3.0;
                changed |= ui
                    .add_sized(
                        [width, 26.0],
                        egui::DragValue::new(value)
                            .range(
                                0.0..=if self.format == Format::Rgb {
                                    255.0
                                } else if i == 0 {
                                    360.0
                                } else {
                                    100.0
                                },
                            )
                            .max_decimals(if self.format == Format::Rgb { 0 } else { 1 })
                            .speed(0.5),
                    )
                    .changed();
                ui.label(egui::RichText::new(labels[i]).size(10.0).weak());
            });
        }
        if changed {
            if self.format == Format::Rgb {
                self.set_rgb(values.map(|x| x.round() as u8));
                self.refresh_text();
            } else {
                let [h, s, v] = values;
                let components = [h / 360.0, s / 100.0, v / 100.0];
                self.set_hsv(if self.format == Format::Hsl {
                    formats::from_hsl(components)
                } else {
                    components
                });
            }
        }
    }
}

pub(super) fn width(ctx: &egui::Context) -> f32 {
    (ctx.content_rect().width() - 48.0).clamp(220.0, 300.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_representation_fits_the_popup_and_static_fields_reuse_textures() {
        for visuals in [egui::Visuals::dark(), egui::Visuals::light()] {
            let ctx = egui::Context::default();
            ctx.set_visuals(visuals);
            let mut editor = Editor::new([245, 136, 70]);
            for format in Format::ALL {
                editor.format = format;
                editor.refresh_text();
                let render = |editor: &mut Editor| {
                    ctx.run_ui(
                        egui::RawInput {
                            screen_rect: Some(Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::vec2(700.0, 700.0),
                            )),
                            ..Default::default()
                        },
                        |ui| {
                            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(
                                Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(300.0, 600.0)),
                            ));
                            editor.controls(&mut child);
                            assert!(
                                child.min_rect().width() <= 300.1,
                                "{} overflowed: {:?}",
                                format.label(),
                                child.min_rect()
                            );
                            assert!(child.min_rect().height() < 450.0);
                        },
                    )
                };
                let mut first = render(&mut editor);
                first.textures_delta.clear();
                let mut steady = render(&mut editor);
                assert!(steady.textures_delta.set.is_empty());
                steady.textures_delta.clear();
            }
        }
    }
}
