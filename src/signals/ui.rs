use super::*;
use crate::{
    icons::{self, Icon},
    modulation::{
        Visualizer,
        ui::{combo, envelope_editor, spectrum_editor},
    },
    modules::{inspector_section, property_row, settings_group},
    parameter::Parameter,
    theme::AppTheme,
    workspace_ui,
};
use egui::{self, Rect};
use std::sync::Mutex;

pub type Shared = Arc<Mutex<Panel>>;
pub const CONTEXT_KEY: &str = "shared-musical-signals";
pub struct Panel {
    pub bank: Bank,
    pub snapshot: Arc<Snapshot>,
    pub selected: Option<u64>,
    pub show_signals: bool,
    pub consumers: BTreeMap<u64, Vec<String>>,
    span: Option<f32>,
    deleted: Option<(Definition, usize)>,
    history: BTreeMap<u64, std::collections::VecDeque<f32>>,
    last_history_time: f64,
    clipboard: Option<(Definition, bool)>,
    rename: Option<(u64, String, bool)>,
    learning: Option<RangeLearning>,
    sections: BTreeMap<u64, SignalSection>,
    pub close_inspector: bool,
    pub inspector_width: f32,
}
#[derive(Clone)]
struct RangeLearning {
    id: u64,
    started: f64,
    last_sample: Option<f64>,
    low: f32,
    high: f32,
    samples: usize,
}
impl RangeLearning {
    fn new(id: u64, now: f64) -> Self {
        Self {
            id,
            started: now,
            last_sample: None,
            low: 1.0,
            high: -1.0,
            samples: 0,
        }
    }
    fn observe(&mut self, snapshot: &Snapshot) {
        if self.last_sample == Some(snapshot.time) {
            return;
        }
        self.last_sample = Some(snapshot.time);
        if let Some(sample) = snapshot.values.get(&self.id)
            && sample.status == Status::Live
            && sample.raw.is_finite()
        {
            self.low = self.low.min(sample.raw);
            self.high = self.high.max(sample.raw);
            self.samples += 1;
        }
    }
    fn apply(&self, normalization: &mut Normalization) -> bool {
        if self.samples < 2 || self.high - self.low < 0.0001 {
            return false;
        }
        normalization.low = self.low;
        normalization.high = self.high;
        normalization.enabled = true;
        normalization.sanitize();
        true
    }
}
impl Default for Panel {
    fn default() -> Self {
        Self {
            bank: Bank::default(),
            snapshot: Arc::new(Snapshot::default()),
            selected: None,
            show_signals: false,
            consumers: BTreeMap::new(),
            span: None,
            deleted: None,
            history: Default::default(),
            last_history_time: 0.0,
            clipboard: None,
            rename: None,
            learning: None,
            sections: BTreeMap::new(),
            close_inspector: false,
            inspector_width: 236.0,
        }
    }
}
impl Panel {
    pub fn replace_bank(&mut self, bank: Bank) {
        self.selected = bank.channels.first().map(|d| d.id);
        self.bank = bank;
        self.clipboard = None;
        self.snapshot = Arc::new(Snapshot::default());
        self.last_history_time = 0.0;
        self.rename = None;
        self.learning = None;
        self.sections.clear();
        self.deleted = None;
        self.history.clear();
    }
    fn can_paste(&self) -> bool {
        self.clipboard.as_ref().is_some_and(|(d, cut)| {
            self.bank.channels.len() < MAX_SIGNALS
                || (*cut && self.bank.channels.iter().any(|v| v.id == d.id))
        })
    }
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        frame: &AnalysisFrame,
        inspector_open: bool,
        theme: &AppTheme,
    ) {
        self.close_inspector = false;
        let inspector_style = ui.style().clone();
        let mut page = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("signals-page")
                .max_rect(rect),
        );
        page.set_clip_rect(rect.intersect(ui.clip_rect()));
        signal_style(&mut page);
        let ui = &mut page;
        self.bank.sync_slots();
        self.sections
            .retain(|id, _| self.bank.channels.iter().any(|d| d.id == *id));
        if let Some(capture) = &mut self.learning {
            capture.observe(&self.snapshot);
            if self.snapshot.time - capture.started >= 5.0 {
                if let Some(d) = self.bank.channels.iter_mut().find(|d| d.id == capture.id) {
                    capture.apply(&mut d.normalization);
                }
                self.learning = None;
            }
        }
        if self.snapshot.time != self.last_history_time {
            self.last_history_time = self.snapshot.time;
            self.history
                .retain(|id, _| self.bank.channels.iter().any(|d| d.id == *id));
            for d in &self.bank.channels {
                let history = self.history.entry(d.id).or_default();
                history.push_back(self.snapshot.values.get(&d.id).map_or(0.0, |s| s.value));
                if history.len() > 96 {
                    history.pop_front();
                }
            }
        }
        let width = if inspector_open {
            self.inspector_width
                .clamp(236.0, 440.0)
                .min((rect.width() - 160.0).max(200.0))
        } else {
            0.0
        };
        let inspector = Rect::from_min_max(egui::pos2(rect.right() - width, rect.top()), rect.max);
        let mixer = Rect::from_min_max(rect.min, egui::pos2(inspector.left(), rect.bottom()));
        if inspector_open {
            ui.painter().rect_filled(inspector, 0.0, theme.panel);
            ui.painter().vline(
                inspector.left(),
                inspector.y_range(),
                egui::Stroke::new(1.0, theme.foreground.gamma_multiply(0.12)),
            );
        }
        let mixer = mixer.shrink(8.0);
        ui.painter()
            .rect_filled(mixer, 7.0, ui.visuals().extreme_bg_color);
        let content = mixer.shrink(1.0);
        let mut board = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("signals-mixer")
                .max_rect(content)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        board.set_clip_rect(content.intersect(ui.clip_rect()));
        let mut action = None;
        let height = (board.available_height() - 12.0).max(460.0);
        board.spacing_mut().item_spacing.x = 0.0;
        let slots = self.bank.slots.clone();
        egui::ScrollArea::both()
            .id_salt("signal-strips")
            .auto_shrink([false, false])
            .show(&mut board, |ui| {
                let mut insertion_line = None;
                ui.horizontal_top(|ui| {
                    for (slot, id) in slots.iter().copied().enumerate() {
                        ui.push_id(("channel-slot", slot), |ui| {
                            let (strip, _) = ui.allocate_exact_size(
                                egui::vec2(156.0, height),
                                egui::Sense::hover(),
                            );
                            if let Some(id) = id
                                && let Some(d) = self.bank.channels.iter_mut().find(|d| d.id == id)
                            {
                                let sample =
                                    self.snapshot.values.get(&id).cloned().unwrap_or_default();
                                let history = self.history.get(&id);
                                if let Some(a) = channel_strip(
                                    ui,
                                    strip,
                                    slot,
                                    d,
                                    &sample,
                                    self.selected == Some(id),
                                    self.consumers.get(&id).map_or(0, Vec::len),
                                    history,
                                ) {
                                    action = Some(a);
                                }
                            }
                            let menu_response = ui.interact(
                                strip,
                                ui.id().with("channel-menu"),
                                egui::Sense::hover(),
                            );
                            let open_menu = ui.rect_contains_pointer(strip)
                                && ui.input(|i| i.pointer.secondary_clicked());
                            if open_menu && let Some(id) = id {
                                self.selected = Some(id);
                            }
                            let definition =
                                id.and_then(|id| self.bank.channels.iter().find(|d| d.id == id));
                            egui::Popup::context_menu(&menu_response)
                                .open_memory(open_menu.then_some(egui::SetOpenCommand::Bool(true)))
                                .show(|ui| {
                                    if let Some(menu_action) = channel_menu(
                                        ui,
                                        definition,
                                        slot,
                                        self.can_paste(),
                                        self.bank.channels.len() < MAX_SIGNALS,
                                        self.deleted.is_some(),
                                    ) {
                                        action = Some(menu_action);
                                    }
                                });
                            if ui.rect_contains_pointer(strip)
                                && let Some(pos) = ui.input(|i| i.pointer.interact_pos())
                            {
                                let (boundary, x) = if pos.x < strip.center().x {
                                    (slot, strip.left())
                                } else {
                                    (slot + 1, strip.right())
                                };
                                insertion_line = Some((
                                    boundary,
                                    egui::pos2(x, strip.top()),
                                    egui::pos2(x, strip.bottom()),
                                ));
                            }
                        });
                    }
                    // A single unstructured workspace follows the last track. It
                    // remains reachable by scrolling even when tracks fill the view.
                    let free_width =
                        (ui.clip_rect().right() - ui.next_widget_position().x).max(120.0);
                    let (space, response) = ui
                        .allocate_exact_size(egui::vec2(free_width, height), egui::Sense::hover());
                    if slots.is_empty() {
                        ui.painter().text(
                            space.center(),
                            egui::Align2::CENTER_CENTER,
                            "Right-click to add a channel",
                            egui::FontId::proportional(13.0),
                            ui.visuals().weak_text_color(),
                        );
                    }
                    let open_menu = ui.rect_contains_pointer(space)
                        && ui.input(|i| i.pointer.secondary_clicked());
                    egui::Popup::context_menu(&response)
                        .open_memory(open_menu.then_some(egui::SetOpenCommand::Bool(true)))
                        .show(|ui| {
                            if let Some(menu_action) = channel_menu(
                                ui,
                                None,
                                slots.len(),
                                self.can_paste(),
                                self.bank.channels.len() < MAX_SIGNALS,
                                self.deleted.is_some(),
                            ) {
                                action = Some(menu_action);
                            }
                        });
                    if ui.rect_contains_pointer(space) {
                        insertion_line = Some((slots.len(), space.left_top(), space.left_bottom()));
                    }
                });
                // Resolve after every header has registered its payload, so moving
                // left shows the guide immediately too. Paint above neighboring tracks.
                if let Some((boundary, top, bottom)) = insertion_line
                    && let Some(payload) = egui::DragAndDrop::payload::<ChannelDrag>(ui.ctx())
                {
                    ui.painter().line_segment(
                        [top, bottom],
                        egui::Stroke::new(1.5, ui.visuals().text_color()),
                    );
                    if ui.input(|i| i.pointer.any_released()) {
                        action = Some(StripAction::Insert(payload.0, boundary));
                        egui::DragAndDrop::take_payload::<ChannelDrag>(ui.ctx());
                    }
                }
            });
        // egui's scissor is rectangular. Mask the track and scrollbar corners after
        // drawing, then paint the shared border above them for a continuous edge.
        rounded_content_corners(ui, content, 6.0, ui.visuals().extreme_bg_color);
        ui.painter().rect_stroke(
            mixer,
            7.0,
            egui::Stroke::new(1.0, ui.visuals().text_color().gamma_multiply(0.22)),
            egui::StrokeKind::Inside,
        );
        if let Some(action) = action {
            self.act(action);
        }
        self.rename_dialog(ui.ctx());
        if !inspector_open {
            return;
        }
        let mut editor = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("signal-inspector")
                .max_rect(inspector)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        editor.set_clip_rect(inspector);
        // Restore the app skin before applying the common compact inspector.
        editor.set_style(inspector_style);
        workspace_ui::sidebar_style(&mut editor, theme);
        let definition = self
            .selected
            .and_then(|id| self.bank.channels.iter().find(|d| d.id == id))
            .cloned();
        let (icon, title, subtitle, metadata): (Icon, String, String, String) =
            definition.as_ref().map_or(
                (
                    Icon::AudioMappings,
                    "Signal inspector".into(),
                    "Select a channel".into(),
                    String::new(),
                ),
                |d| {
                    let index = self
                        .bank
                        .slots
                        .iter()
                        .position(|slot| *slot == Some(d.id))
                        .unwrap_or(0)
                        + 1;
                    let status = self
                        .snapshot
                        .values
                        .get(&d.id)
                        .map_or(Status::Waiting, |s| s.status);
                    let metadata = format!(
                        "Channel {index} · {} · {:.0} kHz",
                        status.label(),
                        frame.sample_rate as f32 / 1000.0
                    );
                    (
                        if d.input == Input::Lfo {
                            Icon::Oscilloscope
                        } else {
                            Icon::AudioMappings
                        },
                        d.name.clone(),
                        d.method.label().into(),
                        metadata,
                    )
                },
            );
        self.close_inspector = workspace_ui::sidebar_header(
            &mut editor,
            icon,
            &title,
            &subtitle,
            &metadata,
            "Close the signal inspector",
        )
        .clicked();
        let mut section = self
            .selected
            .and_then(|id| self.sections.get(&id))
            .copied()
            .unwrap_or_default();
        let lfo = definition.as_ref().is_some_and(|d| d.input == Input::Lfo);
        if lfo && section == SignalSection::Response {
            section = SignalSection::Detection;
        }
        let bounds = editor.available_rect_before_wrap();
        let content = workspace_ui::inspector_rail(
            &mut editor,
            bounds,
            theme,
            ("signal-tabs", self.selected),
            |rail| {
                for tab in SignalSection::ALL {
                    if tab == SignalSection::Response && lfo {
                        continue;
                    }
                    if icons::sized_button(
                        rail,
                        tab.icon(lfo),
                        section == tab,
                        28.0,
                        tab.title(lfo),
                    )
                    .on_hover_text(tab.title(lfo))
                    .clicked()
                    {
                        section = tab;
                    }
                }
            },
        );
        if let Some(id) = self.selected {
            self.sections.insert(id, section);
        }
        let mut properties = editor.new_child(
            egui::UiBuilder::new()
                .id_salt("signal-properties")
                .max_rect(content),
        );
        properties.set_clip_rect(content.intersect(editor.clip_rect()));
        properties.data_mut(|d| {
            d.insert_temp(egui::Id::new("inspector-bounds"), content);
            d.insert_temp(egui::Id::new("property-row-index"), 0_usize);
            d.insert_temp(
                egui::Id::new("parameter-context"),
                format!("Signal · {title}"),
            );
        });
        egui::ScrollArea::vertical()
            .id_salt(("signal-detail-scroll", self.selected, section))
            .auto_shrink([false, false])
            .show(&mut properties, |ui| {
                inspector_section(ui, section.title(lfo), |ui| {
                    if let Some(id) = self.selected {
                        ui.push_id(id, |ui| self.configure(ui, frame, id, section));
                    } else {
                        ui.small("Select a signal channel to configure it.");
                    }
                });
                egui::Frame::new()
                    .inner_margin(8)
                    .show(ui, |ui| self.editor_actions(ui));
            });
        properties.data_mut(|d| {
            d.remove::<Rect>(egui::Id::new("inspector-bounds"));
            d.remove::<String>(egui::Id::new("parameter-context"));
        });
    }
    fn act(&mut self, action: StripAction) {
        match action {
            StripAction::Select(id) => self.selected = Some(id),
            StripAction::Delete(id) => {
                if self.rename.as_ref().is_some_and(|(v, _, _)| *v == id) {
                    self.rename = None;
                }
                self.deleted = self.bank.remove_channel(id);
                if self.selected == Some(id) {
                    self.selected = self.bank.channels.first().map(|d| d.id);
                }
            }
            StripAction::Insert(id, slot) => self.bank.insert_channel(id, slot),
            StripAction::Copy(id) | StripAction::Cut(id) => {
                if let Some(d) = self.bank.channels.iter().find(|d| d.id == id) {
                    let mut copy = d.clone();
                    copy.color = Some(d.color_rgb());
                    let cut = matches!(action, StripAction::Cut(_));
                    self.clipboard = Some((copy, cut));
                    if cut {
                        self.act(StripAction::Delete(id));
                    }
                }
            }
            StripAction::Duplicate(id, boundary) => {
                if let Some(copy) = self.bank.duplicate(id) {
                    self.bank.sync_slots();
                    self.bank
                        .insert_channel(copy, boundary.min(self.bank.slots.len()));
                    self.selected = Some(copy);
                }
            }
            StripAction::Paste(boundary) => {
                if let Some((mut d, cut)) = self.clipboard.clone() {
                    if cut && self.bank.channels.iter().any(|v| v.id == d.id) {
                        self.bank
                            .insert_channel(d.id, boundary.min(self.bank.slots.len()));
                    } else {
                        if self.bank.channels.len() >= MAX_SIGNALS {
                            return;
                        }
                        if !cut {
                            let Some(id) = self.bank.allocate() else {
                                return;
                            };
                            d.id = id;
                            d.name =
                                format!("{} copy", d.name.chars().take(70).collect::<String>());
                        }
                        self.bank.channels.push(d.clone());
                        self.bank.sync_slots();
                        self.bank
                            .insert_channel(d.id, boundary.min(self.bank.slots.len()));
                    }
                    self.selected = Some(d.id);
                    if cut {
                        if self.deleted.as_ref().is_some_and(|(v, _)| v.id == d.id) {
                            self.deleted = None;
                        }
                        self.clipboard = Some((d, false));
                    }
                }
            }
            StripAction::Rename(id) => {
                if let Some(d) = self.bank.channels.iter().find(|d| d.id == id) {
                    self.rename = Some((id, d.name.clone(), true));
                }
            }
            StripAction::Color(id, color) => {
                if let Some(d) = self.bank.channels.iter_mut().find(|d| d.id == id) {
                    d.color = Some(color);
                }
            }
            StripAction::AddAt(method, boundary) => {
                if let Some(id) = self.bank.add(method) {
                    self.bank.sync_slots();
                    self.bank
                        .insert_channel(id, boundary.min(self.bank.slots.len()));
                    self.selected = Some(id);
                }
            }
            StripAction::Undo => {
                if let Some((d, slot)) = self.deleted.take() {
                    if self.bank.channels.len() < MAX_SIGNALS {
                        self.selected = Some(d.id);
                        let id = d.id;
                        self.bank.channels.push(d);
                        self.bank.sync_slots();
                        self.bank
                            .insert_channel(id, slot.min(self.bank.slots.len()));
                    } else {
                        self.deleted = Some((d, slot));
                    }
                }
            }
        }
    }
    fn rename_dialog(&mut self, ctx: &egui::Context) {
        let Some((id, name, focus)) = &mut self.rename else {
            return;
        };
        let mut open = true;
        let mut apply = false;
        let mut cancel = false;
        egui::Window::new("Rename channel")
            .id(egui::Id::new("rename-signal-channel"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                let field = ui.add(
                    egui::TextEdit::singleline(name)
                        .char_limit(80)
                        .desired_width(280.0),
                );
                if *focus {
                    field.request_focus();
                    *focus = false;
                }
                let valid = crate::workspace::valid_name(name);
                if !valid {
                    ui.small("Enter a name of 1–80 characters, without control characters.");
                }
                ui.horizontal(|ui| {
                    apply = ui.add_enabled(valid, egui::Button::new("Rename")).clicked();
                    cancel = ui.button("Cancel").clicked();
                });
                apply |= valid && ui.input(|i| i.key_pressed(egui::Key::Enter));
                cancel |= ui.input(|i| i.key_pressed(egui::Key::Escape));
            });
        if apply && let Some(d) = self.bank.channels.iter_mut().find(|d| d.id == *id) {
            d.name = name.trim().into();
        }
        if apply || cancel || !open {
            self.rename = None;
        }
    }
    fn editor_actions(&mut self, ui: &mut egui::Ui) {
        let mut action = None;
        ui.horizontal_wrapped(|ui| {
            if let Some(id) = self.selected {
                if ui.small_button("Duplicate").clicked() {
                    action = Some(StripAction::Duplicate(id, self.bank.slots.len()));
                }
                if ui.small_button("Delete channel").clicked() {
                    action = Some(StripAction::Delete(id));
                }
            }
            if self.deleted.is_some() && ui.small_button("Undo delete").clicked() {
                action = Some(StripAction::Undo);
            }
        });
        if let Some(action) = action {
            self.act(action);
        }
    }
    fn configure(
        &mut self,
        ui: &mut egui::Ui,
        frame: &AnalysisFrame,
        id: u64,
        section: SignalSection,
    ) {
        let Some(d) = self.bank.channels.iter_mut().find(|d| d.id == id) else {
            return;
        };
        let sample = self.snapshot.values.get(&id).cloned().unwrap_or_default();
        match section {
            SignalSection::Source => settings_group(ui, "Channel", |ui| {
                property_row(ui, "Name", |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut d.name)
                            .desired_width(ui.available_width())
                            .char_limit(80),
                    );
                });
                property_row(ui, "Input", |ui| {
                    egui::ComboBox::from_id_salt("signal-input")
                        .width(ui.available_width())
                        .truncate()
                        .selected_text(d.input.label())
                        .show_ui(ui, |ui| {
                            if ui
                                .selectable_value(
                                    &mut d.input,
                                    Input::System,
                                    Input::System.label(),
                                )
                                .changed()
                            {
                                d.method = Method::Intensity;
                            }
                            if ui
                                .selectable_value(&mut d.input, Input::Lfo, "LFO")
                                .changed()
                            {
                                d.method = Method::Lfo;
                            }
                            for input in Input::ALL.into_iter().skip(2) {
                                ui.add_enabled(false, egui::Button::new(input.label()));
                            }
                        });
                });
                let previous = d.method;
                combo(
                    ui,
                    "signal-method",
                    "Detector",
                    &mut d.method,
                    &Method::ALL,
                    Method::label,
                );
                if d.method != previous {
                    d.input = if d.method == Method::Lfo {
                        Input::Lfo
                    } else {
                        Input::System
                    };
                }
                ui.small(d.method.description());
                property_row(ui, "Output", |ui| {
                    ui.label(match d.kind() {
                        OutputKind::Unipolar => "0 to 1",
                        OutputKind::Bipolar => "−1 to +1",
                        OutputKind::EventsAndEnvelope => "Hits + envelope",
                    });
                });
            }),
            SignalSection::Detection => {
                if d.input == Input::Lfo {
                    settings_group(ui, "Oscillator", |ui| lfo_editor(ui, &mut d.lfo));
                } else {
                    settings_group(ui, "Frequency selection", |ui| {
                        if matches!(d.method, Method::Band | Method::BassHits) {
                            let fallback = AudioFeatures::new(&[], 0.0, [0.0; 2]);
                            spectrum_editor(
                                ui,
                                frame.features.as_deref().unwrap_or(&fallback),
                                &mut d.response,
                                1,
                            );
                        } else {
                            ui.small("This detector uses the full mix. Choose Band energy or Bass hits in Source to select a frequency range.");
                        }
                    });
                    settings_group(ui, "Sensitivity", |ui| {
                        ui.add(
                            Parameter::new(&mut d.response.gain_db, -24.0..=24.0, 0.0)
                                .text("Gain dB"),
                        );
                        ui.add(
                            Parameter::new(&mut d.response.floor_db, -90.0..=-12.0, -66.0)
                                .text("Noise floor dB"),
                        );
                        ui.add(
                            Parameter::new(&mut d.response.curve, 0.25..=4.0, 2.0)
                                .text("Sensitivity curve"),
                        );
                        property_row(ui, "Adaptive", |ui| {
                            ui.checkbox(&mut d.adaptive, "")
                                .on_hover_text("Adaptive sensitivity");
                        });
                    });
                }
            }
            SignalSection::Response => settings_group(ui, "Envelope", |ui| {
                if d.method == Method::BassHits {
                    d.response.response = Response::Transient;
                } else {
                    combo(
                        ui,
                        "signal-response",
                        "Response",
                        &mut d.response.response,
                        &[Response::Follow, Response::Gate, Response::Transient],
                        |r| match r {
                            Response::Follow => "Level follower",
                            Response::Gate => "Gated ADSR",
                            Response::Transient => "Hit ADSR",
                        },
                    );
                }
                envelope_editor(
                    ui,
                    &mut d.response.envelope,
                    d.response.response,
                    &mut self.span,
                    sample.raw.abs().min(1.0),
                );
                if d.response.response != Response::Follow {
                    ui.add(
                        Parameter::new(&mut d.response.threshold, 0.01..=0.99, 0.25)
                            .text("Threshold"),
                    );
                    if d.response.response == Response::Transient {
                        ui.add(
                            Parameter::new(&mut d.response.hold_ms, 20.0..=2000.0, 120.0)
                                .text("Retrigger ms"),
                        );
                    }
                }
            }),
            SignalSection::Output => {
                normalization_editor(ui, d, &sample, &self.snapshot, &mut self.learning)
            }
            SignalSection::Routing => settings_group(ui, "Used by", |ui| {
                if let Some(users) = self.consumers.get(&id) {
                    for name in users {
                        ui.label(name);
                    }
                } else {
                    ui.small("No mappings yet. Connect this source from a macro in Visualize.");
                }
            }),
        }
        d.sanitize();
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum SignalSection {
    #[default]
    Source,
    Detection,
    Response,
    Output,
    Routing,
}
impl SignalSection {
    const ALL: [Self; 5] = [
        Self::Source,
        Self::Detection,
        Self::Response,
        Self::Output,
        Self::Routing,
    ];
    fn title(self, lfo: bool) -> &'static str {
        match self {
            Self::Source => "Source",
            Self::Detection if lfo => "Oscillator",
            Self::Detection => "Detection",
            Self::Response => "Response",
            Self::Output => "Output",
            Self::Routing => "Mappings",
        }
    }
    fn icon(self, lfo: bool) -> Icon {
        match self {
            Self::Source => Icon::AudioMappings,
            Self::Detection if lfo => Icon::Oscilloscope,
            Self::Detection => Icon::Bars,
            Self::Response => Icon::Waveform,
            Self::Output => Icon::Settings,
            Self::Routing => Icon::Inspector,
        }
    }
}

