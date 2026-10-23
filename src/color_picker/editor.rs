use egui::{Color32, Rect, Response, Sense, Stroke, TextureHandle, Ui, Vec2};

use super::formats::{self, Format};
use crate::{
    help::HoverHelp,
    icons::{self, Icon},
};

#[derive(Clone)]
pub(super) struct Editor {
    color: [u8; 3],
    hsv: [f32; 3],
    format: Format,
    text: String,
    invalid: bool,
    field: Option<(f32, TextureHandle)>,
    format_popup: Option<egui::Id>,
}

impl Editor {
    pub fn new(color: [u8; 3]) -> Self {
        Self {
            color,
            hsv: formats::hsv(color),
            format: Format::Hex,
            text: Format::Hex.text(color),
            invalid: false,
            field: None,
            format_popup: None,
        }
    }
    pub fn rgb(&self) -> [u8; 3] {
        self.color
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
        let width = (ui.ctx().content_rect().width() - 32.0).clamp(220.0, 300.0);
        ui.set_width(width);
        ui.spacing_mut().item_spacing = egui::vec2(6.0, 10.0);
        ui.spacing_mut().interact_size.y = 28.0;
        self.saturation_value(ui, width);
        let sample = ui
            .horizontal(|ui| {
                let sample = ui
                    .add_enabled_ui(cfg!(target_os = "macos") || cfg!(test), |ui| {
                        icons::sized_button(
                            ui,
                            Icon::Eyedropper,
                            false,
                            28.0,
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
        self.values(ui);
        sample
    }

    pub fn format_menu_hovered(&self, ctx: &egui::Context) -> bool {
        self.format_popup.is_some_and(|id| {
            egui::Popup::is_id_open(ctx, id)
                && ctx
                    .pointer_hover_pos()
                    .is_some_and(|pos| ctx.read_response(id).is_some_and(|r| r.rect.contains(pos)))
        })
    }

    fn saturation_value(&mut self, ui: &mut Ui, width: f32) {
        let height = (ui.ctx().content_rect().height() - 190.0)
            .clamp(140.0, 270.0)
            .min(width);
        let (bounds, response) =
            ui.allocate_exact_size(Vec2::new(width, height), Sense::click_and_drag());
        let rect = bounds.shrink(8.0);
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
            ui.allocate_exact_size(Vec2::new(width, 28.0), Sense::click_and_drag());
        let rect = bounds.shrink2(egui::vec2(8.0, 5.0));
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
                .selected_text(self.format.label())
                .show_ui(ui, |ui| {
                    for format in Format::ALL {
                        ui.selectable_value(&mut self.format, format, format.label());
                    }
                });
            self.format_popup = Some(menu.response.id.with("popup"));
            if self.format != before {
                self.refresh_text();
            }
            if matches!(self.format, Format::Hex | Format::Css) {
                let edit = egui::TextEdit::singleline(&mut self.text)
                    .char_limit(96)
                    .font(egui::FontId::monospace(12.0))
                    .desired_width(ui.available_width());
                let response = ui.add_sized([ui.available_width(), 28.0], edit);
                if response.changed() {
                    if let Some(color) = self.format.parse(&self.text) {
                        self.set_rgb(color);
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
                    "Enter RGB hex, such as FF8800 or #F80."
                } else {
                    "Enter #hex, rgb(255 136 0), or hsl(32deg 100% 50%). Colors are opaque."
                });
            } else {
                self.components(ui);
            }
        });
        ui.horizontal(|ui| {
            if self.invalid {
                ui.colored_label(ui.visuals().error_fg_color, "Enter a valid opaque color.");
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new("100% opacity").size(10.0).weak())
                    .help_text("This color is opaque.");
            });
        });
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
        let width = ((ui.available_width() - 12.0) / 3.0).max(35.0);
        let mut changed = false;
        for (i, value) in values.iter_mut().enumerate() {
            ui.vertical(|ui| {
                ui.set_width(width);
                ui.spacing_mut().item_spacing.y = 3.0;
                changed |= ui
                    .add_sized(
                        [width, 28.0],
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
