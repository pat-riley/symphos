//! Workspace chrome: a growing component library and one searchable preset browser.
use crate::{
    help::HoverHelp,
    icons::{self, Icon},
    modules::ModuleKind,
    theme::AppTheme,
    workspace::{self, Preset, PresetContent, Storage},
};
use eframe::egui::{self, Align, Color32, FontId, Layout, Rect, RichText, Sense, Stroke, Vec2};

pub const NAV_HEIGHT: f32 = 56.0;

pub const LIBRARY_WIDTH: f32 = 236.0;

fn sidebar_blend(a: Color32, b: Color32, amount: f32) -> Color32 {
    let channel = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * amount) as u8;
    Color32::from_rgb(
        channel(a.r(), b.r()),
        channel(a.g(), b.g()),
        channel(a.b(), b.b()),
    )
}

pub fn sidebar_style(ui: &mut egui::Ui, theme: &AppTheme) {
    ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
    ui.spacing_mut().slider_width = 90.0;
    ui.style_mut()
        .text_styles
        .insert(egui::TextStyle::Body, FontId::proportional(13.0));
    ui.style_mut()
        .text_styles
        .insert(egui::TextStyle::Button, FontId::proportional(13.0));
    ui.spacing_mut().button_padding = Vec2::new(6.0, 3.0);
    ui.spacing_mut().interact_size.y = 28.0;
    ui.spacing_mut().indent = 16.0;
    let field = if theme.dark_mode {
        sidebar_blend(theme.panel, Color32::BLACK, 0.24)
    } else {
        sidebar_blend(theme.panel, theme.foreground, 0.07)
    };
    let visuals = ui.visuals_mut();
    visuals.collapsing_header_frame = false;
    visuals.extreme_bg_color = field;
    visuals.widgets.inactive.bg_fill = field;
    visuals.widgets.inactive.weak_bg_fill = field;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, theme.foreground.gamma_multiply(0.10));
    visuals.widgets.noninteractive.bg_stroke =
        Stroke::new(1.0, theme.foreground.gamma_multiply(0.18));
    for widget in [
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.corner_radius = egui::CornerRadius::same(3);
    }
}

pub fn sidebar_header(
    ui: &mut egui::Ui,
    icon: Icon,
    title: &str,
    subtitle: &str,
    metadata: &str,
    description: &str,
) -> egui::Response {
    egui::Frame::new()
        .inner_margin(12)
        .show(ui, |ui| {
            let close = ui
                .horizontal_top(|ui| {
                    let (badge, _) = ui.allocate_exact_size(Vec2::splat(32.0), Sense::hover());
                    ui.painter().rect_filled(
                        badge,
                        5.0,
                        ui.visuals().widgets.noninteractive.bg_fill,
                    );
                    icons::paint(ui, badge, icon, ui.visuals().text_color());
                    ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                        ui.visuals_mut().button_frame = false;
                        let close = icons::sized_button(ui, Icon::Close, false, 24.0, description);
                        close.widget_info(|| {
                            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, description)
                        });
                        ui.with_layout(Layout::top_down(Align::Min), |ui| {
                            ui.add(
                                egui::Label::new(RichText::new(title).size(15.0).strong())
                                    .truncate(),
                            );
                            ui.add(
                                egui::Label::new(RichText::new(subtitle).size(11.0).weak())
                                    .truncate(),
                            );
                        });
                        close
                    })
                    .inner
                })
                .inner;
            ui.add_space(2.0);
            ui.label(RichText::new(metadata).size(11.0).weak());
            close
        })
        .inner
}

pub fn sidebar_rule(ui: &mut egui::Ui, bounds: Rect) {
    let (row, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
    let mut clip = ui.clip_rect();
    clip.min.x = bounds.left();
    clip.max.x = bounds.right();
    ui.painter().with_clip_rect(clip).hline(
        bounds.x_range(),
        row.center().y,
        Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color),
    );
}

fn navigation_color(theme: &AppTheme) -> Color32 {
    // Match the dark macOS title-bar surface, while retaining Linux's theme palette.
    #[cfg(target_os = "macos")]
    if theme.dark_mode {
        return Color32::from_gray(31);
    }
    theme.card
}