fn normalization_editor(
    ui: &mut egui::Ui,
    d: &mut Definition,
    sample: &Sample,
    snapshot: &Snapshot,
    learning: &mut Option<RangeLearning>,
) {
    settings_group(ui, "Normalization", |ui| {
        ui.checkbox(&mut d.normalization.enabled,"Normalize output")
        .on_hover_text("Stretch a narrow detector/envelope range to 0–1 before the influence fader. Values outside the input limits are clamped.");
        ui.small(format!(
            "Raw {:.3} · output {:.3}",
            sample.raw, sample.value
        ));
        ui.add_enabled_ui(d.normalization.enabled, |ui| {
            ui.add(
                Parameter::new(&mut d.normalization.low, -1.0..=0.9999, 0.0)
                    .bounds(-1.0..=f64::from(d.normalization.high - 0.0001))
                    .text("Input low"),
            );
            ui.add(
                Parameter::new(&mut d.normalization.high, -0.9999..=1.0, 1.0)
                    .bounds(f64::from(d.normalization.low + 0.0001)..=1.0)
                    .text("Input high"),
            );
        });
        if let Some(capture) = learning.as_mut().filter(|l| l.id == d.id) {
            capture.observe(snapshot);
        }
        let running = learning.as_ref().is_some_and(|l| l.id == d.id);
        let elapsed = learning
            .as_ref()
            .filter(|l| l.id == d.id)
            .map_or(0.0, |l| (snapshot.time - l.started).max(0.0));
        let mut finish = running && elapsed >= 5.0;
        ui.horizontal(|ui| {
            if ui
                .button(if running {
                    "Stop & apply"
                } else {
                    "Learn range · 5 s"
                })
                .clicked()
            {
                if running {
                    finish = true;
                } else {
                    *learning = Some(RangeLearning::new(d.id, snapshot.time));
                }
            }
            if running {
                ui.weak(format!("{:.1} s", (5.0 - elapsed).max(0.0)));
                if ui.button("Cancel").clicked() {
                    *learning = None;
                    finish = false;
                }
            }
        });
        if finish && let Some(capture) = learning.take() {
            if capture.apply(&mut d.normalization) {
                ui.weak("Range learned.");
            } else {
                ui.weak("No changing live signal detected; range retained.");
            }
        }
        if running && let Some(l) = learning {
            if l.samples > 0 {
                ui.small(format!("Observed {:.3} – {:.3}", l.low, l.high));
            }
            ui.ctx().request_repaint();
        }
        if d.normalization.enabled {
            let (rect, _) = ui
                .allocate_exact_size(egui::vec2(ui.available_width(), 52.0), egui::Sense::hover());
            let plot = rect.shrink2(egui::vec2(6.0, 5.0));
            let signed = d.input == Input::Lfo && d.lfo.bipolar || d.method == Method::Balance;
            let domain_low = if signed { -1.0 } else { 0.0 };
            let x = |v: f32| {
                plot.left() + plot.width() * ((v - domain_low) / (1.0 - domain_low)).clamp(0.0, 1.0)
            };
            let y = |v: f32| plot.bottom() - plot.height() * v;
            let color = channel_color(d);
            ui.painter()
                .rect_filled(rect, 3.0, ui.visuals().extreme_bg_color);
            ui.painter().line_segment(
                [
                    egui::pos2(plot.left(), y(0.0)),
                    egui::pos2(plot.right(), y(0.0)),
                ],
                ui.visuals().widgets.inactive.bg_stroke,
            );
            ui.painter().add(egui::Shape::line(
                vec![
                    egui::pos2(x(domain_low), y(d.normalization.map(domain_low))),
                    egui::pos2(x(d.normalization.low), y(0.0)),
                    egui::pos2(x(d.normalization.high), y(1.0)),
                    egui::pos2(x(1.0), y(1.0)),
                ],
                egui::Stroke::new(1.5, color),
            ));
            ui.painter().circle_filled(
                egui::pos2(x(sample.raw), y(d.normalization.map(sample.raw))),
                3.0,
                color,
            );
            ui.small("Output 0–1 · before influence");
        }
    });
    settings_group(ui, "Smoothing", |ui| {
        ui.add(Parameter::new(&mut d.smoothing_ms,0.0..=1000.0,0.0)
        .bounds(0.0..=5000.0).text("Smoothing ms"))
        .on_hover_text("Soften sudden changes in this channel's final output. 0 ms is off. Higher values give smoother, slower motion. Applies after normalization and influence to every mapping; hit detection stays immediate.");
        ui.horizontal_wrapped(|ui| {
            for (label, amount) in [
                ("Off", 0.0),
                ("50 ms", 50.0),
                ("200 ms", 200.0),
                ("500 ms", 500.0),
            ] {
                if ui
                    .selectable_label((d.smoothing_ms - amount).abs() < 0.1, label)
                    .clicked()
                {
                    d.smoothing_ms = amount;
                }
            }
        });
    });
}

