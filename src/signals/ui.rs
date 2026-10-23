use super::*;
use crate::help::HoverHelp;
use crate::{
    icons::{self, Icon},
    modulation::{
        Visualizer,
        ui::{combo, envelope_editor, spectrum_editor},
    },
    modules::{property_row, settings_group},
    parameter::Parameter,
    theme::AppTheme,
    workspace_ui,
};
use egui::{self, Rect};
use std::sync::Mutex;
pub(crate) mod audio_channel;
mod history;
mod inserts;
mod master;
mod midi_preview;
mod processor;
/// UI identity for the permanent capture, never serialized as a scene channel.
pub(crate) const MASTER_ID: u64 = 0;

pub type Shared = Arc<Mutex<Panel>>;
pub const MIN_DOCK_HEIGHT: f32 = 320.0;
pub const CONTEXT_KEY: &str = "shared-musical-signals";
pub struct Panel {
    pub bank: Bank,
    pub snapshot: Arc<Snapshot>,
    pub midi_snapshot: Arc<crate::midi::Snapshot>,
    midi_learning: Option<(u64, u64)>,
    pub selected: Option<u64>,
    reveal_selected: bool,
    pub(crate) selected_insert: Option<(u64, u64)>,
    pub show_signals: bool,
    pub inspect_requested: bool,
    pub master_settings_requested: bool,
    pub consumers: BTreeMap<u64, Vec<String>>,
    span: Option<f32>,
    deleted: Option<(Definition, usize)>,
    deleted_audio: Option<(audio::InputBus, usize)>,
    history: BTreeMap<u64, history::History>,
    raw_history: BTreeMap<u64, history::History>,
    processor: Option<u64>,
    processor_section: SignalSection,
    midi_history: BTreeMap<u64, midi_preview::History>,
    last_history_time: f64,
    clipboard: Option<(Definition, bool)>,
    rename: Option<(u64, String, bool)>,
    learning: Option<RangeLearning>,
    learning_notice: Option<(u64, String)>,
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
    fn finish(&self, normalization: &mut Normalization) -> String {
        if self.apply(normalization) {
            format!(
                "Learned {:.3} – {:.3}; now mapped to 0–1.",
                normalization.low, normalization.high
            )
        } else {
            "No changing live signal detected; range retained.".into()
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
            midi_snapshot: Arc::new(crate::midi::Snapshot::default()),
            midi_learning: None,
            selected: None,
            reveal_selected: false,
            selected_insert: None,
            show_signals: false,
            inspect_requested: false,
            master_settings_requested: false,
            consumers: BTreeMap::new(),
            span: None,
            deleted: None,
            deleted_audio: None,
            history: Default::default(),
            raw_history: Default::default(),
            processor: None,
            processor_section: SignalSection::Output,
            midi_history: Default::default(),
            last_history_time: 0.0,
            clipboard: None,
            rename: None,
            learning: None,
            learning_notice: None,
            close_inspector: false,
            inspector_width: 236.0,
        }
    }
}
impl Panel {
    pub fn learning_range(&self) -> bool {
        self.learning.is_some()
    }
    pub fn advance_learning(&mut self) {
        if let Some(capture) = &mut self.learning {
            capture.observe(&self.snapshot);
            if self.snapshot.time - capture.started >= 5.0 {
                if let Some(d) = self.bank.channels.iter_mut().find(|d| d.id == capture.id) {
                    self.learning_notice = Some((capture.id, capture.finish(&mut d.normalization)));
                }
                self.learning = None;
            }
        }
    }

    fn observe_history(&mut self) {
        if self.snapshot.time != self.last_history_time {
            self.last_history_time = self.snapshot.time;
            self.history
                .retain(|id, _| self.bank.channels.iter().any(|d| d.id == *id));
            self.raw_history
                .retain(|id, _| self.bank.channels.iter().any(|d| d.id == *id));
            for d in &self.bank.channels {
                self.raw_history.entry(d.id).or_default().push(
                    self.snapshot.time,
                    self.snapshot.values.get(&d.id).map_or(0.0, |s| s.raw),
                );
                let history = self.history.entry(d.id).or_default();
                history.push(
                    self.snapshot.time,
                    self.snapshot.values.get(&d.id).map_or(0.0, |s| s.value),
                );
            }
        }
    }