pub fn navigation_surface(ui: &egui::Ui, rect: Rect, theme: &AppTheme) {
    let color = navigation_color(theme);
    ui.painter().rect_filled(rect, 0.0, color);
    ui.painter().hline(
        rect.x_range(),
        rect.bottom(),
        Stroke::new(1.0, theme.foreground.gamma_multiply(0.14)),
    );
}

pub fn navigation_region(ui: &mut egui::Ui, rect: Rect, id: &str, theme: &AppTheme) -> egui::Ui {
    let mut child = ui.new_child(egui::UiBuilder::new().id_salt(id).max_rect(rect));
    let field = if theme.dark_mode {
        Color32::from_rgb(
            theme.background.r().saturating_sub(4),
            theme.background.g().saturating_sub(4),
            theme.background.b().saturating_sub(4),
        )
    } else {
        theme.background
    };
    child.spacing_mut().interact_size.y = 28.0;
    child.spacing_mut().button_padding = Vec2::new(8.0, 5.0);
    let visuals = child.visuals_mut();
    visuals.widgets.inactive.bg_fill = field;
    visuals.widgets.inactive.weak_bg_fill = field;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, theme.muted.gamma_multiply(0.65));
    visuals.widgets.inactive.fg_stroke.color = theme.foreground;
    visuals.widgets.hovered.bg_fill = theme.selection;
    visuals.widgets.hovered.weak_bg_fill = theme.selection;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, theme.accent);
    visuals.widgets.hovered.fg_stroke.color = theme.foreground;
    visuals.widgets.active.bg_fill = theme.selection;
    visuals.widgets.active.weak_bg_fill = theme.selection;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, theme.accent);
    visuals.widgets.active.fg_stroke.color = theme.foreground;
    visuals.selection.bg_fill = theme.selection;
    visuals.selection.stroke = Stroke::new(1.0, theme.foreground);
    child
}

