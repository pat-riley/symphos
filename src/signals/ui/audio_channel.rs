use super::*;

pub(crate) fn spectrum(
    ui: &mut egui::Ui,
    rect: Rect,
    frame: Option<&AnalysisFrame>,
    color: egui::Color32,
) {
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, ui.visuals().extreme_bg_color);
    let plot = rect.shrink2(egui::vec2(2.0, 15.0));
    let high = frame.map_or(20000.0, |f| {
        (f.sample_rate as f32 * 0.5).clamp(21.0, 20000.0)
    });
    spectrum_bars(ui, plot, frame, color);
    painter.text(
        rect.left_top() + egui::vec2(5.0, 4.0),
        egui::Align2::LEFT_TOP,
        "AUDIO · SPECTRUM",
        egui::FontId::monospace(9.0),
        ui.visuals().weak_text_color(),
    );
    painter.text(
        rect.left_bottom() + egui::vec2(5.0, -3.0),
        egui::Align2::LEFT_BOTTOM,
        "20 Hz",
        egui::FontId::monospace(9.0),
        ui.visuals().weak_text_color(),
    );
    painter.text(
        rect.right_bottom() + egui::vec2(-5.0, -3.0),
        egui::Align2::RIGHT_BOTTOM,
        format!("{:.0} kHz", high / 1000.0),
        egui::FontId::monospace(9.0),
        ui.visuals().weak_text_color(),
    );
    ui.interact(rect,ui.id().with("audio-spectrum-help"),egui::Sense::hover()).help_text("Audio spectrum · low frequencies on the left, high frequencies on the right. Bar height shows the processed audio level. This channel carries waveform, spectrum and measurements into connected sketches; control signals show a time-history curve instead.");
}

pub(super) fn spectrum_bars(
    ui: &egui::Ui,
    rect: Rect,
    frame: Option<&AnalysisFrame>,
    color: egui::Color32,
) {
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }
    let painter = ui.painter_at(rect);
    for i in 1..4 {
        painter.hline(
            rect.x_range(),
            rect.top() + rect.height() * i as f32 / 4.0,
            egui::Stroke::new(0.5, ui.visuals().weak_text_color().gamma_multiply(0.15)),
        );
    }
    let high = frame.map_or(20000.0, |f| {
        (f.sample_rate as f32 * 0.5).clamp(21.0, 20000.0)
    });
    if let Some(frame) = frame {
        for i in 0..48 {
            let low = 20.0 * (high / 20.0).powf(i as f32 / 48.0);
            let top = 20.0 * (high / 20.0).powf((i + 1) as f32 / 48.0);
            let a = frame.bins.partition_point(|b| b.frequency_hz < low);
            let b = frame.bins.partition_point(|b| b.frequency_hz < top);
            let magnitude = frame.bins[a..b]
                .iter()
                .map(|b| b.magnitude)
                .fold(0.0_f32, f32::max);
            let value = ((20.0 * magnitude.max(1e-7).log10() + 90.0) / 90.0).clamp(0.0, 1.0);
            let x = rect.left() + i as f32 * rect.width() / 48.0;
            painter.rect_filled(
                Rect::from_min_max(
                    egui::pos2(x, rect.bottom() - value * rect.height()),
                    egui::pos2(x + (rect.width() / 48.0 - 0.6).max(0.3), rect.bottom()),
                ),
                0.0,
                color,
            );
        }
    }
}

