use super::*;
use crate::modules::{property_row, settings_group};
use egui::{self, RichText};

impl Runtime {
    pub fn settings(&mut self, ui: &mut egui::Ui) {
        let snapshot = self.snapshot.load_full();
        ui.strong("MIDI inputs");
        ui.separator();
        ui.label("Enable inputs here. Right-click an inspector parameter or assigned macro and choose Learn MIDI controller for direct control. Create a MIDI channel in Signals when you want to shape and smooth MIDI through the signal flow.");
        if let Some(error) = &snapshot.error {
            ui.colored_label(ui.visuals().warn_fg_color, error);
        }
        let mut changed = false;
        for port in &snapshot.ports {
            ui.horizontal(|ui| {
                let mut enabled = self.preferences_value.enabled.contains(&port.id);
                if ui.checkbox(&mut enabled,&port.name).help_text("Allow this MIDI input to feed Signals and MIDI Learn. This machine preference is independent of projects; it does not create channels automatically.").changed() {
                    if enabled { self.preferences_value.enabled.insert(port.id); }
                    else { self.preferences_value.enabled.remove(&port.id); }
                    changed = true;
                }
                if enabled { ui.weak("Input on"); }
            });
        }
        for id in self.preferences_value.enabled.clone() {
            if !snapshot.ports.iter().any(|p| p.id == id) {
                ui.horizontal(|ui| {
                    ui.weak(format!("Input {id} · disconnected"));
                    if ui.small_button("Forget").clicked() {
                        self.preferences_value.enabled.remove(&id);
                        changed = true;
                    }
                });
            }
        }
        if snapshot.ports.iter().all(|p| p.id == VIRTUAL_PORT) {
            ui.small("No hardware inputs found. Connect a USB MIDI device or enable a virtual MIDI bus; the list updates automatically.");
        }
        if changed {
            self.save_preferences();
        }
        if let Some(error) = &self.save_error {
            ui.colored_label(
                ui.visuals().warn_fg_color,
                format!("Could not save MIDI preferences: {error}"),
            );
        }
        ui.add_space(12.0);
        ui.strong("Incoming activity");
        ui.separator();
        if let Some(event) = snapshot.events.back() {
            ui.label(snapshot.port_name(event.port));
            ui.monospace(event.label());
        } else {
            ui.weak("Move a control or play a note on an enabled input.");
        }
        if ui.button("Reset held MIDI values").help_text("Clear held notes, controller values and sustain if a device stops without sending note-off. Inputs stay connected; the next message resumes control.").clicked() { self.panic.store(true,Ordering::Relaxed); }
        if snapshot.dropped > 0 {
            ui.colored_label(ui.visuals().warn_fg_color,format!("{} packets dropped during overload. Held values were cleared to avoid stuck notes.", snapshot.dropped));
        }
        ui.add_space(12.0);
        ui.strong("From Ableton or another DAW");
        ui.separator();
        ui.label("Send a MIDI track to “Symphos Input” and choose its MIDI channel (1–16). Enable that output’s Track setting in your DAW if needed. In Signals, select MIDI → Symphos Input and match the channel, then learn a note or controller.");
        ui.small("CC, notes, pitch bend and aftertouch are supported. This input receives MIDI messages; it does not play instruments, import MIDI files, or synchronize transport/tempo yet.");
    }
}

pub fn binding_editor(
    ui: &mut egui::Ui,
    binding: &mut Binding,
    snapshot: &Snapshot,
    learn: &mut Option<u64>,
) {
    // Learn ignores history already present when armed, and ignores note-offs,
    // clocks and system traffic. Learn the first subsequent supported event.
    if let Some(after) = *learn
        && let Some(event) = snapshot.events.iter().find(|e| e.serial > after)
    {
        binding.port = Some(event.port);
        binding.channel = Some(event.channel);
        binding.message =
            if binding.message == Message::NoteGate && event.message == Message::NoteVelocity {
                Message::NoteGate
            } else {
                event.message
            };
        binding.number = event.number;
        *learn = None;
    }
    settings_group(ui, "MIDI mapping", |ui| {
        property_row(ui, "MIDI input", |ui| {
            crate::dropdown::ComboBox::from_id_salt("midi-port").width(ui.available_width()).truncate()
                .selected_text(binding.port.map_or("All enabled inputs".into(),|id|snapshot.port_name(id)))
                .show_ui(ui,|ui| {
                    ui.selectable_value(&mut binding.port,None,"All enabled inputs");
                    for port in &snapshot.ports {
                        ui.add_enabled_ui(snapshot.enabled.contains(&port.id),|ui| {
                            ui.selectable_value(&mut binding.port,Some(port.id),&port.name);
                        });
                    }
                }).response.help_text("Filter this signal to one enabled MIDI input or listen to all enabled inputs. Configure input availability in Settings → MIDI.");
        });
        property_row(ui, "MIDI channel", |ui| {
            crate::dropdown::ComboBox::from_id_salt("midi-channel")
                .width(ui.available_width())
                .selected_text(
                    binding
                        .channel
                        .map_or("All channels".into(), |c| format!("Channel {c}")),
                )
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut binding.channel, None, "All channels");
                    for c in 1..=16 {
                        ui.selectable_value(&mut binding.channel, Some(c), format!("Channel {c}"));
                    }
                });
        });
        property_row(ui, "Message", |ui| {
            message_picker(ui, &mut binding.message);
        });
        if binding.message.numbered() {
            let notes = binding.message != Message::ControlChange;
            property_row(ui, if notes { "Note" } else { "CC number" }, |ui| {
                if notes {
                    let mut any = binding.number.is_none();
                    if ui.checkbox(&mut any, "Any").changed() {
                        binding.number = if any { None } else { Some(60) };
                    }
                }
                if let Some(number) = &mut binding.number {
                    ui.add(egui::DragValue::new(number).range(0..=127));
                }
            });
        }
        ui.horizontal(|ui| {
            if ui.add(egui::Button::selectable(learn.is_some(),if learn.is_some(){"Cancel Learn"}else{"Learn MIDI"})).help_text("Click Learn, then move a hardware control or play a note. Captures the input port, MIDI channel, message type and controller/note number. No new signal channels are created.").clicked() {
                *learn = if learn.is_some(){None}else{Some(snapshot.serial)};
            }
            if learn.is_some() { ui.label(RichText::new("Listening…").color(ui.visuals().selection.stroke.color)); }
        });
        ui.small(binding.message.description());
        if !snapshot.available(binding) {
            ui.colored_label(
                ui.visuals().warn_fg_color,
                "Input unavailable. Enable or reconnect it in Settings → MIDI.",
            );
        }
        let value = snapshot.value(binding);
        property_row(ui, "MIDI value", |ui| {
            ui.monospace(format!("{value:.3}"));
        });
        if let Some(event) = snapshot.events.back() {
            ui.small(format!("Last received: {}", event.label()));
        }
    });
    binding.sanitize();
}
pub fn message_picker(ui: &mut egui::Ui, message: &mut Message) {
    crate::dropdown::ComboBox::from_id_salt("midi-message")
        .width(ui.available_width())
        .truncate()
        .selected_text(message.label())
        .show_ui(ui, |ui| {
            for kind in Message::ALL {
                ui.selectable_value(message, kind, kind.label())
                    .help_text(kind.description());
            }
        })
        .response
        .help_text(message.description());
}