#[derive(Clone, Copy, Debug)]
pub enum DragPayload {
    Component(ModuleKind),
    Pane(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    View,
    Layout,
    Component,
}
impl Scope {
    fn label(self) -> &'static str {
        match self {
            Self::View => "Full views",
            Self::Layout => "Layouts",
            Self::Component => "Components",
        }
    }
    fn matches(self, content: &PresetContent) -> bool {
        matches!(
            (self, content),
            (Self::View, PresetContent::View { .. })
                | (Self::Layout, PresetContent::Layout(_))
                | (Self::Component, PresetContent::Component(_))
        )
    }
}
pub enum Action {
    Apply(String),
    Add(ModuleKind),
    Save(String, Scope),
    Rename(String, String),
    Delete(String),
}
struct SaveDialog {
    name: String,
    scope: Scope,
}
#[derive(Default)]
pub struct Manager {
    pub library_open: bool,
    component_search: String,
    category: usize,
    preset_search: String,
    scope: Option<Scope>,
    save: Option<SaveDialog>,
    rename: Option<(String, String)>,
    delete: Option<String>,
}
impl Manager {
    pub fn modal_open(&self) -> bool {
        self.save.is_some() || self.rename.is_some() || self.delete.is_some()
    }
    pub fn left_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_centered(|ui| {
            if ui.add(egui::Button::new("New component").selected(self.library_open))
                .help_text("Browse, search, and drag visualizations into the workspace. Click a component to add beside the selected pane.").clicked() {
                self.library_open = !self.library_open;
            }
        });
    }
    pub fn preset_toolbar(
        &mut self,
        ui: &mut egui::Ui,
        storage: &Storage,
        selected_kind: ModuleKind,
        has_panes: bool,
    ) -> Vec<Action> {
        let mut actions = Vec::new();
        let current = storage.presets.iter().find(|p| p.id == storage.active);
        let current_scope = current
            .map(|p| match p.content {
                PresetContent::Layout(_) => Scope::Layout,
                PresetContent::View { .. } => Scope::View,
                PresetContent::Component(_) => Scope::Component,
            })
            .unwrap_or(Scope::View);
        let cycling: Vec<_> = storage
            .presets
            .iter()
            .filter(|p| {
                current_scope.matches(&p.content)
                    && match &p.content {
                        PresetContent::Component(settings) => {
                            has_panes && settings.kind() == selected_kind
                        }
                        _ => true,
                    }
            })
            .collect();
        let save_width = ui
            .painter()
            .layout_no_wrap(
                "Save".into(),
                egui::TextStyle::Button.resolve(ui.style()),
                ui.visuals().text_color(),
            )
            .size()
            .x
            + 2.0 * ui.spacing().button_padding.x;
        let save_width = save_width.max(45.0);
        let spacing = 3.0 * ui.spacing().item_spacing.x;
        let picker_width = (ui.available_width() - 56.0 - save_width - spacing).clamp(110.0, 340.0);
        let row_width = picker_width + 56.0 + save_width + spacing;
        // Center the actual visible controls, rather than their wider container.
        let row = Rect::from_center_size(
            ui.max_rect().center(),
            Vec2::new(row_width, ui.available_height()),
        );
        let mut row_ui = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("preset-controls")
                .max_rect(row),
        );
        row_ui.horizontal_centered(|ui| {
            if ui.add_enabled(!cycling.is_empty(), egui::Button::new("‹").min_size(Vec2::splat(28.0))).help_text("Previous preset in this category.").clicked() {
                actions.push(Action::Apply(cycle(&cycling, &storage.active, -1)));
            }
            let label = format!("{}{}", current.map_or("Custom view", |p| p.name.as_str()), if storage.modified { " *" } else { "" });
            let button = ui.add_sized([picker_width, 28.0], egui::Button::new(label).truncate()).help_text("Browse full views, layouts, and component presets. * means the current view has edits.");
            let arrow = button.rect.right_center() - Vec2::new(11.0, 0.0);
            ui.painter().add(egui::Shape::convex_polygon(vec![arrow + Vec2::new(-3.5, -2.0), arrow + Vec2::new(3.5, -2.0), arrow + Vec2::new(0.0, 2.5)], ui.visuals().text_color(), Stroke::NONE));
            if button.clicked() { self.scope = Some(current_scope); }
            egui::Popup::menu(&button).show(|ui| {
                ui.set_width(390.0);
                let scope = self.scope.get_or_insert(current_scope);
                ui.horizontal(|ui| {
                    for tab in [Scope::View, Scope::Layout, Scope::Component] { ui.selectable_value(scope, tab, tab.label()); }
                });
                ui.add(egui::TextEdit::singleline(&mut self.preset_search).hint_text("Search presets…").desired_width(f32::INFINITY));
                ui.separator();
                let query = self.preset_search.trim().to_lowercase();
                let candidates: Vec<_> = storage.presets.iter().filter(|p| scope.matches(&p.content) && p.name.to_lowercase().contains(&query)).collect();
                egui::ScrollArea::vertical().max_height(340.0).show(ui, |ui| {
                    if candidates.is_empty() { ui.weak("No saved presets in this category. Use Save to create one."); }
                    for preset in candidates {
                        let compatible = match &preset.content { PresetContent::Component(settings) => has_panes && settings.kind() == selected_kind, _ => true };
                        ui.push_id(&preset.id, |ui| {
                            ui.horizontal(|ui| {
                                if let Some(tree) = preset.content.tree() {
                                    let (_, rect) = ui.allocate_space(Vec2::new(44.0, 30.0));
                                    thumbnail(ui, tree, rect, ui.visuals().text_color());
                                }
                                let mut text = preset.name.clone();
                                if let PresetContent::Component(settings) = &preset.content { text.push_str(&format!(" · {}", settings.kind().label())); }
                                let width = ui.available_width() - if preset.factory() { 0.0 } else { 34.0 };
                                let response = ui.add_enabled_ui(compatible, |ui| ui.add_sized([width, 30.0], egui::Button::selectable(storage.active == preset.id, text).truncate())).inner;
                                if response.clicked() { actions.push(Action::Apply(preset.id.clone())); ui.close(); }
                                if !compatible { response.on_hover_text("Select a pane with this component type to apply its preset."); }
                                if !preset.factory() && ui.small_button("···").clicked() { self.rename = Some((preset.id.clone(), preset.name.clone())); ui.close(); }
                            });
                        });
                    }
                });
                ui.separator();
                ui.small(match scope {
                    Scope::Layout => "Pane count, placement, and sizing. Keeps current components.",
                    Scope::View => "Layout, component choices, and their settings.",
                    Scope::Component => "Settings for the selected component. Keeps the layout.",
                });
            });
            if ui.add_enabled(!cycling.is_empty(), egui::Button::new("›").min_size(Vec2::splat(28.0))).help_text("Next preset in this category.").clicked() {
                actions.push(Action::Apply(cycle(&cycling, &storage.active, 1)));
            }
            if ui.add_enabled(has_panes, egui::Button::new("Save").min_size(Vec2::new(save_width, 28.0))).clicked() {
                self.save = Some(SaveDialog { name: String::new(), scope: Scope::Layout });
            }
        });
        actions
    }
    pub fn library(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        theme: &AppTheme,
        can_add: bool,
    ) -> Vec<Action> {
        let mut actions = Vec::new();
        ui.painter().rect_filled(rect, 0.0, theme.panel);
        ui.painter().vline(
            rect.right(),
            rect.y_range(),
            Stroke::new(1.0, theme.foreground.gamma_multiply(0.12)),
        );
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("component-library")
                .max_rect(rect),
        );
        child.set_clip_rect(rect.intersect(ui.clip_rect()));
        sidebar_style(&mut child, theme);
        if sidebar_header(
            &mut child,
            Icon::Plus,
            "New component",
            "Audio visualizations",
            "Click to add · Drag to arrange",
            "Close the component browser",
        )
        .clicked()
        {
            self.library_open = false;
        }
        sidebar_rule(&mut child, rect);
        egui::Frame::new().inner_margin(12).show(&mut child, |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.component_search)
                    .hint_text("Search components…")
                    .desired_width(f32::INFINITY),
            );
            ui.horizontal_wrapped(|ui| {
                for (i, label) in ["All", "Frequency", "Time", "Stereo"]
                    .into_iter()
                    .enumerate()
                {
                    ui.selectable_value(&mut self.category, i, label);
                }
            });
        });
        let query = self.component_search.trim().to_lowercase();
        let mut count = 0;
        egui::ScrollArea::vertical().id_salt("component-results").show(&mut child, |ui| {
            for kind in ModuleKind::ALL {
                let category = match kind { ModuleKind::Waterfall | ModuleKind::Spectrum | ModuleKind::Spectrogram => 1,
                    ModuleKind::Waveform | ModuleKind::Oscilloscope => 2, ModuleKind::Stereometer => 3 };
                let caption = match kind { ModuleKind::Waterfall => "Frequency · 3D history", ModuleKind::Spectrum => "Frequency · Live levels",
                    ModuleKind::Spectrogram => "Frequency · Color history", ModuleKind::Waveform => "Time · Amplitude history",
                    ModuleKind::Oscilloscope => "Time · Stabilized cycles", ModuleKind::Stereometer => "Stereo · Width and phase" };
                if (self.category != 0 && self.category != category)
                    || !format!("{} {caption}", kind.label()).to_lowercase().contains(&query) { continue; }
                count += 1;
                ui.push_id(kind.label(), |ui| {
                    sidebar_rule(ui, rect);
                    let (row, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 68.0), Sense::click_and_drag());
                    if response.hovered() || response.dragged() { ui.painter().rect_filled(row, 0.0, theme.card); }
                    let badge = Rect::from_center_size(egui::pos2(row.left() + 28.0, row.center().y), Vec2::splat(32.0));
                    ui.painter().rect_filled(badge, 5.0, theme.card);
                    icons::paint(ui, badge, kind.icon(), theme.foreground);
                    let labels = Rect::from_min_max(egui::pos2(badge.right() + 8.0, row.top() + 14.0),
                        egui::pos2(row.right() - 30.0, row.bottom() - 10.0));
                    // Paint labels without child widgets so the whole row keeps
                    // its click/drag interaction, including directly over text.
                    for (label, size, color, offset) in [(kind.label(), 13.0, theme.foreground, 0.0),
                        (caption, 11.0, theme.muted, 23.0)] {
                        let mut job = egui::text::LayoutJob::simple(label.into(), FontId::proportional(size), color, labels.width());
                        job.wrap.max_rows = 1;
                        let galley = ui.painter().layout_job(job);
                        ui.painter_at(labels).galley(labels.min + Vec2::new(0.0, offset), galley, color);
                    }
                    icons::paint(ui, Rect::from_center_size(egui::pos2(row.right() - 18.0, row.center().y), Vec2::splat(20.0)), Icon::Plus, theme.muted);
                    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, format!("Add {}", kind.label())));
                    response.dnd_set_drag_payload(DragPayload::Component(kind));
                    if response.clicked() && can_add { actions.push(Action::Add(kind)); }
                    response.help_text(format!("{} Drag to a pane edge to add, or its center to replace. Click to add beside the selected pane.", kind.description()));
                });
            }
            sidebar_rule(ui, rect);
            egui::Frame::new().inner_margin(12).show(ui, |ui| {
                if count == 0 { ui.weak("No components match your search."); }
                ui.small(if can_add { "Drag to a pane edge to split, or its center to replace." }
                    else { "16 panes are open. Close one to add, or drag to replace." });
            });
        });
        actions
    }
    pub fn dialogs(
        &mut self,
        ctx: &egui::Context,
        storage: &Storage,
        selected_kind: ModuleKind,
    ) -> Vec<Action> {
        let mut actions = Vec::new();
        let mut finish = false;
        if let Some(save) = &mut self.save {
            let response =
                egui::Modal::new(egui::Id::new("save-workspace-preset")).show(ctx, |ui| {
                    ui.set_width(360.0);
                    ui.heading("Save preset");
                    ui.add(
                        egui::TextEdit::singleline(&mut save.name)
                            .hint_text("Preset name")
                            .desired_width(f32::INFINITY),
                    );
                    ui.radio_value(
                        &mut save.scope,
                        Scope::Layout,
                        "Layout only — pane placement and sizes",
                    );
                    ui.radio_value(
                        &mut save.scope,
                        Scope::View,
                        "Full view — layout and component settings",
                    );
                    ui.radio_value(
                        &mut save.scope,
                        Scope::Component,
                        format!("Component — {} settings", selected_kind.label()),
                    );
                    let name = save.name.trim();
                    let duplicate = storage.presets.iter().any(|p| {
                        p.name.eq_ignore_ascii_case(name) && save.scope.matches(&p.content)
                    });
                    if duplicate {
                        ui.colored_label(
                            ui.visuals().warn_fg_color,
                            "This name is already used in that category.",
                        );
                    }
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(
                                workspace::valid_name(name)
                                    && !duplicate
                                    && storage.presets.iter().filter(|p| !p.factory()).count()
                                        < 256,
                                egui::Button::new("Save preset"),
                            )
                            .clicked()
                        {
                            actions.push(Action::Save(name.into(), save.scope));
                            finish = true;
                        }
                        if ui.button("Cancel").clicked() {
                            finish = true;
                        }
                    });
                });
            if response.should_close() {
                finish = true;
            }
        }
        if finish {
            self.save = None;
        }
        finish = false;
        if let Some((id, name)) = &mut self.rename {
            let response =
                egui::Modal::new(egui::Id::new("manage-workspace-preset")).show(ctx, |ui| {
                    ui.set_width(320.0);
                    ui.heading("Manage preset");
                    ui.add(egui::TextEdit::singleline(name).desired_width(f32::INFINITY));
                    let same_scope = storage
                        .presets
                        .iter()
                        .find(|p| p.id == *id)
                        .map(|p| p.content.label());
                    let duplicate = storage.presets.iter().any(|p| {
                        p.id != *id
                            && p.name.eq_ignore_ascii_case(name.trim())
                            && Some(p.content.label()) == same_scope
                    });
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(
                                workspace::valid_name(name.trim()) && !duplicate,
                                egui::Button::new("Rename"),
                            )
                            .clicked()
                        {
                            actions.push(Action::Rename(id.clone(), name.trim().into()));
                            finish = true;
                        }
                        if ui.button("Delete…").clicked() {
                            self.delete = Some(id.clone());
                            finish = true;
                        }
                        if ui.button("Cancel").clicked() {
                            finish = true;
                        }
                    });
                });
            if response.should_close() {
                finish = true;
            }
        }
        if finish {
            self.rename = None;
        }
        finish = false;
        if let Some(id) = &self.delete {
            let response =
                egui::Modal::new(egui::Id::new("delete-workspace-preset")).show(ctx, |ui| {
                    ui.heading("Delete preset?");
                    ui.label("The current workspace will stay as it is.");
                    ui.horizontal(|ui| {
                        if ui.button("Delete preset").clicked() {
                            actions.push(Action::Delete(id.clone()));
                            finish = true;
                        }
                        if ui.button("Cancel").clicked() {
                            finish = true;
                        }
                    });
                });
            if response.should_close() {
                finish = true;
            }
        }
        if finish {
            self.delete = None;
        }
        actions
    }
}
fn cycle(presets: &[&Preset], active: &str, delta: isize) -> String {
    let current = presets.iter().position(|p| p.id == active).unwrap_or(0);
    let next = (current as isize + delta).rem_euclid(presets.len() as isize) as usize;
    presets[next].id.clone()
}
fn thumbnail(ui: &egui::Ui, tree: &workspace::Tile, rect: Rect, color: Color32) {
    for (_, cell) in tree.geometry(rect).0 {
        ui.painter()
            .rect_filled(cell, 1.0, color.gamma_multiply(0.18));
        ui.painter()
            .rect_stroke(cell, 1.0, Stroke::new(0.7, color), egui::StrokeKind::Inside);
    }
}

