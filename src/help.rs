//! Shared hover descriptions, displayed in the docked Info View instead of popups.
use eframe::egui::{self, Rect, Response, RichText, Vec2};
mod descriptions;
pub use descriptions::control_description;

const PROMPT: &str = "Hover over a control or visualization to see what it does.";

#[derive(Clone, Default)]
struct HoveredHelp {
    text: String,
    area: f32,
    priority: u8,
    pointer: Option<egui::Pos2>,
}

fn id() -> egui::Id {
    egui::Id::new("info-view-hover")
}

pub fn begin_frame(ui: &mut egui::Ui) {
    ui.ctx().data_mut(|data| {
        let previous = egui::Id::new("info-view-previous");
        if let Some(help) = data.remove_temp::<HoveredHelp>(id()) {
            data.insert_temp(previous, help);
        } else {
            data.remove::<HoveredHelp>(previous);
        }
    });
    // Also suppress egui's built-in truncated-label / widget explanation popups.
    // Descriptions remain at their call sites and flow through HoverHelp below.
    suppress_tooltips(ui.style_mut());
}

pub fn suppress_tooltips(style: &mut egui::Style) {
    style.explanation_tooltips = false;
    style.interaction.tooltip_delay = f32::INFINITY;
    style.interaction.tooltip_grace_time = 0.0;
}

pub fn toggle(ui: &mut egui::Ui, open: &mut bool) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(26.0), egui::Sense::click());
    let color = if *open || response.hovered() {
        ui.visuals().selection.bg_fill
    } else {
        ui.visuals().weak_text_color()
    };
    let painter = ui.painter();
    painter.circle_filled(rect.center(), 9.5, color);
    let ink = if color.r() as u32 + color.g() as u32 + color.b() as u32 > 380 {
        egui::Color32::from_gray(20)
    } else {
        egui::Color32::WHITE
    };
    painter.circle_filled(rect.center() - Vec2::new(0.0, 4.0), 1.2, ink);
    painter.rect_filled(
        Rect::from_center_size(rect.center() + Vec2::new(0.0, 2.0), Vec2::new(2.3, 6.0)),
        0.5,
        ink,
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Show / hide Info View")
    });
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand)
        .help_text("Show / hide Info View\nHover over controls to read their purpose and gestures. Info View fills the bottom dock when open alone, or sits beside Macros or Signals while either dock is open. Shortcut: F1");
    if response.clicked() {
        *open = !*open;
    }
    response
}

pub trait HoverHelp {
    fn help_text(self, text: impl Into<String>) -> Self;
    fn help_text_with(self, text: impl FnOnce() -> String) -> Self;
    fn help_fallback(self, text: impl FnOnce() -> String) -> Self;
}

impl HoverHelp for Response {
    fn help_text(self, text: impl Into<String>) -> Self {
        self.help_text_with(|| text.into())
    }

    fn help_text_with(self, text: impl FnOnce() -> String) -> Self {
        register(self, text, 1)
    }

    fn help_fallback(self, text: impl FnOnce() -> String) -> Self {
        register(self, text, 0)
    }
}

fn register(response: Response, text: impl FnOnce() -> String, priority: u8) -> Response {
    if response.hovered() || response.dragged() {
        let area = response.interact_rect.area();
        let preferred = response.ctx.data(|data| {
            let previous = data.get_temp::<HoveredHelp>(id());
            // Prefer a specific control over its enclosing panel/viewport.
            previous.is_none_or(|previous| {
                priority > previous.priority
                    || (priority == previous.priority && area <= previous.area)
            })
        });
        if preferred {
            // Evaluate outside the context lock so callers can read UI state.
            let text = text();
            let pointer = response.ctx.input(|i| i.pointer.hover_pos());
            response.ctx.data_mut(|data| {
                data.insert_temp(
                    id(),
                    HoveredHelp {
                        text,
                        area,
                        priority,
                        pointer,
                    },
                );
            });
        }
    }
    response
}

