//! One manual value + one modulation source, shared by the three artistic visualizers.
use crate::{modulation::Target, theme::AppTheme};
#[cfg(test)]
use crate::{
    modulation::{ModulationSettings, Route, Visualizer},
    signals::{self, OutputKind},
};
use egui::{self, Pos2, Rect, Stroke, Vec2};

#[derive(Clone, Copy)]
pub(super) struct Control {
    pub label: &'static str,
    pub target: Target,
    pub min: f32,
    pub max: f32,
    pub default: f32,
}
impl Control {
    pub const fn new(
        label: &'static str,
        target: Target,
        min: f32,
        max: f32,
        default: f32,
    ) -> Self {
        Self {
            label,
            target,
            min,
            max,
            default,
        }
    }
    #[cfg(test)]
    pub fn normalize(self, v: f32) -> f32 {
        ((v - self.min) / (self.max - self.min)).clamp(0.0, 1.0)
    }
    pub fn effective(self, base: f32, modulation: f32) -> f32 {
        (base + modulation * (self.max - self.min)).clamp(self.min, self.max)
    }
}
#[cfg(test)]
pub(super) fn height(open: bool) -> f32 {
    if open { 234.0 } else { 36.0 }
}
#[cfg(test)]
#[derive(Default)]
pub(super) struct Action {
    pub inspect: bool,
    pub burst: bool,
    pub selected: Option<usize>,
    pub preset: Option<usize>,
}
/// Duplicate legacy contributions stay in the project, disabled, for recovery in Advanced.
/// Only the first route to each performance destination contributes.
#[cfg(test)]
pub(super) fn single_sources(settings: &mut ModulationSettings, controls: &[Control; 12]) {
    for c in controls {
        let mut seen = false;
        for route in settings
            .routes
            .iter_mut()
            .filter(|r| r.target == c.target && r.enabled)
        {
            if seen {
                route.enabled = false;
            } else {
                seen = true;
            }
        }
    }
}
#[cfg(test)]
#[derive(Clone, Copy, PartialEq, Eq)]
enum MacroSource {
    Channel(u64),
    LayerBands,
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(super) fn bar(
    ui: &mut egui::Ui,
    rect: Rect,
    theme: &AppTheme,
    pane: usize,
    name: &str,
    controls: &[Control; 12],
    values: &mut [f32; 12],
    effective: [f32; 12],
    settings: &mut ModulationSettings,
    open: &mut bool,
    presets: &[&str; 3],
) -> Action {
    single_sources(settings, controls);
    let mut action = Action::default();
    ui.painter().rect_filled(rect, 0.0, theme.panel);
    ui.painter().hline(
        rect.x_range(),
        rect.top(),
        Stroke::new(1.0, theme.foreground.gamma_multiply(0.18)),
    );
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .id_salt(("performance", name, pane))
            .max_rect(rect.shrink2(Vec2::new(8.0, 6.0))),
    );
    child.set_clip_rect(rect.intersect(ui.clip_rect()));
    child.spacing_mut().item_spacing = Vec2::new(6.0, 4.0);
    child.spacing_mut().interact_size.y = 24.0;
    child.spacing_mut().button_padding = Vec2::new(8.0, 4.0);
    child.style_mut().override_font_id = Some(egui::FontId::proportional(11.0));
    child.horizontal(|ui| {
        if ui
            .button(if *open {
                format!("{name} macros")
            } else {
                format!("Show {name} macros")
            })
            .clicked()
        {
            *open = !*open;
        }
        if *open {
            ui.menu_button("Reaction preset", |ui| {
                ui.small("Changes macro values and strength; keeps your assigned sources.");
                for (i, preset) in presets.iter().enumerate() {
                    if ui.button(*preset).clicked() {
                        action.preset = Some(i);
                        ui.close();
                    }
                }
            });
            ui.label(format!("Panel {}", pane + 1));
            if name == "Flux" {
                let shape = (values[7] * 4.0).round().clamp(0.0, 4.0) as usize;
                ui.menu_button(
                    if values[6] < 0.05 {
                        "Cloud"
                    } else {
                        ["Rings", "Helix", "Knot", "Vortex", "Petals"][shape]
                    },
                    |ui| {
                        if ui.button("Cloud").clicked() {
                            values[6] = 0.0;
                            ui.close();
                        }
                        for (i, name) in ["Rings", "Helix", "Knot", "Vortex", "Petals"]
                            .into_iter()
                            .enumerate()
                        {
                            if ui.button(name).clicked() {
                                values[6] = 0.9;
                                values[7] = i as f32 / 4.0;
                                ui.close();
                            }
                        }
                    },
                );
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_sized([56.0, 24.0], egui::Button::new("Hide"))
                    .clicked()
                {
                    *open = false;
                }
                action.inspect = ui
                    .add_sized([80.0, 24.0], egui::Button::new("Advanced"))
                    .clicked();
                if name == "Flux" {
                    action.burst = ui
                        .add_sized([56.0, 24.0], egui::Button::new("Burst"))
                        .clicked();
                }
            });
        }
    });
    if !*open {
        return action;
    }
    let shared =
        ui.data(|d| d.get_temp::<signals::ui::Shared>(egui::Id::new(signals::ui::CONTEXT_KEY)));
    let width = ((child.available_width() - 66.0) / 12.0).max(96.0);
    egui::ScrollArea::horizontal()
        .id_salt("macro-row")
        .max_height(174.0)
        .auto_shrink([false, true])
        .show(&mut child, |ui| {
            ui.horizontal(|ui| {
                for (i, c) in controls.iter().copied().enumerate() {
                    ui.push_id(c.target, |ui| {
                        ui.vertical(|ui| {
                            ui.set_width(width);
                            let route_index = settings
                                .routes
                                .iter()
                                .position(|r| r.target == c.target && r.enabled)
                                .or_else(|| {
                                    settings.routes.iter().position(|r| r.target == c.target)
                                });
                            let mut amount = route_index.map_or(0.0, |j| settings.routes[j].amount);
                            let previous_amount = amount;
                            let (source, color, source_range) = shared.as_ref().map_or(
                                ("Connect…".into(), theme.accent, (0.0, 1.0)),
                                |shared| {
                                    let panel = shared.lock().unwrap_or_else(|e| e.into_inner());
                                    route_index.map_or(
                                        ("Connect…".into(), theme.accent, (0.0, 1.0)),
                                        |j| {
                                            let route = &settings.routes[j];
                                            match route.signal {
                                                Some(id) => panel
                                                    .bank
                                                    .channels
                                                    .iter()
                                                    .find(|d| d.id == id)
                                                    .map_or(
                                                        (
                                                            "Missing signal".into(),
                                                            theme.muted,
                                                            (0.0, 0.0),
                                                        ),
                                                        |d| {
                                                            let peak = d.influence
                                                                * if d.input == signals::Input::Lfo
                                                                {
                                                                    d.lfo.depth
                                                                } else {
                                                                    1.0
                                                                };
                                                            let low = if d.kind()
                                                                == OutputKind::Bipolar
                                                            {
                                                                -peak
                                                            } else {
                                                                0.0
                                                            };
                                                            (
                                                                d.name.clone(),
                                                                signals::ui::channel_color(d),
                                                                (low, peak),
                                                            )
                                                        },
                                                    ),
                                                None if c.target.layered() => {
                                                    ("Layer bands".into(), theme.accent, (0.0, 1.0))
                                                }
                                                None => {
                                                    ("Connect…".into(), theme.accent, (0.0, 0.0))
                                                }
                                            }
                                        },
                                    )
                                },
                            );
                            let range = route_index.map_or((0.0, 1.0), |j| {
                                let r = &settings.routes[j];
                                let map = |v: f32| {
                                    r.output_min
                                        + (if r.invert { -v } else { v })
                                            * (r.output_max - r.output_min)
                                };
                                (map(source_range.0), map(source_range.1))
                            });
                            let base = c.normalize(values[i]);
                            let mut v = base;
                            let knob = knob(
                                ui,
                                c.label,
                                &mut v,
                                c.normalize(c.default),
                                c.normalize(effective[i]),
                                &mut amount,
                                if route_index.is_some_and(|j| settings.routes[j].enabled) {
                                    settings.amount
                                } else {
                                    0.0
                                },
                                width,
                                theme,
                                color,
                                range,
                            );
                            if knob.changed() {
                                values[i] = c.min + v * (c.max - c.min);
                            }
                            if knob.clicked() || knob.dragged() || amount != previous_amount {
                                action.selected = Some(i);
                            }
                            if let Some(j) = route_index {
                                settings.routes[j].amount = amount;
                            }
                            let badge = ui
                                .add_sized(
                                    [width, 24.0],
                                    egui::Button::new(egui::RichText::new(source).color(color))
                                        .truncate(),
                                )
                                .on_hover_text(
                                    "Assign or disconnect this macro’s modulation source",
                                );
                            // The editor owns its open state. Nested ComboBoxes use
                            // egui's single memory-backed popup slot for their menus.
                            let editor_key = egui::Id::new("active-macro-source-editor");
                            let mut popup_open =
                                ui.data(|d| d.get_temp::<egui::Id>(editor_key)) == Some(badge.id);
                            if badge.clicked() {
                                action.selected = Some(i);
                                popup_open = !popup_open;
                                if popup_open {
                                    ui.data_mut(|d| d.insert_temp(editor_key, badge.id));
                                }
                            }
                            let nested_menu_was_open = egui::Popup::is_any_open(ui.ctx());
                            let editor = egui::Popup::from_response(&badge)
                                .open_bool(&mut popup_open)
                                .close_behavior(egui::PopupCloseBehavior::IgnoreClicks)
                                .width(280.0)
                                .show(|ui| {
                                    ui.strong(format!("{name} · {}", c.label));
                                    let Some(shared) = &shared else {
                                        ui.label("Signal engine unavailable.");
                                        return;
                                    };
                                    let panel = shared.lock().unwrap_or_else(|e| e.into_inner());
                                    let mut assignment = route_index.and_then(|j| {
                                        let r = &settings.routes[j];
                                        r.signal.map(MacroSource::Channel).or_else(|| {
                                            (c.target.layered() && r.layer == 0)
                                                .then_some(MacroSource::LayerBands)
                                        })
                                    });
                                    let before = assignment;
                                    let selected = match assignment {
                                        Some(MacroSource::Channel(id)) => panel
                                            .bank
                                            .channels
                                            .iter()
                                            .find(|d| d.id == id)
                                            .map_or("Choose source", |d| d.name.as_str()),
                                        Some(MacroSource::LayerBands) => "Automatic layer bands",
                                        None => "Choose source",
                                    };
                                    ui.label("Source");
                                    egui::ComboBox::from_id_salt("source")
                                        .selected_text(selected)
                                        .width(260.0)
                                        .show_ui(ui, |ui| {
                                            ui.selectable_value(
                                                &mut assignment,
                                                None,
                                                "No modulation",
                                            );
                                            if c.target.layered() {
                                                ui.selectable_value(
                                                    &mut assignment,
                                                    Some(MacroSource::LayerBands),
                                                    "Automatic layer bands",
                                                );
                                            }
                                            for d in &panel.bank.channels {
                                                ui.selectable_value(
                                                    &mut assignment,
                                                    Some(MacroSource::Channel(d.id)),
                                                    &d.name,
                                                );
                                            }
                                        });
                                    if assignment != before {
                                        if let Some(source) = assignment {
                                            let j = route_index.unwrap_or_else(|| {
                                                let id = settings
                                                    .routes
                                                    .iter()
                                                    .map(|r| r.id)
                                                    .max()
                                                    .unwrap_or(0)
                                                    + 1;
                                                settings.routes.push(Route {
                                                    id,
                                                    target: c.target,
                                                    amount: 0.35,
                                                    enabled: false,
                                                    ..Default::default()
                                                });
                                                settings.routes.len() - 1
                                            });
                                            let r = &mut settings.routes[j];
                                            r.signal = match source {
                                                MacroSource::Channel(id) => Some(id),
                                                MacroSource::LayerBands => None,
                                            };
                                            r.enabled = true;
                                            r.layer = 0;
                                        } else {
                                            settings.routes.retain(|r| r.target != c.target);
                                        }
                                    }
                                    if panel.bank.channels.is_empty() {
                                        ui.weak("Add sources in Signals.");
                                    }
                                    if ui
                                        .add_enabled(
                                            assignment.is_some(),
                                            egui::Button::new("Disconnect"),
                                        )
                                        .clicked()
                                    {
                                        settings.routes.retain(|r| r.target != c.target);
                                        ui.close();
                                    }
                                });
                            // A combo menu may extend outside the editor. Its selection
                            // click belongs to the combo, not to outside-click dismissal.
                            if !badge.clicked()
                                && !nested_menu_was_open
                                && !egui::Popup::is_any_open(ui.ctx())
                                && editor
                                    .as_ref()
                                    .is_some_and(|e| e.response.clicked_elsewhere())
                            {
                                popup_open = false;
                            }
                            if !popup_open
                                && ui.data(|d| d.get_temp::<egui::Id>(editor_key)) == Some(badge.id)
                            {
                                ui.data_mut(|d| d.remove::<egui::Id>(editor_key));
                            }
                            let route_index = settings
                                .routes
                                .iter()
                                .position(|r| r.target == c.target && r.enabled)
                                .or_else(|| {
                                    settings.routes.iter().position(|r| r.target == c.target)
                                });
                            ui.allocate_ui_with_layout(
                                Vec2::new(width, 20.0),
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    ui.spacing_mut().interact_size.y = 20.0;
                                    if let Some(j) = route_index.filter(|j| {
                                        settings.routes[*j].signal.is_some() || c.target.layered()
                                    }) {
                                        if modulation_depth(ui, &mut settings.routes[j]).changed() {
                                            action.selected = Some(i);
                                        }
                                    } else {
                                        ui.add_sized(
                                            [width, 20.0],
                                            egui::Label::new(
                                                egui::RichText::new("No modulation")
                                                    .color(theme.muted),
                                            )
                                            .truncate(),
                                        );
                                    }
                                },
                            );
                        });
                    });
                }
            });
        });
    // Keep the hint outside the scrolling row so it cannot disappear behind
    // the horizontal scrollbar or extend beyond the performance panel.
    let mut hint = ui.new_child(
        egui::UiBuilder::new()
            .id_salt(("macro-hint", name, pane))
            .max_rect(Rect::from_min_max(
                Pos2::new(rect.left() + 8.0, rect.bottom() - 20.0),
                rect.max - Vec2::new(8.0, 4.0),
            )),
    );
    hint.set_clip_rect(rect.intersect(ui.clip_rect()));
    hint.add(
        egui::Label::new(
            egui::RichText::new(
                "Drag: value · Outer ring: depth · Shift: fine · Double-click: reset",
            )
            .size(10.0)
            .color(theme.muted),
        )
        .truncate(),
    );
    action
}