#[derive(Clone, Copy)]
struct ChannelDrag(u64);
#[derive(Clone, Copy)]
enum StripAction {
    Select(u64),
    Delete(u64),
    Insert(u64, usize),
    Undo,
    Copy(u64),
    Cut(u64),
    Duplicate(u64, usize),
    Paste(usize),
    Rename(u64),
    Color(u64, [u8; 3]),
    AddAt(Method, usize),
}
pub(crate) fn channel_color(d: &Definition) -> egui::Color32 {
    let [r, g, b] = d.color_rgb();
    egui::Color32::from_rgb(r, g, b)
}

// Reference swatches in their original 14-column order, converted from the
// screenshot's embedded display profile to the sRGB used by egui.
const CHANNEL_SWATCHES: [[[u8; 3]; 14]; 5] = [
    [
        [255, 148, 166],
        [255, 165, 41],
        [204, 153, 38],
        [248, 244, 124],
        [191, 251, 0],
        [24, 255, 46],
        [35, 255, 168],
        [92, 255, 232],
        [139, 197, 255],
        [84, 128, 228],
        [146, 167, 255],
        [216, 108, 228],
        [229, 83, 160],
        [255, 255, 255],
    ],
    [
        [255, 54, 54],
        [246, 108, 3],
        [153, 114, 75],
        [254, 241, 52],
        [135, 255, 104],
        [62, 195, 3],
        [3, 191, 175],
        [24, 233, 255],
        [18, 163, 238],
        [2, 125, 192],
        [135, 108, 228],
        [182, 119, 198],
        [255, 57, 212],
        [208, 208, 208],
    ],
    [
        [226, 103, 90],
        [255, 163, 116],
        [211, 173, 113],
        [237, 255, 174],
        [210, 228, 152],
        [186, 208, 116],
        [155, 196, 141],
        [212, 253, 225],
        [205, 241, 248],
        [186, 193, 227],
        [205, 187, 228],
        [174, 152, 229],
        [229, 220, 225],
        [169, 169, 169],
    ],
    [
        [198, 146, 139],
        [183, 130, 86],
        [152, 131, 106],
        [191, 186, 106],
        [167, 190, 0],
        [125, 176, 77],
        [135, 194, 186],
        [156, 179, 196],
        [133, 165, 194],
        [131, 148, 203],
        [165, 149, 181],
        [191, 159, 190],
        [188, 113, 151],
        [123, 123, 123],
    ],
    [
        [175, 51, 51],
        [169, 81, 49],
        [114, 78, 65],
        [219, 195, 0],
        [133, 151, 30],
        [83, 159, 49],
        [8, 156, 142],
        [34, 99, 132],
        [27, 47, 150],
        [47, 82, 162],
        [98, 75, 173],
        [163, 75, 173],
        [204, 46, 109],
        [60, 60, 60],
    ],
];

