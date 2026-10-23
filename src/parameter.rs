//! Consistent numeric editing with separate everyday slider and safe input ranges.
pub mod schema;
use crate::{help::HoverHelp, issues};
use eframe::egui::{self, Response, Widget, emath::Numeric};
use std::ops::RangeInclusive;
use std::{borrow::Cow, fmt::Display};

pub struct Parameter<'a, T: Numeric + Display> {
    value: &'a mut T,
    range: RangeInclusive<T>,
    safe: RangeInclusive<f64>,
    default: T,
    text: Cow<'a, str>,
    logarithmic: bool,
    key: Option<String>,
    scale: f64,
}
impl<'a, T: Numeric + Display> Parameter<'a, T> {
    pub fn new(value: &'a mut T, range: RangeInclusive<T>, default: T) -> Self {
        let safe = range.start().to_f64()..=range.end().to_f64();
        Self {
            value,
            range,
            safe,
            default,
            text: Cow::Borrowed(""),
            logarithmic: false,
            key: None,
            scale: 1.0,
        }
    }
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }
    pub fn raw_scale(mut self, scale: f64) -> Self {
        self.scale = scale;
        self
    }
    pub fn text(mut self, text: impl Into<Cow<'a, str>>) -> Self {
        self.text = text.into();
        self
    }
    pub fn logarithmic(mut self, enabled: bool) -> Self {
        self.logarithmic = enabled;
        self
    }
    pub fn bounds(mut self, safe: RangeInclusive<f64>) -> Self {
        self.safe = safe;
        self
    }
}

fn parse(input: &str, safe: &RangeInclusive<f64>, integer: bool) -> Result<f64, String> {
    let value = input
        .trim()
        .parse::<f64>()
        .map_err(|_| "Enter a number (scientific notation is supported).".to_string())?;
    if !value.is_finite() {
        return Err("NaN and infinity are not supported.".into());
    }
    if integer && value.fract() != 0.0 {
        return Err("This parameter requires a whole number.".into());
    }
    if !safe.contains(&value) {
        return Err(format!("Safe range is {} to {}.", safe.start(), safe.end()));
    }
    Ok(value)
}

impl<T: Numeric + Display> Widget for Parameter<'_, T> {
    fn ui(self, ui: &mut egui::Ui) -> Response {
        let id = self.text.clone();
        ui.push_id(id, |ui| self.render(ui)).inner
    }
}

