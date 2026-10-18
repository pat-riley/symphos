//! Persistent per-component performance bindings. No audio/runtime data is saved.
use crate::{
    modules::{ModuleKind, ModuleSettings},
    signals,
    theme::AppTheme,
};
use egui::{self, Rect, Vec2};
use serde::{Deserialize, Serialize};
pub const CONTEXT: &str = "macro-parameter-context";
#[derive(Clone)]
pub struct ParameterContext {
    pub pane: String,
    pub kind: ModuleKind,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterSpec {
    pub field: String,
    pub label: String,
    pub min: f64,
    pub max: f64,
    pub safe_min: f64,
    pub safe_max: f64,
    pub default: f64,
    pub integer: bool,
    pub logarithmic: bool,
    pub scale: f64,
}
impl ParameterSpec {
    pub fn valid(&self) -> bool {
        !self.field.is_empty()
            && self.field.len() <= 120
            && !self.label.is_empty()
            && self.label.len() <= 120
            && [
                self.min,
                self.max,
                self.safe_min,
                self.safe_max,
                self.default,
                self.scale,
            ]
            .iter()
            .all(|v| v.is_finite())
            && self.min < self.max
            && self.safe_min <= self.min
            && self.safe_max >= self.max
            && self.safe_min >= -384000.0
            && self.safe_max <= 384000.0
            && self.scale > 0.0
            && (!self.logarithmic || self.min > 0.0)
    }
    pub fn normalized(&self, value: f64) -> f32 {
        if self.logarithmic {
            ((value.max(self.min) / self.min).ln() / (self.max / self.min).ln()).clamp(0.0, 1.0)
                as f32
        } else {
            ((value - self.min) / (self.max - self.min)).clamp(0.0, 1.0) as f32
        }
    }
    pub fn value(&self, normalized: f32) -> f64 {
        let n = f64::from(normalized);
        let value = if self.logarithmic {
            self.min * (self.max / self.min).powf(n)
        } else {
            self.min + n * (self.max - self.min)
        };
        if self.integer { value.round() } else { value }
    }
}
#[derive(Clone)]
pub struct ParameterDrag {
    pub context: ParameterContext,
    pub spec: ParameterSpec,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Slot {
    pub parameter: Option<ParameterSpec>,
    pub signal: Option<u64>,
    pub depth: f32,
    pub enabled: bool,
    pub invert: bool,
    pub output_min: f32,
    pub output_max: f32,
}
impl Default for Slot {
    fn default() -> Self {
        Self {
            parameter: None,
            signal: None,
            depth: 0.35,
            enabled: true,
            invert: false,
            output_min: 0.0,
            output_max: 1.0,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub open: bool,
    #[serde(deserialize_with = "decode_banks")]
    pub banks: [[Slot; 12]; 10],
}
// Preserve the first nine component banks in projects saved before Sphere.
fn decode_banks<'de, D: serde::Deserializer<'de>>(d: D) -> Result<[[Slot; 12]; 10], D::Error> {
    let banks = Vec::<[Slot; 12]>::deserialize(d)?;
    if !(9..=10).contains(&banks.len()) {
        return Err(serde::de::Error::custom("expected nine or ten macro banks"));
    }
    let mut result = std::array::from_fn(|_| std::array::from_fn(|_| Slot::default()));
    for (out, bank) in result.iter_mut().zip(banks) {
        *out = bank;
    }
    Ok(result)
}
impl Default for State {
    fn default() -> Self {
        Self {
            open: true,
            banks: std::array::from_fn(|_| std::array::from_fn(|_| Slot::default())),
        }
    }
}
impl State {
    pub fn valid(&self, settings: &[ModuleSettings]) -> bool {
        self.banks.iter().enumerate().all(|(i, bank)| {
            let mut fields = std::collections::HashSet::new();
            bank.iter().all(|slot| {
                slot.depth.is_finite()
                    && (-12.0..=12.0).contains(&slot.depth)
                    && slot.output_min.is_finite()
                    && slot.output_max.is_finite()
                    && (0.0..=1.0).contains(&slot.output_min)
                    && (slot.output_min..=1.0).contains(&slot.output_max)
                    && slot.signal.is_none_or(|id| id > 0)
                    && slot.parameter.as_ref().is_none_or(|p| {
                        p.valid()
                            && fields.insert(&p.field)
                            && settings
                                .iter()
                                .find(|s| s.kind() == ModuleKind::ALL[i])
                                .and_then(|s| get(s, &p.field))
                                .is_some()
                    })
            })
        })
    }
    pub fn assign(&mut self, kind: ModuleKind, index: usize, spec: ParameterSpec) {
        for (i, slot) in self.banks[kind as usize].iter_mut().enumerate() {
            if i != index
                && slot
                    .parameter
                    .as_ref()
                    .is_some_and(|p| p.field == spec.field)
            {
                *slot = Slot::default();
            }
        }
        self.banks[kind as usize][index].parameter = Some(spec); // Keep this slot's modulation connection.
    }
    pub fn reconcile(&mut self, bank: &signals::Bank) {
        for slot in self.banks.iter_mut().flatten() {
            if slot
                .signal
                .is_some_and(|id| bank.channels.iter().all(|d| d.id != id))
            {
                slot.signal = None;
            }
        }
    }
    pub fn effective(
        &self,
        kind: ModuleKind,
        settings: &ModuleSettings,
        snapshot: Option<&signals::Snapshot>,
    ) -> Vec<(String, f64)> {
        let document = serde_json::to_value(settings).ok().unwrap_or_default();
        self.banks[kind as usize]
            .iter()
            .filter_map(|slot| {
                let p = slot.parameter.as_ref()?;
                if !slot.enabled {
                    return None;
                }
                let sample = snapshot?.values.get(&slot.signal?)?;
                let base = field(&document, &p.field)?.as_f64()? / p.scale;
                let input = if slot.invert {
                    -sample.value
                } else {
                    sample.value
                };
                let modulation = (slot.output_min * sample.availability
                    + input * (slot.output_max - slot.output_min))
                    * slot.depth;
                let value = if p.logarithmic {
                    p.min
                        * (p.max / p.min)
                            .powf(f64::from(p.normalized(base)) + f64::from(modulation))
                } else {
                    base + f64::from(modulation) * (p.max - p.min)
                };
                let value = value.clamp(p.safe_min, p.safe_max);
                Some((
                    p.field.clone(),
                    if p.integer {
                        value.round() * p.scale
                    } else {
                        value * p.scale
                    },
                ))
            })
            .collect()
    }
}
pub const INDICATORS: &str = "inspector-macro-indicators";
#[derive(Clone)]
pub struct InspectorIndicator {
    pub macro_index: usize,
    pub source: String,
    pub color: egui::Color32,
    pub enabled: bool,
    pub current: f64,
    pub depth: f32,
}
#[derive(Clone)]
pub struct InspectorIndicators {
    pub pane: String,
    pub kind: ModuleKind,
    pub values: std::collections::HashMap<String, InspectorIndicator>,
}
pub fn inspector_indicators(
    ui: &egui::Ui,
    pane: &str,
    kind: ModuleKind,
    state: &State,
    settings: &ModuleSettings,
) -> InspectorIndicators {
    let shared =
        ui.data(|d| d.get_temp::<signals::ui::Shared>(egui::Id::new(signals::ui::CONTEXT_KEY)));
    let panel = shared
        .as_ref()
        .map(|s| s.lock().unwrap_or_else(|e| e.into_inner()));
    let effective = state.effective(kind, settings, panel.as_ref().map(|s| s.snapshot.as_ref()));
    let values = state.banks[kind as usize]
        .iter()
        .enumerate()
        .filter_map(|(i, slot)| {
            let p = slot.parameter.as_ref()?;
            let id = slot.signal?;
            let source = panel
                .as_ref()
                .and_then(|s| s.bank.channels.iter().find(|d| d.id == id));
            let base = get(settings, &p.field)?;
            Some((
                p.field.clone(),
                InspectorIndicator {
                    macro_index: i + 1,
                    source: source.map_or_else(|| "Unavailable source".into(), |d| d.name.clone()),
                    color: source
                        .map_or(ui.visuals().selection.bg_fill, signals::ui::channel_color),
                    enabled: slot.enabled,
                    current: effective
                        .iter()
                        .find(|(key, _)| key == &p.field)
                        .map_or(base, |(_, v)| *v)
                        / p.scale,
                    depth: slot.depth,
                },
            ))
        })
        .collect();
    InspectorIndicators {
        pane: pane.into(),
        kind,
        values,
    }
}
fn field<'a>(value: &'a serde_json::Value, path: &str) -> Option<&'a serde_json::Value> {
    let mut value = value.as_object()?.values().next()?;
    for key in path.split('.') {
        value = value.get(key)?;
    }
    Some(value)
}
pub fn get(settings: &ModuleSettings, path: &str) -> Option<f64> {
    field(&serde_json::to_value(settings).ok()?, path)?.as_f64()
}
pub fn set(settings: &mut ModuleSettings, path: &str, value: f64) -> bool {
    if !value.is_finite() {
        return false;
    }
    let Ok(mut document) = serde_json::to_value(&*settings) else {
        return false;
    };
    let Some(mut node) = document.as_object_mut().and_then(|m| m.values_mut().next()) else {
        return false;
    };
    for key in path.split('.') {
        let Some(next) = node.get_mut(key) else {
            return false;
        };
        node = next;
    }
    let Some(number) = node.as_number() else {
        return false;
    };
    let new = if number.is_u64() {
        if value < 0.0 {
            return false;
        }
        serde_json::Value::from(value.round() as u64)
    } else if number.is_i64() {
        serde_json::Value::from(value.round() as i64)
    } else {
        serde_json::Value::from(value)
    };
    if *node == new {
        return false;
    }
    *node = new;
    let Ok(updated) = serde_json::from_value(document) else {
        return false;
    };
    *settings = updated;
    true
}

pub fn bar(
    ui: &mut egui::Ui,
    rect: Rect,
    theme: &AppTheme,
    pane: &str,
    kind: ModuleKind,
    state: &mut State,
    settings: &mut ModuleSettings,
) -> (bool, bool) {
    ui.painter().rect_filled(rect, 0.0, theme.panel);
    ui.painter().hline(
        rect.x_range(),
        rect.top(),
        egui::Stroke::new(1.0, theme.foreground.gamma_multiply(0.18)),
    );
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .id_salt(("assignable-macros", pane, kind as usize))
            .max_rect(rect.shrink2(Vec2::new(8.0, 6.0))),
    );
    child.set_clip_rect(rect.intersect(ui.clip_rect()));
    child.style_mut().override_font_id = Some(egui::FontId::proportional(11.0));
    child.spacing_mut().item_spacing = Vec2::new(6.0, 4.0);
    child.spacing_mut().interact_size.y = 24.0;
    let mut inspect = false;
    let mut burst = false;
    child.horizontal(|ui| {
        ui.strong("Macros");
        ui.weak(kind.label());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add_sized([56.0, 24.0], egui::Button::new("Hide"))
                .clicked()
            {
                state.open = false;
            }
            inspect = ui
                .add_sized([80.0, 24.0], egui::Button::new("Advanced"))
                .clicked();
            if kind == ModuleKind::Flux {
                burst = ui
                    .add_sized([56.0, 24.0], egui::Button::new("Burst"))
                    .clicked();
            }
        });
    });
    let shared =
        ui.data(|d| d.get_temp::<signals::ui::Shared>(egui::Id::new(signals::ui::CONTEXT_KEY)));
    let snapshot = shared
        .as_ref()
        .map(|s| s.lock().unwrap_or_else(|e| e.into_inner()).snapshot.clone());
    let effective = state.effective(kind, settings, snapshot.as_deref());
    let width = ((child.available_width() - 66.0) / 12.0).max(96.0);
    egui::ScrollArea::horizontal()
        .id_salt("assignable-macro-row")
        .max_height(174.0)
        .auto_shrink([false, true])
        .show(&mut child, |ui| {
            ui.horizontal_top(|ui| {
                for i in 0..12 {
                    ui.push_id(i, |ui| {
                        let start = ui.next_widget_position();
                        let slot = &mut state.banks[kind as usize][i];
                        let column = ui.vertical(|ui| {
                            ui.set_width(width);
                            if let Some(p) = slot.parameter.clone() {
                                let base = get(settings, &p.field).unwrap_or(p.default * p.scale)
                                    / p.scale;
                                let normalized = p.normalized(base);
                                let current = effective
                                    .iter()
                                    .find(|(key, _)| key == &p.field)
                                    .map_or(base, |(_, v)| *v / p.scale);
                                let mut value = normalized;
                                let (source, color, range) = shared.as_ref().map_or(
                                    ("Connect…".to_owned(), theme.accent, (0.0, 1.0)),
                                    |shared| {
                                        let panel =
                                            shared.lock().unwrap_or_else(|e| e.into_inner());
                                        slot.signal
                                            .and_then(|id| {
                                                panel.bank.channels.iter().find(|d| d.id == id)
                                            })
                                            .map_or(
                                                ("Connect…".to_owned(), theme.accent, (0.0, 1.0)),
                                                |d| {
                                                    let peak = d.influence
                                                        * if d.input == signals::Input::Lfo
                                                            && !d.normalization.enabled
                                                        {
                                                            d.lfo.depth
                                                        } else {
                                                            1.0
                                                        };
                                                    let low = if d.kind()
                                                        == signals::OutputKind::Bipolar
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
                                            )
                                    },
                                );
                                let map = |v: f32| {
                                    slot.output_min
                                        + (if slot.invert { -v } else { v })
                                            * (slot.output_max - slot.output_min)
                                };
                                let range = (map(range.0), map(range.1));
                                if crate::modules::performance::knob(
                                    ui,
                                    &p.label,
                                    &mut value,
                                    p.normalized(p.default),
                                    p.normalized(current),
                                    &mut slot.depth,
                                    if slot.enabled && slot.signal.is_some() {
                                        1.0
                                    } else {
                                        0.0
                                    },
                                    width,
                                    theme,
                                    color,
                                    range,
                                )
                                .changed()
                                {
                                    set(settings, &p.field, p.value(value) * p.scale);
                                }
                                let badge = ui.add_sized(
                                    [width, 24.0],
                                    egui::Button::new(egui::RichText::new(source).color(color))
                                        .truncate(),
                                );
                                let editor_key = egui::Id::new("assignable-macro-source-popup");
                                let mut opened = ui.data(|d| d.get_temp::<egui::Id>(editor_key))
                                    == Some(badge.id);
                                if badge.clicked() {
                                    opened = !opened;
                                    if opened {
                                        ui.data_mut(|d| d.insert_temp(editor_key, badge.id));
                                    }
                                }
                                let nested = egui::Popup::is_any_open(ui.ctx());
                                let popup = egui::Popup::from_response(&badge)
                                    .open_bool(&mut opened)
                                    .close_behavior(egui::PopupCloseBehavior::IgnoreClicks)
                                    .width(280.0)
                                    .show(|ui| {
                                        ui.strong(&p.label);
                                        if let Some(shared) = &shared {
                                            let panel =
                                                shared.lock().unwrap_or_else(|e| e.into_inner());
                                            let mut signal = slot.signal;
                                            let selected = signal
                                                .and_then(|id| {
                                                    panel.bank.channels.iter().find(|d| d.id == id)
                                                })
                                                .map_or("Choose source", |d| d.name.as_str());
                                            ui.label("Source");
                                            egui::ComboBox::from_id_salt("source")
                                                .width(260.0)
                                                .selected_text(selected)
                                                .show_ui(ui, |ui| {
                                                    ui.selectable_value(
                                                        &mut signal,
                                                        None,
                                                        "No modulation",
                                                    );
                                                    for d in &panel.bank.channels {
                                                        ui.selectable_value(
                                                            &mut signal,
                                                            Some(d.id),
                                                            &d.name,
                                                        );
                                                    }
                                                });
                                            if signal != slot.signal {
                                                slot.signal = signal;
                                                slot.enabled = true;
                                            }
                                            if panel.bank.channels.is_empty() {
                                                ui.weak("Add sources in Signals.");
                                            }
                                            if ui
                                                .add_enabled(
                                                    slot.signal.is_some(),
                                                    egui::Button::new("Disconnect"),
                                                )
                                                .clicked()
                                            {
                                                slot.signal = None;
                                                ui.close();
                                            }
                                        }
                                    });
                                if !badge.clicked()
                                    && !nested
                                    && !egui::Popup::is_any_open(ui.ctx())
                                    && popup
                                        .as_ref()
                                        .is_some_and(|p| p.response.clicked_elsewhere())
                                {
                                    opened = false;
                                }
                                if !opened
                                    && ui.data(|d| d.get_temp::<egui::Id>(editor_key))
                                        == Some(badge.id)
                                {
                                    ui.data_mut(|d| d.remove::<egui::Id>(editor_key));
                                }
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().interact_size.y = 20.0;
                                    if slot.signal.is_some() {
                                        ui.checkbox(&mut slot.enabled, "Mod");
                                        let mut depth = slot.depth * 100.0;
                                        if ui
                                            .add_sized(
                                                [ui.available_width().max(52.0), 20.0],
                                                egui::DragValue::new(&mut depth)
                                                    .range(-1200.0..=1200.0)
                                                    .speed(1.0)
                                                    .fixed_decimals(0)
                                                    .custom_formatter(|v, _| format!("{v:+.0}"))
                                                    .suffix("%"),
                                            )
                                            .changed()
                                        {
                                            slot.depth = depth / 100.0;
                                        }
                                    } else {
                                        ui.weak("No modulation");
                                    }
                                });
                            } else {
                                let (area, _) = ui.allocate_exact_size(
                                    Vec2::new(width, 159.0),
                                    egui::Sense::hover(),
                                );
                                ui.painter().rect_stroke(
                                    area.shrink(1.0),
                                    4.0,
                                    egui::Stroke::new(1.0, theme.foreground.gamma_multiply(0.16)),
                                    egui::StrokeKind::Inside,
                                );
                                ui.painter().text(
                                    area.center_top() + Vec2::new(0.0, 9.0),
                                    egui::Align2::CENTER_TOP,
                                    format!("Macro {}", i + 1),
                                    egui::FontId::proportional(11.0),
                                    theme.muted,
                                );
                                ui.painter().text(
                                    area.center(),
                                    egui::Align2::CENTER_CENTER,
                                    "Drop parameter",
                                    egui::FontId::proportional(10.0),
                                    theme.muted,
                                );
                            }
                        });
                        let area = column
                            .response
                            .rect
                            .union(Rect::from_min_size(start, Vec2::new(width, 159.0)));
                        let response =
                            ui.interact(area, ui.id().with("macro-drop"), egui::Sense::hover());
                        // The knob, value and source badge own their input responses.
                        // Observe a secondary click across the slot without stealing
                        // their primary drags/clicks from the children.
                        let right_click = ui.rect_contains_pointer(area)
                            && ui.input(|input| input.pointer.secondary_clicked());
                        egui::Popup::menu(&response)
                            .open_memory(right_click.then_some(egui::SetOpenCommand::Bool(true)))
                            .at_pointer_fixed()
                            .show(|ui| {
                                if ui
                                    .add_enabled(
                                        state.banks[kind as usize][i].parameter.is_some(),
                                        egui::Button::new("Remove macro"),
                                    )
                                    .clicked()
                                {
                                    state.banks[kind as usize][i] = Slot::default();
                                    ui.close();
                                }
                            });
                        if ui.rect_contains_pointer(area)
                            && let Some(payload) =
                                egui::DragAndDrop::payload::<ParameterDrag>(ui.ctx())
                        {
                            let accepted = payload.context.pane == pane
                                && payload.context.kind == kind
                                && payload.spec.valid()
                                && get(settings, &payload.spec.field).is_some();
                            ui.painter().rect_stroke(
                                area,
                                4.0,
                                egui::Stroke::new(
                                    2.0,
                                    if accepted {
                                        theme.accent
                                    } else {
                                        ui.visuals().warn_fg_color
                                    },
                                ),
                                egui::StrokeKind::Inside,
                            );
                            if accepted && ui.input(|input| input.pointer.any_released()) {
                                state.assign(kind, i, payload.spec.clone());
                                egui::DragAndDrop::take_payload::<ParameterDrag>(ui.ctx());
                            }
                        }
                    });
                }
            });
        });
    let hint = Rect::from_min_max(
        egui::pos2(rect.left() + 8.0, rect.bottom() - 20.0),
        rect.max - Vec2::new(8.0, 4.0),
    );
    ui.painter_at(hint).text(
        hint.left_center(),
        egui::Align2::LEFT_CENTER,
        "Drag parameter labels here · Ring: depth · Shift: fine · Right-click: clear",
        egui::FontId::proportional(10.0),
        theme.muted,
    );
    (inspect, burst)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::ModulePane;
    fn spec(field: &str, label: &str) -> ParameterSpec {
        ParameterSpec {
            field: field.into(),
            label: label.into(),
            min: 0.0,
            max: 10.0,
            safe_min: 0.0,
            safe_max: 10.0,
            default: 1.0,
            integer: false,
            logarithmic: false,
            scale: 1.0,
        }
    }
    #[test]
    fn inspector_label_drag_assigns_and_replaces_a_real_macro_without_changing_the_parameter() {
        let ctx = egui::Context::default();
        let theme = AppTheme::default();
        let mut pane = ModulePane::new(ModuleKind::Waterfall);
        pane.macros
            .assign(ModuleKind::Waterfall, 0, spec("length_x", "Length X"));
        pane.macros.banks[0][0].signal = Some(1);
        pane.macros.banks[0][0].depth = -0.6;
        let base = pane.copy_settings();
        let shared = std::sync::Arc::new(std::sync::Mutex::new(signals::ui::Panel::default()));
        shared
            .lock()
            .unwrap()
            .replace_bank(signals::Bank::test_sources());
        let source_bank = shared.lock().unwrap().bank.clone();
        ctx.data_mut(|d| d.insert_temp(egui::Id::new(signals::ui::CONTEXT_KEY), shared.clone()));
        let mut render = |events| {
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1400.0, 900.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    pane.performance_bar(
                        ui,
                        Rect::from_min_size(egui::pos2(0.0, 650.0), egui::vec2(1400.0, 234.0)),
                        &theme,
                        0,
                    );
                    let bounds =
                        Rect::from_min_size(egui::pos2(1040.0, 0.0), egui::vec2(360.0, 640.0));
                    let mut inspector = ui.new_child(egui::UiBuilder::new().max_rect(bounds));
                    inspector.set_clip_rect(bounds);
                    inspector
                        .data_mut(|d| d.insert_temp(egui::Id::new("inspector-bounds"), bounds));
                    pane.inspector_controls(
                        &mut inspector,
                        &crate::analysis::AnalysisFrame::default(),
                    );
                    ui.data_mut(|d| {
                        d.remove::<Rect>(egui::Id::new("inspector-bounds"));
                        d.remove::<ParameterContext>(egui::Id::new(CONTEXT));
                    });
                },
            );
            out.textures_delta.clear();
            out
        };
        let out = render(vec![]);
        let origin = out
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.job.text == "Height" => {
                    Some(t.pos + t.galley.size() * 0.5)
                }
                _ => None,
            })
            .expect("height inspector label missing");
        let target = egui::pos2(55.0, 720.0);
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            pressed,
            button: egui::PointerButton::Primary,
            modifiers: egui::Modifiers::NONE,
        };
        render(vec![egui::Event::PointerMoved(origin)]);
        render(vec![button(origin, true)]);
        render(vec![egui::Event::PointerMoved(
            origin + egui::vec2(-16.0, 0.0),
        )]);
        assert_eq!(
            egui::DragAndDrop::payload::<ParameterDrag>(&ctx)
                .unwrap()
                .spec
                .field,
            "height"
        );
        render(vec![egui::Event::PointerMoved(target)]);
        render(vec![button(target, false)]);
        render(vec![]);
        let slot = &pane.macros.banks[0][0];
        assert_eq!(slot.parameter.as_ref().unwrap().field, "height");
        assert_eq!(slot.signal, Some(1));
        assert_eq!(slot.depth, -0.6);
        assert_eq!(pane.copy_settings(), base);
        assert_eq!(shared.lock().unwrap().bank, source_bank);
    }
    #[test]
    fn right_click_removes_macros_over_knob_value_source_and_modulation_row() {
        use super::source_picker_tests::position;
        for target in 0..5 {
            let ctx = egui::Context::default();
            let mut pane = ModulePane::new(ModuleKind::Waterfall);
            pane.macros
                .assign(ModuleKind::Waterfall, 0, spec("height", "Assigned"));
            pane.macros.banks[0][0].signal = Some(1);
            let before = pane.copy_settings();
            let mut render = |events| {
                let mut out = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(1400.0, 900.0),
                        )),
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        pane.performance_bar(
                            ui,
                            Rect::from_min_size(egui::pos2(0.0, 650.0), egui::vec2(1400.0, 234.0)),
                            &AppTheme::default(),
                            0,
                        );
                    },
                );
                out.textures_delta.clear();
                out
            };
            render(vec![]);
            let out = render(vec![]);
            let label = position(&out, "Assigned");
            let point = match target {
                0 => label,
                1 => label + egui::vec2(0.0, 40.0),
                2 => label + egui::vec2(0.0, 94.0),
                3 => position(&out, "Connect…"),
                _ => position(&out, "Mod"),
            };
            render(vec![egui::Event::PointerMoved(point)]);
            for pressed in [true, false] {
                render(vec![egui::Event::PointerButton {
                    pos: point,
                    pressed,
                    button: egui::PointerButton::Secondary,
                    modifiers: egui::Modifiers::NONE,
                }]);
            }
            let out = render(vec![]);
            let remove = position(&out, "Remove macro");
            render(vec![egui::Event::PointerMoved(remove)]);
            for pressed in [true, false] {
                render(vec![egui::Event::PointerButton {
                    pos: remove,
                    pressed,
                    button: egui::PointerButton::Primary,
                    modifiers: egui::Modifiers::NONE,
                }]);
            }
            render(vec![]);
            assert_eq!(pane.macros.banks[0][0], Slot::default(), "target {target}");
            assert_eq!(pane.copy_settings(), before);
        }
    }
    #[test]
    fn inspector_indicator_tracks_live_source_and_manual_base_without_editing_it() {
        let ctx = egui::Context::default();
        let shared = std::sync::Arc::new(std::sync::Mutex::new(signals::ui::Panel::default()));
        let id = shared
            .lock()
            .unwrap()
            .bank
            .add(signals::Method::Band)
            .unwrap();
        shared.lock().unwrap().snapshot = std::sync::Arc::new(signals::Snapshot {
            values: [(
                id,
                signals::Sample {
                    value: 0.5,
                    availability: 1.0,
                    ..Default::default()
                },
            )]
            .into(),
            ..Default::default()
        });
        ctx.data_mut(|d| d.insert_temp(egui::Id::new(signals::ui::CONTEXT_KEY), shared));
        let mut pane = ModulePane::new(ModuleKind::Waterfall);
        pane.macros
            .assign(ModuleKind::Waterfall, 0, spec("height", "Height"));
        pane.macros.banks[0][0].signal = Some(id);
        let settings = pane.copy_settings();
        let base = get(&settings, "height").unwrap();
        let mut indicators = None;
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            indicators = Some(inspector_indicators(
                ui,
                "pane",
                ModuleKind::Waterfall,
                &pane.macros,
                &settings,
            ));
        });
        out.textures_delta.clear();
        let m = &indicators.as_ref().unwrap().values["height"];
        assert_eq!(m.macro_index, 1);
        assert_eq!(m.source, "Band energy");
        assert!((m.current - base - 1.75).abs() < 1e-5);
        assert_eq!(pane.copy_settings(), settings);
        pane.macros.banks[0][0] = Slot::default();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            assert!(
                inspector_indicators(ui, "pane", ModuleKind::Waterfall, &pane.macros, &settings)
                    .values
                    .is_empty()
            );
        });
        out.textures_delta.clear();
    }
    #[test]
    fn numeric_parameters_across_all_components_modulate_without_overwriting_saved_manual_values() {
        let fields = [
            "height",
            "thickness",
            "amplitude",
            "contrast",
            "intensity",
            "thickness",
            "brightness",
            "height",
            "speed",
            "radius",
        ];
        for (kind, field) in ModuleKind::ALL.into_iter().zip(fields) {
            let mut pane = ModulePane::new(kind);
            pane.macros.assign(kind, 0, spec(field, "Parameter"));
            pane.macros.banks[kind as usize][0].signal = Some(1);
            pane.macros.banks[kind as usize][0].depth = 0.1;
            let before = pane.copy_settings();
            let base = get(&before, field).unwrap();
            let snapshot = signals::Snapshot {
                values: [(
                    1,
                    signals::Sample {
                        value: 0.5,
                        availability: 1.0,
                        ..Default::default()
                    },
                )]
                .into(),
                ..Default::default()
            };
            let effective = pane.macros.effective(kind, &before, Some(&snapshot));
            assert!((effective[0].1 - base - 0.5).abs() < 1e-6);
            let frame = crate::analysis::AnalysisFrame {
                signals: Some(std::sync::Arc::new(snapshot)),
                ..Default::default()
            };
            let ctx = egui::Context::default();
            let mut out = ctx.run_ui(Default::default(), |ui| {
                pane.draw(
                    ui,
                    Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(600.0, 400.0)),
                    &frame,
                    &AppTheme::default(),
                    false,
                )
            });
            out.textures_delta.clear();
            assert_eq!(get(&pane.copy_settings(), field), Some(base));
            let saved = pane.snapshot();
            assert!(saved.macros.as_ref().unwrap().valid(&saved.settings));
            let encoded = serde_json::to_vec(&saved).unwrap();
            let restored: crate::modules::PaneState = serde_json::from_slice(&encoded).unwrap();
            let mut other = ModulePane::new(kind);
            other.restore(&restored);
            assert_eq!(other.macros, pane.macros);
            assert_eq!(get(&other.copy_settings(), field), Some(base));
        }
    }
    #[test]
    fn missing_sources_unmap_without_clearing_the_parameter_or_rebinding() {
        let mut state = State::default();
        state.assign(ModuleKind::Flux, 0, spec("brightness", "Brightness"));
        let mut bank = signals::Bank::default();
        let id = bank.add(signals::Method::Band).unwrap();
        state.banks[ModuleKind::Flux as usize][0].signal = Some(id);
        bank.remove_channel(id);
        state.reconcile(&bank);
        let slot = &state.banks[ModuleKind::Flux as usize][0];
        assert!(slot.parameter.is_some());
        assert!(slot.signal.is_none());
        bank.add(signals::Method::Band);
        state.reconcile(&bank);
        assert!(state.banks[ModuleKind::Flux as usize][0].signal.is_none());
    }
}

