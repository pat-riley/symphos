use crate::help::HoverHelp;
use eframe::egui::{self, Color32, Pos2, Rect, Response, Stroke, Vec2};

#[derive(Clone, Copy)]
pub enum Icon {
    Camera,
    Time,
    Geometry,
    Frequency,
    Response,
    Appearance,
    Waveform,
    Collapse,
    Expand,
    Stereo,
    Mix,
    Settings,
    Gear,
    Surface,
    Lines,
    YLines,
    Wireframe,
    Dots,
    Stems,
    Bars,
    Fullscreen,
    Restore,
}

pub fn button(ui: &mut egui::Ui, icon: Icon, selected: bool, description: &str) -> Response {
    sized_button(ui, icon, selected, 30.0, description)
}

pub fn sized_button(
    ui: &mut egui::Ui,
    icon: Icon,
    selected: bool,
    size: f32,
    description: &str,
) -> Response {
    let response = ui.add(
        egui::Button::new("")
            .min_size(Vec2::splat(size))
            .selected(selected),
    );
    let color = ui.style().interact(&response).fg_stroke.color;
    paint(ui, response.rect, icon, color);
    response.help_text(description)
}

pub fn paint(ui: &egui::Ui, rect: Rect, icon: Icon, color: Color32) {
    let painter = ui.painter_at(rect);
    let origin = rect.center() - Vec2::splat(10.0);
    let p = |x: f32, y: f32| origin + Vec2::new(x, y);
    let stroke = Stroke::new(1.4, color);
    let line = |points: &[(f32, f32)]| {
        painter.add(egui::Shape::line(
            points.iter().map(|&(x, y)| p(x, y)).collect(),
            stroke,
        ));
    };
    let box_outline = |a: Pos2, b: Pos2| {
        painter.rect_stroke(
            Rect::from_min_max(a, b),
            2.0,
            stroke,
            egui::StrokeKind::Inside,
        );
    };
    match icon {
        Icon::Fullscreen | Icon::Restore => {
            for (x, y) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                let outer = if matches!(icon, Icon::Fullscreen) {
                    7.0
                } else {
                    3.0
                };
                let inner = if matches!(icon, Icon::Fullscreen) {
                    3.0
                } else {
                    7.0
                };
                line(&[
                    (10.0 + x * inner, 10.0 + y * outer),
                    (10.0 + x * outer, 10.0 + y * outer),
                    (10.0 + x * outer, 10.0 + y * inner),
                ]);
            }
        }
        Icon::Bars => {
            for (x, height) in [(2.0, 7.0), (8.0, 15.0), (14.0, 11.0)] {
                painter.rect_filled(
                    Rect::from_min_max(p(x, 18.0 - height), p(x + 4.0, 18.0)),
                    0.6,
                    color,
                );
            }
        }
        Icon::Surface => {
            painter.add(egui::Shape::convex_polygon(
                vec![p(1.0, 16.0), p(7.0, 3.0), p(12.0, 9.0), p(19.0, 16.0)],
                color.gamma_multiply(0.3),
                stroke,
            ));
        }
        Icon::Lines | Icon::YLines => {
            for offset in [0.0, 5.0, 10.0] {
                let points = [
                    (2.0, 8.0 + offset),
                    (7.0, 2.0 + offset),
                    (12.0, 6.0 + offset),
                    (18.0, 8.0 + offset),
                ];
                let points: Vec<_> = points
                    .into_iter()
                    .map(|(x, y)| {
                        if matches!(icon, Icon::YLines) {
                            (y, x)
                        } else {
                            (x, y)
                        }
                    })
                    .collect();
                line(&points);
            }
        }
        Icon::Wireframe => {
            for t in [3.0, 10.0, 17.0] {
                line(&[(t, 2.0), (t, 18.0)]);
                line(&[(2.0, t), (18.0, t)]);
            }
        }
        Icon::Dots => {
            for x in [4.0, 10.0, 16.0] {
                for y in [4.0, 10.0, 16.0] {
                    painter.circle_filled(p(x, y), 1.6, color);
                }
            }
        }
        Icon::Stems => {
            for (x, y) in [(4.0, 10.0), (10.0, 3.0), (16.0, 7.0)] {
                line(&[(x, 18.0), (x, y)]);
                painter.circle_filled(p(x, y), 2.0, color);
            }
        }
        Icon::Camera => {
            box_outline(p(1.0, 5.0), p(19.0, 17.0));
            line(&[(5.0, 5.0), (7.0, 2.0), (13.0, 2.0), (15.0, 5.0)]);
            painter.circle_stroke(p(10.0, 11.0), 3.4, stroke);
        }
        Icon::Time => {
            painter.circle_stroke(p(10.0, 10.0), 8.0, stroke);
            line(&[(10.0, 4.0), (10.0, 10.0), (14.0, 12.0)]);
        }
        Icon::Geometry => {
            line(&[
                (10.0, 1.0),
                (19.0, 6.0),
                (19.0, 15.0),
                (10.0, 19.0),
                (1.0, 15.0),
                (1.0, 6.0),
                (10.0, 1.0),
            ]);
            line(&[(1.0, 6.0), (10.0, 11.0), (19.0, 6.0)]);
            line(&[(10.0, 11.0), (10.0, 19.0)]);
        }
        Icon::Frequency => {
            for (x, height) in [(3.0, 5.0), (7.5, 13.0), (12.0, 9.0), (16.5, 16.0)] {
                line(&[(x, 18.0), (x, 18.0 - height)]);
            }
        }
        Icon::Response => {
            line(&[
                (1.0, 17.0),
                (5.0, 17.0),
                (9.0, 3.0),
                (12.0, 9.0),
                (16.0, 14.0),
                (19.0, 15.0),
            ]);
        }
        Icon::Appearance => {
            // Diagonal paintbrush: outlined handle, solid bristles, and a small
            // paint stroke. Recognizable at toolbar size without tiny dots.
            line(&[
                (7.0, 10.0),
                (16.0, 1.0),
                (19.0, 4.0),
                (10.0, 13.0),
                (7.0, 10.0),
            ]);
            painter.add(egui::Shape::convex_polygon(
                vec![
                    p(2.0, 18.0),
                    p(3.0, 13.0),
                    p(7.0, 10.0),
                    p(10.0, 13.0),
                    p(7.0, 17.0),
                ],
                color,
                Stroke::NONE,
            ));
            line(&[(10.0, 18.0), (17.0, 18.0)]);
        }
        Icon::Waveform => line(&[
            (1.0, 10.0),
            (4.0, 10.0),
            (6.0, 3.0),
            (9.0, 17.0),
            (12.0, 5.0),
            (15.0, 10.0),
            (19.0, 10.0),
        ]),
        Icon::Collapse => line(&[(13.0, 3.0), (6.0, 10.0), (13.0, 17.0)]),
        Icon::Expand => line(&[(7.0, 3.0), (14.0, 10.0), (7.0, 17.0)]),
        Icon::Stereo => {
            painter.circle_stroke(p(6.0, 10.0), 5.0, stroke);
            painter.circle_stroke(p(14.0, 10.0), 5.0, stroke);
        }
        Icon::Mix => {
            painter.circle_stroke(p(10.0, 10.0), 6.0, stroke);
        }
        Icon::Gear => {
            let mut teeth: Vec<_> = (0..32)
                .map(|step| {
                    let angle = step as f32 * std::f32::consts::TAU / 32.0;
                    let radius = if matches!(step % 4, 1 | 2) { 8.0 } else { 6.0 };
                    p(10.0, 10.0) + Vec2::angled(angle) * radius
                })
                .collect();
            teeth.push(teeth[0]);
            painter.add(egui::Shape::line(teeth, stroke));
            painter.circle_stroke(p(10.0, 10.0), 2.8, stroke);
        }
        Icon::Settings => {
            for (y, knob) in [(4.0, 6.0), (10.0, 14.0), (16.0, 8.0)] {
                line(&[(2.0, y), (18.0, y)]);
                painter.circle_filled(p(knob, y), 2.3, color);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pane_focus_icons_are_compact_distinct_and_clickable() {
        let mut glyphs = Vec::new();
        for icon in [Icon::Fullscreen, Icon::Restore] {
            let context = egui::Context::default();
            let render = |events| {
                let mut clicked = false;
                let mut rect = Rect::NOTHING;
                let mut output = context.run_ui(
                    egui::RawInput {
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        ui.spacing_mut().interact_size.y = 24.0;
                        ui.spacing_mut().button_padding = Vec2::new(6.0, 3.0);
                        let response = sized_button(ui, icon, false, 24.0, "Toggle expanded pane");
                        rect = response.rect;
                        clicked = response.clicked();
                    },
                );
                output.textures_delta.clear();
                assert_eq!(rect.size(), Vec2::splat(24.0));
                (output, rect, clicked)
            };
            let (output, rect, _) = render(vec![]);
            assert!(
                !output
                    .shapes
                    .iter()
                    .any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if !text.galley.job.text.is_empty()))
            );
            let paths: Vec<_> = output
                .shapes
                .iter()
                .filter_map(|shape| {
                    if let egui::Shape::Path(path) = &shape.shape {
                        Some(path.points.clone())
                    } else {
                        None
                    }
                })
                .collect();
            assert_eq!(paths.len(), 4);
            assert!(
                paths
                    .iter()
                    .flatten()
                    .all(|point| rect.shrink(3.0).contains(*point))
            );
            glyphs.push(paths);
            let pos = rect.center();
            for pressed in [true, false] {
                let (_, _, clicked) = render(vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ]);
                if !pressed {
                    assert!(clicked);
                }
            }
        }
        assert_ne!(glyphs[0], glyphs[1]);
    }

    #[test]
    fn settings_icon_matches_picker_height_and_opens_its_menu() {
        for picker_height in [26.0, 32.0] {
            let context = egui::Context::default();
            let mut button_rect = Rect::NOTHING;
            let mut render = |events| {
                let mut output = context.run_ui(
                    egui::RawInput {
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        ui.spacing_mut().interact_size.y = picker_height;
                        ui.spacing_mut().button_padding = Vec2::new(6.0, 4.0);
                        ui.style_mut().override_font_id = Some(egui::FontId::proportional(13.0));
                        ui.horizontal_centered(|ui| {
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let picker = egui::ComboBox::from_id_salt("source")
                                        .width(260.0)
                                        .selected_text("System output speakers")
                                        .show_ui(ui, |_| {})
                                        .response;
                                    let options = sized_button(
                                        ui,
                                        Icon::Gear,
                                        false,
                                        picker.rect.height(),
                                        "View settings",
                                    );
                                    button_rect = options.rect;
                                    assert!(
                                        (options.rect.height() - picker.rect.height()).abs() < 0.01
                                    );
                                    assert!(
                                        (options.rect.width() - picker.rect.height()).abs() < 0.01
                                    );
                                    assert!(
                                        (options.rect.center().y - picker.rect.center().y).abs()
                                            < 0.01,
                                        "picker {:?}, icon {:?}",
                                        picker.rect,
                                        options.rect
                                    );
                                    egui::Popup::menu(&options).show(|ui| {
                                        ui.label("Reset pane sizes");
                                    });
                                },
                            );
                        });
                    },
                );
                output.textures_delta.clear();
                (output, button_rect)
            };
            render(vec![]);
            let (_, rect) = render(vec![]);
            let position = rect.center();
            for pressed in [true, false] {
                render(vec![
                    egui::Event::PointerMoved(position),
                    egui::Event::PointerButton {
                        pos: position,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ]);
            }
            let (output, _) = render(vec![]);
            assert!(output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.job.text == "Reset pane sizes")));
        }
    }
}