impl<T: Numeric + Display> Parameter<'_, T> {
    fn render(self, ui: &mut egui::Ui) -> Response {
        #[cfg(test)]
        if let Some(key) = &self.key {
            ui.data_mut(|d| {
                let id = egui::Id::new("test-rendered-parameter-keys");
                let mut keys = d
                    .get_temp::<std::collections::BTreeSet<String>>(id)
                    .unwrap_or_default();
                keys.insert(key.clone());
                d.insert_temp(id, keys);
            });
        }
        let before = self.value.to_f64();
        let low = self.range.start().to_f64();
        let high = self.range.end().to_f64();
        let descriptor = self.key.as_ref().and_then(|key| {
            let target = ui.data(|d| {
                d.get_temp::<schema::ParameterContext>(egui::Id::new(crate::macros::CONTEXT))
            })?;
            Some((
                target,
                schema::ParameterSpec {
                    field: key.clone(),
                    label: self.text.to_string(),
                    min: low,
                    max: high,
                    safe_min: *self.safe.start(),
                    safe_max: *self.safe.end(),
                    default: self.default.to_f64(),
                    integer: T::INTEGRAL,
                    logarithmic: self.logarithmic,
                    scale: self.scale,
                },
            ))
        });
        let midi_mapped = descriptor.as_ref().is_some_and(|(target, spec)| {
            crate::midi::mapping::mapped(ui.ctx(), target, &spec.field)
        });
        let mut value = before;
        let inspector = crate::modules::inspector_bounds(ui).is_some();
        let modulation = self.key.as_ref().and_then(|key| {
            ui.data(|d| {
                let context = d.get_temp::<crate::macros::ParameterContext>(egui::Id::new(
                    crate::macros::CONTEXT,
                ))?;
                let indicators = d.get_temp::<crate::macros::InspectorIndicators>(
                    egui::Id::new(crate::macros::INDICATORS),
                )?;
                (indicators.pane == context.pane && indicators.kind == context.kind)
                    .then(|| indicators.values.get(key).cloned())
                    .flatten()
            })
        });
        let layout = egui::Layout::left_to_right(egui::Align::Center);
        let mut row_rect = ui.available_rect_before_wrap();
        row_rect.max.y = row_rect.top() + ui.spacing().interact_size.y.max(22.0);
        crate::modules::property_background(ui, row_rect);
        let inner = ui.scope_builder(egui::UiBuilder::new().layout(layout).max_rect(row_rect), |ui| {
            let width = ui.available_width().min(420.0);
            let label_width = if inspector { width * 0.43 } else { (width * 0.38).min(104.0) };
            let height = ui.spacing().interact_size.y.max(22.0);
            // Fixed columns align labels and numeric fields throughout the inspector.
            let (label_rect, _) = ui.allocate_exact_size(
                egui::vec2(label_width, height), egui::Sense::hover(),
            );
            let mut label_ui = ui.new_child(egui::UiBuilder::new()
                .max_rect(label_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)));
            label_ui.set_clip_rect(label_rect.intersect(ui.clip_rect()));
            if let Some(m) = &modulation {
                let (badge, response) = label_ui.allocate_exact_size(egui::vec2(10.0,height),egui::Sense::hover());
                let color = if m.enabled { m.color } else { ui.visuals().weak_text_color() };
                label_ui.painter().circle_stroke(badge.center(),3.5,egui::Stroke::new(1.0,color));
                if m.enabled { label_ui.painter().circle_filled(badge.center(),2.0,color); }
                response.help_text(format!("Macro {} · {}\n{} · depth {:+.0}%\nBase {:.3} · current {:.3}",
                    m.macro_index,m.source,if m.enabled {"Modulating"} else {"Bypassed"},m.depth*100.0,before,m.current));
            }
            if midi_mapped {
                label_ui.add(egui::Label::new(egui::RichText::new("M").small().color(ui.visuals().selection.stroke.color)))
                    .help_text("MIDI controller assigned. Right-click to relearn or remove; Settings → MIDI lists all scene mappings.");
            }
            label_ui.add(egui::Label::new(self.text.as_ref()).truncate());
            if let Some((context, spec)) = &descriptor {
                let drag=label_ui.interact(label_rect,label_ui.id().with("macro-parameter-drag"),egui::Sense::drag())
                    .help_text(modulation.as_ref().map_or_else(|| "Drag this parameter label onto a macro slot".into(), |m| {
                        format!("Macro {} · {}\n{} · depth {:+.0}%\nBase {:.3} · current {:.3}\nDrag this label onto a macro slot to reassign it.",
                            m.macro_index,m.source,if m.enabled {"Modulating"} else {"Bypassed"},m.depth*100.0,before,m.current)
                    }));
                if drag.drag_started() {
                    drag.dnd_set_drag_payload(crate::macros::ParameterDrag {
                        context: context.clone(), spec: spec.clone()
                    });
                }
                crate::workspace_ui::drag_cursor(drag);
            }

            let field_width = (width - label_width - ui.spacing().item_spacing.x).min(ui.available_width()).max(32.0);
            let (rect, mut response) = ui.allocate_exact_size(
                egui::vec2(field_width, height), egui::Sense::click_and_drag(),
            );
            let edit_id = response.id.with("typed-value");
            let error_id = response.id.with("error");
            let drag_id = response.id.with("drag-value");
            let stored = ui.data(|d| d.get_temp::<String>(edit_id));
            if let Some(mut draft) = stored {
                let edit = ui.put(rect, egui::TextEdit::singleline(&mut draft)
                    .id(edit_id).desired_width(0.0).horizontal_align(egui::Align::Center));
                let cancel = ui.input(|i| i.key_pressed(egui::Key::Escape));
                let commit = edit.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter));
                if cancel || commit {
                    if !cancel {
                        let context = ui.data(|d| d.get_temp::<String>(egui::Id::new("parameter-context")))
                            .unwrap_or_else(|| "Module".into());
                        match parse(&draft, &self.safe, T::INTEGRAL) {
                            Ok(parsed) => {
                                value = parsed;
                                if parsed != before { issues::note(&format!("{context} · {}", self.text), &format!("Typed value changed from {before} to {parsed}.")); }
                                ui.data_mut(|d| d.remove::<String>(error_id));
                            },
                            Err(reason) => {
                                issues::record(&format!("{context} · {}", self.text), &format!("Rejected {draft:?}; retained {before}. {reason}"));
                                ui.data_mut(|d| d.insert_temp(error_id, reason));
                            },
                        }
                    }
                    ui.data_mut(|d| d.remove::<String>(edit_id));
                    edit.surrender_focus();
                } else { ui.data_mut(|d| d.insert_temp(edit_id, draft)); }
                response = response.union(edit);
            } else {
                // A single field provides the track and value. Relative dragging
                // avoids jumping when a drag starts on the centered number.
                if response.dragged() {
                    let previous = ui.data(|d| d.get_temp::<f64>(drag_id)).unwrap_or(before);
                    let delta = if response.drag_started() { response.drag_delta().x } else { ui.input(|i| i.pointer.delta().x) } as f64;
                    let precision = if ui.input(|i| i.modifiers.shift) { 0.1 } else { 1.0 };
                    let step = delta / field_width as f64 * precision;
                    value = if self.logarithmic && low > 0.0 {
                        previous * ((high / low).ln() * step).exp()
                    } else { previous + (high - low) * step };
                    value = value.clamp(*self.safe.start(), *self.safe.end());
                    // Keep fractional movement for integer controls and fine drags.
                    ui.data_mut(|d| d.insert_temp(drag_id, value));
                } else { ui.data_mut(|d| d.remove::<f64>(drag_id)); }

                let number_rect = egui::Rect::from_center_size(rect.center(), egui::vec2(56.0_f32.min(field_width), height));
                if response.double_clicked() {
                    value = self.default.to_f64();
                    ui.data_mut(|d| d.remove::<String>(error_id));
                } else if response.clicked() {
                    if response.interact_pointer_pos().is_some_and(|pos| number_rect.contains(pos)) {
                        ui.data_mut(|d| d.insert_temp(edit_id, self.value.to_string()));
                        ui.memory_mut(|m| m.request_focus(edit_id));
                    } else if let Some(pos) = response.interact_pointer_pos() {
                        let fraction = ((pos.x - rect.left()) / field_width).clamp(0.0, 1.0) as f64;
                        value = if self.logarithmic && low > 0.0 {
                            low * (high / low).powf(fraction)
                        } else { low + (high - low) * fraction };
                    }
                }
                let accessibility_direction = ui.input(|i| {
                    i.num_accesskit_action_requests(response.id, egui::accesskit::Action::Increment) as f64
                        - i.num_accesskit_action_requests(response.id, egui::accesskit::Action::Decrement) as f64
                });
                let has_focus = response.has_focus();
                if ui.is_enabled() && (has_focus || accessibility_direction != 0.0) {
                    ui.memory_mut(|m| m.set_focus_lock_filter(response.id, egui::EventFilter {
                        horizontal_arrows: true, ..Default::default()
                    }));
                    let (direction, precision, enter) = ui.input_mut(|i| (
                        accessibility_direction + if has_focus { i.num_presses(egui::Key::ArrowRight) as f64 - i.num_presses(egui::Key::ArrowLeft) as f64 } else { 0.0 },
                        if i.modifiers.shift { 0.1 } else { 1.0 },
                        has_focus && i.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
                    ));
                    if direction != 0.0 {
                        let next = if self.logarithmic && low > 0.0 {
                            value * ((high / low).ln() * direction * precision / 100.0).exp()
                        } else { value + (high - low) * direction * precision / 100.0 };
                        value = if T::INTEGRAL && (next - value).abs() < 1.0 { value + direction } else { next };
                    }
                    if enter {
                        ui.data_mut(|d| d.insert_temp(edit_id, T::from_f64(value).to_string()));
                        ui.memory_mut(|m| m.request_focus(edit_id));
                    }
                }
                if ui.is_enabled() {
                    ui.input(|i| {
                        for request in i.accesskit_action_requests(response.id, egui::accesskit::Action::SetValue) {
                            if let Some(egui::accesskit::ActionData::NumericValue(requested)) = request.data
                                && requested.is_finite() {
                                value = requested;
                            }
                        }
                    });
                }
                if T::INTEGRAL { value = value.round(); }
                value = value.clamp(*self.safe.start(), *self.safe.end());
                let fraction = if self.logarithmic && low > 0.0 {
                    (value.max(low) / low).ln() / (high / low).ln()
                } else { (value - low) / (high - low) };
                let fraction = if fraction.is_finite() { fraction.clamp(0.0, 1.0) as f32 } else { 0.0 };
                if ui.is_rect_visible(rect) {
                    let painter = ui.painter_at(rect);
                    let radius = egui::CornerRadius::same(3);
                    painter.rect_filled(rect, radius, ui.visuals().extreme_bg_color);
                    // A slim progress track leaves the value readable at every level.
                    let track = egui::Rect::from_min_max(rect.left_bottom() + egui::vec2(4.0, -4.0), rect.right_bottom() + egui::vec2(-4.0, -2.0));
                    painter.rect_filled(track, 1.0, ui.visuals().widgets.noninteractive.bg_stroke.color);
                    if fraction > 0.0 {
                        let fill = egui::Rect::from_min_max(track.min, egui::pos2(track.left() + track.width() * fraction, track.bottom()));
                        painter.rect_filled(fill, 1.0, ui.visuals().selection.bg_fill.gamma_multiply(if response.dragged() { 1.0 } else { 0.8 }));
                    }
                    if let Some(m) = &modulation {
                        let current = if self.logarithmic && low > 0.0 {
                            (m.current.max(low) / low).ln() / (high / low).ln()
                        } else { (m.current-low)/(high-low) };
                        let current = if current.is_finite() { current.clamp(0.0,1.0) as f32 } else { 0.0 };
                        let color = if m.enabled { m.color } else { ui.visuals().weak_text_color() };
                        painter.line_segment([egui::pos2(track.left()+track.width()*fraction,track.center().y),
                            egui::pos2(track.left()+track.width()*current,track.center().y)],egui::Stroke::new(2.0,color.gamma_multiply(0.6)));
                        painter.circle_filled(egui::pos2(track.left()+track.width()*current,track.center().y),2.5,color);
                    }
                    painter.rect_stroke(rect, radius, ui.visuals().widgets.inactive.bg_stroke, egui::StrokeKind::Inside);
                    if response.hovered() || response.has_focus() {
                        painter.rect_stroke(rect, radius, egui::Stroke::new(1.0, ui.visuals().selection.bg_fill.gamma_multiply(0.7)), egui::StrokeKind::Inside);
                    }
                    let formatted = if T::INTEGRAL { format!("{value:.0}") } else { egui::emath::format_with_decimals_in_range(value, 0..=3) };
                    painter.text(rect.center(), egui::Align2::CENTER_CENTER, formatted,
                        egui::FontId::monospace(12.0), ui.visuals().text_color());
                }
                response = response.on_hover_and_drag_cursor(egui::CursorIcon::ResizeHorizontal);
            }
            response.widget_info(|| egui::WidgetInfo::slider(ui.is_enabled(), value, self.text.as_ref()));
            ui.ctx().accesskit_node_builder(response.id, |node| {
                node.set_min_numeric_value(*self.safe.start());
                node.set_max_numeric_value(*self.safe.end());
                if ui.is_enabled() {
                    node.add_action(egui::accesskit::Action::SetValue);
                    if value < *self.safe.end() { node.add_action(egui::accesskit::Action::Increment); }
                    if value > *self.safe.start() { node.add_action(egui::accesskit::Action::Decrement); }
                }
            });
            response.help_fallback(|| format!("{}\n{}\n\nDrag anywhere to adjust; Shift-drag for fine control. Click the number or press Enter for exact entry; double-click beside the number to reset to {}. Safe input: {} to {}.", self.text, crate::help::control_description(&self.text), self.default, self.safe.start(), self.safe.end()))
        });
        if T::INTEGRAL {
            value = value.round();
        }
        value = value.clamp(*self.safe.start(), *self.safe.end());
        *self.value = T::from_f64(value);
        let mut response = inner.inner.union(inner.response);
        if let Some((target, spec)) = &descriptor {
            response.context_menu(|ui| {
                ui.strong(&spec.label);
                ui.weak(format!(
                    "Range {}–{} · default {}",
                    spec.min, spec.max, spec.default
                ));
                if ui.button("Reset to default").clicked() {
                    *self.value = self.default;
                    ui.close();
                }
                ui.separator();
                crate::midi::mapping::menu(ui, target, spec);
            });
        }
        if self.value.to_f64() != before {
            response.mark_changed();
            ui.data_mut(|d| d.remove::<String>(response.id.with("error")));
        }
        if let Some(reason) = ui.data(|d| d.get_temp::<String>(response.id.with("error"))) {
            ui.colored_label(
                ui.visuals().warn_fg_color,
                format!("{reason} See Issue log."),
            );
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parameter_context_menu_can_begin_midi_learn_without_editing_value() {
        let ctx = egui::Context::default();
        let mut value = 0.85_f32;
        ctx.data_mut(|d| {
            d.insert_temp(
                egui::Id::new(crate::macros::CONTEXT),
                schema::ParameterContext {
                    pane: "test-component".into(),
                    kind: crate::modules::ModuleKind::Waterfall,
                },
            )
        });
        let mut render = |events| {
            let mut bounds = egui::Rect::NOTHING;
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(800.0, 600.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    bounds = ui
                        .add(
                            Parameter::new(&mut value, 0.0..=2.0, 0.85)
                                .key("height")
                                .text("Height"),
                        )
                        .rect;
                },
            );
            out.textures_delta.clear();
            (bounds, out)
        };
        let (bounds, _) = render(vec![]);
        render(vec![egui::Event::PointerMoved(bounds.center())]);
        for pressed in [true, false] {
            render(vec![egui::Event::PointerButton {
                pos: bounds.center(),
                button: egui::PointerButton::Secondary,
                pressed,
                modifiers: Default::default(),
            }]);
        }
        let (_, out) = render(vec![]);
        let learn = out
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.job.text == "Learn MIDI controller…" => {
                    Some(t.pos + t.galley.rect.center().to_vec2())
                }
                _ => None,
            })
            .expect("MIDI Learn in parameter context menu");
        render(vec![egui::Event::PointerMoved(learn)]);
        for pressed in [true, false] {
            render(vec![egui::Event::PointerButton {
                pos: learn,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            }]);
        }
        assert_eq!(value, 0.85);
        assert!(
            crate::midi::mapping::Controller::default()
                .pending_change(&ctx, &crate::midi::Snapshot::default())
        );
    }

    fn frame(ctx: &egui::Context, value: &mut f32, input: egui::RawInput) -> Response {
        frame_with_cursor(ctx, value, input).0
    }

    fn frame_with_cursor(
        ctx: &egui::Context,
        value: &mut f32,
        input: egui::RawInput,
    ) -> (Response, egui::CursorIcon) {
        let mut response = None;
        let mut output = ctx.run_ui(input, |ui| {
            ui.data_mut(|data| data.insert_temp(egui::Id::new("inspector-bounds"), ui.max_rect()));
            response = Some(
                ui.add(
                    Parameter::new(value, 0.25..=10.0, 1.0)
                        .bounds(0.01..=100.0)
                        .text("Length X"),
                ),
            );
        });
        output.textures_delta.clear();
        (response.unwrap(), output.platform_output.cursor_icon)
    }

    #[test]
    fn exact_entry_uses_native_precision_and_cancel_preserves_the_value() {
        for original in [0.85_f32, 1.234567_f32] {
            let ctx = egui::Context::default();
            let mut value = original;
            let response = frame(&ctx, &mut value, Default::default());
            ctx.memory_mut(|memory| memory.request_focus(response.id));
            for key in [egui::Key::Enter, egui::Key::Escape] {
                frame(
                    &ctx,
                    &mut value,
                    egui::RawInput {
                        events: vec![egui::Event::Key {
                            key,
                            physical_key: None,
                            pressed: true,
                            repeat: false,
                            modifiers: egui::Modifiers::NONE,
                        }],
                        ..Default::default()
                    },
                );
                let draft =
                    ctx.data(|data| data.get_temp::<String>(response.id.with("typed-value")));
                if key == egui::Key::Enter {
                    assert_eq!(draft, Some(original.to_string()));
                } else {
                    assert!(draft.is_none());
                }
                assert_eq!(value, original);
            }
        }
    }

    #[test]
    fn typed_values_apply_beyond_slider_range_and_invalid_values_keep_last_valid_setting() {
        let ctx = egui::Context::default();
        let mut value = 2.0;
        let response = frame(&ctx, &mut value, Default::default());
        for (draft, expected) in [("2e1", 20.0), ("NaN", 20.0), ("1000", 20.0)] {
            let id = response.id.with("typed-value");
            ctx.data_mut(|d| d.insert_temp(id, draft.to_string()));
            ctx.memory_mut(|m| m.request_focus(id));
            frame(
                &ctx,
                &mut value,
                egui::RawInput {
                    events: vec![egui::Event::Key {
                        key: egui::Key::Enter,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                    }],
                    ..Default::default()
                },
            );
            assert_eq!(value, expected);
            frame(&ctx, &mut value, Default::default());
            assert_eq!(value, expected);
        }
        assert!(crate::issues::report().contains("Rejected \"NaN\""));
        assert!(
            ctx.data(|d| d.get_temp::<String>(response.id.with("error")))
                .is_some()
        );
        ctx.memory_mut(|memory| memory.request_focus(response.id));
        let corrected = frame(
            &ctx,
            &mut value,
            egui::RawInput {
                events: vec![egui::Event::Key {
                    key: egui::Key::ArrowRight,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..Default::default()
            },
        );
        assert!(corrected.changed());
        assert!(
            ctx.data(|d| d.get_temp::<String>(response.id.with("error")))
                .is_none()
        );
    }

    #[test]
    fn double_click_resets_and_shift_drag_is_fine() {
        let ctx = egui::Context::default();
        let mut value = 5.0;
        let response = frame(&ctx, &mut value, Default::default());
        let start =
            ctx.read_response(response.id).unwrap().rect.left_center() + egui::vec2(8.0, 0.0);
        let pointer = |pos, pressed, modifiers| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers,
        };
        let shift = egui::Modifiers::SHIFT;
        frame(
            &ctx,
            &mut value,
            egui::RawInput {
                events: vec![
                    egui::Event::ModifiersChanged(shift),
                    egui::Event::PointerMoved(start),
                    pointer(start, true, shift),
                ],
                ..Default::default()
            },
        );
        let initial = value;
        for offset in [4.0, 8.0, 12.0] {
            frame(
                &ctx,
                &mut value,
                egui::RawInput {
                    events: vec![egui::Event::PointerMoved(start + egui::vec2(offset, 0.0))],
                    ..Default::default()
                },
            );
        }
        assert!((value - initial).abs() < 0.5);
        frame(
            &ctx,
            &mut value,
            egui::RawInput {
                events: vec![pointer(start, false, shift)],
                ..Default::default()
            },
        );
        for time in [1.0, 1.1] {
            frame(
                &ctx,
                &mut value,
                egui::RawInput {
                    time: Some(time),
                    events: vec![
                        egui::Event::PointerMoved(start),
                        pointer(start, true, egui::Modifiers::NONE),
                    ],
                    ..Default::default()
                },
            );
            frame(
                &ctx,
                &mut value,
                egui::RawInput {
                    time: Some(time + 0.02),
                    events: vec![pointer(start, false, egui::Modifiers::NONE)],
                    ..Default::default()
                },
            );
        }
        assert_eq!(value, 1.0);
    }
    #[test]
    fn compact_field_supports_dragging_and_inline_entry_without_value_jumps() {
        let ctx = egui::Context::default();
        let mut value = 2.0;
        let render = |value: &mut f32, events| {
            frame_with_cursor(
                &ctx,
                value,
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(216.0, 200.0),
                    )),
                    events,
                    ..Default::default()
                },
            )
        };
        let (response, _) = render(&mut value, vec![]);
        let field = ctx.read_response(response.id).unwrap().rect;
        assert!(field.width() > 216.0 * 0.45);
        assert!(field.right() <= 216.0);
        let center = field.center();
        assert_eq!(
            render(&mut value, vec![egui::Event::PointerMoved(center)]).1,
            egui::CursorIcon::ResizeHorizontal,
        );
        let pointer = |pos, pressed| egui::Event::PointerButton {
            pos,
            pressed,
            button: egui::PointerButton::Primary,
            modifiers: egui::Modifiers::NONE,
        };
        render(
            &mut value,
            vec![egui::Event::PointerMoved(center), pointer(center, true)],
        );
        assert_eq!(
            value, 2.0,
            "pressing the field must not jump to the pointer value"
        );
        let end = center + egui::vec2(15.0, 0.0);
        assert_eq!(
            render(&mut value, vec![egui::Event::PointerMoved(end)]).1,
            egui::CursorIcon::ResizeHorizontal,
        );
        render(&mut value, vec![pointer(end, false)]);
        assert!(value > 2.0 && value < 4.0);
        let edit_id = response.id.with("typed-value");
        assert!(ctx.data(|d| d.get_temp::<String>(edit_id)).is_none());

        let before = value;
        for pressed in [true, false] {
            render(
                &mut value,
                vec![egui::Event::PointerMoved(center), pointer(center, pressed)],
            );
        }
        assert_eq!(value, before, "clicking the number only opens the editor");
        assert!(ctx.data(|d| d.get_temp::<String>(edit_id)).is_some());
        render(&mut value, vec![]);
        assert_eq!(
            render(&mut value, vec![egui::Event::PointerMoved(center)]).1,
            egui::CursorIcon::Text,
        );
        ctx.data_mut(|d| d.insert_temp(edit_id, "75".to_string()));
        render(
            &mut value,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert_eq!(value, before);
        assert!(ctx.data(|d| d.get_temp::<String>(edit_id)).is_none());

        ctx.memory_mut(|m| m.request_focus(response.id));
        render(
            &mut value,
            vec![egui::Event::Key {
                key: egui::Key::ArrowRight,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert!(value > before);
    }

    #[test]
    fn custom_input_accepts_scientific_values_but_rejects_unsafe_numbers() {
        assert_eq!(parse("2e1", &(0.01..=100.0), false).unwrap(), 20.0);
        for invalid in ["NaN", "inf", "1e999", "-1", "101", "abc"] {
            assert!(parse(invalid, &(0.01..=100.0), false).is_err());
        }
        assert!(parse("2.5", &(1.0..=128.0), true).is_err());
    }
}
