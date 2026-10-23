//! Logic-style insert slots and visual editors for typed, built-in processors.
use super::*;
use crate::signals::inserts::{BeatOutput, Data, Detector, Effect, Monitor};
#[derive(Clone, Copy)]
struct InsertDrag {
    channel: u64,
    insert: u64,
}

fn insert_menu(ui: &mut egui::Ui, d: &Definition, index: usize) -> Option<StripAction> {
    let mut action = None;
    let chain = d.inserts.as_ref().unwrap();
    let data = chain.data_at(d.input, index);
    ui.add_enabled_ui(chain.stages.len() < crate::signals::inserts::MAX_INSERTS, |ui| {
        let button = insert_button(ui, "+ Insert", false, egui::Sense::click()).help_text("Add a processor at this position. Only inserts compatible with the incoming data type are offered. Audio detectors convert audio to control values; the audio feed remains available to sketches.");
        crate::dropdown::styled_popup(ui, egui::Popup::menu(&button))
            .gap(5.0)
            .width(180.0)
            .show(|ui| {
                ui.small(if data == Data::Audio { "Audio effects & detectors" } else { "Control processors" });
                ui.separator();
                for effect in Effect::choices(data) {
                    let mut candidate = chain.clone();
                    let compatible = candidate.add(d.input, index, effect.clone()).is_some();
                    if ui.add_enabled(compatible, egui::Button::new(effect.label())).help_text(effect.description()).clicked() {
                        action = Some(StripAction::AddEffect(d.id, index, effect));
                        ui.close();
                    }
                }
            });
    });
    action
}

const INSERT_HEIGHT: f32 = 22.0;
const INSERT_GAP: f32 = 8.0;

fn insert_button(
    ui: &mut egui::Ui,
    label: impl Into<String>,
    selected: bool,
    sense: egui::Sense,
) -> egui::Response {
    ui.scope(|ui| {
        let text = ui.visuals().text_color();
        let accent = ui.visuals().selection.bg_fill;
        let widgets = &mut ui.visuals_mut().widgets;
        for (widget, tint, border) in [
            (
                &mut widgets.inactive,
                if selected { 0.04 } else { 0.0 },
                0.28,
            ),
            (&mut widgets.hovered, 0.07, 0.45),
            (&mut widgets.open, 0.07, 0.45),
            (&mut widgets.active, 0.11, 0.55),
        ] {
            widget.weak_bg_fill = text.gamma_multiply(tint);
            widget.bg_fill = widget.weak_bg_fill;
            widget.bg_stroke = egui::Stroke::new(
                1.0,
                if selected {
                    accent
                } else {
                    text.gamma_multiply(border)
                },
            );
            widget.fg_stroke.color = text;
            widget.expansion = 0.0;
        }
        ui.add_sized(
            [ui.available_width(), INSERT_HEIGHT],
            egui::Button::new(egui::RichText::new(label.into()).size(11.0))
                .corner_radius(3.0)
                .sense(sense),
        )
    })
    .inner
}

pub(super) struct StripOutput {
    pub action: Option<StripAction>,
    pub over_inserts: bool,
}
fn slot_list(
    ui: &mut egui::Ui,
    d: &Definition,
    selected: Option<(u64, u64)>,
    append: bool,
) -> Option<StripAction> {
    let mut action = None;
    let chain = d.inserts.as_ref().unwrap();
    for (i, insert) in chain.stages.iter().enumerate() {
        let label = if insert.enabled {
            insert.effect.label().to_owned()
        } else {
            format!("{} · off", insert.effect.label())
        };
        let color = channel_color(d);
        let response = workspace_ui::drag_cursor(insert_button(
            ui,
            label,
            selected == Some((d.id, insert.id)),
            egui::Sense::click_and_drag(),
        ))
        .help_text(insert.effect.description());
        if response.clicked() || response.secondary_clicked() {
            action = Some(StripAction::InspectEffect(d.id, insert.id));
        }
        if response.drag_started() {
            response.dnd_set_drag_payload(InsertDrag {
                channel: d.id,
                insert: insert.id,
            });
        }
        if let Some(payload) = response.dnd_hover_payload::<InsertDrag>()
            && payload.channel == d.id
        {
            let boundary = i + usize::from(
                ui.ctx()
                    .pointer_interact_pos()
                    .is_some_and(|p| p.y > response.rect.center().y),
            );
            let mut check = chain.clone();
            let valid = check.move_to(d.input, payload.insert, boundary);
            ui.painter().hline(
                response.rect.x_range(),
                if boundary == i {
                    response.rect.top()
                } else {
                    response.rect.bottom()
                },
                egui::Stroke::new(
                    1.5,
                    if valid {
                        color
                    } else {
                        ui.visuals().error_fg_color
                    },
                ),
            );
            if ui.input(|i| i.pointer.any_released()) && valid {
                action = Some(StripAction::MoveEffect(d.id, payload.insert, boundary));
                egui::DragAndDrop::take_payload::<InsertDrag>(ui.ctx());
            }
        }
        crate::dropdown::styled_popup(ui, egui::Popup::context_menu(&response)).show(|ui| {
            if ui
                .button(if insert.enabled { "Bypass" } else { "Enable" })
                .clicked()
            {
                action = Some(StripAction::BypassEffect(d.id, insert.id));
                ui.close();
            }
            for (label, boundary) in [
                ("Move earlier", i.saturating_sub(1)),
                ("Move later", (i + 2).min(chain.stages.len())),
            ] {
                let mut candidate = chain.clone();
                if ui
                    .add_enabled(
                        candidate.move_to(d.input, insert.id, boundary) && candidate != *chain,
                        egui::Button::new(label),
                    )
                    .clicked()
                {
                    action = Some(StripAction::MoveEffect(d.id, insert.id, boundary));
                    ui.close();
                }
            }
            if ui
                .button(if insert.effect.input() != insert.effect.output() {
                    "Remove detector + following controls"
                } else {
                    "Remove insert"
                })
                .clicked()
            {
                action = Some(StripAction::RemoveEffect(d.id, insert.id));
                ui.close();
            }
        });
    }
    if append && let Some(a) = insert_menu(ui, d, chain.stages.len()) {
        action = Some(a);
    }
    action
}

