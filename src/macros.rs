//! Persistent per-component performance bindings. No audio/runtime data is saved.
use crate::help::HoverHelp;
use crate::{
    modules::{ModuleKind, ModuleSettings},
    signals,
    theme::AppTheme,
};
use egui::{self, Rect, Vec2};
use serde::{Deserialize, Serialize};
mod inspector;
mod menu;
pub mod scene;
pub const INSPECT_REQUEST: &str = "macro-inspect-request";
pub const INSPECT_SELECTION: &str = "macro-inspect-selection";
use menu::*;
pub(crate) fn delete_scene_slot(
    ctx: &egui::Context,
    scope: &str,
    slots: &mut [Slot; 12],
    index: usize,
) {
    menu::delete_scene_slot(ctx, scope, slots, index);
}
pub(crate) fn clear_clipboard(ctx: &egui::Context) {
    menu::clear_clipboard(ctx);
}
pub const CONTEXT: &str = "macro-parameter-context";
pub use crate::parameter::schema::{ParameterContext, ParameterSpec};
#[derive(Clone)]
pub struct ParameterDrag {
    pub context: ParameterContext,
    pub spec: ParameterSpec,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Slot {
    pub parameter: Option<ParameterSpec>,
    pub target: Option<ParameterContext>,
    pub name: Option<String>,
    pub color: Option<[u8; 3]>,
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
            target: None,
            name: None,
            color: None,
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
    pub banks: [[Slot; 12]; 18],
}
// Preserve older component banks when adding new primitives.
fn decode_banks<'de, D: serde::Deserializer<'de>>(d: D) -> Result<[[Slot; 12]; 18], D::Error> {
    let banks = Vec::<[Slot; 12]>::deserialize(d)?;
    if !(9..=18).contains(&banks.len()) {
        return Err(serde::de::Error::custom(
            "expected nine to eighteen macro banks",
        ));
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
            bank.iter().all(|slot| {
                slot.name
                    .as_ref()
                    .is_none_or(|name| crate::workspace::valid_name(name))
                    && slot.depth.is_finite()
                    && (-12.0..=12.0).contains(&slot.depth)
                    && slot.output_min.is_finite()
                    && slot.output_max.is_finite()
                    && (0.0..=1.0).contains(&slot.output_min)
                    && (slot.output_min..=1.0).contains(&slot.output_max)
                    && slot.signal.is_none_or(|id| id > 0)
                    && slot.parameter.as_ref().is_none_or(|p| {
                        p.valid()
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
    pub fn has_modulation(&self, kind: ModuleKind, snapshot: Option<&signals::Snapshot>) -> bool {
        snapshot.is_some_and(|snapshot| {
            self.banks[kind as usize].iter().any(|slot| {
                slot.enabled
                    && slot.parameter.is_some()
                    && slot
                        .signal
                        .is_some_and(|id| snapshot.values.contains_key(&id))
            })
        })
    }
    pub fn effective(
        &self,
        kind: ModuleKind,
        settings: &ModuleSettings,
        snapshot: Option<&signals::Snapshot>,
    ) -> Vec<(String, f64)> {
        if !self.has_modulation(kind, snapshot) {
            return Vec::new();
        }
        let document = settings.parameter_document().ok().unwrap_or_default();
        self.effective_document(kind, &document, snapshot)
    }
    // Apply all live values in one settings round-trip and return only the
    // affected manual fields. Restoring those fields preserves camera gestures.
    pub fn apply(
        &self,
        kind: ModuleKind,
        settings: &mut ModuleSettings,
        snapshot: Option<&signals::Snapshot>,
    ) -> Vec<(String, f64)> {
        if !self.has_modulation(kind, snapshot) {
            return Vec::new();
        }
        let Ok(mut document) = settings.parameter_document() else {
            return Vec::new();
        };
        let changes = self.effective_document(kind, &document, snapshot);
        let mut restore = Vec::with_capacity(changes.len());
        for (path, value) in changes {
            if let Some(base) = field(&document, &path).and_then(serde_json::Value::as_f64)
                && edit_field(&mut document, &path, value)
            {
                restore.push((path, base));
            }
        }
        if !restore.is_empty() {
            let Ok(updated) = settings.updated_parameters(document) else {
                return Vec::new();
            };
            *settings = updated;
        }
        restore
    }
    fn effective_document(
        &self,
        kind: ModuleKind,
        document: &serde_json::Value,
        snapshot: Option<&signals::Snapshot>,
    ) -> Vec<(String, f64)> {
        // Duplicate controls share a manual value; active modulation depths add.
        // Resolve each destination once so the live apply/restore pair stays exact.
        let mut destinations: std::collections::BTreeMap<&str, (&ParameterSpec, f64)> =
            Default::default();
        for slot in &self.banks[kind as usize] {
            let Some(p) = slot.parameter.as_ref().filter(|_| slot.enabled) else {
                continue;
            };
            let Some(sample) = snapshot.and_then(|s| slot.signal.and_then(|id| s.values.get(&id)))
            else {
                continue;
            };
            let input = if slot.invert {
                -sample.value
            } else {
                sample.value
            };
            let modulation = (slot.output_min * sample.availability
                + input * (slot.output_max - slot.output_min))
                * slot.depth;
            destinations.entry(&p.field).or_insert((p, 0.0)).1 += f64::from(modulation);
        }
        destinations
            .into_iter()
            .filter_map(|(path, (p, modulation))| {
                let base = field(document, path)?.as_f64()? / p.scale;
                let value = if p.logarithmic {
                    p.min * (p.max / p.min).powf(f64::from(p.normalized(base)) + modulation)
                } else {
                    base + modulation * (p.max - p.min)
                };
                let value = value.clamp(p.safe_min, p.safe_max);
                Some((
                    path.to_owned(),
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
    let document = settings.parameter_document().ok().unwrap_or_default();
    let values = state.banks[kind as usize]
        .iter()
        .enumerate()
        .filter_map(|(i, slot)| {
            let p = slot.parameter.as_ref()?;
            let id = slot.signal?;
            let source = panel
                .as_ref()
                .and_then(|s| s.bank.channels.iter().find(|d| d.id == id));
            let base = field(&document, &p.field)?.as_f64()?;
            Some((
                p.field.clone(),
                InspectorIndicator {
                    macro_index: i + 1,
                    source: source.map_or_else(|| "Unavailable source".into(), |d| d.name.clone()),
                    color: slot
                        .color
                        .map(|[r, g, b]| egui::Color32::from_rgb(r, g, b))
                        .unwrap_or_else(|| {
                            source
                                .map_or(ui.visuals().selection.bg_fill, signals::ui::channel_color)
                        }),
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
        value = if value.is_array() {
            value.get(key.parse::<usize>().ok()?)?
        } else {
            value.get(key)?
        };
    }
    Some(value)
}
pub fn get(settings: &ModuleSettings, path: &str) -> Option<f64> {
    field(&settings.parameter_document().ok()?, path)?.as_f64()
}
pub fn set(settings: &mut ModuleSettings, path: &str, value: f64) -> bool {
    set_many(settings, &[(path.to_owned(), value)])
}
pub fn set_many(settings: &mut ModuleSettings, changes: &[(String, f64)]) -> bool {
    if changes.is_empty() {
        return false;
    }
    let Ok(mut document) = settings.parameter_document() else {
        return false;
    };
    let mut changed = false;
    for (path, value) in changes {
        changed |= edit_field(&mut document, path, *value);
    }
    if !changed {
        return false;
    }
    let Ok(updated) = settings.updated_parameters(document) else {
        return false;
    };
    *settings = updated;
    true
}
fn edit_field(document: &mut serde_json::Value, path: &str, value: f64) -> bool {
    if !value.is_finite() {
        return false;
    }
    let Some(mut node) = document.as_object_mut().and_then(|m| m.values_mut().next()) else {
        return false;
    };
    for key in path.split('.') {
        let next = if node.is_array() {
            key.parse::<usize>().ok().and_then(|i| node.get_mut(i))
        } else {
            node.get_mut(key)
        };
        let Some(next) = next else {
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
    true
}

#[cfg(test)]
pub fn bar(
    ui: &mut egui::Ui,
    rect: Rect,
    theme: &AppTheme,
    pane: &str,
    kind: ModuleKind,
    state: &mut State,
    settings: &mut ModuleSettings,
) -> (bool, bool) {
    bar_impl(ui, rect, theme, pane, kind, state, settings, None)
}

#[allow(clippy::too_many_arguments)]
fn bar_impl(
    ui: &mut egui::Ui,
    rect: Rect,
    theme: &AppTheme,
    pane: &str,
    kind: ModuleKind,
    state: &mut State,
    settings: &mut ModuleSettings,
    mut scene: Option<&mut [scene::Component]>,
) -> (bool, bool) {
    let rect = rect.shrink2(Vec2::new(0.0, 8.0));
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .id_salt(("assignable-macros", pane, kind as usize))
            .max_rect(rect.shrink(1.0)),
    );
    child.set_clip_rect(rect.shrink(1.0).intersect(ui.clip_rect()));
    crate::workspace_ui::dock_style(&mut child);
    ui.painter()
        .rect_filled(rect, 7.0, child.visuals().extreme_bg_color);
    child.style_mut().override_font_id = Some(egui::FontId::proportional(11.0));
    child.spacing_mut().item_spacing = Vec2::new(0.0, 4.0);
    child.spacing_mut().interact_size.y = 24.0;
    let ui_key = egui::Id::new(("macro-menu-ui", pane, kind as usize));
    let mut menu_state = ui
        .data(|d| d.get_temp::<MacroMenuState>(ui_key))
        .unwrap_or_default();
    menu_state.sync(&state.banks[kind as usize]);
    let mut action = None;
    let shared =
        ui.data(|d| d.get_temp::<signals::ui::Shared>(egui::Id::new(signals::ui::CONTEXT_KEY)));
    let snapshot = shared
        .as_ref()
        .map(|s| s.lock().unwrap_or_else(|e| e.into_inner()).snapshot.clone());
    let effective = state.effective(kind, settings, snapshot.as_deref());
    if let Some(components) = scene.as_deref_mut() {
        for component in components {
            component.effective = scene::state_for(&state.banks[kind as usize], &component.target)
                .effective(
                    component.target.kind,
                    &component.settings,
                    snapshot.as_deref(),
                );
        }
    }
    let document = state.banks[kind as usize]
        .iter()
        .any(|s| s.parameter.is_some())
        .then(|| settings.parameter_document().ok().unwrap_or_default());
    let width = ((child.available_width() - 1.0) / 12.0).max(116.0);
    let row_bounds = Rect::from_min_max(
        child.available_rect_before_wrap().min,
        rect.max - Vec2::splat(1.0),
    );
    let mut row = child.new_child(
        egui::UiBuilder::new()
            .id_salt("macro-strips")
            .max_rect(row_bounds),
    );
    row.set_clip_rect(row_bounds.intersect(child.clip_rect()));
    egui::ScrollArea::horizontal()
        .id_salt("assignable-macro-row")
        .auto_shrink([false, false])
        .show(&mut row, |ui| {
            let row_height = ui.available_height();
            let mut insertion = None;
            ui.horizontal_top(|ui| {
                for i in 0..12 {
                    ui.push_id(i, |ui| {
                        let start = ui.next_widget_position();
                        let slot = &mut state.banks[kind as usize][i];
                        let strip = Rect::from_min_size(start, Vec2::new(width, row_height));
                        let header_color = slot
                            .color
                            .map(|[r, g, b]| egui::Color32::from_rgb(r, g, b))
                            .or_else(|| {
                                shared.as_ref().and_then(|shared| {
                                    let panel = shared.lock().unwrap_or_else(|e| e.into_inner());
                                    slot.signal
                                        .and_then(|id| {
                                            panel.bank.channels.iter().find(|d| d.id == id)
                                        })
                                        .map(signals::ui::channel_color)
                                })
                            })
                            .unwrap_or(if slot.parameter.is_some() {
                                theme.accent
                            } else {
                                theme.card
                            });
                        crate::workspace_ui::strip_surface(
                            ui,
                            strip,
                            header_color,
                            ui.data(|d| d.get_temp::<usize>(egui::Id::new(INSPECT_SELECTION)))
                                == Some(i),
                        );
                        let luma = 0.2126 * f32::from(header_color.r())
                            + 0.7152 * f32::from(header_color.g())
                            + 0.0722 * f32::from(header_color.b());
                        let title_color = if luma < 110.0 {
                            egui::Color32::from_gray(245)
                        } else {
                            egui::Color32::from_rgb(12, 18, 26)
                        };
                        let column = ui.vertical(|ui| {
                            ui.set_width(width);
                            ui.set_min_height(row_height);
                            let (header, _) = ui
                                .allocate_exact_size(Vec2::new(width, 28.0), egui::Sense::hover());
                            let title = slot_title(slot, i);
                            let drag = crate::workspace_ui::drag_cursor(ui
                                .interact(
                                    header,
                                    ui.id().with("macro-header"),
                                    egui::Sense::click_and_drag(),
                                ))
                                .help_text(
                                    "Click to inspect · drag to reorder · double-click to rename",
                                );
                            if drag.drag_started() {
                                drag.dnd_set_drag_payload(MacroDrag {
                                    pane: pane.to_owned(),
                                    kind,
                                    index: i,
                                });
                            }
                            if drag.double_clicked() {
                                action = Some(MacroAction::Rename(i));
                            }
                            if slot.parameter.is_some() {
                                ui.painter().text(
                                    header.left_center() + Vec2::new(7.0, 0.0),
                                    egui::Align2::LEFT_CENTER,
                                    format!("{}", i + 1),
                                    egui::FontId::proportional(11.0),
                                    title_color,
                                );
                            }
                            let header = Rect::from_min_max(
                                header.min
                                    + Vec2::new(
                                        if slot.parameter.is_some() { 24.0 } else { 8.0 },
                                        0.0,
                                    ),
                                header.max - Vec2::new(6.0, 0.0),
                            );
                            crate::workspace_ui::strip_title(ui, header, title, title_color);
                            if let Some(rename) = macro_rename_field(ui, header, i, header_color, &mut menu_state) {
                                action = Some(rename);
                            }
                            egui::Frame::new()
                                .inner_margin(egui::Margin::symmetric(8, 3))
                                .show(ui, |ui| {
                                    let width = width - 16.0;
                                    ui.set_width(width);
                                    ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
                                    if let Some(p) = slot.parameter.clone() {
                                        let component = scene.as_deref().and_then(|components| {
                                            slot.target.as_ref().and_then(|target| {
                                                components.iter().find(|c| &c.target == target)
                                            })
                                        });
                                        if scene.is_some() {
                                            let owner = component.map_or_else(
                                                || "Component unavailable".into(),
                                                |c| c.label.clone(),
                                            );
                                            ui.add_sized(
                                                [width, 14.0],
                                                egui::Label::new(
                                                    egui::RichText::new(owner.clone())
                                                        .small()
                                                        .color(theme.muted),
                                                )
                                                .truncate(),
                                            )
                                            .help_text(owner);
                                        }
                                        let base = component
                                            .map(|c| &c.document)
                                            .or(document.as_ref())
                                            .and_then(|d| field(d, &p.field))
                                            .and_then(serde_json::Value::as_f64)
                                            .unwrap_or(p.default * p.scale)
                                            / p.scale;
                                        let normalized = p.normalized(base);
                                        let current = component
                                            .map_or(&effective, |c| &c.effective)
                                            .iter()
                                            .find(|(key, _)| key == &p.field)
                                            .map_or(base, |(_, v)| *v / p.scale);
                                        let mut value = normalized;
                                        let (source, color, range) = shared.as_ref().map_or(
                                            ("Connect…".to_owned(), theme.accent, (0.0, 1.0)),
                                            |shared| {
                                                let panel = shared
                                                    .lock()
                                                    .unwrap_or_else(|e| e.into_inner());
                                                slot.signal
                                                    .and_then(|id| {
                                                        panel
                                                            .bank
                                                            .channels
                                                            .iter()
                                                            .find(|d| d.id == id)
                                                    })
                                                    .map_or(
                                                        (
                                                            "Connect…".to_owned(),
                                                            theme.accent,
                                                            (0.0, 1.0),
                                                        ),
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
                                        let color = slot
                                            .color
                                            .map(|[r, g, b]| egui::Color32::from_rgb(r, g, b))
                                            .unwrap_or(color);
                                        let map = |v: f32| {
                                            slot.output_min
                                                + (if slot.invert { -v } else { v })
                                                    * (slot.output_max - slot.output_min)
                                        };
                                        let range = (map(range.0), map(range.1));
                                        if crate::modules::performance::knob(
                                            ui,
                                            "",
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
                                            if let Some(components) = scene.as_deref_mut() {
                                                if let Some(component) =
                                                    slot.target.as_ref().and_then(|target| {
                                                        components
                                                            .iter_mut()
                                                            .find(|c| &c.target == target)
                                                    })
                                                {
                                                    set(
                                                        &mut component.settings,
                                                        &p.field,
                                                        p.value(value) * p.scale,
                                                    );
                                                }
                                            } else {
                                                set(settings, &p.field, p.value(value) * p.scale);
                                            }
                                        }
                                        let badge = ui.add_sized(
                                            [width, 24.0],
                                            egui::Button::new(
                                                egui::RichText::new(source).color(color),
                                            )
                                            .truncate(),
                                        );
                                        let editor_key =
                                            egui::Id::new("assignable-macro-source-popup");
                                        let mut opened = ui
                                            .data(|d| d.get_temp::<egui::Id>(editor_key))
                                            == Some(badge.id);
                                        if badge.clicked() {
                                            opened = !opened;
                                            if opened {
                                                ui.data_mut(|d| {
                                                    d.insert_temp(editor_key, badge.id)
                                                });
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
                                                    let panel = shared
                                                        .lock()
                                                        .unwrap_or_else(|e| e.into_inner());
                                                    let mut signal = slot.signal;
                                                    let selected = signal
                                                        .and_then(|id| {
                                                            panel
                                                                .bank
                                                                .channels
                                                                .iter()
                                                                .find(|d| d.id == id)
                                                        })
                                                        .map_or("Choose source", |d| {
                                                            d.name.as_str()
                                                        });
                                                    ui.label("Source");
                                                    crate::dropdown::ComboBox::from_id_salt("source")
                                                        .width(260.0)
                                                        .selected_text(selected)
                                                        .show_ui(ui, |ui| {
                                                            ui.selectable_value(
                                                                &mut signal,
                                                                None,
                                                                "No modulation",
                                                            );
                                                            for d in panel.bank.channels.iter().filter(|d| d.has_control_output()) {
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
                                                        [ui.available_width().max(1.0), 20.0],
                                                        egui::DragValue::new(&mut depth)
                                                            .range(-1200.0..=1200.0)
                                                            .speed(1.0)
                                                            .fixed_decimals(0)
                                                            .custom_formatter(|v, _| {
                                                                format!("{v:+.0}")
                                                            })
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
                                            Vec2::new(width, (row_height - 38.0).max(30.0)),
                                            egui::Sense::hover(),
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
                        });
                        let area = column.response.rect.union(strip);
                        if ui.rect_contains_pointer(area)
                            && ui.input(|input| input.pointer.primary_clicked())
                        {
                            ui.data_mut(|d| d.insert_temp(egui::Id::new(INSPECT_REQUEST), i));
                        }
                        let response =
                            ui.interact(area, ui.id().with("macro-drop"), egui::Sense::hover());
                        response.clone().help_fallback(|| {
                            let slot = &state.banks[kind as usize][i];
                            if let Some(parameter) = &slot.parameter {
                                format!("{}\nScene macro {} controls {}. Click to inspect; drag the knob vertically to adjust its manual value. Drag the outer ring for modulation strength, or Shift-drag for fine control. Use the source button to connect a signal. Drag the header to reorder; right-click for names, colors and editing.", slot_title(slot, i), i + 1, parameter.label)
                            } else {
                                format!("Macro {} · empty\nDrag a numeric parameter label from any component’s inspector onto this slot. One macro controls one parameter across the scene. Dropping onto an occupied slot replaces its parameter and keeps its modulation source and strength. Click to inspect; right-click for editing and color.", i + 1)
                            }
                        });
                        // The knob, value and source badge own their input responses.
                        // Observe a secondary click across the slot without stealing
                        // their primary drags/clicks from the children.
                        let right_click = ui.rect_contains_pointer(area)
                            && ui.input(|input| input.pointer.secondary_clicked());
                        egui::Popup::menu(&response)
                            .open_memory(right_click.then_some(egui::SetOpenCommand::Bool(true)))
                            .at_pointer_fixed()
                            .show(|ui| {
                                if let Some(a) = macro_menu(
                                    ui,
                                    i,
                                    &state.banks[kind as usize],
                                    pane,
                                    kind,
                                    header_color,
                                    menu_state.undo_available(),
                                ) {
                                    action = Some(a);
                                }
                            });
                        if ui.rect_contains_pointer(area)
                            && let Some(pos) = ui.input(|i| i.pointer.interact_pos())
                        {
                            let (boundary, x) = if pos.x < strip.center().x {
                                (i, strip.left())
                            } else {
                                (i + 1, strip.right())
                            };
                            insertion = Some((
                                boundary,
                                egui::pos2(x, strip.top()),
                                egui::pos2(x, strip.bottom()),
                            ));
                        }
                        if ui.rect_contains_pointer(area)
                            && let Some(payload) =
                                egui::DragAndDrop::payload::<ParameterDrag>(ui.ctx())
                        {
                            let accepted = payload.spec.valid()
                                && if let Some(components) = scene.as_deref() {
                                    components.iter().any(|c| {
                                        c.target == payload.context
                                            && get(&c.settings, &payload.spec.field).is_some()
                                    })
                                } else {
                                    payload.context.pane == pane
                                        && payload.context.kind == kind
                                        && get(settings, &payload.spec.field).is_some()
                                };
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
                                if scene.is_some() {
                                    state.banks[kind as usize][i].target =
                                        Some(payload.context.clone());
                                }
                                egui::DragAndDrop::take_payload::<ParameterDrag>(ui.ctx());
                            }
                        }
                    });
                }
            });
            if let Some((boundary, top, bottom)) = insertion
                && let Some(payload) = egui::DragAndDrop::payload::<MacroDrag>(ui.ctx())
                && payload.pane == pane
                && payload.kind == kind
            {
                ui.painter().line_segment(
                    [top, bottom],
                    egui::Stroke::new(1.5, ui.visuals().text_color()),
                );
                if ui.input(|i| i.pointer.any_released()) {
                    action = Some(MacroAction::Move(payload.index, boundary));
                    egui::DragAndDrop::take_payload::<MacroDrag>(ui.ctx());
                }
            }
        });
    if let Some(action) = action {
        apply_macro_action(ui, pane, kind, state, action, &mut menu_state);
    }
    menu_state.remember(&state.banks[kind as usize]);
    ui.data_mut(|d| d.insert_temp(ui_key, menu_state));
    crate::workspace_ui::rounded_content_corners(
        &child,
        rect.shrink(1.0),
        6.0,
        child.visuals().extreme_bg_color,
    );
    ui.painter().rect_stroke(
        rect,
        7.0,
        egui::Stroke::new(1.0, child.visuals().text_color().gamma_multiply(0.22)),
        egui::StrokeKind::Inside,
    );
    (false, false)
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
    fn twelve_bindings() -> (State, ModuleSettings, signals::Snapshot) {
        let mut state = State::default();
        let settings = ModulePane::new(ModuleKind::Sphere).copy_settings();
        for (i, field) in [
            "radius",
            "displacement",
            "stretch_x",
            "stretch_y",
            "stretch_z",
            "seconds",
            "brightness",
            "contrast",
            "opacity",
            "rear_opacity",
            "idle_brightness",
            "line_width",
        ]
        .into_iter()
        .enumerate()
        {
            state.assign(ModuleKind::Sphere, i, spec(field, field));
            state.banks[ModuleKind::Sphere as usize][i].signal = Some(1);
            state.banks[ModuleKind::Sphere as usize][i].depth = 0.05;
        }
        let snapshot = signals::Snapshot {
            values: [(
                1,
                signals::Sample {
                    value: 0.7,
                    availability: 1.0,
                    ..Default::default()
                },
            )]
            .into(),
            ..Default::default()
        };
        (state, settings, snapshot)
    }
    #[test]
    fn batched_modulation_matches_individual_updates_and_restores_only_bound_fields() {
        let (state, base, snapshot) = twelve_bindings();
        let mut expected = base.clone();
        for (path, value) in state.effective(ModuleKind::Sphere, &base, Some(&snapshot)) {
            set(&mut expected, &path, value);
        }
        let mut settings = base.clone();
        let restore = state.apply(ModuleKind::Sphere, &mut settings, Some(&snapshot));
        assert_eq!(settings, expected);
        assert_eq!(restore.len(), 12);
        assert!(set(&mut settings, "yaw", 0.75));
        assert!(set_many(&mut settings, &restore));
        let mut expected = base.clone();
        set(&mut expected, "yaw", 0.75);
        assert_eq!(settings, expected);
        assert!(
            state
                .apply(ModuleKind::Sphere, &mut settings, None)
                .is_empty()
        );
        assert_eq!(settings, expected);
        let missing = signals::Snapshot::default();
        assert!(!state.has_modulation(ModuleKind::Sphere, Some(&missing)));
        assert!(
            state
                .apply(ModuleKind::Sphere, &mut settings, Some(&missing))
                .is_empty()
        );
        let mut bypassed = state.clone();
        for slot in &mut bypassed.banks[ModuleKind::Sphere as usize] {
            slot.enabled = false;
        }
        assert!(!bypassed.has_modulation(ModuleKind::Sphere, Some(&snapshot)));
    }
    #[test]
    fn batched_numeric_edits_preserve_nested_integer_and_invalid_field_behavior() {
        let mut settings = ModulePane::new(ModuleKind::Sphere).copy_settings();
        let before = settings.clone();
        assert!(!set_many(
            &mut settings,
            &[
                ("radius".into(), f64::NAN),
                ("missing".into(), 1.0),
                ("surface_mesh".into(), 1.0),
                ("latitudes".into(), -1.0)
            ]
        ));
        assert_eq!(settings, before);
        let changes: [(String, f64); 4] = [
            ("latitudes".into(), 30.8),
            ("frequency.gain".into(), 5.0),
            ("missing".into(), 1.0),
            ("radius".into(), 2.0),
        ];
        let mut expected = before;
        for (path, value) in &changes {
            set(&mut expected, path, *value);
        }
        assert!(set_many(&mut settings, &changes));
        assert_eq!(settings, expected);
        assert_eq!(get(&settings, "latitudes"), Some(31.0));
    }
    #[test]
    #[ignore = "manual macro CPU timing, not a frame-rate test"]
    fn benchmark_macro_updates() {
        let (state, base, snapshot) = twelve_bindings();
        for batched in [false, true] {
            let start = std::time::Instant::now();
            for _ in 0..5000 {
                let mut settings = base.clone();
                if batched {
                    let restore = state.apply(ModuleKind::Sphere, &mut settings, Some(&snapshot));
                    set_many(&mut settings, &restore);
                } else {
                    let changes = state.effective(ModuleKind::Sphere, &base, Some(&snapshot));
                    for (path, value) in &changes {
                        set(&mut settings, path, *value);
                    }
                    for (path, _) in &changes {
                        set(&mut settings, path, get(&base, path).unwrap());
                    }
                }
                std::hint::black_box(settings);
            }
            eprintln!(
                "12 macros apply/restore {}: {:.3} ms/frame",
                if batched { "batched" } else { "individual" },
                start.elapsed().as_secs_f64() * 1000.0 / 5000.0
            );
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
                        Rect::from_min_size(egui::pos2(0.0, 650.0), egui::vec2(1400.0, 220.0)),
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
    fn header_drag_shows_insertion_and_context_menu_uses_the_signal_palette() {
        use super::source_picker_tests::position;
        let ctx = egui::Context::default();
        ctx.style_mut_of(egui::Theme::Dark, |s| s.animation_time = 0.0);
        ctx.style_mut_of(egui::Theme::Light, |s| s.animation_time = 0.0);
        let kind = ModuleKind::Waterfall;
        let mut pane = ModulePane::new(kind);
        pane.macros.assign(kind, 0, spec("height", "Height"));
        pane.macros.assign(kind, 1, spec("length_x", "Width"));
        pane.macros.banks[0][0].signal = Some(42);
        let saved = pane.copy_settings();
        let render = |pane: &mut ModulePane, events| {
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1500.0, 700.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    pane.performance_bar(
                        ui,
                        Rect::from_min_size(egui::pos2(0.0, 450.0), egui::vec2(1450.0, 220.0)),
                        &AppTheme::default(),
                        0,
                    );
                },
            );
            out.textures_delta.clear();
            out
        };
        let button = |pos, pressed, button| egui::Event::PointerButton {
            pos,
            pressed,
            button,
            modifiers: egui::Modifiers::NONE,
        };
        render(&mut pane, vec![]);
        let out = render(&mut pane, vec![]);
        let start = position(&out, "Height");
        let end = egui::pos2(350.0, start.y);
        render(
            &mut pane,
            vec![
                egui::Event::PointerMoved(start),
                button(start, true, egui::PointerButton::Primary),
            ],
        );
        let guide = render(&mut pane, vec![egui::Event::PointerMoved(end)]);
        assert!(guide.shapes.iter().any(|s|matches!(&s.shape, egui::Shape::LineSegment{points,stroke} if stroke.width == 1.5 && points[0].x == points[1].x && points[1].y - points[0].y > 150.0)));
        render(
            &mut pane,
            vec![button(end, false, egui::PointerButton::Primary)],
        );
        assert_eq!(
            pane.macros.banks[0][2].parameter.as_ref().unwrap().field,
            "height"
        );
        assert_eq!(pane.macros.banks[0][2].signal, Some(42));
        let out = render(&mut pane, vec![]);
        let pos = position(&out, "Height");
        for pressed in [true, false] {
            render(
                &mut pane,
                vec![
                    egui::Event::PointerMoved(pos),
                    button(pos, pressed, egui::PointerButton::Secondary),
                ],
            );
        }
        let out = render(&mut pane, vec![]);
        for label in [
            "Cut",
            "Copy",
            "Paste",
            "Duplicate",
            "Delete",
            "Rename",
            "Insert empty macro",
            "Macro color",
        ] {
            position(&out, label);
        }
        let swatch = out
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Rect(r)
                    if r.fill == egui::Color32::from_rgb(255, 148, 166)
                        && r.rect.size() == egui::vec2(16.0, 16.0) =>
                {
                    Some(r.rect.center())
                }
                _ => None,
            })
            .expect("same first swatch as Signals");
        for pressed in [true, false] {
            render(
                &mut pane,
                vec![
                    egui::Event::PointerMoved(swatch),
                    button(swatch, pressed, egui::PointerButton::Primary),
                ],
            );
        }
        assert_eq!(pane.macros.banks[0][2].color, Some([255, 148, 166]));
        assert_eq!(pane.copy_settings(), saved);
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
                            Rect::from_min_size(egui::pos2(0.0, 650.0), egui::vec2(1400.0, 220.0)),
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
            let remove = position(&out, "Delete");
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
            "cylinder.height",
            "amplitude",
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
                        Rect::from_min_size(egui::pos2(0.0, 650.0), egui::vec2(1400.0, 220.0)),
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

#[cfg(test)]
mod inline_title_tests {
    use super::*;
    fn render(
        ctx: &egui::Context,
        state: &mut State,
        settings: &mut ModuleSettings,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(1400.0, 700.0),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                bar(
                    ui,
                    Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(1400.0, 320.0)),
                    &AppTheme::default(),
                    "inline-test",
                    ModuleKind::Waterfall,
                    state,
                    settings,
                );
            },
        );
        out.textures_delta.clear();
        out
    }
    fn key(key: egui::Key) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }
    }
    #[test]
    fn double_click_edits_macro_in_its_header_with_replace_cancel_and_click_away() {
        for ending in [Some(egui::Key::Enter), Some(egui::Key::Escape), None] {
            let ctx = egui::Context::default();
            let mut state = State::default();
            state.banks[0][0].name = Some("Original".into());
            state.banks[0][0].color = Some([85, 170, 230]);
            let mut settings =
                crate::modules::ModulePane::new(ModuleKind::Waterfall).copy_settings();
            let original_settings = settings.clone();
            let out = render(&ctx, &mut state, &mut settings, vec![]);
            let pos = out
                .shapes
                .iter()
                .find_map(|s| match &s.shape {
                    egui::Shape::Text(t) if t.galley.job.text == "Original" => {
                        Some(t.pos + t.galley.size() * 0.5)
                    }
                    _ => None,
                })
                .unwrap();
            for _ in 0..2 {
                render(
                    &ctx,
                    &mut state,
                    &mut settings,
                    vec![egui::Event::PointerMoved(pos)],
                );
                for pressed in [true, false] {
                    render(
                        &ctx,
                        &mut state,
                        &mut settings,
                        vec![egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        }],
                    );
                }
            }
            let out = render(&ctx, &mut state, &mut settings, vec![]);
            assert!(crate::shortcuts::editing_text(&ctx));
            assert!(!out.shapes.iter().any(
                |s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.job.text == "Rename macro")
            ));
            // Delete edits the selected title text; it must not remove the macro.
            render(
                &ctx,
                &mut state,
                &mut settings,
                vec![key(egui::Key::Delete)],
            );
            assert_eq!(state.banks[0][0].name.as_deref(), Some("Original"));
            // Typing replaces the selected original name without a separate dialog.
            render(
                &ctx,
                &mut state,
                &mut settings,
                vec![egui::Event::Text("  Northern lights  ".into())],
            );
            if let Some(keycode) = ending {
                render(&ctx, &mut state, &mut settings, vec![key(keycode)]);
            } else {
                let pos = egui::pos2(1100.0, 550.0);
                for pressed in [true, false] {
                    render(
                        &ctx,
                        &mut state,
                        &mut settings,
                        vec![
                            egui::Event::PointerMoved(pos),
                            egui::Event::PointerButton {
                                pos,
                                button: egui::PointerButton::Primary,
                                pressed,
                                modifiers: egui::Modifiers::NONE,
                            },
                        ],
                    );
                }
                render(&ctx, &mut state, &mut settings, vec![]);
            }
            assert_eq!(
                state.banks[0][0].name.as_deref(),
                Some(if ending == Some(egui::Key::Escape) {
                    "Original"
                } else {
                    "Northern lights"
                })
            );
            assert_eq!(settings, original_settings);
            // Stored edits remain normal serialized macro names.
            let saved: State =
                serde_json::from_slice(&serde_json::to_vec(&state).unwrap()).unwrap();
            assert_eq!(saved, state);
        }
    }
}