fn channel_menu(
    ui: &mut egui::Ui,
    d: Option<&Definition>,
    slot: usize,
    can_paste: bool,
    can_create: bool,
    can_undo: bool,
) -> Option<StripAction> {
    ui.set_min_width(328.0);
    let mut action = None;
    if let Some(d) = d {
        for (label, value, enabled) in [
            ("Cut", StripAction::Cut(d.id), true),
            ("Copy", StripAction::Copy(d.id), true),
            ("Paste", StripAction::Paste(slot + 1), can_paste),
            (
                "Duplicate",
                StripAction::Duplicate(d.id, slot + 1),
                can_create,
            ),
            ("Delete", StripAction::Delete(d.id), true),
        ] {
            if ui.add_enabled(enabled, egui::Button::new(label)).clicked() {
                action = Some(value);
                ui.close();
            }
        }
        ui.separator();
        if ui.button("Rename").clicked() {
            action = Some(StripAction::Rename(d.id));
            ui.close();
        }
    } else if ui
        .add_enabled(can_paste, egui::Button::new("Paste channel"))
        .clicked()
    {
        action = Some(StripAction::Paste(slot));
        ui.close();
    }
    ui.add_enabled_ui(can_create, |ui| {
        ui.menu_button(
            if d.is_some() {
                "Insert new channel"
            } else {
                "Add channel"
            },
            |ui| {
                for method in Method::ALL {
                    if ui.button(method.label()).clicked() {
                        action = Some(StripAction::AddAt(method, slot + usize::from(d.is_some())));
                        ui.close();
                    }
                }
            },
        );
    });
    if can_undo && ui.button("Undo delete / cut").clicked() {
        action = Some(StripAction::Undo);
        ui.close();
    }
    if let Some(d) = d {
        ui.separator();
        ui.small("Channel color");
        ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
        ui.spacing_mut().interact_size.y = 16.0;
        for row in CHANNEL_SWATCHES {
            ui.horizontal(|ui| {
                for [r, g, b] in row {
                    let color = egui::Color32::from_rgb(r, g, b);
                    let (rect, response) =
                        ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::click());
                    ui.painter().rect_filled(rect, 0.0, color);
                    if color == channel_color(d) {
                        ui.painter().rect_stroke(
                            rect.expand(1.0),
                            1.0,
                            egui::Stroke::new(1.0, ui.visuals().text_color()),
                            egui::StrokeKind::Outside,
                        );
                    }
                    if response
                        .on_hover_text(format!(
                            "#{:02X}{:02X}{:02X}",
                            color.r(),
                            color.g(),
                            color.b()
                        ))
                        .clicked()
                    {
                        action = Some(StripAction::Color(d.id, [color.r(), color.g(), color.b()]));
                        ui.close();
                    }
                }
            });
        }
    }
    action
}
fn blend(a: egui::Color32, b: egui::Color32, t: f32) -> egui::Color32 {
    egui::Color32::from_rgb(
        (a.r() as f32 + (b.r() as f32 - a.r() as f32) * t) as u8,
        (a.g() as f32 + (b.g() as f32 - a.g() as f32) * t) as u8,
        (a.b() as f32 + (b.b() as f32 - a.b() as f32) * t) as u8,
    )
}
fn signal_style(ui: &mut egui::Ui) {
    let panel = ui.visuals().panel_fill;
    let text = ui.visuals().text_color();
    let field = blend(panel, egui::Color32::BLACK, 0.32);
    let visuals = ui.visuals_mut();
    visuals.override_text_color = Some(blend(text, egui::Color32::WHITE, 0.65));
    visuals.weak_text_color = Some(blend(text, egui::Color32::WHITE, 0.15));
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, text.gamma_multiply(0.30));
    visuals.extreme_bg_color = field;
    visuals.widgets.inactive.bg_fill = field;
    visuals.widgets.inactive.weak_bg_fill = field;
    visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, text.gamma_multiply(0.30));
    for widget in [
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.corner_radius = egui::CornerRadius::same(4);
    }
}
fn rounded_content_corners(ui: &egui::Ui, rect: Rect, radius: f32, color: egui::Color32) {
    let radius = radius
        .min(rect.width() * 0.5)
        .min(rect.height() * 0.5)
        .max(0.0);
    let mut mesh = egui::Mesh::default();
    for (corner, direction) in [
        (rect.left_top(), egui::vec2(1.0, 1.0)),
        (rect.right_top(), egui::vec2(-1.0, 1.0)),
        (rect.left_bottom(), egui::vec2(1.0, -1.0)),
        (rect.right_bottom(), egui::vec2(-1.0, -1.0)),
    ] {
        let start = mesh.vertices.len() as u32;
        mesh.colored_vertex(corner, color);
        for i in 0..=12 {
            let angle = std::f32::consts::PI + i as f32 / 12.0 * std::f32::consts::FRAC_PI_2;
            let offset = egui::vec2(radius + radius * angle.cos(), radius + radius * angle.sin());
            mesh.colored_vertex(corner + offset * direction, color);
        }
        for i in 0..12 {
            mesh.add_triangle(start, start + i + 1, start + i + 2);
        }
    }
    ui.painter().add(egui::Shape::mesh(mesh));
}

