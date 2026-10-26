//! Purpose-first controls backed by the same saved detector and insert chain.
use super::*;
use crate::signals::guided::Kind;

pub(super) fn type_picker(ui: &mut egui::Ui, d: &mut Definition) {
    let current = d.signal_kind();
    crate::dropdown::ComboBox::from_id_salt(("signal-type", d.id))
        .width(ui.available_width())
        .selected_text(current.label())
        .show_ui(ui, |ui| {
            for kind in Kind::ALL {
                let enabled = if kind == Kind::Audio {
                    current == Kind::Audio
                } else {
                    d.detection_index().is_some() || d.inserts.as_ref().is_some_and(|c| c.stages.len() < crate::signals::inserts::MAX_INSERTS)
                };
                let response = ui.add_enabled(enabled,
                    egui::Button::selectable(current == kind, kind.label()));
                if response.clicked() { d.set_signal_kind(kind); }
                response.help_text(if kind == Kind::Audio && !enabled {
                    "To return to audio only, remove the detector and following controls in Advanced. Existing processing is never discarded by this menu."
                } else { kind.description() });
            }
        });
}

pub(super) fn response_controls(ui: &mut egui::Ui, d: &mut Definition, snapshot: &Snapshot) {
    if d.input != Input::System {
        return;
    }
    audio_channel::inspector_group(ui, "Detect", |ui| {
        type_picker(ui, d);
        ui.add(
            egui::Label::new(
                egui::RichText::new(d.signal_kind().description())
                    .weak()
                    .size(11.0),
            )
            .wrap(),
        );
    });
    if let Some(index) = d.detection_index() {
        let stage = &mut d.inserts.as_mut().unwrap().stages[index];
        let monitor = snapshot
            .inserts
            .get(&d.id)
            .and_then(|m| m.iter().find(|m| m.id == stage.id));
        audio_channel::inspector_group(ui, "Response", |ui| {
            ui.checkbox(&mut stage.enabled, "Detection enabled");
            super::inserts::effect_controls(ui, &mut stage.effect, monitor);
        });
    }
}