pub(super) fn strip(
    ui: &mut egui::Ui,
    rect: Rect,
    slot: usize,
    bus: &mut audio::InputBus,
    frame: Option<&AnalysisFrame>,
    selected: bool,
) -> Option<StripAction> {
    let [r, g, b] = bus.color_rgb();
    let color = egui::Color32::from_rgb(r, g, b);
    workspace_ui::strip_surface(ui, rect, color, selected);
    let dark = if 0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * (b as f32) < 110.0 {
        egui::Color32::WHITE
    } else {
        egui::Color32::from_rgb(12, 18, 26)
    };
    let title = Rect::from_min_max(rect.min, egui::pos2(rect.right() - 24.0, rect.top() + 28.0));
    workspace_ui::strip_title(
        ui,
        title.shrink2(egui::vec2(8.0, 0.0)),
        format!("{} {}", slot + 1, bus.name),
        dark,
    );
    let response = workspace_ui::drag_cursor(ui.interact(
        title,
        ui.id().with("audio-title"),
        egui::Sense::click_and_drag(),
    ))
    .help_text("Select audio channel · drag to reorder · right-click for channel actions");
    let mut action = if response.double_clicked() {
        Some(StripAction::Rename(bus.id))
    } else {
        response.clicked().then_some(StripAction::Select(bus.id))
    };
    if response.drag_started() {
        response.dnd_set_drag_payload(ChannelDrag(bus.id));
    }
    let close = Rect::from_min_size(
        egui::pos2(rect.right() - 23.0, rect.top() + 2.0),
        egui::vec2(21.0, 24.0),
    );
    icons::paint(ui, close.shrink(5.0), Icon::Close, dark);
    if ui
        .interact(close, ui.id().with("audio-delete"), egui::Sense::click())
        .clicked()
    {
        action = Some(StripAction::Delete(bus.id));
    }
    let mut controls = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(Rect::from_min_max(
                rect.min + egui::vec2(7.0, 35.0),
                egui::pos2(rect.right() - 7.0, rect.top() + 105.0),
            ))
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    controls.style_mut().override_font_id = Some(egui::FontId::proportional(11.0));
    controls.spacing_mut().item_spacing.y = 3.0;
    controls.spacing_mut().interact_size.y = 21.0;
    controls.small("Source");
    crate::dropdown::ComboBox::from_id_salt("audio-source")
        .width(142.0)
        .selected_text("System audio")
        .show_ui(&mut controls, |ui| {
            ui.label("System audio · selected in Settings");
        });
    controls.small("Audio signal");
    let bottom = rect.bottom() - 144.0;
    let graph = Rect::from_min_max(
        egui::pos2(rect.left() + 8.0, controls.min_rect().bottom() + 8.0),
        egui::pos2(rect.right() - 8.0, bottom - 8.0),
    );
    spectrum(ui, graph, frame, color);
    footer(ui, rect, bus, frame, color, dark, false);
    action
}