fn surface(ui: &egui::Ui, rect: Rect, color: egui::Color32, selected: bool) {
    let body = blend(
        ui.visuals().panel_fill,
        ui.visuals().text_color(),
        if selected { 0.16 } else { 0.075 },
    );
    ui.painter().rect_filled(rect, 0.0, body);
    let header = Rect::from_min_size(rect.min, egui::vec2(rect.width(), 28.0));
    ui.painter().rect_filled(header, 0.0, color);
    let divider = egui::Stroke::new(
        1.0,
        blend(ui.visuals().panel_fill, egui::Color32::BLACK, 0.5),
    );
    ui.painter()
        .vline(rect.right() - 0.5, rect.y_range(), divider);
    ui.painter().hline(rect.x_range(), header.bottom(), divider);
}
fn track_title(ui: &egui::Ui, rect: Rect, text: String, color: egui::Color32) {
    let galley = egui::WidgetText::from(egui::RichText::new(text).size(13.0).strong().color(color))
        .into_galley(
            ui,
            Some(egui::TextWrapMode::Truncate),
            rect.width(),
            egui::FontId::proportional(13.0),
        );
    let origin = egui::pos2(rect.left(), rect.center().y - galley.size().y * 0.5);
    let painter = ui.painter_at(rect);
    // The bundled proportional face is light; a subtle second pass adds portable header weight.
    painter.galley(origin, galley.clone(), color);
    painter.galley(origin + egui::vec2(0.35, 0.0), galley, color);
}
#[allow(clippy::too_many_arguments)]
fn channel_strip(
    ui: &mut egui::Ui,
    rect: Rect,
    slot: usize,
    d: &mut Definition,
    sample: &Sample,
    selected: bool,
    links: usize,
    history: Option<&std::collections::VecDeque<f32>>,
) -> Option<StripAction> {
    let color = channel_color(d);
    surface(ui, rect, color, selected);
    let mut action = ui
        .interact(
            rect,
            ui.id().with("select-channel-body"),
            egui::Sense::click(),
        )
        .clicked()
        .then_some(StripAction::Select(d.id));
    let title = Rect::from_min_max(rect.min, egui::pos2(rect.right() - 24.0, rect.top() + 28.0));
    let luma = 0.2126 * f32::from(color.r())
        + 0.7152 * f32::from(color.g())
        + 0.0722 * f32::from(color.b());
    let dark = if luma < 110.0 {
        egui::Color32::from_gray(245)
    } else {
        egui::Color32::from_rgb(12, 18, 26)
    };
    track_title(
        ui,
        Rect::from_min_max(
            egui::pos2(title.left() + 8.0, title.top()),
            egui::pos2(title.right() - 2.0, title.bottom()),
        ),
        format!("{} {}", slot + 1, d.name),
        dark,
    );
    let response = ui
        .interact(
            title,
            ui.id().with("channel-title"),
            egui::Sense::click_and_drag(),
        )
        .on_hover_text("Select channel · drag to reorder");
    if response.clicked() {
        action = Some(StripAction::Select(d.id));
    }
    if response.drag_started() {
        response.dnd_set_drag_payload(ChannelDrag(d.id));
    }
    let close = Rect::from_min_size(
        egui::pos2(rect.right() - 23.0, rect.top() + 2.0),
        egui::vec2(21.0, 24.0),
    );
    crate::icons::paint(ui, close.shrink(5.0), crate::icons::Icon::Close, dark);
    if ui
        .interact(close, ui.id().with("delete-channel"), egui::Sense::click())
        .on_hover_text("Delete channel · undo is available in the signal inspector")
        .clicked()
    {
        action = Some(StripAction::Delete(d.id));
    }
    let mut controls = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(Rect::from_min_max(
                egui::pos2(rect.left() + 7.0, rect.top() + 35.0),
                egui::pos2(rect.right() - 7.0, rect.top() + 200.0),
            ))
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    controls.style_mut().override_font_id = Some(egui::FontId::proportional(11.0));
    controls.spacing_mut().item_spacing.y = 3.0;
    controls.spacing_mut().interact_size.y = 21.0;
    controls.small("Source");
    egui::ComboBox::from_id_salt("strip-input")
        .width(142.0)
        .selected_text(d.input.label())
        .show_ui(&mut controls, |ui| {
            if ui
                .selectable_value(&mut d.input, Input::System, Input::System.label())
                .changed()
            {
                d.method = Method::Intensity;
            }
            if ui
                .selectable_value(&mut d.input, Input::Lfo, "LFO")
                .changed()
            {
                d.method = Method::Lfo;
            }
            for input in Input::ALL.into_iter().skip(2) {
                ui.add_enabled(false, egui::Button::new(input.label()));
            }
        });
    controls.small("Detector");
    egui::ComboBox::from_id_salt("strip-method")
        .width(142.0)
        .selected_text(d.method.label())
        .show_ui(&mut controls, |ui| {
            for method in Method::ALL {
                if ui
                    .selectable_value(&mut d.method, method, method.label())
                    .changed()
                {
                    d.input = if method == Method::Lfo {
                        Input::Lfo
                    } else {
                        Input::System
                    };
                }
            }
        });
    if controls
        .add_sized(
            [142.0, 22.0],
            egui::Button::new(if matches!(d.method, Method::Band | Method::BassHits) {
                format!("{:.0} – {:.0} Hz", d.response.low, d.response.high)
            } else if d.input == Input::Lfo {
                format!("{:.2} Hz · {}", d.lfo.rate, d.lfo.waveform.label())
            } else {
                "Full mix".into()
            }),
        )
        .on_hover_text("Edit signal frequency and response")
        .clicked()
    {
        action = Some(StripAction::Select(d.id));
    }
    controls.small(sample.status.label());
    if controls
        .small_button(format!("{links} mappings · Inspect"))
        .clicked()
    {
        action = Some(StripAction::Select(d.id));
    }
    d.sanitize();
    let bottom = rect.bottom() - 234.0;
    let graph = Rect::from_min_max(
        egui::pos2(rect.left() + 8.0, rect.top() + 212.0),
        egui::pos2(rect.right() - 8.0, (bottom - 12.0).max(rect.top() + 230.0)),
    );
    ui.painter().rect_filled(
        graph,
        4.0,
        if selected {
            blend(
                ui.visuals().extreme_bg_color,
                ui.visuals().text_color(),
                0.065,
            )
        } else {
            ui.visuals().extreme_bg_color
        },
    );
    for i in 1..4 {
        ui.painter().hline(
            graph.x_range(),
            graph.top() + graph.height() * i as f32 / 4.0,
            egui::Stroke::new(0.5, ui.visuals().weak_text_color().gamma_multiply(0.14)),
        );
    }
    if let Some(history) = history
        && history.len() > 1
    {
        let points: Vec<egui::Pos2> = history
            .iter()
            .enumerate()
            .map(|(i, v)| {
                egui::pos2(
                    graph.left() + i as f32 / 95.0 * graph.width(),
                    if d.kind() == OutputKind::Bipolar {
                        graph.center().y - v.clamp(-2.0, 2.0) * 0.25 * graph.height()
                    } else {
                        graph.bottom() - v.clamp(0.0, 2.0) * 0.5 * graph.height()
                    },
                )
            })
            .collect();
        ui.painter().add(egui::Shape::line(
            points.clone(),
            egui::Stroke::new(4.0, color.gamma_multiply(0.14)),
        ));
        ui.painter()
            .add(egui::Shape::line(points, egui::Stroke::new(1.7, color)));
    }
    ui.painter().hline(
        rect.x_range(),
        bottom,
        ui.visuals().widgets.noninteractive.bg_stroke,
    );
    ui.painter().text(
        egui::pos2(rect.left() + 8.0, bottom + 15.0),
        egui::Align2::LEFT_CENTER,
        "Visual influence",
        egui::FontId::proportional(11.0),
        ui.visuals().text_color(),
    );
    let fader = Rect::from_min_max(
        egui::pos2(rect.left() + 8.0, bottom + 35.0),
        egui::pos2(rect.left() + 65.0, rect.bottom() - 30.0),
    );
    let mut fader_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(fader)
            .layout(egui::Layout::top_down(egui::Align::Center)),
    );
    fader_ui.spacing_mut().slider_width = 136.0;
    fader_ui.visuals_mut().selection.bg_fill = color;
    fader_ui.visuals_mut().widgets.active.bg_fill = color;
    fader_ui.visuals_mut().widgets.hovered.bg_fill = blend(color, egui::Color32::WHITE, 0.20);
    fader_ui.visuals_mut().widgets.inactive.fg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_gray(235));
    fader_ui.set_clip_rect(fader.intersect(ui.clip_rect()));
    fader_ui.add(
        egui::Slider::new(&mut d.influence, 0.0..=2.0)
            .vertical()
            .show_value(false),
    );
    let value_rect = Rect::from_min_size(
        egui::pos2(rect.left() + 8.0, rect.bottom() - 56.0),
        egui::vec2(56.0, 24.0),
    );
    let mut value_ui = ui.new_child(
        egui::UiBuilder::new()
            .id_salt("channel-influence-value")
            .max_rect(value_rect)
            .layout(egui::Layout::top_down(egui::Align::Center)),
    );
    value_ui.set_clip_rect(value_rect.intersect(ui.clip_rect()));
    value_ui.spacing_mut().interact_size.y = 22.0;
    value_ui.style_mut().override_font_id = Some(egui::FontId::proportional(11.0));
    value_ui.add_sized(
        [52.0, 22.0],
        egui::DragValue::new(&mut d.influence)
            .range(0.0..=2.0)
            .speed(0.01)
            .fixed_decimals(2),
    );
    for (i, value, label) in [(0, sample.level, "In"), (1, sample.value.abs(), "Out")] {
        let meter = Rect::from_min_size(
            egui::pos2(rect.left() + 80.0 + i as f32 * 23.0, bottom + 40.0),
            egui::vec2(9.0, 136.0),
        );
        let bg = ui.visuals().panel_fill;
        let track = egui::Color32::from_rgb(
            bg.r().saturating_sub(20),
            bg.g().saturating_sub(20),
            bg.b().saturating_sub(20),
        );
        ui.painter().rect_filled(meter, 3.0, track);
        ui.painter().rect_stroke(
            meter,
            1.0,
            egui::Stroke::new(0.5, ui.visuals().weak_text_color().gamma_multiply(0.25)),
            egui::StrokeKind::Inside,
        );
        let fill = Rect::from_min_max(
            egui::pos2(
                meter.left(),
                meter.bottom() - value.clamp(0.0, 1.0) * meter.height(),
            ),
            meter.max,
        );
        if value > 0.001 {
            ui.painter()
                .rect_filled(fill.expand(1.0), 3.0, color.gamma_multiply(0.18));
            ui.painter().rect_filled(fill, 2.0, color);
            ui.painter().hline(
                fill.x_range(),
                fill.top(),
                egui::Stroke::new(1.5, blend(color, egui::Color32::WHITE, 0.6)),
            );
        }
        ui.painter().text(
            egui::pos2(meter.center().x, meter.bottom() + 5.0),
            egui::Align2::CENTER_TOP,
            label,
            egui::FontId::proportional(10.0),
            ui.visuals().weak_text_color(),
        );
    }
    let mut footer = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(Rect::from_min_max(
                egui::pos2(rect.left() + 8.0, rect.bottom() - 28.0),
                egui::pos2(rect.right() - 8.0, rect.bottom() - 3.0),
            ))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    footer.set_clip_rect(footer.max_rect().intersect(ui.clip_rect()));
    footer.visuals_mut().selection.bg_fill = color;
    footer.visuals_mut().selection.stroke.color = dark;
    footer.spacing_mut().interact_size.y = 20.0;
    footer.spacing_mut().button_padding = egui::vec2(4.0, 1.0);
    footer.spacing_mut().item_spacing.x = 4.0;
    footer.style_mut().override_font_id = Some(egui::FontId::proportional(11.0));
    footer.label(
        egui::RichText::new(format!("{}", slot + 1))
            .color(color)
            .strong(),
    );
    if footer
        .add(egui::Button::selectable(
            d.enabled,
            egui::RichText::new(if d.enabled { "On" } else { "Muted" }).color(if d.enabled {
                dark
            } else {
                footer.visuals().text_color()
            }),
        ))
        .on_hover_text("Enable / mute visual influence")
        .clicked()
    {
        d.enabled = !d.enabled;
    }
    footer.label(format!("{:+.2}", sample.value));
    action
}

