//! Floating editor for the same built-in signal stages used by the inspector.
//! This is not an external audio plug-in host; all edits stay in the scene bank.
use super::*;

impl Panel {
    pub fn show_processor(&mut self, ctx: &egui::Context, frame: &AnalysisFrame) {
        let Some(id) = self.processor else {
            return;
        };
        let Some(definition) = self.bank.channels.iter().find(|d| d.id == id).cloned() else {
            self.processor = None;
            return;
        };
        self.observe_history();
        if !processor_section_available(definition.input, self.processor_section) {
            self.processor_section = SignalSection::Output;
        }
        let mut open = true;
        egui::Window::new(format!("Signal processor · {}", definition.name))
            .id(egui::Id::new("signal-processor-window"))
            .open(&mut open)
            .default_width(620.0)
            .default_height(680.0)
            .min_width(360.0)
            .min_height(360.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.weak(
                    "These stages shape the control signal used by every mapping on this channel.",
                );
                self.processing_scope(ui, &definition);
                ui.horizontal_wrapped(|ui| {
                    for section in SignalSection::ALL {
                        if !processor_section_available(definition.input, section) {
                            continue;
                        }
                        ui.selectable_value(
                            &mut self.processor_section,
                            section,
                            section.title(definition.input == Input::Lfo),
                        );
                    }
                });
                ui.separator();
                let bounds = ui.available_rect_before_wrap();
                ui.data_mut(|d| {
                    d.insert_temp(egui::Id::new("inspector-bounds"), bounds);
                    d.insert_temp(egui::Id::new("property-row-index"), 0_usize);
                });
                egui::ScrollArea::vertical()
                    .id_salt(("processor-details", id, self.processor_section))
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        self.configure(ui, frame, id, self.processor_section);
                    });
                ui.data_mut(|d| {
                    d.remove::<Rect>(egui::Id::new("inspector-bounds"));
                    d.remove::<usize>(egui::Id::new("property-row-index"));
                });
            });
        if !open {
            self.processor = None;
        }
    }

    fn processing_scope(&self, ui: &mut egui::Ui, definition: &Definition) {
        let raw: Vec<_> = self
            .raw_history
            .get(&definition.id)
            .into_iter()
            .flat_map(|h| h.points())
            .collect();
        let final_values: Vec<_> = self
            .history
            .get(&definition.id)
            .into_iter()
            .flat_map(|h| h.points())
            .collect();
        let signed = raw.iter().chain(&final_values).any(|p| p[1] < 0.0);
        let high = raw
            .iter()
            .chain(&final_values)
            .map(|p| p[1].abs())
            .fold(1.0_f32, f32::max);
        let low = if signed { -high } else { 0.0 };
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), 160.0),
            egui::Sense::hover(),
        );
        response.help_text("Last two seconds\nThe gray trace is detector/envelope output before normalization. The colored trace is the final signal, after range, curve, influence and smoothing. Both use the same vertical scale so you can see what the processing changes.");
        ui.painter()
            .rect_filled(rect, 6.0, ui.visuals().extreme_bg_color);
        let plot = rect.shrink2(egui::vec2(10.0, 18.0));
        let point = |p: [f32; 2]| {
            egui::pos2(
                plot.left() + p[0] * plot.width(),
                plot.bottom() - (p[1] - low) / (high - low) * plot.height(),
            )
        };
        for level in [low, 0.0, high] {
            ui.painter().hline(
                plot.x_range(),
                point([0.0, level]).y,
                ui.visuals().widgets.noninteractive.bg_stroke,
            );
        }
        let painter = ui.painter_at(plot);
        for (values, color, width) in [
            (&raw, ui.visuals().weak_text_color(), 1.0),
            (&final_values, channel_color(definition), 1.8),
        ] {
            if values.len() > 1 {
                painter.add(egui::Shape::line(
                    values.iter().copied().map(point).collect(),
                    egui::Stroke::new(width, color),
                ));
            }
        }
        ui.painter().text(
            rect.left_top() + egui::vec2(8.0, 3.0),
            egui::Align2::LEFT_TOP,
            format!("{high:.2}"),
            egui::FontId::monospace(10.0),
            ui.visuals().weak_text_color(),
        );
        ui.horizontal(|ui| {
            ui.colored_label(ui.visuals().weak_text_color(), "Before processing");
            ui.colored_label(channel_color(definition), "Final output");
            ui.weak("2 seconds");
        });
    }
}

fn processor_section_available(input: Input, section: SignalSection) -> bool {
    match input {
        Input::Midi => !matches!(section, SignalSection::Detection | SignalSection::Response),
        Input::Lfo => section != SignalSection::Response,
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn switching_source_keeps_only_applicable_processor_stages() {
        assert!(!processor_section_available(
            Input::Midi,
            SignalSection::Detection
        ));
        assert!(!processor_section_available(
            Input::Lfo,
            SignalSection::Response
        ));
        for input in [Input::Midi, Input::Lfo] {
            assert!(processor_section_available(input, SignalSection::Output));
            assert!(processor_section_available(input, SignalSection::Source));
        }
    }

    #[test]
    fn floating_processor_keeps_inspector_styling_scoped_to_its_window() {
        let ctx = egui::Context::default();
        let mut panel = Panel::default();
        let id = panel.bank.add(Method::Lfo).unwrap();
        panel.processor = Some(id);
        let frame = AnalysisFrame::default();
        for section in SignalSection::ALL {
            panel.processor_section = section;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(900.0, 800.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    panel.show_processor(ui.ctx(), &frame);
                    assert!(crate::modules::inspector_bounds(ui).is_none());
                },
            );
            output.textures_delta.clear();
            assert!(panel.bank.valid());
        }
    }
}
