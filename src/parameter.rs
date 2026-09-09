//! Consistent numeric editing with separate everyday slider and safe input ranges.
use crate::{help::HoverHelp, issues};
use eframe::egui::{self, Response, Widget, emath::Numeric};
use std::ops::RangeInclusive;

pub struct Parameter<'a, T: Numeric> {
    value: &'a mut T,
    range: RangeInclusive<T>,
    safe: RangeInclusive<f64>,
    default: T,
    text: String,
    logarithmic: bool,
}
impl<'a, T: Numeric> Parameter<'a, T> {
    pub fn new(value: &'a mut T, range: RangeInclusive<T>, default: T) -> Self {
        let safe = range.start().to_f64()..=range.end().to_f64();
        Self {
            value,
            range,
            safe,
            default,
            text: String::new(),
            logarithmic: false,
        }
    }
    pub fn text(mut self, text: impl Into<String>) -> Self {
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

impl<T: Numeric> Widget for Parameter<'_, T> {
    fn ui(self, ui: &mut egui::Ui) -> Response {
        let id = self.text.clone();
        ui.push_id(id, |ui| self.render(ui)).inner
    }
}

impl<T: Numeric> Parameter<'_, T> {
    fn render(self, ui: &mut egui::Ui) -> Response {
        let before = self.value.to_f64();
        let low = self.range.start().to_f64();
        let high = self.range.end().to_f64();
        let mut value = before;
        let context = ui
            .data(|d| d.get_temp::<String>(egui::Id::new("parameter-context")))
            .unwrap_or_else(|| "Module".into());
        let inner = ui.horizontal(|ui| {
            let width = ui.available_width().min(420.0);
            let label_width = (width * 0.42).min(108.0);
            let value_width = 48.0;
            ui.add_sized(
                [label_width, ui.spacing().interact_size.y],
                egui::Label::new(&self.text).truncate().halign(egui::Align::RIGHT),
            );
            ui.spacing_mut().slider_width =
                (width - label_width - value_width - ui.spacing().item_spacing.x * 2.0).max(16.0);
            let bar = ui.add(egui::Slider::new(&mut value, low..=high).show_value(false)
                .clamping(egui::SliderClamping::Never).logarithmic(self.logarithmic))
                .interact(egui::Sense::click_and_drag());
            let fine_id = bar.id.with("fine-drag");
            if bar.is_pointer_button_down_on() && ui.input(|i| i.modifiers.shift) {
                let mut fine = ui.data(|d| d.get_temp::<f64>(fine_id)).unwrap_or(before);
                let delta = if bar.drag_started() { 0.0 } else { ui.input(|i| i.pointer.delta().x) as f64 };
                fine = if self.logarithmic && low > 0.0 {
                    fine * ((high / low).ln() * delta / bar.rect.width() as f64 * 0.1).exp()
                } else { fine + (high - low) * delta / bar.rect.width() as f64 * 0.1 };
                value = fine.clamp(*self.safe.start(), *self.safe.end());
                ui.data_mut(|d| d.insert_temp(fine_id, value));
            } else { ui.data_mut(|d| d.remove::<f64>(fine_id)); }
            if bar.double_clicked() { value = self.default.to_f64(); ui.data_mut(|d| d.remove::<String>(bar.id.with("error"))); }
            bar.clone().help_text(format!("{}: Shift-drag for fine control; double-click the slider to reset to {}. Click the number to type. Safe input: {} to {}.", self.text, self.default.to_f64(), self.safe.start(), self.safe.end()));
            let edit_id = bar.id.with("typed-value");
            let stored = ui.data(|d| d.get_temp::<String>(edit_id));
            if let Some(mut draft) = stored {
                let edit = ui.add_sized([value_width, ui.spacing().interact_size.y], egui::TextEdit::singleline(&mut draft).id(edit_id).desired_width(0.0));
                let cancel = ui.input(|i| i.key_pressed(egui::Key::Escape));
                let commit = edit.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter));
                if cancel || commit {
                    if !cancel {
                        match parse(&draft, &self.safe, T::INTEGRAL) {
                            Ok(parsed) => {
                                value = parsed;
                                if parsed != before { issues::note(&format!("{context} · {}", self.text), &format!("Typed value changed from {before} to {parsed}.")); }
                                ui.data_mut(|d| d.remove::<String>(bar.id.with("error")));
                            },
                            Err(reason) => {
                                issues::record(&format!("{context} · {}", self.text), &format!("Rejected {draft:?}; retained {before}. {reason}"));
                                ui.data_mut(|d| d.insert_temp(bar.id.with("error"), reason));
                            },
                        }
                    }
                    ui.data_mut(|d| d.remove::<String>(edit_id));
                    edit.surrender_focus();
                } else { ui.data_mut(|d| d.insert_temp(edit_id, draft)); }
            } else {
                let formatted = if T::INTEGRAL { format!("{value:.0}") } else { egui::emath::format_with_decimals_in_range(value, 0..=3) };
                if ui.add_sized([value_width, ui.spacing().interact_size.y], egui::Button::new(formatted).truncate())
                    .help_text("Click to enter an exact value, including scientific notation. Enter or click away to apply; Escape cancels. Invalid values go to Settings → Issue log.")
                    .clicked() {
                    ui.data_mut(|d| d.insert_temp(edit_id, before.to_string()));
                    ui.memory_mut(|m| m.request_focus(edit_id));
                }
            }
            bar
        });
        if let Some(reason) = ui.data(|d| d.get_temp::<String>(inner.inner.id.with("error"))) {
            ui.colored_label(
                ui.visuals().warn_fg_color,
                format!("{reason} See Issue log."),
            );
        }
        if T::INTEGRAL {
            value = value.round();
        }
        value = value.clamp(*self.safe.start(), *self.safe.end());
        *self.value = T::from_f64(value);
        let mut response = inner.inner.union(inner.response);
        if self.value.to_f64() != before {
            response.mark_changed();
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(ctx: &egui::Context, value: &mut f32, input: egui::RawInput) -> Response {
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
        response.unwrap()
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
    }

    #[test]
    fn double_click_resets_and_shift_drag_is_fine() {
        let ctx = egui::Context::default();
        let mut value = 5.0;
        let response = frame(&ctx, &mut value, Default::default());
        let start = ctx.read_response(response.id).unwrap().rect.center();
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
    fn custom_input_accepts_scientific_values_but_rejects_unsafe_numbers() {
        assert_eq!(parse("2e1", &(0.01..=100.0), false).unwrap(), 20.0);
        for invalid in ["NaN", "inf", "1e999", "-1", "101", "abc"] {
            assert!(parse(invalid, &(0.01..=100.0), false).is_err());
        }
        assert!(parse("2.5", &(1.0..=128.0), true).is_err());
    }
}