/// One macro's signed depth: the amount field, outer ring, and popup all edit
/// the same route value. Changing it never changes the manual knob value.
#[cfg(test)]
fn modulation_depth(ui: &mut egui::Ui, route: &mut Route) -> egui::Response {
    ui.checkbox(&mut route.enabled, "Mod")
        .on_hover_text("Enable or bypass this macro's modulation");
    let mut percent = route.amount * 100.0;
    let response = ui.add_sized(
        [ui.available_width().max(52.0), 20.0],
        egui::DragValue::new(&mut percent)
            .range(-300.0..=300.0)
            .speed(1.0)
            .fixed_decimals(0)
            .custom_formatter(|value, _| format!("{value:+.0}"))
            .suffix("%"),
    ).on_hover_text("Modulation amount for this macro. Drag up/right to increase, down/left to decrease; Shift for fine adjustment. Click to type.");
    if response.changed() {
        route.amount = percent / 100.0;
    }
    response
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn knob(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f32,
    default: f32,
    effective: f32,
    amount: &mut f32,
    global: f32,
    width: f32,
    theme: &AppTheme,
    color: egui::Color32,
    range: (f32, f32),
) -> egui::Response {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 107.0), egui::Sense::hover());
    let center = Pos2::new(rect.center().x, rect.top() + 46.0);
    // Annulus owns the pointer only outside the knob body; dragging cannot change both.
    let pointer = ui.input(|i| i.pointer.interact_pos());
    let ring_id = ui.id().with("modulation-ring");
    let on_ring = pointer.is_some_and(|p| {
        let d = p.distance(center);
        (21.0..=33.0).contains(&d)
    });
    let ring_rect = if on_ring || ui.ctx().is_being_dragged(ring_id) {
        Rect::from_center_size(center, Vec2::splat(66.0))
    } else {
        Rect::NOTHING
    };
    let ring = ui
        .interact(ring_rect, ring_id, egui::Sense::click_and_drag())
        .on_hover_text("Drag vertically to change modulation strength; Shift for fine adjustment.");
    let mut response = ui
        .interact(
            Rect::from_center_size(center, Vec2::splat(39.0)),
            ui.id().with("base-knob"),
            egui::Sense::click_and_drag(),
        )
        .on_hover_cursor(egui::CursorIcon::ResizeVertical);
    let before = *value;
    let scale = if ui.input(|i| i.modifiers.shift) {
        1000.0
    } else {
        150.0
    };
    if ring.dragged() {
        let delta = if ring.drag_started() {
            ring.drag_delta().y
        } else {
            ui.input(|i| i.pointer.delta().y)
        };
        *amount = (*amount - delta / scale * 3.0).clamp(-12.0, 12.0);
    }
    if response.dragged() {
        let delta = if response.drag_started() {
            response.drag_delta().y
        } else {
            ui.input(|i| i.pointer.delta().y)
        };
        *value = (*value - delta / scale).clamp(0.0, 1.0);
    }
    if response.double_clicked() {
        *value = default;
    }
    if response.clicked() {
        response.request_focus();
    }
    if response.has_focus() {
        let delta = ui.input(|i| {
            i.key_pressed(egui::Key::ArrowUp) as i32 - i.key_pressed(egui::Key::ArrowDown) as i32
        });
        *value = (*value + delta as f32 / scale * 3.0).clamp(0.0, 1.0);
    }
    let p = ui.painter_at(rect);
    p.text(
        egui::pos2(center.x, rect.top()),
        egui::Align2::CENTER_TOP,
        label,
        egui::FontId::proportional(11.0),
        theme.foreground,
    );
    let arc = |from: f32, to: f32, r: f32| {
        (0..=48)
            .map(|i| {
                center + Vec2::angled(2.35 + 4.72 * (from + (to - from) * i as f32 / 48.0)) * r
            })
            .collect()
    };
    p.circle_filled(center, 18.0, theme.card);
    p.circle_stroke(
        center,
        18.0,
        Stroke::new(1.0, theme.foreground.gamma_multiply(0.3)),
    );
    p.add(egui::Shape::line(
        arc(0.0, 1.0, 23.0),
        Stroke::new(2.0, theme.foreground.gamma_multiply(0.12)),
    ));
    p.add(egui::Shape::line(
        arc(0.0, *value, 23.0),
        Stroke::new(2.0, theme.foreground),
    ));
    let offsets = [range.0 * *amount * global, range.1 * *amount * global];
    let a = (*value + offsets[0].min(offsets[1])).clamp(0.0, 1.0);
    let b = (*value + offsets[0].max(offsets[1])).clamp(0.0, 1.0);
    p.add(egui::Shape::line(
        arc(0.0, 1.0, 29.0),
        Stroke::new(3.0, color.gamma_multiply(0.12)),
    ));
    p.add(egui::Shape::line(
        arc(a, b, 29.0),
        Stroke::new(3.0, color.gamma_multiply(0.65)),
    ));
    let dir = Vec2::angled(2.35 + 4.72 * *value);
    p.line_segment(
        [center + dir * 7.0, center + dir * 15.0],
        Stroke::new(2.0, theme.foreground),
    );
    p.circle_filled(
        center + Vec2::angled(2.35 + 4.72 * effective) * 29.0,
        3.0,
        color,
    );
    let field = Rect::from_center_size(
        egui::pos2(center.x, rect.bottom() - 12.0),
        Vec2::new(width, 24.0),
    );
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .id_salt("macro-value")
            .max_rect(field),
    );
    child.spacing_mut().interact_size.y = 24.0;
    let mut percent = *value * 100.0;
    if child
        .add_sized(
            field.size(),
            egui::DragValue::new(&mut percent)
                .range(0.0..=100.0)
                .speed(0.2)
                .fixed_decimals(1)
                .suffix("%"),
        )
        .changed()
    {
        *value = percent / 100.0;
    }
    if *value != before {
        response.mark_changed();
    }
    response
}

