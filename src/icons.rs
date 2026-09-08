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
    Surface,
    Lines,
    YLines,
    Wireframe,
    Dots,
    Stems,
}

pub fn button(ui: &mut egui::Ui, icon: Icon, selected: bool, description: &str) -> Response {
    let response = ui.add(
        egui::Button::new("")
            .min_size(Vec2::splat(30.0))
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
            painter.circle_stroke(p(10.0, 10.0), 8.0, stroke);
            for (x, y) in [(6.0, 6.0), (13.0, 5.0), (15.0, 11.0), (7.0, 13.0)] {
                painter.circle_filled(p(x, y), 1.6, color);
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
        Icon::Collapse => line(&[(13.0, 3.0), (6.0, 10.0), (13.0, 17.0)]),
        Icon::Expand => line(&[(7.0, 3.0), (14.0, 10.0), (7.0, 17.0)]),
        Icon::Stereo => {
            painter.circle_stroke(p(6.0, 10.0), 5.0, stroke);
            painter.circle_stroke(p(14.0, 10.0), 5.0, stroke);
        }
        Icon::Mix => {
            painter.circle_stroke(p(10.0, 10.0), 6.0, stroke);
        }
        Icon::Settings => {
            for (y, knob) in [(4.0, 6.0), (10.0, 14.0), (16.0, 8.0)] {
                line(&[(2.0, y), (18.0, y)]);
                painter.circle_filled(p(knob, y), 2.3, color);
            }
        }
    }
}