fn description(context: &egui::Context) -> String {
    let pointer = context.input(|i| i.pointer.hover_pos());
    context
        .data(|data| {
            data.get_temp::<HoveredHelp>(id()).or_else(|| {
                // Modal settings and menus are drawn after the workspace. Their
                // descriptions become visible next frame while the pointer stays put.
                data.get_temp::<HoveredHelp>(egui::Id::new("info-view-previous"))
                    .filter(|help| help.pointer == pointer)
            })
        })
        .map_or_else(|| PROMPT.to_owned(), |help| help.text)
}

pub fn draw(ui: &mut egui::Ui, rect: Rect, open: &mut bool) {
    let mut text = description(ui.ctx());
    let last_key = egui::Id::new("info-view-reading");
    if ui.rect_contains_pointer(rect) {
        if let Some(last) = ui.data(|d| d.get_temp::<String>(last_key)) {
            text = last;
        }
    } else {
        ui.data_mut(|d| d.insert_temp(last_key, text.clone()));
    }
    let inner = rect.shrink(12.0);
    if !inner.is_positive() {
        return;
    }
    let mut panel = ui.new_child(egui::UiBuilder::new().id_salt("info-view").max_rect(inner));
    panel.set_clip_rect(inner.intersect(ui.clip_rect()));
    let header = Rect::from_min_size(inner.min, Vec2::new(inner.width(), 24.0));
    let close_rect =
        Rect::from_min_size(header.right_top() - Vec2::new(24.0, 0.0), Vec2::splat(24.0));
    let mut title = panel.new_child(
        egui::UiBuilder::new()
            .id_salt("title")
            .max_rect(Rect::from_min_max(
                header.min,
                close_rect.left_bottom() - Vec2::new(8.0, 0.0),
            ))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    title.add(
        egui::Label::new(RichText::new("INFO VIEW").small().strong())
            .truncate()
            .selectable(false),
    );
    let mut tools = panel.new_child(
        egui::UiBuilder::new()
            .id_salt("close")
            .max_rect(close_rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    if crate::icons::toolbar_toggle(
        &mut tools,
        crate::icons::Icon::Close,
        false,
        24.0,
        "Hide Info View. Reopen it with the circular info button at the bottom left, or press F1.",
    )
    .clicked()
    {
        *open = false;
    }
    let divider_y = panel
        .painter()
        .round_to_pixel_center(header.bottom() + 10.0);
    panel.painter().hline(
        inner.x_range(),
        divider_y,
        egui::Stroke::new(
            1.0 / panel.ctx().pixels_per_point(),
            panel.visuals().widgets.noninteractive.bg_stroke.color,
        ),
    );
    let body = Rect::from_min_max(egui::pos2(inner.left(), divider_y + 10.0), inner.max);
    if body.is_positive() {
        let mut content = panel.new_child(egui::UiBuilder::new().id_salt("body").max_rect(body));
        content.set_clip_rect(body.intersect(panel.clip_rect()));
        egui::ScrollArea::vertical().show(&mut content, |ui| {
            ui.add(egui::Label::new(RichText::new(text).small()).wrap());
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_toggle_shows_and_hides_the_docked_panel() {
        let context = egui::Context::default();
        let mut open = false;
        let render = |open: &mut bool, events| {
            let mut button = Rect::NOTHING;
            let mut output = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(700.0, 500.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    begin_frame(ui);
                    button = toggle(ui, open).rect;
                    if *open {
                        draw(
                            ui,
                            Rect::from_min_size(egui::pos2(12.0, 300.0), egui::vec2(236.0, 160.0)),
                            open,
                        );
                    }
                },
            );
            output.textures_delta.clear();
            let panel_visible = output.shapes.iter().any(|shape| {
                matches!(&shape.shape,
                    egui::Shape::Text(text) if text.galley.job.text == "INFO VIEW"
                )
            });
            (button.center(), panel_visible)
        };
        let (position, visible) = render(&mut open, vec![]);
        assert!(!visible);
        for expected in [true, false, true] {
            for pressed in [true, false] {
                render(
                    &mut open,
                    vec![
                        egui::Event::PointerMoved(position),
                        egui::Event::PointerButton {
                            pos: position,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                );
            }
            assert_eq!(open, expected);
            assert_eq!(render(&mut open, vec![]).1, expected);
        }
    }

    #[test]
    fn specific_control_help_wins_and_builtin_popups_stay_suppressed() {
        let context = egui::Context::default();
        for theme in [egui::Theme::Light, egui::Theme::Dark] {
            context.style_mut_of(theme, suppress_tooltips);
        }
        for time in [0.0, 1.0, 10.0] {
            let specific_calls = std::cell::Cell::new(0);
            let parent_calls = std::cell::Cell::new(0);
            let mut output = context.run_ui(
                egui::RawInput {
                    time: Some(time),
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(500.0, 300.0),
                    )),
                    events: vec![egui::Event::PointerMoved(egui::pos2(20.0, 20.0))],
                    ..Default::default()
                },
                |ui| {
                    begin_frame(ui);
                    ui.button("Control")
                        .help_text_with(|| {
                            specific_calls.set(specific_calls.get() + 1);
                            // Lazy descriptions may safely inspect context state.
                            ui.input(|input| assert_eq!(input.time, time));
                            "Specific description".into()
                        })
                        .on_hover_text("Built-in popup must not appear");
                    ui.interact(ui.max_rect(), ui.id().with("parent"), egui::Sense::hover())
                        .help_text_with(|| {
                            parent_calls.set(parent_calls.get() + 1);
                            "Generic panel description".into()
                        });
                    if time > 0.0 {
                        assert_eq!(description(ui.ctx()), "Specific description");
                        assert_eq!(specific_calls.get(), 1);
                        assert_eq!(parent_calls.get(), 0);
                    }
                },
            );
            output.textures_delta.clear();
            assert!(output.shapes.iter().all(|shape| !matches!(&shape.shape,
                egui::Shape::Text(text) if text.galley.job.text == "Built-in popup must not appear"
            )));
        }
    }

    #[test]
    fn semantic_help_overrides_small_generic_controls_and_survives_late_drawing() {
        let context = egui::Context::default();
        for frame in 0..4 {
            let position = if frame == 3 {
                egui::pos2(450.0, 250.0)
            } else {
                egui::pos2(20.0, 20.0)
            };
            let mut output = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(500.0, 300.0),
                    )),
                    events: vec![egui::Event::PointerMoved(position)],
                    ..Default::default()
                },
                |ui| {
                    begin_frame(ui);
                    if frame == 2 {
                        assert_eq!(description(ui.ctx()), "What this parameter means");
                    } else if frame == 3 {
                        assert_eq!(description(ui.ctx()), PROMPT);
                    }
                    ui.button("Control")
                        .help_fallback(|| "Drag to adjust".into());
                    ui.interact(
                        ui.max_rect(),
                        ui.id().with("semantic"),
                        egui::Sense::hover(),
                    )
                    .help_text("What this parameter means");
                    if frame == 1 || frame == 2 {
                        assert_eq!(description(ui.ctx()), "What this parameter means");
                    }
                },
            );
            output.textures_delta.clear();
        }
    }

    #[test]
    fn descriptions_follow_hover_and_clear_without_popup_layers() {
        let context = egui::Context::default();
        let mut target = Rect::NOTHING;
        for (position, expected) in [
            (None, PROMPT),
            (Some(egui::pos2(20.0, 20.0)), "Saved tooltip text"),
            (Some(egui::pos2(450.0, 250.0)), PROMPT),
        ] {
            let mut output = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(500.0, 300.0),
                    )),
                    events: position
                        .map(|position| vec![egui::Event::PointerMoved(position)])
                        .unwrap_or_default(),
                    ..Default::default()
                },
                |ui| {
                    begin_frame(ui);
                    target = ui
                        .button("Hover target")
                        .help_text("Saved tooltip text")
                        .rect;
                    assert_eq!(description(ui.ctx()), expected);
                },
            );
            output.textures_delta.clear();
            assert!(target.is_positive());
            assert!(output.shapes.iter().all(|shape| !matches!(&shape.shape, egui::Shape::Text(text) if text.galley.job.text == "Saved tooltip text")));
        }
    }
}