impl Panel {
    pub(super) fn guided_preview(&self, ui: &mut egui::Ui, d: &Definition) {
        audio_channel::inspector_group(ui, "Live response", |ui| {
            let output = self.snapshot.values.get(&d.id).cloned().unwrap_or_default();
            let color = channel_color(d);
            let (rect, response) = ui.allocate_exact_size(
                egui::vec2(ui.available_width(), 112.0),
                egui::Sense::hover(),
            );
            response.help_text("Two seconds of detected signal (gray) and final modulation output (color). Output includes Advanced processing and Visual influence. Changes affect connected visuals, not speaker audio.");
            ui.painter()
                .rect_filled(rect, 4.0, ui.visuals().extreme_bg_color);
            let plot = rect.shrink(7.0);
            let signed = d.kind() == OutputKind::Bipolar;
            let high = self
                .history
                .get(&d.id)
                .into_iter()
                .flat_map(|h| h.points())
                .map(|p| p[1].abs())
                .fold(1.0_f32, f32::max);
            let low = if signed { -high } else { 0.0 };
            for level in [0.0, 0.5, 1.0] {
                ui.painter().hline(
                    plot.x_range(),
                    egui::lerp(plot.top()..=plot.bottom(), level),
                    egui::Stroke::new(0.5, ui.visuals().weak_text_color().gamma_multiply(0.2)),
                );
            }
            for (history, tint, width) in [
                (
                    self.raw_history.get(&d.id),
                    ui.visuals().weak_text_color(),
                    1.0,
                ),
                (self.history.get(&d.id), color, 1.8),
            ] {
                let points: Vec<_> = history
                    .into_iter()
                    .flat_map(|h| h.points())
                    .map(|[x, y]| {
                        egui::pos2(
                            plot.left() + x * plot.width(),
                            plot.bottom()
                                - ((y - low) / (high - low)).clamp(0.0, 1.0) * plot.height(),
                        )
                    })
                    .collect();
                if points.len() > 1 {
                    ui.painter_at(plot)
                        .add(egui::Shape::line(points, egui::Stroke::new(width, tint)));
                }
            }
            ui.horizontal_wrapped(|ui| {
                ui.weak("Detected");
                ui.colored_label(color, format!("Output {:.0}%", output.value * 100.0));
                ui.weak(output.status.label());
            });
            if let Some(stage) = d
                .detection_index()
                .and_then(|i| d.inserts.as_ref().map(|c| &c.stages[i]))
                && let Some(m) = self
                    .snapshot
                    .inserts
                    .get(&d.id)
                    .and_then(|m| m.iter().find(|m| m.id == stage.id))
                && m.bpm > 0.0
            {
                ui.label(format!(
                    "{:.1} BPM · {:.0}% confidence",
                    m.bpm,
                    m.confidence * 100.0
                ));
            }
        });
        audio_channel::inspector_group(ui, "Mapped parameters", |ui| {
            if let Some(users) = self.consumers.get(&d.id).filter(|users| !users.is_empty()) {
                for user in users {
                    ui.label(user);
                }
            } else {
                ui.add(egui::Label::new(egui::RichText::new("No parameters mapped. Choose this signal as a macro's modulation source to drive a visual parameter.").weak()).wrap());
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signals::inserts::Effect;
    fn render(
        panel: &mut Panel,
        ctx: &egui::Context,
        id: u64,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(500.0, 1800.0),
                )),
                events,
                ..Default::default()
            },
            |ui| panel.chain_contents(ui, id),
        );
        out.textures_delta.clear();
        out
    }
    fn position(out: &egui::FullOutput, label: &str) -> egui::Pos2 {
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
    fn click(panel: &mut Panel, ctx: &egui::Context, id: u64, pos: egui::Pos2) -> egui::FullOutput {
        render(panel, ctx, id, vec![egui::Event::PointerMoved(pos)]);
        for pressed in [true, false] {
            render(
                panel,
                ctx,
                id,
                vec![egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
        }
        render(panel, ctx, id, vec![])
    }
    #[test]
    fn guided_inspector_keeps_custom_processing_hidden_and_lossless_until_opened() {
        let ctx = egui::Context::default();
        for theme in [egui::Theme::Dark, egui::Theme::Light] {
            ctx.style_mut_of(theme, |s| s.animation_time = 0.0);
        }
        let mut panel = Panel::default();
        panel.act(StripAction::AddGuided(Kind::Hits, 0));
        let id = panel.selected.unwrap();
        let d = &mut panel.bank.channels[0];
        d.name = "My kick".into();
        d.inserts
            .as_mut()
            .unwrap()
            .add(Input::System, 1, Effect::Invert);
        panel.bank.sync_audio_channels();
        let before = panel.bank.clone();
        render(&mut panel, &ctx, id, vec![]);
        let out = render(&mut panel, &ctx, id, vec![]);
        for label in [
            "Detect",
            "Response",
            "Sensitivity",
            "Attack",
            "Release",
            "Live response",
            "Mapped parameters",
            "Advanced · Insert chain",
        ] {
            position(&out, label);
        }
        assert!(!out.shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.job.text == "Invert" || t.galley.job.text == "+ Insert")));
        assert_eq!(
            panel.bank, before,
            "viewing a saved custom signal must not change it"
        );
        let out = click(
            &mut panel,
            &ctx,
            id,
            position(&out, "Advanced · Insert chain"),
        );
        let out = click(&mut panel, &ctx, id, position(&out, "Invert"));
        assert!(panel.selected_insert.is_some());
        position(&out, "Back to signal");
        assert_eq!(panel.bank, before);
        click(&mut panel, &ctx, id, position(&out, "Back to signal"));
        assert!(panel.selected_insert.is_none());
    }
    #[test]
    fn guided_type_picker_changes_detector_without_discarding_mappings_or_extra_controls() {
        let ctx = egui::Context::default();
        let mut panel = Panel::default();
        panel.act(StripAction::AddGuided(Kind::Loudness, 0));
        let id = panel.selected.unwrap();
        panel.bank.channels[0].name = "Brightness".into();
        panel.bank.channels[0]
            .inserts
            .as_mut()
            .unwrap()
            .add(Input::System, 1, Effect::Invert);
        let effect_id = panel.bank.channels[0].inserts.as_ref().unwrap().stages[0].id;
        render(&mut panel, &ctx, id, vec![]);
        let out = render(&mut panel, &ctx, id, vec![]);
        let out = click(&mut panel, &ctx, id, position(&out, "Loudness"));
        click(&mut panel, &ctx, id, position(&out, "Bass Hits"));
        let d = &panel.bank.channels[0];
        assert_eq!(d.signal_kind(), Kind::Hits);
        assert_eq!(d.name, "Brightness");
        assert_eq!(d.id, id);
        assert_eq!(d.inserts.as_ref().unwrap().stages[0].id, effect_id);
        assert_eq!(d.inserts.as_ref().unwrap().stages[1].effect, Effect::Invert);
        assert!(panel.bank.valid());
    }
}