pub fn mapping_editor(ui: &mut egui::Ui, route: &mut Route, _kind: Visualizer) -> bool {
    let Some(shared) = ui.data(|d| d.get_temp::<Shared>(egui::Id::new(CONTEXT_KEY))) else {
        return false;
    };
    let mut panel = shared.lock().unwrap_or_else(|e| e.into_inner());
    // The one intentional local extractor is Strata's automatically distributed layer bank.
    let local = route.signal.is_none() && route.target.layered() && route.layer == 0;
    ui.label("Signal");
    let selected = route
        .signal
        .and_then(|id| panel.bank.channels.iter().find(|d| d.id == id))
        .map_or(
            if local {
                "Automatic layer bands"
            } else {
                "Choose a source"
            },
            |d| d.name.as_str(),
        );
    egui::ComboBox::from_id_salt(("named-signal", route.id))
        .selected_text(selected)
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            if route.target.layered() {
                ui.selectable_value(&mut route.signal, None, "Automatic layer bands");
            } else if ui
                .selectable_value(&mut route.signal, None, "No modulation")
                .clicked()
            {
                route.enabled = false;
            }
            for d in &panel.bank.channels {
                if ui
                    .selectable_value(&mut route.signal, Some(d.id), &d.name)
                    .clicked()
                {
                    route.enabled = true;
                }
            }
        });
    if route.signal.is_none() && route.target.layered() && route.layer == 0 {
        ui.small("Local band bank: bass at the bottom, treble at the top. Re-divides when layer count changes.");
        return false;
    }
    if route.signal.is_none() {
        route.enabled = false;
        ui.small("Create sources in Signals, then choose one here.");
    }
    ui.small(format!("Destination: {}", route.target.label()));
    if let Some(id) = route.signal {
        let sample = panel.snapshot.values.get(&id).cloned().unwrap_or_default();
        meter(
            ui,
            sample.value.abs().min(1.0),
            &format!("Output {:+.2}", sample.value),
        );
        ui.small(sample.status.label());
        ui.horizontal_wrapped(|ui| {
            if ui.button("Edit signal").clicked() {
                panel.selected = Some(id);
                panel.show_signals = true;
            }
        });
        ui.small(format!(
            "Shared by {} mappings. Depth below is local.",
            panel.consumers.get(&id).map_or(0, Vec::len)
        ));
    }
    ui.add(Parameter::new(&mut route.amount, -3.0..=3.0, 1.0).text("Depth"));
    ui.checkbox(&mut route.invert, "Invert polarity");
    ui.add(
        Parameter::new(&mut route.output_min, 0.0..=route.output_max, 0.0).text("Range minimum"),
    );
    ui.add(
        Parameter::new(&mut route.output_max, route.output_min..=1.0, 1.0).text("Range maximum"),
    );
    ui.small("Signals add bounded influence to the manual base. Click the source badge below its macro to edit detection and envelopes inline.");
    true
}