pub(super) fn footer(
    ui: &mut egui::Ui,
    rect: Rect,
    bus: &mut audio::InputBus,
    frame: Option<&AnalysisFrame>,
    color: egui::Color32,
    dark: egui::Color32,
    combined: bool,
) {
    let bottom = rect.bottom()
        - if combined {
            outputs::FOOTER_HEIGHT
        } else {
            144.0
        };
    ui.painter().hline(
        rect.x_range(),
        bottom,
        ui.visuals().widgets.noninteractive.bg_stroke,
    );
    ui.painter().text(
        egui::pos2(rect.left() + 8.0, bottom + 15.0),
        egui::Align2::LEFT_CENTER,
        "Input gain",
        egui::FontId::proportional(11.0),
        ui.visuals().text_color(),
    );
    let knob = Rect::from_center_size(
        egui::pos2(
            rect.left() + 40.0,
            bottom + if combined { 36.0 } else { 58.0 },
        ),
        egui::Vec2::splat(if combined { 36.0 } else { 54.0 }),
    );
    let response=ui.interact(knob,ui.id().with("audio-gain"),egui::Sense::click_and_drag()).on_hover_and_drag_cursor(egui::CursorIcon::ResizeVertical).help_text("Audio input gain · drag vertically · Shift: fine · double-click: 0 dB. Applies to this channel's waveform, spectrum and measurements.");
    if response.dragged() {
        bus.gain_db = (bus.gain_db
            - ui.input(|i| i.pointer.delta().y)
                * if ui.input(|i| i.modifiers.shift) {
                    0.03
                } else {
                    0.2
                })
        .clamp(-48.0, 36.0);
    }
    if response.double_clicked() {
        bus.gain_db = 0.0;
    }
    let t = (bus.gain_db + 48.0) / 84.0;
    let arc = |end: f32| {
        (0..=40)
            .map(|i| {
                knob.center()
                    + egui::Vec2::angled(2.35 + 4.72 * end * i as f32 / 40.0)
                        * if combined { 16.0 } else { 23.0 }
            })
            .collect()
    };
    ui.painter().add(egui::Shape::line(
        arc(1.0),
        egui::Stroke::new(2.5, ui.visuals().weak_text_color().gamma_multiply(0.3)),
    ));
    ui.painter()
        .add(egui::Shape::line(arc(t), egui::Stroke::new(2.5, color)));
    ui.painter().circle_filled(
        knob.center(),
        if combined { 12.0 } else { 18.0 },
        ui.visuals().extreme_bg_color,
    );
    let direction = egui::Vec2::angled(2.35 + 4.72 * t);
    ui.painter().line_segment(
        [
            knob.center() + direction * if combined { 6.0 } else { 9.0 },
            knob.center() + direction * if combined { 11.0 } else { 16.0 },
        ],
        egui::Stroke::new(1.5, ui.visuals().text_color()),
    );
    let value_rect = Rect::from_min_size(
        egui::pos2(
            rect.left() + 12.0,
            bottom + if combined { 56.0 } else { 88.0 },
        ),
        egui::vec2(76.0, 22.0),
    );
    let mut value = ui.new_child(egui::UiBuilder::new().max_rect(value_rect));
    value.set_clip_rect(value_rect.intersect(ui.clip_rect()));
    value.spacing_mut().interact_size.y = 22.0;
    value.style_mut().override_font_id = Some(egui::FontId::proportional(11.0));
    value.add_sized(
        value_rect.size(),
        egui::DragValue::new(&mut bus.gain_db)
            .range(-48.0..=36.0)
            .speed(0.1)
            .fixed_decimals(1)
            .suffix(" dB"),
    );
    let level = frame.map_or(0.0, |f| {
        ((20.0 * f.mix_rms.max(1e-7).log10() + 90.0) / 90.0).clamp(0.0, 1.0)
    });
    let meter = Rect::from_min_size(
        egui::pos2(
            rect.left() + 113.0,
            bottom + if combined { 22.0 } else { 35.0 },
        ),
        egui::vec2(9.0, if combined { 54.0 } else { 65.0 }),
    );
    ui.painter()
        .rect_filled(meter, 2.0, ui.visuals().extreme_bg_color);
    ui.painter().rect_filled(
        Rect::from_min_max(
            egui::pos2(meter.left(), meter.bottom() - level * meter.height()),
            meter.max,
        ),
        1.0,
        color,
    );
    ui.painter().text(
        meter.center_bottom() + egui::vec2(0.0, 4.0),
        egui::Align2::CENTER_TOP,
        if combined { "Audio" } else { "Out" },
        egui::FontId::proportional(10.0),
        ui.visuals().weak_text_color(),
    );
    channel_buttons(
        ui,
        value_rect,
        color,
        dark,
        &mut bus.enabled,
        &mut bus.solo,
        "Mute this audio channel and its consumers",
        if combined {
            "Solo this source alongside any other soloed channels"
        } else {
            "Solo among audio channels; LFO/MIDI channels remain independent"
        },
    );
}