/// Reaction presets change macro depth without changing the user's source bank
/// or substituting any of the sources assigned to this visualizer.
#[cfg(test)]
pub(super) fn preset_routes(
    _ui: &egui::Ui,
    kind: Visualizer,
    preset: usize,
    controls: &[Control; 12],
    settings: &mut ModulationSettings,
) {
    let previous = settings.routes.clone();
    settings.routes.clear();
    settings.amount = 1.0;
    settings.ambient = 0.6;
    for (i, c) in controls.iter().enumerate() {
        if matches!(
            c.target,
            Target::Expansion
                | Target::Impact
                | Target::LayerBend
                | Target::Brightness
                | Target::Turbulence
                | Target::LayerRipple
                | Target::LayerShimmer
                | Target::Swirl
                | Target::Shape
                | Target::Drift
        ) {
            let local = kind == Visualizer::Strata && c.target.layered();
            let mut route = previous
                .iter()
                .find(|r| r.target == c.target)
                .cloned()
                .unwrap_or_else(|| Route {
                    id: i as u64 + 1,
                    target: c.target,
                    enabled: local,
                    ..Default::default()
                });
            route.amount = if preset == 1 { 0.40 } else { 0.18 };
            if route.signal.is_none() && !local {
                route.enabled = false;
            }
            settings.routes.push(route);
        } else if let Some(route) = previous.iter().find(|r| r.target == c.target) {
            settings.routes.push(route.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ring_drag_changes_strength_without_changing_manual_base() {
        let ctx = egui::Context::default();
        let theme = AppTheme::default();
        let mut base = 0.4;
        let mut amount = 0.2;
        let mut render = |events| {
            let mut rect = Rect::NOTHING;
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(300.0, 300.0))),
                    events,
                    ..Default::default()
                },
                |ui| {
                    rect = knob(
                        ui,
                        "Expansion",
                        &mut base,
                        0.5,
                        0.7,
                        &mut amount,
                        1.0,
                        100.0,
                        &theme,
                        theme.accent,
                        (0.0, 1.0),
                    )
                    .rect;
                },
            );
            out.textures_delta.clear();
            rect
        };
        let rect = render(vec![]);
        let a = rect.center() + Vec2::new(29.0, 0.0);
        let b = a - Vec2::new(0.0, 20.0);
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        render(vec![egui::Event::PointerMoved(a)]);
        render(vec![button(a, true)]);
        render(vec![egui::Event::PointerMoved(b)]);
        render(vec![button(b, false)]);
        assert_eq!(base, 0.4);
        assert!(amount > 0.3, "strength was {amount}");
    }
    #[test]
    fn dragging_a_macros_amount_changes_only_its_depth_and_preserves_base_and_sources() {
        let ctx = egui::Context::default();
        let theme = AppTheme::default();
        let shared = std::sync::Arc::new(std::sync::Mutex::new(signals::ui::Panel::default()));
        let id = shared
            .lock()
            .unwrap()
            .bank
            .add(signals::Method::Band)
            .unwrap();
        let original_bank = shared.lock().unwrap().bank.clone();
        ctx.data_mut(|d| d.insert_temp(egui::Id::new(signals::ui::CONTEXT_KEY), shared.clone()));
        let controls = [
            Target::Motion,
            Target::Expansion,
            Target::Swirl,
            Target::Turbulence,
            Target::Brightness,
            Target::Impact,
            Target::Magnetism,
            Target::Shape,
            Target::Depth,
            Target::Trails,
            Target::Glow,
            Target::Color,
        ]
        .map(|t| Control::new("Macro", t, 0.0, 1.0, 0.4));
        let mut values = [0.4; 12];
        let mut settings = ModulationSettings {
            amount: 1.6,
            routes: vec![
                Route {
                    id: 1,
                    target: Target::Motion,
                    signal: Some(id),
                    amount: 0.25,
                    ..Default::default()
                },
                Route {
                    id: 2,
                    target: Target::Brightness,
                    signal: Some(id),
                    amount: -0.15,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let original_other = settings.routes[1].clone();
        let mut open = true;
        let mut render = |events| {
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1400.0, 900.0))),
                    events,
                    ..Default::default()
                },
                |ui| {
                    bar(
                        ui,
                        Rect::from_min_size(Pos2::new(0.0, 650.0), Vec2::new(1400.0, height(true))),
                        &theme,
                        0,
                        "Flux",
                        &controls,
                        &mut values,
                        [0.4; 12],
                        &mut settings,
                        &mut open,
                        &["A", "B", "C"],
                    );
                },
            );
            out.textures_delta.clear();
            out
        };
        let out = render(vec![]);
        assert!(!out.shapes.iter().any(
            |s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.job.text == "Mod amount")
        ));
        let start = out
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.job.text == "+25" => {
                    Some(t.pos + t.galley.size() * 0.5)
                }
                _ => None,
            })
            .expect("per-macro depth field missing");
        let end = start - Vec2::new(0.0, 20.0);
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            pressed,
            button: egui::PointerButton::Primary,
            modifiers: egui::Modifiers::NONE,
        };
        render(vec![egui::Event::PointerMoved(start)]);
        render(vec![button(start, true)]);
        render(vec![egui::Event::PointerMoved(end)]);
        render(vec![button(end, false)]);
        render(vec![]);
        assert!(settings.routes[0].amount > 0.4);
        assert_eq!(settings.routes[0].signal, Some(id));
        assert_eq!(settings.routes[1], original_other);
        assert_eq!(settings.amount, 1.6);
        assert_eq!(values, [0.4; 12]);
        assert_eq!(shared.lock().unwrap().bank, original_bank);
    }

    #[test]
    fn reaction_presets_preserve_assigned_sources_and_never_create_channels() {
        let ctx = egui::Context::default();
        let shared = std::sync::Arc::new(std::sync::Mutex::new(signals::ui::Panel::default()));
        ctx.data_mut(|d| d.insert_temp(egui::Id::new(signals::ui::CONTEXT_KEY), shared.clone()));
        let controls = [Control::new("Expansion", Target::Expansion, 0.0, 1.0, 0.4); 12];
        let mut settings = ModulationSettings::default();
        for kind in [Visualizer::Flux, Visualizer::Veroni, Visualizer::Strata] {
            for preset in 0..3 {
                let mut out = ctx.run_ui(Default::default(), |ui| {
                    preset_routes(ui, kind, preset, &controls, &mut settings)
                });
                out.textures_delta.clear();
                assert_eq!(shared.lock().unwrap().bank, signals::Bank::default());
                assert!(
                    settings
                        .routes
                        .iter()
                        .all(|r| r.signal.is_none() && !r.enabled)
                );
            }
        }
        let id = shared
            .lock()
            .unwrap()
            .bank
            .add(signals::Method::BassHits)
            .unwrap();
        settings.routes = vec![Route {
            signal: Some(id),
            target: Target::Expansion,
            ..Default::default()
        }];
        let original = shared.lock().unwrap().bank.clone();
        let mut out = ctx.run_ui(Default::default(), |ui| {
            preset_routes(ui, Visualizer::Flux, 1, &controls, &mut settings)
        });
        out.textures_delta.clear();
        assert_eq!(shared.lock().unwrap().bank, original);
        assert!(settings.routes.iter().all(|r| r.signal == Some(id)));
    }
    #[test]
    fn legacy_duplicates_are_retained_but_do_not_sum() {
        let controls = [Control::new("Expansion", Target::Expansion, 0.0, 1.0, 0.4); 12];
        let mut settings = ModulationSettings {
            routes: vec![
                Route::default(),
                Route {
                    id: 2,
                    amount: 0.5,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        single_sources(&mut settings, &controls);
        assert!(settings.routes[0].enabled);
        assert!(!settings.routes[1].enabled);
        assert_eq!(settings.routes[1].amount, 0.5);
        assert_eq!(settings.routes.len(), 2);
    }
    #[test]
    fn macro_popup_can_assign_existing_sources_and_keep_editing_the_connection() {
        fn position(out: &egui::FullOutput, label: &str) -> Pos2 {
            out.shapes
                .iter()
                .find_map(|s| match &s.shape {
                    egui::Shape::Text(t) if t.galley.job.text == label => {
                        Some(t.pos + t.galley.size() * 0.5)
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("missing {label}"))
        }
        for name in ["Flux", "Veroni", "Strata"] {
            let ctx = egui::Context::default();
            let theme = AppTheme::default();
            let shared = std::sync::Arc::new(std::sync::Mutex::new(signals::ui::Panel::default()));
            let audio = shared
                .lock()
                .unwrap()
                .bank
                .add(signals::Method::Band)
                .unwrap();
            let lfo = shared
                .lock()
                .unwrap()
                .bank
                .add(signals::Method::Lfo)
                .unwrap();
            {
                let mut panel = shared.lock().unwrap();
                panel.bank.channels[0].name = "Test audio".into();
                panel.bank.channels[1].name = "Test LFO".into();
            }
            let original = shared.lock().unwrap().bank.clone();
            ctx.data_mut(|d| {
                d.insert_temp(egui::Id::new(signals::ui::CONTEXT_KEY), shared.clone())
            });
            let controls = [
                Target::Motion,
                Target::Expansion,
                Target::Swirl,
                Target::Turbulence,
                Target::Brightness,
                Target::Impact,
                Target::Magnetism,
                Target::Shape,
                Target::Depth,
                Target::Trails,
                Target::Glow,
                Target::Color,
            ]
            .map(|t| Control::new("Macro", t, 0.0, 1.0, 0.4));
            let mut values = [0.4; 12];
            let mut settings = ModulationSettings::default();
            let mut open = true;
            let mut render = |events| {
                let mut out = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(
                            Pos2::ZERO,
                            Vec2::new(1400.0, 900.0),
                        )),
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        bar(
                            ui,
                            Rect::from_min_size(
                                Pos2::new(0.0, 650.0),
                                Vec2::new(1400.0, height(true)),
                            ),
                            &theme,
                            0,
                            name,
                            &controls,
                            &mut values,
                            [0.4; 12],
                            &mut settings,
                            &mut open,
                            &["A", "B", "C"],
                        );
                    },
                );
                out.textures_delta.clear();
                out
            };
            fn button(pos: Pos2, pressed: bool) -> egui::Event {
                egui::Event::PointerButton {
                    pos,
                    pressed,
                    button: egui::PointerButton::Primary,
                    modifiers: egui::Modifiers::NONE,
                }
            }
            let mut click = |pos: Pos2| {
                render(vec![egui::Event::PointerMoved(pos)]);
                render(vec![button(pos, true)]);
                render(vec![button(pos, false)]);
                render(vec![])
            };
            // Opening Connect creates a mapping only; clicking its nested chooser
            // must leave the editor alive while the source list is open.
            let out = click(Pos2::new(30.0, 10.0));
            let out = click(position(&out, "Connect…"));
            let out = click(position(&out, "Choose source"));
            let out = click(position(&out, "Test audio"));
            position(&out, "Disconnect");
            // The same name also appears in the badge and detector name field.
            // The chooser is the topmost occurrence in the editor above the bar.
            let chooser = out
                .shapes
                .iter()
                .filter_map(|s| match &s.shape {
                    egui::Shape::Text(t) if t.galley.job.text == "Test audio" => {
                        Some(t.pos + t.galley.size() * 0.5)
                    }
                    _ => None,
                })
                .min_by(|a, b| a.y.total_cmp(&b.y))
                .unwrap();
            let out = click(chooser);
            let out = click(position(&out, "Test LFO"));
            position(&out, "Disconnect");
            let out = click(Pos2::new(30.0, 10.0));
            assert!(!out.shapes.iter().any(
                |s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.job.text == "Disconnect")
            ));
            let out = click(position(&out, "Test LFO"));
            position(&out, "Disconnect");
            render(vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }]);
            let out = render(vec![]);
            assert!(!out.shapes.iter().any(
                |s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.job.text == "Disconnect")
            ));
            let route = settings
                .routes
                .iter()
                .find(|r| r.target == Target::Motion)
                .unwrap();
            assert_eq!(route.signal, Some(lfo));
            assert!(route.enabled && !route.invert);
            assert_ne!(audio, lfo);
            assert_eq!(values, [0.4; 12]);
            assert_eq!(shared.lock().unwrap().bank, original);
        }
    }

    #[test]
    fn disconnect_clears_only_this_macros_mapping_and_preserves_shared_sources() {
        let ctx = egui::Context::default();
        let theme = AppTheme::default();
        let shared = std::sync::Arc::new(std::sync::Mutex::new(signals::ui::Panel::default()));
        shared
            .lock()
            .unwrap()
            .replace_bank(signals::Bank::test_sources());
        let original_bank = shared.lock().unwrap().bank.clone();
        ctx.data_mut(|d| d.insert_temp(egui::Id::new(signals::ui::CONTEXT_KEY), shared.clone()));
        let controls = [
            Target::Motion,
            Target::Expansion,
            Target::Swirl,
            Target::Turbulence,
            Target::Brightness,
            Target::Impact,
            Target::Magnetism,
            Target::Shape,
            Target::Depth,
            Target::Trails,
            Target::Glow,
            Target::Color,
        ]
        .map(|t| Control::new("Macro", t, 0.0, 1.0, 0.4));
        let other = Route {
            id: 2,
            target: Target::Brightness,
            signal: Some(2),
            amount: -0.45,
            ..Default::default()
        };
        let mut settings = ModulationSettings {
            routes: vec![
                Route {
                    id: 1,
                    target: Target::Motion,
                    signal: Some(1),
                    ..Default::default()
                },
                other.clone(),
            ],
            ..Default::default()
        };
        let mut values = [0.4; 12];
        let mut open = true;
        let mut render = |events| {
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1400.0, 900.0))),
                    events,
                    ..Default::default()
                },
                |ui| {
                    bar(
                        ui,
                        Rect::from_min_size(Pos2::new(0.0, 650.0), Vec2::new(1400.0, height(true))),
                        &theme,
                        0,
                        "Flux",
                        &controls,
                        &mut values,
                        [0.4; 12],
                        &mut settings,
                        &mut open,
                        &["A", "B", "C"],
                    );
                },
            );
            out.textures_delta.clear();
            out
        };
        fn position(out: &egui::FullOutput, label: &str) -> Pos2 {
            out.shapes
                .iter()
                .find_map(|s| match &s.shape {
                    egui::Shape::Text(t) if t.galley.job.text == label => {
                        Some(t.pos + t.galley.size() * 0.5)
                    }
                    _ => None,
                })
                .unwrap()
        }
        let mut click = |pos: Pos2| {
            render(vec![egui::Event::PointerMoved(pos)]);
            for pressed in [true, false] {
                render(vec![egui::Event::PointerButton {
                    pos,
                    pressed,
                    button: egui::PointerButton::Primary,
                    modifiers: egui::Modifiers::NONE,
                }]);
            }
            render(vec![])
        };
        let out = click(Pos2::new(30.0, 10.0));
        let out = click(position(&out, "Overall intensity"));
        click(position(&out, "Disconnect"));
        assert_eq!(settings.routes, vec![other]);
        assert_eq!(values, [0.4; 12]);
        assert_eq!(shared.lock().unwrap().bank, original_bank);
    }

    #[test]
    fn source_badge_opens_only_mapping_controls_without_switching_tabs_or_editing_sources() {
        let ctx = egui::Context::default();
        let theme = AppTheme::default();
        let shared = std::sync::Arc::new(std::sync::Mutex::new(signals::ui::Panel::default()));
        shared
            .lock()
            .unwrap()
            .replace_bank(signals::Bank::test_sources());
        ctx.data_mut(|d| d.insert_temp(egui::Id::new(signals::ui::CONTEXT_KEY), shared.clone()));
        let controls = [
            Target::Motion,
            Target::Expansion,
            Target::Swirl,
            Target::Turbulence,
            Target::Brightness,
            Target::Impact,
            Target::Magnetism,
            Target::Shape,
            Target::Depth,
            Target::Trails,
            Target::Glow,
            Target::Color,
        ]
        .map(|t| Control::new("Expansion", t, 0.0, 1.0, 0.4));
        let mut values = [0.4; 12];
        let mut settings = ModulationSettings {
            routes: vec![Route {
                signal: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        };
        let mut open = true;
        let mut render = |events| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1400.0, 900.0))),
                    events,
                    ..Default::default()
                },
                |ui| {
                    bar(
                        ui,
                        Rect::from_min_size(Pos2::new(0.0, 650.0), Vec2::new(1400.0, height(true))),
                        &theme,
                        0,
                        "Flux",
                        &controls,
                        &mut values,
                        [0.6; 12],
                        &mut settings,
                        &mut open,
                        &["A", "B", "C"],
                    );
                },
            );
            output.textures_delta.clear();
            output
        };
        let out = render(vec![]);
        let position = out
            .shapes
            .iter()
            .find_map(|s| {
                if let egui::Shape::Text(t) = &s.shape {
                    (t.galley.job.text == "Overall intensity")
                        .then_some(t.pos + t.galley.size() * 0.5)
                } else {
                    None
                }
            })
            .unwrap();
        let event = |pressed| egui::Event::PointerButton {
            pos: position,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        render(vec![egui::Event::PointerMoved(position)]);
        render(vec![event(true)]);
        render(vec![event(false)]);
        let out = render(vec![]);
        assert!(
            out.shapes.iter().any(
                |s| matches!(&s.shape,egui::Shape::Text(t) if t.galley.job.text=="Disconnect")
            ),
            "no inline source editor"
        );
        for label in [
            "New audio",
            "New LFO",
            "Make independent",
            "Strength",
            "Invert source",
            "Modulation enabled",
            "Source range",
            "Input",
            "Waveform",
            "Rate Hz",
            "Gain dB",
            "Response",
        ] {
            assert!(
                !out.shapes.iter().any(
                    |s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.job.text == label)
                )
            );
        }
        assert_eq!(shared.lock().unwrap().bank.channels.len(), 5);
        assert!(!shared.lock().unwrap().show_signals);
    }
}
