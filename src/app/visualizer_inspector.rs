//! Shared visualizer inspector: pinned identity, horizontal tabs and flat properties.
use super::*;
#[cfg(test)]
use crate::modules::SettingsSection;

pub(super) fn style(ui: &mut egui::Ui, theme: &AppTheme) {
    workspace_ui::inspector_style(ui, theme);
}

pub(super) fn tabs(ui: &mut egui::Ui, pane: &mut ModulePane) -> egui::Response {
    pane.resolve_inspector_focus();
    let (row, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 30.0), Sense::hover());
    let base = ui.visuals().panel_fill;
    let text = ui.visuals().text_color();
    let inactive = ui.visuals().extreme_bg_color;
    let muted = ui.visuals().weak_text_color();
    let divider = ui.visuals().widgets.noninteractive.bg_stroke;
    ui.painter().rect_filled(row, 0.0, inactive);
    let sections = pane.inspector_sections();
    let width = (row.width() - 30.0) / sections.len() as f32;
    let font_size = if row.width() < 280.0 { 11.0 } else { 12.0 };
    for (i, &section) in sections.iter().enumerate() {
        let rect = Rect::from_min_size(
            row.min + Vec2::new(i as f32 * width, 0.0),
            Vec2::new(width, row.height()),
        );
        let response = ui.interact(
            rect,
            ui.id().with(("visualizer-tab", section.title())),
            Sense::click(),
        );
        let selected = pane.active_inspector_section() == section;
        if selected || response.hovered() {
            ui.painter().rect_filled(
                rect,
                0.0,
                if selected {
                    base
                } else {
                    ui.visuals().widgets.hovered.weak_bg_fill
                },
            );
        }
        ui.painter().vline(rect.right(), rect.y_range(), divider);
        if !selected {
            ui.painter().hline(rect.x_range(), rect.bottom(), divider);
        }
        let galley = egui::WidgetText::from(
            RichText::new(section.title())
                .strong()
                .size(font_size)
                .color(if selected { text } else { muted }),
        )
        .into_galley(
            ui,
            Some(egui::TextWrapMode::Truncate),
            (width - 6.0).max(1.0),
            FontId::proportional(font_size),
        );
        ui.painter()
            .galley(rect.center() - galley.size() * 0.5, galley, text);
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::SelectableLabel, true, section.title())
        });
        if response
            .help_text(format!(
                "{} · {} properties",
                section.title(),
                pane.kind.label()
            ))
            .clicked()
        {
            pane.select_inspector_section(section);
        }
    }
    let menu_rect = Rect::from_min_max(egui::pos2(row.right() - 30.0, row.top()), row.max);
    let menu = ui.interact(
        menu_rect,
        ui.id().with("visualizer-panel-menu"),
        Sense::click(),
    );
    if menu.hovered() {
        ui.painter().rect_filled(menu_rect, 0.0, base);
    }
    for y in [-3.0, 0.0, 3.0] {
        ui.painter().line_segment(
            [
                menu_rect.center() + Vec2::new(-5.0, y),
                menu_rect.center() + Vec2::new(5.0, y),
            ],
            Stroke::new(1.0, text),
        );
    }
    // Separate the pinned identity/source header from the property tabs.
    ui.painter().hline(row.x_range(), row.top() + 0.5, divider);
    menu.help_text("Component actions · presets, copy, paste and reset")
}

impl SymphosApp {
    pub(super) fn component_actions_contents(&mut self, ui: &mut egui::Ui) {
        if ui.button("Browse component presets…").clicked() {
            self.manager.open_presets(Scope::Component);
            ui.close();
        }
        ui.separator();
        if ui.button("Copy settings").clicked() {
            self.settings_clipboard = Some(self.panes[self.selected_pane].export_settings());
            ui.close();
        }
        let compatible = self
            .settings_clipboard
            .as_ref()
            .is_some_and(|s| s.kind() == self.panes[self.selected_pane].kind);
        if ui
            .add_enabled(compatible, egui::Button::new("Paste settings"))
            .clicked()
        {
            self.paste_module_settings();
            ui.close();
        }
        ui.separator();
        if ui
            .button("Reset component")
            .help_text("Restore component defaults. Undo is available from Edit.")
            .clicked()
        {
            self.panes[self.selected_pane].reset_settings();
            ui.close();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_tabs_fit_all_visualizers_and_preserve_theme_and_selection() {
        for (kind, width) in ModuleKind::ALL
            .into_iter()
            .flat_map(|kind| [236.0, 400.0].map(|width| (kind, width)))
        {
            let ctx = egui::Context::default();
            let original = ctx.style_of(egui::Theme::Dark).visuals.panel_fill;
            let mut pane = ModulePane::new(kind);
            let render = |pane: &mut ModulePane, events| {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(
                            Pos2::ZERO,
                            Vec2::new(width, 2200.0),
                        )),
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        workspace_ui::sidebar_style(ui, &AppTheme::default());
                        let shared = ui.visuals().panel_fill;
                        ui.scope(|ui| {
                            style(ui, &AppTheme::default());
                            tabs(ui, pane);
                            ui.data_mut(|d| {
                                d.insert_temp(egui::Id::new("inspector-bounds"), ui.max_rect())
                            });
                            pane.inspector_controls(ui, &AnalysisFrame::default());
                            assert!(ui.min_rect().width() <= width);
                            assert!(!ui.data(|d| {
                                d.get_temp::<bool>(egui::Id::new(crate::modules::FLAT_INSPECTOR))
                                    .unwrap_or(false)
                            }));
                        });
                        assert_eq!(ui.visuals().panel_fill, shared);
                    },
                );
                output.textures_delta.clear();
                assert_eq!(ctx.style_of(egui::Theme::Dark).visuals.panel_fill, original);
                output
            };
            render(&mut pane, vec![]);
            let out = render(&mut pane, vec![]);
            let pos = out
                .shapes
                .iter()
                .find_map(|s| match &s.shape {
                    egui::Shape::Text(t) if t.galley.job.text == "Appearance" => {
                        Some(t.pos + t.galley.size() * 0.5)
                    }
                    _ => None,
                })
                .unwrap();
            render(&mut pane, vec![egui::Event::PointerMoved(pos)]);
            for pressed in [true, false] {
                render(
                    &mut pane,
                    vec![egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    }],
                );
            }
            let out = render(&mut pane, vec![]);
            assert_eq!(pane.active_inspector_section(), SettingsSection::Appearance);
            let mut restored = ModulePane::new(kind);
            restored.restore(&pane.snapshot());
            assert_eq!(
                restored.active_inspector_section(),
                SettingsSection::Appearance
            );
            assert_eq!(
                pane.inspector_sections().len(),
                if pane.sections().contains(&SettingsSection::Camera) {
                    3
                } else {
                    2
                }
            );
            for label in if kind == ModuleKind::Waterfall {
                vec!["Render style", "Palette", "Base floor"]
            } else {
                vec![]
            } {
                assert!(
                    out.shapes.iter().any(
                        |s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.job.text == label)
                    ),
                    "missing {label}"
                );
            }
            assert!(!out.shapes.iter().any(
                |s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.job.text == "Dimensions")
            ));
        }
    }
}