pub(super) fn menu(
    ui: &mut egui::Ui,
    bus: &audio::InputBus,
    slot: usize,
    can_create: bool,
) -> Option<StripAction> {
    let mut action = None;
    for (label, a) in [
        ("Rename", StripAction::Rename(bus.id)),
        ("Duplicate", StripAction::Duplicate(bus.id, slot + 1)),
        ("Delete", StripAction::Delete(bus.id)),
    ] {
        if ui
            .add_enabled(label != "Duplicate" || can_create, egui::Button::new(label))
            .clicked()
        {
            action = Some(a);
            ui.close();
        }
    }
    crate::dropdown::menu_separator(ui);
    ui.small("Channel color");
    let [r, g, b] = bus.color_rgb();
    if let Some(color) = color_palette(ui, egui::Color32::from_rgb(r, g, b)) {
        action = Some(StripAction::Color(bus.id, color));
    }
    action
}

/// Audio inspector groups use the existing disclosure style with room around fields.
pub(crate) fn inspector_group(ui: &mut egui::Ui, title: &str, body: impl FnOnce(&mut egui::Ui)) {
    settings_group(ui, title, body);
}

pub(crate) fn controls(ui: &mut egui::Ui, bus: &mut audio::InputBus) {
    inspector_group(ui, "Channel", |ui| {
        property_row(ui, "Name", |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut bus.name)
                    .char_limit(80)
                    .desired_width(ui.available_width()),
            );
        });
        if bus.name.trim().is_empty() {
            bus.name = "Main audio".into();
        }
        property_row(ui, "Source", |ui| {
            ui.label("System audio").help_text("Uses the main capture selected in Settings. This channel carries audio and its complete spectrum, rather than a single modulation curve.");
        });
        property_row(ui, "Channels", |ui| {
            crate::dropdown::ComboBox::from_id_salt("audio-channels")
                .width(ui.available_width())
                .selected_text(bus.channels.label())
                .show_ui(ui, |ui| {
                    for c in audio::Channels::ALL {
                        ui.selectable_value(&mut bus.channels, c, c.label());
                    }
                });
        });
        property_row(ui, "Enabled", |ui| {
            ui.checkbox(&mut bus.enabled, "")
                .help_text("Disable to silence this audio channel and its connected sketches.");
        });
    });
    inspector_group(ui, "Audio processing", |ui| {
        property_row(ui, "Input gain", |ui| {
            ui.add_sized(
                egui::vec2(ui.available_width(), 22.0),
                egui::DragValue::new(&mut bus.gain_db)
                    .range(-48.0..=36.0)
                    .speed(0.1)
                    .suffix(" dB"),
            );
        });
        property_row(ui, "High-pass", |ui| {
            ui.checkbox(&mut bus.high_pass, "")
                .help_text("Remove frequencies below the cutoff.");
            ui.add_enabled_ui(bus.high_pass, |ui| {
                ui.add_sized(
                    egui::vec2(ui.available_width(), 22.0),
                    egui::DragValue::new(&mut bus.low_hz)
                        .range(20.0..=20000.0)
                        .suffix(" Hz"),
                );
            });
        });
        property_row(ui, "Low-pass", |ui| {
            ui.checkbox(&mut bus.low_pass, "")
                .help_text("Remove frequencies above the cutoff.");
            ui.add_enabled_ui(bus.low_pass, |ui| {
                ui.add_sized(
                    egui::vec2(ui.available_width(), 22.0),
                    egui::DragValue::new(&mut bus.high_hz)
                        .range(20.0..=20000.0)
                        .suffix(" Hz"),
                );
            });
        });
        if bus.high_pass && bus.low_pass {
            bus.high_hz = bus.high_hz.max(bus.low_hz + 1.0).min(20000.0);
            bus.low_hz = bus.low_hz.min(bus.high_hz - 1.0);
        }
        property_row(ui, "Noise gate", |ui| {
            ui.checkbox(&mut bus.gate, "")
                .help_text("Silence audio below this threshold.");
            ui.add_enabled_ui(bus.gate, |ui| {
                ui.add_sized(
                    egui::vec2(ui.available_width(), 22.0),
                    egui::DragValue::new(&mut bus.gate_db)
                        .range(-100.0..=0.0)
                        .suffix(" dBFS"),
                );
            });
        });
    });
}
