use super::*;
use crate::signals::audio::Connection;
use crate::signals::ui::audio_channel;

impl SymphosApp {
    pub(super) fn signals_dock(&mut self, ui: &mut egui::Ui, rect: Rect, frame: &AnalysisFrame) {
        let mut panel = self.signal_ui.lock().unwrap_or_else(|e| e.into_inner());
        panel.show(ui, rect, frame, false, &self.theme);
        if std::mem::take(&mut panel.master_settings_requested) {
            self.settings_window.open = true;
            self.settings_window.category = settings::Category::Audio;
        }
        if std::mem::take(&mut panel.inspect_requested) {
            self.inspecting_signal = true;
            self.inspecting_macro = None;
            self.sidebar_open = true;
        }
    }

    /// Jump to the existing source, without creating or substituting channels.
    pub(super) fn inspect_audio_signal(&mut self, index: usize) {
        let id = match self.panes[index].audio_input {
            Connection::Main => crate::signals::ui::MASTER_ID,
            Connection::Input(id) if id != crate::signals::ui::MASTER_ID => id,
            _ => return,
        };
        let mut panel = self.signal_ui.lock().unwrap_or_else(|e| e.into_inner());
        if id != crate::signals::ui::MASTER_ID
            && !panel.bank.audio_inputs.iter().any(|bus| bus.id == id)
        {
            return;
        }
        panel.selected = Some(id);
        panel.selected_insert = None;
        panel.show_signals = true;
        self.scene_macros.open = false;
        self.inspecting_signal = true;
        self.inspecting_macro = None;
        self.sidebar_open = true;
    }

    pub(super) fn signal_inspector(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        frame: &AnalysisFrame,
    ) {
        let (selected, bank) = {
            let panel = self.signal_ui.lock().unwrap_or_else(|e| e.into_inner());
            (panel.selected, panel.bank.clone())
        };
        let bus = selected
            .and_then(|id| bank.audio_inputs.iter().find(|b| b.id == id))
            .cloned();
        let Some(mut bus) = bus else {
            let mut panel = self.signal_ui.lock().unwrap_or_else(|e| e.into_inner());
            panel.show_inspector(ui, rect, frame, &self.theme);
            if std::mem::take(&mut panel.master_settings_requested) {
                self.settings_window.open = true;
                self.settings_window.category = settings::Category::Audio;
            }
            if std::mem::take(&mut panel.close_inspector) {
                self.sidebar_open = false;
            }
            return;
        };
        let insert_title = self
            .signal_ui
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert_title(bus.id);
        let chained = self
            .signal_ui
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .has_insert_channel(bus.id);
        let original = bus.clone();
        let mut editor = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(("audio-signal-inspector", selected))
                .max_rect(rect),
        );
        editor.set_clip_rect(rect);
        workspace_ui::inspector_style(&mut editor, &self.theme);
        if workspace_ui::sidebar_header(
            &mut editor,
            Icon::AudioMappings,
            insert_title.as_deref().unwrap_or(&bus.name),
            if insert_title.is_some() {
                "Audio channel · Insert"
            } else {
                "Audio input"
            },
            &format!("System audio · {} kHz", frame.sample_rate / 1000),
            "Close audio inspector",
        )
        .clicked()
        {
            self.sidebar_open = false;
        }
        workspace_ui::inspector_scroll(
            &mut editor,
            ("audio-signal-properties", selected, insert_title.is_some()),
            |ui| {
                if chained {
                    self.signal_ui
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .chain_contents(ui, bus.id);
                } else {
                    audio_channel::controls(ui, &mut bus);
                }
                if insert_title.is_some() {
                    return;
                }
                audio_channel::inspector_group(ui, "Live spectrum", |ui| {
                    let output = frame
                        .signals
                        .as_ref()
                        .and_then(|s| s.audio.get(&bus.id))
                        .map(|f| f.frame.as_ref());
                    let (plot, _) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), 100.0),
                        Sense::hover(),
                    );
                    let [r, g, b] = bus.color_rgb();
                    audio_channel::spectrum(ui, plot, output, Color32::from_rgb(r, g, b));
                });
                audio_channel::inspector_group(ui, "Connected sketches", |ui| {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new("Choose where this audio is sent.")
                                .weak()
                                .size(11.0),
                        )
                        .wrap(),
                    );
                    for id in self.open_panes() {
                        ui.push_id(id, |ui| {
                                let pane = &mut self.panes[id];
                                let mut connected = pane.audio_input == Connection::Input(bus.id);
                                if ui.checkbox(&mut connected, format!("{} · {}", id + 1, pane.kind.label()))
                                    .help_text("Feed this channel's waveform, spectrum and measurements to the sketch. Uncheck to disconnect; no replacement source is created.").changed() {
                                    pane.audio_input = if connected { Connection::Input(bus.id) } else { Connection::Off };
                                }
                                if connected {
                                    egui::CollapsingHeader::new("Sketch response").show(ui, |ui| {
                                        let routed = pane.routed_frame(frame);
                                        pane.input_controls(ui, &routed);
                                    });
                                }
                            });
                    }
                    if self.open_panes().is_empty() {
                        ui.weak("Add a visualizer to connect this audio signal.");
                    }
                });
            },
        );
        if !chained
            && bus != original
            && let Some(destination) = self
                .signal_ui
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .bank
                .audio_inputs
                .iter_mut()
                .find(|b| b.id == bus.id)
        {
            *destination = bus;
        }
    }
}