#[cfg(test)]
mod source_picker_tests {
    use super::*;
    use crate::modules::ModulePane;
    pub(super) fn position(out: &egui::FullOutput, label: &str) -> egui::Pos2 {
        out.shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.job.text == label => {
                    Some(t.pos + t.galley.size() * 0.5)
                }
                _ => None,
            })
            .min_by(|a, b| a.y.total_cmp(&b.y))
            .unwrap_or_else(|| panic!("missing {label}"))
    }
    #[test]
    fn generic_macro_picker_connects_existing_audio_and_lfo_and_disconnects_without_editing_sources()
     {
        let ctx = egui::Context::default();
        let mut pane = ModulePane::new(ModuleKind::Waterfall);
        pane.macros.assign(
            ModuleKind::Waterfall,
            0,
            ParameterSpec {
                field: "height".into(),
                label: "Height".into(),
                min: 0.0,
                max: 2.0,
                safe_min: 0.0,
                safe_max: 10.0,
                default: 0.85,
                integer: false,
                logarithmic: false,
                scale: 1.0,
            },
        );
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
        let original = shared.lock().unwrap().bank.clone();
        ctx.data_mut(|d| d.insert_temp(egui::Id::new(signals::ui::CONTEXT_KEY), shared.clone()));
        let manual = pane.copy_settings();
        let mut render = |events| {
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1400.0, 900.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    pane.performance_bar(
                        ui,
                        Rect::from_min_size(egui::pos2(0.0, 650.0), egui::vec2(1400.0, 234.0)),
                        &AppTheme::default(),
                        0,
                    );
                },
            );
            out.textures_delta.clear();
            out
        };
        let mut click = |pos| {
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
        let out = click(egui::pos2(30.0, 10.0));
        let out = click(position(&out, "Connect…"));
        let out = click(position(&out, "Choose source"));
        let out = click(position(&out, "Band energy"));
        position(&out, "Disconnect");
        let out = click(position(&out, "Band energy"));
        let out = click(position(&out, "LFO"));
        position(&out, "Disconnect");
        click(position(&out, "Disconnect"));
        assert_eq!(pane.macros.banks[0][0].signal, None);
        assert!(pane.macros.banks[0][0].parameter.is_some());
        assert_eq!(pane.macros.banks[0][0].depth, 0.35);
        assert_eq!(pane.copy_settings(), manual);
        assert_ne!(audio, lfo);
        assert_eq!(shared.lock().unwrap().bank, original);
    }
}