#[allow(clippy::too_many_arguments)]
pub(super) fn strip(
    ui: &mut egui::Ui,
    rect: Rect,
    slot: usize,
    d: &mut Definition,
    bus: Option<&mut audio::InputBus>,
    snapshot: &Snapshot,
    selected: bool,
    selected_insert: Option<(u64, u64)>,
    history: Option<&history::History>,
    midi: (&crate::midi::Snapshot, Option<&midi_preview::History>),
) -> StripOutput {
    let color = channel_color(d);
    let [r, g, b] = d.color_rgb();
    let ink = if 0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * (b as f32) < 110.0 {
        egui::Color32::WHITE
    } else {
        egui::Color32::from_rgb(12, 18, 26)
    };
    workspace_ui::strip_surface(ui, rect, color, selected);
    let title = Rect::from_min_max(rect.min, egui::pos2(rect.right() - 24.0, rect.top() + 28.0));
    workspace_ui::strip_title(
        ui,
        title.shrink2(egui::vec2(8.0, 0.0)),
        format!("{} {}", slot + 1, d.name),
        ink,
    );
    let response=workspace_ui::drag_cursor(ui.interact(title,ui.id().with("insert-channel-title"),egui::Sense::click_and_drag())).help_text("Select channel · double-click to rename · drag to reorder · right-click for channel actions");
    let mut action = if response.double_clicked() {
        Some(StripAction::Rename(d.id))
    } else {
        response.clicked().then_some(StripAction::Select(d.id))
    };
    if response.drag_started() {
        response.dnd_set_drag_payload(ChannelDrag(d.id));
    }
    let close = Rect::from_min_size(
        egui::pos2(rect.right() - 23.0, rect.top() + 2.0),
        egui::vec2(21.0, 24.0),
    );
    icons::paint(ui, close.shrink(5.0), Icon::Close, ink);
    if ui
        .interact(
            close,
            ui.id().with("delete-insert-channel"),
            egui::Sense::click(),
        )
        .clicked()
    {
        action = Some(StripAction::Delete(d.id));
    }
    let controls_rect = Rect::from_min_max(
        rect.min + egui::vec2(INSERT_GAP, 34.0),
        egui::pos2(rect.right() - INSERT_GAP, rect.top() + 82.0),
    );
    let mut source = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(controls_rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    source.style_mut().override_font_id = Some(egui::FontId::proportional(11.0));
    source.spacing_mut().item_spacing.y = 3.0;
    source.spacing_mut().interact_size.y = 21.0;
    source.small("Source");
    if source
        .add_sized(
            [source.available_width(), 21.0],
            egui::Button::new(match d.input {
                Input::System => "System audio",
                Input::Lfo => "LFO",
                _ => "MIDI",
            }),
        )
        .clicked()
    {
        action = Some(StripAction::Select(d.id));
    }
    let bottom = rect.bottom() - 144.0;
    let list_height = (d.inserts.as_ref().unwrap().stages.len() as f32
        * (INSERT_HEIGHT + INSERT_GAP)
        + INSERT_HEIGHT)
        .clamp(
            INSERT_HEIGHT,
            (rect.height() - 258.0).clamp(INSERT_HEIGHT, 145.0),
        );
    let list_rect = Rect::from_min_size(
        egui::pos2(
            rect.left() + INSERT_GAP,
            source.min_rect().bottom() + INSERT_GAP,
        ),
        egui::vec2(rect.width() - 2.0 * INSERT_GAP, list_height),
    );
    let mut list = ui.new_child(
        egui::UiBuilder::new()
            .id_salt("strip-inserts")
            .max_rect(list_rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    list.set_clip_rect(list_rect.intersect(ui.clip_rect()));
    list.spacing_mut().item_spacing.y = INSERT_GAP;
    list.spacing_mut().interact_size.y = 20.0;
    list.style_mut().override_font_id = Some(egui::FontId::proportional(11.0));
    let stage_count = d.inserts.as_ref().unwrap().stages.len();
    let stage_height = stage_count as f32 * (INSERT_HEIGHT + INSERT_GAP) - INSERT_GAP;
    let viewport_height = (list_height - INSERT_HEIGHT - INSERT_GAP).max(1.0);
    if stage_count > 0 && stage_height <= viewport_height {
        if let Some(a) = slot_list(&mut list, d, selected_insert, false) {
            action = Some(a);
        }
    } else if stage_count > 0 {
        egui::ScrollArea::vertical()
            .id_salt("insert-slots")
            .max_height(viewport_height)
            .min_scrolled_height(0.0)
            .auto_shrink([false, false])
            .show(&mut list, |ui| {
                if let Some(a) = slot_list(ui, d, selected_insert, false) {
                    action = Some(a);
                }
            });
    }
    let mut append = ui.new_child(
        egui::UiBuilder::new()
            .id_salt("append-strip-insert")
            .max_rect(Rect::from_min_max(
                egui::pos2(list_rect.left(), list_rect.bottom() - INSERT_HEIGHT),
                list_rect.max,
            ))
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    append.spacing_mut().interact_size.y = 20.0;
    append.style_mut().override_font_id = Some(egui::FontId::proportional(11.0));
    if let Some(a) = insert_menu(&mut append, d, d.inserts.as_ref().unwrap().stages.len()) {
        action = Some(a);
    }
    let graph = Rect::from_min_max(
        egui::pos2(rect.left() + 8.0, list_rect.bottom() + INSERT_GAP),
        egui::pos2(
            rect.right() - 8.0,
            (bottom - 8.0).max(list_rect.bottom() + 23.0),
        ),
    );
    let audio_only = d.input == Input::System
        && d.inserts.as_ref().unwrap().data_at(d.input, usize::MAX) == Data::Audio;
    let frame = snapshot.audio.get(&d.id).map(|f| f.frame.as_ref());
    if d.input == Input::Midi && d.inserts.as_ref().unwrap().stages.is_empty() {
        midi_preview::draw(ui, graph, &d.midi, midi.0, midi.1, snapshot.time, color);
    } else if audio_only {
        audio_channel::spectrum(ui, graph, frame, color);
    } else {
        ui.painter()
            .rect_filled(graph, 4.0, ui.visuals().extreme_bg_color);
        let signed = d.kind() == OutputKind::Bipolar;
        for level in 1..4 {
            ui.painter().hline(
                graph.x_range(),
                graph.top() + graph.height() * level as f32 / 4.0,
                egui::Stroke::new(0.5, ui.visuals().weak_text_color().gamma_multiply(0.15)),
            );
        }
        if let Some(history) = history {
            let points: Vec<_> = history
                .points()
                .map(|[x, y]| {
                    egui::pos2(
                        graph.left() + x * graph.width(),
                        graph.bottom()
                            - (if signed { (y + 1.0) * 0.5 } else { y * 0.5 }).clamp(0.0, 1.0)
                                * graph.height(),
                    )
                })
                .collect();
            if points.len() > 1 {
                ui.painter_at(graph)
                    .add(egui::Shape::line(points, egui::Stroke::new(1.7, color)));
            }
        }
        ui.painter().text(
            graph.left_top() + egui::vec2(5.0, 4.0),
            egui::Align2::LEFT_TOP,
            if d.input == Input::Midi {
                "MIDI · CONTROL"
            } else if d.input == Input::Lfo {
                "LFO · CONTROL"
            } else {
                "AUDIO / CONTROL"
            },
            egui::FontId::monospace(9.0),
            ui.visuals().weak_text_color(),
        );
    }
    if audio_only && let Some(bus) = bus {
        bus.enabled = d.enabled;
        bus.solo = d.solo;
        audio_channel::footer(ui, rect, bus, frame, color, ink, true);
        d.enabled = bus.enabled;
        d.solo = bus.solo;
    } else {
        channel_footer(
            ui,
            rect,
            d,
            &snapshot.values.get(&d.id).cloned().unwrap_or_default(),
            color,
            ink,
        );
    }
    StripOutput {
        action,
        over_inserts: ui.rect_contains_pointer(list_rect),
    }
}

impl Panel {
    pub(crate) fn insert_title(&self, id: u64) -> Option<String> {
        self.selected_insert
            .filter(|(channel, _)| *channel == id)
            .and_then(|(_, insert)| {
                self.bank
                    .channels
                    .iter()
                    .find(|d| d.id == id)?
                    .inserts
                    .as_ref()?
                    .stages
                    .iter()
                    .find(|s| s.id == insert)
                    .map(|s| s.effect.label().to_owned())
            })
    }
    pub(crate) fn has_insert_channel(&self, id: u64) -> bool {
        self.bank
            .channels
            .iter()
            .any(|d| d.id == id && d.inserts.is_some())
    }
    pub(crate) fn chain_contents(&mut self, ui: &mut egui::Ui, id: u64) {
        let Some(mut d) = self.bank.channels.iter().find(|d| d.id == id).cloned() else {
            return;
        };
        if d.inserts.is_none() {
            return;
        }
        let mut action = None;
        if let Some((_, insert_id)) = self.selected_insert.filter(|(channel, _)| *channel == id) {
            if ui.small_button("Back to channel & inserts").clicked() {
                self.selected_insert = None;
            }
            if let Some(insert) = d
                .inserts
                .as_mut()
                .unwrap()
                .stages
                .iter_mut()
                .find(|s| s.id == insert_id)
            {
                ui.add_space(8.0);
                ui.checkbox(&mut insert.enabled,"Insert enabled").help_text("Bypassed effects pass their input through. A bypassed detector silences the control branch while preserving audio.");
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(insert.effect.description())
                            .weak()
                            .size(11.0),
                    )
                    .wrap(),
                );
                let monitor = self
                    .snapshot
                    .inserts
                    .get(&id)
                    .and_then(|m| m.iter().find(|m| m.id == insert.id));
                audio_channel::inspector_group(ui, "Live monitor", |ui| {
                    scope(
                        ui,
                        monitor,
                        channel_color_from_rgb(
                            d.color
                                .unwrap_or(CHANNEL_COLORS[(id.saturating_sub(1) % 8) as usize]),
                        ),
                    );
                    if insert.effect.input() == Data::Audio {
                        let (rect, _) = ui.allocate_exact_size(
                            egui::vec2(ui.available_width(), 82.0),
                            egui::Sense::hover(),
                        );
                        audio_channel::spectrum(
                            ui,
                            rect,
                            self.snapshot.audio.get(&id).map(|f| f.frame.as_ref()),
                            channel_color_from_rgb(
                                d.color
                                    .unwrap_or(CHANNEL_COLORS[(id.saturating_sub(1) % 8) as usize]),
                            ),
                        );
                    }
                });
                audio_channel::inspector_group(ui, "Controls", |ui| {
                    effect_controls(ui, &mut insert.effect, monitor);
                });
                if ui
                    .small_button(if insert.effect.input() != insert.effect.output() {
                        "Remove detector + following controls"
                    } else {
                        "Remove insert"
                    })
                    .clicked()
                {
                    action = Some(StripAction::RemoveEffect(id, insert_id));
                }
            } else {
                self.selected_insert = None;
            }
        } else {
            audio_channel::inspector_group(ui, "Channel", |ui| {
                property_row(ui, "Name", |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut d.name)
                            .char_limit(80)
                            .desired_width(ui.available_width()),
                    );
                });
                property_row(ui, "Type", |ui| {
                    ui.label(match d.input {
                        Input::System => "Audio",
                        Input::Lfo => "LFO",
                        _ => "MIDI",
                    });
                });
                property_row(ui, "Enabled", |ui| {
                    ui.checkbox(&mut d.enabled, "");
                });
                if d.input == Input::System {
                    property_row(ui, "Capture", |ui| {
                        ui.label("System audio").help_text("The main capture is configured in Settings. Inserts process a private branch without changing speaker output.");
                    });
                }
                if d.input == Input::Lfo {
                    lfo_editor(ui, &mut d.lfo);
                }
                if d.input == Input::Midi {
                    let mut learn = self
                        .midi_learning
                        .filter(|(channel, _)| *channel == id)
                        .map(|(_, serial)| serial);
                    crate::midi::ui::binding_editor(
                        ui,
                        &mut d.midi,
                        &self.midi_snapshot,
                        &mut learn,
                    );
                    self.midi_learning = learn.map(|serial| (id, serial));
                }
            });
            audio_channel::inspector_group(ui, "Inserts", |ui| {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(if d.input == Input::System {
                            "Audio effects > detector > control processors"
                        } else {
                            "Control processors · audio effects are unavailable"
                        })
                        .weak()
                        .size(11.0),
                    )
                    .wrap(),
                );
                if let Some(a) = slot_list(ui, &d, self.selected_insert, true) {
                    action = Some(a);
                }
            });
            let output = self.snapshot.values.get(&id).cloned().unwrap_or_default();
            audio_channel::inspector_group(ui, "Output", |ui| {
                let data = d.inserts.as_ref().unwrap().data_at(d.input, usize::MAX);
                ui.label(if data == Data::Audio {
                    "Waveform + spectrum · add a detector for modulation"
                } else if d.kind() == OutputKind::Bipolar {
                    "Control curve · −1 to +1"
                } else {
                    "Control curve · 0 to 1"
                });
                if data == Data::Control {
                    ui.label(format!(
                        "Current {:.3} · {}",
                        output.value,
                        output.status.label()
                    ));
                }
                if let Some(users) = self.consumers.get(&id) {
                    for user in users {
                        ui.label(user);
                    }
                }
            });
        }
        d.sanitize();
        if let Some(dest) = self.bank.channels.iter_mut().find(|s| s.id == id) {
            *dest = d;
        }
        self.bank.sync_audio_channels();
        if let Some(a) = action {
            self.act(a);
        }
    }
    pub(super) fn chain_inspector(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        theme: &AppTheme,
        id: u64,
    ) {
        let d = self.bank.channels.iter().find(|d| d.id == id).unwrap();
        let title = self.insert_title(id).unwrap_or_else(|| d.name.clone());
        let subtitle = if self
            .selected_insert
            .is_some_and(|(channel, _)| channel == id)
        {
            format!("{} · Insert", d.name)
        } else {
            match d.input {
                Input::Lfo => "LFO channel",
                Input::Midi => "MIDI channel",
                _ => "Audio channel",
            }
            .into()
        };
        let mut editor = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(("chain-inspector", id))
                .max_rect(rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        editor.set_clip_rect(rect.intersect(ui.clip_rect()));
        workspace_ui::inspector_style(&mut editor, theme);
        self.close_inspector = workspace_ui::sidebar_header(
            &mut editor,
            Icon::AudioMappings,
            &title,
            &subtitle,
            "Source > inserts > modulation",
            "Close signal inspector",
        )
        .clicked();
        workspace_ui::inspector_scroll(
            &mut editor,
            ("chain-properties", id, self.selected_insert),
            |ui| {
                self.chain_contents(ui, id);
            },
        );
    }
}
fn channel_color_from_rgb([r, g, b]: [u8; 3]) -> egui::Color32 {
    egui::Color32::from_rgb(r, g, b)
}
fn scope(ui: &mut egui::Ui, monitor: Option<&Monitor>, color: egui::Color32) {
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), 145.0),
        egui::Sense::hover(),
    );
    response.help_text("Live input and output of this insert over two seconds. Gray is input; color is output. Audio stages show RMS activity; detector and control stages show the control curve. Both traces use the same scale.");
    ui.painter()
        .rect_filled(rect, 5.0, ui.visuals().extreme_bg_color);
    let plot = rect.shrink2(egui::vec2(8.0, 17.0));
    let history = monitor.map(|m| m.history.as_ref()).unwrap_or(&[]);
    let high = history
        .iter()
        .map(|p| p.input.abs().max(p.output.abs()))
        .fold(1.0_f32, f32::max);
    let low = if history.iter().any(|p| p.input < 0.0 || p.output < 0.0) {
        -high
    } else {
        0.0
    };
    let now = history.last().map_or(0.0, |p| p.time);
    for t in [0.0, 0.5, 1.0] {
        ui.painter().hline(
            plot.x_range(),
            plot.top() + plot.height() * t,
            egui::Stroke::new(0.5, ui.visuals().weak_text_color().gamma_multiply(0.18)),
        );
    }
    for (output, tint, width) in [
        (false, ui.visuals().weak_text_color(), 1.0),
        (true, color, 1.8),
    ] {
        let points: Vec<_> = history
            .iter()
            .map(|p| {
                egui::pos2(
                    plot.right() + (p.time - now) as f32 * plot.width() / 2.0,
                    plot.bottom()
                        - ((if output { p.output } else { p.input }) - low) / (high - low)
                            * plot.height(),
                )
            })
            .collect();
        if points.len() > 1 {
            ui.painter_at(plot)
                .add(egui::Shape::line(points, egui::Stroke::new(width, tint)));
        }
    }
    ui.painter().text(
        rect.left_top() + egui::vec2(7.0, 3.0),
        egui::Align2::LEFT_TOP,
        "INPUT / OUTPUT · 2s",
        egui::FontId::monospace(9.0),
        ui.visuals().weak_text_color(),
    );
    if let Some(m) = monitor {
        ui.horizontal_wrapped(|ui| {
            ui.weak(format!("In {:.3}", m.input));
            ui.colored_label(color, format!("Out {:.3}", m.output));
            if m.bpm > 0.0 {
                ui.label(format!(
                    "{:.1} BPM · {:.0}% confidence",
                    m.bpm,
                    m.confidence * 100.0
                ));
            }
        });
    }
}
fn number(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    suffix: &str,
) {
    property_row(ui, label, |ui| {
        ui.add_sized(
            egui::vec2(ui.available_width(), 22.0),
            egui::DragValue::new(value)
                .range(range)
                .speed(if suffix.is_empty() { 0.001 } else { 0.1 })
                .max_decimals(if suffix.is_empty() { 4 } else { 2 })
                .suffix(suffix),
        );
    });
}
fn effect_controls(ui: &mut egui::Ui, effect: &mut Effect, monitor: Option<&Monitor>) {
    match effect {
        Effect::AudioGain { db } => number(ui, "Gain", db, -48.0..=36.0, " dB"),
        Effect::Filter { low, high } => {
            number(ui, "Low cutoff", low, 20.0..=(*high - 1.0), " Hz");
            number(ui, "High cutoff", high, (*low + 1.0)..=20000.0, " Hz");
        }
        Effect::HighPass { hz } | Effect::LowPass { hz } => {
            number(ui, "Cutoff", hz, 20.0..=20000.0, " Hz")
        }
        Effect::AudioGate { db } => number(ui, "Threshold", db, -100.0..=0.0, " dBFS"),
        Effect::Channels { channels } => property_row(ui, "Channels", |ui| {
            crate::dropdown::ComboBox::from_id_salt("insert-audio-channels")
                .width(ui.available_width())
                .selected_text(channels.label())
                .show_ui(ui, |ui| {
                    for c in audio::Channels::ALL {
                        ui.selectable_value(channels, c, c.label());
                    }
                });
        }),
        Effect::Detect(d) => {
            if matches!(d.detector, Detector::Band | Detector::Bass | Detector::Hits) {
                number(
                    ui,
                    "Low frequency",
                    &mut d.response.low,
                    20.0..=(d.response.high / 1.001),
                    " Hz",
                );
                number(
                    ui,
                    "High frequency",
                    &mut d.response.high,
                    (d.response.low * 1.001)..=20000.0,
                    " Hz",
                );
            }
            number(
                ui,
                "Sensitivity",
                &mut d.response.gain_db,
                -24.0..=24.0,
                " dB",
            );
            number(
                ui,
                "Noise floor",
                &mut d.response.floor_db,
                -90.0..=-12.0,
                " dB",
            );
            number(
                ui,
                "Attack",
                &mut d.response.envelope.attack,
                1.0..=2000.0,
                " ms",
            );
            number(
                ui,
                "Release",
                &mut d.response.envelope.release,
                1.0..=5000.0,
                " ms",
            );
            property_row(ui, "Adaptive", |ui| {
                ui.checkbox(&mut d.adaptive, "");
            });
            if d.detector == Detector::Hits {
                number(
                    ui,
                    "Hit threshold",
                    &mut d.response.threshold,
                    0.01..=0.99,
                    "",
                );
                number(
                    ui,
                    "Retrigger",
                    &mut d.response.hold_ms,
                    20.0..=2000.0,
                    " ms",
                );
            }
        }
        Effect::Beat {
            sensitivity,
            manual,
            bpm,
            output,
        } => {
            property_row(ui, "Timing", |ui| {
                crate::dropdown::ComboBox::from_id_salt("beat-timing")
                    .selected_text(if *manual {
                        "Manual BPM"
                    } else {
                        "Detect beats"
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(manual, false, "Detect beats");
                        ui.selectable_value(manual, true, "Manual BPM");
                    });
            });
            if *manual {
                number(ui, "Tempo", bpm, 40.0..=240.0, " BPM");
            } else {
                number(ui, "Sensitivity", sensitivity, 0.5..=4.0, " ×");
            }
            property_row(ui, "Output", |ui| {
                crate::dropdown::ComboBox::from_id_salt("beat-output")
                    .selected_text(if *output == BeatOutput::Pulse {
                        "Beat pulse"
                    } else {
                        "Beat phase"
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(output, BeatOutput::Pulse, "Beat pulse");
                        ui.selectable_value(output, BeatOutput::Phase, "Beat phase · 0–1");
                    });
            });
            let phase = monitor.map_or(0.0, |m| m.phase);
            ui.add(egui::ProgressBar::new(phase).text("Beat phase"));
        }
        Effect::Gain { gain } => number(ui, "Gain", gain, 0.0..=8.0, " ×"),
        Effect::Normalize {
            low,
            high,
            automatic,
        } => {
            property_row(ui, "Auto range", |ui| {
                ui.checkbox(automatic, "");
            });
            ui.add_enabled_ui(!*automatic, |ui| {
                number(ui, "Input minimum", low, -8.0..=(*high - 0.0001), "");
                number(ui, "Input maximum", high, (*low + 0.0001)..=8.0, "");
            });
            let observed = monitor.and_then(|m| {
                let low = m
                    .history
                    .iter()
                    .map(|p| p.input)
                    .fold(f32::INFINITY, f32::min)
                    .clamp(-8.0, 8.0);
                let high = m
                    .history
                    .iter()
                    .map(|p| p.input)
                    .fold(f32::NEG_INFINITY, f32::max)
                    .clamp(-8.0, 8.0);
                (high - low >= 0.0001).then_some((low, high))
            });
            if ui.add_enabled(observed.is_some(), egui::Button::new("Use recent input range"))
                .help_text("Use the lowest and highest input observed in this insert's last two seconds, then map them to 0–1. A constant input cannot define a usable range.").clicked()
                && let Some((a,b)) = observed { *low = a; *high = b; *automatic = false; }
            ui.weak("Output range · 0 to 1");
        }
        Effect::Smooth { ms } => number(ui, "Smoothing", ms, 0.0..=5000.0, " ms"),
        Effect::Envelope { attack, release } => {
            number(ui, "Attack", attack, 1.0..=2000.0, " ms");
            number(ui, "Release", release, 1.0..=5000.0, " ms");
        }
        Effect::Threshold {
            threshold,
            hysteresis,
        } => {
            number(ui, "Threshold", threshold, -1.0..=1.0, "");
            number(ui, "Hysteresis", hysteresis, 0.0..=0.5, "");
        }
        Effect::Shape(s) => {
            number(ui, "Curve", &mut s.exponent, 0.1..=8.0, "");
            number(ui, "Dead zone", &mut s.dead_zone, 0.0..=0.95, "");
            property_row(ui, "Invert", |ui| {
                ui.checkbox(&mut s.invert, "");
            });
        }
        Effect::Invert => {
            ui.weak("Reverses the control curve; audio passes unchanged.");
        }
        Effect::Quantize { steps } => property_row(ui, "Levels", |ui| {
            ui.add(egui::DragValue::new(steps).range(2..=128));
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn insert_context_delete_never_opens_channel_menu_or_deletes_channel() {
        let ctx = egui::Context::default();
        let mut panel = Panel::default();
        let id = panel.bank.add_source(Input::Lfo).unwrap();
        panel.act(StripAction::AddEffect(id, 0, Effect::Invert));
        panel.act(StripAction::AddEffect(id, 1, Effect::Smooth { ms: 100.0 }));
        let render = |panel: &mut Panel, events| {
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(700.0, 600.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    panel.show(
                        ui,
                        ui.available_rect_before_wrap(),
                        &AnalysisFrame::default(),
                        false,
                        &AppTheme::default(),
                    )
                },
            );
            out.textures_delta.clear();
            out
        };
        let position = |out: &egui::FullOutput, label: &str| {
            out.shapes
                .iter()
                .find_map(|s| match &s.shape {
                    egui::Shape::Text(t) if t.galley.job.text == label => {
                        Some(t.pos + t.galley.size() * 0.5)
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("missing {label}"))
        };
        let click = |panel: &mut Panel, pos, button| {
            render(panel, vec![egui::Event::PointerMoved(pos)]);
            for pressed in [true, false] {
                render(
                    panel,
                    vec![egui::Event::PointerButton {
                        pos,
                        button,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    }],
                );
            }
            render(panel, vec![])
        };
        render(&mut panel, vec![]);
        let out = render(&mut panel, vec![]);
        let out = click(
            &mut panel,
            position(&out, "Invert"),
            egui::PointerButton::Secondary,
        );
        assert!(!out.shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.job.text == "Delete channel" || t.galley.job.text == "Channel color")));
        click(
            &mut panel,
            position(&out, "Remove insert"),
            egui::PointerButton::Primary,
        );
        assert_eq!(panel.bank.channels.len(), 1);
        assert_eq!(
            panel.bank.channels[0]
                .inserts
                .as_ref()
                .unwrap()
                .stages
                .len(),
            1
        );
        // Clicking the remaining insert then the channel header changes the delete target.
        let out = render(&mut panel, vec![]);
        click(
            &mut panel,
            position(&out, "Smooth"),
            egui::PointerButton::Primary,
        );
        assert!(panel.selected_insert.is_some());
        let out = render(&mut panel, vec![]);
        click(
            &mut panel,
            position(&out, "1 LFO"),
            egui::PointerButton::Primary,
        );
        assert!(panel.selected_insert.is_none());
        panel.delete_selection();
        assert!(panel.bank.channels.is_empty());
    }

    #[test]
    fn audio_insert_menu_creates_a_detector_and_control_menu_filters_audio_effects() {
        let ctx = egui::Context::default();
        let mut panel = Panel::default();
        let id = panel.bank.add_source(Input::System).unwrap();
        let render = |panel: &mut Panel, events| {
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(600.0, 700.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    let d = panel.bank.channels.iter().find(|d| d.id == id).unwrap();
                    if let Some(a) = slot_list(ui, d, panel.selected_insert, true) {
                        panel.act(a);
                    }
                },
            );
            out.textures_delta.clear();
            out
        };
        let position = |out: &egui::FullOutput, label: &str| {
            out.shapes
                .iter()
                .find_map(|s| match &s.shape {
                    egui::Shape::Text(t) if t.galley.job.text == label => {
                        Some(t.pos + t.galley.size() * 0.5)
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("missing {label}"))
        };
        let click = |panel: &mut Panel, pos| {
            render(panel, vec![egui::Event::PointerMoved(pos)]);
            let mut out = None;
            for pressed in [true, false] {
                out = Some(render(
                    panel,
                    vec![egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    }],
                ));
            }
            let _ = out;
            render(panel, vec![])
        };
        render(&mut panel, vec![]);
        let out = render(&mut panel, vec![]);
        let out = click(&mut panel, position(&out, "+ Insert"));
        click(&mut panel, position(&out, "Bass energy"));
        assert!(panel.bank.channels[0].has_control_output());
        assert!(panel.selected_insert.is_some());
        let out = render(&mut panel, vec![]);
        let out = click(&mut panel, position(&out, "+ Insert"));
        position(&out, "Smooth");
        position(&out, "Normalize");
        assert!(!out.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t) if t.galley.job.text=="Frequency filter"||t.galley.job.text=="Bass hits")));
        assert_eq!(panel.bank.slots.len(), 1);
        assert_eq!(panel.bank.channels.len(), 1);
        assert!(panel.bank.valid());
    }
    #[test]
    fn new_channel_strips_and_insert_editors_fit_resized_docks_and_compact_inspectors() {
        for height in [320.0, 450.0, 800.0] {
            let ctx = egui::Context::default();
            let mut panel = Panel::default();
            let id = panel.bank.add_source(Input::System).unwrap();
            panel.act(StripAction::AddEffect(
                id,
                0,
                Effect::Detect(crate::signals::inserts::Detection::new(Detector::Bass)),
            ));
            panel.act(StripAction::AddEffect(id, 1, Effect::Smooth { ms: 100.0 }));
            panel.bank.add_source(Input::Lfo);
            panel.bank.add_source(Input::Midi);
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(900.0, height + 20.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    panel.show(
                        ui,
                        ui.available_rect_before_wrap(),
                        &AnalysisFrame::default(),
                        false,
                        &AppTheme::default(),
                    );
                },
            );
            out.textures_delta.clear();
            for label in ["1 System audio", "Bass energy", "M", "S"] {
                assert!(
                    out.shapes.iter().any(
                        |s| matches!(&s.shape,egui::Shape::Text(t) if t.galley.job.text==label)
                    ),
                    "missing {label} at {height}"
                );
            }
            for effect in Effect::choices(Data::Audio)
                .into_iter()
                .chain(Effect::choices(Data::Control))
            {
                let mut out = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(236.0, 1200.0),
                        )),
                        ..Default::default()
                    },
                    |ui| {
                        workspace_ui::sidebar_style(ui, &AppTheme::default());
                        effect_controls(ui, &mut effect.clone(), None);
                        assert!(
                            ui.min_rect().right() <= 236.0,
                            "{} overflows",
                            effect.label()
                        );
                    },
                );
                out.textures_delta.clear();
            }
        }
    }
}
