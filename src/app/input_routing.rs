use super::*;
use crate::signals::audio::{self, Connection};

impl SymphosApp {
    pub(super) fn signals_dock(&mut self, ui: &mut egui::Ui, rect: Rect, frame: &AnalysisFrame) {
        let mut dock = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("signals-routing-dock")
                .max_rect(rect),
        );
        dock.set_clip_rect(rect);
        dock.horizontal(|ui| {
            for (i, title) in ["Modulation", "Audio inputs", "Sketch connections"]
                .iter()
                .enumerate()
            {
                if ui
                    .selectable_label(self.signals_page == i, *title)
                    .clicked()
                {
                    self.signals_page = i;
                    self.inspecting_signal = i == 0;
                }
            }
        });
        let body = dock.available_rect_before_wrap();
        if self.signals_page == 0 {
            let mut panel = self.signal_ui.lock().unwrap_or_else(|e| e.into_inner());
            panel.show(&mut dock, body, frame, false, &self.theme);
            if std::mem::take(&mut panel.inspect_requested) {
                self.inspecting_signal = true;
                self.inspecting_macro = None;
                self.sidebar_open = true;
            }
            return;
        }
        self.inspecting_signal = false;
        let mut page = dock.new_child(
            egui::UiBuilder::new()
                .id_salt(self.signals_page)
                .max_rect(body),
        );
        let mut bank = self
            .signal_ui
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .bank
            .clone();
        if self.signals_page == 1 {
            let original = bank.clone();
            page.horizontal(|ui| {
                ui.label("Waveform · spectrum · measurements").help_text("Each input processes the selected master capture once, then shares its stereo audio, spectrum and measurements with connected sketches and modulation detectors. No sound is generated.");
                if ui.add_enabled(bank.audio_inputs.len() < audio::MAX_INPUTS, egui::Button::new("+ Audio input")).clicked() {
                    self.selected_audio_input = bank.add_audio_input();
                }
            });
            page.separator();
            let mut delete = None;
            page.columns(2, |columns| {
                egui::ScrollArea::vertical().id_salt("audio-input-list").show(&mut columns[0], |ui| {
                    ui.selectable_value(&mut self.selected_audio_input, None, "Main audio");
                    for bus in &bank.audio_inputs {
                        let count = self.open_panes().iter().filter(|id| self.panes[**id].audio_input == Connection::Input(bus.id)).count();
                        ui.selectable_value(&mut self.selected_audio_input, Some(bus.id), format!("{} · {} sketches{}", bus.name, count, if bus.enabled { "" } else { " · Muted" }));
                    }
                    if bank.audio_inputs.is_empty() { ui.weak("Create an input to share filtering and gain across sketches. Existing scenes use Main audio."); }
                });
                egui::ScrollArea::vertical().id_salt("audio-input-editor").show(&mut columns[1], |ui| {
                    if let Some(bus) = self.selected_audio_input.and_then(|id| bank.audio_inputs.iter_mut().find(|b| b.id == id)) {
                        ui.horizontal(|ui| {
                            ui.label("Name");
                            ui.add(egui::TextEdit::singleline(&mut bus.name).char_limit(80).desired_width(180.0));
                        });
                        if bus.name.trim().is_empty() { bus.name = "Audio input".into(); }
                        ui.checkbox(&mut bus.enabled, "Input enabled").help_text("Muting this input silences every connected sketch and detector. LFO and MIDI mappings keep running.");
                        egui::ComboBox::from_id_salt("input-channels").selected_text(bus.channels.label()).show_ui(ui, |ui| {
                            for channels in audio::Channels::ALL { ui.selectable_value(&mut bus.channels, channels, channels.label()); }
                        });
                        ui.add(egui::Slider::new(&mut bus.gain_db, -48.0..=36.0).text("Input gain dB"));
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut bus.high_pass, "High-pass");
                            ui.add_enabled(bus.high_pass, egui::DragValue::new(&mut bus.low_hz).range(20.0..=20000.0).speed(1.0).suffix(" Hz"));
                        });
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut bus.low_pass, "Low-pass");
                            ui.add_enabled(bus.low_pass, egui::DragValue::new(&mut bus.high_hz).range(20.0..=20000.0).speed(1.0).suffix(" Hz"));
                        });
                        if bus.high_pass && bus.low_pass { bus.high_hz = bus.high_hz.max(bus.low_hz + 1.0).min(20000.0); bus.low_hz = bus.low_hz.min(bus.high_hz - 1.0); }
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut bus.gate, "Noise gate");
                            ui.add_enabled(bus.gate, egui::DragValue::new(&mut bus.gate_db).range(-100.0..=0.0).suffix(" dBFS"));
                        });
                        let output = frame.signals.as_ref().and_then(|s| s.audio.get(&bus.id)).map(|f| f.frame.as_ref());
                        if let Some(output) = output { preview(ui, output); } else { ui.weak("Waiting for input processing…"); }
                        if ui.button("Delete input").help_text("Disconnect consumers. They stay unassigned; no replacement input is created.").clicked() { delete = Some(bus.id); }
                    } else {
                        ui.strong("Main audio");
                        ui.label("The capture selected in Settings. New sketches use this input until you choose another connection.");
                        preview(ui, frame);
                        ui.weak("Create a named input to apply independent processing without changing the master capture.");
                    }
                });
            });
            if let Some(id) = delete {
                bank.audio_inputs.retain(|b| b.id != id);
                for definition in &mut bank.channels {
                    if definition.audio_input == Connection::Input(id) {
                        definition.audio_input = Connection::Off;
                    }
                }
                for pane in &mut self.panes {
                    if pane.audio_input == Connection::Input(id) {
                        pane.audio_input = Connection::Off;
                    }
                }
                self.selected_audio_input = None;
            }
            if bank != original {
                self.signal_ui
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .bank = bank;
            }
        } else {
            let open = self.open_panes();
            if !self
                .selected_input_sketch
                .is_some_and(|id| open.contains(&id))
            {
                self.selected_input_sketch = open.first().copied();
            }
            page.columns(2, |columns| {
                egui::ScrollArea::vertical().id_salt("input-sketch-list").show(&mut columns[0], |ui| {
                    ui.strong("Sketch inputs");
                    for id in open {
                        let pane = &self.panes[id];
                        let source = match pane.audio_input {
                            Connection::Main => "Main audio",
                            Connection::Off => "Unassigned",
                            Connection::Input(id) => bank.audio_inputs.iter().find(|b| b.id == id).map_or("Unavailable", |b| b.name.as_str()),
                        };
                        ui.selectable_value(&mut self.selected_input_sketch, Some(id), format!("{} · {} ← {}", id+1, pane.kind.label(), source));
                    }
                    if self.selected_input_sketch.is_none() { ui.weak("Add a visualizer to connect an input."); }
                });
                egui::ScrollArea::vertical().id_salt(("sketch-input-response", self.selected_input_sketch)).show(&mut columns[1], |ui| {
                    if let Some(id) = self.selected_input_sketch {
                        let pane = &mut self.panes[id];
                        ui.strong(format!("{} · {}", id+1, pane.kind.label()));
                        audio::connection_picker(ui, &mut pane.audio_input, &bank.audio_inputs);
                        ui.weak("Response settings below affect this sketch only. Edit Audio inputs to share processing.");
                        let routed = pane.routed_frame(frame);
                        preview(ui, &routed);
                        pane.input_controls(ui, &routed);
                    }
                });
            });
        }
    }
}

