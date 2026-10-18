use crate::help::HoverHelp;
use eframe::egui::{self, Color32, Rect, Response, Stroke, Vec2};

#[derive(Clone, Copy)]
pub enum Icon {
    Pause,
    Play,
    Waveform,
    Waterfall,
    Sphere,
    Cylinder,
    Rings,
    Spectrogram,
    Oscilloscope,
    Veroni,
    Strata,
    Flux,
    Eye,
    Inspector,
    Plus,
    Fullscreen,
    Restore,
    Close,
    Stereo,
    Settings,
    Surface,
    Lines,
    YLines,
    Wireframe,
    Dots,
    Stems,
    Bars,
    Palette,
    Camera,
    Clock,
    AudioMappings,
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

/// A monochrome, borderless toolbar control matching a native sidebar toggle.
pub fn sidebar_toggle(ui: &mut egui::Ui, open: bool, size: f32, description: &str) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::click());
    if response.hovered() || response.is_pointer_button_down_on() {
        ui.painter()
            .rect_filled(rect, 5.0, ui.visuals().text_color().gamma_multiply(0.08));
    }
    let color = ui
        .style()
        .interact(&response)
        .fg_stroke
        .color
        .gamma_multiply(if open { 1.0 } else { 0.65 });
    paint(ui, rect, Icon::Inspector, color);
    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .help_text(description)
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
    match icon {
        Icon::Palette => {
            painter.circle_stroke(p(10.0, 10.0), 8.0, stroke);
            for (x, y) in [(6.0, 5.0), (11.0, 4.0), (15.0, 8.0), (6.0, 11.0)] {
                painter.circle_filled(p(x, y), 1.5, color);
            }
            painter.circle_stroke(p(12.0, 14.0), 2.0, stroke);
        }
        Icon::Camera => {
            painter.rect_stroke(
                Rect::from_min_max(p(2.0, 5.0), p(18.0, 17.0)),
                2.0,
                stroke,
                egui::StrokeKind::Inside,
            );
            painter.circle_stroke(p(10.0, 11.0), 3.4, stroke);
            line(&[(5.0, 5.0), (6.0, 2.0), (12.0, 2.0), (14.0, 5.0)]);
        }
        Icon::Clock => {
            painter.circle_stroke(p(10.0, 10.0), 8.0, stroke);
            line(&[(10.0, 4.0), (10.0, 10.0), (14.0, 12.0)]);
        }
        Icon::AudioMappings => {
            line(&[(2.0, 10.0), (8.0, 10.0), (13.0, 4.0), (18.0, 4.0)]);
            line(&[(8.0, 10.0), (13.0, 16.0), (18.0, 16.0)]);
            for (x, y) in [(2.0, 10.0), (18.0, 4.0), (18.0, 16.0)] {
                painter.circle_filled(p(x, y), 2.0, color);
            }
        }
        Icon::Plus => {
            line(&[(10.0, 3.0), (10.0, 17.0)]);
            line(&[(3.0, 10.0), (17.0, 10.0)]);
        }
        Icon::Inspector => {
            painter.rect_stroke(
                Rect::from_min_max(p(1.0, 3.0), p(19.0, 17.0)),
                2.0,
                stroke,
                egui::StrokeKind::Inside,
            );
            line(&[(12.0, 3.0), (12.0, 17.0)]);
        }
        Icon::Rings => {
            for angle in [-0.8_f32, 0.0, 0.8] {
                let (sa, ca) = angle.sin_cos();
                let points = (0..=40)
                    .map(|i| {
                        let t = i as f32 * std::f32::consts::TAU / 40.0;
                        let x = t.cos() * 9.0;
                        let y = t.sin() * 3.5;
                        p(10.0 + x * ca - y * sa, 10.0 + x * sa + y * ca)
                    })
                    .collect();
                painter.add(egui::Shape::line(points, stroke));
            }
        }
        Icon::Cylinder => {
            for y in [4.0, 16.0] {
                let points = (0..=32)
                    .map(|i| {
                        let t = i as f32 * std::f32::consts::TAU / 32.0;
                        p(10.0 + t.cos() * 7.5, y + t.sin() * 3.0)
                    })
                    .collect();
                painter.add(egui::Shape::line(points, stroke));
            }
            line(&[(2.5, 4.0), (2.5, 16.0)]);
            line(&[(17.5, 4.0), (17.5, 16.0)]);
        }
        Icon::Sphere => {
            painter.circle_stroke(p(10.0, 10.0), 8.5, stroke);
            for vertical in [true, false] {
                let points = (0..=32)
                    .map(|i| {
                        let t = i as f32 * std::f32::consts::TAU / 32.0;
                        p(
                            10.0 + t.cos() * if vertical { 4.0 } else { 8.5 },
                            10.0 + t.sin() * if vertical { 8.5 } else { 3.5 },
                        )
                    })
                    .collect();
                painter.add(egui::Shape::line(points, stroke));
            }
        }
        Icon::Waterfall => {
            line(&[
                (1.0, 7.0),
                (10.0, 2.0),
                (19.0, 7.0),
                (19.0, 15.0),
                (10.0, 20.0),
                (1.0, 15.0),
                (1.0, 7.0),
                (10.0, 12.0),
                (19.0, 7.0),
            ]);
            line(&[(10.0, 12.0), (10.0, 20.0)]);
            line(&[(3.0, 12.0), (5.0, 9.0), (7.0, 13.0)]);
        }
        Icon::Spectrogram => {
            for x in 0..4 {
                for y in 0..4 {
                    let cell = Rect::from_min_size(
                        p(1.0 + x as f32 * 5.0, 1.0 + y as f32 * 5.0),
                        Vec2::splat(3.0),
                    );
                    painter.rect_filled(
                        cell,
                        0.5,
                        color.gamma_multiply(if (x + y) % 3 == 0 { 0.4 } else { 1.0 }),
                    );
                }
            }
        }
        Icon::Flux => {
            for (x, y, radius) in [
                (3.0, 8.0, 1.1),
                (5.0, 13.0, 1.6),
                (9.0, 16.0, 1.0),
                (14.0, 15.0, 1.3),
                (17.0, 10.0, 1.6),
                (14.0, 5.0, 1.1),
                (9.0, 3.0, 1.4),
                (9.0, 9.0, 2.0),
                (3.0, 3.0, 0.7),
                (19.0, 18.0, 0.7),
            ] {
                painter.circle_filled(p(x, y), radius, color);
            }
            line(&[(5.0, 13.0), (9.0, 9.0), (14.0, 5.0)]);
        }
        Icon::Strata => {
            for y in [3.0, 8.0, 13.0] {
                line(&[
                    (1.0, y + 1.0),
                    (10.0, y - 1.0),
                    (19.0, y + 1.0),
                    (10.0, y + 5.0),
                    (1.0, y + 1.0),
                ]);
                line(&[(5.0, y + 0.1), (10.0, y + 2.5), (15.0, y + 0.1)]);
            }
        }
        Icon::Veroni => {
            line(&[
                (1.0, 5.0),
                (7.0, 1.0),
                (14.0, 3.0),
                (19.0, 9.0),
                (16.0, 17.0),
                (7.0, 19.0),
                (1.0, 13.0),
                (1.0, 5.0),
            ]);
            line(&[(7.0, 1.0), (8.0, 8.0), (1.0, 13.0)]);
            line(&[(8.0, 8.0), (14.0, 11.0), (19.0, 9.0)]);
            line(&[(14.0, 11.0), (12.0, 18.0)]);
            for (x, y) in [(4.0, 6.0), (13.0, 6.0), (7.0, 14.0)] {
                painter.circle_filled(p(x, y), 1.3, color);
            }
        }
        Icon::Oscilloscope => {
            painter.circle_stroke(p(10.0, 10.0), 9.0, stroke);
            line(&[
                (2.0, 10.0),
                (5.0, 10.0),
                (7.0, 4.0),
                (10.0, 16.0),
                (13.0, 4.0),
                (15.0, 10.0),
                (18.0, 10.0),
            ]);
        }
        Icon::Fullscreen | Icon::Restore => {
            for (x, y) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                let (outer, inner) = if matches!(icon, Icon::Fullscreen) {
                    (7.0, 3.0)
                } else {
                    (3.0, 7.0)
                };
                line(&[
                    (10.0 + x * inner, 10.0 + y * outer),
                    (10.0 + x * outer, 10.0 + y * outer),
                    (10.0 + x * outer, 10.0 + y * inner),
                ]);
            }
        }
        Icon::Pause => {
            for x in [5.0, 12.0] {
                painter.rect_filled(Rect::from_min_max(p(x, 4.0), p(x + 3.0, 16.0)), 0.5, color);
            }
        }
        Icon::Play => {
            painter.add(egui::Shape::convex_polygon(
                vec![p(5.0, 3.0), p(17.0, 10.0), p(5.0, 17.0)],
                color,
                Stroke::NONE,
            ));
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
        Icon::Waveform => line(&[
            (1.0, 10.0),
            (4.0, 10.0),
            (6.0, 3.0),
            (9.0, 17.0),
            (12.0, 5.0),
            (15.0, 10.0),
            (19.0, 10.0),
        ]),
        Icon::Eye => {
            line(&[
                (1.0, 10.0),
                (5.0, 5.0),
                (10.0, 3.0),
                (15.0, 5.0),
                (19.0, 10.0),
                (15.0, 15.0),
                (10.0, 17.0),
                (5.0, 15.0),
                (1.0, 10.0),
            ]);
            painter.circle_stroke(p(10.0, 10.0), 3.0, stroke);
        }
        Icon::Close => {
            line(&[(5.0, 5.0), (15.0, 15.0)]);
            line(&[(15.0, 5.0), (5.0, 15.0)]);
        }
        Icon::Stereo => {
            painter.circle_stroke(p(6.0, 10.0), 5.0, stroke);
            painter.circle_stroke(p(14.0, 10.0), 5.0, stroke);
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
    fn pane_inspect_icon_is_compact_and_clickable() {
        for icon in [Icon::Eye] {
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
                        let response = sized_button(ui, icon, false, 24.0, "Inspect panel");
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
            assert!(!paths.is_empty());
            assert!(
                output
                    .shapes
                    .iter()
                    .any(|shape| matches!(&shape.shape, egui::Shape::Circle(_)))
            );
            assert!(
                paths
                    .iter()
                    .flatten()
                    .all(|point| rect.shrink(3.0).contains(*point))
            );
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
    }
}
