//! Macro details use the same property rows as component and signal inspectors.
use super::*;
use crate::help::HoverHelp;
use crate::{
    icons::Icon,
    modules::{ModulePane, inspector_section, property_row},
    workspace_ui,
};
use scene::{InspectAction, SceneMacros};

impl SceneMacros {
    pub fn inspector(
        &mut self,
        ui: &mut egui::Ui,
        index: usize,
        theme: &AppTheme,
        panes: &mut [ModulePane],
        shared: &signals::ui::Shared,
    ) -> InspectAction {
        let Some(slot) = self.slots.get(index) else {
            return InspectAction::None;
        };
        workspace_ui::sidebar_style(ui, theme);
        let title = slot_title(slot, index);
        if workspace_ui::sidebar_header(
            ui,
            Icon::AudioMappings,
            &title,
            "Macro inspector",
            &format!("Scene macro {}", index + 1),
            "Close the macro inspector",
        )
        .clicked()
        {
            return InspectAction::Close;
        }
        let bounds = ui.available_rect_before_wrap();
        let (bank, snapshot) = {
            let panel = shared.lock().unwrap_or_else(|e| e.into_inner());
            (panel.bank.clone(), panel.snapshot.clone())
        };
        let target = slot.target.clone();
        let pane_index = target
            .as_ref()
            .and_then(|target| panes.iter().position(|p| p.instance_id() == target.pane));
        let mut settings = pane_index.and_then(|i| {
            let target = target.as_ref()?;
            panes[i]
                .snapshot()
                .settings
                .into_iter()
                .find(|s| s.kind() == target.kind)
        });
        let original = settings.clone();
        let effective = target
            .as_ref()
            .zip(settings.as_ref())
            .map(|(target, settings)| {
                scene::state_for(&self.slots, target).effective(
                    target.kind,
                    settings,
                    Some(&snapshot),
                )
            });
        let mut action = InspectAction::None;
        ui.data_mut(|d| {
            d.insert_temp(egui::Id::new("inspector-bounds"), bounds);
            d.insert_temp(egui::Id::new("property-row-index"), 0_usize);
        });
        let slot = &mut self.slots[index];
        egui::ScrollArea::vertical()
            .id_salt(("macro-inspector", index))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                inspector_section(ui, "Macro", |ui| {
                    property_row(ui, "Name", |ui| {
                        let mut name = slot.name.clone().unwrap_or_default();
                        if ui.add(egui::TextEdit::singleline(&mut name)
                            .hint_text("Automatic").char_limit(80)
                            .desired_width(ui.available_width())).changed()
                        {
                            if name.trim().is_empty() {
                                slot.name = None;
                            } else if crate::workspace::valid_name(&name) {
                                slot.name = Some(name);
                            }
                        }
                    });
                    property_row(ui, "Color", |ui| {
                        let mut automatic = slot.color.is_none();
                        if ui.checkbox(&mut automatic, "Auto").changed() {
                            let [r, g, b, _] = theme.accent.to_array();
                            slot.color = (!automatic).then_some([r, g, b]);
                        }
                        if let Some(color) = &mut slot.color {
                            ui.color_edit_button_srgb(color);
                        }
                    });
                    if let Some(parameter) = &slot.parameter {
                        value_row(ui, "Parameter", &parameter.label);
                        if let Some(target) = &target {
                            value_row(ui, "Component", &format!("{} · {}", pane_index.map_or(0, |i| i + 1), target.kind.label()));
                            if pane_index.is_some_and(|i| panes[i].kind != target.kind) {
                                ui.weak("Target component type is inactive.");
                            }
                        }
                    } else {
                        ui.weak("Empty macro. Drag a parameter from a component inspector onto this slot.");
                    }
                });
                if let Some(parameter) = slot.parameter.clone() {
                    inspector_section(ui, "Parameter", |ui| {
                        if let Some(settings) = settings.as_mut() {
                            let base = get(settings, &parameter.field).unwrap_or(parameter.default * parameter.scale) / parameter.scale;
                            let live = effective.as_ref().and_then(|v| v.iter().find(|(field, _)| field == &parameter.field))
                                .map_or(base, |(_, value)| value / parameter.scale);
                            property_row(ui, "Manual value", |ui| {
                                let mut value = base;
                                if ui.add(egui::DragValue::new(&mut value)
                                    .speed((parameter.max - parameter.min) / 500.0)
                                    .range(parameter.safe_min..=parameter.safe_max)
                                    .max_decimals(if parameter.integer { 0 } else { 3 })).changed()
                                {
                                    set(settings, &parameter.field, value * parameter.scale);
                                }
                            });
                            value_row(ui, "Live value", &format!("{live:.3}"));
                            value_row(ui, "Knob range", &format!("{:.3} – {:.3}", parameter.min, parameter.max));
                            value_row(ui, "Default", &format!("{:.3}", parameter.default));
                            if ui.button("Reset value").clicked() {
                                set(settings, &parameter.field, parameter.default * parameter.scale);
                            }
                        } else {
                            ui.weak("Target component unavailable.");
                        }
                        if ui.button("Remove assignment").clicked() {
                            slot.parameter = None;
                            slot.target = None;
                        }
                    });
                }
                inspector_section(ui, "Modulation", |ui| {
                    property_row(ui, "Source", |ui| {
                        let selected = slot.signal.and_then(|id| bank.channels.iter().find(|d| d.id == id))
                            .map_or("None", |d| d.name.as_str());
                        let previous = slot.signal;
                        egui::ComboBox::from_id_salt("macro-inspector-source")
                            .width(ui.available_width()).selected_text(selected)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut slot.signal, None, "None");
                                for source in &bank.channels {
                                    ui.selectable_value(&mut slot.signal, Some(source.id), &source.name);
                                }
                            });
                        if slot.signal != previous { slot.enabled = true; }
                    });
                    ui.add_enabled_ui(slot.signal.is_some(), |ui| {
                        property_row(ui, "Enabled", |ui| { ui.checkbox(&mut slot.enabled, ""); });
                        property_row(ui, "Strength", |ui| {
                            let mut depth = slot.depth * 100.0;
                            if ui.add(egui::DragValue::new(&mut depth).range(-1200.0..=1200.0)
                                .speed(0.5).suffix("%").max_decimals(1)).changed()
                            { slot.depth = depth / 100.0; }
                        });
                        property_row(ui, "Invert", |ui| { ui.checkbox(&mut slot.invert, ""); });
                        property_row(ui, "Range start", |ui| {
                            ui.add(egui::DragValue::new(&mut slot.output_min).range(0.0..=slot.output_max).speed(0.01));
                        });
                        property_row(ui, "Range end", |ui| {
                            ui.add(egui::DragValue::new(&mut slot.output_max).range(slot.output_min..=1.0).speed(0.01));
                        });
                    });
                    if let Some(id) = slot.signal {
                        if let Some(source) = bank.channels.iter().find(|d| d.id == id) {
                            value_row(ui, "Detector", source.method.label());
                        }
                        if let Some(sample) = snapshot.values.get(&id) {
                            value_row(ui, "Status", sample.status.label());
                            value_row(ui, "Signal output", &format!("{:+.3}", sample.value));
                        }
                        if ui.button("Inspect source").clicked() {
                            action = InspectAction::Signal(id);
                        }
                    } else if bank.channels.is_empty() {
                        ui.weak("Create modulation sources in Signals.");
                    }
                });
            });
        ui.data_mut(|d| {
            d.remove::<Rect>(egui::Id::new("inspector-bounds"));
        });
        if settings != original
            && let Some((i, settings)) = pane_index.zip(settings)
        {
            panes[i].apply_macro_settings(&settings);
        }
        action
    }
}

fn value_row(ui: &mut egui::Ui, label: &str, value: &str) {
    property_row(ui, label, |ui| {
        ui.add(egui::Label::new(value).truncate())
            .help_fallback(|| {
                format!(
                    "{label}\n{}\n{value}",
                    crate::help::control_description(label)
                )
            });
    });
}
