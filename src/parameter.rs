//! Consistent numeric editing with separate everyday slider and safe input ranges.
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
        }
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
        let before = self.value.to_f64();
        let low = self.range.start().to_f64();
        let high = self.range.end().to_f64();
        let mut value = before;
        let inner = ui.horizontal(|ui| {
            let width = ui.available_width().min(420.0);
            let label_width = (width * 0.38).min(104.0);
            let height = ui.spacing().interact_size.y.max(22.0);
            // add_sized centers widgets, even when the label's text is aligned.
            // Give the label a right-aligned column with a fixed width instead.
            let (label_rect, _) = ui.allocate_exact_size(
                egui::vec2(label_width, height), egui::Sense::hover(),
            );
            let mut label_ui = ui.new_child(egui::UiBuilder::new()
                .max_rect(label_rect)
                .layout(egui::Layout::right_to_left(egui::Align::Center)));
            label_ui.set_clip_rect(label_rect.intersect(ui.clip_rect()));
            label_ui.add(egui::Label::new(self.text.as_ref()).truncate());
            let field_width = (width - label_width - ui.spacing().item_spacing.x)
                .min(ui.available_width()).max(32.0);
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
                    if fraction > 0.0 {
                        let fill_rect = egui::Rect::from_min_max(rect.min, egui::pos2(rect.left() + rect.width() * fraction, rect.bottom()));
                        let fill_radius = egui::CornerRadius { nw: 3, sw: 3, ne: if fraction == 1.0 { 3 } else { 0 }, se: if fraction == 1.0 { 3 } else { 0 } };
                        let opacity = if response.dragged() { 0.65 } else if response.hovered() { 0.5 } else { 0.35 };
                        painter.rect_filled(fill_rect, fill_radius, ui.visuals().selection.bg_fill.gamma_multiply(opacity));
                    }
                    if response.hovered() || response.has_focus() {
                        painter.rect_stroke(rect, radius, egui::Stroke::new(1.0, ui.visuals().selection.bg_fill.gamma_multiply(0.7)), egui::StrokeKind::Inside);
                    }
                    let formatted = if T::INTEGRAL { format!("{value:.0}") } else { egui::emath::format_with_decimals_in_range(value, 0..=3) };
                    painter.text(rect.center(), egui::Align2::CENTER_CENTER, formatted,
                        egui::TextStyle::Button.resolve(ui.style()), ui.visuals().text_color());
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
            response.help_text_with(|| format!("{}: Drag anywhere to adjust; Shift-drag for fine control. Click the number or press Enter for exact entry; double-click beside the number to reset to {}. Safe input: {} to {}.", self.text, self.default, self.safe.start(), self.safe.end()))
        });
        if T::INTEGRAL {
            value = value.round();
        }
        value = value.clamp(*self.safe.start(), *self.safe.end());
        *self.value = T::from_f64(value);
        let mut response = inner.inner.union(inner.response);
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
    fn wide_field_supports_dragging_and_inline_entry_without_value_jumps() {
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
        assert!(field.width() > 216.0 * 0.55);
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