fn meter(ui: &mut egui::Ui, value: f32, label: &str) {
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 22.0), egui::Sense::hover());
    ui.painter()
        .rect_filled(rect, 2.0, ui.visuals().extreme_bg_color);
    let mut fill = rect;
    fill.max.x = fill.min.x + fill.width() * value.clamp(0.0, 1.0);
    ui.painter().rect_filled(
        fill,
        2.0,
        ui.visuals().selection.bg_fill.gamma_multiply(0.6),
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(11.0),
        ui.visuals().text_color(),
    );
}

pub(crate) fn lfo_editor(ui: &mut egui::Ui, lfo: &mut Lfo) {
    combo(
        ui,
        "waveform",
        "Waveform",
        &mut lfo.waveform,
        &Waveform::ALL,
        Waveform::label,
    );
    ui.add(Parameter::new(&mut lfo.rate, 0.01..=20.0, 0.25).text("Rate Hz"));
    ui.add(Parameter::new(&mut lfo.phase, 0.0..=1.0, 0.0).text("Phase"));
    ui.add(Parameter::new(&mut lfo.depth, 0.0..=1.0, 1.0).text("Depth"));
    ui.checkbox(&mut lfo.bipolar, "Bipolar · −1 to +1");
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 64.0), egui::Sense::hover());
    let points = (0..=100)
        .map(|i| {
            egui::pos2(
                rect.left() + rect.width() * i as f32 / 100.0,
                rect.center().y
                    - lfo.waveform.value(i as f64 / 100.0 + f64::from(lfo.phase))
                        * rect.height()
                        * 0.4
                        * lfo.depth,
            )
        })
        .collect();
    ui.painter().hline(
        rect.x_range(),
        rect.center().y,
        ui.visuals().widgets.noninteractive.bg_stroke,
    );
    ui.painter().add(egui::Shape::line(
        points,
        egui::Stroke::new(1.5, ui.visuals().selection.bg_fill),
    ));
    ui.small("Free-running · independent of audio · rate changes preserve phase");
}

#[cfg(test)]
mod tests {
    use super::*;
    fn test_panel() -> Panel {
        let mut panel = Panel::default();
        panel.replace_bank(Bank::test_sources());
        panel
    }
    #[test]
    fn range_learning_uses_changing_live_raw_samples_and_ignores_disconnections() {
        let mut learning = RangeLearning::new(1, 0.0);
        let mut snapshot = Snapshot {
            time: 1.0,
            values: [(
                1,
                Sample {
                    raw: 0.5,
                    value: 0.01,
                    status: Status::Live,
                    ..Default::default()
                },
            )]
            .into(),
        };
        learning.observe(&snapshot);
        let mut normalization = Normalization::default();
        assert!(!learning.apply(&mut normalization));
        snapshot.time = 2.0;
        snapshot.values.get_mut(&1).unwrap().raw = 0.55;
        learning.observe(&snapshot);
        snapshot.time = 3.0;
        snapshot.values.get_mut(&1).unwrap().status = Status::Stale;
        snapshot.values.get_mut(&1).unwrap().raw = 0.0;
        learning.observe(&snapshot);
        assert!(learning.apply(&mut normalization));
        assert_eq!(normalization.low, 0.5);
        assert_eq!(normalization.high, 0.55);
        assert!(normalization.enabled);
        assert!((normalization.map(0.525) - 0.5).abs() < 1e-5);
    }
    #[test]
    fn channel_slots_add_move_delete_and_undo_keep_the_same_signal_identity() {
        let mut panel = test_panel();
        panel.bank.sync_slots();
        let original = panel.bank.channels[0].clone();
        panel.act(StripAction::Insert(original.id, 5));
        assert_eq!(panel.bank.slots[4], Some(original.id));
        assert_eq!(panel.bank.slots[0], Some(2));
        panel.act(StripAction::Delete(original.id));
        assert_eq!(panel.bank.slots, vec![Some(2), Some(3), Some(4), Some(5)]);
        assert!(panel.bank.channels.iter().all(|d| d.id != original.id));
        panel.act(StripAction::Undo);
        assert_eq!(panel.bank.slots[4], Some(original.id));
        assert_eq!(
            panel.bank.channels.iter().find(|d| d.id == original.id),
            Some(&original)
        );
        panel.act(StripAction::AddAt(Method::BassHits, 4));
        let created = panel.selected.unwrap();
        assert_eq!(panel.bank.slots[4], Some(created));
        assert_ne!(created, original.id);
        assert!(panel.bank.valid());
        let saved: Bank =
            serde_json::from_slice(&serde_json::to_vec(&panel.bank).unwrap()).unwrap();
        assert_eq!(saved.slots, panel.bank.slots);
        assert!(saved.valid());
    }
    #[test]
    fn clipboard_cut_restores_identity_and_copy_and_duplicate_are_independent() {
        let mut panel = test_panel();
        panel.bank.sync_slots();
        panel.act(StripAction::Color(1, [240, 90, 60]));
        let original = panel.bank.channels[0].clone();
        panel.act(StripAction::Copy(1));
        panel.act(StripAction::Paste(2));
        let copy = panel.selected.unwrap();
        assert_ne!(copy, 1);
        assert_eq!(panel.bank.slots[2], Some(copy));
        let d = panel.bank.channels.iter().find(|d| d.id == copy).unwrap();
        assert_eq!(d.response, original.response);
        assert_eq!(d.color_rgb(), original.color_rgb());
        panel.act(StripAction::Cut(1));
        assert!(panel.bank.channels.iter().all(|d| d.id != 1));
        panel.act(StripAction::Paste(4));
        assert_eq!(panel.selected, Some(1));
        assert_eq!(panel.bank.slots[4], Some(1));
        assert!(panel.deleted.is_none());
        assert_eq!(
            panel.bank.channels.iter().find(|d| d.id == 1),
            Some(&original)
        );
        panel.act(StripAction::Duplicate(1, 4));
        let duplicate = panel.selected.unwrap();
        assert_ne!(duplicate, 1);
        assert_eq!(panel.bank.slots[4], Some(duplicate));
        panel.act(StripAction::Color(duplicate, [30, 70, 220]));
        assert_eq!(
            panel
                .bank
                .channels
                .iter()
                .find(|d| d.id == 1)
                .unwrap()
                .color_rgb(),
            [240, 90, 60]
        );
        let restored: Bank =
            serde_json::from_slice(&serde_json::to_vec(&panel.bank).unwrap()).unwrap();
        assert_eq!(restored, panel.bank);
        assert!(restored.valid());
        panel.act(StripAction::AddAt(Method::Lfo, 1));
        let id = panel.selected.unwrap();
        assert_eq!(panel.bank.slots[1], Some(id));
        assert_eq!(
            panel
                .bank
                .channels
                .iter()
                .find(|d| d.id == id)
                .unwrap()
                .input,
            Input::Lfo
        );
        panel.act(StripAction::Copy(id));
        panel.replace_bank(Bank::default());
        assert!(panel.clipboard.is_none());
        assert!(panel.bank.valid());
    }
    #[test]
    fn context_menu_opens_from_header_and_body_without_changing_signals() {
        for position in [egui::pos2(60.0, 22.0), egui::pos2(60.0, 250.0)] {
            let ctx = egui::Context::default();
            let mut panel = test_panel();
            panel.bank.sync_slots();
            let original = panel.bank.clone();
            let mut render = |events| {
                let mut out = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(1200.0, 800.0),
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
            render(vec![]);
            render(vec![egui::Event::PointerMoved(position)]);
            for pressed in [true, false] {
                render(vec![egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Secondary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                }]);
            }
            let out = render(vec![]);
            for label in [
                "Cut",
                "Copy",
                "Paste",
                "Duplicate",
                "Delete",
                "Rename",
                "Insert new channel",
                "Channel color",
            ] {
                assert!(
                    out.shapes.iter().any(
                        |s| matches!(&s.shape,egui::Shape::Text(t) if t.galley.job.text==label)
                    ),
                    "missing {label} at {position:?}"
                );
            }
            assert_eq!(panel.bank, original);
        }
    }
    #[test]
    fn vacant_project_slots_collapse_and_blank_workspace_can_create_a_channel() {
        fn render(
            ctx: &egui::Context,
            panel: &mut Panel,
            events: Vec<egui::Event>,
        ) -> egui::FullOutput {
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1200.0, 800.0),
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
        }
        fn text_position(out: &egui::FullOutput, label: &str) -> egui::Pos2 {
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
        fn click(
            ctx: &egui::Context,
            panel: &mut Panel,
            pos: egui::Pos2,
            button: egui::PointerButton,
        ) -> egui::FullOutput {
            render(ctx, panel, vec![egui::Event::PointerMoved(pos)]);
            for pressed in [true, false] {
                render(
                    ctx,
                    panel,
                    vec![egui::Event::PointerButton {
                        pos,
                        button,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    }],
                );
            }
            render(ctx, panel, vec![])
        }
        for count in [0, 2] {
            let ctx = egui::Context::default();
            let mut panel = test_panel();
            panel.bank.channels.truncate(count);
            let original = panel.bank.channels.clone();
            panel.bank.slots = if count == 0 {
                vec![None; 8]
            } else {
                vec![None, Some(2), None, Some(1), None, None, None, None]
            };
            let out = render(&ctx, &mut panel, vec![]);
            assert_eq!(panel.bank.slots.len(), count);
            if count == 2 {
                assert_eq!(panel.bank.slots, vec![Some(2), Some(1)]);
                text_position(&out, "1 Band energy");
                text_position(&out, "2 Overall intensity");
            }
            assert!(
                !out.shapes
                    .iter()
                    .any(|s| matches!(&s.shape, egui::Shape::Text(t)
                if t.galley.job.text.contains("Empty") || t.galley.job.text == "+ Add channel"))
            );
            let out = click(
                &ctx,
                &mut panel,
                egui::pos2(750.0, 220.0),
                egui::PointerButton::Secondary,
            );
            assert_eq!(panel.bank.channels, original);
            let out = click(
                &ctx,
                &mut panel,
                text_position(&out, "Add channel"),
                egui::PointerButton::Primary,
            );
            click(
                &ctx,
                &mut panel,
                text_position(&out, "LFO"),
                egui::PointerButton::Primary,
            );
            assert_eq!(panel.bank.channels.len(), count + 1);
            let id = panel.selected.unwrap();
            assert_eq!(panel.bank.slots.last(), Some(&Some(id)));
            assert_eq!(panel.bank.channels.last().unwrap().method, Method::Lfo);
            assert!(panel.bank.valid());
        }
    }

