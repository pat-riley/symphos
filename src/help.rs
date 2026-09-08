//! Shared hover descriptions, displayed in the docked Info View instead of popups.
use eframe::egui::{self, Rect, Response, RichText, Vec2};

const PROMPT: &str = "Hover over a control or visualization to see what it does.";

#[derive(Clone, Default)]
struct HoveredHelp {
    text: String,
    area: f32,
}

fn id() -> egui::Id {
    egui::Id::new("info-view-hover")
}

pub fn begin_frame(ui: &mut egui::Ui) {
    ui.ctx().data_mut(|data| data.remove::<HoveredHelp>(id()));
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
    let response = ui.selectable_label(*open, "? Help")
        .help_text("Show or hide Info View at the bottom left. Hover over controls to read their descriptions without popups.");
    if response.clicked() {
        *open = !*open;
    }
    response
}

pub trait HoverHelp {
    fn help_text(self, text: impl Into<String>) -> Self;
}

impl HoverHelp for Response {
    fn help_text(self, text: impl Into<String>) -> Self {
        if self.hovered() || self.dragged() {
            let area = self.interact_rect.area();
            self.ctx.data_mut(|data| {
                let previous = data.get_temp::<HoveredHelp>(id());
                // Prefer a specific control over its enclosing panel/viewport.
                if previous.is_none_or(|previous| area <= previous.area) {
                    data.insert_temp(
                        id(),
                        HoveredHelp {
                            text: text.into(),
                            area,
                        },
                    );
                }
            });
        }
        self
    }
}

fn description(context: &egui::Context) -> String {
    context
        .data(|data| data.get_temp::<HoveredHelp>(id()))
        .map_or_else(|| PROMPT.to_owned(), |help| help.text)
}

pub fn draw(ui: &mut egui::Ui, rect: Rect, open: &mut bool) {
    let text = description(ui.ctx());
    let mut panel = ui.new_child(
        egui::UiBuilder::new()
            .id_salt("info-view")
            .max_rect(rect.shrink(10.0)),
    );
    panel.set_clip_rect(rect.intersect(ui.clip_rect()));
    panel.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
    panel.horizontal(|ui| {
        ui.label(RichText::new("INFO VIEW").small().strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .small_button("×")
                .help_text("Hide Info View. Reopen it with Help at the bottom left.")
                .clicked()
            {
                *open = false;
            }
        });
    });
    panel.separator();
    egui::ScrollArea::vertical().show(&mut panel, |ui| {
        ui.add(egui::Label::new(RichText::new(text).small()).wrap());
    });
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
                        .help_text("Specific description")
                        .on_hover_text("Built-in popup must not appear");
                    ui.interact(ui.max_rect(), ui.id().with("parent"), egui::Sense::hover())
                        .help_text("Generic panel description");
                    if time > 0.0 {
                        assert_eq!(description(ui.ctx()), "Specific description");
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
