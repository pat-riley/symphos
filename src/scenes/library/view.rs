use super::*;
use crate::icons::{self, Icon};
use egui::{Align, Color32, FontId, Layout, Pos2, Response, RichText, Sense, Stroke};

const CONTROL_HEIGHT: f32 = 30.0;
const ROW_HEIGHT: f32 = 70.0;

enum Edit {
    Rename(Selection, String),
    Delete(Selection, String),
}
#[derive(Clone, Copy)]
struct Availability {
    duplicate: bool,
    edit: bool,
    delete: bool,
    refresh: bool,
    load: bool,
    earlier: bool,
    later: bool,
}
impl Availability {
    fn new(key: Selection, index: usize, book: &Book, presets: &Presets, failed: bool) -> Self {
        let preset = matches!(key, Selection::Preset(_));
        let count = if preset {
            presets.entries.len()
        } else {
            book.scenes.len()
        };
        let edit = !preset || !failed;
        Self {
            duplicate: edit
                && count
                    < if preset {
                        256
                    } else {
                        super::super::MAX_SCENES
                    },
            edit,
            delete: edit && (preset || count > 1),
            refresh: key == Selection::Scene(book.active_scene),
            load: !preset || book.scenes.len() < super::super::MAX_SCENES,
            earlier: edit && index > 0,
            later: edit && index + 1 < count,
        }
    }
}