    #[test]
    fn rename_commits_on_enter_and_cancel_leaves_the_definition_untouched() {
        let ctx = egui::Context::default();
        let mut panel = test_panel();
        panel.act(StripAction::Rename(1));
        let render = |panel: &mut Panel, key: Option<egui::Key>| {
            let mut out = ctx.run_ui(
                egui::RawInput {
                    events: key
                        .map(|key| {
                            vec![egui::Event::Key {
                                key,
                                physical_key: None,
                                pressed: true,
                                repeat: false,
                                modifiers: egui::Modifiers::NONE,
                            }]
                        })
                        .unwrap_or_default(),
                    ..Default::default()
                },
                |ui| panel.rename_dialog(ui.ctx()),
            );
            out.textures_delta.clear();
        };
        render(&mut panel, None);
        panel.rename.as_mut().unwrap().1 = "  Kick pulse  ".into();
        render(&mut panel, Some(egui::Key::Enter));
        assert_eq!(panel.bank.channels[0].name, "Kick pulse");
        assert!(panel.rename.is_none());
        panel.act(StripAction::Rename(1));
        render(&mut panel, None);
        panel.rename.as_mut().unwrap().1 = "Discarded".into();
        render(&mut panel, Some(egui::Key::Escape));
        assert_eq!(panel.bank.channels[0].name, "Kick pulse");
        assert!(panel.rename.is_none());
    }

    #[test]
    fn dragged_header_supplies_a_real_channel_payload() {
        let ctx = egui::Context::default();
        let mut definition = Definition::recipe(42, Method::Band);
        let mut draw = |events| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(600.0, 700.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    channel_strip(
                        ui,
                        Rect::from_min_size(egui::pos2(20.0, 20.0), egui::vec2(156.0, 600.0)),
                        0,
                        &mut definition,
                        &Sample::default(),
                        true,
                        0,
                        None,
                    );
                },
            );
            output.textures_delta.clear();
        };
        let start = egui::pos2(60.0, 34.0);
        let end = egui::pos2(260.0, 34.0);
        draw(vec![]);
        draw(vec![
            egui::Event::PointerMoved(start),
            egui::Event::PointerButton {
                pos: start,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        draw(vec![egui::Event::PointerMoved(end)]);
        assert_eq!(
            egui::DragAndDrop::payload::<ChannelDrag>(&ctx).unwrap().0,
            42
        );
    }

    #[test]
    fn drag_insertion_line_matches_drop_order_in_both_directions() {
        let ctx = egui::Context::default();
        let mut panel = test_panel();
        let original = panel.bank.channels.clone();
        let render = |panel: &mut Panel, events| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1200.0, 800.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    panel.show(
                        ui,
                        ui.available_rect_before_wrap(),
                        &AnalysisFrame::default(),
                        true,
                        &AppTheme::default(),
                    );
                },
            );
            output.textures_delta.clear();
            output
        };
        let out = render(&mut panel, vec![]);
        let left = out
            .shapes
            .iter()
            .find_map(|s| {
                if let egui::Shape::Text(t) = &s.shape {
                    (t.galley.job.text == "1 Overall intensity").then_some(t.pos.x - 8.0)
                } else {
                    None
                }
            })
            .unwrap();
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let drag = |panel: &mut Panel, from: egui::Pos2, to: egui::Pos2, line_x: f32| {
            render(panel, vec![egui::Event::PointerMoved(from)]);
            render(panel, vec![button(from, true)]);
            let preview = render(panel, vec![egui::Event::PointerMoved(to)]);
            assert!(preview.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::LineSegment {points,stroke}
                if (stroke.width-1.5).abs()<0.01 && (points[0].x-line_x).abs()<0.01 && points[0].x==points[1].x && points[1].y-points[0].y>400.0)),"missing insertion line at {line_x}");
            render(panel, vec![button(to, false)]);
        };
        drag(
            &mut panel,
            egui::pos2(left + 40.0, 22.0),
            egui::pos2(left + 280.0, 90.0),
            left + 312.0,
        );
        assert_eq!(
            &panel.bank.slots[..5],
            &[Some(2), Some(1), Some(3), Some(4), Some(5)]
        );
        for d in &original {
            assert_eq!(panel.bank.channels.iter().find(|v| v.id == d.id), Some(d));
        }
        drag(
            &mut panel,
            egui::pos2(left + 196.0, 22.0),
            egui::pos2(left + 20.0, 90.0),
            left,
        );
        assert_eq!(
            &panel.bank.slots[..5],
            &[Some(1), Some(2), Some(3), Some(4), Some(5)]
        );
        assert_eq!(panel.bank.channels, original);
        // The blank workspace is an append target, with its guide at the last track edge.
        drag(
            &mut panel,
            egui::pos2(left + 40.0, 22.0),
            egui::pos2(left + 810.0, 90.0),
            left + 780.0,
        );
        assert_eq!(
            panel.bank.slots,
            vec![Some(2), Some(3), Some(4), Some(5), Some(1)]
        );
        for d in &original {
            assert_eq!(panel.bank.channels.iter().find(|v| v.id == d.id), Some(d));
        }
    }

    #[test]
    fn mixer_and_graphical_inspectors_render_at_small_and_large_ui_scales() {
        let ctx = egui::Context::default();
        let mut panel = test_panel();
        for method in Method::ALL {
            panel.selected = panel
                .bank
                .channels
                .iter()
                .find(|d| d.method == method)
                .map(|d| d.id);
            for (section, size) in SignalSection::ALL.into_iter().flat_map(|section| {
                [
                    egui::vec2(512.0, 300.0),
                    egui::vec2(1024.0, 650.0),
                    egui::vec2(1800.0, 1000.0),
                ]
                .map(|size| (section, size))
            }) {
                if let Some(id) = panel.selected {
                    panel.sections.insert(id, section);
                }
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, size)),
                        ..Default::default()
                    },
                    |ui| {
                        panel.show(
                            ui,
                            ui.available_rect_before_wrap(),
                            &AnalysisFrame::default(),
                            true,
                            &AppTheme::default(),
                        );
                    },
                );
                assert!(panel.bank.valid());
                let meshes =
                    ctx.tessellate(std::mem::take(&mut output.shapes), output.pixels_per_point);
                for mesh in meshes {
                    if let egui::epaint::Primitive::Mesh(m) = mesh.primitive {
                        assert!(m.vertices.iter().all(|v| v.pos.is_finite()));
                    }
                }
                output.textures_delta.clear();
            }
        }
    }
}