    pub fn replace_bank(&mut self, mut bank: Bank) {
        bank.migrate_inserts();
        self.selected = bank.channels.first().map(|d| d.id);
        self.bank = bank;
        self.selected_insert = None;
        self.reveal_selected = false;
        self.master_settings_requested = false;
        self.clipboard = None;
        self.snapshot = Arc::new(Snapshot::default());
        self.last_history_time = 0.0;
        self.rename = None;
        self.learning = None;
        self.learning_notice = None;
        self.midi_learning = None;
        self.deleted = None;
        self.deleted_audio = None;
        self.history.clear();
        self.raw_history.clear();
        self.processor = None;
        self.midi_history.clear();
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
        let mut page = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("signals-page")
                .max_rect(rect),
        );
        page.set_clip_rect(rect.intersect(ui.clip_rect()));
        workspace_ui::sidebar_style(&mut page, theme);
        let ui = &mut page;
        self.bank.sync_slots();
        self.observe_history();
        self.midi_history.retain(|id, _| {
            self.bank
                .channels
                .iter()
                .any(|d| d.id == *id && d.input == Input::Midi)
        });
        for definition in self.bank.channels.iter().filter(|d| d.input == Input::Midi) {
            self.midi_history
                .entry(definition.id)
                .or_insert_with(|| {
                    midi_preview::History::new(&definition.midi, self.midi_snapshot.serial)
                })
                .update(&definition.midi, &self.midi_snapshot, self.snapshot.time);
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
        // Keep the dock edges aligned with the visual panes above it.
        let mixer = mixer.shrink2(egui::vec2(0.0, 8.0));
        ui.painter().rect_filled(mixer, 7.0, theme.navigation);
        let content = mixer.shrink(1.0);
        let (tracks, master) = master::layout(content);
        let mut board = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("signals-mixer")
                .max_rect(tracks)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        board.set_clip_rect(tracks.intersect(ui.clip_rect()));
        let mut action = None;
        board.spacing_mut().item_spacing.x = 0.0;
        let slots = self.bank.slots.clone();
        let reveal_selected = std::mem::take(&mut self.reveal_selected);
        let mut scroll = egui::ScrollArea::horizontal().id_salt("signal-strips");
        if reveal_selected && let Some(index) = slots.iter().position(|id| *id == self.selected) {
            scroll = scroll.horizontal_scroll_offset(workspace_ui::scroll_offset_for_strip(
                &board,
                "signal-strips",
                index,
                156.0,
            ));
        }
        let mixer_output = scroll.auto_shrink([false, false]).show(&mut board, |ui| {
            let height = ui.available_height();
            let viewport_right = ui.max_rect().right();
            let mut insertion_line = None;
            ui.horizontal_top(|ui| {
                for (slot, id) in slots.iter().copied().enumerate() {
                    ui.push_id(("channel-slot", slot), |ui| {
                        let mut over_inserts = false;
                        let (strip, _) =
                            ui.allocate_exact_size(egui::vec2(156.0, height), egui::Sense::hover());
                        if let Some(id) = id
                            && self
                                .bank
                                .channels
                                .iter()
                                .any(|d| d.id == id && d.inserts.is_some())
                        {
                            let d = self.bank.channels.iter_mut().find(|d| d.id == id).unwrap();
                            let bus = self.bank.audio_inputs.iter_mut().find(|b| b.id == id);
                            let output = inserts::strip(
                                ui,
                                strip,
                                slot,
                                d,
                                bus,
                                &self.snapshot,
                                self.selected == Some(id),
                                self.selected_insert,
                                self.history.get(&id),
                                (&self.midi_snapshot, self.midi_history.get(&id)),
                            );
                            over_inserts = output.over_inserts;
                            if let Some(a) = output.action {
                                action = Some(a);
                            }
                        } else {
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
                                    history,
                                    Some((
                                        &self.midi_snapshot,
                                        self.midi_history.get(&id),
                                        self.snapshot.time,
                                    )),
                                ) {
                                    action = Some(a);
                                }
                            } else if let Some(bus) = id.and_then(|id| {
                                self.bank.audio_inputs.iter_mut().find(|b| b.id == id)
                            }) {
                                let feed =
                                    self.snapshot.audio.get(&bus.id).map(|f| f.frame.as_ref());
                                if let Some(a) = audio_channel::strip(
                                    ui,
                                    strip,
                                    slot,
                                    bus,
                                    feed,
                                    self.selected == id,
                                ) {
                                    action = Some(a);
                                }
                            }
                        }
                        if let Some(id) = id {
                            let title = Rect::from_min_max(
                                strip.min,
                                egui::pos2(strip.right() - 24.0, strip.top() + 28.0),
                            );
                            self.rename_field(ui, title, id);
                        }
                        if ui.rect_contains_pointer(strip)
                            && ui.input(|i| i.pointer.primary_pressed())
                        {
                            self.selected = id;
                            if !over_inserts {
                                self.selected_insert = None;
                            }
                            self.inspect_requested = true;
                        }
                        let menu_response =
                            ui.interact(strip, ui.id().with("channel-menu"), egui::Sense::hover());
                        let open_menu = ui.rect_contains_pointer(strip)
                            && !over_inserts
                            && ui.input(|i| i.pointer.secondary_clicked());
                        if open_menu && let Some(id) = id {
                            self.selected = Some(id);
                            self.selected_insert = None;
                            self.inspect_requested = true;
                        }
                        let definition =
                            id.and_then(|id| self.bank.channels.iter().find(|d| d.id == id));
                        crate::dropdown::styled_popup(
                            ui,
                            egui::Popup::context_menu(&menu_response),
                        )
                        .open_memory(open_menu.then_some(egui::SetOpenCommand::Bool(true)))
                        .show(|ui| {
                            if definition.is_none()
                                && let Some(bus) = id.and_then(|id| {
                                    self.bank.audio_inputs.iter().find(|b| b.id == id)
                                })
                            {
                                if let Some(a) = audio_channel::menu(
                                    ui,
                                    bus,
                                    slot,
                                    self.bank.audio_inputs.len() < audio::MAX_INPUTS,
                                ) {
                                    action = Some(a);
                                }
                            } else if let Some(menu_action) = channel_menu(
                                ui,
                                definition,
                                slot,
                                self.can_paste(),
                                self.bank.channels.len() < MAX_SIGNALS,
                                self.deleted.is_some() || self.deleted_audio.is_some(),
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
                // Fill only the remaining viewport; adding an offscreen blank
                // track's worth of space would force an unnecessary scrollbar.
                let free_width = (viewport_right - ui.next_widget_position().x - 1.0).max(0.0);
                let (space, response) =
                    ui.allocate_exact_size(egui::vec2(free_width, height), egui::Sense::hover());
                if slots.is_empty() {
                    ui.painter().text(
                        space.center(),
                        egui::Align2::CENTER_CENTER,
                        "Right-click to add a channel",
                        egui::FontId::proportional(13.0),
                        ui.visuals().weak_text_color(),
                    );
                }
                let open_menu =
                    ui.rect_contains_pointer(space) && ui.input(|i| i.pointer.secondary_clicked());
                crate::dropdown::styled_popup(ui, egui::Popup::context_menu(&response))
                    .open_memory(open_menu.then_some(egui::SetOpenCommand::Bool(true)))
                    .show(|ui| {
                        if let Some(menu_action) = channel_menu(
                            ui,
                            None,
                            slots.len(),
                            self.can_paste(),
                            self.bank.channels.len() < MAX_SIGNALS,
                            self.deleted.is_some() || self.deleted_audio.is_some(),
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
        debug_assert!(mixer_output.content_size.y <= mixer_output.inner_rect.height() + 1.0);
        debug_assert!(
            slots.len() as f32 * 156.0 > mixer_output.inner_rect.width()
                || mixer_output.content_size.x <= mixer_output.inner_rect.width()
        );
        let (selected, settings) = master::strip(
            ui,
            master,
            frame,
            self.selected == Some(MASTER_ID),
            theme.accent,
        );
        if selected {
            self.act(StripAction::Select(MASTER_ID));
        }
        self.master_settings_requested |= settings;
        // egui's scissor is rectangular. Mask the track and scrollbar corners after
        // drawing, then paint the shared border above them for a continuous edge.
        workspace_ui::rounded_content_corners(ui, content, 6.0, theme.navigation);
        ui.painter().rect_stroke(
            mixer,
            7.0,
            egui::Stroke::new(1.0, ui.visuals().text_color().gamma_multiply(0.22)),
            egui::StrokeKind::Inside,
        );
        if let Some(action) = action {
            self.act(action);
        }
        self.bank.sync_audio_channels();
        if inspector_open {
            self.show_inspector(ui, inspector, frame, theme);
        }
    }

    pub fn show_inspector(
        &mut self,
        ui: &mut egui::Ui,
        inspector: Rect,
        frame: &AnalysisFrame,
        theme: &AppTheme,
    ) {
        self.close_inspector = false;
        if self.selected == Some(MASTER_ID) {
            let (close, settings) = master::inspector(ui, inspector, frame, theme);
            self.close_inspector = close;
            self.master_settings_requested |= settings;
            return;
        }
        if let Some(id) = self.selected
            && self.has_insert_channel(id)
        {
            self.chain_inspector(ui, inspector, theme, id);
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
        workspace_ui::inspector_style(&mut editor, theme);
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
                    let input_detail = match d.input {
                        Input::Midi => "MIDI input".into(),
                        Input::Lfo => "Free-running".into(),
                        _ => format!("{:.0} kHz", frame.sample_rate as f32 / 1000.0),
                    };
                    let metadata = format!("Channel {index} · {} · {input_detail}", status.label());
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
        editor.data_mut(|d| {
            d.insert_temp(
                egui::Id::new("parameter-context"),
                format!("Signal · {title}"),
            );
        });
        workspace_ui::inspector_scroll(
            &mut editor,
            ("signal-detail-scroll", self.selected),
            |ui| {
                if let Some(id) = self.selected {
                    for section in SignalSection::ALL {
                        // Re-read the input after Source: changing it reveals the
                        // appropriate controls during this same frame.
                        let input = self
                            .bank
                            .channels
                            .iter()
                            .find(|d| d.id == id)
                            .map(|d| d.input);
                        if (input == Some(Input::Midi)
                            && matches!(
                                section,
                                SignalSection::Detection | SignalSection::Response
                            ))
                            || (input == Some(Input::Lfo) && section == SignalSection::Response)
                        {
                            continue;
                        }
                        ui.push_id((id, section), |ui| self.configure(ui, frame, id, section));
                    }
                    crate::modules::inspector_rule(ui);
                    self.editor_actions(ui);
                } else {
                    ui.weak("Select a signal channel to configure it.");
                }
            },
        );
        editor.data_mut(|d| {
            d.remove::<String>(egui::Id::new("parameter-context"));
        });
    }

    /// Follow mixer order, including the fixed input at the right-hand end.
    pub(crate) fn select_relative(&mut self, forward: bool) {
        let order: Vec<_> = self
            .bank
            .slots
            .iter()
            .flatten()
            .copied()
            .chain(std::iter::once(MASTER_ID))
            .collect();
        let Some(index) = self
            .selected
            .and_then(|id| order.iter().position(|value| *value == id))
        else {
            return;
        };
        let next = if forward {
            (index + 1).min(order.len() - 1)
        } else {
            index.saturating_sub(1)
        };
        self.act(StripAction::Select(order[next]));
        self.reveal_selected = true;
    }

    pub(crate) fn delete_selection(&mut self) {
        let Some(id) = self.selected else {
            return;
        };
        if id == MASTER_ID {
            return;
        }
        if let Some((channel, insert)) = self.selected_insert.filter(|(channel, _)| *channel == id)
        {
            // An expired insert selection must never fall through to deleting its channel.
            self.act(StripAction::RemoveEffect(channel, insert));
        } else {
            self.act(StripAction::Delete(id));
        }
    }

    fn act(&mut self, action: StripAction) {
        match action {
            StripAction::InspectEffect(id, insert) => {
                self.selected = Some(id);
                self.selected_insert = Some((id, insert));
                self.inspect_requested = true;
            }
            StripAction::AddEffect(id, index, effect) => {
                if let Some(d) = self.bank.channels.iter_mut().find(|d| d.id == id)
                    && let Some(chain) = &mut d.inserts
                    && let Some(insert) = chain.add(d.input, index, effect)
                {
                    self.selected = Some(id);
                    self.selected_insert = Some((id, insert));
                    self.inspect_requested = true;
                }
            }
            StripAction::MoveEffect(id, insert, boundary) => {
                if let Some(d) = self.bank.channels.iter_mut().find(|d| d.id == id)
                    && let Some(chain) = &mut d.inserts
                {
                    chain.move_to(d.input, insert, boundary);
                }
            }
            StripAction::RemoveEffect(id, insert) => {
                if let Some(d) = self.bank.channels.iter_mut().find(|d| d.id == id)
                    && let Some(chain) = &mut d.inserts
                {
                    chain.remove(insert);
                }
                self.selected_insert = None;
            }
            StripAction::BypassEffect(id, insert) => {
                if let Some(d) = self.bank.channels.iter_mut().find(|d| d.id == id)
                    && let Some(chain) = &mut d.inserts
                    && let Some(stage) = chain.stages.iter_mut().find(|s| s.id == insert)
                {
                    stage.enabled = !stage.enabled;
                }
            }
            StripAction::Processor(id) => {
                if self.has_insert_channel(id) {
                    self.selected = Some(id);
                    self.selected_insert = None;
                    self.inspect_requested = true;
                    return;
                }
                self.processor = Some(id);
                self.processor_section = SignalSection::Output;
            }
            StripAction::Select(id) => {
                self.selected_insert = None;
                self.selected = Some(id);
                self.inspect_requested = true;
            }
            StripAction::Delete(id) => {
                if id == MASTER_ID {
                    return;
                }
                if self
                    .selected_insert
                    .is_some_and(|(channel, _)| channel == id)
                {
                    self.selected_insert = None;
                }
                if self.rename.as_ref().is_some_and(|(v, _, _)| *v == id) {
                    self.rename = None;
                }
                if self.bank.channels.iter().any(|d| d.id == id) {
                    self.deleted = self.bank.remove_channel(id);
                    self.deleted_audio = None;
                } else if self.bank.audio_inputs.iter().any(|b| b.id == id) {
                    self.deleted_audio = self.bank.remove_audio_input(id);
                    self.deleted = None;
                } else {
                    self.deleted = self.bank.remove_channel(id);
                    self.deleted_audio = None;
                }
                if self.selected == Some(id) {
                    self.selected = self.bank.slots.first().copied().flatten();
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
                if let Some(copy) = self
                    .bank
                    .duplicate(id)
                    .or_else(|| self.bank.duplicate_audio_input(id))
                {
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
                if let Some(name) = self
                    .bank
                    .channels
                    .iter()
                    .find(|d| d.id == id)
                    .map(|d| d.name.clone())
                    .or_else(|| {
                        self.bank
                            .audio_inputs
                            .iter()
                            .find(|b| b.id == id)
                            .map(|b| b.name.clone())
                    })
                {
                    self.rename = Some((id, name, true));
                    self.selected = Some(id);
                    self.selected_insert = None;
                    self.inspect_requested = true;
                }
            }
            StripAction::Color(id, color) => {
                if let Some(d) = self.bank.channels.iter_mut().find(|d| d.id == id) {
                    d.color = Some(color);
                }
                if let Some(b) = self.bank.audio_inputs.iter_mut().find(|b| b.id == id) {
                    b.color = Some(color);
                }
            }
            StripAction::AddAudioAt(boundary) => {
                if let Some(id) = self.bank.add_source(Input::System) {
                    self.bank.sync_slots();
                    self.bank
                        .insert_channel(id, boundary.min(self.bank.slots.len()));
                    self.selected = Some(id);
                    self.inspect_requested = true;
                }
            }
            StripAction::AddAt(method, boundary) => {
                let id = if matches!(method, Method::Lfo | Method::Midi) {
                    self.bank.add_source(if method == Method::Lfo {
                        Input::Lfo
                    } else {
                        Input::Midi
                    })
                } else {
                    self.bank.add(method)
                };
                if let Some(id) = id {
                    self.bank.sync_slots();
                    self.bank
                        .insert_channel(id, boundary.min(self.bank.slots.len()));
                    self.selected = Some(id);
                }
            }
            StripAction::Undo => {
                if let Some((bus, slot)) = self.deleted_audio.take() {
                    if self.bank.audio_inputs.len() < audio::MAX_INPUTS {
                        let id = bus.id;
                        self.bank.audio_inputs.push(bus);
                        self.bank.sync_slots();
                        self.bank
                            .insert_channel(id, slot.min(self.bank.slots.len()));
                        self.selected = Some(id);
                    } else {
                        self.deleted_audio = Some((bus, slot));
                    }
                }
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
    fn rename_field(&mut self, ui: &mut egui::Ui, rect: Rect, target: u64) {
        if !self.rename.as_ref().is_some_and(|(id, _, _)| *id == target) {
            return;
        }
        let color = self
            .bank
            .channels
            .iter()
            .find(|d| d.id == target)
            .map(channel_color)
            .or_else(|| {
                self.bank
                    .audio_inputs
                    .iter()
                    .find(|bus| bus.id == target)
                    .map(|bus| {
                        let [r, g, b] = bus.color_rgb();
                        egui::Color32::from_rgb(r, g, b)
                    })
            })
            .unwrap_or(ui.visuals().panel_fill);
        let Some((id, name, focus)) = self.rename.as_mut().filter(|(id, _, _)| *id == target)
        else {
            return;
        };
        let Some(commit) = workspace_ui::inline_strip_name(ui, rect, name, focus, color) else {
            return;
        };
        if commit {
            if let Some(d) = self.bank.channels.iter_mut().find(|d| d.id == *id) {
                d.name = name.trim().into();
            }
            if let Some(b) = self.bank.audio_inputs.iter_mut().find(|b| b.id == *id) {
                b.name = name.trim().into();
            }
        }
        self.rename = None;
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
            if (self.deleted.is_some() || self.deleted_audio.is_some())
                && ui.small_button("Undo delete").clicked()
            {
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
                if d.input == Input::System {
                    property_row(ui, "Audio feed", |ui| {
                        audio::connection_picker(ui, &mut d.audio_input, &self.bank.audio_inputs);
                    });
                }
                property_row(ui, "Name", |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut d.name)
                            .desired_width(ui.available_width())
                            .char_limit(80),
                    );
                });
                property_row(ui, "Input", |ui| {
                    crate::dropdown::ComboBox::from_id_salt("signal-input")
                        .width(ui.available_width())
                        .truncate()
                        .selected_text(d.input.label())
                        .show_ui(ui, |ui| input_options(ui, d));
                });
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
                } else {
                    let previous = d.method;
                    let methods: Vec<_> = Method::ALL
                        .into_iter()
                        .filter(|m| *m != Method::Midi)
                        .collect();
                    combo(
                        ui,
                        "signal-method",
                        "Detector",
                        &mut d.method,
                        &methods,
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
                }
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
                                .help_text(crate::help::control_description("Adaptive"));
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
                if self.processor != Some(id) && ui.small_button("Open signal processor…").clicked()
                {
                    self.processor = Some(id);
                }
                normalization_editor(
                    ui,
                    d,
                    &sample,
                    &self.snapshot,
                    &mut self.learning,
                    &mut self.learning_notice,
                )
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

    const ALL: [Self; 5] = [
        Self::Source,
        Self::Detection,
        Self::Response,
        Self::Output,
        Self::Routing,
    ];
}

fn normalization_editor(
    ui: &mut egui::Ui,
    d: &mut Definition,
    sample: &Sample,
    snapshot: &Snapshot,
    learning: &mut Option<RangeLearning>,
    notice: &mut Option<(u64, String)>,
) {
    ui.horizontal_wrapped(|ui| {
        for (label, value) in [
            ("Raw", sample.raw),
            ("Range", d.normalization.map(sample.raw)),
            ("Final", sample.value),
        ] {
            ui.group(|ui| {
                ui.small(label);
                ui.strong(format!("{value:.3}"));
            });
        }
    });
    ui.weak("Detector > Range > Shape > Influence > Smooth");
    settings_group(ui, "Normalization", |ui| {
        ui.checkbox(&mut d.normalization.enabled,"Normalize output")
        .help_text("Stretch a narrow detector/envelope range to 0–1 before the influence knob. Values outside the input limits are clamped.");
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
                    *notice = None;
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
            *notice = Some((d.id, capture.finish(&mut d.normalization)));
        }
        if let Some((_, message)) = notice.as_ref().filter(|(id, _)| *id == d.id) {
            ui.weak(message);
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
            let signed = d.kind() == OutputKind::Bipolar;
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
    settings_group(ui, "Output shape", |ui| {
        ui.checkbox(&mut d.shaping.enabled,"Shape output").help_text("Change the motion curve after range normalization. Bypass preserves the original signal. These controls affect every parameter mapped to this channel.");
        ui.add_enabled_ui(d.shaping.enabled,|ui|{
            ui.add(Parameter::new(&mut d.shaping.exponent,0.1..=8.0,1.0).logarithmic(true).text("Curve"))
                .help_text("1 is linear. Below 1 lifts quiet movement; above 1 reserves strong movement for louder peaks. Signed sources keep their center and direction.");
            ui.add(Parameter::new(&mut d.shaping.dead_zone,0.0..=0.95,0.0).text("Dead zone"))
                .help_text("Ignore small input values, then stretch the remaining range. Useful for stopping tiny unwanted motion near silence or a controller's center.");
            ui.add(Parameter::new(&mut d.shaping.steps,0..=32,0).bounds(0.0..=128.0).text("Steps"))
                .help_text("0 or 1 keeps continuous motion. 2 creates an on/off signal; higher counts divide the output into equally spaced levels. Final smoothing can soften the transitions.");
            ui.checkbox(&mut d.shaping.invert,"Invert output").help_text("For 0–1 sources, swaps high and low. For signed sources, reverses direction around zero.");
        });
        let signed = d.kind() == OutputKind::Bipolar;
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), 100.0),
            egui::Sense::hover(),
        );
        ui.painter()
            .rect_filled(rect, 4.0, ui.visuals().extreme_bg_color);
        let plot = rect.shrink(8.0);
        let low = if signed { -1.0 } else { 0.0 };
        let span = 1.0 - low;
        let point = |x: f32, y: f32| {
            egui::pos2(
                plot.left() + (x - low) / span * plot.width(),
                plot.bottom() - (y - low) / span * plot.height(),
            )
        };
        ui.painter().line_segment(
            [point(low, low), point(1.0, 1.0)],
            ui.visuals().widgets.inactive.bg_stroke,
        );
        let color = channel_color(d);
        ui.painter().add(egui::Shape::line(
            (0..=128)
                .map(|i| {
                    let x = low + span * i as f32 / 128.0;
                    point(x, d.shaping.map(x, signed))
                })
                .collect(),
            egui::Stroke::new(1.5, color),
        ));
        let input = d.normalization.map(sample.raw).clamp(low, 1.0);
        ui.painter()
            .circle_filled(point(input, d.shaping.map(input, signed)), 4.0, color);
        ui.small("Input / output · dot shows the live signal");
    });
    settings_group(ui, "Smoothing", |ui| {
        ui.add(Parameter::new(&mut d.smoothing_ms,0.0..=1000.0,0.0)
        .bounds(0.0..=5000.0).text("Smoothing ms"))
        .help_text("Soften sudden changes in this channel's final output. 0 ms is off. Higher values give smoother, slower motion. Applies after normalization and influence to every mapping; hit detection stays immediate.");
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
#[derive(Clone)]
enum StripAction {
    Select(u64),
    Processor(u64),
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
    AddAudioAt(usize),
    InspectEffect(u64, u64),
    AddEffect(u64, usize, crate::signals::inserts::Effect),
    MoveEffect(u64, u64, usize),
    RemoveEffect(u64, u64),
    BypassEffect(u64, u64),
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
    ui.set_min_width(190.0);
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
            ("Delete channel", StripAction::Delete(d.id), true),
        ] {
            if ui.add_enabled(enabled, egui::Button::new(label)).clicked() {
                action = Some(value);
                ui.close();
            }
        }
        ui.separator();
        if ui
            .button(if d.inserts.is_some() {
                "Inspect channel & inserts"
            } else {
                "Open signal processor…"
            })
            .clicked()
        {
            action = Some(StripAction::Processor(d.id));
            ui.close();
        }
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
                if ui.button("Audio").clicked() {
                    action = Some(StripAction::AddAudioAt(slot + usize::from(d.is_some())));
                    ui.close();
                }
                for method in [Method::Lfo, Method::Midi] {
                    if ui
                        .button(if method == Method::Midi {
                            "MIDI"
                        } else {
                            "LFO"
                        })
                        .clicked()
                    {
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
        if let Some(color) = color_palette(ui, channel_color(d)) {
            action = Some(StripAction::Color(d.id, color));
        }
    }
    action
}
pub(crate) fn color_palette(ui: &mut egui::Ui, selected: egui::Color32) -> Option<[u8; 3]> {
    let mut choice = None;
    ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
    ui.spacing_mut().interact_size.y = 16.0;
    for row in CHANNEL_SWATCHES {
        ui.horizontal(|ui| {
            for [r, g, b] in row {
                let color = egui::Color32::from_rgb(r, g, b);
                let (rect, response) =
                    ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::click());
                ui.painter().rect_filled(rect, 0.0, color);
                if color == selected {
                    ui.painter().rect_stroke(
                        rect.expand(1.0),
                        1.0,
                        egui::Stroke::new(1.0, ui.visuals().text_color()),
                        egui::StrokeKind::Outside,
                    );
                }
                if response
                    .help_text(format!(
                        "#{:02X}{:02X}{:02X}",
                        color.r(),
                        color.g(),
                        color.b()
                    ))
                    .clicked()
                {
                    choice = Some([color.r(), color.g(), color.b()]);
                    ui.close();
                }
            }
        });
    }
    choice
}

fn blend(a: egui::Color32, b: egui::Color32, t: f32) -> egui::Color32 {
    egui::Color32::from_rgb(
        (a.r() as f32 + (b.r() as f32 - a.r() as f32) * t) as u8,
        (a.g() as f32 + (b.g() as f32 - a.g() as f32) * t) as u8,
        (a.b() as f32 + (b.b() as f32 - a.b() as f32) * t) as u8,
    )
}

#[allow(clippy::too_many_arguments)]
fn channel_strip(
    ui: &mut egui::Ui,
    rect: Rect,
    slot: usize,
    d: &mut Definition,
    sample: &Sample,
    selected: bool,
    history: Option<&history::History>,
    midi: Option<(&crate::midi::Snapshot, Option<&midi_preview::History>, f64)>,
) -> Option<StripAction> {
    let color = channel_color(d);
    workspace_ui::strip_surface(ui, rect, color, selected);
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
    workspace_ui::strip_title(
        ui,
        Rect::from_min_max(
            egui::pos2(title.left() + 8.0, title.top()),
            egui::pos2(title.right() - 2.0, title.bottom()),
        ),
        format!("{} {}", slot + 1, d.name),
        dark,
    );
    let response = workspace_ui::drag_cursor(ui.interact(
        title,
        ui.id().with("channel-title"),
        egui::Sense::click_and_drag(),
    ))
    .help_text("Select channel · double-click to rename · drag to reorder");
    if response.double_clicked() {
        action = Some(StripAction::Rename(d.id));
    } else if response.clicked() {
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
        .help_text("Delete channel · undo is available in the signal inspector")
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
    crate::dropdown::ComboBox::from_id_salt("strip-input")
        .width(142.0)
        .selected_text(d.input.label())
        .show_ui(&mut controls, |ui| input_options(ui, d))
        .response
        .help_text(crate::help::control_description("Input"));
    if d.input == Input::Midi {
        controls.small("Message");
        crate::midi::ui::message_picker(&mut controls, &mut d.midi.message);
    } else {
        controls.small("Detector");
        crate::dropdown::ComboBox::from_id_salt("strip-method")
            .width(142.0)
            .selected_text(d.method.label())
            .show_ui(&mut controls, |ui| {
                for method in Method::ALL.into_iter().filter(|m| *m != Method::Midi) {
                    if ui
                        .selectable_value(&mut d.method, method, method.label())
                        .help_text(method.description())
                        .changed()
                    {
                        d.input = if method == Method::Lfo {
                            Input::Lfo
                        } else {
                            Input::System
                        };
                    }
                }
            })
            .response
            .help_text(d.method.description());
    }
    d.sanitize();
    ui.interact(rect, ui.id().with("signal-strip-help"), egui::Sense::hover())
        .help_fallback(|| format!("{}\n{}\nClick to inspect this channel. Drag its header to reorder; right-click to duplicate, rename, delete or choose a color. The influence knob scales visual modulation, not audio volume. M mutes this source; S solos it alongside any other soloed sources.", d.name, d.method.description()));
    let bottom = rect.bottom() - 144.0;
    let graph_top = controls.min_rect().bottom() + 8.0;
    let graph = Rect::from_min_max(
        egui::pos2(rect.left() + 8.0, graph_top),
        egui::pos2(rect.right() - 8.0, (bottom - 8.0).max(graph_top + 16.0)),
    );
    if d.input != Input::Midi {
        ui.interact(graph, ui.id().with("signal-history-help"), egui::Sense::hover())
        .help_text("Signal history\nRecent final control output, after response, normalization, influence and smoothing. A flat line means the signal is steady or unavailable; use the channel inspector’s Output tab to expand a narrow range or smooth jitter.");
    }
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
    if d.input == Input::Midi {
        let empty = crate::midi::Snapshot::default();
        let (snapshot, history, time) = midi.unwrap_or((&empty, None, 0.0));
        midi_preview::draw(ui, graph, &d.midi, snapshot, history, time, color);
    } else {
        for i in 1..4 {
            ui.painter().hline(
                graph.x_range(),
                graph.top() + graph.height() * i as f32 / 4.0,
                egui::Stroke::new(0.5, ui.visuals().weak_text_color().gamma_multiply(0.14)),
            );
        }
        if let Some(history) = history {
            let points: Vec<egui::Pos2> = history
                .points()
                .map(|[x, v]| {
                    egui::pos2(
                        graph.left() + x * graph.width(),
                        if d.kind() == OutputKind::Bipolar {
                            graph.center().y - v.clamp(-2.0, 2.0) * 0.25 * graph.height()
                        } else {
                            graph.bottom() - v.clamp(0.0, 2.0) * 0.5 * graph.height()
                        },
                    )
                })
                .collect();
            ui.painter().with_clip_rect(graph).add(egui::Shape::line(
                points.clone(),
                egui::Stroke::new(4.0, color.gamma_multiply(0.14)),
            ));
            ui.painter()
                .with_clip_rect(graph)
                .add(egui::Shape::line(points, egui::Stroke::new(1.7, color)));
        }
    }
    channel_footer(ui, rect, d, sample, color, dark);
    action
}

fn channel_footer(
    ui: &mut egui::Ui,
    rect: Rect,
    d: &mut Definition,
    sample: &Sample,
    color: egui::Color32,
    dark: egui::Color32,
) {
    let bottom = rect.bottom() - 144.0;
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
    let knob = Rect::from_center_size(
        egui::pos2(rect.left() + 40.0, bottom + 58.0),
        egui::vec2(54.0, 54.0),
    );
    influence_knob(ui, knob, &mut d.influence, color);
    let value_rect = Rect::from_min_size(
        egui::pos2(rect.left() + 12.0, bottom + 88.0),
        egui::vec2(76.0, 22.0),
    );
    let mut value_ui = ui.new_child(
        egui::UiBuilder::new()
            .id_salt("channel-influence-value")
            .max_rect(value_rect),
    );
    value_ui.set_clip_rect(value_rect.intersect(ui.clip_rect()));
    value_ui.spacing_mut().interact_size.y = 22.0;
    value_ui.style_mut().override_font_id = Some(egui::FontId::proportional(11.0));
    let mut percent = d.influence * 100.0;
    if value_ui
        .add_sized(
            value_rect.size(),
            egui::DragValue::new(&mut percent)
                .range(0.0..=200.0)
                .speed(0.5)
                .fixed_decimals(0)
                .suffix("%"),
        )
        .help_text("Visual influence · 100% is unity · drag or type a value")
        .changed()
    {
        d.influence = percent / 100.0;
    }
    let meter_height = 65.0;
    for (i, value, label) in [(0, sample.level, "In"), (1, sample.value.abs(), "Out")] {
        let meter = Rect::from_min_size(
            egui::pos2(rect.left() + 98.0 + i as f32 * 23.0, bottom + 35.0),
            egui::vec2(9.0, meter_height),
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
    channel_buttons(
        ui,
        value_rect,
        color,
        dark,
        &mut d.enabled,
        &mut d.solo,
        if d.inserts.is_some() && d.input == Input::System {
            "Mute this channel's audio and control outputs"
        } else {
            "Mute this channel's visual modulation output"
        },
        "Solo this channel's visual modulation output · multiple solos are allowed",
    );
}

#[allow(clippy::too_many_arguments)]
fn channel_buttons(
    ui: &mut egui::Ui,
    value_rect: Rect,
    color: egui::Color32,
    ink: egui::Color32,
    enabled: &mut bool,
    solo: &mut bool,
    mute_hint: &str,
    solo_hint: &str,
) {
    let row = Rect::from_min_size(
        value_rect.left_bottom() + egui::vec2(0.0, 6.0),
        egui::vec2(value_rect.width(), 22.0),
    );
    let mut buttons = ui.new_child(
        egui::UiBuilder::new()
            .id_salt("channel-mute-solo")
            .max_rect(row),
    );
    buttons.set_clip_rect(row.intersect(ui.clip_rect()));
    buttons.spacing_mut().interact_size.y = row.height();
    buttons.spacing_mut().button_padding = egui::vec2(4.0, 1.0);
    let width = (row.width() - 4.0) * 0.5;
    for (i, label, active, hint) in [(0, "M", !*enabled, mute_hint), (1, "S", *solo, solo_hint)] {
        let bounds = Rect::from_min_size(
            row.min + egui::vec2(i as f32 * (width + 4.0), 0.0),
            egui::vec2(width, row.height()),
        );
        if buttons
            .put(
                bounds,
                egui::Button::new(egui::RichText::new(label).size(11.0).strong().color(
                    if active {
                        ink
                    } else {
                        ui.visuals().text_color()
                    },
                ))
                .fill(if active {
                    color
                } else {
                    ui.visuals().extreme_bg_color
                })
                .corner_radius(3.0)
                .stroke(egui::Stroke::new(
                    1.0,
                    ui.visuals().text_color().gamma_multiply(0.25),
                )),
            )
            .help_text(hint)
            .clicked()
        {
            if i == 0 {
                *enabled = !*enabled;
            } else {
                *solo = !*solo;
            }
        }
    }
}

fn input_options(ui: &mut egui::Ui, d: &mut Definition) {
    for (input, method) in [
        (Input::System, Method::Intensity),
        (Input::Lfo, Method::Lfo),
        (Input::Midi, Method::Midi),
    ] {
        if ui
            .selectable_value(&mut d.input, input, input.label())
            .changed()
        {
            d.method = method;
        }
    }
    for input in Input::ALL.into_iter().skip(3) {
        ui.add_enabled(false, egui::Button::new(input.label()));
    }
}

fn influence_knob(ui: &mut egui::Ui, rect: Rect, value: &mut f32, color: egui::Color32) {
    let response = ui
        .interact(
            rect,
            ui.id().with("influence-knob"),
            egui::Sense::click_and_drag(),
        )
        .on_hover_and_drag_cursor(egui::CursorIcon::ResizeVertical)
        .help_text("Visual influence · drag vertically · Shift: fine · double-click: 100%");
    let speed = if ui.input(|i| i.modifiers.shift) {
        0.002
    } else {
        0.013333
    };
    if response.dragged() {
        let delta = if response.drag_started() {
            response.drag_delta().y
        } else {
            ui.input(|i| i.pointer.delta().y)
        };
        *value = (*value - delta * speed).clamp(0.0, 2.0);
    }
    if response.double_clicked() {
        *value = 1.0;
    }
    if response.clicked() {
        response.request_focus();
    }
    if response.has_focus() {
        let delta = ui.input(|i| {
            i.key_pressed(egui::Key::ArrowUp) as i32 - i.key_pressed(egui::Key::ArrowDown) as i32
        });
        *value = (*value + delta as f32 * speed * 3.0).clamp(0.0, 2.0);
    }
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Slider, true, "Visual influence")
    });
    let center = rect.center();
    let painter = ui.painter_at(rect);
    let arc = |end: f32| {
        (0..=40)
            .map(|i| center + egui::Vec2::angled(2.35 + 4.72 * end * i as f32 / 40.0) * 23.0)
            .collect()
    };
    painter.add(egui::Shape::line(
        arc(1.0),
        egui::Stroke::new(2.5, ui.visuals().text_color().gamma_multiply(0.15)),
    ));
    painter.add(egui::Shape::line(
        arc(*value / 2.0),
        egui::Stroke::new(2.5, color),
    ));
    painter.circle_filled(center, 18.0, ui.visuals().extreme_bg_color);
    painter.circle_stroke(
        center,
        18.0,
        egui::Stroke::new(1.0, ui.visuals().text_color().gamma_multiply(0.4)),
    );
    let direction = egui::Vec2::angled(2.35 + 4.72 * *value / 2.0);
    painter.line_segment(
        [center + direction * 7.0, center + direction * 15.0],
        egui::Stroke::new(2.0, ui.visuals().text_color()),
    );
}

/// List control sources and offer a visible, explicit conversion for audio-only tracks.
/// Only a clicked conversion adds an insert; merely opening the menu never edits a track.
pub(crate) fn modulation_source_options(
    ui: &mut egui::Ui,
    bank: &mut Bank,
    selected: &mut Option<u64>,
) {
    ui.selectable_value(selected, None, "No modulation");
    for source in &mut bank.channels {
        if source.has_control_output() {
            ui.selectable_value(selected, Some(source.id), &source.name);
        } else {
            let can_add = source.inserts.as_ref().is_some_and(|chain| {
                chain.stages.len() < super::inserts::MAX_INSERTS && chain.next_id < u64::MAX
            });
            let response = ui.add_enabled(
                can_add,
                egui::Button::new(format!("{} · add Overall intensity", source.name)).frame(false),
            );
            if response.clicked() {
                let chain = source
                    .inserts
                    .as_mut()
                    .expect("audio-only source has a chain");
                if chain
                    .add(
                        source.input,
                        chain.stages.len(),
                        super::inserts::Effect::Detect(super::inserts::Detection::new(
                            super::inserts::Detector::Level,
                        )),
                    )
                    .is_some()
                {
                    *selected = Some(source.id);
                    ui.close();
                }
            }
            response.on_hover_text(if can_add {
                "Add an Overall intensity insert to convert this audio into a modulation curve. Existing audio processing and the visualizer feed stay intact."
            } else { "This channel has reached its insert limit. Add a detector in Signals after making room." });
        }
    }
    if bank.channels.is_empty() {
        ui.weak("Add an Audio, LFO, or MIDI channel in Signals.");
    }
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
    let previous_signal = route.signal;
    crate::dropdown::ComboBox::from_id_salt(("named-signal", route.id))
        .selected_text(selected)
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            modulation_source_options(ui, &mut panel.bank, &mut route.signal);
            if route.target.layered() {
                ui.selectable_value(&mut route.signal, None, "Automatic layer bands");
            }
        });
    if route.signal != previous_signal {
        route.enabled = route.signal.is_some();
    }
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
                panel.inspect_requested = true;
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
    ui.add(Parameter::new(&mut lfo.depth, 0.0..=1.0, 1.0).text("Depth")).help_text("LFO depth\nScale the oscillator amplitude before the channel’s output processing. Zero produces no oscillation; one uses the full waveform range.");
    ui.checkbox(&mut lfo.bipolar, "Bipolar · −1 to +1").help_text("Bipolar LFO\nOn: oscillate from −1 to +1 around zero. Off: oscillate from 0 to 1. Depth and channel influence scale this range.");
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
        Panel {
            bank: Bank::test_sources(),
            ..Default::default()
        }
    }

    #[test]
    fn keyboard_selection_reveals_offscreen_signals_and_handles_a_master_only_mixer() {
        let ctx = egui::Context::default();
        let mut panel = Panel {
            selected: Some(MASTER_ID),
            ..Default::default()
        };
        for forward in [false, true] {
            panel.select_relative(forward);
            assert_eq!(panel.selected, Some(MASTER_ID));
        }
        assert!(panel.bank.channels.is_empty());
        for _ in 0..12 {
            panel.bank.add_source(Input::Lfo).unwrap();
        }
        let last = panel.bank.slots.last().copied().flatten().unwrap();
        let bank = panel.bank.clone();
        let render = |panel: &mut Panel| {
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(500.0, 400.0),
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
                    )
                },
            );
            out.textures_delta.clear();
            out
        };
        render(&mut panel);
        panel.select_relative(false); // From the pinned master to the last scene signal.
        render(&mut panel);
        let out = render(&mut panel);
        assert_eq!(panel.selected, Some(last));
        let (clip, label) = out
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.job.text == "12 LFO" => Some((
                    shape.clip_rect,
                    Rect::from_min_size(text.pos, text.galley.size()),
                )),
                _ => None,
            })
            .unwrap();
        assert!(
            clip.contains_rect(label),
            "the selected signal must scroll into view: clip {clip:?}, label {label:?}"
        );
        assert_eq!(panel.bank, bank);
    }

    #[test]
    fn master_cannot_be_deleted_and_does_not_consume_channel_undo() {
        let mut panel = test_panel();
        panel.act(StripAction::Delete(1));
        let bank = panel.bank.clone();
        panel.act(StripAction::Select(MASTER_ID));
        panel.delete_selection();
        panel.act(StripAction::Delete(MASTER_ID));
        assert_eq!(panel.selected, Some(MASTER_ID));
        assert_eq!(panel.bank, bank);
        assert_eq!(panel.deleted.as_ref().unwrap().0.id, 1);
        panel.act(StripAction::Undo);
        assert!(panel.bank.channels.iter().any(|d| d.id == 1));
        assert!(panel.bank.valid());
        assert!(!panel.bank.slots.contains(&Some(MASTER_ID)));
    }

    #[test]
    fn audio_channels_share_mixer_order_actions_and_have_spectral_previews() {
        let mut panel = Panel::default();
        panel.act(StripAction::AddAt(Method::Lfo, 0));
        let lfo = panel.selected.unwrap();
        panel.act(StripAction::AddAudioAt(0));
        let id = panel.selected.unwrap();
        assert_eq!(panel.bank.slots, vec![Some(id), Some(lfo)]);
        let frame = AnalysisFrame {
            sequence: 1,
            bins: vec![crate::analysis::FrequencyBin {
                frequency_hz: 1000.0,
                magnitude: 0.5,
                dbfs: -6.0,
            }]
            .into(),
            mix_rms: 0.2,
            ..Default::default()
        };
        panel.snapshot = Arc::new(Processor::default().step(&panel.bank, &frame, 0.02, true));
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1200.0, 600.0),
                )),
                ..Default::default()
            },
            |ui| {
                panel.show(
                    ui,
                    ui.available_rect_before_wrap(),
                    &frame,
                    false,
                    &AppTheme::default(),
                );
            },
        );
        output.textures_delta.clear();
        assert!(output.shapes.iter().any(
            |s| matches!(&s.shape,egui::Shape::Text(t) if t.galley.job.text=="AUDIO · SPECTRUM")
        ));
        assert!(
            output.shapes.iter().any(
                |s| matches!(&s.shape,egui::Shape::Text(t) if t.galley.job.text=="System audio")
            )
        );
        assert_eq!(panel.bank.channels.len(), 2);
        assert!(panel.bank.valid());
        panel.act(StripAction::Insert(id, 2));
        assert_eq!(panel.bank.slots, vec![Some(lfo), Some(id)]);
        panel.act(StripAction::Color(id, [10, 20, 30]));
        panel.act(StripAction::Duplicate(id, 2));
        assert_eq!(panel.bank.audio_inputs.len(), 2);
        let copy = panel.selected.unwrap();
        assert_eq!(
            panel
                .bank
                .audio_inputs
                .iter()
                .find(|b| b.id == copy)
                .unwrap()
                .color,
            Some([10, 20, 30])
        );
        panel.act(StripAction::Delete(copy));
        panel.act(StripAction::Undo);
        assert_eq!(panel.bank.audio_inputs.len(), 2);
        assert!(panel.bank.valid());
        output.textures_delta.clear();
    }

    #[test]
    fn audio_strip_controls_fit_with_horizontal_parent_and_large_app_font() {
        let ctx = egui::Context::default();
        let mut bank = Bank::default();
        bank.add_audio_input();
        let bus = &mut bank.audio_inputs[0];
        let bounds = Rect::from_min_size(egui::pos2(20.0, 20.0), egui::vec2(156.0, 300.0));
        for _ in 0..2 {
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                ui.style_mut().override_font_id = Some(egui::FontId::proportional(18.0));
                ui.horizontal(|ui| {
                    audio_channel::strip(ui, bounds, 0, bus, None, false);
                });
            });
            for label in ["Source", "System audio", "Audio signal", "M", "S"] {
                let text = output
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::Shape::Text(text) if text.galley.job.text == label => Some(text),
                        _ => None,
                    })
                    .unwrap_or_else(|| panic!("missing {label}"));
                assert!(
                    bounds.contains_rect(Rect::from_min_size(text.pos, text.galley.size())),
                    "{label} overflows channel"
                );
            }
            output.textures_delta.clear();
        }
    }