pub fn drag_grip(ui: &mut egui::Ui, id: usize) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(18.0, 24.0), Sense::drag());
    let color = ui.visuals().weak_text_color();
    for x in [-2.5, 2.5] {
        for y in [-5.0, 0.0, 5.0] {
            ui.painter()
                .circle_filled(rect.center() + Vec2::new(x, y), 1.1, color);
        }
    }
    response.dnd_set_drag_payload(DragPayload::Pane(id));
    response.on_hover_cursor(egui::CursorIcon::Grab).help_text(
        "Drag this pane to an edge to move it, or onto another pane's center to swap positions.",
    )
}
pub fn drop_hint(ui: &egui::Ui, rect: Rect, message: &str, accent: Color32) {
    let painter = ui.painter_at(ui.clip_rect());
    painter.rect_filled(rect, 6.0, accent.gamma_multiply(0.22));
    painter.rect_stroke(
        rect,
        6.0,
        Stroke::new(2.0, accent),
        egui::StrokeKind::Inside,
    );
    let label = painter.layout_no_wrap(message.into(), FontId::proportional(12.0), Color32::WHITE);
    let size = label.size() + Vec2::new(16.0, 10.0);
    let backing = Rect::from_center_size(rect.center(), size);
    painter.rect_filled(backing, 4.0, Color32::from_black_alpha(225));
    painter.galley(backing.min + Vec2::new(8.0, 5.0), label, Color32::WHITE);
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::Pos2;
    #[test]
    fn preset_navigation_wraps_within_the_filtered_category() {
        let presets = workspace::factory_presets();
        let views: Vec<_> = presets
            .iter()
            .filter(|p| Scope::View.matches(&p.content))
            .collect();
        assert_eq!(cycle(&views, &views[0].id, -1), views.last().unwrap().id);
        assert_eq!(cycle(&views, &views.last().unwrap().id, 1), views[0].id);
    }
    #[test]
    fn drop_targets_are_unambiguous_at_edges_and_center() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::splat(400.0));
        for (point, expected) in [
            (Pos2::new(5.0, 200.0), workspace::Edge::Left),
            (Pos2::new(395.0, 200.0), workspace::Edge::Right),
            (Pos2::new(200.0, 5.0), workspace::Edge::Top),
            (Pos2::new(200.0, 395.0), workspace::Edge::Bottom),
            (rect.center(), workspace::Edge::Center),
        ] {
            assert_eq!(workspace::drop_zone(rect, point).0, expected);
        }
    }
}