impl Library {
    pub fn show(&mut self, ui: &mut egui::Ui, rect: Rect, book: &Book) -> Option<Command> {
        let mut command = None;
        let mut edit = None;
        let mut selection = self.selection;
        let list_before = self.list_view;
        let mut page = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("scene-library")
                .layout(Layout::top_down(Align::Min))
                .max_rect(rect.shrink(24.0)),
        );
        page.set_clip_rect(rect);
        page.spacing_mut().interact_size.y = CONTROL_HEIGHT;
        page.spacing_mut().button_padding = egui::vec2(10.0, 6.0);
        page.horizontal(|ui| {
            ui.heading(RichText::new("Library").size(24.0).strong());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .button("Back to workspace")
                    .help_text("Close Library · Escape")
                    .clicked()
                {
                    self.open = false;
                }
            });
        });
        page.add_space(4.0);
        page.label(
            RichText::new("Your scenes, ready to perform.").color(page.visuals().weak_text_color()),
        );
        page.add_space(18.0);
        page.horizontal(|ui| {
            let previous = self.preset_tab;
            ui.selectable_value(
                &mut self.preset_tab,
                false,
                format!("Project scenes  {}", book.scenes.len()),
            );
            ui.selectable_value(
                &mut self.preset_tab,
                true,
                format!("Saved presets  {}", self.presets.entries.len()),
            );
            if previous != self.preset_tab {
                self.query.clear();
                selection = if self.preset_tab {
                    self.presets
                        .entries
                        .first()
                        .map(|s| Selection::Preset(s.id))
                } else {
                    Some(Selection::Scene(book.active_scene))
                };
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let list = icons::sized_button(
                    ui,
                    Icon::ListView,
                    self.list_view,
                    CONTROL_HEIGHT,
                    "List view",
                );
                list.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "List view")
                });
                if list.clicked() {
                    self.list_view = true;
                }
                let grid = icons::sized_button(
                    ui,
                    Icon::GridView,
                    !self.list_view,
                    CONTROL_HEIGHT,
                    "Grid view",
                );
                grid.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Grid view")
                });
                if grid.clicked() {
                    self.list_view = false;
                }
                let width = (ui.available_width() - 12.0).clamp(100.0, 240.0);
                ui.add_sized(
                    [width, CONTROL_HEIGHT],
                    egui::TextEdit::singleline(&mut self.query).hint_text("Search scenes…"),
                );
            });
        });
        if self.list_view != list_before
            && let Some(path) = &self.view_path
            && let Err(error) = crate::workspace::atomic_write(
                path,
                if self.list_view { b"true" } else { b"false" },
            )
        {
            self.notice = Some(format!("Could not remember Library view: {error}"));
        }
        page.add_space(12.0);
        let entries = if self.preset_tab {
            &self.presets.entries
        } else {
            &book.scenes
        };
        let selected = selection.and_then(|key| {
            entries
                .iter()
                .enumerate()
                .find(|(_, s)| key == item_key(self.preset_tab, s.id))
                .map(|(i, s)| (key, i, s))
        });
        page.horizontal(|ui| {
            if self.preset_tab {
                if ui
                    .button("Visual presets…")
                    .help_text("Browse the existing visual and component preset collection.")
                    .clicked()
                {
                    command = Some(Command::LegacyPresets);
                }
            } else if ui
                .add_enabled(
                    book.scenes.len() < super::super::MAX_SCENES,
                    egui::Button::new("+ New scene"),
                )
                .clicked()
            {
                command = Some(Command::New);
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let load = selected.is_some_and(|(key, i, _)| {
                    Availability::new(key, i, book, &self.presets, self.load_failed).load
                });
                if ui
                    .add_enabled(
                        load,
                        egui::Button::new(
                            RichText::new("Load scene").color(ui.visuals().selection.stroke.color),
                        )
                        .fill(ui.visuals().selection.bg_fill),
                    )
                    .clicked()
                    && let Some((key, _, _)) = selected
                {
                    command = Some(Command::Load(key));
                }
                ui.add_enabled_ui(selected.is_some(), |ui| {
                    ui.menu_button("More  ···", |ui| {
                        if let Some((key, index, scene)) = selected {
                            edit_menu(
                                ui,
                                key,
                                index,
                                &scene.name,
                                Availability::new(
                                    key,
                                    index,
                                    book,
                                    &self.presets,
                                    self.load_failed,
                                ),
                                &mut command,
                                &mut edit,
                            );
                        }
                    });
                });
                if let Some((_, _, scene)) = selected {
                    let width = (ui.available_width() - 16.0).min(300.0);
                    ui.add_sized(
                        [width, CONTROL_HEIGHT],
                        egui::Label::new(
                            RichText::new(&scene.name).color(ui.visuals().weak_text_color()),
                        )
                        .truncate(),
                    );
                }
            });
        });
        if let Some(notice) = &self.notice {
            page.add_space(6.0);
            page.label(
                RichText::new(notice)
                    .small()
                    .color(page.visuals().weak_text_color()),
            );
        }
        page.add_space(12.0);
        let body = Rect::from_min_max(
            page.cursor().min,
            egui::pos2(page.max_rect().right(), page.max_rect().bottom() - 32.0),
        );
        let mut body_ui = page.new_child(
            egui::UiBuilder::new()
                .id_salt("scene-library-body")
                .layout(Layout::top_down(Align::Min))
                .max_rect(body),
        );
        body_ui.set_clip_rect(body.intersect(rect));
        let query = self.query.trim().to_lowercase();
        let visible: Vec<_> = entries
            .iter()
            .enumerate()
            .filter(|(_, s)| s.name.to_lowercase().contains(&query))
            .collect();
        let results = visible.len();
        if self.list_view {
            list_header(&mut body_ui);
        }
        egui::ScrollArea::vertical()
            .id_salt(("scene-library-scroll", self.preset_tab, self.list_view))
            .auto_shrink([false, false])
            .show(&mut body_ui, |ui| {
                if visible.is_empty() {
                    empty_state(ui, self.preset_tab, !query.is_empty());
                } else {
                    let columns = if self.list_view {
                        1
                    } else {
                        ((ui.available_width() + 16.0) / 260.0).floor().max(1.0) as usize
                    };
                    let width =
                        (ui.available_width() - 16.0 * (columns - 1) as f32) / columns as f32;
                    for row in visible.chunks(columns) {
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing.x = 16.0;
                            for (index, scene) in row {
                                let key = item_key(self.preset_tab, scene.id);
                                let active = key == Selection::Scene(book.active_scene)
                                    || (self.preset_tab
                                        && book.current().is_some_and(|current| {
                                            current.name == scene.name
                                                && current
                                                    .preset
                                                    .as_ref()
                                                    .is_some_and(|p| p.id == scene.id)
                                        }));
                                let response = draw_item(
                                    ui,
                                    Item {
                                        scene,
                                        index: *index,
                                        key,
                                        selected: selection == Some(key),
                                        active,
                                    },
                                    width,
                                    self.list_view,
                                    &mut self.textures,
                                );
                                #[cfg(test)]
                                ui.data_mut(|d| {
                                    d.insert_temp(
                                        egui::Id::new(("library-item-rect", key)),
                                        response.rect,
                                    )
                                });
                                if response.clicked() || response.secondary_clicked() {
                                    selection = Some(key);
                                }
                                let available = Availability::new(
                                    key,
                                    *index,
                                    book,
                                    &self.presets,
                                    self.load_failed,
                                );
                                if response.double_clicked() && available.load {
                                    command = Some(Command::Load(key));
                                }
                                crate::dropdown::context_popup(
                                    ui,
                                    egui::Popup::context_menu(&response),
                                )
                                .show(|ui| {
                                    edit_menu(
                                        ui,
                                        key,
                                        *index,
                                        &scene.name,
                                        available,
                                        &mut command,
                                        &mut edit,
                                    )
                                });
                                if query.is_empty() && available.edit {
                                    let response = crate::workspace_ui::drag_cursor(response);
                                    if response.drag_started() {
                                        response.dnd_set_drag_payload(key);
                                    }
                                    let after = ui.input(|i| {
                                        i.pointer.interact_pos().is_some_and(|p| {
                                            if self.list_view {
                                                p.y >= response.rect.center().y
                                            } else {
                                                p.x >= response.rect.center().x
                                            }
                                        })
                                    });
                                    if let Some(source) = response.dnd_hover_payload::<Selection>()
                                        && *source != key
                                    {
                                        let rect = response.rect;
                                        let points = if self.list_view {
                                            let y = if after {
                                                rect.bottom() + 2.0
                                            } else {
                                                rect.top() - 2.0
                                            };
                                            [
                                                egui::pos2(rect.left(), y),
                                                egui::pos2(rect.right(), y),
                                            ]
                                        } else {
                                            let x = if after {
                                                rect.right() + 6.0
                                            } else {
                                                rect.left() - 6.0
                                            };
                                            [
                                                egui::pos2(x, rect.top()),
                                                egui::pos2(x, rect.bottom()),
                                            ]
                                        };
                                        ui.painter().line_segment(
                                            points,
                                            Stroke::new(2.0, ui.visuals().selection.bg_fill),
                                        );
                                    }
                                    if let Some(source) =
                                        response.dnd_release_payload::<Selection>()
                                    {
                                        command = Some(Command::Reorder(
                                            *source,
                                            *index + usize::from(after),
                                        ));
                                    }
                                }
                            }
                        });
                        ui.add_space(if self.list_view { 4.0 } else { 16.0 });
                    }
                }
            });
        let footer = Rect::from_min_max(
            egui::pos2(body.left(), body.bottom() + 8.0),
            page.max_rect().max,
        );
        ui.painter().text(
            footer.left_center(),
            egui::Align2::LEFT_CENTER,
            format!(
                "{results} {}{}",
                if self.preset_tab { "presets" } else { "scenes" },
                if query.is_empty() { "" } else { " found" }
            ),
            FontId::proportional(12.0),
            ui.visuals().weak_text_color(),
        );
        if footer.width() > 560.0 {
            ui.painter().text(
                footer.right_center(),
                egui::Align2::RIGHT_CENTER,
                "Double-click to load · Drag to reorder · Right-click for actions",
                FontId::proportional(12.0),
                ui.visuals().weak_text_color(),
            );
        }
        self.selection = selection;
        if let Some(edit) = edit {
            self.dialog = Some(match edit {
                Edit::Rename(key, name) => {
                    self.focus_dialog = true;
                    Dialog::Rename(key, name)
                }
                Edit::Delete(key, name) => Dialog::Delete(key, name),
            });
        }
        if self.dialog.is_none()
            && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            self.open = false;
        }
        command
    }
}
fn item_key(preset: bool, id: u64) -> Selection {
    if preset {
        Selection::Preset(id)
    } else {
        Selection::Scene(id)
    }
}
fn edit_menu(
    ui: &mut egui::Ui,
    key: Selection,
    index: usize,
    name: &str,
    available: Availability,
    command: &mut Option<Command>,
    edit: &mut Option<Edit>,
) {
    if ui
        .add_enabled(available.load, egui::Button::new("Load scene"))
        .clicked()
    {
        *command = Some(Command::Load(key));
        ui.close();
    }
    if ui
        .add_enabled(available.duplicate, egui::Button::new("Duplicate"))
        .clicked()
    {
        *command = Some(Command::Duplicate(key));
        ui.close();
    }
    if ui
        .add_enabled(available.edit, egui::Button::new("Rename…"))
        .clicked()
    {
        *edit = Some(Edit::Rename(key, name.into()));
        ui.close();
    }
    if matches!(key, Selection::Scene(_))
        && ui
            .add_enabled(available.refresh, egui::Button::new("Refresh preview"))
            .help_text(
                "Capture the active scene. Load another scene before refreshing its preview.",
            )
            .clicked()
    {
        *command = Some(Command::Refresh);
        ui.close();
    }
    crate::dropdown::menu_separator(ui);
    if ui
        .add_enabled(available.earlier, egui::Button::new("Move earlier"))
        .clicked()
    {
        *command = Some(Command::Reorder(key, index - 1));
        ui.close();
    }
    if ui
        .add_enabled(available.later, egui::Button::new("Move later"))
        .clicked()
    {
        *command = Some(Command::Reorder(key, index + 2));
        ui.close();
    }
    crate::dropdown::menu_separator(ui);
    if ui
        .add_enabled(available.delete, egui::Button::new("Delete…"))
        .clicked()
    {
        *edit = Some(Edit::Delete(key, name.into()));
        ui.close();
    }
}
fn macro_count(scene: &Scene) -> usize {
    scene.state.macros.as_ref().map_or(0, |m| {
        m.slots.iter().filter(|s| s.parameter.is_some()).count()
    })
}
fn component_names(scene: &Scene) -> String {
    scene.state.layout.as_ref().map_or_else(
        || "Empty scene".into(),
        |layout| {
            layout
                .ids()
                .into_iter()
                .filter_map(|i| scene.state.panes.get(i))
                .map(|p| p.kind.label())
                .collect::<Vec<_>>()
                .join(", ")
        },
    )
}
struct Item<'a> {
    scene: &'a Scene,
    index: usize,
    key: Selection,
    selected: bool,
    active: bool,
}
fn draw_item(
    ui: &mut egui::Ui,
    item: Item<'_>,
    width: f32,
    list: bool,
    textures: &mut HashMap<Selection, egui::TextureHandle>,
) -> Response {
    let Item {
        scene,
        index,
        key,
        selected,
        active,
    } = item;
    let height = if list {
        ROW_HEIGHT
    } else {
        (width - 16.0) * 0.5625 + 104.0
    };
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(width, height), Sense::click_and_drag());
    let base = ui.visuals().widgets.inactive.bg_fill;
    let fill = if response.hovered() {
        ui.visuals().widgets.hovered.bg_fill
    } else {
        base
    };
    ui.painter().rect_filled(rect, 8.0, fill);
    if selected {
        ui.painter().rect_stroke(
            rect,
            8.0,
            Stroke::new(1.5, ui.visuals().selection.bg_fill),
            egui::StrokeKind::Inside,
        );
    }
    let preview = if list {
        Rect::from_min_size(rect.min + egui::vec2(40.0, 8.0), egui::vec2(92.0, 52.0))
    } else {
        Rect::from_min_size(
            rect.min + Vec2::splat(8.0),
            egui::vec2(width - 16.0, (width - 16.0) * 0.5625),
        )
    };
    paint_preview(ui, preview, scene, key, textures);
    let muted = ui.visuals().weak_text_color();
    if list {
        ui.painter().text(
            egui::pos2(rect.left() + 18.0, rect.center().y),
            egui::Align2::CENTER_CENTER,
            format!("{:02}", index + 1),
            FontId::proportional(12.0),
            muted,
        );
        let name_left = rect.left() + 146.0;
        let detailed = width >= 740.0;
        let name_right = if detailed {
            rect.right() - 360.0
        } else {
            rect.right() - 80.0
        };
        paint_text(
            ui,
            Rect::from_min_max(
                egui::pos2(name_left, rect.top() + 12.0),
                egui::pos2(name_right, rect.top() + 34.0),
            ),
            &scene.name,
            14.0,
            ui.visuals().text_color(),
        );
        paint_text(
            ui,
            Rect::from_min_max(
                egui::pos2(name_left, rect.top() + 37.0),
                egui::pos2(name_right, rect.bottom() - 8.0),
            ),
            &component_names(scene),
            12.0,
            muted,
        );
        if detailed {
            for (offset, count) in [
                (310.0, scene.state.signals.channels.len()),
                (210.0, macro_count(scene)),
            ] {
                ui.painter().text(
                    egui::pos2(rect.right() - offset, rect.center().y),
                    egui::Align2::CENTER_CENTER,
                    count.to_string(),
                    FontId::proportional(13.0),
                    muted,
                );
            }
        }
        if active {
            badge(
                ui,
                Rect::from_center_size(
                    egui::pos2(rect.right() - 51.0, rect.center().y),
                    egui::vec2(66.0, 22.0),
                ),
                "Active",
            );
        }
    } else {
        let title = Rect::from_min_max(
            egui::pos2(rect.left() + 12.0, preview.bottom() + 11.0),
            egui::pos2(rect.right() - 12.0, preview.bottom() + 33.0),
        );
        paint_text(
            ui,
            title,
            &format!("{:02}  {}", index + 1, scene.name),
            14.0,
            ui.visuals().text_color(),
        );
        paint_text(
            ui,
            title.translate(egui::vec2(0.0, 24.0)),
            &component_names(scene),
            12.0,
            muted,
        );
        paint_text(
            ui,
            title.translate(egui::vec2(0.0, 46.0)),
            &format!(
                "{} signals · {} macros",
                scene.state.signals.channels.len(),
                macro_count(scene)
            ),
            12.0,
            muted,
        );
        if active {
            badge(
                ui,
                Rect::from_min_size(preview.min + Vec2::splat(8.0), egui::vec2(66.0, 22.0)),
                "Active",
            );
        }
    }
    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .help_text(format!(
            "{}\n{}\nDouble-click to load this scene.",
            scene.name,
            component_names(scene)
        ))
}
fn paint_preview(
    ui: &egui::Ui,
    rect: Rect,
    scene: &Scene,
    key: Selection,
    textures: &mut HashMap<Selection, egui::TextureHandle>,
) {
    ui.painter()
        .rect_filled(rect, 5.0, ui.visuals().extreme_bg_color);
    if let Some(preview) = &scene.preview {
        let texture = textures.entry(key).or_insert_with(|| {
            ui.ctx().load_texture(
                format!(
                    "scene-preview-{}-{}",
                    matches!(key, Selection::Preset(_)),
                    scene.id
                ),
                preview.image(),
                Default::default(),
            )
        });
        let size = egui::vec2(preview.width as f32, preview.height as f32);
        let fit = (rect.width() / size.x).min(rect.height() / size.y);
        ui.painter().image(
            texture.id(),
            Rect::from_center_size(rect.center(), size * fit),
            Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    } else {
        let center = if rect.width() > 140.0 {
            rect.center() - egui::vec2(0.0, 9.0)
        } else {
            rect.center()
        };
        icons::paint(
            ui,
            Rect::from_center_size(center, Vec2::splat(22.0)),
            Icon::Surface,
            ui.visuals().weak_text_color().gamma_multiply(0.4),
        );
        if rect.width() > 140.0 {
            ui.painter().text(
                center + egui::vec2(0.0, 25.0),
                egui::Align2::CENTER_CENTER,
                "Preview not captured",
                FontId::proportional(12.0),
                ui.visuals().weak_text_color(),
            );
        }
    }
}
fn badge(ui: &egui::Ui, rect: Rect, label: &str) {
    ui.painter()
        .rect_filled(rect, 5.0, ui.visuals().extreme_bg_color);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        FontId::proportional(11.0),
        ui.visuals().selection.bg_fill,
    );
}
fn paint_text(ui: &egui::Ui, rect: Rect, text: &str, size: f32, color: Color32) {
    let mut job = egui::text::LayoutJob::simple(
        text.into(),
        FontId::proportional(size),
        color,
        rect.width().max(1.0),
    );
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    job.wrap.overflow_character = Some('…');
    let galley = ui.painter().layout_job(job);
    ui.painter()
        .with_clip_rect(rect.intersect(ui.clip_rect()))
        .galley(
            rect.left_center() - egui::vec2(0.0, galley.size().y * 0.5),
            galley,
            color,
        );
}
fn list_header(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 26.0), Sense::hover());
    let muted = ui.visuals().weak_text_color();
    ui.painter().text(
        rect.min + egui::vec2(146.0, 13.0),
        egui::Align2::LEFT_CENTER,
        "SCENE",
        FontId::proportional(11.0),
        muted,
    );
    if rect.width() >= 740.0 {
        for (offset, label) in [(310.0, "SIGNALS"), (210.0, "MACROS"), (51.0, "STATUS")] {
            ui.painter().text(
                egui::pos2(rect.right() - offset, rect.center().y),
                egui::Align2::CENTER_CENTER,
                label,
                FontId::proportional(11.0),
                muted,
            );
        }
    }
}
fn empty_state(ui: &mut egui::Ui, preset: bool, search: bool) {
    ui.add_space(60.0);
    ui.vertical_centered(|ui| {
        ui.heading(if search {
            "No matching scenes"
        } else {
            "Your preset collection starts here"
        });
        ui.add_space(8.0);
        ui.label(if search {
            "Try another name or clear your search."
        } else if preset {
            "Use the Save icon in the workspace to capture a scene and all its mappings."
        } else {
            "Create a scene to get started."
        });
    });
}