    #[test]
    fn audio_preview_bar_height_tracks_frequency_energy_and_silence() {
        let context = egui::Context::default();
        let render = |magnitude| {
            let frame = AnalysisFrame {
                bins: vec![crate::analysis::FrequencyBin {
                    frequency_hz: 1000.0,
                    magnitude,
                    ..Default::default()
                }]
                .into(),
                ..Default::default()
            };
            let mut output = context.run_ui(Default::default(), |ui| {
                audio_channel::spectrum(
                    ui,
                    Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(180.0, 160.0)),
                    Some(&frame),
                    egui::Color32::RED,
                )
            });
            let height = output
                .shapes
                .iter()
                .filter_map(|s| match &s.shape {
                    egui::Shape::Rect(r) if r.fill == egui::Color32::RED => Some(r.rect.height()),
                    _ => None,
                })
                .fold(0.0_f32, f32::max);
            output.textures_delta.clear();
            height
        };
        assert!(render(0.5) > 100.0);
        assert!(render(0.01) < 100.0);
        assert_eq!(render(0.0), 0.0);
    }
    #[test]
    fn range_learning_finishes_with_both_editors_hidden() {
        let mut panel = test_panel();
        let id = panel.bank.channels[0].id;
        panel.learning = Some(RangeLearning::new(id, 0.0));
        for (time, raw) in [(1.0, 0.50), (5.1, 0.55)] {
            panel.snapshot = Arc::new(Snapshot {
                inserts: Default::default(),
                audio: Default::default(),
                time,
                values: [(
                    id,
                    Sample {
                        raw,
                        status: Status::Live,
                        ..Default::default()
                    },
                )]
                .into(),
            });
            panel.advance_learning();
        }
        assert!(!panel.learning_range());
        let range = &panel.bank.channels[0].normalization;
        assert!(range.enabled);
        assert_eq!((range.low, range.high), (0.50, 0.55));
        assert_eq!(panel.learning_notice.as_ref().unwrap().0, id);
        assert!(
            panel
                .learning_notice
                .as_ref()
                .unwrap()
                .1
                .contains("0.500 – 0.550")
        );
        panel.advance_learning();
        assert!(panel.learning_notice.is_some());
        panel.replace_bank(Bank::default());
        assert!(panel.learning_notice.is_none());
    }
    #[test]
    fn range_learning_uses_changing_live_raw_samples_and_ignores_disconnections() {
        let mut learning = RangeLearning::new(1, 0.0);
        let mut snapshot = Snapshot {
            inserts: Default::default(),
            audio: Default::default(),
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
                "Delete channel",
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
    fn double_click_renames_audio_lfo_midi_and_legacy_signal_headers_inline() {
        for input in [Input::System, Input::Lfo, Input::Midi] {
            for legacy in [false, true] {
                let ctx = egui::Context::default();
                let mut panel = Panel::default();
                let id = panel.bank.add_source(input).unwrap();
                panel.bank.channels[0].name = "Original".into();
                if legacy {
                    panel.bank.channels[0].inserts = None;
                    // Legacy control-only channels have no same-ID audio bus.
                    panel.bank.audio_inputs.clear();
                }
                assert!(panel.bank.valid());
                let render = |panel: &mut Panel, events| {
                    let mut out = ctx.run_ui(
                        egui::RawInput {
                            screen_rect: Some(Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::vec2(800.0, 500.0),
                            )),
                            events,
                            ..Default::default()
                        },
                        |ui| {
                            panel.show(
                                ui,
                                Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 400.0)),
                                &AnalysisFrame::default(),
                                false,
                                &AppTheme::default(),
                            )
                        },
                    );
                    out.textures_delta.clear();
                    out
                };
                render(&mut panel, vec![]);
                let pos = egui::pos2(65.0, 22.0);
                for _ in 0..2 {
                    render(&mut panel, vec![egui::Event::PointerMoved(pos)]);
                    for pressed in [true, false] {
                        render(
                            &mut panel,
                            vec![egui::Event::PointerButton {
                                pos,
                                button: egui::PointerButton::Primary,
                                pressed,
                                modifiers: egui::Modifiers::NONE,
                            }],
                        );
                    }
                }
                let out = render(&mut panel, vec![]);
                assert!(panel.rename.is_some(), "{input:?}, legacy {legacy}");
                assert!(crate::shortcuts::editing_text(&ctx));
                assert!(!out.shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.job.text == "Rename channel")));
                render(&mut panel, vec![egui::Event::Text("New signal".into())]);
                render(
                    &mut panel,
                    vec![egui::Event::Key {
                        key: egui::Key::Enter,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                    }],
                );
                assert_eq!(panel.bank.channels[0].name, "New signal");
                assert_eq!(panel.selected, Some(id));
                if let Some(bus) = panel.bank.audio_inputs.iter().find(|b| b.id == id) {
                    assert_eq!(bus.name, "New signal");
                }
                assert!(panel.rename.is_none());
                assert!(panel.bank.valid());
            }
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
                |ui| {
                    panel.rename_field(
                        ui,
                        Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(156.0, 28.0)),
                        1,
                    )
                },
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
        panel.act(StripAction::Rename(1));
        render(&mut panel, None);
        panel.rename.as_mut().unwrap().1 = "   ".into();
        render(&mut panel, Some(egui::Key::Enter));
        assert_eq!(panel.bank.channels[0].name, "Kick pulse");
        assert!(panel.rename.is_some(), "invalid names remain editable");
        render(&mut panel, Some(egui::Key::Escape));
        assert!(panel.rename.is_none());
    }

    #[test]
    fn strip_knob_mute_and_solo_work_and_controls_fit_at_minimum_height() {
        let ctx = egui::Context::default();
        let mut definition = Definition::recipe(42, Method::Band);
        let bounds = Rect::from_min_size(egui::pos2(20.0, 20.0), egui::vec2(156.0, 300.0));
        let render = |d: &mut Definition, events| {
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(600.0, 500.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    channel_strip(ui, bounds, 0, d, &Sample::default(), false, None, None);
                },
            );
            out.textures_delta.clear();
            out
        };
        render(&mut definition, vec![]);
        let out = render(&mut definition, vec![]);
        let text = |label: &str| {
            out.shapes
                .iter()
                .find_map(|s| match &s.shape {
                    egui::Shape::Text(t) if t.galley.job.text == label => {
                        Some(t.pos + t.galley.size() * 0.5)
                    }
                    _ => None,
                })
                .unwrap()
        };
        for label in ["Source", "Detector", "Visual influence", "M", "S"] {
            assert!(bounds.contains(text(label)));
        }
        assert!(!out.shapes.iter().any(|s| matches!(&s.shape,egui::Shape::Text(t) if t.galley.job.text == "Full mix" || t.galley.job.text.contains("mappings · Inspect"))));
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        for label in ["M", "S"] {
            let pos = text(label);
            render(
                &mut definition,
                vec![egui::Event::PointerMoved(pos), button(pos, true)],
            );
            render(&mut definition, vec![button(pos, false)]);
        }
        assert!(!definition.enabled && definition.solo);
        let knob = egui::pos2(bounds.left() + 40.0, bounds.bottom() - 86.0);
        render(
            &mut definition,
            vec![egui::Event::PointerMoved(knob), button(knob, true)],
        );
        render(
            &mut definition,
            vec![egui::Event::PointerMoved(knob - egui::vec2(0.0, 30.0))],
        );
        render(
            &mut definition,
            vec![button(knob - egui::vec2(0.0, 30.0), false)],
        );
        assert!(definition.influence > 1.2 && definition.influence <= 2.0);
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
                        None,
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
            egui::pos2(left + 790.0, 90.0),
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
    fn untabbed_inspectors_keep_source_and_processing_controls_together_and_scoped() {
        for width in [236.0, 420.0] {
            for input in [Input::System, Input::Lfo, Input::Midi] {
                for legacy in [false, true] {
                    let ctx = egui::Context::default();
                    ctx.memory_mut(|m| m.set_everything_is_visible(true));
                    let mut panel = Panel::default();
                    let id = panel.bank.add_source(input).unwrap();
                    panel.selected = Some(id);
                    if legacy {
                        panel.bank.channels[0].inserts = None;
                    }
                    let mut output = ctx.run_ui(
                        egui::RawInput {
                            screen_rect: Some(Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::vec2(width, 5000.0),
                            )),
                            ..Default::default()
                        },
                        |ui| {
                            let before = ui.style().clone();
                            panel.show_inspector(
                                ui,
                                ui.available_rect_before_wrap(),
                                &AnalysisFrame::default(),
                                &AppTheme::default(),
                            );
                            assert_eq!(ui.style(), &before);
                            assert!(crate::modules::inspector_bounds(ui).is_none());
                            assert!(
                                ui.data(|d| d.get_temp::<bool>(egui::Id::new(
                                    crate::modules::FLAT_INSPECTOR
                                )))
                                .is_none()
                            );
                        },
                    );
                    let labels: Vec<_> = output
                        .shapes
                        .iter()
                        .filter_map(|s| match &s.shape {
                            egui::Shape::Text(t) => Some(t.galley.job.text.as_str()),
                            _ => None,
                        })
                        .collect();
                    for label in if legacy {
                        ["Channel", "Normalization", "Used by"]
                    } else {
                        ["Channel", "Inserts", "Output"]
                    } {
                        assert!(
                            labels.contains(&label),
                            "{input:?}, legacy={legacy}: missing {label}"
                        );
                    }
                    assert!(!labels.contains(&"Geometry") && !labels.contains(&"Appearance"));
                    output.textures_delta.clear();
                }
            }
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
            for size in [
                egui::vec2(512.0, 300.0),
                egui::vec2(1024.0, 650.0),
                egui::vec2(1800.0, 1000.0),
            ] {
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