fn preview(ui: &mut egui::Ui, frame: &AnalysisFrame) {
    let db = 20.0 * frame.mix_rms.max(1e-7).log10();
    ui.label(format!("{db:.1} dBFS · {} Hz", frame.sample_rate));
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width().max(1.0), 54.0),
        Sense::hover(),
    );
    ui.painter()
        .rect_filled(rect, 4.0, ui.visuals().extreme_bg_color);
    let color = ui.visuals().selection.bg_fill;
    for band in 0..64 {
        let lo = 20.0_f32 * 1000.0_f32.powf(band as f32 / 64.0);
        let hi = 20.0_f32 * 1000.0_f32.powf((band + 1) as f32 / 64.0);
        let level = frame.bins[frame.bins.partition_point(|b| b.frequency_hz < lo)
            ..frame.bins.partition_point(|b| b.frequency_hz < hi)]
            .iter()
            .map(|b| b.magnitude)
            .fold(0.0_f32, f32::max);
        let value = ((20.0 * level.max(1e-7).log10() + 90.0) / 90.0).clamp(0.0, 1.0);
        let x = rect.left() + band as f32 / 64.0 * rect.width();
        ui.painter().rect_filled(
            Rect::from_min_max(
                Pos2::new(x, rect.bottom() - value * rect.height()),
                Pos2::new(x + (rect.width() / 64.0 - 1.0).max(0.5), rect.bottom()),
            ),
            0.0,
            color,
        );
    }
}
